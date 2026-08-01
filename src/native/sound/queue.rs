use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

const REALTIME_RECOVERY_MIN_CAPACITY_FRAMES: usize = 4096;
pub(crate) const MAX_CONCEALMENT_FRAMES: usize = 128;
const CALLBACK_MISS_CONCEALMENT_FRAMES: usize = 32;
const CALLBACK_PENDING_RESCUE_MAX_FRAMES: usize = 1024;
const PRODUCER_DEFERRED_MAX_FRAMES: usize = 32_768;
const PRODUCER_PENDING_FLUSH_MAX_FRAMES: usize = 4096;
const REALTIME_PENDING_FLUSH_EXTRA_FRAMES: usize = 512;
const PLAYOUT_RATE_ONE: u32 = 1 << 16;
const PLAYOUT_RATE_EMERGENCY: u32 = PLAYOUT_RATE_ONE / 2;
const PLAYOUT_RATE_CRITICAL: u32 = (PLAYOUT_RATE_ONE * 2) / 3;
const PLAYOUT_RATE_LOW: u32 = (PLAYOUT_RATE_ONE * 3) / 4;
const PLAYOUT_RATE_GUARD: u32 = (PLAYOUT_RATE_ONE * 7) / 8;
const MAX_ADAPTIVE_STRETCH_RATIO_NUMERATOR: u64 = 1;
const MAX_ADAPTIVE_STRETCH_RATIO_DENOMINATOR: u64 = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StereoSample {
    pub left: i16,
    pub right: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StereoQueueStats {
    pub capacity_frames: usize,
    pub queued_frames: usize,
    pub pushed_frames: u64,
    pub popped_frames: u64,
    pub dropped_frames: u64,
    pub underflow_frames: u64,
    pub output_frames: u64,
    pub repeated_frames: u64,
    pub concealed_frames: u64,
    pub starvation_events: u64,
    pub callback_miss_frames: u64,
    pub callback_miss_events: u64,
    pub callback_rescue_frames: u64,
    pub callback_rescue_events: u64,
    pub callback_fallback_frames: u64,
    pub callback_fallback_events: u64,
    pub callback_silence_frames: u64,
    pub callback_silence_events: u64,
    pub producer_miss_frames: u64,
    pub producer_miss_events: u64,
    pub producer_deferred_frames: u64,
    pub producer_deferred_events: u64,
    pub producer_deferred_dropped_frames: u64,
    pub pending_producer_frames: usize,
    pub low_water_frames: usize,
    pub critical_water_frames: usize,
    pub peak_queued_frames: usize,
    pub coreaudio_started: bool,
    pub coreaudio_running: bool,
    pub coreaudio_callback_count: u64,
    pub coreaudio_callback_output_frames: u64,
    pub coreaudio_enqueue_errors: u64,
    pub coreaudio_last_status: i32,
}

impl StereoQueueStats {
    pub fn json(self) -> String {
        format!(
            "{{\"capacity_frames\":{},\"queued_frames\":{},\"pushed_frames\":{},\"popped_frames\":{},\"dropped_frames\":{},\"underflow_frames\":{},\"output_frames\":{},\"repeated_frames\":{},\"concealed_frames\":{},\"starvation_events\":{},\"callback_miss_frames\":{},\"callback_miss_events\":{},\"callback_rescue_frames\":{},\"callback_rescue_events\":{},\"callback_fallback_frames\":{},\"callback_fallback_events\":{},\"callback_silence_frames\":{},\"callback_silence_events\":{},\"producer_miss_frames\":{},\"producer_miss_events\":{},\"producer_deferred_frames\":{},\"producer_deferred_events\":{},\"producer_deferred_dropped_frames\":{},\"pending_producer_frames\":{},\"low_water_frames\":{},\"critical_water_frames\":{},\"peak_queued_frames\":{},\"coreaudio_started\":{},\"coreaudio_running\":{},\"coreaudio_callback_count\":{},\"coreaudio_callback_output_frames\":{},\"coreaudio_enqueue_errors\":{},\"coreaudio_last_status\":{}}}",
            self.capacity_frames,
            self.queued_frames,
            self.pushed_frames,
            self.popped_frames,
            self.dropped_frames,
            self.underflow_frames,
            self.output_frames,
            self.repeated_frames,
            self.concealed_frames,
            self.starvation_events,
            self.callback_miss_frames,
            self.callback_miss_events,
            self.callback_rescue_frames,
            self.callback_rescue_events,
            self.callback_fallback_frames,
            self.callback_fallback_events,
            self.callback_silence_frames,
            self.callback_silence_events,
            self.producer_miss_frames,
            self.producer_miss_events,
            self.producer_deferred_frames,
            self.producer_deferred_events,
            self.producer_deferred_dropped_frames,
            self.pending_producer_frames,
            self.low_water_frames,
            self.critical_water_frames,
            self.peak_queued_frames,
            self.coreaudio_started,
            self.coreaudio_running,
            self.coreaudio_callback_count,
            self.coreaudio_callback_output_frames,
            self.coreaudio_enqueue_errors,
            self.coreaudio_last_status
        )
    }
}

#[derive(Clone, Debug)]
pub struct BoundedStereoQueue {
    frames: VecDeque<StereoSample>,
    capacity_frames: usize,
    pushed_frames: u64,
    popped_frames: u64,
    dropped_frames: u64,
    underflow_frames: u64,
    output_frames: u64,
    repeated_frames: u64,
    concealed_frames: u64,
    starvation_events: u64,
    peak_queued_frames: usize,
    playback_prepared: bool,
    low_water_frames: usize,
    critical_water_frames: usize,
    last_output: StereoSample,
    last_output_valid: bool,
    previous_pop_starved: bool,
    playout_phase: u32,
    concealment_run_frames: usize,
    stretch_run_frames: usize,
}

impl BoundedStereoQueue {
    pub fn new(capacity_frames: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity_frames.max(1)),
            capacity_frames: capacity_frames.max(1),
            pushed_frames: 0,
            popped_frames: 0,
            dropped_frames: 0,
            underflow_frames: 0,
            output_frames: 0,
            repeated_frames: 0,
            concealed_frames: 0,
            starvation_events: 0,
            peak_queued_frames: 0,
            playback_prepared: false,
            low_water_frames: 0,
            critical_water_frames: 0,
            last_output: StereoSample::default(),
            last_output_valid: false,
            previous_pop_starved: false,
            playout_phase: PLAYOUT_RATE_ONE,
            concealment_run_frames: 0,
            stretch_run_frames: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn push(&mut self, frame: StereoSample) {
        if self.frames.len() == self.capacity_frames {
            self.frames.pop_front();
            self.dropped_frames = self.dropped_frames.saturating_add(1);
        }
        self.frames.push_back(frame);
        self.pushed_frames = self.pushed_frames.saturating_add(1);
        self.peak_queued_frames = self.peak_queued_frames.max(self.frames.len());
        self.reset_recovery_runs_after_refill();
    }

    pub fn push_slice(&mut self, frames: &[StereoSample]) {
        if frames.is_empty() {
            return;
        }

        self.pushed_frames = self.pushed_frames.saturating_add(frames.len() as u64);
        let retained_len = self.capacity_frames.min(self.frames.len() + frames.len());
        let dropped = self.frames.len() + frames.len() - retained_len;
        self.dropped_frames = self.dropped_frames.saturating_add(dropped as u64);

        if frames.len() >= self.capacity_frames {
            self.frames.clear();
            self.frames.extend(
                frames[frames.len() - self.capacity_frames..]
                    .iter()
                    .copied(),
            );
        } else {
            while self.frames.len() + frames.len() > self.capacity_frames {
                self.frames.pop_front();
            }
            self.frames.extend(frames.iter().copied());
        }
        self.peak_queued_frames = self.peak_queued_frames.max(self.frames.len());
        self.reset_recovery_runs_after_refill();
    }

    pub fn prepare_for_playback(&mut self, prebuffer_frames: usize) {
        let keep = prebuffer_frames.min(self.capacity_frames);
        while self.frames.len() > keep {
            self.frames.pop_front();
        }
        let (low_water_frames, critical_water_frames) =
            playback_watermarks(keep, self.capacity_frames);
        self.pushed_frames = 0;
        self.popped_frames = 0;
        self.dropped_frames = 0;
        self.underflow_frames = 0;
        self.output_frames = 0;
        self.repeated_frames = 0;
        self.concealed_frames = 0;
        self.starvation_events = 0;
        self.peak_queued_frames = self.frames.len();
        self.playback_prepared = true;
        self.low_water_frames = low_water_frames;
        self.critical_water_frames = critical_water_frames;
        self.last_output = self.frames.front().copied().unwrap_or_default();
        self.last_output_valid = !self.frames.is_empty();
        self.previous_pop_starved = false;
        self.playout_phase = PLAYOUT_RATE_ONE;
        self.concealment_run_frames = 0;
        self.stretch_run_frames = 0;
    }

    pub fn pop_or_silence(&mut self) -> StereoSample {
        match self.frames.pop_front() {
            Some(frame) => {
                self.popped_frames = self.popped_frames.saturating_add(1);
                self.output_frames = self.output_frames.saturating_add(1);
                self.last_output = frame;
                self.last_output_valid = true;
                self.previous_pop_starved = false;
                frame
            }
            None => {
                self.underflow_frames = self.underflow_frames.saturating_add(1);
                self.output_frames = self.output_frames.saturating_add(1);
                StereoSample::default()
            }
        }
    }

    pub fn pop_interleaved_i16(&mut self, output: &mut [i16]) {
        let mut chunks = output.chunks_exact_mut(2);
        for chunk in &mut chunks {
            let frame = self.pop_for_playback();
            chunk[0] = frame.left;
            chunk[1] = frame.right;
        }
        chunks.into_remainder().fill(0);
    }

    pub fn stats(&self) -> StereoQueueStats {
        StereoQueueStats {
            capacity_frames: self.capacity_frames,
            queued_frames: self.frames.len(),
            pushed_frames: self.pushed_frames,
            popped_frames: self.popped_frames,
            dropped_frames: self.dropped_frames,
            underflow_frames: self.underflow_frames,
            output_frames: self.output_frames,
            repeated_frames: self.repeated_frames,
            concealed_frames: self.concealed_frames,
            starvation_events: self.starvation_events,
            callback_miss_frames: 0,
            callback_miss_events: 0,
            callback_rescue_frames: 0,
            callback_rescue_events: 0,
            callback_fallback_frames: 0,
            callback_fallback_events: 0,
            callback_silence_frames: 0,
            callback_silence_events: 0,
            producer_miss_frames: 0,
            producer_miss_events: 0,
            producer_deferred_frames: 0,
            producer_deferred_events: 0,
            producer_deferred_dropped_frames: 0,
            pending_producer_frames: 0,
            low_water_frames: self.low_water_frames,
            critical_water_frames: self.critical_water_frames,
            peak_queued_frames: self.peak_queued_frames,
            coreaudio_started: false,
            coreaudio_running: false,
            coreaudio_callback_count: 0,
            coreaudio_callback_output_frames: 0,
            coreaudio_enqueue_errors: 0,
            coreaudio_last_status: 0,
        }
    }

    fn pop_for_playback(&mut self) -> StereoSample {
        if !self.frames.is_empty() {
            if let Some(rate) = self.low_water_playout_rate() {
                self.playout_phase = self.playout_phase.saturating_add(rate);
                if self.playout_phase < PLAYOUT_RATE_ONE {
                    if !self.can_emit_adaptive_stretch_frame() {
                        self.playout_phase = PLAYOUT_RATE_ONE;
                    } else {
                        self.output_frames = self.output_frames.saturating_add(1);
                        self.repeated_frames = self.repeated_frames.saturating_add(1);
                        self.stretch_run_frames = self.stretch_run_frames.saturating_add(1);
                        self.previous_pop_starved = false;
                        return interpolated_sample_at_phase(
                            self.last_output,
                            self.frames.front().copied().unwrap_or(self.last_output),
                            self.playout_phase,
                        );
                    }
                }
                self.playout_phase -= PLAYOUT_RATE_ONE;
            } else {
                self.playout_phase = PLAYOUT_RATE_ONE;
            }

            self.previous_pop_starved = false;

            let frame = self.frames.pop_front().unwrap_or_default();
            self.popped_frames = self.popped_frames.saturating_add(1);
            self.output_frames = self.output_frames.saturating_add(1);
            self.last_output = frame;
            self.last_output_valid = true;
            return frame;
        }

        self.playout_phase = PLAYOUT_RATE_ONE;
        self.output_frames = self.output_frames.saturating_add(1);
        if self.playback_prepared && !self.previous_pop_starved {
            self.starvation_events = self.starvation_events.saturating_add(1);
            self.previous_pop_starved = true;
        }
        if let Some(frame) = self.conceal_starved_frame() {
            self.concealed_frames = self.concealed_frames.saturating_add(1);
            return frame;
        }
        self.underflow_frames = self.underflow_frames.saturating_add(1);
        StereoSample::default()
    }

    fn low_water_playout_rate(&self) -> Option<u32> {
        if !self.can_recover_realtime_starvation()
            || !self.last_output_valid
            || self.frames.is_empty()
            || self.frames.len() >= self.low_water_frames
        {
            return None;
        }

        let queued = self.frames.len();
        let critical = self.critical_water_frames.max(1);
        if queued <= critical / 2 {
            Some(PLAYOUT_RATE_EMERGENCY)
        } else if queued <= critical {
            Some(PLAYOUT_RATE_CRITICAL)
        } else if queued <= critical.saturating_mul(2) {
            Some(PLAYOUT_RATE_LOW)
        } else {
            Some(PLAYOUT_RATE_GUARD)
        }
    }

    fn can_emit_adaptive_stretch_frame(&self) -> bool {
        let repeated_frames = self.repeated_frames.saturating_add(1);
        let output_frames = self.output_frames.saturating_add(1);
        repeated_frames.saturating_mul(MAX_ADAPTIVE_STRETCH_RATIO_DENOMINATOR)
            <= output_frames.saturating_mul(MAX_ADAPTIVE_STRETCH_RATIO_NUMERATOR)
    }

    fn conceal_starved_frame(&mut self) -> Option<StereoSample> {
        if !self.can_recover_realtime_starvation()
            || !self.last_output_valid
            || self.concealment_run_frames >= MAX_CONCEALMENT_FRAMES
        {
            return None;
        }

        let remaining = MAX_CONCEALMENT_FRAMES - self.concealment_run_frames;
        self.concealment_run_frames += 1;
        Some(scale_sample(
            self.last_output,
            remaining,
            MAX_CONCEALMENT_FRAMES,
        ))
    }

    fn can_recover_realtime_starvation(&self) -> bool {
        self.playback_prepared && self.capacity_frames >= REALTIME_RECOVERY_MIN_CAPACITY_FRAMES
    }

    fn reset_recovery_runs_after_refill(&mut self) {
        if self.playback_prepared && self.frames.len() >= self.critical_water_frames.max(1) {
            self.concealment_run_frames = 0;
        }
        if self.playback_prepared && self.frames.len() >= self.stretch_recovery_frames() {
            self.stretch_run_frames = 0;
        }
    }

    fn stretch_recovery_frames(&self) -> usize {
        self.low_water_frames
            .saturating_add(self.critical_water_frames / 2)
            .min(self.capacity_frames)
            .max(self.low_water_frames.max(1))
    }

    fn cached_output_sample(&self) -> Option<StereoSample> {
        self.last_output_valid.then_some(self.last_output)
    }
}

fn playback_watermarks(prebuffer_frames: usize, capacity_frames: usize) -> (usize, usize) {
    let target = prebuffer_frames.min(capacity_frames);
    if target == 0 {
        return (0, 0);
    }
    let low = if capacity_frames < 1024 {
        (target / 2).max(1)
    } else {
        (target / 2).clamp(512, capacity_frames)
    };
    let critical = if capacity_frames < 1024 {
        (low / 4).max(1)
    } else {
        (low / 2).clamp(128, low)
    };
    (low, critical.min(low))
}

fn interpolated_sample_at_phase(
    previous: StereoSample,
    next: StereoSample,
    phase: u32,
) -> StereoSample {
    let phase = phase.min(PLAYOUT_RATE_ONE);
    StereoSample {
        left: interpolate_i16(previous.left, next.left, phase),
        right: interpolate_i16(previous.right, next.right, phase),
    }
}

fn interpolate_i16(previous: i16, next: i16, phase: u32) -> i16 {
    let previous_weight = i64::from(PLAYOUT_RATE_ONE - phase);
    let next_weight = i64::from(phase);
    let value = i64::from(previous) * previous_weight + i64::from(next) * next_weight;
    (value / i64::from(PLAYOUT_RATE_ONE)).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

fn scale_sample(sample: StereoSample, numerator: usize, denominator: usize) -> StereoSample {
    if denominator == 0 {
        return StereoSample::default();
    }
    StereoSample {
        left: scale_i16(sample.left, numerator, denominator),
        right: scale_i16(sample.right, numerator, denominator),
    }
}

fn scale_i16(sample: i16, numerator: usize, denominator: usize) -> i16 {
    let scaled = i32::from(sample) * numerator as i32 / denominator as i32;
    scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[derive(Clone, Debug)]
pub struct SharedStereoQueue {
    inner: Arc<Mutex<BoundedStereoQueue>>,
    producer_pending: Arc<Mutex<VecDeque<StereoSample>>>,
    capacity_frames: usize,
    pushed_frames: Arc<AtomicU64>,
    callback_cached_sample: Arc<AtomicU32>,
    callback_cached_sample_valid: Arc<AtomicBool>,
    callback_miss_run_frames: Arc<AtomicU64>,
    callback_miss_frames: Arc<AtomicU64>,
    callback_miss_events: Arc<AtomicU64>,
    callback_rescue_frames: Arc<AtomicU64>,
    callback_rescue_events: Arc<AtomicU64>,
    callback_fallback_frames: Arc<AtomicU64>,
    callback_fallback_events: Arc<AtomicU64>,
    callback_silence_frames: Arc<AtomicU64>,
    callback_silence_events: Arc<AtomicU64>,
    producer_miss_frames: Arc<AtomicU64>,
    producer_miss_events: Arc<AtomicU64>,
    producer_deferred_frames: Arc<AtomicU64>,
    producer_deferred_events: Arc<AtomicU64>,
    producer_deferred_dropped_frames: Arc<AtomicU64>,
    coreaudio_started: Arc<AtomicBool>,
    coreaudio_running: Arc<AtomicBool>,
    coreaudio_callback_count: Arc<AtomicU64>,
    coreaudio_callback_output_frames: Arc<AtomicU64>,
    coreaudio_enqueue_errors: Arc<AtomicU64>,
    coreaudio_last_status: Arc<AtomicI32>,
}

impl SharedStereoQueue {
    pub fn new(capacity_frames: usize) -> Self {
        let capacity_frames = capacity_frames.max(1);
        Self {
            inner: Arc::new(Mutex::new(BoundedStereoQueue::new(capacity_frames))),
            producer_pending: Arc::new(Mutex::new(VecDeque::with_capacity(
                PRODUCER_DEFERRED_MAX_FRAMES.min(capacity_frames.max(1)),
            ))),
            capacity_frames,
            pushed_frames: Arc::new(AtomicU64::new(0)),
            callback_cached_sample: Arc::new(AtomicU32::new(pack_sample(StereoSample::default()))),
            callback_cached_sample_valid: Arc::new(AtomicBool::new(false)),
            callback_miss_run_frames: Arc::new(AtomicU64::new(0)),
            callback_miss_frames: Arc::new(AtomicU64::new(0)),
            callback_miss_events: Arc::new(AtomicU64::new(0)),
            callback_rescue_frames: Arc::new(AtomicU64::new(0)),
            callback_rescue_events: Arc::new(AtomicU64::new(0)),
            callback_fallback_frames: Arc::new(AtomicU64::new(0)),
            callback_fallback_events: Arc::new(AtomicU64::new(0)),
            callback_silence_frames: Arc::new(AtomicU64::new(0)),
            callback_silence_events: Arc::new(AtomicU64::new(0)),
            producer_miss_frames: Arc::new(AtomicU64::new(0)),
            producer_miss_events: Arc::new(AtomicU64::new(0)),
            producer_deferred_frames: Arc::new(AtomicU64::new(0)),
            producer_deferred_events: Arc::new(AtomicU64::new(0)),
            producer_deferred_dropped_frames: Arc::new(AtomicU64::new(0)),
            coreaudio_started: Arc::new(AtomicBool::new(false)),
            coreaudio_running: Arc::new(AtomicBool::new(false)),
            coreaudio_callback_count: Arc::new(AtomicU64::new(0)),
            coreaudio_callback_output_frames: Arc::new(AtomicU64::new(0)),
            coreaudio_enqueue_errors: Arc::new(AtomicU64::new(0)),
            coreaudio_last_status: Arc::new(AtomicI32::new(0)),
        }
    }

    pub fn push_slice(&self, frames: &[StereoSample]) {
        if frames.is_empty() {
            return;
        }
        self.pushed_frames
            .fetch_add(frames.len() as u64, Ordering::AcqRel);
        self.cache_callback_sample(frames[frames.len() - 1]);
        if let Some(mut queue) = try_lock_unpoisoned(&self.inner) {
            self.flush_pending_into_limited(&mut queue, PRODUCER_PENDING_FLUSH_MAX_FRAMES);
            queue.push_slice(frames);
            return;
        }
        self.defer_producer_frames(frames);
    }

    pub fn pop_interleaved_i16(&self, output: &mut [i16]) {
        if let Some(mut queue) = try_lock_unpoisoned(&self.inner) {
            self.reset_callback_miss_run();
            self.flush_all_pending_into(&mut queue);
            queue.pop_interleaved_i16(output);
            if let Some(sample) = queue.cached_output_sample() {
                self.cache_callback_sample(sample);
            }
            return;
        }
        self.fill_callback_miss(output, false);
    }

    pub(crate) fn pop_interleaved_i16_realtime(&self, output: &mut [i16]) {
        if let Some(mut queue) = try_lock_unpoisoned(&self.inner) {
            self.reset_callback_miss_run();
            self.flush_pending_into_limited(&mut queue, realtime_pending_flush_limit(output.len()));
            queue.pop_interleaved_i16(output);
            if let Some(sample) = queue.cached_output_sample() {
                self.cache_callback_sample(sample);
            }
            return;
        }
        self.fill_callback_miss(output, true);
    }

    pub fn prepare_for_playback(&self, prebuffer_frames: usize) {
        // Reset before taking the queue lock so producer pushes racing with
        // playback startup remain visible in the lock-free statistics.
        self.pushed_frames.store(0, Ordering::Release);
        let mut queue = lock_unpoisoned(&self.inner);
        self.flush_all_pending_into(&mut queue);
        queue.prepare_for_playback(prebuffer_frames);
        if let Some(sample) = queue.cached_output_sample() {
            self.cache_callback_sample(sample);
        } else {
            self.callback_cached_sample_valid
                .store(false, Ordering::Release);
        }
    }

    pub fn stats(&self) -> StereoQueueStats {
        let mut stats = try_lock_unpoisoned(&self.inner)
            .map(|queue| queue.stats())
            .unwrap_or_else(|| StereoQueueStats {
                capacity_frames: self.capacity_frames,
                pushed_frames: self.pushed_frames.load(Ordering::Acquire),
                output_frames: self
                    .coreaudio_callback_output_frames
                    .load(Ordering::Acquire),
                ..StereoQueueStats::default()
            });
        stats.pushed_frames = self.pushed_frames.load(Ordering::Acquire);
        stats.callback_miss_frames = self.callback_miss_frames.load(Ordering::Acquire);
        stats.callback_miss_events = self.callback_miss_events.load(Ordering::Acquire);
        stats.callback_rescue_frames = self.callback_rescue_frames.load(Ordering::Acquire);
        stats.callback_rescue_events = self.callback_rescue_events.load(Ordering::Acquire);
        stats.callback_fallback_frames = self.callback_fallback_frames.load(Ordering::Acquire);
        stats.callback_fallback_events = self.callback_fallback_events.load(Ordering::Acquire);
        stats.callback_silence_frames = self.callback_silence_frames.load(Ordering::Acquire);
        stats.callback_silence_events = self.callback_silence_events.load(Ordering::Acquire);
        stats.producer_miss_frames = self.producer_miss_frames.load(Ordering::Acquire);
        stats.producer_miss_events = self.producer_miss_events.load(Ordering::Acquire);
        stats.producer_deferred_frames = self.producer_deferred_frames.load(Ordering::Acquire);
        stats.producer_deferred_events = self.producer_deferred_events.load(Ordering::Acquire);
        stats.producer_deferred_dropped_frames = self
            .producer_deferred_dropped_frames
            .load(Ordering::Acquire);
        stats.pending_producer_frames = try_lock_unpoisoned(&self.producer_pending)
            .map(|pending| pending.len())
            .unwrap_or(0);
        stats.coreaudio_started = self.coreaudio_started.load(Ordering::Acquire);
        stats.coreaudio_running = self.coreaudio_running.load(Ordering::Acquire);
        stats.coreaudio_callback_count = self.coreaudio_callback_count.load(Ordering::Acquire);
        stats.coreaudio_callback_output_frames = self
            .coreaudio_callback_output_frames
            .load(Ordering::Acquire);
        stats.coreaudio_enqueue_errors = self.coreaudio_enqueue_errors.load(Ordering::Acquire);
        stats.coreaudio_last_status = self.coreaudio_last_status.load(Ordering::Acquire);
        stats
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        lock_unpoisoned(&self.inner).len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        lock_unpoisoned(&self.inner).is_empty()
    }

    pub fn queued_frames(&self) -> Option<usize> {
        try_lock_unpoisoned(&self.inner).map(|queue| queue.len())
    }

    pub fn snapshot_latest_frames(&self, max_frames: usize) -> Vec<StereoSample> {
        if max_frames == 0 {
            return Vec::new();
        }
        let Some(queue) = try_lock_unpoisoned(&self.inner) else {
            return Vec::new();
        };
        let skip = queue.frames.len().saturating_sub(max_frames);
        queue.frames.iter().skip(skip).copied().collect()
    }

    pub(crate) fn record_coreaudio_start(&self) {
        self.coreaudio_started.store(true, Ordering::Release);
        self.coreaudio_running.store(true, Ordering::Release);
        self.coreaudio_last_status.store(0, Ordering::Release);
    }

    pub(crate) fn record_coreaudio_callback(&self, frame_count: usize) {
        self.coreaudio_callback_count.fetch_add(1, Ordering::AcqRel);
        self.coreaudio_callback_output_frames
            .fetch_add(frame_count as u64, Ordering::AcqRel);
    }

    pub(crate) fn record_coreaudio_error(&self, status: i32) {
        self.coreaudio_enqueue_errors.fetch_add(1, Ordering::AcqRel);
        self.coreaudio_last_status.store(status, Ordering::Release);
        self.coreaudio_running.store(false, Ordering::Release);
    }

    pub(crate) fn record_coreaudio_stop(&self) {
        self.coreaudio_running.store(false, Ordering::Release);
    }

    fn cache_callback_sample(&self, sample: StereoSample) {
        self.callback_cached_sample
            .store(pack_sample(sample), Ordering::Release);
        self.callback_cached_sample_valid
            .store(true, Ordering::Release);
    }

    fn fill_callback_miss(&self, output: &mut [i16], rescue_pending: bool) {
        let frame_count = output.len() / 2;
        self.record_callback_miss(frame_count);
        if frame_count == 0 {
            output.fill(0);
            return;
        }

        let rescued = if rescue_pending {
            self.fill_from_pending_producer(output, CALLBACK_PENDING_RESCUE_MAX_FRAMES)
        } else {
            0
        };
        if rescued > 0 {
            self.record_callback_rescue(rescued);
            self.reset_callback_miss_run();
            if rescued == frame_count {
                output[rescued * 2..].fill(0);
                return;
            }
            self.fill_callback_gap(&mut output[rescued * 2..]);
            return;
        }

        self.fill_callback_gap(output);
    }

    fn fill_from_pending_producer(&self, output: &mut [i16], max_frames: usize) -> usize {
        let Some(mut pending) = try_lock_unpoisoned(&self.producer_pending) else {
            return 0;
        };
        let limit = (output.len() / 2).min(max_frames);
        let mut rescued = 0usize;
        let mut last = None;
        for chunk in output.chunks_exact_mut(2).take(limit) {
            let Some(frame) = pending.pop_front() else {
                break;
            };
            chunk[0] = frame.left;
            chunk[1] = frame.right;
            rescued += 1;
            last = Some(frame);
        }
        if let Some(frame) = last {
            self.cache_callback_sample(frame);
        }
        rescued
    }

    fn fill_callback_gap(&self, output: &mut [i16]) {
        let frame_count = output.len() / 2;
        if frame_count == 0 {
            output.fill(0);
            return;
        }
        if self.callback_cached_sample_valid.load(Ordering::Acquire) {
            let sample = unpack_sample(self.callback_cached_sample.load(Ordering::Acquire));
            let previous_miss_run = self.extend_callback_miss_run(frame_count);
            let conceal_frames = frame_count.min(CALLBACK_MISS_CONCEALMENT_FRAMES.saturating_sub(
                previous_miss_run.min(CALLBACK_MISS_CONCEALMENT_FRAMES as u64) as usize,
            ));
            let mut chunks = output.chunks_exact_mut(2);
            for (index, chunk) in chunks.by_ref().take(conceal_frames).enumerate() {
                let remaining = conceal_frames.saturating_sub(index);
                chunk[0] = scale_i16(sample.left, remaining, conceal_frames);
                chunk[1] = scale_i16(sample.right, remaining, conceal_frames);
            }
            for chunk in &mut chunks {
                chunk[0] = 0;
                chunk[1] = 0;
            }
            chunks.into_remainder().fill(0);
            self.record_callback_fallback(conceal_frames);
            self.record_callback_silence(frame_count.saturating_sub(conceal_frames));
        } else {
            output.fill(0);
            self.record_callback_silence(frame_count);
        }
    }

    fn extend_callback_miss_run(&self, frame_count: usize) -> u64 {
        self.callback_miss_run_frames
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(
                    current
                        .saturating_add(frame_count as u64)
                        .min(CALLBACK_MISS_CONCEALMENT_FRAMES as u64),
                )
            })
            .unwrap_or(CALLBACK_MISS_CONCEALMENT_FRAMES as u64)
    }

    fn reset_callback_miss_run(&self) {
        self.callback_miss_run_frames.store(0, Ordering::Release);
    }

    fn flush_all_pending_into(&self, queue: &mut BoundedStereoQueue) {
        self.flush_pending_into_limited(queue, usize::MAX);
    }

    fn flush_pending_into_limited(&self, queue: &mut BoundedStereoQueue, max_frames: usize) {
        if max_frames == 0 {
            return;
        }
        let Some(mut pending) = try_lock_unpoisoned(&self.producer_pending) else {
            return;
        };
        for _ in 0..max_frames {
            let Some(frame) = pending.pop_front() else {
                break;
            };
            queue.push(frame);
        }
    }

    fn defer_producer_frames(&self, frames: &[StereoSample]) {
        let Some(mut pending) = try_lock_unpoisoned(&self.producer_pending) else {
            self.record_producer_miss(frames.len());
            return;
        };

        let max_pending = PRODUCER_DEFERRED_MAX_FRAMES.max(1);
        let retained = frames.len().min(max_pending);
        let dropped_from_new = frames.len().saturating_sub(retained);
        if dropped_from_new > 0 {
            self.record_producer_miss(dropped_from_new);
        }
        if retained > 0 {
            self.cache_callback_sample(frames[frames.len() - 1]);
        }

        while pending.len() + retained > max_pending {
            pending.pop_front();
            self.producer_deferred_dropped_frames
                .fetch_add(1, Ordering::AcqRel);
        }

        pending.extend(frames[frames.len() - retained..].iter().copied());
        self.producer_deferred_frames
            .fetch_add(retained as u64, Ordering::AcqRel);
        self.producer_deferred_events.fetch_add(1, Ordering::AcqRel);
    }

    fn record_callback_miss(&self, frame_count: usize) {
        if frame_count > 0 {
            self.callback_miss_frames
                .fetch_add(frame_count as u64, Ordering::AcqRel);
            self.callback_miss_events.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_callback_rescue(&self, frame_count: usize) {
        if frame_count > 0 {
            self.callback_rescue_frames
                .fetch_add(frame_count as u64, Ordering::AcqRel);
            self.callback_rescue_events.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_callback_fallback(&self, frame_count: usize) {
        if frame_count > 0 {
            self.callback_fallback_frames
                .fetch_add(frame_count as u64, Ordering::AcqRel);
            self.callback_fallback_events.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_callback_silence(&self, frame_count: usize) {
        if frame_count > 0 {
            self.callback_silence_frames
                .fetch_add(frame_count as u64, Ordering::AcqRel);
            self.callback_silence_events.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_producer_miss(&self, frame_count: usize) {
        if frame_count > 0 {
            self.producer_miss_frames
                .fetch_add(frame_count as u64, Ordering::AcqRel);
            self.producer_miss_events.fetch_add(1, Ordering::AcqRel);
        }
    }
}

fn realtime_pending_flush_limit(output_samples_len: usize) -> usize {
    let output_frames = output_samples_len / 2;
    output_frames
        .saturating_add(REALTIME_PENDING_FLUSH_EXTRA_FRAMES)
        .clamp(output_frames, PRODUCER_PENDING_FLUSH_MAX_FRAMES)
}

fn pack_sample(sample: StereoSample) -> u32 {
    (u32::from(sample.left as u16) << 16) | u32::from(sample.right as u16)
}

fn unpack_sample(packed: u32) -> StereoSample {
    StereoSample {
        left: (packed >> 16) as u16 as i16,
        right: (packed & 0xffff) as u16 as i16,
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn try_lock_unpoisoned<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match mutex.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{self, AssertUnwindSafe};

    use super::{
        BoundedStereoQueue, CALLBACK_MISS_CONCEALMENT_FRAMES, CALLBACK_PENDING_RESCUE_MAX_FRAMES,
        MAX_CONCEALMENT_FRAMES, PRODUCER_DEFERRED_MAX_FRAMES, PRODUCER_PENDING_FLUSH_MAX_FRAMES,
        SharedStereoQueue, StereoSample, realtime_pending_flush_limit,
    };

    #[test]
    fn bounded_queue_drops_oldest_and_counts_underflow() {
        let mut queue = BoundedStereoQueue::new(2);
        queue.push(StereoSample { left: 1, right: 2 });
        queue.push(StereoSample { left: 3, right: 4 });
        queue.push(StereoSample { left: 5, right: 6 });

        assert_eq!(queue.stats().dropped_frames, 1);
        assert_eq!(queue.pop_or_silence(), StereoSample { left: 3, right: 4 });
        assert_eq!(queue.pop_or_silence(), StereoSample { left: 5, right: 6 });
        assert_eq!(queue.pop_or_silence(), StereoSample::default());
        assert_eq!(queue.stats().underflow_frames, 1);
    }

    #[test]
    fn playback_preparation_keeps_latest_audio_and_resets_live_stats() {
        let mut queue = BoundedStereoQueue::new(4);
        for value in 1..=6 {
            queue.push(StereoSample {
                left: value,
                right: -value,
            });
        }
        let _ = queue.pop_or_silence();
        let _ = queue.pop_or_silence();
        let _ = queue.pop_or_silence();

        queue.prepare_for_playback(2);

        assert_eq!(queue.stats().queued_frames, 1);
        assert_eq!(queue.stats().pushed_frames, 0);
        assert_eq!(queue.stats().popped_frames, 0);
        assert_eq!(queue.stats().dropped_frames, 0);
        assert_eq!(queue.stats().underflow_frames, 0);
        assert_eq!(queue.pop_or_silence().left, 6);
    }

    #[test]
    fn playback_pop_preserves_real_frames_and_reports_missing_frames() {
        let mut queue = BoundedStereoQueue::new(16);
        for value in 1..=4 {
            queue.push(StereoSample {
                left: value,
                right: value,
            });
        }
        queue.prepare_for_playback(12);

        let mut output = [0i16; 16];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(output, [1, 1, 2, 2, 3, 3, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(stats.popped_frames, 4, "{stats:?}");
        assert_eq!(stats.underflow_frames, 4, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
    }

    #[test]
    fn prepared_playback_does_not_fade_or_repeat_after_real_audio_runs_out() {
        let mut queue = BoundedStereoQueue::new(8);
        queue.push(StereoSample {
            left: 1024,
            right: -1024,
        });
        queue.prepare_for_playback(8);

        let mut output = [0i16; 12];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(output, [1024, -1024, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(stats.underflow_frames, 5, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
    }

    #[test]
    fn sustained_slow_producer_does_not_explode_repeat_or_conceal_stats() {
        let mut queue = BoundedStereoQueue::new(1024);
        queue.push(StereoSample {
            left: 2048,
            right: -2048,
        });
        queue.prepare_for_playback(512);

        let mut output = vec![1i16; 120 * 2];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(output[0], 2048);
        assert_eq!(output[1], -2048);
        assert!(output[2..].iter().all(|sample| *sample == 0));
        assert_eq!(stats.popped_frames, 1, "{stats:?}");
        assert_eq!(stats.underflow_frames, 119, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
    }

    #[test]
    fn prepared_playback_counts_underflow_when_started_without_any_audio() {
        let mut queue = BoundedStereoQueue::new(8);
        queue.prepare_for_playback(8);

        let mut output = [1i16; 8];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(output, [0; 8]);
        assert_eq!(stats.underflow_frames, 4, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
    }

    #[test]
    fn realtime_refill_after_prebuffer_has_no_underflow_or_concealment() {
        let mut queue = BoundedStereoQueue::new(2048);
        let block_frames = 128;
        let prebuffer_frames = 512;
        let seed = vec![
            StereoSample {
                left: 512,
                right: -512,
            };
            prebuffer_frames
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(prebuffer_frames);

        for tick in 0..32 {
            let mut output = vec![0i16; block_frames * 2];
            queue.pop_interleaved_i16(&mut output);
            assert!(
                output
                    .chunks_exact(2)
                    .any(|frame| frame[0] != 0 || frame[1] != 0),
                "tick {tick}"
            );
            let refill = vec![
                StereoSample {
                    left: 600 + tick,
                    right: -600 - tick,
                };
                block_frames
            ];
            queue.push_slice(&refill);
        }

        let stats = queue.stats();
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 0, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
    }

    #[test]
    fn low_water_adaptive_stretch_reduces_realtime_queue_drain() {
        let mut queue = BoundedStereoQueue::new(8192);
        let seed: Vec<_> = (0..700)
            .map(|value| StereoSample {
                left: value,
                right: -value,
            })
            .collect();
        queue.push_slice(&seed);
        queue.prepare_for_playback(4096);

        let mut output = vec![0i16; 512 * 2];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(stats.output_frames, 512, "{stats:?}");
        assert!(stats.popped_frames < 512, "{stats:?}");
        assert!(stats.repeated_frames > 0, "{stats:?}");
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert!(stats.queued_frames > 700 - 512, "{stats:?}");
    }

    #[test]
    fn large_realtime_prebuffer_does_not_stretch_while_guard_audio_remains() {
        let mut queue = BoundedStereoQueue::new(22_050);
        let seed = vec![
            StereoSample {
                left: 1700,
                right: -1700,
            };
            9_000
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(16_384);

        let mut output = vec![0i16; 512 * 2];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(stats.output_frames, 512, "{stats:?}");
        assert_eq!(stats.popped_frames, 512, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert!(stats.queued_frames >= 8_000, "{stats:?}");
        assert!(
            output
                .chunks_exact(2)
                .all(|frame| frame[0] != 0 || frame[1] != 0)
        );
    }

    #[test]
    fn adaptive_stretch_tracks_sustained_slow_producer_without_silence() {
        let mut queue = BoundedStereoQueue::new(8192);
        let block_frames = 512;
        let refill_frames = 500;
        let seed = vec![
            StereoSample {
                left: 1200,
                right: -1200,
            };
            768
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(4096);

        for tick in 0..80 {
            let mut output = vec![0i16; block_frames * 2];
            queue.pop_interleaved_i16(&mut output);
            assert!(
                output
                    .chunks_exact(2)
                    .all(|frame| frame[0] != 0 || frame[1] != 0),
                "tick {tick}"
            );
            let refill = vec![
                StereoSample {
                    left: 1300 + tick,
                    right: -1300 - tick,
                };
                refill_frames
            ];
            queue.push_slice(&refill);
        }

        let stats = queue.stats();
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 0, "{stats:?}");
        assert!(stats.repeated_frames > 0, "{stats:?}");
        assert!(stats.popped_frames < stats.output_frames, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
    }

    #[test]
    fn adaptive_stretch_reports_unrecoverable_macos_gui_slowdown_instead_of_hiding_it() {
        let mut queue = BoundedStereoQueue::new(22_050);
        let block_frames = 512;
        let refill_frames = 196;
        let seed = vec![
            StereoSample {
                left: 1800,
                right: -1800,
            };
            8192
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(8192);

        for tick in 0..160 {
            let mut output = vec![0i16; block_frames * 2];
            queue.pop_interleaved_i16(&mut output);
            let refill = vec![
                StereoSample {
                    left: 1900 + tick,
                    right: -1900 - tick,
                };
                refill_frames
            ];
            queue.push_slice(&refill);
        }

        let stats = queue.stats();
        assert_eq!(stats.output_frames, 81_920, "{stats:?}");
        assert!(stats.underflow_frames > 0, "{stats:?}");
        assert!(stats.starvation_events > 0, "{stats:?}");
        assert!(stats.concealed_frames <= MAX_CONCEALMENT_FRAMES as u64);
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
        assert!(
            stats.popped_frames <= stats.pushed_frames + 8192,
            "{stats:?}"
        );
    }

    #[test]
    fn adaptive_stretch_is_bounded_when_slow_producer_stops() {
        let mut queue = BoundedStereoQueue::new(8192);
        let seed_frames = 1024;
        let seed = vec![
            StereoSample {
                left: 2200,
                right: -2200,
            };
            seed_frames
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(4096);

        let output_frames = seed_frames * 4 + MAX_CONCEALMENT_FRAMES + 32;
        let mut output = vec![0i16; output_frames * 2];
        queue.pop_interleaved_i16(&mut output);

        let stats = queue.stats();
        assert_eq!(stats.popped_frames, seed_frames as u64, "{stats:?}");
        assert_eq!(
            stats.concealed_frames, MAX_CONCEALMENT_FRAMES as u64,
            "{stats:?}"
        );
        assert!(stats.repeated_frames > 0, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
        assert!(stats.underflow_frames > 0, "{stats:?}");
        assert_eq!(&output[output.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn adaptive_resampling_tracks_macos_gui_42fps_without_underflow() {
        let mut queue = BoundedStereoQueue::new(22_050);
        let block_frames = 512;
        let refill_frames = 358;
        let seed = vec![
            StereoSample {
                left: 2300,
                right: -2300,
            };
            8192
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(8192);

        for tick in 0..240 {
            let mut output = vec![0i16; block_frames * 2];
            queue.pop_interleaved_i16(&mut output);
            assert!(
                output
                    .chunks_exact(2)
                    .all(|frame| frame[0] != 0 || frame[1] != 0),
                "tick {tick}"
            );
            queue.push_slice(&vec![
                StereoSample {
                    left: 2400 + tick,
                    right: -2400 - tick,
                };
                refill_frames
            ]);
        }

        let stats = queue.stats();
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 0, "{stats:?}");
        assert!(stats.repeated_frames > stats.output_frames / 8, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
        assert!(stats.queued_frames > 0, "{stats:?}");
    }

    #[test]
    fn adaptive_resampling_tracks_observed_native_gui_producer_rate_long_term() {
        let mut queue = BoundedStereoQueue::new(22_050);
        let block_frames = 512;
        let refill_frames = 417;
        let seed = vec![
            StereoSample {
                left: 2500,
                right: -2500,
            };
            8192
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(8192);

        for tick in 0..2400 {
            let mut output = vec![0i16; block_frames * 2];
            queue.pop_interleaved_i16(&mut output);
            assert!(
                output
                    .chunks_exact(2)
                    .all(|frame| frame[0] != 0 || frame[1] != 0),
                "tick {tick}"
            );
            queue.push_slice(&vec![
                StereoSample {
                    left: 2600 + tick,
                    right: -2600 - tick,
                };
                refill_frames
            ]);
        }

        let stats = queue.stats();
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 0, "{stats:?}");
        assert!(stats.repeated_frames > stats.output_frames / 6, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
        assert!(stats.queued_frames > 0, "{stats:?}");
    }

    #[test]
    fn empty_realtime_queue_conceals_short_starvation_before_underflow() {
        let mut queue = BoundedStereoQueue::new(8192);
        queue.push(StereoSample {
            left: 2048,
            right: -2048,
        });
        queue.prepare_for_playback(4096);

        let mut output = vec![0i16; 64 * 2];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(output[0], 2048);
        assert_eq!(output[1], -2048);
        assert!(output[2] != 0);
        assert!(output[3] != 0);
        assert_eq!(stats.popped_frames, 1, "{stats:?}");
        assert_eq!(stats.concealed_frames, 63, "{stats:?}");
        assert_eq!(stats.underflow_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
    }

    #[test]
    fn realtime_concealment_is_bounded_and_then_reports_underflow() {
        let mut queue = BoundedStereoQueue::new(8192);
        queue.push(StereoSample {
            left: 4096,
            right: -4096,
        });
        queue.prepare_for_playback(4096);

        let mut output = vec![0i16; (MAX_CONCEALMENT_FRAMES + 3) * 2];
        queue.pop_interleaved_i16(&mut output);
        let stats = queue.stats();

        assert_eq!(stats.popped_frames, 1, "{stats:?}");
        assert_eq!(
            stats.concealed_frames, MAX_CONCEALMENT_FRAMES as u64,
            "{stats:?}"
        );
        assert_eq!(stats.underflow_frames, 2, "{stats:?}");
        assert_eq!(stats.starvation_events, 1, "{stats:?}");
        assert_eq!(&output[output.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn odd_interleaved_output_remainder_is_cleared() {
        let mut queue = BoundedStereoQueue::new(4);
        queue.push(StereoSample { left: 9, right: -9 });

        let mut output = [7i16; 3];
        queue.pop_interleaved_i16(&mut output);

        assert_eq!(output, [9, -9, 0]);
    }

    #[test]
    fn shared_queue_uses_cached_pcm_on_callback_miss_without_silence() {
        let queue = SharedStereoQueue::new(4);
        queue.push_slice(&[StereoSample {
            left: 123,
            right: -456,
        }]);
        queue.prepare_for_playback(4);
        let mut output = [9i16; 5];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16(&mut output);
        }
        let stats = queue.stats();

        assert_eq!(output, [123, -456, 61, -228, 0]);
        assert_eq!(stats.callback_miss_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_miss_events, 1, "{stats:?}");
        assert_eq!(stats.callback_fallback_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_fallback_events, 1, "{stats:?}");
        assert_eq!(stats.callback_silence_frames, 0, "{stats:?}");
        assert_eq!(stats.callback_silence_events, 0, "{stats:?}");
        assert_eq!(stats.repeated_frames, 0, "{stats:?}");
        assert_eq!(stats.concealed_frames, 0, "{stats:?}");
        assert_eq!(stats.starvation_events, 0, "{stats:?}");
    }

    #[test]
    fn shared_queue_realtime_path_flushes_pending_producer_audio() {
        let queue = SharedStereoQueue::new(8);
        queue.push_slice(&[StereoSample { left: 1, right: -1 }]);
        let frames = [
            StereoSample { left: 2, right: -2 },
            StereoSample { left: 3, right: -3 },
        ];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
        }

        let mut output = [0i16; 4];
        queue.pop_interleaved_i16_realtime(&mut output);
        let realtime = queue.stats();

        assert_eq!(output, [1, -1, 2, -2]);
        assert_eq!(realtime.pending_producer_frames, 0, "{realtime:?}");
        assert_eq!(realtime.popped_frames, 2, "{realtime:?}");
        assert_eq!(realtime.underflow_frames, 0, "{realtime:?}");
        assert_eq!(realtime.queued_frames, 1, "{realtime:?}");

        let mut flushed = [0i16; 4];
        queue.pop_interleaved_i16(&mut flushed);
        let stats = queue.stats();

        assert_eq!(flushed, [3, -3, 0, 0]);
        assert_eq!(stats.pending_producer_frames, 0, "{stats:?}");
    }

    #[test]
    fn shared_queue_realtime_miss_drains_deferred_pcm_without_silence() {
        let queue = SharedStereoQueue::new(4);
        let frames = [
            StereoSample {
                left: 1234,
                right: -1234,
            },
            StereoSample {
                left: 2345,
                right: -2345,
            },
        ];
        let mut output = [0i16; 4];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
            queue.pop_interleaved_i16_realtime(&mut output);
        }
        let stats = queue.stats();

        assert_eq!(output, [1234, -1234, 2345, -2345]);
        assert_eq!(stats.callback_miss_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_rescue_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_rescue_events, 1, "{stats:?}");
        assert_eq!(stats.callback_fallback_frames, 0, "{stats:?}");
        assert_eq!(stats.callback_silence_frames, 0, "{stats:?}");
        assert_eq!(stats.pending_producer_frames, 0, "{stats:?}");
    }

    #[test]
    fn shared_queue_realtime_pending_rescue_is_bounded_and_reports_missing_tail() {
        let queue = SharedStereoQueue::new(CALLBACK_PENDING_RESCUE_MAX_FRAMES * 2);
        let frames = vec![
            StereoSample {
                left: 1550,
                right: -1550,
            };
            CALLBACK_PENDING_RESCUE_MAX_FRAMES + 100
        ];
        let output_frames =
            CALLBACK_PENDING_RESCUE_MAX_FRAMES + CALLBACK_MISS_CONCEALMENT_FRAMES + 4;
        let mut output = vec![9i16; output_frames * 2];

        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
            queue.pop_interleaved_i16_realtime(&mut output);
        }
        let stats = queue.stats();

        assert_eq!(
            stats.callback_rescue_frames, CALLBACK_PENDING_RESCUE_MAX_FRAMES as u64,
            "{stats:?}"
        );
        assert_eq!(stats.callback_rescue_events, 1, "{stats:?}");
        assert_eq!(
            stats.pending_producer_frames,
            frames.len() - CALLBACK_PENDING_RESCUE_MAX_FRAMES,
            "{stats:?}"
        );
        assert_eq!(
            stats.callback_fallback_frames, CALLBACK_MISS_CONCEALMENT_FRAMES as u64,
            "{stats:?}"
        );
        assert_eq!(stats.callback_silence_frames, 4, "{stats:?}");
        assert!(
            output[..CALLBACK_PENDING_RESCUE_MAX_FRAMES * 2]
                .chunks_exact(2)
                .all(|frame| frame[0] == 1550 && frame[1] == -1550)
        );
        assert_eq!(&output[output.len() - 8..], &[0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn shared_queue_callback_miss_conceals_briefly_then_silences_instead_of_repeating_effect() {
        let queue = SharedStereoQueue::new(256);
        queue.push_slice(&[StereoSample {
            left: 3200,
            right: -3200,
        }]);
        queue.prepare_for_playback(256);

        let mut output = vec![9i16; 128 * 2];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16_realtime(&mut output);
        }
        let stats = queue.stats();

        assert!(
            output[..CALLBACK_MISS_CONCEALMENT_FRAMES * 2]
                .chunks_exact(2)
                .any(|frame| frame[0] != 0 || frame[1] != 0)
        );
        assert!(
            output[CALLBACK_MISS_CONCEALMENT_FRAMES * 2..]
                .chunks_exact(2)
                .all(|frame| frame[0] == 0 && frame[1] == 0)
        );
        assert_eq!(stats.callback_miss_frames, 128, "{stats:?}");
        assert_eq!(
            stats.callback_fallback_frames, CALLBACK_MISS_CONCEALMENT_FRAMES as u64,
            "{stats:?}"
        );
        assert_eq!(
            stats.callback_silence_frames,
            (128 - CALLBACK_MISS_CONCEALMENT_FRAMES) as u64,
            "{stats:?}"
        );
    }

    #[test]
    fn shared_queue_consecutive_callback_misses_conceal_once_then_silence() {
        let queue = SharedStereoQueue::new(256);
        queue.push_slice(&[StereoSample {
            left: 3000,
            right: -3000,
        }]);
        queue.prepare_for_playback(256);

        let mut first = vec![9i16; 24 * 2];
        let mut second = vec![9i16; 24 * 2];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16_realtime(&mut first);
            queue.pop_interleaved_i16_realtime(&mut second);
        }
        let stats = queue.stats();

        assert!(
            first
                .chunks_exact(2)
                .all(|frame| frame[0] != 0 || frame[1] != 0),
            "{first:?}"
        );
        assert!(
            second[..(CALLBACK_MISS_CONCEALMENT_FRAMES - 24) * 2]
                .chunks_exact(2)
                .all(|frame| frame[0] != 0 || frame[1] != 0),
            "{second:?}"
        );
        assert!(
            second[(CALLBACK_MISS_CONCEALMENT_FRAMES - 24) * 2..]
                .chunks_exact(2)
                .all(|frame| frame[0] == 0 && frame[1] == 0),
            "{second:?}"
        );
        assert_eq!(stats.callback_miss_frames, 48, "{stats:?}");
        assert_eq!(
            stats.callback_fallback_frames, CALLBACK_MISS_CONCEALMENT_FRAMES as u64,
            "{stats:?}"
        );
        assert_eq!(
            stats.callback_silence_frames,
            (48 - CALLBACK_MISS_CONCEALMENT_FRAMES) as u64,
            "{stats:?}"
        );
    }

    #[test]
    fn shared_queue_callback_miss_concealment_resets_after_successful_pop() {
        let queue = SharedStereoQueue::new(256);
        queue.push_slice(&[StereoSample {
            left: 1200,
            right: -1200,
        }]);
        queue.prepare_for_playback(256);

        let mut first_miss = vec![9i16; CALLBACK_MISS_CONCEALMENT_FRAMES * 2];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16_realtime(&mut first_miss);
        }
        queue.push_slice(&[StereoSample {
            left: 2200,
            right: -2200,
        }]);
        let mut successful = [0i16; 2];
        queue.pop_interleaved_i16_realtime(&mut successful);

        let mut second_miss = [9i16; 2];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16_realtime(&mut second_miss);
        }
        let stats = queue.stats();

        assert_eq!(successful, [1200, -1200]);
        assert_ne!(second_miss, [0, 0]);
        assert_eq!(
            stats.callback_fallback_frames,
            (CALLBACK_MISS_CONCEALMENT_FRAMES + 1) as u64,
            "{stats:?}"
        );
    }

    #[test]
    fn shared_queue_reports_callback_silence_only_when_no_cached_pcm_exists() {
        let queue = SharedStereoQueue::new(4);
        let mut output = [9i16; 4];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.pop_interleaved_i16(&mut output);
        }
        let stats = queue.stats();

        assert_eq!(output, [0, 0, 0, 0]);
        assert_eq!(stats.callback_miss_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_miss_events, 1, "{stats:?}");
        assert_eq!(stats.callback_fallback_frames, 0, "{stats:?}");
        assert_eq!(stats.callback_fallback_events, 0, "{stats:?}");
        assert_eq!(stats.callback_silence_frames, 2, "{stats:?}");
        assert_eq!(stats.callback_silence_events, 1, "{stats:?}");
    }

    #[test]
    fn shared_queue_stats_and_snapshot_do_not_block_callback_lock_holder() {
        let queue = SharedStereoQueue::new(8);
        queue.push_slice(&[
            StereoSample {
                left: 10,
                right: -10,
            },
            StereoSample {
                left: 20,
                right: -20,
            },
        ]);
        queue.record_coreaudio_start();
        queue.record_coreaudio_callback(512);

        let _guard = queue.inner.lock().unwrap();
        let stats = queue.stats();
        let snapshot = queue.snapshot_latest_frames(2);

        assert_eq!(stats.capacity_frames, 8, "{stats:?}");
        assert_eq!(stats.coreaudio_callback_output_frames, 512, "{stats:?}");
        assert!(stats.coreaudio_started, "{stats:?}");
        assert!(stats.coreaudio_running, "{stats:?}");
        assert!(snapshot.is_empty(), "{snapshot:?}");
    }

    #[test]
    fn shared_queue_stats_do_not_block_pending_producer_lock_holder() {
        let queue = SharedStereoQueue::new(8);
        queue.push_slice(&[StereoSample {
            left: 30,
            right: -30,
        }]);
        queue.record_coreaudio_start();

        let _guard = queue.producer_pending.lock().unwrap();
        let stats = queue.stats();

        assert_eq!(stats.capacity_frames, 8, "{stats:?}");
        assert_eq!(stats.queued_frames, 1, "{stats:?}");
        assert_eq!(stats.pending_producer_frames, 0, "{stats:?}");
        assert!(stats.coreaudio_started, "{stats:?}");
    }

    #[test]
    fn shared_queue_defers_producer_audio_instead_of_dropping_on_callback_contention() {
        let queue = SharedStereoQueue::new(8);
        let frames = [
            StereoSample { left: 1, right: -1 },
            StereoSample { left: 2, right: -2 },
            StereoSample { left: 3, right: -3 },
        ];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
        }

        let deferred = queue.stats();
        assert_eq!(deferred.queued_frames, 0, "{deferred:?}");
        assert_eq!(
            deferred.pending_producer_frames,
            frames.len(),
            "{deferred:?}"
        );
        assert_eq!(
            deferred.producer_deferred_frames,
            frames.len() as u64,
            "{deferred:?}"
        );
        assert_eq!(deferred.producer_deferred_events, 1, "{deferred:?}");
        assert_eq!(deferred.producer_miss_frames, 0, "{deferred:?}");
        assert_eq!(deferred.producer_miss_events, 0, "{deferred:?}");

        let mut output = [0i16; 6];
        queue.pop_interleaved_i16(&mut output);
        let flushed = queue.stats();

        assert_eq!(output, [1, -1, 2, -2, 3, -3]);
        assert_eq!(flushed.pending_producer_frames, 0, "{flushed:?}");
        assert_eq!(flushed.pushed_frames, frames.len() as u64, "{flushed:?}");
        assert_eq!(flushed.popped_frames, frames.len() as u64, "{flushed:?}");
        assert_eq!(flushed.underflow_frames, 0, "{flushed:?}");
    }

    #[test]
    fn shared_queue_preserves_producer_audio_after_callback_lock_releases() {
        let queue = SharedStereoQueue::new(8);
        let frames = vec![
            StereoSample {
                left: 321,
                right: -321,
            };
            6
        ];
        {
            let _guard = queue.inner.lock().unwrap();
            assert!(queue.inner.try_lock().is_err());
        }
        queue.push_slice(&frames);
        let stats = queue.stats();

        assert_eq!(queue.len(), 6);
        assert_eq!(stats.pushed_frames, 6, "{stats:?}");
        assert_eq!(stats.producer_miss_frames, 0, "{stats:?}");
        assert_eq!(stats.producer_miss_events, 0, "{stats:?}");
    }

    #[test]
    fn shared_queue_bounds_deferred_audio_and_reports_only_overflow_as_miss() {
        let queue = SharedStereoQueue::new(8);
        let frames = vec![
            StereoSample {
                left: 11,
                right: -11,
            };
            PRODUCER_DEFERRED_MAX_FRAMES + 4
        ];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
        }
        let deferred = queue.stats();

        assert_eq!(
            deferred.pending_producer_frames, PRODUCER_DEFERRED_MAX_FRAMES,
            "{deferred:?}"
        );
        assert_eq!(deferred.producer_miss_frames, 4, "{deferred:?}");
        assert_eq!(deferred.producer_miss_events, 1, "{deferred:?}");
        assert_eq!(
            deferred.producer_deferred_frames, PRODUCER_DEFERRED_MAX_FRAMES as u64,
            "{deferred:?}"
        );
    }

    #[test]
    fn realtime_flush_of_large_deferred_burst_is_bounded_per_callback() {
        let queue = SharedStereoQueue::new(PRODUCER_PENDING_FLUSH_MAX_FRAMES * 2 + 16);
        let frames = vec![
            StereoSample {
                left: 77,
                right: -77,
            };
            PRODUCER_PENDING_FLUSH_MAX_FRAMES + 8
        ];
        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
        }
        assert_eq!(queue.stats().pending_producer_frames, frames.len());

        let mut output = [0i16; 4];
        queue.pop_interleaved_i16_realtime(&mut output);
        let first = queue.stats();
        let first_flush_limit = realtime_pending_flush_limit(output.len());

        assert_eq!(output, [77, -77, 77, -77]);
        assert_eq!(first.popped_frames, 2, "{first:?}");
        assert_eq!(
            first.pending_producer_frames,
            frames.len() - first_flush_limit,
            "{first:?}"
        );
        assert_eq!(first.queued_frames, first_flush_limit - 2, "{first:?}");

        let mut next_output = [0i16; 16];
        queue.pop_interleaved_i16_realtime(&mut next_output);
        let second = queue.stats();
        let second_flush_limit = realtime_pending_flush_limit(next_output.len());

        assert_eq!(
            second.pending_producer_frames,
            frames.len() - first_flush_limit - second_flush_limit,
            "{second:?}"
        );
        assert_eq!(second.underflow_frames, 0, "{second:?}");
        assert_eq!(second.callback_silence_frames, 0, "{second:?}");
        assert!(
            next_output
                .chunks_exact(2)
                .all(|frame| frame[0] != 0 || frame[1] != 0)
        );
    }

    #[test]
    fn proactive_realtime_guard_reports_bursty_gui_slowdown_instead_of_masking_it() {
        let mut queue = BoundedStereoQueue::new(22_050);
        let callback_frames = 512;
        let burst_period_callbacks = 8;
        let refill_per_callback = 188;
        let seed = vec![
            StereoSample {
                left: 1800,
                right: -1800,
            };
            8192
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(8192);

        for callback_index in 0..4200 {
            let mut output = vec![0i16; callback_frames * 2];
            queue.pop_interleaved_i16(&mut output);

            if callback_index % burst_period_callbacks == burst_period_callbacks - 1 {
                let refill_frames = refill_per_callback * burst_period_callbacks;
                let refill = vec![
                    StereoSample {
                        left: 1900 + (callback_index % 97) as i16,
                        right: -1900 - (callback_index % 97) as i16,
                    };
                    refill_frames
                ];
                queue.push_slice(&refill);
            }
        }

        let stats = queue.stats();
        assert_eq!(stats.output_frames, 2_150_400, "{stats:?}");
        assert!(stats.underflow_frames > 0, "{stats:?}");
        assert!(stats.starvation_events > 0, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
        assert!(
            stats.popped_frames <= stats.pushed_frames + 8192,
            "{stats:?}"
        );
    }

    #[test]
    fn shared_queue_recovers_from_poisoned_mutex_instead_of_silencing_forever() {
        let queue = SharedStereoQueue::new(4);
        let inner = queue.inner.clone();
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = inner.lock().unwrap();
            panic!("poison queue lock");
        }));
        panic::set_hook(previous_hook);

        queue.push_slice(&[StereoSample { left: 7, right: 8 }]);
        let mut output = [0i16; 2];
        queue.pop_interleaved_i16(&mut output);

        assert_eq!(output, [7, 8]);
    }

    #[test]
    fn playback_preparation_flushes_deferred_pcm_before_coreaudio_primes_buffers() {
        let queue = SharedStereoQueue::new(8);
        let frames = [
            StereoSample {
                left: 101,
                right: -101,
            },
            StereoSample {
                left: 202,
                right: -202,
            },
            StereoSample {
                left: 303,
                right: -303,
            },
        ];

        {
            let _guard = queue.inner.lock().unwrap();
            queue.push_slice(&frames);
        }
        assert_eq!(queue.stats().pending_producer_frames, frames.len());

        queue.prepare_for_playback(8);
        let prepared = queue.stats();
        assert_eq!(prepared.pending_producer_frames, 0, "{prepared:?}");
        assert_eq!(prepared.queued_frames, frames.len(), "{prepared:?}");
        assert_eq!(prepared.peak_queued_frames, frames.len(), "{prepared:?}");

        let mut output = [0i16; 6];
        queue.pop_interleaved_i16_realtime(&mut output);
        let realtime = queue.stats();
        assert_eq!(output, [101, -101, 202, -202, 303, -303]);
        assert_eq!(realtime.underflow_frames, 0, "{realtime:?}");
        assert_eq!(realtime.callback_silence_frames, 0, "{realtime:?}");
    }

    #[test]
    fn shared_queue_reports_push_progress_while_callback_holds_inner_lock() {
        let queue = SharedStereoQueue::new(8);
        let frames = [
            StereoSample {
                left: 101,
                right: -101,
            },
            StereoSample {
                left: 202,
                right: -202,
            },
            StereoSample {
                left: 303,
                right: -303,
            },
        ];

        let guard = queue.inner.lock().unwrap();
        queue.push_slice(&frames);
        let stats = queue.stats();
        drop(guard);

        assert_eq!(stats.pushed_frames, frames.len() as u64, "{stats:?}");
        assert_eq!(stats.pending_producer_frames, frames.len(), "{stats:?}");
    }

    #[test]
    fn playback_preparation_resets_lock_free_push_progress() {
        let queue = SharedStereoQueue::new(8);
        queue.push_slice(&[
            StereoSample { left: 1, right: -1 },
            StereoSample { left: 2, right: -2 },
        ]);

        queue.prepare_for_playback(8);

        assert_eq!(queue.stats().pushed_frames, 0);
    }
}
