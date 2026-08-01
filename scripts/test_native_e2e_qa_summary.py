#!/usr/bin/env python3

from __future__ import annotations

import argparse
import importlib.util
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from PIL import Image


MODULE_PATH = Path(__file__).with_name("native_e2e_qa_summary.py")
SPEC = importlib.util.spec_from_file_location("native_e2e_qa_summary", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
summary = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(summary)
EVIDENCE_DIR = Path("tmp/native-e2e-failure-evidence-20260731")


class NativeE2EQASummaryTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.artifact_dir = self.root / "artifacts" / "smoke"
        self.artifact_dir.mkdir(parents=True)
        self.png = self.artifact_dir / "timeline.tail-04-noop.window.png"
        self._write_select_png(large_portrait=True)
        self.combat_png = self.artifact_dir / "timeline.tail-05-punch.window.png"
        self.beast_png = self.artifact_dir / "timeline.tail-06-beast.window.png"
        self.post_beast_png = self.artifact_dir / "timeline.tail-07-noop.window.png"
        self._write_combat_png(self.combat_png, variant=0)
        self._write_combat_png(self.beast_png, variant=1)
        self._write_combat_png(self.post_beast_png, variant=2)
        self.stderr = self.artifact_dir / "native-match-tail-timeline.stderr.log"
        self.stderr.write_text("", encoding="utf-8")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _frame(self, *, gameplay: bool = False) -> dict[str, object]:
        return {
            "width": 512,
            "height": 480,
            "visible_content": True,
            "unique_colors": 257,
            "occupied_row_span": 480,
            "scene_detail": True,
            "gameplay_scene": gameplay,
            "render_ready_scene": True,
            "blocking_display_artifact": False,
            "missing_texture_recovery_artifact": False,
        }

    def _title_frame(self) -> dict[str, object]:
        frame = self._frame()
        frame.update(
            {
                "title_screen_frame": True,
                "bottom_caption_band": True,
                "intro_caption_band": True,
            }
        )
        return frame

    def _activity(self) -> dict[str, object]:
        return {
            "system_coin_active_reads": 1,
            "system_start_active_reads": 1,
            "p1_up_active_reads": 1,
            "p1_down_active_reads": 1,
            "p1_left_active_reads": 1,
            "p1_right_active_reads": 1,
            "p1_punch_active_reads": 1,
            "p1_kick_active_reads": 1,
            "p1_beast_active_reads": 1,
            "p3_input_reads": 0,
            "system_p2_coin_active_reads": 1,
            "system_p2_start_active_reads": 1,
            "p2_up_active_reads": 1,
            "p2_down_active_reads": 1,
            "p2_left_active_reads": 1,
            "p2_right_active_reads": 1,
            "p2_punch_active_reads": 1,
            "p2_kick_active_reads": 1,
            "p2_beast_active_reads": 1,
            "p2_guard_active_reads": 1,
        }

    def _no_model_recovery(self) -> dict[str, object]:
        return {
            "last_reason": "submitted",
            "last_chain_model_selection_reason": "no_model_draws",
            "last_chain_model_texture_draws": 0,
        }

    def _write_select_png(
        self,
        *,
        large_portrait: bool,
        right_large_portrait: bool = False,
        right_name_atlas: bool = False,
    ) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))

        # Character-select backdrop with sparse blue stripes in the large
        # portrait area. This alone must not satisfy the portrait gate.
        for y in range(0, 120):
            for x in range(512):
                image.putpixel((x, y), ((x * 5 + y * 3) % 80, 0, (x + y) % 24))
        for x in range(30, 240, 16):
            for y in range(120, 316):
                image.putpixel((x, y), (0, 0, 72 + (x * 3 + y) % 96))

        # Bottom roster tiles are deliberately colorful. The QA must not treat
        # this band as the large selected-character portrait.
        colors = [
            (230, 60, 40),
            (30, 180, 220),
            (240, 220, 40),
            (80, 220, 80),
            (220, 80, 180),
            (210, 170, 120),
        ]
        for index, x0 in enumerate(range(80, 420, 58)):
            color = colors[index % len(colors)]
            for y in range(322, 388):
                for x in range(x0, min(x0 + 48, 512)):
                    image.putpixel(
                        (x, y),
                        (
                            (color[0] + x * 3 + y) % 256,
                            (color[1] + x + y * 5) % 256,
                            (color[2] + x * 7 + y * 2) % 256,
                        ),
                    )

        if large_portrait:
            for y in range(126, 316):
                for x in range(24, 238):
                    cx = (x - 131) / 86.0
                    cy = (y - 221) / 92.0
                    if cx * cx + cy * cy > 1.0:
                        continue
                    image.putpixel(
                        (x, y),
                        (
                            48 + (x * 7 + y * 3) % 176,
                            40 + (x * 5 + y * 11) % 152,
                            48 + (x * 13 + y * 2) % 168,
                        ),
                    )
            for x in range(52, 210, 12):
                for y in range(140, 300):
                    image.putpixel((x, y), (245, 245, 230))
            for y in range(148, 296, 11):
                for x in range(50, 216):
                    image.putpixel((x, y), (20, 20, 20))

        if right_large_portrait:
            for y in range(126, 316):
                for x in range(274, 488):
                    cx = (x - 381) / 86.0
                    cy = (y - 221) / 92.0
                    if cx * cx + cy * cy > 1.0:
                        continue
                    image.putpixel(
                        (x, y),
                        (
                            48 + (x * 11 + y * 5) % 168,
                            40 + (x * 3 + y * 13) % 160,
                            48 + (x * 7 + y * 2) % 176,
                        ),
                    )
            for x in range(302, 460, 12):
                for y in range(140, 300):
                    image.putpixel((x, y), (240, 232, 215))
            for y in range(148, 296, 11):
                for x in range(300, 466):
                    image.putpixel((x, y), (24, 24, 24))

        if right_name_atlas:
            left, top, right, bottom = summary.SELECT_RIGHT_LARGE_PORTRAIT_ROI
            for y in range(top, bottom):
                for x in range(left, right):
                    tile_x = (x - left) % 16
                    tile_y = (y - top) % 16
                    ink = tile_x in {2, 3, 8, 9} or tile_y in {4, 5, 12}
                    image.putpixel(
                        (x, y),
                        (235, 180, 72) if ink else (18, 38, 72),
                    )

        image.save(self.png)

    def _write_combat_png(self, path: Path, *, variant: int = 0) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))
        for y in range(480):
            for x in range(512):
                sky = y < 250
                base = 70 if sky else 95
                red = (base + x * 3 + y * 2 + variant * 17) % 256
                green = (80 + x * 5 + y * 7 + variant * 23) % 256
                blue = (110 + x * 11 + y * 3 + variant * 31) % 256
                image.putpixel((x, y), (red, green, blue))
        for x0, color in ((130, (230, 185, 125)), (330, (95, 160, 220))):
            for y in range(150, 350):
                for x in range(x0, x0 + 58):
                    cx = (x - x0 - 29) / 29.0
                    cy = (y - 250) / 94.0
                    if cx * cx + cy * cy <= 1.0:
                        image.putpixel(
                            (x, y),
                            (
                                (color[0] + y + variant * 9) % 256,
                                (color[1] + x + variant * 11) % 256,
                                (color[2] + x * 2 + y + variant * 13) % 256,
                            ),
                        )
        image.save(path)

    def _write_half_title_png(self, path: Path) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))
        for y in range(0, 180):
            for x in range(512):
                image.putpixel((x, y), ((x * 5) % 180, 20 + (y * 3) % 160, 40))
        image.save(path)

    def _write_title_with_caption_png(self, path: Path) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))
        for y in range(0, 240):
            for x in range(512):
                image.putpixel(
                    (x, y),
                    (
                        40 + (x * 5 + y * 3) % 200,
                        20 + (x * 2 + y * 7) % 120,
                        16 + (x * 11 + y) % 96,
                    ),
                )
        for y in range(360, 416):
            for x in range(96, 416):
                if (x * 7 + y * 11) % 19 < 3:
                    image.putpixel((x, y), (240, 224, 96))
        image.save(path)

    def _write_title_logo_reuse_select_png(self, title_path: Path) -> None:
        self._write_select_png(large_portrait=True)
        select = Image.open(self.png).convert("RGB")
        title = Image.open(title_path).convert("RGB")
        left, top, right, bottom = summary.SELECT_LARGE_PORTRAIT_ROI
        source_top = summary.TITLE_LOGO_REUSE_MIN_SOURCE_TOP
        patch = title.crop(
            (0, source_top, right - left, source_top + bottom - top)
        )
        select.paste(patch, (left, top))
        select.save(self.png)

    def _write_low_color_stage_png(self, path: Path) -> None:
        image = Image.new("RGB", (512, 480), (190, 42, 34))
        for y in range(72, 430):
            for x in range(512):
                if x % 128 < 8 and y % 128 < 8:
                    image.putpixel((x, y), (224, 118, 80))
        image.save(path)

    def _write_noisy_select_png(self, path: Path) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))
        for block_y in range(0, 480, 2):
            for block_x in range(0, 512, 2):
                color = (
                    (block_x * 17 + block_y * 41) % 256,
                    (block_x * 53 + block_y * 29) % 256,
                    (block_x * 97 + block_y * 13) % 256,
                )
                for y in range(block_y, min(block_y + 2, 480)):
                    for x in range(block_x, min(block_x + 2, 512)):
                        image.putpixel((x, y), color)
        image.save(path)

    def _write_repeating_tile_png(self, path: Path) -> None:
        image = Image.new("RGB", (512, 480), (0, 0, 0))
        tile: dict[tuple[int, int], tuple[int, int, int]] = {}
        for y in range(32):
            for x in range(32):
                tile[(x, y)] = (
                    (40 + x * 7 + y * 11) % 256,
                    (80 + x * 13 + y * 5) % 256,
                    (120 + x * 3 + y * 17) % 256,
                )
        for y in range(480):
            for x in range(512):
                image.putpixel((x, y), tile[(x % 32, y % 32)])
        image.save(path)

    def _evidence_png(self, name: str) -> Path:
        path = EVIDENCE_DIR / name
        if not path.exists():
            self.skipTest(f"failure evidence image is not present: {path}")
        return path

    def _assert_evidence_fails_gameplay_image_gate(
        self,
        name: str,
        reason_suffix: str,
    ) -> None:
        path = self._evidence_png(name)
        for kind in ("combat", "beast", "final"):
            with self.subTest(name=name, kind=kind):
                diagnostics = summary.frame_image_diagnostics(
                    str(path),
                    kind,
                    self._frame(gameplay=True),
                )
                self.assertEqual(diagnostics["status"], "fail")
                self.assertIn(f"{kind}_image_{reason_suffix}", diagnostics["reasons"])

    def _full_button_set(self) -> dict[str, bool]:
        return {key: True for key in summary.LIVE_REQUIRED_BUTTON_FIELDS}

    def _live_gui_input_verification(self) -> dict[str, object]:
        return {
            **{key: True for key in summary.LIVE_REQUIRED_GUI_INPUT_FLAGS},
            "required_buttons": self._full_button_set(),
            "guest_buttons": self._full_button_set(),
        }

    def _live_audio(self, *, sfx: bool = True) -> dict[str, object]:
        key_on_events = 5 if sfx else 0
        latch_nonzero_writes = 4 if sfx else 0
        return {
            "available": True,
            "generated_frames": 4096,
            "audio_render_batches": 8,
            "audio_queue_push_batches": 8,
            "latch_nonzero_writes": latch_nonzero_writes,
            "coreaudio_start_attempts": 1,
            "coreaudio_started": True,
            "ymf271": {
                "key_on_events": key_on_events,
                "generated_frames": 4096,
                "nonzero_frames": 4096,
                "last_rms_left": 12,
                "last_rms_right": 11,
            },
            "queue": {
                "coreaudio_started": True,
                "coreaudio_running": True,
                "coreaudio_callback_output_frames": 2048,
            },
        }

    def _live_audio_health(self) -> dict[str, object]:
        return {
            "state": "active",
            "reason": "pcm_nonzero_and_realtime_queue_healthy",
            "audible": True,
            "render_progressing": True,
            "pcm_nonzero": True,
            "realtime_output_seen": True,
            "realtime_ok": True,
        }

    def _live_performance(self) -> dict[str, object]:
        return {
            "verified": True,
            "frame_samples_present": True,
            "attack_samples_required": True,
            "attack_samples_present": True,
            "beast_samples_required": True,
            "beast_samples_present": True,
            "p95_budget_us": 20_000,
            "max_stall_budget_us": 55_000,
            "frame_p95_within_budget": True,
            "attack_p95_within_budget": True,
            "beast_p95_within_budget": True,
            "frame_max_stall_within_budget": True,
            "attack_max_stall_within_budget": True,
            "beast_max_stall_within_budget": True,
            "frame_over_33_ms_ratio_within_budget": True,
            "attack_over_33_ms_ratio_within_budget": True,
            "beast_over_33_ms_ratio_within_budget": True,
            "no_missed_vblank_attempts": True,
        }

    def _live_timing(self, *, frame_p95_us: int = 12_000) -> dict[str, object]:
        return {
            "frame": {
                "samples": 240,
                "p95_us": frame_p95_us,
                "p99_us": frame_p95_us + 1_000,
                "max_us": 24_000,
            },
            "attack_frame": {
                "samples": 20,
                "p95_us": 13_000,
                "p99_us": 15_000,
                "max_us": 25_000,
            },
            "beast_frame": {
                "samples": 12,
                "p95_us": 14_000,
                "p99_us": 16_000,
                "max_us": 26_000,
            },
        }

    def _live_data(self) -> dict[str, object]:
        return {
            "playable": True,
            "gameplay_ready": True,
            "render_ready": True,
            "native_play_test_input_verified": True,
            "audio_realtime_verified": True,
            "performance_verified": True,
            "final_frame_full_size": True,
            "final_frame_visible_content": True,
            "final_frame_scene_detail": True,
            "final_frame_render_ready": True,
            "final_frame_gameplay_scene": True,
            "final_frame_render_verified": True,
            "final_frame": self._frame(gameplay=True),
            "gui_watchdog": {"missed_vblank_attempts": 0, "stop_reason": "max_frames"},
            "gui_input_verification": self._live_gui_input_verification(),
            "test_input_activity": self._activity(),
            "input_activity": self._activity(),
            "audio_health": self._live_audio_health(),
            "audio": self._live_audio(),
            "performance": self._live_performance(),
            "timing": self._live_timing(),
        }

    def _write_capture(
        self,
        capture_dir: Path,
        gui_frame: int,
        label: str,
        *,
        frame: dict[str, object],
        buttons: dict[str, object] | None = None,
        image: str = "combat",
        variant: int = 0,
        native_sync: dict[str, object] | None = None,
    ) -> Path:
        json_path = capture_dir / f"{gui_frame:06}-{label}.json"
        png_path = json_path.with_suffix(".png")
        if image == "title":
            self._write_title_with_caption_png(png_path)
        elif image == "select":
            self._write_select_png(large_portrait=True)
            Image.open(self.png).save(png_path)
        elif image == "blank_select":
            self._write_select_png(large_portrait=False)
            Image.open(self.png).save(png_path)
        elif image == "p2_select":
            self._write_select_png(large_portrait=True, right_large_portrait=True)
            Image.open(self.png).save(png_path)
        elif image == "blank_p2_select":
            self._write_select_png(large_portrait=True)
            Image.open(self.png).save(png_path)
        elif image == "p2_name_atlas":
            self._write_select_png(large_portrait=True, right_name_atlas=True)
            Image.open(self.png).save(png_path)
        elif image == "half_title":
            self._write_half_title_png(png_path)
        elif image == "noisy_select":
            self._write_noisy_select_png(png_path)
        elif image == "tile_stage":
            self._write_repeating_tile_png(png_path)
        else:
            self._write_combat_png(png_path, variant=variant)
        payload: dict[str, object] = {
            "label": label,
            "gui_frame": gui_frame,
            "buttons": buttons or {},
            "frame": frame,
            "input_activity": self._activity(),
        }
        if native_sync is not None:
            payload["native_sync"] = native_sync
        json_path.write_text(json.dumps(payload), encoding="utf-8")
        return json_path

    def _write_live_capture_set(
        self,
        *,
        half_title: bool = False,
        blank_select: bool = False,
        blank_p2_select: bool = False,
        p2_name_atlas: bool = False,
        stuck_beast: bool = False,
        recover_after_stuck: bool = False,
        beast_transition: bool = False,
    ) -> Path:
        capture_dir = self.artifact_dir / "captures"
        capture_dir.mkdir()
        boot_frame = self._frame()
        boot_frame.update(
            {
                "title_screen_frame": True,
                "bottom_caption_band": True,
                "intro_caption_band": True,
            }
        )
        self._write_capture(
            capture_dir,
            1,
            "initial",
            frame=boot_frame,
            image="half_title" if half_title else "title",
        )
        self._write_capture(
            capture_dir,
            60,
            "presented",
            frame=self._frame(),
            buttons={"start": True},
            image="blank_select" if blank_select else "select",
        )
        self._write_capture(
            capture_dir,
            120,
            "presented",
            frame=self._frame(gameplay=True),
            buttons={"punch": True},
            image="combat",
            variant=0,
        )
        self._write_capture(
            capture_dir,
            130,
            "presented",
            frame=self._frame(gameplay=True),
            buttons={"p2_start": True},
            image="combat",
            variant=0,
        )
        self._write_capture(
            capture_dir,
            145,
            "presented",
            frame=self._frame(),
            buttons={},
            image=(
                "blank_p2_select"
                if blank_p2_select
                else "p2_name_atlas"
                if p2_name_atlas
                else "p2_select"
            ),
        )
        self._write_capture(
            capture_dir,
            150,
            "presented",
            frame=self._frame(gameplay=True),
            buttons={},
            image="combat",
            variant=0,
        )
        beast_sync = None
        if stuck_beast:
            beast_sync = {
                "native_otc_dma_recovery": {
                    "last_reason": "submitted",
                    "last_chain_model_selection_reason": "selected_reused_current_otc",
                }
            }
        self._write_capture(
            capture_dir,
            160,
            "presented",
            frame=self._frame(gameplay=not beast_transition),
            buttons={"beast": True},
            image="select" if beast_transition else "combat",
            variant=1,
        )
        self._write_capture(
            capture_dir,
            180,
            "presented",
            frame=self._frame(gameplay=True),
            buttons={},
            image="combat",
            variant=1 if stuck_beast else 2,
            native_sync=beast_sync,
        )
        self._write_capture(
            capture_dir,
            220,
            "final",
            frame=self._frame(gameplay=True),
            buttons={},
            image="combat",
            variant=2 if not stuck_beast or recover_after_stuck else 1,
        )
        return capture_dir

    def _summarize_live_data(
        self,
        data: dict[str, object],
        capture_dir: Path,
    ) -> dict[str, object]:
        stdout = self.artifact_dir / "native-play.stdout.json"
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            live_status=0,
            live_stdout=str(stdout),
            live_stderr=str(self.stderr),
            live_capture_dir=str(capture_dir),
        )
        return summary.summarize_live(args)

    def _preview_draw(
        self,
        *,
        clut_blank_samples: int = 0,
        palette_fallback_samples: int = 0,
        color_hash_hex: str = "0x12345678",
    ) -> dict[str, object]:
        return {
            "kind": "textured_quad",
            "source": {"address_hex": "0x003910fc"},
            "texture_page_hex": "0x0288",
            "clut_hex": "0x7d40",
            "bounds": {"left": 33, "top": 113, "right": 224, "bottom": 368},
            "sampled_pixels": 48_705,
            "drawn_pixels": 43_127,
            "written_pixels": 43_127,
            "texture_nonzero_samples": 43_284,
            "clut_blank_samples": clut_blank_samples,
            "palette_fallback_samples": palette_fallback_samples,
            "color_changes": 30_989,
            "color_hash_hex": color_hash_hex,
        }

    def _roster_tile_draw(self) -> dict[str, object]:
        return {
            "kind": "textured_quad",
            "source": {"address_hex": "0x003911c4"},
            "texture_page_hex": "0x0218",
            "clut_hex": "0x781c",
            "bounds": {"left": 0, "top": 293, "right": 256, "bottom": 484},
            "sampled_pixels": 47_872,
            "drawn_pixels": 39_645,
            "written_pixels": 39_645,
            "texture_nonzero_samples": 39_645,
            "clut_blank_samples": 0,
            "palette_fallback_samples": 0,
            "color_changes": 16_822,
            "color_hash_hex": "0x665adfb7",
        }

    def _snapshot(
        self,
        tail_index: int,
        *,
        action: str = "noop",
        gameplay: bool = False,
        recovery: dict[str, object] | None = None,
        draw: dict[str, object] | None = None,
    ) -> dict[str, object]:
        snap: dict[str, object] = {
            "tail_index": tail_index,
            "action": action,
            "window_output": str(self.combat_png if gameplay else self.png),
            "window_frame": self._frame(gameplay=gameplay),
        }
        if action == "beast":
            snap["window_output"] = str(self.beast_png)
        if recovery is not None:
            snap["native_sync"] = {"native_otc_dma_recovery": recovery}
        if draw is not None:
            snap["gpu"] = {"gpu_recent_draw_commands": [draw]}
        return snap

    def _summarize(
        self,
        select_snapshot: dict[str, object],
        *,
        boot_png: Path | None = None,
        boot_frame: dict[str, object] | None = None,
    ) -> dict[str, object]:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        data = {
            "boot": {
                "window_output": str(boot_png or self.png),
                "window_frame": boot_frame or self._frame(),
            },
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                select_snapshot,
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                self._snapshot(7, action="noop", gameplay=True),
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 120,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )
        return summary.summarize_smoke(args)

    def test_select_preview_draw_suppresses_no_model_draws_false_positive(self) -> None:
        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._preview_draw(),
            )
        )

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "pass")
        self.assertNotIn("select_model_preview_no_texture_draws", select["reasons"])
        self.assertEqual(select["select_preview"]["drawn_pixels"], 43_127)
        self.assertEqual(select["select_preview"]["palette_fallback_ratio"], 0.0)
        self.assertEqual(
            select["select_preview"]["large_portrait_image"]["status"], "pass"
        )
        self.assertTrue(select["select_preview"]["gp0_evidence"]["valid"])

    def test_recovery_transient_warning_passes_after_final_model_selection(self) -> None:
        result = summary.recovery_verdict(
            [
                {
                    "last_reason": "preserved_sparse_recovered_presentation",
                    "last_chain_model_selection_reason": "no_model_draws",
                    "last_chain_model_texture_draws": 0,
                    "last_chain_model_packets": 0,
                },
                {
                    "last_reason": "submitted_chain_model_replacement",
                    "last_chain_model_selection_reason": "selected",
                    "last_chain_model_texture_draws": 69,
                    "last_chain_model_packets": 32,
                },
            ]
        )

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["warn_reason_counts"], {})
        self.assertEqual(
            result["transient_warn_reason_counts"],
            {
                "preserved_sparse_recovered_presentation": 1,
                "no_model_draws": 1,
            },
        )

    def test_recovery_final_missing_model_remains_warning(self) -> None:
        result = summary.recovery_verdict(
            [
                {
                    "last_reason": "submitted",
                    "last_chain_model_selection_reason": "no_model_draws",
                    "last_chain_model_texture_draws": 0,
                    "last_chain_model_packets": 0,
                }
            ]
        )

        self.assertEqual(result["status"], "warn")
        self.assertEqual(result["warn_reason_counts"], {"no_model_draws": 1})
        self.assertEqual(result["transient_warn_reason_counts"], {})

    def test_select_portrait_rejects_title_logo_reuse_from_boot(self) -> None:
        title = self.artifact_dir / "title-logo.window.png"
        self._write_title_with_caption_png(title)
        self._write_title_logo_reuse_select_png(title)

        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._preview_draw(),
            ),
            boot_png=title,
            boot_frame=self._title_frame(),
        )

        select = result["stages"]["select"]
        reuse = select["large_portrait_image"]["title_logo_reuse"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(select["status"], "fail")
        self.assertIn("select_large_portrait_title_logo_reuse", select["reasons"])
        self.assertEqual(reuse["status"], "fail")
        self.assertGreaterEqual(
            reuse["best_match"]["luma_correlation"],
            summary.TITLE_LOGO_REUSE_MIN_LUMA_CORRELATION,
        )
        self.assertIn(
            "select_large_portrait_title_logo_reuse",
            select["select_preview"]["reasons"],
        )

    def test_select_portrait_allows_non_reused_portrait_with_title_boot(self) -> None:
        title = self.artifact_dir / "title-logo.window.png"
        self._write_title_with_caption_png(title)

        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._preview_draw(),
            ),
            boot_png=title,
            boot_frame=self._title_frame(),
        )

        select = result["stages"]["select"]
        reuse = select["large_portrait_image"]["title_logo_reuse"]
        self.assertEqual(select["status"], "pass")
        self.assertNotIn("select_large_portrait_title_logo_reuse", select["reasons"])
        self.assertEqual(reuse["status"], "pass")

    def test_select_preview_rejects_blank_large_portrait_roster_tile_false_pass(self) -> None:
        self._write_select_png(large_portrait=False)

        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._roster_tile_draw(),
            )
        )

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "fail")
        self.assertEqual(select["select_preview"]["candidate_count"], 0)
        self.assertIn("select_large_portrait_low_nonblack", select["reasons"])
        self.assertIn("select_model_preview_no_texture_draws", select["reasons"])

    def test_select_preview_reports_palette_quality_instead_of_no_draw(self) -> None:
        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._preview_draw(
                    clut_blank_samples=43_284,
                    palette_fallback_samples=43_284,
                    color_hash_hex="0xe2a11fb6",
                ),
            )
        )

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "fail")
        self.assertIn("select_model_preview_known_bad_palette", select["reasons"])
        self.assertNotIn("select_model_preview_no_texture_draws", select["reasons"])
        self.assertEqual(select["select_preview"]["source_address_hex"], "0x003910fc")

    def test_select_preview_allows_expected_blank_clut_alias_with_new_palette(self) -> None:
        result = self._summarize(
            self._snapshot(
                3,
                recovery=self._no_model_recovery(),
                draw=self._preview_draw(
                    clut_blank_samples=43_284,
                    palette_fallback_samples=43_284,
                    color_hash_hex="0x89abcdef",
                ),
            )
        )

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "pass")
        self.assertEqual(select["select_preview"]["clut_blank_ratio"], 0.8887)
        self.assertEqual(select["select_preview"]["palette_fallback_ratio"], 0.8887)

    def test_missing_select_preview_telemetry_passes_when_large_portrait_is_visually_valid(
        self,
    ) -> None:
        result = self._summarize(
            self._snapshot(3, recovery=self._no_model_recovery())
        )

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "pass")
        self.assertNotIn("select_model_preview_no_texture_draws", select["reasons"])
        self.assertEqual(select["select_preview"]["candidate_count"], 0)
        self.assertEqual(select["select_preview"]["telemetry_status"], "missing")
        self.assertIn(
            "select_model_preview_no_texture_draws",
            select["select_preview"]["telemetry_reasons"],
        )

    def test_fixed_smoke_blank_large_portrait_regression_fails_when_available(self) -> None:
        stdout = Path(
            "tmp/native-e2e-qa-fixed-smoke/artifacts/smoke/native-match-tail-timeline.stdout.json"
        )
        stderr = Path(
            "tmp/native-e2e-qa-fixed-smoke/artifacts/smoke/native-match-tail-timeline.stderr.log"
        )
        if not stdout.exists() or not stderr.exists():
            self.skipTest("fixed smoke artifact is not present")

        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(stderr),
            smoke_prefix="tmp/native-e2e-qa-fixed-smoke/artifacts/smoke/timeline",
        )
        result = summary.summarize_smoke(args)

        select = result["stages"]["select"]
        self.assertEqual(select["status"], "fail")
        self.assertIn("select_large_portrait_low_nonblack", select["reasons"])

    def test_oracle_large_portrait_image_metrics_pass_when_available(self) -> None:
        oracle = Path("tmp/mame-oracle-select-snap/br2-oracle-character-select.png")
        if not oracle.exists():
            self.skipTest("oracle select snapshot is not present")

        diagnostics = summary.select_large_portrait_image_diagnostics(str(oracle))

        self.assertEqual(diagnostics["status"], "pass")
        self.assertGreaterEqual(
            diagnostics["nonblack_ratio"],
            summary.SELECT_LARGE_PORTRAIT_MIN_NONBLACK_RATIO,
        )
        self.assertGreaterEqual(
            diagnostics["edge_ratio"],
            summary.SELECT_LARGE_PORTRAIT_MIN_EDGE_RATIO,
        )

    def test_large_smoke_stdout_uses_streaming_loader(self) -> None:
        stdout = self.artifact_dir / "large-native-match-tail-timeline.stdout.json"
        filler = "x" * (summary.GENERIC_JSON_FULL_READ_LIMIT + 1024)
        data = {
            "ignored_large_blob": filler,
            "total_frames": 120,
            "missed_vblank_frames": 0,
            "executed_steps": 1,
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(
                    3,
                    recovery=self._no_model_recovery(),
                    draw=self._preview_draw(),
                ),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                self._snapshot(7, action="noop", gameplay=True),
            ],
            "state": {"input_activity": self._activity()},
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        original_read_text = Path.read_text

        def guarded_read_text(path: Path, *args: object, **kwargs: object) -> str:
            if path == stdout:
                raise AssertionError("large smoke stdout must not use Path.read_text")
            return original_read_text(path, *args, **kwargs)

        with mock.patch.object(Path, "read_text", guarded_read_text):
            result = summary.summarize_smoke(args)

        self.assertIn(result["status"], {"pass", "warn"})
        self.assertEqual(result["parse_error"], None)
        self.assertEqual(result["total_frames"], 120)

    def test_title_half_height_image_does_not_false_pass(self) -> None:
        half_title = self.artifact_dir / "half-title.window.png"
        self._write_half_title_png(half_title)

        diagnostics = summary.frame_image_diagnostics(str(half_title), "boot")

        self.assertEqual(diagnostics["status"], "fail")
        self.assertIn("boot_image_bottom_half_sparse", diagnostics["reasons"])
        self.assertTrue(
            any(reason.startswith("boot_image_short_active_row_span") for reason in diagnostics["reasons"])
        )

    def test_full_title_with_verified_caption_band_passes_sparse_bottom(self) -> None:
        title = self.artifact_dir / "full-title.window.png"
        self._write_title_with_caption_png(title)
        frame = {
            "title_screen_frame": True,
            "bottom_caption_band": True,
            "intro_caption_band": True,
        }

        diagnostics = summary.frame_image_diagnostics(str(title), "boot", frame)

        self.assertEqual(diagnostics["status"], "pass")
        self.assertNotIn("boot_image_bottom_half_sparse", diagnostics["reasons"])

    def test_low_color_combat_map_does_not_false_pass(self) -> None:
        broken_map = self.artifact_dir / "broken-map.window.png"
        self._write_low_color_stage_png(broken_map)

        diagnostics = summary.frame_image_diagnostics(str(broken_map), "combat")

        self.assertEqual(diagnostics["status"], "fail")
        self.assertIn("combat_image_low_playfield_color_diversity", diagnostics["reasons"])
        self.assertIn("combat_image_dominant_color_fill", diagnostics["reasons"])

    def test_noisy_select_portrait_does_not_false_pass(self) -> None:
        noisy_select = self.artifact_dir / "noisy-select.window.png"
        self._write_noisy_select_png(noisy_select)

        diagnostics = summary.select_large_portrait_image_diagnostics(str(noisy_select))

        self.assertEqual(diagnostics["status"], "fail")
        self.assertGreater(diagnostics["edge_ratio"], 0.30)
        self.assertLess(diagnostics["edge_ratio"], 0.55)
        self.assertIn(
            "select_large_portrait_noisy_texture_artifact",
            diagnostics["reasons"],
        )

    def test_repeating_tile_combat_map_does_not_false_pass(self) -> None:
        tile_map = self.artifact_dir / "tile-map.window.png"
        self._write_repeating_tile_png(tile_map)

        diagnostics = summary.frame_image_diagnostics(str(tile_map), "combat")

        self.assertEqual(diagnostics["status"], "fail")
        self.assertIn("combat_image_repeating_tile_grid", diagnostics["reasons"])

    def test_normal_combat_fixture_does_not_trigger_new_visual_artifact_gates(self) -> None:
        diagnostics = summary.frame_image_diagnostics(
            str(self.combat_png),
            "combat",
            self._frame(gameplay=True),
        )

        self.assertEqual(diagnostics["status"], "pass")
        self.assertNotIn("combat_image_challenger_overlay_stale", diagnostics["reasons"])
        self.assertNotIn("combat_image_black_character_silhouette", diagnostics["reasons"])
        self.assertNotIn("combat_image_vs_texture_corruption", diagnostics["reasons"])

    def test_failure_evidence_challenger_overlay_does_not_false_pass(self) -> None:
        self._assert_evidence_fails_gameplay_image_gate(
            "timeline.tail-23-beast.window.png",
            "challenger_overlay_stale",
        )

    def test_failure_evidence_corrupt_vs_actual_display_does_not_false_pass(self) -> None:
        path = self._evidence_png("timeline.tail-23-beast.actual-display.png")
        for kind in ("combat", "beast", "final"):
            with self.subTest(kind=kind):
                diagnostics = summary.frame_image_diagnostics(
                    str(path),
                    kind,
                    self._frame(gameplay=True),
                )
                self.assertEqual(diagnostics["status"], "fail")
                self.assertIn(f"{kind}_image_vs_texture_corruption", diagnostics["reasons"])
                self.assertIn(f"{kind}_image_black_character_silhouette", diagnostics["reasons"])

    def test_failure_evidence_black_silhouette_frames_do_not_false_pass(self) -> None:
        for name in (
            "timeline.tail-39-p2_beast.window.png",
            "timeline.tail-40-noop.window.png",
        ):
            self._assert_evidence_fails_gameplay_image_gate(
                name,
                "black_character_silhouette",
            )

    def test_smoke_summary_checks_actual_display_when_window_primary_passes(self) -> None:
        broken_actual_display = self._evidence_png(
            "timeline.tail-23-beast.actual-display.png"
        )
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        post_beast = self._snapshot(7, action="noop", gameplay=True)
        post_beast["window_output"] = str(self.post_beast_png)
        post_beast["actual_display_output"] = str(broken_actual_display)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                post_beast,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 180,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        beast = result["stages"]["beast"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(beast["status"], "fail")
        self.assertEqual(beast["png"], str(self.post_beast_png))
        self.assertIn("beast_image_vs_texture_corruption", beast["reasons"])
        self.assertIn("beast_image_black_character_silhouette", beast["reasons"])

    def test_beast_stuck_frame_and_residual_draw_do_not_false_pass(self) -> None:
        self._write_combat_png(self.beast_png, variant=1)
        self._write_combat_png(self.post_beast_png, variant=1)
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        residual_draw = {
            "kind": "textured_quad",
            "source": {"address_hex": "0x0038b1b8"},
            "bounds": {"left": -197, "top": 193, "right": 60, "bottom": 294},
        }
        post_beast = self._snapshot(7, action="noop", gameplay=True, draw=residual_draw)
        post_beast["window_output"] = str(self.post_beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                post_beast,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 180,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "fail")
        self.assertIn("beast_effect_stuck_frame", result["beast_effect"]["reasons"])
        self.assertIn("beast_effect_residual_draw:0x0038b1b8", result["beast_effect"]["reasons"])

    def test_smoke_beast_transient_stuck_then_following_gameplay_recovers(self) -> None:
        self._write_combat_png(self.beast_png, variant=1)
        self._write_combat_png(self.post_beast_png, variant=2)
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        transient_post = self._snapshot(7, action="noop", gameplay=True)
        transient_post["window_output"] = str(self.beast_png)
        recovered_post = self._snapshot(8, action="noop", gameplay=True)
        recovered_post["window_output"] = str(self.post_beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                transient_post,
                recovered_post,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertIn(result["status"], {"pass", "warn"})
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["post_tail_index"], 8)
        self.assertIn(
            "beast_effect_stuck_frame",
            result["beast_effect"]["checks"][0]["reasons"],
        )
        self.assertEqual(result["beast_effect"]["checks"][1]["reasons"], [])
        self.assertEqual(result["stages"]["beast"]["png"], str(self.post_beast_png))

    def test_smoke_beast_persistent_repeated_effect_sequence_fails(self) -> None:
        self._write_combat_png(self.beast_png, variant=1)
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        repeated_posts = []
        for tail_index in (7, 8, 9):
            repeated = self._snapshot(tail_index, action="noop", gameplay=True)
            repeated["window_output"] = str(self.beast_png)
            repeated_posts.append(repeated)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                *repeated_posts,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "fail")
        self.assertIn("beast_effect_stuck_frame", result["beast_effect"]["reasons"])
        self.assertIn(
            "beast_effect_persistent_stuck_sequence",
            result["beast_effect"]["reasons"],
        )
        self.assertEqual(result["beast_effect"]["checked_post_snapshot_count"], 3)

    def test_smoke_beast_detection_ignores_early_non_gameplay_p1_beast(self) -> None:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        early_beast = self._snapshot(4, action="beast")
        early_beast["window_output"] = str(self.png)
        early_post = self._snapshot(5, action="noop")
        early_post["window_output"] = str(self.png)
        gameplay_beast = self._snapshot(8, action="p2+beast", gameplay=True)
        gameplay_beast["window_output"] = str(self.beast_png)
        post_gameplay_beast = self._snapshot(9, action="noop", gameplay=True)
        post_gameplay_beast["window_output"] = str(self.post_beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                early_beast,
                early_post,
                self._snapshot(6, action="punch", gameplay=True),
                self._snapshot(7, action="noop", gameplay=True),
                gameplay_beast,
                post_gameplay_beast,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertIn(result["status"], {"pass", "warn"})
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["action"], "p2+beast")
        self.assertEqual(result["beast_effect"]["beast_tail_index"], 8)
        self.assertEqual(result["beast_effect"]["post_tail_index"], 9)
        self.assertEqual(result["beast_effect"]["beast_snapshot_count"], 2)
        self.assertEqual(result["beast_effect"]["gameplay_beast_snapshot_count"], 1)
        self.assertEqual(result["stages"]["beast"]["png"], str(self.post_beast_png))

    def test_smoke_requires_every_gameplay_beast_sequence_to_recover(self) -> None:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        p1_beast = self._snapshot(6, action="beast", gameplay=True)
        p1_beast["window_output"] = str(self.beast_png)
        p1_post = self._snapshot(7, action="noop", gameplay=True)
        p1_post["window_output"] = str(self.post_beast_png)
        p2_beast = self._snapshot(8, action="p2+beast", gameplay=True)
        p2_beast["window_output"] = str(self.beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                p1_beast,
                p1_post,
                p2_beast,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "fail")
        self.assertEqual(result["beast_effect"]["action"], "p2+beast")
        self.assertIn(
            "post_beast_gameplay_snapshot_missing",
            result["beast_effect"]["reasons"],
        )

    def test_smoke_final_timeline_frame_must_remain_visually_gameplay(self) -> None:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        broken_final = self._snapshot(8, action="noop", gameplay=False)
        broken_final["window_output"] = str(self.png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                self._snapshot(6, action="beast", gameplay=True),
                self._snapshot(7, action="noop", gameplay=True),
                broken_final,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["stages"]["final"]["status"], "fail")
        self.assertTrue(
            any(
                reason.startswith("combat_image_")
                for reason in result["stages"]["final"]["reasons"]
            )
        )

    def test_smoke_beast_transition_uses_first_gameplay_noop_as_result(self) -> None:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        beast_transition = self._snapshot(6, action="p2+beast", gameplay=False)
        beast_transition["window_output"] = str(self.png)
        beast_result = self._snapshot(7, action="noop", gameplay=True)
        beast_result["window_output"] = str(self.beast_png)
        post_beast = self._snapshot(8, action="noop", gameplay=True)
        post_beast["window_output"] = str(self.post_beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                beast_transition,
                beast_result,
                post_beast,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertIn(result["status"], {"pass", "warn"})
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["action"], "p2+beast")
        self.assertEqual(result["beast_effect"]["beast_action_tail_index"], 6)
        self.assertEqual(result["beast_effect"]["beast_tail_index"], 7)
        self.assertEqual(result["beast_effect"]["post_tail_index"], 7)
        self.assertEqual(result["beast_effect"]["post_action"], "noop")
        self.assertEqual(result["beast_effect"]["beast_snapshot_count"], 1)
        self.assertEqual(result["beast_effect"]["gameplay_beast_snapshot_count"], 1)
        self.assertEqual(result["beast_effect"]["delta"]["status"], "not_checked")
        self.assertEqual(result["stages"]["beast"]["png"], str(self.beast_png))

    def test_smoke_beast_transition_single_recovering_noop_passes(self) -> None:
        stdout = self.artifact_dir / "native-match-tail-timeline.stdout.json"
        beast_transition = self._snapshot(6, action="p2+beast", gameplay=False)
        beast_transition["window_output"] = str(self.png)
        recovered_post = self._snapshot(7, action="noop", gameplay=True)
        recovered_post["window_output"] = str(self.post_beast_png)
        data = {
            "boot": {"window_output": str(self.png), "window_frame": self._frame()},
            "snapshots": [
                self._snapshot(0),
                self._snapshot(1),
                self._snapshot(2),
                self._snapshot(3, recovery=self._no_model_recovery(), draw=self._preview_draw()),
                self._snapshot(5, action="punch", gameplay=True),
                beast_transition,
                recovered_post,
            ],
            "state": {"input_activity": self._activity()},
            "missed_vblank_frames": 0,
            "total_frames": 240,
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            smoke_status=0,
            smoke_stdout=str(stdout),
            smoke_stderr=str(self.stderr),
            smoke_prefix=str(self.artifact_dir / "timeline"),
        )

        result = summary.summarize_smoke(args)

        self.assertIn(result["status"], {"pass", "warn"})
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["beast_action_tail_index"], 6)
        self.assertEqual(result["beast_effect"]["beast_tail_index"], 7)
        self.assertEqual(result["beast_effect"]["post_tail_index"], 7)
        self.assertEqual(result["beast_effect"]["delta"]["status"], "not_checked")
        self.assertEqual(result["stages"]["beast"]["png"], str(self.post_beast_png))

    def test_live_audio_and_performance_gates_do_not_false_pass(self) -> None:
        capture_dir = self.artifact_dir / "captures"
        capture_dir.mkdir()
        select_json = capture_dir / "000010-select.json"
        select_png = select_json.with_suffix(".png")
        combat_json = capture_dir / "000100-combat.json"
        combat_png = combat_json.with_suffix(".png")
        final_json = capture_dir / "000200-final.json"
        final_png = final_json.with_suffix(".png")
        self._write_select_png(large_portrait=True)
        self.png.replace(select_png)
        self._write_combat_png(combat_png, variant=0)
        self._write_combat_png(final_png, variant=2)
        select_json.write_text(
            json.dumps(
                {
                    "label": "select",
                    "gui_frame": 10,
                    "buttons": {"start": True},
                    "frame": self._frame(),
                }
            ),
            encoding="utf-8",
        )
        combat_json.write_text(
            json.dumps(
                {
                    "label": "combat",
                    "gui_frame": 100,
                    "buttons": {"punch": True},
                    "frame": self._frame(gameplay=True),
                }
            ),
            encoding="utf-8",
        )
        final_json.write_text(
            json.dumps(
                {
                    "label": "final",
                    "gui_frame": 200,
                    "buttons": {},
                    "frame": self._frame(gameplay=True),
                }
            ),
            encoding="utf-8",
        )
        stdout = self.artifact_dir / "native-play.stdout.json"
        data = {
            "playable": True,
            "gameplay_ready": True,
            "render_ready": True,
            "native_play_test_input_verified": True,
            "audio_realtime_verified": False,
            "performance_verified": False,
            "final_frame_full_size": True,
            "final_frame_visible_content": True,
            "final_frame_scene_detail": True,
            "final_frame_render_ready": True,
            "final_frame_gameplay_scene": True,
            "final_frame_render_verified": True,
            "final_frame": self._frame(gameplay=True),
            "gui_watchdog": {"missed_vblank_attempts": 0, "stop_reason": "max_frames"},
            "audio_health": {
                "state": "silent_pcm",
                "reason": "rendered_pcm_is_silent",
                "audible": False,
                "render_progressing": True,
                "pcm_nonzero": False,
                "realtime_output_seen": False,
                "realtime_ok": False,
            },
            "performance": {
                "verified": False,
                "frame_samples_present": True,
                "attack_samples_required": True,
                "attack_samples_present": False,
                "beast_samples_required": True,
                "beast_samples_present": False,
                "frame_p95_within_budget": False,
                "frame_max_stall_within_budget": False,
                "frame_over_33_ms_ratio_within_budget": False,
                "no_missed_vblank_attempts": True,
            },
        }
        stdout.write_text(json.dumps(data), encoding="utf-8")
        args = argparse.Namespace(
            live_status=0,
            live_stdout=str(stdout),
            live_stderr=str(self.stderr),
            live_capture_dir=str(capture_dir),
        )

        result = summary.summarize_live(args)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["audio"]["status"], "fail")
        self.assertIn("audio_pcm_nonzero:false", result["audio"]["reasons"])
        self.assertEqual(result["performance"]["status"], "fail")
        self.assertIn("attack_samples_present:false", result["performance"]["reasons"])
        self.assertIn("beast_samples_present:false", result["performance"]["reasons"])

    def test_live_complete_fixture_passes_strict_e2e_gates(self) -> None:
        capture_dir = self._write_live_capture_set()
        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["stages"]["boot"]["status"], "pass")
        self.assertEqual(result["stages"]["select"]["status"], "pass")
        self.assertEqual(result["stages"]["p2_select"]["status"], "pass")
        self.assertEqual(result["stages"]["combat"]["status"], "pass")
        self.assertEqual(result["stages"]["beast"]["status"], "pass")
        self.assertEqual(result["input"]["status"], "pass")
        self.assertEqual(result["audio"]["status"], "pass")
        self.assertEqual(result["performance"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["status"], "pass")

    def test_live_half_title_capture_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set(half_title=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        boot = result["stages"]["boot"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(boot["status"], "fail")
        self.assertIn("boot_image_bottom_half_sparse", boot["reasons"])

    def test_live_blank_select_portrait_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set(blank_select=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        select = result["stages"]["select"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(select["status"], "fail")
        self.assertIn("select_large_portrait_low_nonblack", select["reasons"])

    def test_live_blank_p2_right_portrait_does_not_pass_from_p1_portrait(self) -> None:
        capture_dir = self._write_live_capture_set(blank_p2_select=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["stages"]["select"]["status"], "pass")
        p2_select = result["stages"]["p2_select"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(p2_select["status"], "fail")
        self.assertEqual(
            p2_select["large_portrait_image"]["roi"],
            {
                "left": 274,
                "top": 126,
                "right": 488,
                "bottom": 316,
            },
        )
        self.assertIn("select_large_portrait_low_nonblack", p2_select["reasons"])

    def test_live_p2_right_name_atlas_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set(p2_name_atlas=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        p2_select = result["stages"]["p2_select"]
        self.assertEqual(result["status"], "fail")
        self.assertEqual(p2_select["status"], "fail")
        self.assertTrue(
            {
                "select_large_portrait_noisy_texture_artifact",
                "select_large_portrait_repeating_tile_grid",
            }
            & set(p2_select["reasons"])
        )

    def test_live_p2_input_missing_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set()
        data = self._live_data()
        verification = dict(data["gui_input_verification"])
        verification["p2_full_observed"] = False
        guest_buttons = dict(verification["guest_buttons"])
        guest_buttons["p2_punch"] = False
        verification["guest_buttons"] = guest_buttons
        data["gui_input_verification"] = verification
        activity = self._activity()
        activity["p2_punch_active_reads"] = 0
        data["test_input_activity"] = activity
        data["input_activity"] = activity
        for path in capture_dir.glob("*.json"):
            payload = json.loads(path.read_text(encoding="utf-8"))
            capture_activity = dict(payload.get("input_activity") or {})
            capture_activity["p2_punch_active_reads"] = 0
            payload["input_activity"] = capture_activity
            path.write_text(json.dumps(payload), encoding="utf-8")

        result = self._summarize_live_data(data, capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["input"]["status"], "fail")
        self.assertIn("gui_input_p2_full_observed:false", result["input"]["reasons"])
        self.assertIn("guest_button_missing:p2_punch", result["input"]["reasons"])
        self.assertTrue(
            any(
                reason.startswith("input_activity_missing:")
                for reason in result["input"]["reasons"]
            )
        )

    def test_live_beast_stuck_effect_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set(stuck_beast=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "fail")
        self.assertIn("beast_effect_stuck_frame", result["beast_effect"]["reasons"])
        self.assertIn(
            "beast_effect_persistent_stuck_sequence",
            result["beast_effect"]["reasons"],
        )
        self.assertIn(
            "beast_reused_current_otc_stuck_after_noop",
            result["beast_effect"]["reasons"],
        )

    def test_live_beast_transient_stuck_then_following_capture_recovers(self) -> None:
        capture_dir = self._write_live_capture_set(
            stuck_beast=True,
            recover_after_stuck=True,
        )
        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["post_gui_frame"], 220)
        self.assertIn(
            "beast_effect_stuck_frame",
            result["beast_effect"]["checks"][0]["reasons"],
        )
        self.assertEqual(result["beast_effect"]["checks"][1]["reasons"], [])
        self.assertEqual(result["stages"]["beast"]["gui_frame"], 220)

    def test_live_beast_transition_uses_first_gameplay_post_capture(self) -> None:
        capture_dir = self._write_live_capture_set(beast_transition=True)
        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["beast_effect"]["status"], "pass")
        self.assertEqual(result["beast_effect"]["beast_gui_frame"], 180)
        self.assertEqual(result["beast_effect"]["post_gui_frame"], 180)
        self.assertEqual(result["beast_effect"]["gameplay_beast_capture_count"], 0)
        self.assertEqual(result["beast_effect"]["delta"]["status"], "not_checked")
        self.assertEqual(result["stages"]["beast"]["gui_frame"], 180)

    def test_live_audio_missing_sfx_activity_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set()
        data = self._live_data()
        data["audio"] = self._live_audio(sfx=False)

        result = self._summarize_live_data(data, capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["audio"]["status"], "fail")
        self.assertIn("audio_sfx_latch_nonzero_writes:0", result["audio"]["reasons"])
        self.assertIn("audio_sfx_key_on_events:0", result["audio"]["reasons"])

    def test_live_audio_queue_glitch_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set()
        data = self._live_data()
        audio = dict(data["audio"])
        queue = dict(audio["queue"])
        queue["underflow_frames"] = 256
        queue["callback_miss_frames"] = 256
        queue["callback_miss_events"] = 2
        audio["queue"] = queue
        data["audio"] = audio

        result = self._summarize_live_data(data, capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["audio"]["status"], "fail")
        self.assertIn("audio_queue_underflow_frames:256", result["audio"]["reasons"])
        self.assertIn("audio_queue_callback_miss_frames:256", result["audio"]["reasons"])

    def test_live_audio_transient_startup_callback_gap_passes_after_recovery(self) -> None:
        capture_dir = self._write_live_capture_set()
        data = self._live_data()
        audio = dict(data["audio"])
        queue = dict(audio["queue"])
        queue.update(
            {
                "callback_miss_frames": 8,
                "callback_miss_events": 1,
                "callback_fallback_frames": 4,
                "callback_fallback_events": 1,
                "callback_silence_frames": 4,
                "callback_silence_events": 1,
            }
        )
        audio["queue"] = queue
        data["audio"] = audio

        result = self._summarize_live_data(data, capture_dir)

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["audio"]["status"], "pass")

    def test_live_timing_over_budget_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set()
        data = self._live_data()
        data["timing"] = self._live_timing(frame_p95_us=45_000)

        result = self._summarize_live_data(data, capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["performance"]["status"], "fail")
        self.assertIn(
            "timing_frame_p95_over_budget:45000",
            result["performance"]["reasons"],
        )

    def test_live_render_ready_texture_artifact_does_not_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set()
        self._write_capture(
            capture_dir,
            105,
            "presented",
            frame=self._frame(),
            buttons={},
            image="tile_stage",
        )

        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["capture_sequence"]["status"], "fail")
        self.assertTrue(
            any(
                "capture_105_capture_image_repeating_tile_grid" == reason
                for reason in result["capture_sequence"]["reasons"]
            )
        )

    def test_live_non_render_transition_tile_is_ignored(self) -> None:
        capture_dir = self._write_live_capture_set()
        transition_frame = self._frame()
        transition_frame["render_ready_scene"] = False
        self._write_capture(
            capture_dir,
            105,
            "presented",
            frame=transition_frame,
            buttons={},
            image="tile_stage",
        )

        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["capture_sequence"]["status"], "pass")
        self.assertIn(105, result["capture_sequence"]["skipped_transition_frames"])

    def test_live_beast_detection_ignores_early_non_gameplay_false_pass(self) -> None:
        capture_dir = self._write_live_capture_set(stuck_beast=True)
        self._write_capture(
            capture_dir,
            80,
            "presented",
            frame=self._frame(),
            buttons={"beast": True},
            image="select",
        )
        self._write_capture(
            capture_dir,
            90,
            "presented",
            frame=self._frame(),
            buttons={},
            image="combat",
            variant=2,
        )

        result = self._summarize_live_data(self._live_data(), capture_dir)

        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["beast_effect"]["status"], "fail")
        self.assertEqual(result["beast_effect"]["gameplay_beast_capture_count"], 1)
        self.assertIn("beast_effect_stuck_frame", result["beast_effect"]["reasons"])

    def test_frame_artifact_flags_cover_known_character_and_map_corruption(self) -> None:
        frame = self._frame(gameplay=True)
        frame["br2_dense_live_field_texture_atlas_artifact"] = True
        frame["br2_character_select_name_overlay_artifact"] = True

        reasons = summary.frame_reasons(frame, "combat")

        self.assertIn("br2_dense_live_field_texture_atlas_artifact", reasons)
        self.assertIn("br2_character_select_name_overlay_artifact", reasons)


if __name__ == "__main__":
    unittest.main()
