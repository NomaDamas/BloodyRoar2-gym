use crate::action::{Action, ActionButtons};
use crate::backend::BackendError;
use crate::native::emulator::{NativeDisplayFrame, NativeEmulator};

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
const ROSTER_TOP_ROW_COLUMNS: usize = 5;
const ROSTER_SLOTS: usize = 11;
const ROSTER_P2_INITIAL_SLOT: usize = 4;
const ROSTER_NAVIGATION_PULSE_FRAMES: u64 = 1;
const ROSTER_NAVIGATION_RELEASE_FRAMES: u64 = 24;
const ROSTER_PREVIEW_SETTLE_FRAMES: u64 = 90;
const ROSTER_SELECT_CONFIRM_FRAMES: u64 = 36;
const ROSTER_SELECT_CONFIRM_RELEASE_FRAMES: u64 = 36;
const ROSTER_P2_JOIN_SETTLE_FRAMES: u64 = 90;
const ROSTER_GAMEPLAY_WAIT_FRAMES: u64 = 1_800;
const ROSTER_GAMEPLAY_MIN_SETTLE_FRAMES: u64 = 120;
const ROSTER_GAMEPLAY_CHECK_STRIDE_FRAMES: u64 = 6;
const ROSTER_GAMEPLAY_POST_READY_SETTLE_FRAMES: u64 = 180;

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
        segment(Action::Noop, 24),
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
    if native_player_session_ready(emulator) {
        return finish_playable_startup(emulator, frames);
    }

    for _ in 0..PLAYABLE_EXTRA_WAIT_FRAMES {
        advance_one_script_frame(emulator)?;
        frames = frames.saturating_add(1);
        if frames.is_multiple_of(PLAYABLE_EXTRA_WAIT_SAMPLE_FRAMES) {
            emulator.capture_vblank_presented_frame();
            if native_player_session_ready(emulator) {
                return finish_playable_startup(emulator, frames);
            }
        }
    }

    Err(BackendError::new(format!(
        "native playable startup completed {frames} frames without reaching a credited player session: {}",
        emulator.br2_native_credit_hle_json()
    )))
}

pub fn prepare_native_character_select_emulator(
    emulator: &mut NativeEmulator,
) -> Result<u64, BackendError> {
    let script = native_playable_match_entry_script();
    let first_credit = script
        .iter()
        .position(|segment| native_playable_segment_is_credit_attempt(*segment))
        .unwrap_or(script.len());
    let boot_segments = &script[..first_credit];
    let mut frames = 0_u64;

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
            "native character-select startup completed {frames} boot frames without reaching title input polling readiness: {}",
            emulator.input_activity_json()
        )));
    }

    emulator.set_native_otc_dma_recovery_suppressed(false);
    emulator.set_vblank_presentation_capture_interval(Some(PLAYABLE_PRESENTATION_CAPTURE_INTERVAL));
    emulator.set_unlinked_primitive_replay_enabled(true);
    emulator.set_unlinked_primitive_replay_interval(None);
    run_segments(
        emulator,
        &[
            segment(Action::Coin, 18),
            segment(Action::Noop, 24),
            segment(Action::Start, 24),
            segment(Action::Noop, 240),
        ],
        &mut frames,
    )?;
    emulator.set_input(ActionButtons::default());
    emulator.capture_vblank_presented_frame();
    ensure_running(emulator)?;
    Ok(frames)
}

pub fn prepare_native_selected_match_emulator(
    emulator: &mut NativeEmulator,
    p1_slot: usize,
    p2_slot: usize,
) -> Result<NativePlayableStartup, BackendError> {
    if p1_slot >= ROSTER_SLOTS || p2_slot >= ROSTER_SLOTS {
        return Err(BackendError::new(format!(
            "native roster slots must be between 0 and {}",
            ROSTER_SLOTS - 1
        )));
    }

    let mut frames = 0_u64;
    let p1_navigation = native_roster_navigation_segments(p1_slot, false);
    run_segments(emulator, &p1_navigation, &mut frames)?;
    run_segments(
        emulator,
        &[
            segment(Action::P2Coin, 18),
            segment(Action::Noop, 24),
            segment(Action::P2Start, 24),
            segment(Action::Noop, ROSTER_P2_JOIN_SETTLE_FRAMES),
        ],
        &mut frames,
    )?;
    let p2_navigation = native_roster_navigation_segments(p2_slot, true);
    run_segments(emulator, &p2_navigation, &mut frames)?;
    run_segments(
        emulator,
        &[
            segment(Action::Punch, ROSTER_SELECT_CONFIRM_FRAMES),
            segment(Action::Noop, ROSTER_SELECT_CONFIRM_RELEASE_FRAMES),
            segment(Action::P2Punch, ROSTER_SELECT_CONFIRM_FRAMES),
            segment(Action::Noop, ROSTER_SELECT_CONFIRM_RELEASE_FRAMES),
        ],
        &mut frames,
    )?;

    emulator.set_input(ActionButtons::default());
    for gameplay_frame in 1..=ROSTER_GAMEPLAY_WAIT_FRAMES {
        advance_one_script_frame(emulator)?;
        frames = frames.saturating_add(1);
        ensure_running(emulator)?;
        if gameplay_frame >= ROSTER_GAMEPLAY_MIN_SETTLE_FRAMES
            && gameplay_frame.is_multiple_of(ROSTER_GAMEPLAY_CHECK_STRIDE_FRAMES)
        {
            emulator.capture_vblank_presented_frame();
            if native_two_player_match_ready(emulator) {
                emulator.set_input(ActionButtons::default());
                for _ in 0..ROSTER_GAMEPLAY_POST_READY_SETTLE_FRAMES {
                    advance_one_script_frame(emulator)?;
                    frames = frames.saturating_add(1);
                    ensure_running(emulator)?;
                }
                emulator.capture_vblank_presented_frame();
                if native_two_player_match_ready(emulator) {
                    return Ok(NativePlayableStartup {
                        frames,
                        playable: true,
                    });
                }
            }
        }
    }

    Err(BackendError::new(format!(
        "native two-player match startup completed {frames} frames without reaching a rendered fight: readiness={} input={} sync={}",
        native_two_player_match_readiness_json(emulator),
        emulator.input_activity_json(),
        emulator.native_sync_compact_json()
    )))
}

