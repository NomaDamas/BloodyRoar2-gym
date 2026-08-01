#!/usr/bin/env python3
"""Summarize Bloody Roar 2 native macOS E2E QA artifacts.

The shell runner keeps native CLI stdout/stderr in files. This helper reads the
last JSON object from those files, picks representative PNGs, and emits compact
human/CI verdicts without copying or materializing ROM assets.
"""

from __future__ import annotations

import argparse
import json
import mmap
import shutil
import sys
from collections import Counter
from pathlib import Path
from typing import Any


PNG_OUTPUT_KEYS = (
    "window_output",
    "actual_display_output",
    "display_output",
    "observation_output",
)

BAD_RECOVERY_REASONS = {
    "preserved_severely_corrupt_recovered_chain",
    "replay_rejected_after_validation",
    "replay_no_packets",
    "stale_body_generation",
}

WARN_RECOVERY_REASONS = {
    "preserved_sparse_recovered_presentation",
    "no_model_draws",
    "no_complete_generation",
}

BLOCKING_FRAME_FLAGS = (
    "blocking_display_artifact",
    "periodic_horizontal_ghosting",
    "texture_page_fragment_blocking",
    "stage_texture_field_smear",
    "stage_texture_replay_fragment_artifact",
    "br2_character_select_texture_atlas_overlay",
    "br2_select_name_texture_atlas_overlay",
    "br2_character_select_name_overlay_artifact",
    "br2_low_color_select_texture_atlas_overlay",
    "br2_caption_stalled_select_ui_artifact",
    "br2_low_color_stage_texture_atlas_artifact",
    "br2_warm_stage_name_texture_atlas_artifact",
    "br2_high_vertical_field_pair_texture_atlas_artifact",
    "br2_dense_live_field_texture_atlas_artifact",
    "br2_sparse_high_vertical_mixed_field_artifact",
    "br2_high_palette_sparse_title_field_fragment",
    "br2_red_warning_backdrop_texture_artifact",
    "br2_red_cave_replay_artifact",
    "full_window_texture_atlas_fragment_artifact",
    "sparse_title_texture_atlas_fragment_artifact",
    "cropped_safe_field_texture_fragment",
    "sparse_texture_fragment_handoff_artifact",
    "corrupt_title_texture_fragment",
    "repeating_tile_grid_artifact",
    "saturated_transition_planes",
    "warm_dominant_fill_frame",
    "missing_texture_recovery_artifact",
    "br2_low_palette_vertical_stage_strip_artifact",
    "title_logo_overlay_blocking",
    "br2_title_caption_live_atlas_mixture",
    "br2_character_select_title_atlas_mixture",
)

LIVE_REQUIRED_GUI_INPUT_FLAGS = (
    "test_input_completed",
    "release_observed",
    "requested_guest_verified",
    "p1_direction_observed",
    "p1_play_observed",
    "p1_full_observed",
    "p2_direction_observed",
    "p2_play_observed",
    "p2_full_observed",
    "p1_play_guest_verified",
    "p1_full_guest_verified",
    "p2_play_guest_verified",
    "p2_full_guest_verified",
)

LIVE_REQUIRED_BUTTON_FIELDS = (
    "coin",
    "start",
    "up",
    "down",
    "left",
    "right",
    "punch",
    "kick",
    "beast",
    "guard",
    "p2_coin",
    "p2_start",
    "p2_up",
    "p2_down",
    "p2_left",
    "p2_right",
    "p2_punch",
    "p2_kick",
    "p2_beast",
    "p2_guard",
)

SELECT_PREVIEW_MIN_DRAWN_PIXELS = 10_000
SELECT_PREVIEW_MIN_HEIGHT = 160
SELECT_PREVIEW_MIN_WIDTH = 96
SELECT_PREVIEW_MAX_BAD_PALETTE_RATIO = 0.50
SELECT_LARGE_PORTRAIT_ROI = (24, 126, 238, 316)
SELECT_RIGHT_LARGE_PORTRAIT_ROI = (274, 126, 488, 316)
SELECT_ROSTER_ROI = (72, 315, 512, 470)
SELECT_ROSTER_MIN_EDGE_RATIO = 0.01
SELECT_ROSTER_MAX_NONBLACK_RATIO = 0.82
SELECT_ROSTER_MIN_QUANTIZED_COLORS = 48
SELECT_LARGE_PORTRAIT_MIN_GP0_ROI_OVERLAP = 0.45
SELECT_LARGE_PORTRAIT_MIN_NONBLACK_RATIO = 0.30
SELECT_LARGE_PORTRAIT_MIN_CHROMA_RATIO = 0.14
SELECT_LARGE_PORTRAIT_MIN_EDGE_RATIO = 0.06
SELECT_LARGE_PORTRAIT_MAX_EDGE_RATIO = 0.30
SELECT_LARGE_PORTRAIT_MIN_QUANTIZED_COLORS = 48
TITLE_LOGO_REUSE_COMPARE_SIZE = (64, 56)
TITLE_LOGO_REUSE_SEARCH_STEP = 4
TITLE_LOGO_REUSE_MAX_SOURCE_LEFT = 296
TITLE_LOGO_REUSE_MIN_SOURCE_TOP = 144
TITLE_LOGO_REUSE_MAX_SOURCE_TOP = 288
TITLE_LOGO_REUSE_MIN_LUMA_CORRELATION = 0.72
TITLE_LOGO_REUSE_MAX_MEAN_RGB_DELTA = 52.0
SELECT_PREVIEW_KNOWN_BAD_PALETTE_HASHES = {
    "0xe2a11fb6",
}

GENERIC_JSON_FULL_READ_LIMIT = 8 * 1024 * 1024
GENERIC_JSON_TAIL_READ_LIMIT = 2 * 1024 * 1024
FRAME_IMAGE_MIN_ACTIVE_ROW_SPAN = {
    "boot": 360,
    "select": 360,
    "combat": 390,
    "beast": 390,
    "final": 390,
}
FRAME_IMAGE_MIN_ACTIVE_ROWS = {
    "boot": 180,
    "select": 220,
    "combat": 260,
    "beast": 260,
    "final": 260,
}
COMBAT_IMAGE_ROI = (0, 72, 512, 430)
COMBAT_IMAGE_MIN_NONBLACK_RATIO = 0.28
COMBAT_IMAGE_MIN_CHROMA_RATIO = 0.08
COMBAT_IMAGE_MIN_EDGE_RATIO = 0.025
COMBAT_IMAGE_MAX_EDGE_RATIO = 0.42
COMBAT_IMAGE_MIN_QUANTIZED_COLORS = 32
COMBAT_IMAGE_MAX_DOMINANT_RATIO = 0.78
COMBAT_CHALLENGER_OVERLAY_ROI = (16, 208, 496, 326)
COMBAT_MODEL_CENTER_ROI = (96, 120, 416, 420)
COMBAT_LOWER_MODEL_ROI = (48, 176, 464, 430)
COMBAT_CHALLENGER_MIN_TEXT_EDGE_RATIO = 0.12
COMBAT_CHALLENGER_MIN_YELLOW_RATIO = 0.14
COMBAT_CHALLENGER_MIN_ORANGE_RATIO = 0.16
COMBAT_CHALLENGER_MAX_BLUE_RATIO = 0.06
COMBAT_CHALLENGER_MAX_QUANTIZED_COLORS = 512
COMBAT_BLACK_COMPONENT_LUMA_MAX = 24
COMBAT_BLACK_COMPONENT_MIN_PIXEL_RATIO = 0.12
COMBAT_BLACK_COMPONENT_MIN_LARGEST_RATIO = 0.08
COMBAT_BLACK_COMPONENT_SAMPLE_STEP = 2
COMBAT_VS_TEXTURE_MAX_PLAYFIELD_SPAN = 340
COMBAT_VS_TEXTURE_MIN_DARK_RATIO = 0.42
COMBAT_VS_TEXTURE_MAX_QUANTIZED_COLORS = 96
BEAST_STUCK_MAX_CHANGED_RATIO = 0.015
BEAST_STUCK_MAX_MEAN_DELTA = 2.5
BEAST_POST_ACTION_SEQUENCE_LIMIT = 4
REPEATING_TILE_PERIODS = (16, 32, 64)
REPEATING_TILE_SAMPLE_STEP = 4
REPEATING_TILE_MAX_RGB_DISTANCE = 18
REPEATING_TILE_MIN_SIMILARITY = 0.92
CAPTURE_SEQUENCE_MAX_EDGE_RATIO = 0.55
CAPTURE_SEQUENCE_MAX_DOMINANT_RATIO = 0.92
AUDIO_QUEUE_HARD_BLOCKING_COUNTER_FIELDS = (
    "dropped_frames",
    "producer_miss_frames",
    "producer_miss_events",
    "producer_deferred_dropped_frames",
    "coreaudio_enqueue_errors",
)
AUDIO_QUEUE_MAX_GAP_RATIO_NUMERATOR = 1
AUDIO_QUEUE_MAX_GAP_RATIO_DENOMINATOR = 100
AUDIO_QUEUE_MAX_REPEAT_RATIO_NUMERATOR = 1
AUDIO_QUEUE_MAX_REPEAT_RATIO_DENOMINATOR = 3


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarize macOS native smoke/live QA artifacts."
    )
    parser.add_argument("--mode", required=True, choices=("smoke", "live", "all"))
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--cleanup", required=True, choices=("never", "on-pass", "always"))
    parser.add_argument("--build-status", type=int, default=0)
    parser.add_argument("--smoke-status", type=int, default=-1)
    parser.add_argument("--smoke-stdout", default="")
    parser.add_argument("--smoke-stderr", default="")
    parser.add_argument("--smoke-prefix", default="")
    parser.add_argument("--live-status", type=int, default=-1)
    parser.add_argument("--live-stdout", default="")
    parser.add_argument("--live-stderr", default="")
    parser.add_argument("--live-capture-dir", default="")
    parser.add_argument("--commands-log", default="")
    return parser.parse_args()


