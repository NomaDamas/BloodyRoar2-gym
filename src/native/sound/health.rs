use crate::native::sound::ps9805::Ps9805SoundStats;
use crate::native::sound::queue::StereoQueueStats;
use crate::native::sound::ymf271::Ymf271Stats;

const MAX_HEALTHY_REPEATED_OUTPUT_RATIO_NUMERATOR: u64 = 1;
const MAX_HEALTHY_REPEATED_OUTPUT_RATIO_DENOMINATOR: u64 = 3;
const MAX_HEALTHY_CALLBACK_MISS_RATIO_NUMERATOR: u64 = 1;
const MAX_HEALTHY_CALLBACK_MISS_RATIO_DENOMINATOR: u64 = 100;
const MAX_HEALTHY_CALLBACK_SILENCE_RATIO_NUMERATOR: u64 = 1;
const MAX_HEALTHY_CALLBACK_SILENCE_RATIO_DENOMINATOR: u64 = 100;
const MAX_HEALTHY_UNDERFLOW_RATIO_NUMERATOR: u64 = 1;
const MAX_HEALTHY_UNDERFLOW_RATIO_DENOMINATOR: u64 = 100;
const MAX_HEALTHY_CATCHUP_RATIO_NUMERATOR: u64 = 1;
const MAX_HEALTHY_CATCHUP_RATIO_DENOMINATOR: u64 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioHealthState {
    NoRender,
    SilentPcm,
    OutputIdle,
    RealtimeStarved,
    CallbackBlocked,
    Active,
}

impl AudioHealthState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoRender => "no_render",
            Self::SilentPcm => "silent_pcm",
            Self::OutputIdle => "output_idle",
            Self::RealtimeStarved => "realtime_starved",
            Self::CallbackBlocked => "callback_blocked",
            Self::Active => "active",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioHealth {
    pub state: AudioHealthState,
    pub reason: &'static str,
    pub render_progressing: bool,
    pub pcm_nonzero: bool,
    pub realtime_output_seen: bool,
    pub realtime_ok: bool,
}

impl AudioHealth {
    pub fn from_stats(sound: Ps9805SoundStats, ymf: Ymf271Stats, queue: StereoQueueStats) -> Self {
        let render_progressing = sound.audio_render_batches > 0
            || sound.audio_queue_push_batches > 0
            || ymf.generated_frames > 0
            || queue.pushed_frames > 0;
        let pcm_nonzero = ymf.nonzero_frames > 0 || ymf.last_rms_left > 0 || ymf.last_rms_right > 0;
        let coreaudio_started = sound.coreaudio_started && queue.coreaudio_started;
        let realtime_output_seen = queue.coreaudio_callback_output_frames > 0;
        let callback_silence_excessive = callback_silence_excessive(queue);
        let callback_miss_excessive = callback_miss_excessive(queue);
        let callback_failed =
            queue.coreaudio_enqueue_errors > 0 || (coreaudio_started && !queue.coreaudio_running);
        let callback_blocked =
            callback_failed || callback_silence_excessive || callback_miss_excessive;
        let producer_blocked = queue.producer_miss_events > 0 || queue.producer_miss_frames > 0;
        let repeated_output_excessive = repeated_output_excessive(queue);
        let underflow_excessive = underflow_excessive(queue);
        let realtime_catchup_excessive = realtime_catchup_excessive(sound, ymf, queue);
        let realtime_starved = producer_blocked
            || underflow_excessive
            || repeated_output_excessive
            || realtime_catchup_excessive;

        let (state, reason) = if !render_progressing {
            (
                AudioHealthState::NoRender,
                "audio_renderer_has_not_produced_frames",
            )
        } else if !pcm_nonzero {
            (AudioHealthState::SilentPcm, "rendered_pcm_is_silent")
        } else if !coreaudio_started {
            (AudioHealthState::OutputIdle, "coreaudio_not_started")
        } else if !realtime_output_seen {
            (
                AudioHealthState::OutputIdle,
                "coreaudio_callback_has_not_output_frames",
            )
        } else if callback_blocked {
            let reason = if callback_failed {
                "coreaudio_callback_stopped_or_failed"
            } else if callback_silence_excessive {
                "coreaudio_callback_silence_ratio_exceeded"
            } else {
                "coreaudio_callback_miss_ratio_exceeded"
            };
            (AudioHealthState::CallbackBlocked, reason)
        } else if realtime_starved {
            (
                AudioHealthState::RealtimeStarved,
                if producer_blocked {
                    "audio_producer_dropped_realtime_pcm"
                } else if underflow_excessive {
                    "coreaudio_queue_underflow_ratio_exceeded"
                } else if realtime_catchup_excessive {
                    "audio_realtime_catchup_ratio_exceeded"
                } else {
                    "coreaudio_output_overstretched"
                },
            )
        } else {
            (
                AudioHealthState::Active,
                "pcm_nonzero_and_realtime_queue_healthy",
            )
        };

        Self {
            state,
            reason,
            render_progressing,
            pcm_nonzero,
            realtime_output_seen,
            realtime_ok: coreaudio_started
                && realtime_output_seen
                && !callback_blocked
                && !realtime_starved,
        }
    }