fn native_roster_navigation_segments(slot: usize, player_two: bool) -> Vec<NativePlayableSegment> {
    let (row, column) = native_roster_slot_position(slot);
    let (initial_row, initial_column) = if player_two {
        native_roster_slot_position(ROSTER_P2_INITIAL_SLOT)
    } else {
        (0, 0)
    };
    let up = if player_two { Action::P2Up } else { Action::Up };
    let down = if player_two {
        Action::P2Down
    } else {
        Action::Down
    };
    let left = if player_two {
        Action::P2Left
    } else {
        Action::Left
    };
    let right = if player_two {
        Action::P2Right
    } else {
        Action::Right
    };
    let mut segments = Vec::new();
    if row > initial_row {
        append_navigation_pulses(&mut segments, down, row - initial_row);
    } else {
        append_navigation_pulses(&mut segments, up, initial_row - row);
    }
    if column > initial_column {
        append_navigation_pulses(&mut segments, right, column - initial_column);
    } else {
        append_navigation_pulses(&mut segments, left, initial_column - column);
    }
    segments.push(segment(Action::Noop, ROSTER_PREVIEW_SETTLE_FRAMES));
    segments
}

fn native_roster_slot_position(slot: usize) -> (usize, usize) {
    if slot < ROSTER_TOP_ROW_COLUMNS {
        (0, slot)
    } else {
        (1, slot - ROSTER_TOP_ROW_COLUMNS)
    }
}

fn append_navigation_pulses(
    segments: &mut Vec<NativePlayableSegment>,
    action: Action,
    count: usize,
) {
    for _ in 0..count {
        segments.push(segment(action, ROSTER_NAVIGATION_PULSE_FRAMES));
        segments.push(segment(Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES));
    }
}

fn native_two_player_match_ready(emulator: &NativeEmulator) -> bool {
    let activity = emulator.input_activity();
    let frame = emulator.fast_gameplay_window_frame_with_checksum().0;
    native_two_player_match_signals_ready(
        emulator.br2_native_credit_hle_game_start_accepted_seen(),
        activity.system_p2_start_active_reads > 0,
        emulator.native_3d_gameplay_signal(),
        native_two_player_match_frame_ready(&frame),
    )
}

fn native_two_player_match_signals_ready(
    game_start_accepted: bool,
    p2_start_observed: bool,
    native_3d_gameplay_signal: bool,
    frame_ready: bool,
) -> bool {
    game_start_accepted && p2_start_observed && native_3d_gameplay_signal && frame_ready
}

fn native_two_player_match_readiness_json(emulator: &NativeEmulator) -> String {
    let activity = emulator.input_activity();
    let (frame, checksum) = emulator.fast_gameplay_window_frame_with_checksum();
    let p1_health_columns = native_match_health_bar_columns(&frame, 35..230);
    let p2_health_columns = native_match_health_bar_columns(&frame, 282..477);
    format!(
        "{{\"game_start_accepted\":{},\"p2_start_observed\":{},\"native_3d_gameplay_signal\":{},\"gpu_rendered_scene_candidate\":{},\"native_playable_candidate\":{},\"frame_ready\":{},\"frame_width\":{},\"frame_height\":{},\"frame_checksum\":{},\"p1_health_columns\":{},\"p2_health_columns\":{}}}",
        emulator.br2_native_credit_hle_game_start_accepted_seen(),
        activity.system_p2_start_active_reads > 0,
        emulator.native_3d_gameplay_signal(),
        emulator.gpu_native_rendered_scene_candidate(),
        emulator.native_playable_candidate(),
        native_two_player_match_frame_ready(&frame),
        frame.width,
        frame.height,
        checksum,
        p1_health_columns,
        p2_health_columns,
    )
}