def load_last_json(path: str) -> tuple[Any | None, str | None]:
    if not path:
        return None, "path_not_set"
    p = Path(path)
    if not p.exists():
        return None, "file_missing"
    try:
        size = p.stat().st_size
    except OSError as exc:
        return None, f"stat_failed:{exc}"
    if size > GENERIC_JSON_FULL_READ_LIMIT:
        tail_json, tail_error = load_last_json_from_tail(p)
        if tail_json is not None or tail_error != "no_json_object":
            return tail_json, tail_error
        return None, f"json_too_large_for_generic_loader:{size}"
    try:
        text = p.read_text(encoding="utf-8", errors="replace").strip()
    except OSError as exc:
        return None, f"read_failed:{exc}"
    if not text:
        return None, "file_empty"
    try:
        return json.loads(text), None
    except json.JSONDecodeError:
        pass
    for line in reversed(text.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            return json.loads(line), None
        except json.JSONDecodeError:
            continue
    return None, "no_json_object"


def load_last_json_from_tail(path: Path) -> tuple[Any | None, str | None]:
    try:
        size = path.stat().st_size
        with path.open("rb") as handle:
            handle.seek(max(0, size - GENERIC_JSON_TAIL_READ_LIMIT))
            text = handle.read().decode("utf-8", errors="replace")
    except OSError as exc:
        return None, f"read_failed:{exc}"
    for line in reversed(text.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            return json.loads(line), None
        except json.JSONDecodeError:
            continue
    return None, "no_json_object"


def _skip_json_ws(mm: mmap.mmap, index: int) -> int:
    size = len(mm)
    while index < size and mm[index] in b" \t\r\n":
        index += 1
    return index


def _find_json_string_end(mm: mmap.mmap, index: int) -> int:
    if index >= len(mm) or mm[index] != ord('"'):
        raise ValueError(f"expected JSON string at byte {index}")
    cursor = index + 1
    escaped = False
    while cursor < len(mm):
        byte = mm[cursor]
        if escaped:
            escaped = False
        elif byte == ord("\\"):
            escaped = True
        elif byte == ord('"'):
            return cursor + 1
        cursor += 1
    raise ValueError(f"unterminated JSON string at byte {index}")


def _parse_json_string(mm: mmap.mmap, index: int) -> tuple[str, int]:
    end = _find_json_string_end(mm, index)
    return json.loads(mm[index:end].decode("utf-8")), end


def _find_json_value_end(mm: mmap.mmap, index: int) -> int:
    index = _skip_json_ws(mm, index)
    if index >= len(mm):
        raise ValueError("unexpected EOF before JSON value")

    first = mm[index]
    if first == ord('"'):
        return _find_json_string_end(mm, index)
    if first not in (ord("{"), ord("[")):
        cursor = index
        while cursor < len(mm) and mm[cursor] not in b",}]":
            cursor += 1
        return cursor

    stack = [first]
    cursor = index + 1
    in_string = False
    escaped = False
    while cursor < len(mm):
        byte = mm[cursor]
        if in_string:
            if escaped:
                escaped = False
            elif byte == ord("\\"):
                escaped = True
            elif byte == ord('"'):
                in_string = False
        elif byte == ord('"'):
            in_string = True
        elif byte in (ord("{"), ord("[")):
            stack.append(byte)
        elif byte in (ord("}"), ord("]")):
            if not stack:
                raise ValueError(f"unexpected JSON close at byte {cursor}")
            opener = stack.pop()
            if (opener, byte) not in (
                (ord("{"), ord("}")),
                (ord("["), ord("]")),
            ):
                raise ValueError(f"mismatched JSON close at byte {cursor}")
            if not stack:
                return cursor + 1
        cursor += 1
    raise ValueError(f"unterminated JSON value at byte {index}")


def _decode_json_slice(mm: mmap.mmap, start: int, end: int) -> Any:
    return json.loads(mm[start:end].decode("utf-8"))


def compact_smoke_snapshot(snapshot: dict[str, Any]) -> dict[str, Any]:
    compact: dict[str, Any] = {}
    for key in (
        "tail_index",
        "action",
        "window_output",
        "actual_display_output",
        "display_output",
        "observation_output",
        "window_frame",
        "input_activity",
    ):
        if key in snapshot:
            compact[key] = snapshot[key]

    rec = get_path(snapshot, "native_sync", "native_otc_dma_recovery")
    if rec is None:
        rec = find_first_key(snapshot.get("native_sync", {}), "native_otc_dma_recovery")
    if isinstance(rec, dict):
        compact["native_sync"] = {"native_otc_dma_recovery": rec}

    gpu = snapshot.get("gpu")
    if isinstance(gpu, dict):
        compact_gpu = {}
        for key in (
            "gpu_recent_focus_draw_commands",
            "gpu_recent_overlap_draw_commands",
            "gpu_recent_draw_commands",
            "gpu_top_draw_commands",
            "gpu_largest_draw_command",
            "gpu_retained_transfer_coverage",
        ):
            if key in gpu:
                compact_gpu[key] = gpu[key]
        if compact_gpu:
            compact["gpu"] = compact_gpu
    return compact


def _load_smoke_snapshots_from_mmap(mm: mmap.mmap, index: int) -> tuple[list[dict[str, Any]], int]:
    index = _skip_json_ws(mm, index)
    if index >= len(mm) or mm[index] != ord("["):
        raise ValueError(f"expected snapshots array at byte {index}")
    index += 1
    snapshots: list[dict[str, Any]] = []
    while True:
        index = _skip_json_ws(mm, index)
        if index >= len(mm):
            raise ValueError("unterminated snapshots array")
        if mm[index] == ord("]"):
            return snapshots, index + 1
        value_start = index
        value_end = _find_json_value_end(mm, value_start)
        snapshot = _decode_json_slice(mm, value_start, value_end)
        if isinstance(snapshot, dict):
            snapshots.append(compact_smoke_snapshot(snapshot))
        index = _skip_json_ws(mm, value_end)
        if index < len(mm) and mm[index] == ord(","):
            index += 1
            continue
        if index < len(mm) and mm[index] == ord("]"):
            return snapshots, index + 1
        raise ValueError(f"expected comma or array close at byte {index}")


def load_smoke_json(path: str) -> tuple[Any | None, str | None]:
    if not path:
        return None, "path_not_set"
    p = Path(path)
    if not p.exists():
        return None, "file_missing"
    try:
        size = p.stat().st_size
    except OSError as exc:
        return None, f"stat_failed:{exc}"
    if size <= GENERIC_JSON_FULL_READ_LIMIT:
        return load_last_json(path)

    top_level_keys = {
        "instructions_per_frame",
        "fast_forward_instructions_per_frame",
        "auto_mapping",
        "total_frames",
        "missed_vblank_frames",
        "executed_steps",
        "boot",
        "state",
        "input_activity",
        "native_sync",
        "playability",
        "rom_compatibility",
        "last_outcome",
    }
    try:
        with p.open("rb") as handle:
            with mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_READ) as mm:
                index = _skip_json_ws(mm, 0)
                if index >= len(mm) or mm[index] != ord("{"):
                    return None, "top_level_json_object_missing"
                index += 1
                data: dict[str, Any] = {"snapshots": []}
                while True:
                    index = _skip_json_ws(mm, index)
                    if index >= len(mm):
                        return None, "unterminated_top_level_json_object"
                    if mm[index] == ord("}"):
                        return data, None
                    key, index = _parse_json_string(mm, index)
                    index = _skip_json_ws(mm, index)
                    if index >= len(mm) or mm[index] != ord(":"):
                        return None, f"top_level_colon_missing:{key}"
                    index = _skip_json_ws(mm, index + 1)
                    if key == "snapshots":
                        snapshots, index = _load_smoke_snapshots_from_mmap(mm, index)
                        data["snapshots"] = snapshots
                    else:
                        value_end = _find_json_value_end(mm, index)
                        if key in top_level_keys:
                            data[key] = _decode_json_slice(mm, index, value_end)
                        index = value_end
                    index = _skip_json_ws(mm, index)
                    if index < len(mm) and mm[index] == ord(","):
                        index += 1
                        continue
                    if index < len(mm) and mm[index] == ord("}"):
                        return data, None
                    return None, f"top_level_separator_missing:{index}"
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        return None, f"stream_parse_failed:{exc}"


def tail_text(path: str, lines: int = 20) -> list[str]:
    if not path:
        return []
    p = Path(path)
    if not p.exists():
        return []
    try:
        return p.read_text(encoding="utf-8", errors="replace").splitlines()[-lines:]
    except OSError:
        return []


def get_path(obj: Any, *parts: str, default: Any = None) -> Any:
    cur = obj
    for part in parts:
        if not isinstance(cur, dict) or part not in cur:
            return default
        cur = cur[part]
    return cur


def find_first_key(obj: Any, key: str) -> Any | None:
    if isinstance(obj, dict):
        if key in obj:
            return obj[key]
        for value in obj.values():
            found = find_first_key(value, key)
            if found is not None:
                return found
    elif isinstance(obj, list):
        for value in obj:
            found = find_first_key(value, key)
            if found is not None:
                return found
    return None


def existing_png(path: str | None) -> str | None:
    if not path:
        return None
    return path if Path(path).exists() else None


def snapshot_png(snapshot: dict[str, Any] | None) -> str | None:
    pngs = snapshot_pngs(snapshot)
    return pngs[0] if pngs else None


def snapshot_pngs(snapshot: dict[str, Any] | None) -> list[str]:
    if not isinstance(snapshot, dict):
        return []
    pngs: list[str] = []
    for key in PNG_OUTPUT_KEYS:
        png = existing_png(snapshot.get(key))
        if png and png not in pngs:
            pngs.append(png)
    return pngs


def boolish(value: Any) -> bool:
    return bool(value)


def intish(value: Any) -> int:
    try:
        return int(value or 0)
    except (TypeError, ValueError):
        return 0


def numeric_path(obj: Any, *parts: str) -> int:
    return intish(get_path(obj, *parts))


def capture_label(capture: dict[str, Any] | None) -> str:
    return str(capture.get("label") or "").lower() if isinstance(capture, dict) else ""


def capture_buttons(capture: dict[str, Any] | None) -> dict[str, Any]:
    buttons = capture.get("buttons") if isinstance(capture, dict) else None
    return buttons if isinstance(buttons, dict) else {}


def capture_has_any_button(capture: dict[str, Any] | None, *button_names: str) -> bool:
    buttons = capture_buttons(capture)
    return any(boolish(buttons.get(name)) for name in button_names)


def capture_is_title_like(capture: dict[str, Any] | None) -> bool:
    if not isinstance(capture, dict):
        return False
    frame = capture.get("frame")
    return capture_label(capture) in {"initial", "boot", "title"} or boolish(
        get_path(frame, "title_screen_frame")
    )


def capture_scene_image_diagnostics(
    capture: dict[str, Any] | None,
    kind: str,
) -> dict[str, Any]:
    if not isinstance(capture, dict):
        return {"status": "fail", "reasons": ["capture_missing"]}
    cache_key = f"_qa_{kind}_image_diagnostics"
    cached = capture.get(cache_key)
    if isinstance(cached, dict):
        return cached
    diagnostics = frame_image_diagnostics(
        existing_png(capture.get("_png_path")),
        kind,
        capture.get("frame") if isinstance(capture.get("frame"), dict) else None,
    )
    capture[cache_key] = diagnostics
    return diagnostics


def capture_select_scene_diagnostics(
    capture: dict[str, Any] | None,
) -> dict[str, Any]:
    if not isinstance(capture, dict):
        return {"status": "fail", "reasons": ["capture_missing"]}
    cache_key = "_qa_select_scene_diagnostics"
    cached = capture.get(cache_key)
    if isinstance(cached, dict):
        return cached
    layout = capture_select_layout_diagnostics(capture)
    image = capture_scene_image_diagnostics(capture, "select")
    portrait = select_large_portrait_image_diagnostics(
        existing_png(capture.get("_png_path"))
    )
    reasons = unique_reasons(
        [
            reason
            for diagnostics in (layout, image, portrait)
            if diagnostics.get("status") == "fail"
            for reason in diagnostics.get("reasons", [])
            if isinstance(reason, str)
        ]
    )
    result = {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "layout": layout,
        "image": image,
        "large_portrait_image": portrait,
    }
    capture[cache_key] = result
    return result


def capture_select_layout_diagnostics(
    capture: dict[str, Any] | None,
) -> dict[str, Any]:
    if not isinstance(capture, dict):
        return {"status": "fail", "reasons": ["capture_missing"]}
    cache_key = "_qa_select_layout_diagnostics"
    cached = capture.get(cache_key)
    if isinstance(cached, dict):
        return cached
    png = existing_png(capture.get("_png_path"))
    if not png:
        result = {"status": "fail", "reasons": ["select_layout_png_missing"]}
        capture[cache_key] = result
        return result
    try:
        from PIL import Image
    except ImportError:
        result = {"status": "fail", "reasons": ["select_layout_pillow_missing"]}
        capture[cache_key] = result
        return result
    try:
        image = Image.open(png).convert("RGB")
    except (OSError, ValueError) as exc:
        result = {
            "status": "fail",
            "reasons": [f"select_layout_png_unreadable:{exc}"],
            "png": png,
        }
        capture[cache_key] = result
        return result
    if image.width < SELECT_ROSTER_ROI[2] or image.height < SELECT_ROSTER_ROI[3]:
        result = {
            "status": "fail",
            "reasons": [f"select_layout_png_too_small:{image.width}x{image.height}"],
            "png": png,
        }
        capture[cache_key] = result
        return result
    roster = image_region_metrics(image, SELECT_ROSTER_ROI)
    reasons: list[str] = []
    if float(roster.get("edge_ratio") or 0.0) < SELECT_ROSTER_MIN_EDGE_RATIO:
        reasons.append("select_layout_roster_low_edge")
    if float(roster.get("nonblack_ratio") or 0.0) > SELECT_ROSTER_MAX_NONBLACK_RATIO:
        reasons.append("select_layout_roster_overfilled")
    if intish(roster.get("quantized_colors")) < SELECT_ROSTER_MIN_QUANTIZED_COLORS:
        reasons.append("select_layout_roster_low_color_diversity")
    result = {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "png": png,
        "roster": roster,
    }
    capture[cache_key] = result
    return result


def capture_is_select_candidate(capture: dict[str, Any] | None) -> bool:
    if not isinstance(capture, dict):
        return False
    frame = capture.get("frame")
    if not isinstance(frame, dict):
        return False
    if boolish(frame.get("gameplay_scene")) or boolish(frame.get("title_screen_frame")):
        return False
    if not boolish(frame.get("render_ready_scene")):
        return False
    return capture_select_layout_diagnostics(capture).get("status") == "pass"


def capture_is_select_like(capture: dict[str, Any] | None) -> bool:
    if not capture_is_select_candidate(capture):
        return False
    return capture_select_scene_diagnostics(capture).get("status") == "pass"


def capture_is_combat_like(capture: dict[str, Any] | None) -> bool:
    if not isinstance(capture, dict):
        return False
    frame = capture.get("frame")
    if not isinstance(frame, dict):
        return False
    if boolish(frame.get("title_screen_frame")):
        return False
    if not (
        boolish(frame.get("gameplay_scene"))
        or boolish(frame.get("render_ready_scene"))
    ):
        return False
    if capture_is_select_candidate(capture):
        return False
    return capture_scene_image_diagnostics(capture, "combat").get("status") == "pass"


def capture_is_non_render_transition(capture: dict[str, Any] | None) -> bool:
    if not isinstance(capture, dict):
        return False
    frame = capture.get("frame")
    if not isinstance(frame, dict):
        return False
    return not boolish(frame.get("render_ready_scene")) and not boolish(
        frame.get("gameplay_scene")
    )


def first_capture_index(
    captures: list[dict[str, Any]],
    predicate,
    start: int = 0,
    end: int | None = None,
) -> int | None:
    stop = len(captures) if end is None else min(max(end, 0), len(captures))
    for index in range(max(start, 0), stop):
        if predicate(captures[index]):
            return index
    return None


def first_capture_after(
    captures: list[dict[str, Any]], predicate, start: int = 0
) -> dict[str, Any] | None:
    index = first_capture_index(captures, predicate, start)
    return captures[index] if index is not None else None


def frame_reasons(frame: dict[str, Any] | None, kind: str) -> list[str]:
    if not isinstance(frame, dict):
        return ["frame_stats_missing"]

    reasons: list[str] = []
    width = int(frame.get("width") or 0)
    height = int(frame.get("height") or 0)
    if width < 512:
        reasons.append(f"width_lt_512:{width}")
    if height < 480:
        reasons.append(f"height_lt_480:{height}")
    if not boolish(frame.get("visible_content")):
        reasons.append("no_visible_content")
    if int(frame.get("unique_colors") or 0) < 32:
        reasons.append(f"low_unique_colors:{frame.get('unique_colors')}")
    if kind in {"combat", "beast", "final"} and not boolish(frame.get("scene_detail")):
        reasons.append("missing_scene_detail")
    if kind in {"combat", "final"} and not (
        boolish(frame.get("gameplay_scene"))
        or boolish(frame.get("render_ready_scene"))
    ):
        reasons.append("not_gameplay_scene")
    if kind == "select" and int(frame.get("occupied_row_span") or 0) < 300:
        reasons.append(f"short_occupied_row_span:{frame.get('occupied_row_span')}")
    for flag in BLOCKING_FRAME_FLAGS:
        if boolish(frame.get(flag)):
            reasons.append(flag)
    return reasons


def frame_status(frame: dict[str, Any] | None, kind: str) -> str:
    reasons = frame_reasons(frame, kind)
    return "pass" if not reasons else "fail"


def summarize_frame(frame: dict[str, Any] | None, kind: str) -> dict[str, Any]:
    reasons = frame_reasons(frame, kind)
    compact = {
        "width": get_path(frame, "width"),
        "height": get_path(frame, "height"),
        "unique_colors": get_path(frame, "unique_colors"),
        "occupied_rows": get_path(frame, "occupied_rows"),
        "occupied_row_span": get_path(frame, "occupied_row_span"),
        "scene_detail": get_path(frame, "scene_detail"),
        "gameplay_scene": get_path(frame, "gameplay_scene"),
        "render_ready_scene": get_path(frame, "render_ready_scene"),
        "title_screen_frame": get_path(frame, "title_screen_frame"),
        "bottom_caption_band": get_path(frame, "bottom_caption_band"),
        "intro_caption_band": get_path(frame, "intro_caption_band"),
        "blocking_display_artifact": get_path(frame, "blocking_display_artifact"),
        "missing_texture_recovery_artifact": get_path(frame, "missing_texture_recovery_artifact"),
    }
    return {"status": "pass" if not reasons else "fail", "reasons": reasons, "frame": compact}


def image_region_metrics(image: Any, roi: tuple[int, int, int, int]) -> dict[str, Any]:
    left, top, right, bottom = roi
    crop = image.crop((left, top, right, bottom))
    width, height = crop.size
    pixel_reader = getattr(crop, "get_flattened_data", crop.getdata)
    pixels = list(pixel_reader())
    total = max(len(pixels), 1)
    nonblack = 0
    chroma = 0
    quantized_counter: Counter[tuple[int, int, int]] = Counter()
    row_nonblack = [0 for _ in range(height)]
    luminance: list[float] = []
    for index, (red, green, blue) in enumerate(pixels):
        row = index // width
        lum = 0.2126 * red + 0.7152 * green + 0.0722 * blue
        luminance.append(lum)
        if max(red, green, blue) > 24:
            nonblack += 1
            row_nonblack[row] += 1
            quantized_counter[(red // 16, green // 16, blue // 16)] += 1
        if max(red, green, blue) > 50 and max(red, green, blue) - min(red, green, blue) > 32:
            chroma += 1

    active_rows = [row for row, count in enumerate(row_nonblack) if count >= max(width // 64, 2)]
    active_row_span = 0
    if active_rows:
        active_row_span = active_rows[-1] - active_rows[0] + 1

    edge = 0
    for y in range(height):
        row = y * width
        for x in range(width - 1):
            if abs(luminance[row + x] - luminance[row + x + 1]) > 28:
                edge += 1
    for y in range(height - 1):
        row = y * width
        next_row = (y + 1) * width
        for x in range(width):
            if abs(luminance[row + x] - luminance[next_row + x]) > 28:
                edge += 1

    edge_basis = max((width - 1) * height + width * (height - 1), 1)
    dominant = quantized_counter.most_common(1)[0][1] if quantized_counter else 0
    return {
        "roi": {"left": left, "top": top, "right": right, "bottom": bottom},
        "nonblack_pixels": nonblack,
        "nonblack_ratio": round(nonblack / total, 4),
        "chroma_pixels": chroma,
        "chroma_ratio": round(chroma / total, 4),
        "edge_pixels": edge,
        "edge_ratio": round(edge / edge_basis, 4),
        "quantized_colors": len(quantized_counter),
        "dominant_quantized_ratio": round(dominant / max(nonblack, 1), 4),
        "active_rows": len(active_rows),
        "active_row_span": active_row_span,
    }


def image_color_class_metrics(image: Any, roi: tuple[int, int, int, int]) -> dict[str, Any]:
    left, top, right, bottom = roi
    left = max(0, min(left, image.width))
    top = max(0, min(top, image.height))
    right = max(left, min(right, image.width))
    bottom = max(top, min(bottom, image.height))
    crop = image.crop((left, top, right, bottom))
    width, height = crop.size
    pixel_reader = getattr(crop, "get_flattened_data", crop.getdata)
    pixels = list(pixel_reader())
    total = max(len(pixels), 1)
    quantized_counter: Counter[tuple[int, int, int]] = Counter()
    counts = {
        "black": 0,
        "dark": 0,
        "white": 0,
        "yellow": 0,
        "orange": 0,
        "blue": 0,
        "warm": 0,
    }
    for red, green, blue in pixels:
        high = max(red, green, blue)
        low = min(red, green, blue)
        if high > 24:
            quantized_counter[(red // 16, green // 16, blue // 16)] += 1
        if high <= 24:
            counts["black"] += 1
        if high <= 42:
            counts["dark"] += 1
        if low >= 168:
            counts["white"] += 1
        if red >= 170 and green >= 95 and blue <= 100 and red >= green:
            counts["yellow"] += 1
        if red >= 170 and 65 <= green <= 170 and blue <= 90 and red > green:
            counts["orange"] += 1
        if blue >= 120 and red <= 120 and green <= 180:
            counts["blue"] += 1
        if red >= 140 and green >= 70 and blue <= 100 and red - green < 110:
            counts["warm"] += 1
    dominant = quantized_counter.most_common(1)[0][1] if quantized_counter else 0
    return {
        "roi": {"left": left, "top": top, "right": right, "bottom": bottom},
        "width": width,
        "height": height,
        "quantized_colors": len(quantized_counter),
        "dominant_quantized_ratio": round(dominant / max(sum(quantized_counter.values()), 1), 4),
        **{f"{name}_ratio": round(count / total, 4) for name, count in counts.items()},
    }


def low_luma_component_metrics(
    image: Any,
    roi: tuple[int, int, int, int],
    *,
    threshold: int,
    sample_step: int = COMBAT_BLACK_COMPONENT_SAMPLE_STEP,
) -> dict[str, Any]:
    left, top, right, bottom = roi
    left = max(0, min(left, image.width))
    top = max(0, min(top, image.height))
    right = max(left, min(right, image.width))
    bottom = max(top, min(bottom, image.height))
    crop = image.crop((left, top, right, bottom))
    width, height = crop.size
    step = max(1, intish(sample_step))
    if step > 1:
        source_pixels = crop.load()
        pixels = [
            source_pixels[x, y]
            for y in range(0, height, step)
            for x in range(0, width, step)
        ]
        width = (width + step - 1) // step
        height = (height + step - 1) // step
    else:
        pixel_reader = getattr(crop, "get_flattened_data", crop.getdata)
        pixels = list(pixel_reader())
    total = len(pixels)
    if width <= 0 or height <= 0 or total <= 0:
        return {
            "roi": {"left": left, "top": top, "right": right, "bottom": bottom},
            "threshold": threshold,
            "sample_step": step,
            "pixel_ratio": 0.0,
            "largest_component_pixels": 0,
            "largest_component_ratio": 0.0,
            "component_count": 0,
            "largest_component_bbox": None,
        }

    low_mask = bytearray(total)
    low_pixels = 0
    for index, (red, green, blue) in enumerate(pixels):
        if max(red, green, blue) <= threshold:
            low_mask[index] = 1
            low_pixels += 1

    seen = bytearray(total)
    largest_pixels = 0
    largest_bbox: dict[str, int] | None = None
    component_count = 0
    for start in range(total):
        if not low_mask[start] or seen[start]:
            continue
        component_count += 1
        stack = [start]
        seen[start] = 1
        count = 0
        min_x = max_x = start % width
        min_y = max_y = start // width
        while stack:
            current = stack.pop()
            count += 1
            x = current % width
            y = current // width
            if x < min_x:
                min_x = x
            if x > max_x:
                max_x = x
            if y < min_y:
                min_y = y
            if y > max_y:
                max_y = y
            for neighbor in (
                current - 1 if x > 0 else None,
                current + 1 if x + 1 < width else None,
                current - width if y > 0 else None,
                current + width if y + 1 < height else None,
            ):
                if neighbor is None or seen[neighbor] or not low_mask[neighbor]:
                    continue
                seen[neighbor] = 1
                stack.append(neighbor)
        if count > largest_pixels:
            largest_pixels = count
            largest_bbox = {
                "left": left + min_x * step,
                "top": top + min_y * step,
                "right": min(right, left + (max_x + 1) * step),
                "bottom": min(bottom, top + (max_y + 1) * step),
            }

    return {
        "roi": {"left": left, "top": top, "right": right, "bottom": bottom},
        "threshold": threshold,
        "sample_step": step,
        "pixel_ratio": round(low_pixels / total, 4),
        "largest_component_pixels": largest_pixels,
        "largest_component_ratio": round(largest_pixels / total, 4),
        "component_count": component_count,
        "largest_component_bbox": largest_bbox,
    }


def rgb_distance(left: tuple[int, int, int], right: tuple[int, int, int]) -> int:
    return abs(left[0] - right[0]) + abs(left[1] - right[1]) + abs(left[2] - right[2])


def flattened_pixels(image: Any) -> list[Any]:
    reader = getattr(image, "get_flattened_data", image.getdata)
    return list(reader())


def grayscale_correlation(left_image: Any, right_image: Any) -> float:
    left_pixels = [float(value) for value in flattened_pixels(left_image)]
    right_pixels = [float(value) for value in flattened_pixels(right_image)]
    if len(left_pixels) != len(right_pixels) or not left_pixels:
        return 0.0

    left_mean = sum(left_pixels) / len(left_pixels)
    right_mean = sum(right_pixels) / len(right_pixels)
    left_variance = sum((value - left_mean) ** 2 for value in left_pixels)
    right_variance = sum((value - right_mean) ** 2 for value in right_pixels)
    if left_variance <= 0.0 or right_variance <= 0.0:
        return 0.0

    covariance = sum(
        (left - left_mean) * (right - right_mean)
        for left, right in zip(left_pixels, right_pixels)
    )
    return covariance / ((left_variance * right_variance) ** 0.5)


def title_logo_reference_is_credible(
    title_png: str | None,
    title_frame: dict[str, Any] | None,
) -> bool:
    if not title_png:
        return False
    return isinstance(title_frame, dict) and boolish(title_frame.get("title_screen_frame"))


def select_title_logo_reuse_diagnostics(
    select_png: str | None,
    title_png: str | None = None,
    title_frame: dict[str, Any] | None = None,
    portrait_roi: tuple[int, int, int, int] = SELECT_LARGE_PORTRAIT_ROI,
) -> dict[str, Any]:
    if not title_logo_reference_is_credible(title_png, title_frame):
        return {"status": "not_checked", "reason": "title_reference_not_confirmed"}
    if not select_png or not title_png:
        return {"status": "not_checked", "reason": "title_logo_reuse_png_missing"}
    if Path(select_png) == Path(title_png):
        return {"status": "not_checked", "reason": "title_logo_reuse_same_png"}

    try:
        from PIL import Image, ImageChops, ImageOps, ImageStat
    except ImportError:
        return {"status": "not_checked", "reason": "pillow_missing"}

    try:
        select_image = Image.open(select_png).convert("RGB")
        title_image = Image.open(title_png).convert("RGB")
    except (OSError, ValueError) as exc:
        return {"status": "not_checked", "reason": f"title_logo_reuse_png_unreadable:{exc}"}

    left, top, right, bottom = portrait_roi
    width = right - left
    height = bottom - top
    if select_image.width < right or select_image.height < bottom:
        return {"status": "not_checked", "reason": "select_large_portrait_png_too_small"}
    if title_image.width < width or title_image.height < height:
        return {"status": "not_checked", "reason": "title_reference_png_too_small"}

    select_crop = select_image.crop(portrait_roi)
    select_small = select_crop.resize(TITLE_LOGO_REUSE_COMPARE_SIZE, Image.Resampling.BILINEAR)
    select_gray = ImageOps.grayscale(select_small)

    max_left = min(TITLE_LOGO_REUSE_MAX_SOURCE_LEFT, title_image.width - width)
    max_top = min(TITLE_LOGO_REUSE_MAX_SOURCE_TOP, title_image.height - height)
    if max_top < TITLE_LOGO_REUSE_MIN_SOURCE_TOP:
        return {"status": "not_checked", "reason": "title_logo_reuse_no_search_window"}
    best: dict[str, Any] | None = None
    for source_top in range(
        TITLE_LOGO_REUSE_MIN_SOURCE_TOP,
        max_top + 1,
        TITLE_LOGO_REUSE_SEARCH_STEP,
    ):
        for source_left in range(0, max_left + 1, TITLE_LOGO_REUSE_SEARCH_STEP):
            source_roi = (
                source_left,
                source_top,
                source_left + width,
                source_top + height,
            )
            source_small = title_image.crop(source_roi).resize(
                TITLE_LOGO_REUSE_COMPARE_SIZE,
                Image.Resampling.BILINEAR,
            )
            source_gray = ImageOps.grayscale(source_small)
            luma_correlation = grayscale_correlation(select_gray, source_gray)
            mean_rgb_delta = sum(
                ImageStat.Stat(ImageChops.difference(select_small, source_small)).mean
            ) / 3.0
            candidate = {
                "source_roi": {
                    "left": source_left,
                    "top": source_top,
                    "right": source_left + width,
                    "bottom": source_top + height,
                },
                "luma_correlation": round(luma_correlation, 4),
                "mean_rgb_delta": round(mean_rgb_delta, 4),
            }
            if best is None:
                best = candidate
                continue
            if (
                luma_correlation > float(best["luma_correlation"])
                or (
                    round(luma_correlation, 4) == best["luma_correlation"]
                    and mean_rgb_delta < float(best["mean_rgb_delta"])
                )
            ):
                best = candidate

    if best is None:
        return {"status": "not_checked", "reason": "title_logo_reuse_no_search_window"}

    reused = (
        float(best["luma_correlation"]) >= TITLE_LOGO_REUSE_MIN_LUMA_CORRELATION
        and float(best["mean_rgb_delta"]) <= TITLE_LOGO_REUSE_MAX_MEAN_RGB_DELTA
    )
    return {
        "status": "fail" if reused else "pass",
        "reason": "select_large_portrait_title_logo_reuse"
        if reused
        else "title_logo_reuse_not_detected",
        "title_png": title_png,
        "select_png": select_png,
        "best_match": best,
        "thresholds": {
            "min_luma_correlation": TITLE_LOGO_REUSE_MIN_LUMA_CORRELATION,
            "max_mean_rgb_delta": TITLE_LOGO_REUSE_MAX_MEAN_RGB_DELTA,
        },
    }


def repeating_tile_grid_metrics(
    image: Any,
    roi: tuple[int, int, int, int],
) -> dict[str, Any]:
    """Detect exact/near-exact texture-atlas tiles repeated over a live frame."""
    left, top, right, bottom = roi
    width = max(0, right - left)
    height = max(0, bottom - top)
    if width <= 0 or height <= 0:
        return {"status": "not_checked", "reason": "empty_roi"}

    pixels = image.load()
    best: dict[str, Any] = {
        "period": None,
        "horizontal_similarity": 0.0,
        "vertical_similarity": 0.0,
        "combined_similarity": 0.0,
        "comparisons": 0,
    }
    for period in REPEATING_TILE_PERIODS:
        if width <= period * 2 or height <= period * 2:
            continue

        horizontal_matches = 0
        horizontal_total = 0
        for y in range(top, bottom, REPEATING_TILE_SAMPLE_STEP):
            for x in range(left, right - period, REPEATING_TILE_SAMPLE_STEP):
                horizontal_total += 1
                if rgb_distance(pixels[x, y], pixels[x + period, y]) <= REPEATING_TILE_MAX_RGB_DISTANCE:
                    horizontal_matches += 1

        vertical_matches = 0
        vertical_total = 0
        for y in range(top, bottom - period, REPEATING_TILE_SAMPLE_STEP):
            for x in range(left, right, REPEATING_TILE_SAMPLE_STEP):
                vertical_total += 1
                if rgb_distance(pixels[x, y], pixels[x, y + period]) <= REPEATING_TILE_MAX_RGB_DISTANCE:
                    vertical_matches += 1

        horizontal_similarity = horizontal_matches / max(horizontal_total, 1)
        vertical_similarity = vertical_matches / max(vertical_total, 1)
        combined_similarity = min(horizontal_similarity, vertical_similarity)
        if combined_similarity > float(best["combined_similarity"]):
            best = {
                "period": period,
                "horizontal_similarity": round(horizontal_similarity, 4),
                "vertical_similarity": round(vertical_similarity, 4),
                "combined_similarity": round(combined_similarity, 4),
                "comparisons": horizontal_total + vertical_total,
            }
    return best


def frame_image_diagnostics(
    png: str | None,
    kind: str,
    frame: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if not png:
        return {"status": "fail", "reasons": [f"{kind}_image_png_missing"]}
    try:
        from PIL import Image
    except ImportError:
        return {"status": "fail", "reasons": [f"{kind}_image_pillow_missing"]}

    try:
        image = Image.open(png).convert("RGB")
    except (OSError, ValueError) as exc:
        return {"status": "fail", "reasons": [f"{kind}_image_unreadable:{exc}"]}

    reasons: list[str] = []
    if image.width < 512 or image.height < 480:
        reasons.append(f"{kind}_image_size_lt_512x480:{image.width}x{image.height}")

    full_roi = (0, 0, min(image.width, 512), min(image.height, 480))
    full = image_region_metrics(image, full_roi)
    min_span = FRAME_IMAGE_MIN_ACTIVE_ROW_SPAN.get(kind, 360)
    min_rows = FRAME_IMAGE_MIN_ACTIVE_ROWS.get(kind, 220)
    if intish(full.get("active_row_span")) < min_span:
        reasons.append(f"{kind}_image_short_active_row_span:{full.get('active_row_span')}")
    if intish(full.get("active_rows")) < min_rows:
        reasons.append(f"{kind}_image_low_active_rows:{full.get('active_rows')}")

    top = image_region_metrics(image, (0, 0, min(image.width, 512), min(image.height, 240)))
    bottom = image_region_metrics(
        image, (0, min(image.height, 240), min(image.width, 512), min(image.height, 480))
    )
    recognized_boot_title = (
        kind == "boot"
        and boolish(get_path(frame, "title_screen_frame"))
        and boolish(get_path(frame, "bottom_caption_band"))
        and boolish(get_path(frame, "intro_caption_band"))
        and intish(full.get("active_row_span")) >= min_span
        and intish(full.get("active_rows")) >= min_rows
    )
    if (
        float(top.get("nonblack_ratio") or 0.0) > 0.10
        and float(bottom.get("nonblack_ratio") or 0.0) < 0.06
        and not recognized_boot_title
    ):
        reasons.append(f"{kind}_image_bottom_half_sparse")
    if (
        float(bottom.get("nonblack_ratio") or 0.0) > 0.10
        and float(top.get("nonblack_ratio") or 0.0) < 0.06
        and not recognized_boot_title
    ):
        reasons.append(f"{kind}_image_top_half_sparse")

    playfield = None
    playfield_tile_grid = None
    challenger_overlay = None
    model_center = None
    lower_model = None
    low_luma_component = None
    if kind in {"combat", "beast", "final"} and image.width >= 512 and image.height >= 430:
        playfield = image_region_metrics(image, COMBAT_IMAGE_ROI)
        if float(playfield.get("nonblack_ratio") or 0.0) < COMBAT_IMAGE_MIN_NONBLACK_RATIO:
            reasons.append(f"{kind}_image_low_playfield_nonblack")
        if float(playfield.get("chroma_ratio") or 0.0) < COMBAT_IMAGE_MIN_CHROMA_RATIO:
            reasons.append(f"{kind}_image_low_playfield_chroma")
        if float(playfield.get("edge_ratio") or 0.0) < COMBAT_IMAGE_MIN_EDGE_RATIO:
            reasons.append(f"{kind}_image_low_playfield_edge")
        if float(playfield.get("edge_ratio") or 0.0) > COMBAT_IMAGE_MAX_EDGE_RATIO:
            reasons.append(f"{kind}_image_noisy_texture_artifact")
        if intish(playfield.get("quantized_colors")) < COMBAT_IMAGE_MIN_QUANTIZED_COLORS:
            reasons.append(f"{kind}_image_low_playfield_color_diversity")
        if float(playfield.get("dominant_quantized_ratio") or 0.0) > COMBAT_IMAGE_MAX_DOMINANT_RATIO:
            reasons.append(f"{kind}_image_dominant_color_fill")
        playfield_tile_grid = repeating_tile_grid_metrics(image, COMBAT_IMAGE_ROI)
        if (
            float(playfield_tile_grid.get("combined_similarity") or 0.0)
            >= REPEATING_TILE_MIN_SIMILARITY
        ):
            reasons.append(f"{kind}_image_repeating_tile_grid")
        challenger_overlay = {
            "region": image_region_metrics(image, COMBAT_CHALLENGER_OVERLAY_ROI),
            "colors": image_color_class_metrics(image, COMBAT_CHALLENGER_OVERLAY_ROI),
        }
        challenger_colors = challenger_overlay["colors"]
        challenger_region = challenger_overlay["region"]
        if (
            float(challenger_region.get("edge_ratio") or 0.0)
            >= COMBAT_CHALLENGER_MIN_TEXT_EDGE_RATIO
            and float(challenger_colors.get("yellow_ratio") or 0.0)
            >= COMBAT_CHALLENGER_MIN_YELLOW_RATIO
            and float(challenger_colors.get("orange_ratio") or 0.0)
            >= COMBAT_CHALLENGER_MIN_ORANGE_RATIO
            and float(challenger_colors.get("blue_ratio") or 0.0)
            <= COMBAT_CHALLENGER_MAX_BLUE_RATIO
            and intish(challenger_colors.get("quantized_colors"))
            <= COMBAT_CHALLENGER_MAX_QUANTIZED_COLORS
        ):
            reasons.append(f"{kind}_image_challenger_overlay_stale")

        model_center = image_color_class_metrics(image, COMBAT_MODEL_CENTER_ROI)
        lower_model = image_color_class_metrics(image, COMBAT_LOWER_MODEL_ROI)
        if (
            float(lower_model.get("black_ratio") or 0.0)
            >= COMBAT_BLACK_COMPONENT_MIN_PIXEL_RATIO
        ):
            low_luma_component = low_luma_component_metrics(
                image,
                COMBAT_LOWER_MODEL_ROI,
                threshold=COMBAT_BLACK_COMPONENT_LUMA_MAX,
            )
        else:
            low_luma_component = {
                "roi": lower_model.get("roi"),
                "threshold": COMBAT_BLACK_COMPONENT_LUMA_MAX,
                "sample_step": COMBAT_BLACK_COMPONENT_SAMPLE_STEP,
                "pixel_ratio": lower_model.get("black_ratio"),
                "largest_component_pixels": 0,
                "largest_component_ratio": 0.0,
                "component_count": 0,
                "largest_component_bbox": None,
                "reason": "skipped_low_black_ratio",
            }
        if (
            float(low_luma_component.get("pixel_ratio") or 0.0)
            >= COMBAT_BLACK_COMPONENT_MIN_PIXEL_RATIO
            and float(low_luma_component.get("largest_component_ratio") or 0.0)
            >= COMBAT_BLACK_COMPONENT_MIN_LARGEST_RATIO
        ):
            reasons.append(f"{kind}_image_black_character_silhouette")
        if (
            intish(playfield.get("active_row_span")) < COMBAT_VS_TEXTURE_MAX_PLAYFIELD_SPAN
            and float(model_center.get("dark_ratio") or 0.0)
            >= COMBAT_VS_TEXTURE_MIN_DARK_RATIO
            and intish(model_center.get("quantized_colors"))
            <= COMBAT_VS_TEXTURE_MAX_QUANTIZED_COLORS
        ):
            reasons.append(f"{kind}_image_vs_texture_corruption")

    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "png": png,
        "full": full,
        "top_half": top,
        "bottom_half": bottom,
        "playfield": playfield,
        "playfield_repeating_tile_grid": playfield_tile_grid,
        "challenger_overlay": challenger_overlay,
        "model_center": model_center,
        "lower_model": lower_model,
        "low_luma_component": low_luma_component,
    }


def attach_stage_image_diagnostics(stage: dict[str, Any], kind: str) -> None:
    pngs: list[str] = []
    primary_png = stage.get("png")
    if isinstance(primary_png, str) and primary_png:
        pngs.append(primary_png)
    alternate_pngs = stage.get("alternate_pngs")
    if isinstance(alternate_pngs, list):
        for png in alternate_pngs:
            if isinstance(png, str) and png and png not in pngs:
                pngs.append(png)

    if not pngs:
        pngs = [None]  # type: ignore[list-item]

    image_checks = [
        frame_image_diagnostics(png, kind, stage.get("frame"))
        for png in pngs
    ]
    image = image_checks[0]
    stage["image"] = image
    if len(image_checks) > 1:
        stage["image_checks"] = image_checks

    failing_reasons = unique_reasons(
        [
            reason
            for check in image_checks
            if check["status"] == "fail"
            for reason in check["reasons"]
            if isinstance(reason, str)
        ]
    )
    if not failing_reasons:
        return
    stage["status"] = "fail"
    reasons = stage.setdefault("reasons", [])
    for reason in failing_reasons:
        if reason not in reasons:
            reasons.append(reason)


def attach_select_portrait_image_diagnostics(
    stage: dict[str, Any],
    title_stage: dict[str, Any] | None = None,
    portrait_roi: tuple[int, int, int, int] = SELECT_LARGE_PORTRAIT_ROI,
) -> None:
    title_png = title_stage.get("png") if isinstance(title_stage, dict) else None
    title_frame = title_stage.get("frame") if isinstance(title_stage, dict) else None
    portrait = select_large_portrait_image_diagnostics(
        stage.get("png"),
        title_png=title_png,
        title_frame=title_frame,
        portrait_roi=portrait_roi,
    )
    stage["large_portrait_image"] = portrait
    if portrait["status"] != "fail":
        return
    stage["status"] = "fail"
    reasons = stage.setdefault("reasons", [])
    for reason in portrait["reasons"]:
        if reason not in reasons:
            reasons.append(reason)


def draw_bounds(draw: dict[str, Any]) -> tuple[int, int, int, int] | None:
    bounds = draw.get("bounds")
    if not isinstance(bounds, dict):
        return None
    left = intish(bounds.get("left"))
    top = intish(bounds.get("top"))
    right = intish(bounds.get("right"))
    bottom = intish(bounds.get("bottom"))
    if right < left or bottom < top:
        return None
    return left, top, right, bottom


def rect_area_exclusive(rect: tuple[int, int, int, int]) -> int:
    left, top, right, bottom = rect
    return max(0, right - left) * max(0, bottom - top)


def bounds_overlap_exclusive(
    bounds: tuple[int, int, int, int],
    roi: tuple[int, int, int, int],
) -> int:
    left, top, right, bottom = bounds
    roi_left, roi_top, roi_right, roi_bottom = roi
    return rect_area_exclusive(
        (
            max(left, roi_left),
            max(top, roi_top),
            min(right + 1, roi_right),
            min(bottom + 1, roi_bottom),
        )
    )


def select_preview_gp0_evidence(draw: dict[str, Any]) -> dict[str, Any]:
    bounds = draw_bounds(draw)
    source = draw.get("source") if isinstance(draw.get("source"), dict) else {}
    roi_area = max(rect_area_exclusive(SELECT_LARGE_PORTRAIT_ROI), 1)
    overlap = bounds_overlap_exclusive(bounds, SELECT_LARGE_PORTRAIT_ROI) if bounds else 0
    overlap_ratio = overlap / roi_area
    return {
        "has_source_address": bool(source.get("address_hex")),
        "large_portrait_roi_overlap_pixels": overlap,
        "large_portrait_roi_overlap_ratio": round(overlap_ratio, 4),
        "valid": bool(source.get("address_hex"))
        and overlap_ratio >= SELECT_LARGE_PORTRAIT_MIN_GP0_ROI_OVERLAP,
    }


def is_select_preview_draw(draw: dict[str, Any]) -> bool:
    kind = str(draw.get("kind") or "")
    if not kind.startswith("textured_"):
        return False

    bounds = draw_bounds(draw)
    if bounds is None:
        return False
    left, top, right, bottom = bounds
    width = right - left + 1
    height = bottom - top + 1
    drawn = intish(draw.get("drawn_pixels") or draw.get("written_pixels"))

    if drawn <= 0 or width < SELECT_PREVIEW_MIN_WIDTH or height < SELECT_PREVIEW_MIN_HEIGHT:
        return False
    if top < 64 or bottom < 260:
        return False
    if left > 280 or right < 96:
        return False

    # Full-screen/backdrop tiles are large too, but they start at the top-left
    # and are not the vertical character preview region.
    if left <= 16 and top <= 32 and width >= 220 and height >= 220:
        return False
    if not select_preview_gp0_evidence(draw)["valid"]:
        return False
    return True


def select_large_portrait_image_diagnostics(
    png: str | None,
    title_png: str | None = None,
    title_frame: dict[str, Any] | None = None,
    portrait_roi: tuple[int, int, int, int] = SELECT_LARGE_PORTRAIT_ROI,
) -> dict[str, Any]:
    if not png:
        return {"status": "fail", "reasons": ["select_large_portrait_png_missing"]}
    try:
        from PIL import Image
    except ImportError:
        return {"status": "fail", "reasons": ["select_large_portrait_pillow_missing"]}

    try:
        image = Image.open(png).convert("RGB")
    except (OSError, ValueError) as exc:
        return {
            "status": "fail",
            "reasons": [f"select_large_portrait_png_unreadable:{exc}"],
        }

    left, top, right, bottom = portrait_roi
    if image.width < right or image.height < bottom:
        return {
            "status": "fail",
            "reasons": [
                f"select_large_portrait_png_too_small:{image.width}x{image.height}"
            ],
            "roi": {
                "left": left,
                "top": top,
                "right": right,
                "bottom": bottom,
            },
        }

    crop = image.crop(portrait_roi)
    width, height = crop.size
    pixel_reader = getattr(crop, "get_flattened_data", crop.getdata)
    pixels = list(pixel_reader())
    total = max(len(pixels), 1)

    nonblack = 0
    chroma = 0
    quantized_counter: Counter[tuple[int, int, int]] = Counter()
    luminance: list[float] = []
    for red, green, blue in pixels:
        lum = 0.2126 * red + 0.7152 * green + 0.0722 * blue
        luminance.append(lum)
        if max(red, green, blue) > 24:
            nonblack += 1
            quantized_counter[(red // 16, green // 16, blue // 16)] += 1
        if max(red, green, blue) > 50 and max(red, green, blue) - min(red, green, blue) > 32:
            chroma += 1

    edge = 0
    for y in range(height):
        row = y * width
        for x in range(width - 1):
            if abs(luminance[row + x] - luminance[row + x + 1]) > 28:
                edge += 1
    for y in range(height - 1):
        row = y * width
        next_row = (y + 1) * width
        for x in range(width):
            if abs(luminance[row + x] - luminance[next_row + x]) > 28:
                edge += 1

    edge_basis = max((width - 1) * height + width * (height - 1), 1)
    nonblack_ratio = nonblack / total
    chroma_ratio = chroma / total
    edge_ratio = edge / edge_basis
    dominant = quantized_counter.most_common(1)[0][1] if quantized_counter else 0
    dominant_quantized_ratio = dominant / max(nonblack, 1)
    tile_grid = repeating_tile_grid_metrics(image, portrait_roi)
    title_logo_reuse = select_title_logo_reuse_diagnostics(
        png,
        title_png,
        title_frame,
        portrait_roi=portrait_roi,
    )
    reasons: list[str] = []
    if nonblack_ratio < SELECT_LARGE_PORTRAIT_MIN_NONBLACK_RATIO:
        reasons.append("select_large_portrait_low_nonblack")
    if chroma_ratio < SELECT_LARGE_PORTRAIT_MIN_CHROMA_RATIO:
        reasons.append("select_large_portrait_low_chroma")
    if edge_ratio < SELECT_LARGE_PORTRAIT_MIN_EDGE_RATIO:
        reasons.append("select_large_portrait_low_edge")
    if edge_ratio > SELECT_LARGE_PORTRAIT_MAX_EDGE_RATIO:
        reasons.append("select_large_portrait_noisy_texture_artifact")
    if len(quantized_counter) < SELECT_LARGE_PORTRAIT_MIN_QUANTIZED_COLORS:
        reasons.append("select_large_portrait_low_color_diversity")
    if float(tile_grid.get("combined_similarity") or 0.0) >= REPEATING_TILE_MIN_SIMILARITY:
        reasons.append("select_large_portrait_repeating_tile_grid")
    if title_logo_reuse.get("status") == "fail":
        reasons.append("select_large_portrait_title_logo_reuse")

    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "png": png,
        "roi": {
            "left": left,
            "top": top,
            "right": right,
            "bottom": bottom,
        },
        "nonblack_pixels": nonblack,
        "nonblack_ratio": round(nonblack_ratio, 4),
        "chroma_pixels": chroma,
        "chroma_ratio": round(chroma_ratio, 4),
        "edge_pixels": edge,
        "edge_ratio": round(edge_ratio, 4),
        "quantized_colors": len(quantized_counter),
        "dominant_quantized_ratio": round(dominant_quantized_ratio, 4),
        "repeating_tile_grid": tile_grid,
        "title_logo_reuse": title_logo_reuse,
    }


def iter_gpu_draw_commands(snapshot: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not isinstance(snapshot, dict):
        return []
    gpu = snapshot.get("gpu")
    if not isinstance(gpu, dict):
        return []

    commands: list[dict[str, Any]] = []
    for key in (
        "gpu_recent_focus_draw_commands",
        "gpu_recent_overlap_draw_commands",
        "gpu_recent_draw_commands",
        "gpu_top_draw_commands",
    ):
        value = gpu.get(key)
        if isinstance(value, list):
            commands.extend(item for item in value if isinstance(item, dict))
    largest = gpu.get("gpu_largest_draw_command")
    if isinstance(largest, dict):
        commands.append(largest)
    return commands


def draw_palette_basis(draw: dict[str, Any]) -> int:
    return max(
        intish(draw.get("texture_nonzero_samples")),
        intish(draw.get("drawn_pixels")),
        intish(draw.get("written_pixels")),
        intish(draw.get("sampled_pixels")),
        1,
    )


def select_preview_diagnostics(
    snapshot: dict[str, Any] | None,
    title_png: str | None = None,
    title_frame: dict[str, Any] | None = None,
) -> dict[str, Any]:
    image = select_large_portrait_image_diagnostics(
        snapshot_png(snapshot),
        title_png=title_png,
        title_frame=title_frame,
    )
    candidates = [draw for draw in iter_gpu_draw_commands(snapshot) if is_select_preview_draw(draw)]
    if not candidates:
        reasons: list[str] = []
        if image["status"] == "fail":
            reasons.extend(image["reasons"])
            reasons.append("select_model_preview_no_texture_draws")
        return {
            "status": "fail" if reasons else "pass",
            "reasons": reasons,
            "candidate_count": 0,
            "large_portrait_image": image,
            "telemetry_status": "missing",
            "telemetry_reasons": ["select_model_preview_no_texture_draws"],
        }

    best = max(
        candidates,
        key=lambda draw: (
            intish(draw.get("drawn_pixels") or draw.get("written_pixels")),
            intish(draw.get("sampled_pixels")),
        ),
    )
    basis = draw_palette_basis(best)
    clut_blank = intish(best.get("clut_blank_samples"))
    palette_fallback = intish(best.get("palette_fallback_samples"))
    clut_blank_ratio = clut_blank / basis
    palette_fallback_ratio = palette_fallback / basis
    drawn = intish(best.get("drawn_pixels") or best.get("written_pixels"))
    color_hash_hex = str(best.get("color_hash_hex") or "").lower()

    reasons: list[str] = []
    if drawn < SELECT_PREVIEW_MIN_DRAWN_PIXELS:
        reasons.append("select_model_preview_no_texture_draws")
    if image["status"] == "fail":
        reasons.extend(image["reasons"])
    if (
        color_hash_hex in SELECT_PREVIEW_KNOWN_BAD_PALETTE_HASHES
        and clut_blank_ratio > SELECT_PREVIEW_MAX_BAD_PALETTE_RATIO
        and palette_fallback_ratio > SELECT_PREVIEW_MAX_BAD_PALETTE_RATIO
    ):
        reasons.append("select_model_preview_known_bad_palette")

    source = best.get("source") if isinstance(best.get("source"), dict) else {}
    gp0_evidence = select_preview_gp0_evidence(best)
    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "candidate_count": len(candidates),
        "large_portrait_image": image,
        "gp0_evidence": gp0_evidence,
        "source_address_hex": source.get("address_hex"),
        "texture_page_hex": best.get("texture_page_hex"),
        "clut_hex": best.get("clut_hex"),
        "bounds": best.get("bounds"),
        "sampled_pixels": intish(best.get("sampled_pixels")),
        "drawn_pixels": drawn,
        "written_pixels": intish(best.get("written_pixels")),
        "texture_nonzero_samples": intish(best.get("texture_nonzero_samples")),
        "clut_blank_samples": clut_blank,
        "palette_fallback_samples": palette_fallback,
        "clut_blank_ratio": round(clut_blank_ratio, 4),
        "palette_fallback_ratio": round(palette_fallback_ratio, 4),
        "color_changes": intish(best.get("color_changes")),
        "color_hash_hex": color_hash_hex or None,
    }


def aggregate_activity(values: list[dict[str, Any]]) -> dict[str, Any]:
    keys: set[str] = set()
    for value in values:
        if isinstance(value, dict):
            keys.update(value.keys())
    out: dict[str, Any] = {}
    for key in keys:
        seen = [value.get(key) for value in values if isinstance(value, dict)]
        if all(isinstance(v, bool) for v in seen):
            out[key] = any(seen)
        elif all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in seen):
            out[key] = max(seen) if seen else 0
        elif seen:
            out[key] = seen[-1]
    return out


def input_verdict(activity: dict[str, Any]) -> dict[str, Any]:
    required_groups = {
        "p1": {
            "coin": (
                "system_coin_active_reads",
                "coin_insert_edges",
                "coin_counter_0_edges",
                "native_credit_adapter_edges",
            ),
            "start": (
                "system_start_active_reads",
                "p1_start_active_reads",
                "legacy_system_start_latch_edges",
            ),
            "up": ("p1_up_active_reads",),
            "down": ("p1_down_active_reads",),
            "left": ("p1_left_active_reads",),
            "right": ("p1_right_active_reads",),
            "punch": ("p1_punch_active_reads",),
            "kick": ("p1_kick_active_reads",),
            "beast": ("p1_beast_active_reads",),
            "guard": ("p3_guard_active_reads",),
        },
        "p2": {
            "coin": (
                "system_p2_coin_active_reads",
                "p2_coin_insert_edges",
                "coin_counter_1_edges",
            ),
            "start": ("system_p2_start_active_reads",),
            "up": ("p2_up_active_reads",),
            "down": ("p2_down_active_reads",),
            "left": ("p2_left_active_reads",),
            "right": ("p2_right_active_reads",),
            "punch": ("p2_punch_active_reads",),
            "kick": ("p2_kick_active_reads",),
            "beast": ("p2_beast_active_reads",),
            "guard": ("p2_guard_active_reads",),
        },
    }
    guard_not_polled = int(activity.get("p3_input_reads") or 0) == 0
    missing: dict[str, list[str]] = {}
    for player, groups in required_groups.items():
        missing[player] = []
        for name, keys in groups.items():
            if player == "p1" and name == "guard" and guard_not_polled:
                continue
            if not any(int(activity.get(key) or 0) > 0 for key in keys):
                missing[player].append(name)
    status = "pass" if not missing["p1"] and not missing["p2"] else "fail"
    counter_keys = sorted({key for groups in required_groups.values() for keys in groups.values() for key in keys})
    return {
        "status": status,
        "missing": missing,
        "booleans": {
            "has_p1_full_control_activity": activity.get("has_p1_full_control_activity"),
            "has_p2_full_control_activity": activity.get("has_p2_full_control_activity"),
            "has_combined_full_control_activity": activity.get(
                "has_combined_full_control_activity"
            ),
            "has_any_control_activity": activity.get("has_any_control_activity"),
        },
        "counters": {key: activity.get(key) for key in counter_keys},
    }


def recovery_verdict(recoveries: list[dict[str, Any]]) -> dict[str, Any]:
    reasons = Counter(str(rec.get("last_reason")) for rec in recoveries if isinstance(rec, dict))
    model_reasons = Counter(
        str(rec.get("last_chain_model_selection_reason"))
        for rec in recoveries
        if isinstance(rec, dict) and rec.get("last_chain_model_selection_reason") is not None
    )
    bad = {reason: count for reason, count in reasons.items() if reason in BAD_RECOVERY_REASONS}
    warn = {
        reason: count
        for reason, count in (reasons + model_reasons).items()
        if reason in WARN_RECOVERY_REASONS
    }
    newest = recoveries[-1] if recoveries else {}
    final_model_selected = (
        newest.get("last_reason") in {"submitted", "submitted_chain_model_replacement"}
        and newest.get("last_chain_model_selection_reason")
        in {"selected", "selected_reused_current_otc"}
        and intish(newest.get("last_chain_model_texture_draws")) > 0
        and intish(newest.get("last_chain_model_packets")) > 0
    )
    transient_warn = warn if final_model_selected else {}
    active_warn = {} if final_model_selected else warn
    status = "fail" if bad else "warn" if active_warn else "pass"
    return {
        "status": status,
        "bad_reason_counts": bad,
        "warn_reason_counts": active_warn,
        "transient_warn_reason_counts": transient_warn,
        "last_reason_counts": dict(reasons),
        "model_selection_reason_counts": dict(model_reasons),
        "final": {
            "last_reason": newest.get("last_reason"),
            "last_chain_model_selection_reason": newest.get("last_chain_model_selection_reason"),
            "last_chain_model_texture_draws": newest.get("last_chain_model_texture_draws"),
            "last_chain_model_packets": newest.get("last_chain_model_packets"),
            "last_model_candidate_packets": newest.get("last_model_candidate_packets"),
            "last_model_packets": newest.get("last_model_packets"),
            "last_recovered_chain_words": newest.get("last_recovered_chain_words"),
        },
    }


def status_from_failures(failures: list[str], warnings: list[str] | None = None) -> str:
    if failures:
        return "fail"
    if warnings:
        return "warn"
    return "pass"


def unique_reasons(reasons: list[str]) -> list[str]:
    unique: list[str] = []
    for reason in reasons:
        if reason not in unique:
            unique.append(reason)
    return unique


def first_snapshot_after_action(
    snapshots: list[dict[str, Any]], action: str, prefer_next: bool = False
) -> dict[str, Any] | None:
    for index, snap in enumerate(snapshots):
        if snap.get("action") == action:
            if prefer_next and index + 1 < len(snapshots):
                return snapshots[index + 1]
            return snap
    return None


def first_snapshot_index_after_action(
    snapshots: list[dict[str, Any]], action: str
) -> int | None:
    for index, snap in enumerate(snapshots):
        if snap.get("action") == action:
            return index
    return None


SMOKE_BEAST_ACTIONS = ("beast", "p2+beast")


def snapshot_is_gameplay(snapshot: dict[str, Any] | None) -> bool:
    return isinstance(snapshot, dict) and boolish(
        get_path(snapshot, "window_frame", "gameplay_scene")
    )


def snapshot_has_smoke_beast_action(snapshot: dict[str, Any] | None) -> bool:
    return isinstance(snapshot, dict) and snapshot.get("action") in SMOKE_BEAST_ACTIONS


def smoke_beast_snapshot_indices(snapshots: list[dict[str, Any]]) -> list[int]:
    return [
        index
        for index, snap in enumerate(snapshots)
        if snapshot_has_smoke_beast_action(snap)
    ]


def smoke_gameplay_beast_snapshot_indices(snapshots: list[dict[str, Any]]) -> list[int]:
    indices: list[int] = []
    for beast_action_index in smoke_beast_snapshot_indices(snapshots):
        beast_result_index = smoke_beast_result_index_after_action(
            snapshots,
            beast_action_index,
        )
        if beast_result_index is not None:
            indices.append(beast_result_index)
    return indices


def first_gameplay_snapshot_index_after(
    snapshots: list[dict[str, Any]], start: int
) -> int | None:
    for index in range(max(start, 0), len(snapshots)):
        if snapshot_is_gameplay(snapshots[index]):
            return index
    return None


def previous_gameplay_snapshot_index_before(
    snapshots: list[dict[str, Any]], start: int
) -> int | None:
    for index in range(min(start, len(snapshots)) - 1, -1, -1):
        if snapshot_is_gameplay(snapshots[index]):
            return index
    return None


def smoke_beast_result_index_after_action(
    snapshots: list[dict[str, Any]], beast_action_index: int
) -> int | None:
    if beast_action_index < 0 or beast_action_index >= len(snapshots):
        return None
    if snapshot_is_gameplay(snapshots[beast_action_index]):
        return beast_action_index

    first_gameplay_index = first_gameplay_snapshot_index_after(
        snapshots,
        beast_action_index + 1,
    )
    if (
        first_gameplay_index is not None
        and snapshots[first_gameplay_index].get("action") == "noop"
    ):
        return first_gameplay_index
    return None


def first_post_gameplay_snapshot_index_after(
    snapshots: list[dict[str, Any]], start: int
) -> int | None:
    for index in range(max(start, 0), len(snapshots)):
        snap = snapshots[index]
        if (
            snapshot_is_gameplay(snap)
            and snap.get("action") == "noop"
            and not snapshot_has_smoke_beast_action(snap)
        ):
            return index
    return None


def first_smoke_gameplay_beast_pair(
    snapshots: list[dict[str, Any]],
) -> tuple[int | None, int | None, int | None, list[int], list[int]]:
    beast_indices = smoke_beast_snapshot_indices(snapshots)
    gameplay_beast_indices: list[int] = []
    for beast_action_index in beast_indices:
        beast_index = smoke_beast_result_index_after_action(
            snapshots,
            beast_action_index,
        )
        if beast_index is None:
            continue
        gameplay_beast_indices.append(beast_index)
        post_index = first_post_gameplay_snapshot_index_after(snapshots, beast_index + 1)
        if post_index is not None:
            return (
                beast_index,
                post_index,
                beast_action_index,
                beast_indices,
                gameplay_beast_indices,
            )
    if gameplay_beast_indices:
        beast_index = gameplay_beast_indices[0]
        beast_action_index = next(
            (
                index
                for index in beast_indices
                if smoke_beast_result_index_after_action(snapshots, index) == beast_index
            ),
            None,
        )
        return (
            beast_index,
            None,
            beast_action_index,
            beast_indices,
            gameplay_beast_indices,
        )
    return None, None, None, beast_indices, gameplay_beast_indices


def image_delta_diagnostics(before_png: str | None, after_png: str | None) -> dict[str, Any]:
    if not before_png or not after_png:
        return {"status": "not_checked", "reason": "beast_delta_png_missing"}
    try:
        from PIL import Image
    except ImportError:
        return {"status": "not_checked", "reason": "pillow_missing"}
    try:
        before = Image.open(before_png).convert("RGB")
        after = Image.open(after_png).convert("RGB")
    except (OSError, ValueError) as exc:
        return {"status": "not_checked", "reason": f"png_unreadable:{exc}"}
    if before.size != after.size:
        return {
            "status": "pass",
            "reason": "image_size_changed",
            "before_size": before.size,
            "after_size": after.size,
        }

    before_pixels = list(getattr(before, "get_flattened_data", before.getdata)())
    after_pixels = list(getattr(after, "get_flattened_data", after.getdata)())
    total = max(len(before_pixels), 1)
    changed = 0
    delta_sum = 0
    for left, right in zip(before_pixels, after_pixels):
        delta = abs(left[0] - right[0]) + abs(left[1] - right[1]) + abs(left[2] - right[2])
        delta_sum += delta
        if delta > 24:
            changed += 1
    changed_ratio = changed / total
    mean_delta = delta_sum / (total * 3)
    stuck = (
        changed_ratio < BEAST_STUCK_MAX_CHANGED_RATIO
        and mean_delta < BEAST_STUCK_MAX_MEAN_DELTA
    )
    return {
        "status": "fail" if stuck else "pass",
        "reason": "beast_effect_stuck_frame" if stuck else "frame_changed",
        "before_png": before_png,
        "after_png": after_png,
        "changed_pixels": changed,
        "changed_ratio": round(changed_ratio, 4),
        "mean_delta": round(mean_delta, 4),
    }


def draw_source_hex(draw: dict[str, Any]) -> str | None:
    source = draw.get("source") if isinstance(draw.get("source"), dict) else {}
    value = source.get("address_hex")
    return str(value).lower() if value else None


def smoke_post_beast_candidate_indices(
    snapshots: list[dict[str, Any]],
    beast_action_index: int,
    limit: int = BEAST_POST_ACTION_SEQUENCE_LIMIT,
) -> list[int]:
    indices: list[int] = []
    for index in range(beast_action_index + 1, len(snapshots)):
        snap = snapshots[index]
        if snapshot_has_smoke_beast_action(snap):
            break
        if snapshot_is_gameplay(snap) and snap.get("action") == "noop":
            indices.append(index)
            if len(indices) >= limit:
                break
    return indices


def snapshot_gameplay_image_reasons(
    snapshot: dict[str, Any],
    kind: str,
) -> tuple[list[str], dict[str, Any]]:
    reasons: list[str] = []
    if not snapshot_is_gameplay(snapshot):
        reasons.append("post_beast_not_gameplay")
    for reason in frame_reasons(get_path(snapshot, "window_frame"), kind):
        if reason not in reasons:
            reasons.append(reason)
    image = frame_image_diagnostics(
        snapshot_png(snapshot),
        kind,
        get_path(snapshot, "window_frame"),
    )
    if image["status"] == "fail":
        for reason in image["reasons"]:
            if reason not in reasons:
                reasons.append(reason)
    return reasons, image


def snapshot_post_beast_effect_reasons(
    post_snapshot: dict[str, Any],
    delta: dict[str, Any] | None = None,
) -> list[str]:
    reasons: list[str] = []
    for draw in iter_gpu_draw_commands(post_snapshot):
        if draw_source_hex(draw) == "0x0038b1b8":
            reasons.append("beast_effect_residual_draw:0x0038b1b8")
            break

    rec = get_path(post_snapshot, "native_sync", "native_otc_dma_recovery", default={})
    if (
        isinstance(rec, dict)
        and rec.get("last_chain_model_selection_reason") == "selected_reused_current_otc"
        and isinstance(delta, dict)
        and delta.get("status") == "fail"
    ):
        reasons.append("beast_reused_current_otc_stuck_after_noop")
    return reasons


def snapshot_pair_beast_effect_reasons(
    beast_snapshot: dict[str, Any],
    post_snapshot: dict[str, Any],
) -> tuple[list[str], dict[str, Any]]:
    reasons: list[str] = []
    delta = image_delta_diagnostics(snapshot_png(beast_snapshot), snapshot_png(post_snapshot))
    if delta.get("status") == "fail":
        reasons.append(str(delta.get("reason") or "beast_effect_stuck_frame"))
    reasons.extend(snapshot_post_beast_effect_reasons(post_snapshot, delta))
    return unique_reasons(reasons), delta


def smoke_beast_sequence_failure_reasons(checks: list[dict[str, Any]]) -> list[str]:
    reasons = unique_reasons(
        [
            reason
            for check in checks
            for reason in check.get("reasons", [])
            if isinstance(reason, str)
        ]
    )
    if (
        len(checks) > 1
        and all("beast_effect_stuck_frame" in check.get("reasons", []) for check in checks)
    ):
        reasons.append("beast_effect_persistent_stuck_sequence")
    if (
        len(checks) > 1
        and all(
            "beast_effect_residual_draw:0x0038b1b8" in check.get("reasons", [])
            for check in checks
        )
    ):
        reasons.append("beast_effect_persistent_residual_draw")
    return unique_reasons(reasons or ["post_beast_gameplay_snapshot_invalid"])


def smoke_beast_sequence_for_action(
    snapshots: list[dict[str, Any]],
    beast_action_index: int,
) -> dict[str, Any]:
    beast_action = snapshots[beast_action_index]
    reference_index = beast_action_index if snapshot_is_gameplay(beast_action) else None
    if (
        reference_index is None
        and previous_gameplay_snapshot_index_before(snapshots, beast_action_index) is None
    ):
        return {
            "status": "fail",
            "reasons": ["beast_gameplay_snapshot_missing"],
            "action": beast_action.get("action"),
            "beast_action_snapshot_index": beast_action_index,
            "beast_action_tail_index": beast_action.get("tail_index"),
            "beast_snapshot_index": None,
            "beast_tail_index": None,
            "post_snapshot_index": None,
            "post_tail_index": None,
            "post_action": None,
            "checked_post_snapshot_count": 0,
            "checks": [],
            "hard_failure": False,
        }
    candidate_indices = smoke_post_beast_candidate_indices(snapshots, beast_action_index)
    checks: list[dict[str, Any]] = []

    if not candidate_indices:
        return {
            "status": "fail",
            "reasons": ["post_beast_gameplay_snapshot_missing"],
            "action": beast_action.get("action"),
            "beast_action_snapshot_index": beast_action_index,
            "beast_action_tail_index": beast_action.get("tail_index"),
            "beast_snapshot_index": reference_index,
            "beast_tail_index": beast_action.get("tail_index") if reference_index is not None else None,
            "post_snapshot_index": None,
            "post_tail_index": None,
            "post_action": None,
            "checked_post_snapshot_count": 0,
            "checks": checks,
            "hard_failure": True,
        }

    for candidate_index in candidate_indices:
        post_snapshot = snapshots[candidate_index]
        valid_reasons, image = snapshot_gameplay_image_reasons(post_snapshot, "beast")
        if reference_index is not None:
            effect_reasons, delta = snapshot_pair_beast_effect_reasons(
                snapshots[reference_index],
                post_snapshot,
            )
        else:
            delta = {"status": "not_checked", "reason": "beast_reference_not_gameplay"}
            effect_reasons = snapshot_post_beast_effect_reasons(post_snapshot)
        check_reasons = unique_reasons(valid_reasons + effect_reasons)
        check = {
            "post_snapshot_index": candidate_index,
            "post_tail_index": post_snapshot.get("tail_index"),
            "post_action": post_snapshot.get("action"),
            "reasons": check_reasons,
            "delta": delta,
            "image": image,
        }
        checks.append(check)
        if not check_reasons:
            selected_beast_index = reference_index if reference_index is not None else candidate_index
            return {
                "status": "pass",
                "reasons": [],
                "action": beast_action.get("action"),
                "beast_action_snapshot_index": beast_action_index,
                "beast_action_tail_index": beast_action.get("tail_index"),
                "beast_snapshot_index": selected_beast_index,
                "beast_tail_index": snapshots[selected_beast_index].get("tail_index")
                if selected_beast_index is not None
                else None,
                "post_snapshot_index": candidate_index,
                "post_tail_index": post_snapshot.get("tail_index"),
                "post_action": post_snapshot.get("action"),
                "checked_post_snapshot_count": len(checks),
                "candidate_post_snapshot_count": len(candidate_indices),
                "checks": checks,
                "delta": delta,
                "hard_failure": False,
            }

    return {
        "status": "fail",
        "reasons": smoke_beast_sequence_failure_reasons(checks),
        "action": beast_action.get("action"),
        "beast_action_snapshot_index": beast_action_index,
        "beast_action_tail_index": beast_action.get("tail_index"),
        "beast_snapshot_index": reference_index,
        "beast_tail_index": beast_action.get("tail_index") if reference_index is not None else None,
        "post_snapshot_index": candidate_indices[0],
        "post_tail_index": snapshots[candidate_indices[0]].get("tail_index"),
        "post_action": snapshots[candidate_indices[0]].get("action"),
        "checked_post_snapshot_count": len(checks),
        "candidate_post_snapshot_count": len(candidate_indices),
        "checks": checks,
        "delta": checks[-1].get("delta") if checks else None,
        "hard_failure": True,
    }


def beast_effect_verdict(snapshots: list[dict[str, Any]]) -> dict[str, Any]:
    beast_indices = smoke_beast_snapshot_indices(snapshots)
    if not beast_indices:
        return {
            "status": "not_checked",
            "reason": "beast_action_missing",
            "beast_snapshot_count": 0,
            "gameplay_beast_snapshot_count": 0,
        }

    sequences = [
        smoke_beast_sequence_for_action(snapshots, beast_index)
        for beast_index in beast_indices
    ]
    gameplay_beast_indices = [
        int(sequence["beast_snapshot_index"])
        for sequence in sequences
        if sequence.get("beast_snapshot_index") is not None
    ]
    relevant_sequences = [
        sequence
        for sequence in sequences
        if sequence.get("beast_snapshot_index") is not None
        or boolish(sequence.get("hard_failure"))
    ]
    failures = [
        sequence
        for sequence in relevant_sequences
        if sequence.get("status") != "pass"
    ]
    if failures:
        failure = failures[0]
        return {
            **failure,
            "status": "fail",
            "beast_snapshot_count": len(beast_indices),
            "gameplay_beast_snapshot_count": len(gameplay_beast_indices),
            "sequences": sequences,
        }

    passing = relevant_sequences[-1] if relevant_sequences else None
    if passing is not None:
        return {
            **passing,
            "beast_snapshot_count": len(beast_indices),
            "gameplay_beast_snapshot_count": len(gameplay_beast_indices),
            "sequences": sequences,
        }

    failure = sequences[0]
    reasons = unique_reasons(
        [
            reason
            for sequence in sequences
            for reason in sequence.get("reasons", [])
            if isinstance(reason, str)
        ]
        or ["beast_gameplay_snapshot_missing"]
    )
    return {
        **failure,
        "status": "fail",
        "reasons": reasons,
        "beast_snapshot_count": len(beast_indices),
        "gameplay_beast_snapshot_count": len(gameplay_beast_indices),
        "sequences": sequences,
    }


def summarize_smoke(args: argparse.Namespace) -> dict[str, Any]:
    data, parse_error = load_smoke_json(args.smoke_stdout)
    result: dict[str, Any] = {
        "command": "native-match-tail-timeline",
        "exit_code": args.smoke_status,
        "stdout": args.smoke_stdout,
        "stderr": args.smoke_stderr,
        "output_prefix": args.smoke_prefix,
        "parse_error": parse_error,
        "stderr_tail": tail_text(args.smoke_stderr),
    }
    if not isinstance(data, dict):
        result["status"] = "fail"
        result["failures"] = ["stdout_json_unavailable"]
        return result

    snapshots = [snap for snap in data.get("snapshots", []) if isinstance(snap, dict)]
    boot = data.get("boot") if isinstance(data.get("boot"), dict) else {}
    select = snapshots[3] if len(snapshots) > 3 else (snapshots[-1] if snapshots else None)
    final = snapshots[-1] if snapshots else None
    combat = next(
        (
            snap
            for snap in snapshots
            if int(snap.get("tail_index") or 0) >= 5
            and boolish(get_path(snap, "window_frame", "gameplay_scene"))
        ),
        snapshots[-1] if snapshots else None,
    )
    beast_effect_summary = beast_effect_verdict(snapshots)
    selected_beast_index = beast_effect_summary.get("post_snapshot_index")
    if not isinstance(selected_beast_index, int):
        selected_beast_index = beast_effect_summary.get("beast_snapshot_index")
    beast = None
    if isinstance(selected_beast_index, int) and 0 <= selected_beast_index < len(snapshots):
        beast = snapshots[selected_beast_index]

    activities = [snap.get("input_activity", {}) for snap in snapshots]
    final_activity = get_path(data, "state", "input_activity", default={})
    if isinstance(final_activity, dict):
        activities.append(final_activity)
    activity = aggregate_activity([a for a in activities if isinstance(a, dict)])

    recoveries: list[dict[str, Any]] = []
    for snap in snapshots:
        rec = get_path(snap, "native_sync", "native_otc_dma_recovery")
        if isinstance(rec, dict):
            recoveries.append(rec)
    final_rec = find_first_key(data.get("native_sync", {}), "native_otc_dma_recovery")
    if isinstance(final_rec, dict):
        recoveries.append(final_rec)

    stage_items = {
        "boot": {
            "png": existing_png(boot.get("window_output")),
            **summarize_frame(boot.get("window_frame"), "boot"),
        },
        "select": {
            "png": snapshot_png(select),
            "alternate_pngs": snapshot_pngs(select),
            **summarize_frame(get_path(select, "window_frame"), "select"),
        },
        "combat": {
            "png": snapshot_png(combat),
            "alternate_pngs": snapshot_pngs(combat),
            **summarize_frame(get_path(combat, "window_frame"), "combat"),
        },
        "beast": {
            "png": snapshot_png(beast),
            "alternate_pngs": snapshot_pngs(beast),
            **summarize_frame(get_path(beast, "window_frame"), "beast"),
        },
        "final": {
            "png": snapshot_png(final),
            "alternate_pngs": snapshot_pngs(final),
            **summarize_frame(get_path(final, "window_frame"), "final"),
        },
    }
    attach_stage_image_diagnostics(stage_items["boot"], "boot")
    attach_stage_image_diagnostics(stage_items["select"], "select")
    attach_select_portrait_image_diagnostics(stage_items["select"], stage_items["boot"])
    attach_stage_image_diagnostics(stage_items["combat"], "combat")
    attach_stage_image_diagnostics(stage_items["beast"], "beast")
    attach_stage_image_diagnostics(stage_items["final"], "combat")

    select_rec = get_path(select, "native_sync", "native_otc_dma_recovery", default={})
    select_model_missing = (
        isinstance(select_rec, dict)
        and select_rec.get("last_chain_model_selection_reason") == "no_model_draws"
        and intish(select_rec.get("last_chain_model_texture_draws")) == 0
    )
    select_preview = select_preview_diagnostics(
        select,
        title_png=stage_items["boot"].get("png"),
        title_frame=stage_items["boot"].get("frame"),
    )
    if (
        not select_model_missing
        and select_preview["candidate_count"] == 0
        and get_path(select_preview, "large_portrait_image", "status") != "fail"
    ):
        select_preview = {
            **select_preview,
            "status": "not_checked",
            "reasons": [],
            "reason": "select_model_chain_not_missing",
        }
    stage_items["select"]["select_preview"] = select_preview
    if select_model_missing or select_preview["candidate_count"] > 0:
        if select_preview["status"] == "fail":
            stage_items["select"]["status"] = "fail"
            reasons = stage_items["select"].setdefault("reasons", [])
            for reason in select_preview["reasons"]:
                if reason not in reasons:
                    reasons.append(reason)

    input_summary = input_verdict(activity)
    recovery_summary = recovery_verdict(recoveries)
    png_count = len(list(Path(args.smoke_prefix).parent.glob("*.png"))) if args.smoke_prefix else 0

    failures: list[str] = []
    warnings: list[str] = []
    if args.smoke_status != 0:
        failures.append(f"command_exit_code:{args.smoke_status}")
    if int(data.get("missed_vblank_frames") or 0) != 0:
        failures.append(f"missed_vblank_frames:{data.get('missed_vblank_frames')}")
    if not snapshots:
        failures.append("no_timeline_snapshots")
    for name, stage in stage_items.items():
        if stage["status"] == "fail":
            failures.append(f"{name}_frame:{','.join(stage['reasons'])}")
        if not stage.get("png"):
            failures.append(f"{name}_png_missing")
    if input_summary["status"] != "pass":
        failures.append("input_missing:" + json.dumps(input_summary["missing"], sort_keys=True))
    if recovery_summary["status"] == "fail":
        failures.append("recovery_bad_reason")
    elif recovery_summary["status"] == "warn":
        warnings.append("recovery_warn_reason")
    if beast_effect_summary["status"] == "fail":
        failures.append("beast_effect:" + ",".join(beast_effect_summary["reasons"]))
    if png_count == 0:
        failures.append("no_png_artifacts")

    result.update(
        {
            "status": status_from_failures(failures, warnings),
            "failures": failures,
            "warnings": warnings,
            "total_frames": data.get("total_frames"),
            "missed_vblank_frames": data.get("missed_vblank_frames"),
            "executed_steps": data.get("executed_steps"),
            "png_count": png_count,
            "stages": stage_items,
            "input": input_summary,
            "audio": {
                "status": "not_checked_headless",
                "reason": "native-match-tail-timeline does not start realtime CoreAudio",
            },
            "beast_effect": beast_effect_summary,
            "recovery": recovery_summary,
        }
    )
    return result


def load_capture_jsons(capture_dir: str) -> list[dict[str, Any]]:
    if not capture_dir:
        return []
    root = Path(capture_dir)
    if not root.exists():
        return []
    captures: list[dict[str, Any]] = []
    for path in sorted(root.glob("*.json")):
        data, _ = load_last_json(str(path))
        if isinstance(data, dict):
            if not isinstance(data.get("frame"), dict) or data.get("label") is None:
                continue
            data["_json_path"] = str(path)
            data["_png_path"] = str(path.with_suffix(".png"))
            captures.append(data)
    return captures


def capture_by_predicate(
    captures: list[dict[str, Any]], predicate
) -> dict[str, Any] | None:
    for capture in captures:
        if predicate(capture):
            return capture
    return None


def summarize_capture(
    capture: dict[str, Any] | None,
    kind: str,
    expected_labels: set[str] | None = None,
) -> dict[str, Any]:
    if not isinstance(capture, dict):
        return {
            "status": "fail",
            "reasons": ["capture_missing"],
            "png": None,
            "json": None,
            "frame": {},
        }
    summary = summarize_frame(capture.get("frame"), kind)
    summary.update(
        {
            "png": existing_png(capture.get("_png_path")),
            "json": capture.get("_json_path"),
            "label": capture.get("label"),
            "gui_frame": capture.get("gui_frame"),
            "buttons": capture.get("buttons"),
        }
    )
    if not summary["png"]:
        summary["status"] = "fail"
        summary.setdefault("reasons", []).append("capture_png_missing")
    if expected_labels is not None and capture_label(capture) not in expected_labels:
        summary["status"] = "fail"
        summary.setdefault("reasons", []).append(
            f"capture_label_unexpected:{capture.get('label')}"
        )
    return summary


def live_input_verdict(data: dict[str, Any], captures: list[dict[str, Any]]) -> dict[str, Any]:
    reasons: list[str] = []
    if not boolish(data.get("native_play_test_input_verified")):
        reasons.append("native_play_test_input_verified:false")

    verification = (
        data.get("gui_input_verification")
        if isinstance(data.get("gui_input_verification"), dict)
        else {}
    )
    if not verification:
        reasons.append("gui_input_verification_missing")
    else:
        for key in LIVE_REQUIRED_GUI_INPUT_FLAGS:
            if not boolish(verification.get(key)):
                reasons.append(f"gui_input_{key}:false")

        required_buttons = (
            verification.get("required_buttons")
            if isinstance(verification.get("required_buttons"), dict)
            else {}
        )
        guest_buttons = (
            verification.get("guest_buttons")
            if isinstance(verification.get("guest_buttons"), dict)
            else {}
        )
        for key in LIVE_REQUIRED_BUTTON_FIELDS:
            if not boolish(required_buttons.get(key)):
                reasons.append(f"required_button_missing:{key}")
            if not boolish(guest_buttons.get(key)):
                reasons.append(f"guest_button_missing:{key}")

    activities = []
    for key in ("test_input_activity", "input_activity"):
        value = data.get(key)
        if isinstance(value, dict):
            activities.append(value)
    for capture in captures:
        value = capture.get("input_activity")
        if isinstance(value, dict):
            activities.append(value)
    activity = aggregate_activity(activities)
    activity_summary = input_verdict(activity)
    if activity_summary["status"] != "pass":
        reasons.append(
            "input_activity_missing:"
            + json.dumps(activity_summary["missing"], sort_keys=True)
        )

    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "native_play_test_input_verified": data.get("native_play_test_input_verified"),
        "gui_input_verification": data.get("gui_input_verification"),
        "test_input_activity": data.get("test_input_activity"),
        "input_activity": data.get("input_activity"),
        "activity": activity_summary,
    }


def live_audio_verdict(data: dict[str, Any]) -> dict[str, Any]:
    health = data.get("audio_health") if isinstance(data.get("audio_health"), dict) else {}
    audio = data.get("audio") if isinstance(data.get("audio"), dict) else {}
    reasons: list[str] = []
    if not boolish(data.get("audio_realtime_verified")):
        reasons.append("audio_realtime_verified:false")
    if not health:
        reasons.append("audio_health_missing")
    else:
        if not boolish(health.get("audible")):
            reasons.append(f"audio_not_audible:{health.get('state')}")
        for key in ("render_progressing", "pcm_nonzero", "realtime_output_seen", "realtime_ok"):
            if not boolish(health.get(key)):
                reasons.append(f"audio_{key}:false")
        reason = health.get("reason")
        if reason and health.get("state") != "active":
            reasons.append(f"audio_health_reason:{reason}")
    if not audio:
        reasons.append("audio_stats_missing")
    else:
        if not boolish(audio.get("available")):
            reasons.append("audio_unavailable")
        if not boolish(audio.get("coreaudio_started")):
            reasons.append("audio_coreaudio_started:false")
        if intish(audio.get("audio_render_batches")) <= 0:
            reasons.append("audio_render_batches:0")
        if intish(audio.get("audio_queue_push_batches")) <= 0:
            reasons.append("audio_queue_push_batches:0")
        if intish(audio.get("generated_frames")) <= 0:
            reasons.append("audio_generated_frames:0")
        if intish(audio.get("latch_nonzero_writes")) <= 0:
            reasons.append("audio_sfx_latch_nonzero_writes:0")

        ymf = audio.get("ymf271") if isinstance(audio.get("ymf271"), dict) else {}
        if not ymf:
            reasons.append("audio_ymf271_stats_missing")
        else:
            if intish(ymf.get("key_on_events")) <= 0:
                reasons.append("audio_sfx_key_on_events:0")
            if intish(ymf.get("nonzero_frames")) <= 0:
                reasons.append("audio_ymf271_nonzero_frames:0")

        queue = audio.get("queue") if isinstance(audio.get("queue"), dict) else {}
        if not queue:
            reasons.append("audio_queue_stats_missing")
        else:
            realtime_frames = intish(queue.get("coreaudio_callback_output_frames"))
            if realtime_frames <= 0:
                reasons.append("audio_coreaudio_callback_output_frames:0")
            for key in AUDIO_QUEUE_HARD_BLOCKING_COUNTER_FIELDS:
                if key in queue and intish(queue.get(key)) > 0:
                    reasons.append(f"audio_queue_{key}:{queue.get(key)}")
            unresolved_callback_miss_frames = max(
                0,
                intish(queue.get("callback_miss_frames"))
                - intish(queue.get("callback_rescue_frames")),
            )
            ratio_counters = (
                ("underflow_frames", intish(queue.get("underflow_frames"))),
                ("callback_miss_frames", unresolved_callback_miss_frames),
                ("callback_silence_frames", intish(queue.get("callback_silence_frames"))),
            )
            for key, value in ratio_counters:
                if ratio_exceeds(
                    value,
                    realtime_frames,
                    AUDIO_QUEUE_MAX_GAP_RATIO_NUMERATOR,
                    AUDIO_QUEUE_MAX_GAP_RATIO_DENOMINATOR,
                ):
                    reasons.append(f"audio_queue_{key}:{value}")
            repeated_like_frames = intish(queue.get("repeated_frames")) + intish(
                queue.get("callback_fallback_frames")
            )
            if ratio_exceeds(
                repeated_like_frames,
                realtime_frames,
                AUDIO_QUEUE_MAX_REPEAT_RATIO_NUMERATOR,
                AUDIO_QUEUE_MAX_REPEAT_RATIO_DENOMINATOR,
            ):
                reasons.append(f"audio_queue_repeated_like_frames:{repeated_like_frames}")
        if "audio_cpu_dropped_cycles" in audio and intish(audio.get("audio_cpu_dropped_cycles")) > 0:
            reasons.append(f"audio_cpu_dropped_cycles:{audio.get('audio_cpu_dropped_cycles')}")
    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "audio_realtime_verified": data.get("audio_realtime_verified"),
        "audio_health": health or data.get("audio_health"),
        "audio": data.get("audio"),
    }


def ratio_exceeds(
    value: int,
    total: int,
    numerator: int,
    denominator: int,
) -> bool:
    return total > 0 and value * denominator > total * numerator


def live_capture_pair_beast_effect_reasons(
    beast_capture: dict[str, Any],
    post_capture: dict[str, Any],
) -> tuple[list[str], dict[str, Any]]:
    reasons: list[str] = []
    delta = image_delta_diagnostics(
        existing_png(beast_capture.get("_png_path")),
        existing_png(post_capture.get("_png_path")),
    )
    if delta.get("status") == "fail":
        reasons.append(str(delta.get("reason") or "beast_effect_stuck_frame"))

    for draw in iter_gpu_draw_commands(post_capture):
        if draw_source_hex(draw) == "0x0038b1b8":
            reasons.append("beast_effect_residual_draw:0x0038b1b8")
            break

    rec = find_first_key(post_capture, "native_otc_dma_recovery")
    if (
        isinstance(rec, dict)
        and rec.get("last_chain_model_selection_reason") == "selected_reused_current_otc"
        and delta.get("status") == "fail"
    ):
        reasons.append("beast_reused_current_otc_stuck_after_noop")
    return reasons, delta


def live_post_beast_candidate_indices(
    captures: list[dict[str, Any]],
    beast_capture_index: int,
    limit: int = BEAST_POST_ACTION_SEQUENCE_LIMIT,
) -> list[int]:
    indices: list[int] = []
    for index in range(beast_capture_index + 1, len(captures)):
        capture = captures[index]
        if capture_has_any_button(capture, "beast", "p2_beast"):
            break
        if capture_is_combat_like(capture):
            indices.append(index)
            if len(indices) >= limit:
                break
    return indices


def previous_gameplay_capture_index_before(
    captures: list[dict[str, Any]], start: int
) -> int | None:
    for index in range(min(start, len(captures)) - 1, -1, -1):
        if capture_is_combat_like(captures[index]):
            return index
    return None


def capture_gameplay_image_reasons(
    capture: dict[str, Any],
    kind: str,
) -> tuple[list[str], dict[str, Any]]:
    reasons: list[str] = []
    if not capture_is_combat_like(capture):
        reasons.append("post_beast_not_gameplay")
    for reason in frame_reasons(capture.get("frame"), kind):
        if reason not in reasons:
            reasons.append(reason)
    image = frame_image_diagnostics(
        existing_png(capture.get("_png_path")),
        kind,
        capture.get("frame") if isinstance(capture.get("frame"), dict) else None,
    )
    if image["status"] == "fail":
        for reason in image["reasons"]:
            if reason not in reasons:
                reasons.append(reason)
    return reasons, image


def capture_post_beast_effect_reasons(
    post_capture: dict[str, Any],
    delta: dict[str, Any] | None = None,
) -> list[str]:
    reasons: list[str] = []
    for draw in iter_gpu_draw_commands(post_capture):
        if draw_source_hex(draw) == "0x0038b1b8":
            reasons.append("beast_effect_residual_draw:0x0038b1b8")
            break

    rec = find_first_key(post_capture, "native_otc_dma_recovery")
    if (
        isinstance(rec, dict)
        and rec.get("last_chain_model_selection_reason") == "selected_reused_current_otc"
        and isinstance(delta, dict)
        and delta.get("status") == "fail"
    ):
        reasons.append("beast_reused_current_otc_stuck_after_noop")
    return reasons


def live_beast_sequence_failure_reasons(checks: list[dict[str, Any]]) -> list[str]:
    reasons = unique_reasons(
        [
            reason
            for check in checks
            for reason in check.get("reasons", [])
            if isinstance(reason, str)
        ]
    )
    if (
        len(checks) > 1
        and all("beast_effect_stuck_frame" in check.get("reasons", []) for check in checks)
    ):
        reasons.append("beast_effect_persistent_stuck_sequence")
    if (
        len(checks) > 1
        and all(
            "beast_effect_residual_draw:0x0038b1b8" in check.get("reasons", [])
            for check in checks
        )
    ):
        reasons.append("beast_effect_persistent_residual_draw")
    return unique_reasons(reasons or ["post_beast_gameplay_capture_invalid"])


def live_beast_sequence_for_action(
    captures: list[dict[str, Any]],
    beast_capture_index: int,
) -> dict[str, Any]:
    beast_capture = captures[beast_capture_index]
    reference_index = beast_capture_index if capture_is_combat_like(beast_capture) else None
    if (
        reference_index is None
        and previous_gameplay_capture_index_before(captures, beast_capture_index) is None
    ):
        return {
            "status": "fail",
            "reasons": ["beast_gameplay_capture_missing"],
            "beast_capture_index": None,
            "beast_action_capture_index": beast_capture_index,
            "beast_json": None,
            "beast_gui_frame": None,
            "post_capture_index": None,
            "post_json": None,
            "post_gui_frame": None,
            "checked_post_capture_count": 0,
            "checks": [],
            "hard_failure": False,
        }
    candidate_indices = live_post_beast_candidate_indices(captures, beast_capture_index)
    checks: list[dict[str, Any]] = []

    if not candidate_indices:
        return {
            "status": "fail",
            "reasons": ["post_beast_gameplay_capture_missing"],
            "beast_capture_index": reference_index,
            "beast_action_capture_index": beast_capture_index,
            "beast_json": beast_capture.get("_json_path") if reference_index is not None else None,
            "beast_gui_frame": beast_capture.get("gui_frame") if reference_index is not None else None,
            "post_capture_index": None,
            "post_json": None,
            "post_gui_frame": None,
            "checked_post_capture_count": 0,
            "checks": checks,
            "hard_failure": False,
        }

    for candidate_index in candidate_indices:
        post_capture = captures[candidate_index]
        valid_reasons, image = capture_gameplay_image_reasons(post_capture, "beast")
        if reference_index is not None:
            effect_reasons, delta = live_capture_pair_beast_effect_reasons(
                captures[reference_index],
                post_capture,
            )
        else:
            delta = {"status": "not_checked", "reason": "beast_reference_not_gameplay"}
            effect_reasons = capture_post_beast_effect_reasons(post_capture)
        check_reasons = unique_reasons(valid_reasons + effect_reasons)
        check = {
            "post_capture_index": candidate_index,
            "post_json": post_capture.get("_json_path"),
            "post_gui_frame": post_capture.get("gui_frame"),
            "reasons": check_reasons,
            "delta": delta,
            "image": image,
        }
        checks.append(check)
        if not check_reasons:
            selected_beast_index = reference_index if reference_index is not None else candidate_index
            return {
                "status": "pass",
                "reasons": [],
                "beast_capture_index": selected_beast_index,
                "beast_action_capture_index": beast_capture_index,
                "beast_json": captures[selected_beast_index].get("_json_path")
                if selected_beast_index is not None
                else None,
                "beast_gui_frame": captures[selected_beast_index].get("gui_frame")
                if selected_beast_index is not None
                else None,
                "post_capture_index": candidate_index,
                "post_json": post_capture.get("_json_path"),
                "post_gui_frame": post_capture.get("gui_frame"),
                "checked_post_capture_count": len(checks),
                "candidate_post_capture_count": len(candidate_indices),
                "checks": checks,
                "delta": delta,
                "hard_failure": False,
            }

    return {
        "status": "fail",
        "reasons": live_beast_sequence_failure_reasons(checks),
        "beast_capture_index": reference_index,
        "beast_action_capture_index": beast_capture_index,
        "beast_json": beast_capture.get("_json_path") if reference_index is not None else None,
        "beast_gui_frame": beast_capture.get("gui_frame") if reference_index is not None else None,
        "post_capture_index": candidate_indices[0],
        "post_json": captures[candidate_indices[0]].get("_json_path"),
        "post_gui_frame": captures[candidate_indices[0]].get("gui_frame"),
        "checked_post_capture_count": len(checks),
        "candidate_post_capture_count": len(candidate_indices),
        "checks": checks,
        "delta": checks[-1].get("delta") if checks else None,
        "hard_failure": True,
    }


def live_performance_verdict(data: dict[str, Any]) -> dict[str, Any]:
    performance = data.get("performance") if isinstance(data.get("performance"), dict) else {}
    timing = data.get("timing") if isinstance(data.get("timing"), dict) else {}
    reasons: list[str] = []
    if not boolish(data.get("performance_verified")):
        reasons.append("performance_verified:false")
    if not performance:
        reasons.append("performance_missing")
    elif not boolish(performance.get("verified")):
        reasons.append("performance_gate_verified:false")
    for key in (
        "frame_samples_present",
        "attack_samples_present",
        "beast_samples_present",
        "frame_p95_within_budget",
        "attack_p95_within_budget",
        "beast_p95_within_budget",
        "frame_max_stall_within_budget",
        "attack_max_stall_within_budget",
        "beast_max_stall_within_budget",
        "frame_over_33_ms_ratio_within_budget",
        "attack_over_33_ms_ratio_within_budget",
        "beast_over_33_ms_ratio_within_budget",
        "no_missed_vblank_attempts",
    ):
        if key in performance and not boolish(performance.get(key)):
            required_key = key.replace("_samples_present", "_samples_required")
            if key.endswith("_samples_present") and performance.get(required_key) is False:
                continue
            reasons.append(f"{key}:false")
    if not timing:
        reasons.append("timing_missing")
    else:
        p95_budget = intish(performance.get("p95_budget_us"))
        max_stall_budget = intish(performance.get("max_stall_budget_us"))
        if p95_budget <= 0:
            reasons.append("performance_p95_budget_missing")
        if max_stall_budget <= 0:
            reasons.append("performance_max_stall_budget_missing")
        timing_requirements = (
            ("frame", True),
            ("attack_frame", boolish(performance.get("attack_samples_required"))),
            ("beast_frame", boolish(performance.get("beast_samples_required"))),
        )
        for key, required in timing_requirements:
            stats = timing.get(key) if isinstance(timing.get(key), dict) else {}
            if not stats:
                if required:
                    reasons.append(f"timing_{key}_missing")
                continue
            if required and intish(stats.get("samples")) <= 0:
                reasons.append(f"timing_{key}_samples:0")
            if p95_budget > 0 and intish(stats.get("p95_us")) > p95_budget:
                reasons.append(f"timing_{key}_p95_over_budget:{stats.get('p95_us')}")
            if max_stall_budget > 0 and intish(stats.get("max_us")) > max_stall_budget:
                reasons.append(f"timing_{key}_max_stall_over_budget:{stats.get('max_us')}")
    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "performance_verified": data.get("performance_verified"),
        "performance": performance or data.get("performance"),
        "timing": timing,
    }


def live_beast_action_is_gameplay_related(
    captures: list[dict[str, Any]],
    index: int,
) -> bool:
    if index < 0 or index >= len(captures):
        return False
    if capture_is_combat_like(captures[index]):
        return True
    previous_index = previous_gameplay_capture_index_before(captures, index)
    return previous_index is not None and previous_index + 1 == index


def live_beast_effect_verdict(captures: list[dict[str, Any]]) -> dict[str, Any]:
    all_beast_indices = [
        index
        for index, item in enumerate(captures)
        if capture_has_any_button(item, "beast", "p2_beast")
    ]
    beast_indices = [
        index
        for index in all_beast_indices
        if live_beast_action_is_gameplay_related(captures, index)
    ]
    gameplay_beast_indices = [
        index for index in beast_indices if capture_is_combat_like(captures[index])
    ]
    if not beast_indices:
        return {
            "status": "fail",
            "reasons": ["gameplay_beast_capture_missing"],
            "beast_capture_count": len(all_beast_indices),
            "gameplay_beast_capture_count": 0,
            "ignored_non_gameplay_beast_capture_count": len(all_beast_indices),
        }

    sequences = [
        live_beast_sequence_for_action(captures, beast_index)
        for beast_index in beast_indices
    ]
    hard_failures = [
        sequence
        for sequence in sequences
        if sequence.get("status") == "fail" and boolish(sequence.get("hard_failure"))
    ]
    if hard_failures:
        failure = hard_failures[0]
        return {
            **failure,
            "status": "fail",
            "beast_capture_count": len(all_beast_indices),
            "gameplay_beast_capture_count": len(gameplay_beast_indices),
            "ignored_non_gameplay_beast_capture_count": len(all_beast_indices)
            - len(beast_indices),
            "sequences": sequences,
        }

    passing = next(
        (sequence for sequence in sequences if sequence.get("status") == "pass"),
        None,
    )
    if passing is not None:
        return {
            **passing,
            "beast_capture_count": len(all_beast_indices),
            "gameplay_beast_capture_count": len(gameplay_beast_indices),
            "ignored_non_gameplay_beast_capture_count": len(all_beast_indices)
            - len(beast_indices),
            "sequences": sequences,
        }

    failure = sequences[0]
    reasons = unique_reasons(
        [
            reason
            for sequence in sequences
            for reason in sequence.get("reasons", [])
            if isinstance(reason, str)
        ]
        or ["beast_gameplay_capture_missing"]
    )
    return {
        **failure,
        "status": "fail",
        "reasons": reasons,
        "beast_capture_count": len(all_beast_indices),
        "gameplay_beast_capture_count": len(gameplay_beast_indices),
        "ignored_non_gameplay_beast_capture_count": len(all_beast_indices)
        - len(beast_indices),
        "sequences": sequences,
    }


def capture_image_artifact_diagnostics(capture: dict[str, Any]) -> dict[str, Any]:
    png = existing_png(capture.get("_png_path"))
    if not png:
        return {"status": "not_checked", "reason": "capture_png_missing"}
    try:
        from PIL import Image
    except ImportError:
        return {"status": "not_checked", "reason": "pillow_missing"}
    try:
        image = Image.open(png).convert("RGB")
    except (OSError, ValueError) as exc:
        return {"status": "fail", "reasons": [f"capture_image_unreadable:{exc}"], "png": png}

    full_roi = (0, 0, min(image.width, 512), min(image.height, 480))
    full = image_region_metrics(image, full_roi)
    top = image_region_metrics(image, (0, 0, min(image.width, 512), min(image.height, 240)))
    bottom = image_region_metrics(
        image, (0, min(image.height, 240), min(image.width, 512), min(image.height, 480))
    )
    tile_grid = repeating_tile_grid_metrics(image, full_roi)
    reasons: list[str] = []
    frame = capture.get("frame") if isinstance(capture.get("frame"), dict) else {}
    recognized_boot_title = (
        capture_is_title_like(capture)
        and boolish(get_path(frame, "title_screen_frame"))
        and boolish(get_path(frame, "bottom_caption_band"))
        and boolish(get_path(frame, "intro_caption_band"))
    )
    if (
        float(top.get("nonblack_ratio") or 0.0) > 0.10
        and float(bottom.get("nonblack_ratio") or 0.0) < 0.06
        and not recognized_boot_title
    ):
        reasons.append("capture_image_bottom_half_sparse")
    if (
        float(bottom.get("nonblack_ratio") or 0.0) > 0.10
        and float(top.get("nonblack_ratio") or 0.0) < 0.06
        and not recognized_boot_title
    ):
        reasons.append("capture_image_top_half_sparse")
    if float(full.get("edge_ratio") or 0.0) > CAPTURE_SEQUENCE_MAX_EDGE_RATIO:
        reasons.append("capture_image_noisy_texture_artifact")
    if float(full.get("dominant_quantized_ratio") or 0.0) > CAPTURE_SEQUENCE_MAX_DOMINANT_RATIO:
        reasons.append("capture_image_dominant_color_fill")
    if float(tile_grid.get("combined_similarity") or 0.0) >= REPEATING_TILE_MIN_SIMILARITY:
        reasons.append("capture_image_repeating_tile_grid")
    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "png": png,
        "gui_frame": capture.get("gui_frame"),
        "json": capture.get("_json_path"),
        "label": capture.get("label"),
        "full": full,
        "top_half": top,
        "bottom_half": bottom,
        "repeating_tile_grid": tile_grid,
    }


def capture_sequence_verdict(captures: list[dict[str, Any]]) -> dict[str, Any]:
    reasons: list[str] = []
    artifact_checks: list[dict[str, Any]] = []
    skipped_transition_frames: list[int] = []
    for capture in captures:
        if capture_is_non_render_transition(capture):
            skipped_transition_frames.append(intish(capture.get("gui_frame")))
            continue
        frame = capture.get("frame") if isinstance(capture.get("frame"), dict) else {}
        for flag in BLOCKING_FRAME_FLAGS:
            if boolish(frame.get(flag)):
                reasons.append(f"capture_{capture.get('gui_frame')}_{flag}")
        image = capture_image_artifact_diagnostics(capture)
        if image["status"] == "fail":
            artifact_checks.append(image)
            for reason in image["reasons"]:
                reasons.append(f"capture_{capture.get('gui_frame')}_{reason}")
    return {
        "status": "pass" if not reasons else "fail",
        "reasons": reasons,
        "artifact_checks": artifact_checks,
        "capture_count": len(captures),
        "skipped_transition_count": len(skipped_transition_frames),
        "skipped_transition_frames": skipped_transition_frames,
    }


def summarize_live(args: argparse.Namespace) -> dict[str, Any]:
    data, parse_error = load_last_json(args.live_stdout)
    captures = load_capture_jsons(args.live_capture_dir)
    result: dict[str, Any] = {
        "command": "native-play",
        "exit_code": args.live_status,
        "stdout": args.live_stdout,
        "stderr": args.live_stderr,
        "capture_dir": args.live_capture_dir,
        "parse_error": parse_error,
        "stderr_tail": tail_text(args.live_stderr),
        "capture_count": len(captures),
        "png_count": len(list(Path(args.live_capture_dir).glob("*.png")))
        if args.live_capture_dir and Path(args.live_capture_dir).exists()
        else 0,
    }
    if not isinstance(data, dict):
        result["status"] = "fail"
        result["failures"] = ["stdout_json_unavailable"]
        return result

    boot_index = first_capture_index(captures, capture_is_title_like)
    boot_capture = captures[boot_index] if boot_index is not None else None
    capture_search_start = 0 if boot_index is None else boot_index + 1
    p2_start_index = first_capture_index(
        captures,
        lambda item: capture_has_any_button(item, "p2_start"),
        capture_search_start,
    )
    select_index = first_capture_index(
        captures,
        capture_is_select_like,
        capture_search_start,
        p2_start_index,
    )
    if select_index is None:
        select_index = first_capture_index(
            captures,
            capture_is_select_candidate,
            capture_search_start,
            p2_start_index,
        )
    select_capture = captures[select_index] if select_index is not None else None
    p2_control_index = None
    p2_select_index = None
    if p2_start_index is not None:
        p2_control_index = first_capture_index(
            captures,
            lambda item: capture_has_any_button(
                item,
                "right",
                "down",
                "left",
                "up",
                "punch",
                "kick",
                "beast",
                "guard",
                "p2_right",
                "p2_down",
                "p2_left",
                "p2_up",
                "p2_punch",
                "p2_kick",
                "p2_beast",
                "p2_guard",
            ),
            p2_start_index + 1,
        )
        p2_select_index = first_capture_index(
            captures,
            capture_is_select_like,
            p2_start_index + 1,
            p2_control_index,
        )
        if p2_select_index is None:
            p2_select_index = first_capture_index(
                captures,
                capture_is_select_candidate,
                p2_start_index + 1,
                p2_control_index,
            )
    p2_select_capture = (
        captures[p2_select_index] if p2_select_index is not None else None
    )
    combat_index = first_capture_index(
        captures,
        lambda item: capture_is_combat_like(item)
        and not capture_has_any_button(item, "beast", "p2_beast"),
        0 if select_index is None else select_index + 1,
    )
    combat_capture = captures[combat_index] if combat_index is not None else None
    beast_effect_summary = live_beast_effect_verdict(captures)
    selected_beast_capture_index = beast_effect_summary.get("post_capture_index")
    if not isinstance(selected_beast_capture_index, int):
        selected_beast_capture_index = beast_effect_summary.get("beast_capture_index")
    if (
        isinstance(selected_beast_capture_index, int)
        and 0 <= selected_beast_capture_index < len(captures)
    ):
        beast_capture = captures[selected_beast_capture_index]
    else:
        beast_capture = first_capture_after(
            captures,
            lambda item: boolish(get_path(item, "buttons", "beast"))
            or boolish(get_path(item, "buttons", "p2_beast")),
            0 if combat_index is None else combat_index + 1,
        )
    final_capture = capture_by_predicate(captures, lambda item: item.get("label") == "final")
    if final_capture is None and captures:
        final_capture = captures[-1]

    stage_items = {
        "boot": summarize_capture(boot_capture, "boot", {"initial", "boot", "title"}),
        "select": summarize_capture(select_capture, "select"),
        "p2_select": summarize_capture(p2_select_capture, "select"),
        "combat": summarize_capture(combat_capture, "combat"),
        "beast": summarize_capture(beast_capture, "beast"),
        "final": {
            **summarize_frame(data.get("final_frame"), "final"),
            "png": existing_png(get_path(final_capture, "_png_path")),
            "json": get_path(final_capture, "_json_path"),
        },
    }
    attach_stage_image_diagnostics(stage_items["boot"], "boot")
    attach_stage_image_diagnostics(stage_items["select"], "select")
    attach_select_portrait_image_diagnostics(stage_items["select"], stage_items["boot"])
    attach_stage_image_diagnostics(stage_items["p2_select"], "select")
    attach_select_portrait_image_diagnostics(
        stage_items["p2_select"],
        stage_items["boot"],
        portrait_roi=SELECT_RIGHT_LARGE_PORTRAIT_ROI,
    )
    attach_stage_image_diagnostics(stage_items["combat"], "combat")
    attach_stage_image_diagnostics(stage_items["beast"], "beast")
    attach_stage_image_diagnostics(stage_items["final"], "final")

    rec = find_first_key(data, "native_otc_dma_recovery")
    capture_recoveries = [
        find_first_key(capture, "native_otc_dma_recovery") for capture in captures
    ]
    recoveries = [item for item in capture_recoveries if isinstance(item, dict)]
    if isinstance(rec, dict):
        recoveries.append(rec)
    recovery_summary = recovery_verdict(recoveries)

    input_summary = live_input_verdict(data, captures)
    audio_summary = live_audio_verdict(data)
    performance_summary = live_performance_verdict(data)
    capture_sequence_summary = capture_sequence_verdict(captures)
    performance = data.get("performance") if isinstance(data.get("performance"), dict) else {}
    watchdog = data.get("gui_watchdog") if isinstance(data.get("gui_watchdog"), dict) else {}

    failures: list[str] = []
    warnings: list[str] = []
    if args.live_status != 0:
        failures.append(f"command_exit_code:{args.live_status}")
    for key in (
        "playable",
        "gameplay_ready",
        "render_ready",
        "native_play_test_input_verified",
        "audio_realtime_verified",
        "performance_verified",
        "final_frame_full_size",
        "final_frame_visible_content",
        "final_frame_scene_detail",
        "final_frame_render_ready",
        "final_frame_gameplay_scene",
        "final_frame_render_verified",
    ):
        if not boolish(data.get(key)):
            failures.append(f"{key}:false")
    if int(watchdog.get("missed_vblank_attempts") or 0) != 0:
        failures.append(f"gui_missed_vblank_attempts:{watchdog.get('missed_vblank_attempts')}")
    if watchdog.get("stop_reason") not in (None, "max_frames"):
        failures.append(f"gui_stop_reason:{watchdog.get('stop_reason')}")
    if get_path(data, "final_frame", "blocking_display_artifact"):
        failures.append("final_frame_blocking_display_artifact")
    for name, stage in stage_items.items():
        if stage["status"] == "fail":
            failures.append(f"{name}_frame:{','.join(stage['reasons'])}")
    if input_summary["status"] != "pass":
        failures.append("live_input_not_verified")
    if audio_summary["status"] != "pass":
        failures.append("live_audio_not_verified:" + ",".join(audio_summary["reasons"]))
    if performance_summary["status"] != "pass":
        failures.append(
            "live_performance_not_verified:" + ",".join(performance_summary["reasons"])
        )
    if beast_effect_summary["status"] != "pass":
        failures.append("live_beast_effect:" + ",".join(beast_effect_summary["reasons"]))
    if capture_sequence_summary["status"] != "pass":
        failures.append(
            "live_capture_sequence:" + ",".join(capture_sequence_summary["reasons"][:12])
        )
    if recovery_summary["status"] == "fail":
        failures.append("recovery_bad_reason")
    elif recovery_summary["status"] == "warn":
        warnings.append("recovery_warn_reason")
    if not captures:
        failures.append("no_gui_capture_json")

    result.update(
        {
            "status": status_from_failures(failures, warnings),
            "failures": failures,
            "warnings": warnings,
            "rendered_frames": data.get("rendered_frames"),
            "gui_rendered_frames": data.get("gui_rendered_frames"),
            "gui_watchdog": watchdog,
            "performance": performance_summary,
            "stages": stage_items,
            "input": input_summary,
            "audio": audio_summary,
            "beast_effect": beast_effect_summary,
            "capture_sequence": capture_sequence_summary,
            "recovery": recovery_summary,
        }
    )
    return result


def write_text_summary(summary: dict[str, Any], path: Path) -> None:
    lines = [
        f"BR2 macOS E2E QA: {summary['status'].upper()}",
        f"mode: {summary['mode']}",
        f"out_dir: {summary['out_dir']}",
    ]
    for name in ("smoke", "live"):
        section = summary.get(name)
        if not isinstance(section, dict):
            continue
        lines.append("")
        lines.append(f"{name}: {section.get('status', 'skip').upper()}")
        lines.append(f"  command: {section.get('command')} exit={section.get('exit_code')}")
        if section.get("failures"):
            lines.append("  failures:")
            for failure in section["failures"][:20]:
                lines.append(f"    - {failure}")
        if section.get("warnings"):
            lines.append("  warnings:")
            for warning in section["warnings"][:20]:
                lines.append(f"    - {warning}")
        for stage_name, stage in (section.get("stages") or {}).items():
            lines.append(
                "  "
                + f"{stage_name}: {stage.get('status')} png={stage.get('png')} "
                + f"reasons={','.join(stage.get('reasons') or []) or 'none'}"
            )
        if isinstance(section.get("input"), dict):
            lines.append(f"  input: {section['input'].get('status')}")
        if isinstance(section.get("audio"), dict):
            lines.append(f"  audio: {section['audio'].get('status')}")
        if isinstance(section.get("performance"), dict):
            lines.append(f"  performance: {section['performance'].get('status')}")
        if isinstance(section.get("beast_effect"), dict):
            lines.append(f"  beast_effect: {section['beast_effect'].get('status')}")
        if isinstance(section.get("recovery"), dict):
            final = section["recovery"].get("final") or {}
            lines.append(
                "  recovery: "
                + f"{section['recovery'].get('status')} "
                + f"last_reason={final.get('last_reason')} "
                + f"model_reason={final.get('last_chain_model_selection_reason')}"
            )
        select_preview = get_path(section, "stages", "select", "select_preview", default={})
        if isinstance(select_preview, dict) and select_preview.get("candidate_count"):
            lines.append(
                "  select_preview: "
                + f"status={select_preview.get('status')} "
                + f"source={select_preview.get('source_address_hex')} "
                + f"drawn={select_preview.get('drawn_pixels')} "
                + f"clut_blank_ratio={select_preview.get('clut_blank_ratio')} "
                + f"palette_fallback_ratio={select_preview.get('palette_fallback_ratio')}"
            )
    lines.append("")
    lines.append(
        "cleanup: "
        + f"{summary['cleanup']['policy']} performed={summary['cleanup']['performed']} "
        + f"artifacts_retained={summary['cleanup']['artifacts_retained']}"
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def safe_cleanup_artifacts(artifact_root: Path, out_dir: Path) -> bool:
    try:
        artifact_root = artifact_root.resolve()
        out_dir = out_dir.resolve()
    except OSError:
        return False
    if artifact_root.name != "artifacts":
        return False
    if out_dir not in artifact_root.parents:
        return False
    if not artifact_root.exists():
        return False
    shutil.rmtree(artifact_root)
    return True


def main() -> int:
    args = parse_args()
    out_dir = Path(args.out_dir)
    artifact_root = Path(args.artifact_root)

    summary: dict[str, Any] = {
        "mode": args.mode,
        "status": "pass",
        "out_dir": str(out_dir),
        "commands_log": args.commands_log,
        "build_status": args.build_status,
        "cleanup": {
            "policy": args.cleanup,
            "performed": False,
            "artifact_root": str(artifact_root),
            "artifacts_retained": True,
        },
    }

    failures: list[str] = []
    warnings: list[str] = []
    if args.build_status != 0:
        failures.append(f"build_exit_code:{args.build_status}")

    if args.mode in ("smoke", "all") and args.build_status == 0:
        summary["smoke"] = summarize_smoke(args)
        if summary["smoke"]["status"] == "fail":
            failures.append("smoke_failed")
        elif summary["smoke"]["status"] == "warn":
            warnings.append("smoke_warn")
    if args.mode in ("live", "all") and args.build_status == 0:
        summary["live"] = summarize_live(args)
        if summary["live"]["status"] == "fail":
            failures.append("live_failed")
        elif summary["live"]["status"] == "warn":
            warnings.append("live_warn")

    summary["status"] = status_from_failures(failures, warnings)
    summary["failures"] = failures
    summary["warnings"] = warnings

    should_cleanup = args.cleanup == "always" or (
        args.cleanup == "on-pass" and summary["status"] == "pass"
    )
    if should_cleanup:
        performed = safe_cleanup_artifacts(artifact_root, out_dir)
        summary["cleanup"]["performed"] = performed
        summary["cleanup"]["artifacts_retained"] = not performed

    out_dir.mkdir(parents=True, exist_ok=True)
    summary_json = out_dir / "qa-summary.json"
    summary_txt = out_dir / "qa-summary.txt"
    summary_json.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    write_text_summary(summary, summary_txt)
    print(summary_txt.read_text(encoding="utf-8"), end="")
    print(f"summary_json: {summary_json}")
    return 0 if summary["status"] in {"pass", "warn"} else 1


if __name__ == "__main__":
    sys.exit(main())