    pub fn audible(self) -> bool {
        self.state == AudioHealthState::Active
    }

    pub fn json(self) -> String {
        format!(
            "{{\"state\":\"{}\",\"reason\":\"{}\",\"audible\":{},\"render_progressing\":{},\"pcm_nonzero\":{},\"realtime_output_seen\":{},\"realtime_ok\":{}}}",
            self.state.as_str(),
            self.reason,
            self.audible(),
            self.render_progressing,
            self.pcm_nonzero,
            self.realtime_output_seen,
            self.realtime_ok
        )
    }
}

fn repeated_output_excessive(queue: StereoQueueStats) -> bool {
    let realtime_frames = realtime_frame_count(queue);
    let repeated_like_frames = queue
        .repeated_frames
        .saturating_add(queue.callback_fallback_frames);
    realtime_frames > 0
        && ratio_exceeds(
            repeated_like_frames,
            realtime_frames,
            MAX_HEALTHY_REPEATED_OUTPUT_RATIO_NUMERATOR,
            MAX_HEALTHY_REPEATED_OUTPUT_RATIO_DENOMINATOR,
        )
}

fn callback_miss_excessive(queue: StereoQueueStats) -> bool {
    let realtime_frames = realtime_frame_count(queue);
    let unresolved_miss_frames = queue
        .callback_miss_frames
        .saturating_sub(queue.callback_rescue_frames);
    realtime_frames > 0
        && ratio_exceeds(
            unresolved_miss_frames,
            realtime_frames,
            MAX_HEALTHY_CALLBACK_MISS_RATIO_NUMERATOR,
            MAX_HEALTHY_CALLBACK_MISS_RATIO_DENOMINATOR,
        )
}

fn callback_silence_excessive(queue: StereoQueueStats) -> bool {
    let realtime_frames = realtime_frame_count(queue);
    realtime_frames > 0
        && ratio_exceeds(
            queue.callback_silence_frames,
            realtime_frames,
            MAX_HEALTHY_CALLBACK_SILENCE_RATIO_NUMERATOR,
            MAX_HEALTHY_CALLBACK_SILENCE_RATIO_DENOMINATOR,
        )
}

fn underflow_excessive(queue: StereoQueueStats) -> bool {
    let realtime_frames = realtime_frame_count(queue);
    realtime_frames > 0
        && ratio_exceeds(
            queue.underflow_frames,
            realtime_frames,
            MAX_HEALTHY_UNDERFLOW_RATIO_NUMERATOR,
            MAX_HEALTHY_UNDERFLOW_RATIO_DENOMINATOR,
        )
}

fn realtime_catchup_excessive(
    sound: Ps9805SoundStats,
    ymf: Ymf271Stats,
    queue: StereoQueueStats,
) -> bool {
    let rendered_frames = ymf
        .generated_frames
        .max(queue.pushed_frames)
        .max(sound.audio_realtime_catchup_frames);
    rendered_frames > 0
        && ratio_exceeds(
            sound.audio_realtime_catchup_frames,
            rendered_frames,
            MAX_HEALTHY_CATCHUP_RATIO_NUMERATOR,
            MAX_HEALTHY_CATCHUP_RATIO_DENOMINATOR,
        )
}