fn native_two_player_match_frame_ready(frame: &NativeDisplayFrame) -> bool {
    if frame.width < 512
        || frame.height < 480
        || frame.pixels.len() < frame.width.saturating_mul(frame.height)
    {
        return false;
    }
    native_match_health_bar_columns(frame, 35..230) * 5 >= 195 * 3
        && native_match_health_bar_columns(frame, 282..477) * 5 >= 195 * 3
}

fn native_match_health_bar_columns(
    frame: &NativeDisplayFrame,
    x_range: std::ops::Range<usize>,
) -> usize {
    x_range
        .filter(|&x| {
            (38..54)
                .filter(|&y| native_match_health_bar_pixel(frame.pixels[y * frame.width + x]))
                .count()
                >= 6
        })
        .count()
}

fn native_match_health_bar_pixel(pixel: u32) -> bool {
    let red = (pixel >> 16) & 0xff;
    let green = (pixel >> 8) & 0xff;
    let blue = pixel & 0xff;
    red >= 150 && green >= 120 && blue <= 96 && red.saturating_sub(blue) >= 80
}

fn native_player_session_ready(emulator: &NativeEmulator) -> bool {
    emulator.br2_native_credit_hle_game_start_accepted_seen()
        && emulator.native_playable_candidate()
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
        // Startup only needs architectural state and vblank progress. Avoid
        // retaining per-instruction diagnostic state in this hot path.
        let slice_executed = emulator.step_instructions_until_vblank_untracked(slice);
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
    use super::{
        ROSTER_NAVIGATION_PULSE_FRAMES, ROSTER_NAVIGATION_RELEASE_FRAMES,
        ROSTER_PREVIEW_SETTLE_FRAMES, native_playable_match_entry_script,
        native_playable_segment_is_credit_attempt, native_roster_navigation_segments,
        native_two_player_match_frame_ready, native_two_player_match_signals_ready,
    };
    use crate::{Action, native::NativeDisplayFrame};

    #[test]
    fn playable_script_contains_boot_match_and_all_controls() {
        let script = native_playable_match_entry_script();
        let total_frames = script.iter().map(|segment| segment.frames).sum::<u64>();

        assert_eq!(total_frames, 1_554);
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
        assert_eq!(tail_frames, 888);
        assert!(
            script[..first_credit]
                .iter()
                .all(|segment| !native_playable_segment_is_credit_attempt(*segment))
        );
        assert_eq!(script[first_credit].action, Action::Coin);
        assert_eq!(script[first_credit + 1].action, Action::Noop);
        assert_eq!(script[first_credit + 2].action, Action::Start);
        assert_eq!(script[first_credit + 3].action, Action::Noop);
        assert_eq!(script[first_credit + 4].action, Action::Coin);
        assert_eq!(script[first_credit + 5].action, Action::Noop);
        assert_eq!(script[first_credit + 6].action, Action::Start);
    }

    #[test]
    fn two_player_match_readiness_requires_both_health_bars() {
        let mut frame = NativeDisplayFrame {
            width: 512,
            height: 480,
            pixels: vec![0; 512 * 480],
        };
        for y in 38..54 {
            for x in 35..230 {
                frame.pixels[y * frame.width + x] = 0x00e0_c020;
            }
        }
        assert!(!native_two_player_match_frame_ready(&frame));
        for y in 38..54 {
            for x in 282..477 {
                frame.pixels[y * frame.width + x] = 0x00e0_c020;
            }
        }
        assert!(native_two_player_match_frame_ready(&frame));
    }

    #[test]
    fn two_player_match_readiness_uses_rendered_fight_proof_not_gpu_heuristics() {
        assert!(native_two_player_match_signals_ready(
            true, true, true, true
        ));
        assert!(!native_two_player_match_signals_ready(
            false, true, true, true
        ));
        assert!(!native_two_player_match_signals_ready(
            true, false, true, true
        ));
        assert!(!native_two_player_match_signals_ready(
            true, true, false, true
        ));
        assert!(!native_two_player_match_signals_ready(
            true, true, true, false
        ));
    }

    #[test]
    fn p2_roster_navigation_starts_on_long() {
        let actions = |slot| {
            native_roster_navigation_segments(slot, true)
                .into_iter()
                .map(|segment| (segment.action, segment.frames))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            actions(4),
            vec![(Action::Noop, ROSTER_PREVIEW_SETTLE_FRAMES)]
        );
        assert_eq!(
            actions(0),
            vec![
                (Action::P2Left, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::P2Left, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::P2Left, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::P2Left, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::Noop, ROSTER_PREVIEW_SETTLE_FRAMES),
            ]
        );
        assert_eq!(
            actions(10),
            vec![
                (Action::P2Down, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::P2Right, ROSTER_NAVIGATION_PULSE_FRAMES),
                (Action::Noop, ROSTER_NAVIGATION_RELEASE_FRAMES),
                (Action::Noop, ROSTER_PREVIEW_SETTLE_FRAMES),
            ]
        );
    }
}
