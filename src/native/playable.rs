use crate::action::{Action, ActionButtons};
use crate::backend::BackendError;
use crate::native::emulator::NativeEmulator;

const PLAYABLE_STARTUP_INSTRUCTIONS_PER_FRAME: u64 = 500_000;
const PLAYABLE_STARTUP_INSTRUCTION_SLICE: u64 = 5_000;
const WARNING_SKIP_INITIAL_FRAMES: u64 = 30;
const WARNING_SKIP_PULSE_FRAMES: u64 = 12;
const WARNING_SKIP_GAP_FRAMES: u64 = 24;
const TITLE_ENTRY_FRAMES: u64 = 528;
const TITLE_RUNTIME_MIN_P1_POLLS: u64 = 1_150;
const TITLE_READY_EXTRA_WAIT_FRAMES: u64 = 1_800;
const PLAYABLE_RENDER_SETTLE_FRAMES: u64 = 120;
const PLAYABLE_EXTRA_WAIT_FRAMES: u64 = 900;
const PLAYABLE_EXTRA_WAIT_SAMPLE_FRAMES: u64 = 120;
const PLAYABLE_PRESENTATION_CAPTURE_INTERVAL: u64 = 6_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePlayableSegment {
    pub action: Action,
    pub frames: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePlayableStartup {
    pub frames: u64,
    pub playable: bool,
}

pub fn native_playable_match_entry_script() -> Vec<NativePlayableSegment> {
    let mut segments = vec![
        segment(Action::Noop, WARNING_SKIP_INITIAL_FRAMES),
        segment(Action::Start, WARNING_SKIP_PULSE_FRAMES),
        segment(Action::Noop, WARNING_SKIP_GAP_FRAMES),
        segment(Action::Punch, WARNING_SKIP_PULSE_FRAMES),
        segment(Action::Noop, WARNING_SKIP_GAP_FRAMES),
        segment(Action::Start, WARNING_SKIP_PULSE_FRAMES),
        segment(Action::Noop, WARNING_SKIP_GAP_FRAMES),
        segment(Action::Noop, TITLE_ENTRY_FRAMES),
        segment(Action::Coin, 12),
        segment(Action::Noop, 24),
        segment(Action::Start, 18),
        segment(Action::Noop, 120),
        segment(Action::Punch, 30),
        segment(Action::Noop, 120),
        segment(Action::Punch, 30),
        segment(Action::Noop, 180),
    ];

    for action in [
        Action::Up,
        Action::Down,
        Action::Left,
        Action::Right,
        Action::Punch,
        Action::Kick,
        Action::Beast,
        Action::Guard,
    ] {
        segments.push(segment(action, 6));
        segments.push(segment(Action::Noop, 6));
    }
    segments.push(segment(Action::Noop, 180));
    segments
}

pub fn prepare_native_playable_emulator(
    emulator: &mut NativeEmulator,
) -> Result<NativePlayableStartup, BackendError> {
    let mut frames = 0_u64;
    let script = native_playable_match_entry_script();
    let first_credit = script
        .iter()
        .position(|segment| native_playable_segment_is_credit_attempt(*segment))
        .unwrap_or(script.len());
    let (boot_segments, match_entry_segments) = script.split_at(first_credit);

    emulator.set_native_otc_dma_recovery_suppressed(true);
    emulator.set_vblank_presentation_capture_interval(None);
    emulator.set_unlinked_primitive_replay_enabled(false);
    emulator.set_unlinked_primitive_replay_interval(None);

    run_segments(emulator, boot_segments, &mut frames)?;

    emulator.set_input(ActionButtons::default());
    for _ in 0..TITLE_READY_EXTRA_WAIT_FRAMES {
        if native_title_runtime_poll_ready(emulator) {
            break;
        }
        advance_one_script_frame(emulator)?;
        frames = frames.saturating_add(1);
        ensure_running(emulator)?;
    }
    if !native_title_runtime_poll_ready(emulator) {
        return Err(BackendError::new(format!(
            "native playable startup completed {frames} boot frames without reaching title input polling readiness: {}",
            emulator.input_activity_json()
        )));
    }

    run_segments(emulator, match_entry_segments, &mut frames)?;

    emulator.set_input(ActionButtons::default());
    emulator.set_native_otc_dma_recovery_suppressed(false);
    emulator.set_vblank_presentation_capture_interval(Some(PLAYABLE_PRESENTATION_CAPTURE_INTERVAL));
    for _ in 0..PLAYABLE_RENDER_SETTLE_FRAMES {
        advance_one_script_frame(emulator)?;
        frames = frames.saturating_add(1);
    }
    emulator.capture_vblank_presented_frame();
    if emulator.native_playable_candidate() {
        return finish_playable_startup(emulator, frames);
    }

    for _ in 0..PLAYABLE_EXTRA_WAIT_FRAMES {
        advance_one_script_frame(emulator)?;
        frames = frames.saturating_add(1);
        if frames.is_multiple_of(PLAYABLE_EXTRA_WAIT_SAMPLE_FRAMES) {
            emulator.capture_vblank_presented_frame();
            if emulator.native_playable_candidate() {
                return finish_playable_startup(emulator, frames);
            }
        }
    }

    Err(BackendError::new(format!(
        "native playable startup completed {frames} frames without reaching a playable rendered state"
    )))
}

fn segment(action: Action, frames: u64) -> NativePlayableSegment {
    NativePlayableSegment { action, frames }
}

fn native_playable_segment_is_credit_attempt(segment: NativePlayableSegment) -> bool {
    matches!(
        segment.action,
        Action::Coin | Action::CoinStart | Action::Service
    )
}

fn native_title_runtime_poll_ready(emulator: &NativeEmulator) -> bool {
    let activity = emulator.input_activity();
    activity.p1_input_reads >= TITLE_RUNTIME_MIN_P1_POLLS
        && activity.p3_input_reads >= TITLE_RUNTIME_MIN_P1_POLLS
        && activity.system_input_reads >= activity.p1_input_reads.saturating_mul(2)
        && activity.p1_punch_active_reads > 0
        && (activity.system_start_active_reads > 0 || activity.p1_start_active_reads > 0)
        && activity.coin_register_writes > 0
        && activity.native_credit_adapter_writes > 0
}

fn run_segments(
    emulator: &mut NativeEmulator,
    segments: &[NativePlayableSegment],
    frames: &mut u64,
) -> Result<(), BackendError> {
    for segment in segments {
        emulator.set_input(segment.action.buttons());
        for _ in 0..segment.frames {
            advance_one_script_frame(emulator)?;
            *frames = frames.saturating_add(1);
            ensure_running(emulator)?;
        }
    }
    Ok(())
}

fn ensure_running(emulator: &NativeEmulator) -> Result<(), BackendError> {
    if emulator.is_terminal() {
        return Err(BackendError::new(
            "native emulator terminated during playable startup",
        ));
    }
    Ok(())
}

fn finish_playable_startup(
    emulator: &mut NativeEmulator,
    frames: u64,
) -> Result<NativePlayableStartup, BackendError> {
    emulator.set_input(ActionButtons::default());
    Ok(NativePlayableStartup {
        frames,
        playable: true,
    })
}

fn advance_one_script_frame(emulator: &mut NativeEmulator) -> Result<(), BackendError> {
    let start_vblank = emulator.vblank_count();
    let mut executed = 0_u64;
    while emulator.vblank_count() == start_vblank
        && !emulator.is_terminal()
        && executed < PLAYABLE_STARTUP_INSTRUCTIONS_PER_FRAME
    {
        let remaining = PLAYABLE_STARTUP_INSTRUCTIONS_PER_FRAME - executed;
        let slice = remaining.min(PLAYABLE_STARTUP_INSTRUCTION_SLICE);
        let slice_executed = emulator.step_instructions(slice);
        executed = executed.saturating_add(slice_executed);
        if slice_executed == 0 {
            break;
        }
    }

    if emulator.vblank_count() == start_vblank {
        return Err(BackendError::new(format!(
            "native playable startup did not reach vblank within {PLAYABLE_STARTUP_INSTRUCTIONS_PER_FRAME} instructions"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{native_playable_match_entry_script, native_playable_segment_is_credit_attempt};
    use crate::Action;

    #[test]
    fn playable_script_contains_boot_match_and_all_controls() {
        let script = native_playable_match_entry_script();
        let total_frames = script.iter().map(|segment| segment.frames).sum::<u64>();

        assert_eq!(total_frames, 1_476);
        for action in [
            Action::Coin,
            Action::Start,
            Action::Up,
            Action::Down,
            Action::Left,
            Action::Right,
            Action::Punch,
            Action::Kick,
            Action::Beast,
            Action::Guard,
        ] {
            assert!(script.iter().any(|segment| segment.action == action));
        }
    }

    #[test]
    fn playable_script_splits_boot_wait_from_credit_tail() {
        let script = native_playable_match_entry_script();
        let first_credit = script
            .iter()
            .position(|segment| native_playable_segment_is_credit_attempt(*segment))
            .expect("playable script must contain a credit action");
        let boot_frames = script[..first_credit]
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();
        let tail_frames = script[first_credit..]
            .iter()
            .map(|segment| segment.frames)
            .sum::<u64>();

        assert_eq!(boot_frames, 666);
        assert_eq!(tail_frames, 810);
        assert!(
            script[..first_credit]
                .iter()
                .all(|segment| !native_playable_segment_is_credit_attempt(*segment))
        );
        assert_eq!(script[first_credit].action, Action::Coin);
    }
}
