"""Minimal Gymnasium-style client for the Rust bloodyroar2-gym HTTP API.

This file does not vendor Gymnasium. If Gymnasium is installed, wrap this class
in your own Env subclass; otherwise use it directly from LLM/RL scripts.
"""

from __future__ import annotations

import json
import urllib.request

MAX_STEP_FRAMES = 600


class BloodyRoar2Client:
    def __init__(self, base_url: str = "http://127.0.0.1:8765") -> None:
        self.base_url = base_url.rstrip("/")

    def action_space(self) -> dict:
        return self._get("/action_space")

    def observation_space(self) -> dict:
        return self._get("/observation_space")

    def reset(self, screenshot: bool = False) -> tuple[dict, dict]:
        payload = self._post("/reset", {"screenshot": screenshot})
        return payload["observation"], payload.get("info", {})

    def step(
        self,
        action: int,
        frames: int = 1,
        screenshot: bool = False,
    ) -> tuple[dict, float, bool, bool, dict]:
        return self._step_payload(
            {"action": action, "frames": frames, "screenshot": screenshot}
        )

    def step_buttons(
        self,
        buttons: dict[str, bool],
        frames: int = 1,
        screenshot: bool = False,
    ) -> tuple[dict, float, bool, bool, dict]:
        """Step arbitrary simultaneous controls, for example up+punch+guard."""
        return self._step_payload(
            {"buttons": buttons, "frames": frames, "screenshot": screenshot}
        )

    def _step_payload(self, request: dict) -> tuple[dict, float, bool, bool, dict]:
        frames = request["frames"]
        if not 1 <= frames <= MAX_STEP_FRAMES:
            raise ValueError(f"frames must be between 1 and {MAX_STEP_FRAMES}")
        payload = self._post("/step", request)
        return (
            payload["observation"],
            float(payload["reward"]),
            bool(payload["terminated"]),
            bool(payload["truncated"]),
            payload.get("info", {}),
        )

    def _get(self, path: str) -> dict:
        with urllib.request.urlopen(self.base_url + path, timeout=5) as response:
            return json.loads(response.read().decode("utf-8"))

    def _post(self, path: str, payload: dict) -> dict:
        data = json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(
            self.base_url + path,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=5) as response:
            return json.loads(response.read().decode("utf-8"))
