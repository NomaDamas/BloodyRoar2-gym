#[cfg(not(target_os = "macos"))]
use crate::native::sound::queue::SharedStereoQueue;

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::native::sound::queue::SharedStereoQueue;

    type AudioQueueRef = *mut c_void;
    type OSStatus = i32;

    const NO_ERR: OSStatus = 0;
    const AUDIO_FORMAT_LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
    #[cfg(any(test, target_endian = "big"))]
    const AUDIO_FORMAT_FLAG_IS_BIG_ENDIAN: u32 = 1 << 1;
    const AUDIO_FORMAT_FLAG_IS_SIGNED_INTEGER: u32 = 1 << 2;
    const AUDIO_FORMAT_FLAG_IS_PACKED: u32 = 1 << 3;
    #[cfg(target_endian = "big")]
    const AUDIO_FORMAT_FLAG_IS_NATIVE_ENDIAN: u32 = AUDIO_FORMAT_FLAG_IS_BIG_ENDIAN;
    #[cfg(target_endian = "little")]
    const AUDIO_FORMAT_FLAG_IS_NATIVE_ENDIAN: u32 = 0;
    const BUFFER_COUNT: usize = 6;
    const BUFFER_FRAMES: usize = 512;
    const CHANNELS: usize = 2;
    const BYTES_PER_SAMPLE: usize = 2;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AudioStreamBasicDescription {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    #[repr(C)]
    struct AudioQueueBuffer {
        audio_data_bytes_capacity: u32,
        audio_data: *mut c_void,
        audio_data_byte_size: u32,
        user_data: *mut c_void,
        packet_description_capacity: u32,
        packet_descriptions: *mut c_void,
        packet_description_count: u32,
    }

    type AudioQueueBufferRef = *mut AudioQueueBuffer;
    type AudioQueueOutputCallback = extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef);

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioQueueNewOutput(
            format: *const AudioStreamBasicDescription,
            callback: AudioQueueOutputCallback,
            user_data: *mut c_void,
            callback_run_loop: *mut c_void,
            callback_run_loop_mode: *const c_void,
            flags: u32,
            out_queue: *mut AudioQueueRef,
        ) -> OSStatus;
        fn AudioQueueAllocateBuffer(
            queue: AudioQueueRef,
            buffer_byte_size: u32,
            out_buffer: *mut AudioQueueBufferRef,
        ) -> OSStatus;
        fn AudioQueueEnqueueBuffer(
            queue: AudioQueueRef,
            buffer: AudioQueueBufferRef,
            packet_description_count: u32,
            packet_descriptions: *const c_void,
        ) -> OSStatus;
        fn AudioQueueStart(queue: AudioQueueRef, start_time: *const c_void) -> OSStatus;
        fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> OSStatus;
        fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> OSStatus;
    }

    struct CallbackState {
        queue: SharedStereoQueue,
        running: AtomicBool,
        realtime_started: AtomicBool,
    }

    fn stream_description(sample_rate: u32) -> AudioStreamBasicDescription {
        AudioStreamBasicDescription {
            sample_rate: sample_rate as f64,
            format_id: AUDIO_FORMAT_LINEAR_PCM,
            format_flags: AUDIO_FORMAT_FLAG_IS_SIGNED_INTEGER
                | AUDIO_FORMAT_FLAG_IS_PACKED
                | AUDIO_FORMAT_FLAG_IS_NATIVE_ENDIAN,
            bytes_per_packet: coreaudio_buffer_byte_size(1),
            frames_per_packet: 1,
            bytes_per_frame: coreaudio_buffer_byte_size(1),
            channels_per_frame: CHANNELS as u32,
            bits_per_channel: (BYTES_PER_SAMPLE * 8) as u32,
            reserved: 0,
        }
    }

    fn coreaudio_buffer_byte_size(frames: usize) -> u32 {
        let bytes = frames
            .saturating_mul(CHANNELS)
            .saturating_mul(BYTES_PER_SAMPLE);
        u32::try_from(bytes).unwrap_or(u32::MAX)
    }

    extern "C" fn output_callback(
        user_data: *mut c_void,
        queue: AudioQueueRef,
        buffer: AudioQueueBufferRef,
    ) {
        if user_data.is_null() || buffer.is_null() {
            return;
        }

        unsafe {
            let state = &*(user_data as *const CallbackState);
            if !state.running.load(Ordering::Acquire) {
                return;
            }
            let buffer_ref = &mut *buffer;
            if buffer_ref.audio_data.is_null() {
                buffer_ref.audio_data_byte_size = 0;
                return;
            }
            let frames =
                (buffer_ref.audio_data_bytes_capacity as usize) / (CHANNELS * BYTES_PER_SAMPLE);
            if frames == 0 {
                buffer_ref.audio_data_byte_size = 0;
                return;
            }
            let samples = std::slice::from_raw_parts_mut(
                buffer_ref.audio_data as *mut i16,
                frames * CHANNELS,
            );
            state.queue.pop_interleaved_i16_realtime(samples);
            if state.realtime_started.load(Ordering::Acquire) {
                state.queue.record_coreaudio_callback(frames);
            }
            buffer_ref.audio_data_byte_size = (samples.len() * BYTES_PER_SAMPLE) as u32;
            let status = AudioQueueEnqueueBuffer(queue, buffer, 0, ptr::null());
            if status != NO_ERR {
                state.queue.record_coreaudio_error(status);
                state.running.store(false, Ordering::Release);
            }
        }
    }

    #[derive(Debug)]
    pub struct CoreAudioOutput {
        queue: AudioQueueRef,
        _buffers: Vec<AudioQueueBufferRef>,
        state: *mut CallbackState,
    }

    impl CoreAudioOutput {
        pub fn start(queue: SharedStereoQueue, sample_rate: u32) -> Result<Self, String> {
            let format = stream_description(sample_rate);
            let state = Box::into_raw(Box::new(CallbackState {
                queue: queue.clone(),
                running: AtomicBool::new(true),
                realtime_started: AtomicBool::new(false),
            }));
            let mut audio_queue: AudioQueueRef = ptr::null_mut();
            let status = unsafe {
                AudioQueueNewOutput(
                    &format,
                    output_callback,
                    state as *mut c_void,
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    &mut audio_queue,
                )
            };
            if status != NO_ERR {
                queue.record_coreaudio_error(status);
                unsafe {
                    (*state).running.store(false, Ordering::Release);
                    drop(Box::from_raw(state));
                }
                return Err(format!("AudioQueueNewOutput failed with OSStatus {status}"));
            }

            let mut buffers = Vec::with_capacity(BUFFER_COUNT);
            let buffer_bytes = coreaudio_buffer_byte_size(BUFFER_FRAMES);
            for _ in 0..BUFFER_COUNT {
                let mut buffer: AudioQueueBufferRef = ptr::null_mut();
                let status =
                    unsafe { AudioQueueAllocateBuffer(audio_queue, buffer_bytes, &mut buffer) };
                if status != NO_ERR {
                    queue.record_coreaudio_error(status);
                    unsafe {
                        (*state).running.store(false, Ordering::Release);
                        let _ = AudioQueueDispose(audio_queue, 1);
                        drop(Box::from_raw(state));
                    }
                    return Err(format!(
                        "AudioQueueAllocateBuffer failed with OSStatus {status}"
                    ));
                }
                unsafe {
                    let buffer_ref = &mut *buffer;
                    let samples = std::slice::from_raw_parts_mut(
                        buffer_ref.audio_data as *mut i16,
                        BUFFER_FRAMES * CHANNELS,
                    );
                    queue.pop_interleaved_i16(samples);
                    buffer_ref.audio_data_byte_size = buffer_bytes;
                }
                let status =
                    unsafe { AudioQueueEnqueueBuffer(audio_queue, buffer, 0, ptr::null()) };
                if status != NO_ERR {
                    queue.record_coreaudio_error(status);
                    unsafe {
                        (*state).running.store(false, Ordering::Release);
                        let _ = AudioQueueDispose(audio_queue, 1);
                        drop(Box::from_raw(state));
                    }
                    return Err(format!(
                        "AudioQueueEnqueueBuffer failed with OSStatus {status}"
                    ));
                }
                buffers.push(buffer);
            }

            unsafe {
                (*state).realtime_started.store(true, Ordering::Release);
            }
            let status = unsafe { AudioQueueStart(audio_queue, ptr::null()) };
            if status != NO_ERR {
                queue.record_coreaudio_error(status);
                unsafe {
                    (*state).realtime_started.store(false, Ordering::Release);
                    (*state).running.store(false, Ordering::Release);
                    let _ = AudioQueueDispose(audio_queue, 1);
                    drop(Box::from_raw(state));
                }
                return Err(format!("AudioQueueStart failed with OSStatus {status}"));
            }
            if unsafe { !(*state).running.load(Ordering::Acquire) } {
                unsafe {
                    (*state).realtime_started.store(false, Ordering::Release);
                    let _ = AudioQueueStop(audio_queue, 1);
                    let _ = AudioQueueDispose(audio_queue, 1);
                    drop(Box::from_raw(state));
                }
                return Err(
                    "CoreAudio output callback stopped while starting the audio queue".to_string(),
                );
            }
            queue.record_coreaudio_start();

            Ok(Self {
                queue: audio_queue,
                _buffers: buffers,
                state,
            })
        }
    }

    impl Drop for CoreAudioOutput {
        fn drop(&mut self) {
            unsafe {
                if !self.state.is_null() {
                    (*self.state)
                        .realtime_started
                        .store(false, Ordering::Release);
                    (*self.state).running.store(false, Ordering::Release);
                    (*self.state).queue.record_coreaudio_stop();
                }
                let _ = AudioQueueStop(self.queue, 1);
                let _ = AudioQueueDispose(self.queue, 1);
                if !self.state.is_null() {
                    drop(Box::from_raw(self.state));
                    self.state = ptr::null_mut();
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            AUDIO_FORMAT_FLAG_IS_BIG_ENDIAN, AUDIO_FORMAT_FLAG_IS_NATIVE_ENDIAN,
            AUDIO_FORMAT_FLAG_IS_PACKED, AUDIO_FORMAT_FLAG_IS_SIGNED_INTEGER,
            AUDIO_FORMAT_LINEAR_PCM, BUFFER_FRAMES, coreaudio_buffer_byte_size, stream_description,
        };

        #[test]
        fn stream_description_is_native_i16_stereo_interleaved_pcm() {
            let format = stream_description(44_100);

            assert_eq!(format.sample_rate, 44_100.0);
            assert_eq!(format.format_id, AUDIO_FORMAT_LINEAR_PCM);
            assert_eq!(
                format.format_flags,
                AUDIO_FORMAT_FLAG_IS_SIGNED_INTEGER
                    | AUDIO_FORMAT_FLAG_IS_PACKED
                    | AUDIO_FORMAT_FLAG_IS_NATIVE_ENDIAN
            );
            #[cfg(target_endian = "little")]
            assert_eq!(format.format_flags & AUDIO_FORMAT_FLAG_IS_BIG_ENDIAN, 0);
            #[cfg(target_endian = "big")]
            assert_ne!(format.format_flags & AUDIO_FORMAT_FLAG_IS_BIG_ENDIAN, 0);
            assert_eq!(format.bytes_per_packet, 4);
            assert_eq!(format.frames_per_packet, 1);
            assert_eq!(format.bytes_per_frame, 4);
            assert_eq!(format.channels_per_frame, 2);
            assert_eq!(format.bits_per_channel, 16);
        }

        #[test]
        fn buffer_size_matches_interleaved_i16_frame_count() {
            assert_eq!(coreaudio_buffer_byte_size(1), 4);
            assert_eq!(
                coreaudio_buffer_byte_size(BUFFER_FRAMES),
                (BUFFER_FRAMES as u32) * 4
            );
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::CoreAudioOutput;

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct CoreAudioOutput;

#[cfg(not(target_os = "macos"))]
impl CoreAudioOutput {
    pub fn start(_queue: SharedStereoQueue, _sample_rate: u32) -> Result<Self, String> {
        Err("CoreAudio output is only available on macOS".to_string())
    }
}
