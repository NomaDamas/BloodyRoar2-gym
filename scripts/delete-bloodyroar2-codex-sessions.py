#!/usr/bin/env python3
"""Delete only Codex sessions whose recorded cwd is this repository."""

from __future__ import annotations

import argparse
import json
import subprocess
import uuid
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SESSION_ROOT = Path.home() / ".codex" / "sessions"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--delete",
        action="store_true",
        help="Permanently delete matches with `codex delete --force`.",
    )
    return parser.parse_args()


def matching_sessions() -> dict[str, Path]:
    sessions: dict[str, Path] = {}

    for path in SESSION_ROOT.rglob("*.jsonl"):
        try:
            with path.open(encoding="utf-8", errors="replace") as handle:
                record = json.loads(handle.readline())
        except (OSError, json.JSONDecodeError):
            continue

        payload = record.get("payload", {})
        cwd = payload.get("cwd")
        session_id = payload.get("id")

        if record.get("type") != "session_meta" or not cwd or not session_id:
            continue

        try:
            recorded_root = Path(cwd).resolve()
            uuid.UUID(session_id)
        except (OSError, ValueError):
            continue

        if recorded_root == PROJECT_ROOT:
            sessions[session_id] = path

    return sessions


def main() -> int:
    args = parse_args()
    sessions = matching_sessions()

    print(f"Project: {PROJECT_ROOT}")
    print(f"Matching sessions: {len(sessions)}")
    for session_id, path in sorted(sessions.items()):
        print(f"{session_id}  {path}")

    if not args.delete:
        print("\nDry run only. Re-run with --delete to request deletion.")
        return 0

    expected = f"DELETE {len(sessions)} BLOODYROAR2-RUST SESSIONS"
    confirmation = input(f"\nType exactly:\n{expected}\n> ")
    if confirmation != expected:
        print("Cancelled.")
        return 1

    deleted = 0
    for session_id in sorted(sessions):
        print(f"Deleting {session_id}")
        subprocess.run(
            ["codex", "delete", "--force", session_id],
            check=True,
        )
        deleted += 1

    print(f"\nDeleted sessions: {deleted}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