fn realtime_frame_count(queue: StereoQueueStats) -> u64 {
    queue
        .output_frames
        .saturating_add(queue.callback_miss_frames)
}

fn ratio_exceeds(value: u64, total: u64, numerator: u64, denominator: u64) -> bool {
    total > 0 && value.saturating_mul(denominator) > total.saturating_mul(numerator)
}

#[cfg(test)]
mod tests {
    use super::{AudioHealth, AudioHealthState};
    use crate::native::sound::ps9805::Ps9805SoundStats;
    use crate::native::sound::queue::StereoQueueStats;
    use crate::native::sound::ymf271::Ymf271Stats;

    fn health(ymf: Ymf271Stats, mut queue: StereoQueueStats) -> AudioHealth {
        if queue.output_frames > 0 && !queue.coreaudio_started {
            queue.coreaudio_started = true;
            queue.coreaudio_running = true;
            queue.coreaudio_callback_count = 1;
            queue.coreaudio_callback_output_frames = queue.output_frames;
        }
        AudioHealth::from_stats(
            Ps9805SoundStats {
                available: true,
                audio_render_batches: u64::from(ymf.generated_frames > 0),
                audio_queue_push_batches: u64::from(queue.pushed_frames > 0),
                coreaudio_started: queue.coreaudio_started,
                ..Ps9805SoundStats::default()
            },
            ymf,
            queue,
        )
    }

    #[test]
    fn health_reports_silent_pcm_when_frames_are_generated_but_zero() {
        let result = health(
            Ymf271Stats {
                generated_frames: 1024,
                nonzero_frames: 0,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 1024,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::SilentPcm);
        assert!(result.render_progressing);
        assert!(!result.pcm_nonzero);
        assert!(!result.audible());
    }

    #[test]
    fn health_reports_callback_block_before_generic_starvation() {
        let result = health(
            Ymf271Stats {
                generated_frames: 1024,
                nonzero_frames: 1024,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 1024,
                output_frames: 512,
                underflow_frames: 8,
                starvation_events: 1,
                callback_miss_events: 1,
                callback_miss_frames: 512,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::CallbackBlocked);
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_allows_single_callback_fallback_when_drop_and_repeat_ratios_are_low() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4_158_672,
                nonzero_frames: 2_557_356,
                last_rms_left: 5_952,
                last_rms_right: 5_952,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 2_678_832,
                popped_frames: 2_687_959,
                output_frames: 2_731_008,
                repeated_frames: 43_049,
                callback_miss_frames: 512,
                callback_miss_events: 1,
                callback_fallback_frames: 512,
                callback_fallback_events: 1,
                queued_frames: 11_353,
                capacity_frames: 22_050,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert_eq!(result.reason, "pcm_nonzero_and_realtime_queue_healthy");
        assert!(result.audible());
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_reports_callback_block_when_miss_ratio_is_excessive() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                callback_miss_frames: 512,
                callback_miss_events: 1,
                callback_fallback_frames: 512,
                callback_fallback_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::CallbackBlocked);
        assert_eq!(result.reason, "coreaudio_callback_miss_ratio_exceeded");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_allows_rescued_callback_miss_when_pending_pcm_kept_output_continuous() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                callback_miss_frames: 512,
                callback_miss_events: 1,
                callback_rescue_frames: 512,
                callback_rescue_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert_eq!(result.reason, "pcm_nonzero_and_realtime_queue_healthy");
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_allows_transient_startup_callback_silence_after_output_recovers() {
        let result = health(
            Ymf271Stats {
                generated_frames: 65_536,
                nonzero_frames: 65_536,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 65_536,
                output_frames: 65_536,
                callback_miss_frames: 32,
                callback_miss_events: 1,
                callback_fallback_frames: 16,
                callback_fallback_events: 1,
                callback_silence_frames: 16,
                callback_silence_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_reports_callback_block_when_silence_ratio_is_excessive() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                callback_miss_frames: 128,
                callback_miss_events: 1,
                callback_fallback_frames: 32,
                callback_fallback_events: 1,
                callback_silence_frames: 96,
                callback_silence_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::CallbackBlocked);
        assert_eq!(result.reason, "coreaudio_callback_silence_ratio_exceeded");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_reports_producer_drop_as_realtime_starvation() {
        let result = health(
            Ymf271Stats {
                generated_frames: 1024,
                nonzero_frames: 1024,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 1024,
                output_frames: 512,
                producer_miss_events: 1,
                producer_miss_frames: 128,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::RealtimeStarved);
        assert_eq!(result.reason, "audio_producer_dropped_realtime_pcm");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_allows_transient_startup_underflow_after_realtime_recovers() {
        let result = health(
            Ymf271Stats {
                generated_frames: 65_536,
                nonzero_frames: 65_536,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 65_536,
                output_frames: 65_536,
                underflow_frames: 32,
                starvation_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_reports_realtime_starved_when_underflow_ratio_is_excessive() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                underflow_frames: 128,
                starvation_events: 1,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::RealtimeStarved);
        assert_eq!(result.reason, "coreaudio_queue_underflow_ratio_exceeded");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_allows_deferred_producer_frames_when_queue_output_is_continuous() {
        let result = health(
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 2048,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                producer_deferred_events: 3,
                producer_deferred_frames: 256,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_reports_excessive_realtime_stretch_as_starvation() {
        let result = health(
            Ymf271Stats {
                generated_frames: 8192,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                repeated_frames: 2048,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::RealtimeStarved);
        assert_eq!(result.reason, "coreaudio_output_overstretched");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_accepts_adaptive_resampling_for_native_gui_slowdown() {
        let result = health(
            Ymf271Stats {
                generated_frames: 2_257_828,
                nonzero_frames: 663_152,
                last_rms_left: 2_790,
                last_rms_right: 2_790,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                capacity_frames: 22_050,
                queued_frames: 7_468,
                pushed_frames: 777_988,
                popped_frames: 786_904,
                output_frames: 998_400,
                repeated_frames: 211_496,
                coreaudio_callback_count: 1_944,
                coreaudio_callback_output_frames: 995_328,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert_eq!(result.reason, "pcm_nonzero_and_realtime_queue_healthy");
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_reports_excessive_ps9805_realtime_catchup_as_starvation() {
        let result = AudioHealth::from_stats(
            Ps9805SoundStats {
                available: true,
                audio_render_batches: 4,
                audio_queue_push_batches: 4,
                audio_realtime_catchup_frames: 2048,
                coreaudio_started: true,
                ..Ps9805SoundStats::default()
            },
            Ymf271Stats {
                generated_frames: 4096,
                nonzero_frames: 4096,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 4096,
                output_frames: 4096,
                coreaudio_started: true,
                coreaudio_running: true,
                coreaudio_callback_count: 8,
                coreaudio_callback_output_frames: 4096,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::RealtimeStarved);
        assert_eq!(result.reason, "audio_realtime_catchup_ratio_exceeded");
        assert!(!result.realtime_ok);
    }

    #[test]
    fn health_reports_active_when_pcm_is_nonzero_and_queue_has_no_gap() {
        let result = health(
            Ymf271Stats {
                generated_frames: 1024,
                nonzero_frames: 512,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 1024,
                output_frames: 512,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::Active);
        assert!(result.audible());
        assert!(result.realtime_ok);
    }

    #[test]
    fn health_does_not_claim_audible_before_coreaudio_callback_progresses() {
        let result = health(
            Ymf271Stats {
                generated_frames: 1024,
                nonzero_frames: 512,
                ..Ymf271Stats::default()
            },
            StereoQueueStats {
                pushed_frames: 1024,
                ..StereoQueueStats::default()
            },
        );

        assert_eq!(result.state, AudioHealthState::OutputIdle);
        assert_eq!(result.reason, "coreaudio_not_started");
        assert!(!result.audible());
        assert!(!result.realtime_output_seen);
        assert!(!result.realtime_ok);
    }
}
