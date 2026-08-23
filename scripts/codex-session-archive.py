#!/usr/bin/env python3
"""Build a compact, auditable archive of one Codex JSONL session."""

from __future__ import annotations

import argparse
import collections
import gzip
import hashlib
import heapq
import json
import re
import shlex
from pathlib import Path
from typing import Any, Iterable


EXACT_OUTPUT_LIMIT = 8 * 1024
OUTPUT_EDGE_LIMIT = 2 * 1024
MAX_DIAGNOSTIC_LINES = 64
MAX_DIAGNOSTIC_LINE_CHARS = 2 * 1024
MAX_SECRET_FINDINGS = 200
MAX_LARGEST_LINES = 40

DIAGNOSTIC_RE = re.compile(
    r"(?i)(error|failed|failure|warning|test result|passed|ignored|"
    r"finished|panic|fatal|sha-?256|commit|clippy|rustfmt|cargo test|"
    r"pytest|render|audio|underflow|vblank|input|playable|rom|cache)"
)
SECRET_PATTERNS = {
    "openai_key": re.compile(r"\bsk-[A-Za-z0-9_-]{16,}\b"),
    "github_token": re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}\b"),
    "bearer_token": re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~-]{20,}"),
    "aws_access_key": re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    "private_key": re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
}
PATCH_FILE_RE = re.compile(
    r"^\*\*\* (?:Add|Update|Delete) File: (.+)$", re.MULTILINE
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("session", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument(
        "--sanitize-existing",
        action="store_true",
        help="Redact secret-shaped strings in existing semantic gzip archives.",
    )
    return parser.parse_args()


def json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode(
        "utf-8"
    )


def text_sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8", errors="replace")).hexdigest()


def redact_secrets(text: str) -> str:
    redacted = text
    for name, pattern in SECRET_PATTERNS.items():
        redacted = pattern.sub(
            lambda match: (
                f"[REDACTED:{name}:bytes="
                f"{len(match.group(0).encode('utf-8'))}:"
                f"sha256={text_sha256(match.group(0))}]"
            ),
            redacted,
        )
    return redacted


def redact_value(value: Any) -> Any:
    if isinstance(value, str):
        return redact_secrets(value)
    if isinstance(value, list):
        return [redact_value(item) for item in value]
    if isinstance(value, dict):
        return {key: redact_value(item) for key, item in value.items()}
    return value


def normalized_text(value: Any) -> str:
    if isinstance(value, str):
        return value
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def message_text(content: Any) -> str:
    if isinstance(content, str):
        return content
    if not isinstance(content, list):
        return normalized_text(content)

    chunks: list[str] = []
    for item in content:
        if not isinstance(item, dict):
            chunks.append(normalized_text(item))
            continue
        for key in ("text", "input_text", "output_text"):
            value = item.get(key)
            if isinstance(value, str):
                chunks.append(value)
                break
    return "\n".join(chunks)


def iter_replacement_messages(history: Any) -> Iterable[tuple[str, str]]:
    if not isinstance(history, list):
        return
    for item in history:
        if not isinstance(item, dict) or item.get("type") != "message":
            continue
        role = str(item.get("role", "unknown"))
        yield role, message_text(item.get("content", []))


def parse_function_arguments(arguments: Any) -> tuple[str, dict[str, Any] | None]:
    text = normalized_text(arguments)
    try:
        parsed = json.loads(text)
    except (TypeError, ValueError):
        return text, None
    return text, parsed if isinstance(parsed, dict) else None


def command_prefix(command: str) -> str:
    try:
        tokens = shlex.split(command)
    except ValueError:
        tokens = command.strip().split()
    if not tokens:
        return "<empty>"
    return " ".join(tokens[:3])


def output_archive_record(output: str) -> dict[str, Any]:
    encoded = output.encode("utf-8", errors="replace")
    record: dict[str, Any] = {
        "bytes": len(encoded),
        "sha256": hashlib.sha256(encoded).hexdigest(),
    }
    if len(encoded) <= EXACT_OUTPUT_LIMIT:
        record["complete"] = True
        record["text"] = redact_secrets(output)
        return record

    diagnostic_lines = [
        redact_secrets(line[:MAX_DIAGNOSTIC_LINE_CHARS])
        for line in output.splitlines()
        if DIAGNOSTIC_RE.search(line)
    ][:MAX_DIAGNOSTIC_LINES]
    record.update(
        {
            "complete": False,
            "head": redact_secrets(
                encoded[:OUTPUT_EDGE_LIMIT].decode("utf-8", errors="replace")
            ),
            "tail": redact_secrets(
                encoded[-OUTPUT_EDGE_LIMIT:].decode("utf-8", errors="replace")
            ),
            "diagnostic_lines": diagnostic_lines,
        }
    )
    return record


def write_jsonl(handle: gzip.GzipFile, value: dict[str, Any]) -> None:
    handle.write(json_bytes(value))
    handle.write(b"\n")


def scan_secrets(
    text: str,
    *,
    line_no: int,
    category: str,
    findings: list[dict[str, Any]],
) -> None:
    if len(findings) >= MAX_SECRET_FINDINGS:
        return
    for name, pattern in SECRET_PATTERNS.items():
        for match in pattern.finditer(text):
            secret = match.group(0)
            findings.append(
                {
                    "line": line_no,
                    "category": category,
                    "pattern": name,
                    "match_bytes": len(secret.encode("utf-8")),
                    "match_sha256": text_sha256(secret),
                }
            )
            if len(findings) >= MAX_SECRET_FINDINGS:
                return


def sanitize_jsonl_gzip(path: Path) -> int:
    temporary_path = path.with_name(f"{path.name}.redacted")
    changed_records = 0
    with (
        gzip.open(path, "rb") as source,
        gzip.open(temporary_path, "wb", compresslevel=6) as destination,
    ):
        for raw in source:
            record = json.loads(raw)
            redacted = redact_value(record)
            if redacted != record:
                changed_records += 1
            write_jsonl(destination, redacted)
    temporary_path.replace(path)
    return changed_records


def sanitize_existing_archive(output_dir: Path) -> int:
    results = {}
    for name in ("messages.jsonl.gz", "compactions.jsonl.gz", "tool-events.jsonl.gz"):
        path = output_dir / name
        results[name] = sanitize_jsonl_gzip(path)
    print(json.dumps(results, ensure_ascii=False))
    return 0


def main() -> int:
    args = parse_args()
    session_path = args.session.expanduser().resolve()
    output_dir = args.output_dir.resolve()
    if args.sanitize_existing:
        return sanitize_existing_archive(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    inventory_path = output_dir / "inventory.json"
    messages_path = output_dir / "messages.jsonl.gz"
    compactions_path = output_dir / "compactions.jsonl.gz"
    tools_path = output_dir / "tool-events.jsonl.gz"
    manifest_path = output_dir / "event-manifest.tsv.gz"

    source_hash = hashlib.sha256()
    line_count = 0
    byte_count = 0
    invalid_json: list[dict[str, Any]] = []
    first_timestamp: str | None = None
    last_timestamp: str | None = None

    top_type_counts: collections.Counter[str] = collections.Counter()
    top_type_bytes: collections.Counter[str] = collections.Counter()
    subtype_counts: collections.Counter[str] = collections.Counter()
    subtype_bytes: collections.Counter[str] = collections.Counter()
    role_counts: collections.Counter[str] = collections.Counter()
    role_text_bytes: collections.Counter[str] = collections.Counter()
    function_counts: collections.Counter[str] = collections.Counter()
    command_prefix_counts: collections.Counter[str] = collections.Counter()
    cwd_counts: collections.Counter[str] = collections.Counter()
    patch_file_counts: collections.Counter[str] = collections.Counter()
    archive_record_counts: collections.Counter[str] = collections.Counter()
    archive_text_bytes: collections.Counter[str] = collections.Counter()
    secret_findings: list[dict[str, Any]] = []
    session_ids: set[str] = set()
    largest_lines: list[tuple[int, int, dict[str, Any]]] = []

    with (
        session_path.open("rb") as source,
        gzip.open(messages_path, "wb", compresslevel=6) as messages,
        gzip.open(compactions_path, "wb", compresslevel=6) as compactions,
        gzip.open(tools_path, "wb", compresslevel=6) as tools,
        gzip.open(manifest_path, "wt", encoding="utf-8", compresslevel=6) as manifest,
    ):
        manifest.write("line\traw_bytes\tsha256\ttimestamp\ttype\tsubtype\n")
        for line_count, raw in enumerate(source, 1):
            raw_size = len(raw)
            byte_count += raw_size
            source_hash.update(raw)
            raw_hash = hashlib.sha256(raw).hexdigest()

            try:
                event = json.loads(raw)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                invalid_json.append(
                    {
                        "line": line_count,
                        "bytes": raw_size,
                        "sha256": raw_hash,
                        "error": str(error),
                    }
                )
                manifest.write(
                    f"{line_count}\t{raw_size}\t{raw_hash}\t\tinvalid_json\t\n"
                )
                continue

            timestamp = str(event.get("timestamp", ""))
            if timestamp:
                first_timestamp = first_timestamp or timestamp
                last_timestamp = timestamp
            top_type = str(event.get("type", "unknown"))
            payload = event.get("payload")
            payload_dict = payload if isinstance(payload, dict) else {}
            subtype = str(payload_dict.get("type", ""))

            top_type_counts[top_type] += 1
            top_type_bytes[top_type] += raw_size
            subtype_key = f"{top_type}:{subtype or '<none>'}"
            subtype_counts[subtype_key] += 1
            subtype_bytes[subtype_key] += raw_size
            manifest.write(
                f"{line_count}\t{raw_size}\t{raw_hash}\t{timestamp}\t"
                f"{top_type}\t{subtype}\n"
            )

            largest_entry = {
                "line": line_count,
                "bytes": raw_size,
                "sha256": raw_hash,
                "timestamp": timestamp,
                "type": top_type,
                "subtype": subtype,
            }
            heap_item = (raw_size, line_count, largest_entry)
            if len(largest_lines) < MAX_LARGEST_LINES:
                heapq.heappush(largest_lines, heap_item)
            elif heap_item[:2] > largest_lines[0][:2]:
                heapq.heapreplace(largest_lines, heap_item)

            if top_type == "session_meta":
                session_id = payload_dict.get("id")
                if isinstance(session_id, str):
                    session_ids.add(session_id)
                cwd = payload_dict.get("cwd")
                if isinstance(cwd, str):
                    cwd_counts[cwd] += 1
                continue

            if top_type == "turn_context":
                cwd = payload_dict.get("cwd")
                if isinstance(cwd, str):
                    cwd_counts[cwd] += 1
                continue

            if top_type == "compacted":
                compaction_message = normalized_text(payload_dict.get("message", ""))
                replacement_history = payload_dict.get("replacement_history", [])
                record = {
                    "line": line_count,
                    "timestamp": timestamp,
                    "message": redact_secrets(compaction_message),
                    "message_bytes": len(compaction_message.encode("utf-8")),
                    "message_sha256": text_sha256(compaction_message),
                    "replacement_message_count": (
                        len(replacement_history)
                        if isinstance(replacement_history, list)
                        else 0
                    ),
                    "replacement_history_sha256": text_sha256(
                        normalized_text(replacement_history)
                    ),
                }
                write_jsonl(compactions, record)
                archive_record_counts["compaction"] += 1
                archive_text_bytes["compaction"] += record["message_bytes"]
                scan_secrets(
                    compaction_message,
                    line_no=line_count,
                    category="compaction",
                    findings=secret_findings,
                )
                for role, text in iter_replacement_messages(replacement_history):
                    role_counts[f"compacted:{role}"] += 1
                    role_text_bytes[f"compacted:{role}"] += len(
                        text.encode("utf-8")
                    )
                continue

            if top_type == "event_msg":
                if subtype in {"user_message", "agent_message"}:
                    text = normalized_text(
                        payload_dict.get("message", payload_dict.get("text", ""))
                    )
                    role = "user" if subtype == "user_message" else "assistant"
                    record = {
                        "line": line_count,
                        "timestamp": timestamp,
                        "source": "event_msg",
                        "role": role,
                        "text": redact_secrets(text),
                        "text_bytes": len(text.encode("utf-8")),
                        "sha256": text_sha256(text),
                    }
                    write_jsonl(messages, record)
                    archive_record_counts[f"message:{role}"] += 1
                    archive_text_bytes[f"message:{role}"] += record["text_bytes"]
                    role_counts[f"event_msg:{role}"] += 1
                    role_text_bytes[f"event_msg:{role}"] += record["text_bytes"]
                    scan_secrets(
                        text,
                        line_no=line_count,
                        category=f"event_msg:{role}",
                        findings=secret_findings,
                    )
                continue

            if top_type != "response_item":
                continue

            if subtype == "message":
                role = str(payload_dict.get("role", "unknown"))
                text = message_text(payload_dict.get("content", []))
                text_size = len(text.encode("utf-8"))
                role_counts[f"response_item:{role}"] += 1
                role_text_bytes[f"response_item:{role}"] += text_size
                if role in {"user", "assistant"}:
                    record = {
                        "line": line_count,
                        "timestamp": timestamp,
                        "source": "response_item",
                        "role": role,
                        "phase": payload_dict.get("phase"),
                        "text": redact_secrets(text),
                        "text_bytes": text_size,
                        "sha256": text_sha256(text),
                    }
                    write_jsonl(messages, record)
                    archive_record_counts[f"message:{role}"] += 1
                    archive_text_bytes[f"message:{role}"] += text_size
                    scan_secrets(
                        text,
                        line_no=line_count,
                        category=f"response_item:{role}",
                        findings=secret_findings,
                    )
                continue

            if subtype in {"function_call", "custom_tool_call"}:
                name = str(
                    payload_dict.get("name", payload_dict.get("tool_name", "unknown"))
                )
                arguments = payload_dict.get(
                    "arguments", payload_dict.get("input", "")
                )
                arguments_text, parsed_arguments = parse_function_arguments(arguments)
                function_counts[name] += 1
                record = {
                    "line": line_count,
                    "timestamp": timestamp,
                    "event": subtype,
                    "name": name,
                    "call_id": payload_dict.get("call_id"),
                    "arguments": redact_secrets(arguments_text),
                    "arguments_bytes": len(arguments_text.encode("utf-8")),
                    "arguments_sha256": text_sha256(arguments_text),
                }
                if parsed_arguments is not None:
                    command = parsed_arguments.get("cmd")
                    if isinstance(command, str):
                        record["command"] = command
                        command_prefix_counts[command_prefix(command)] += 1
                    workdir = parsed_arguments.get("workdir")
                    if isinstance(workdir, str):
                        cwd_counts[workdir] += 1
                if "apply_patch" in name or "*** Begin Patch" in arguments_text:
                    for patch_file in PATCH_FILE_RE.findall(arguments_text):
                        patch_file_counts[patch_file] += 1
                write_jsonl(tools, record)
                archive_record_counts[f"tool_call:{name}"] += 1
                archive_text_bytes["tool_call_arguments"] += record[
                    "arguments_bytes"
                ]
                scan_secrets(
                    arguments_text,
                    line_no=line_count,
                    category=f"tool_call:{name}",
                    findings=secret_findings,
                )
                continue

            if subtype in {"function_call_output", "custom_tool_call_output"}:
                output = normalized_text(
                    payload_dict.get("output", payload_dict.get("content", ""))
                )
                record = {
                    "line": line_count,
                    "timestamp": timestamp,
                    "event": subtype,
                    "call_id": payload_dict.get("call_id"),
                    "output": output_archive_record(output),
                }
                write_jsonl(tools, record)
                completeness = (
                    "complete"
                    if record["output"]["complete"]
                    else "excerpted"
                )
                archive_record_counts[f"tool_output:{completeness}"] += 1
                archive_text_bytes["tool_output_source"] += record["output"]["bytes"]
                scan_secrets(
                    output,
                    line_no=line_count,
                    category="tool_output",
                    findings=secret_findings,
                )
                continue

            if subtype == "reasoning":
                summary = payload_dict.get("summary", [])
                summary_text = normalized_text(summary)
                encrypted = payload_dict.get("encrypted_content", "")
                encrypted_text = encrypted if isinstance(encrypted, str) else ""
                archive_record_counts["reasoning"] += 1
                archive_text_bytes["reasoning_summary"] += len(
                    summary_text.encode("utf-8")
                )
                archive_text_bytes["reasoning_encrypted"] += len(
                    encrypted_text.encode("utf-8")
                )

    inventory = {
        "archive_format_version": 1,
        "source": {
            "path": str(session_path),
            "bytes": byte_count,
            "lines": line_count,
            "sha256": source_hash.hexdigest(),
            "first_timestamp": first_timestamp,
            "last_timestamp": last_timestamp,
            "session_ids": sorted(session_ids),
        },
        "preservation_policy": {
            "messages": "Exact user and assistant text except secret-shaped values, which are replaced with type, length, and SHA-256.",
            "compactions": "Exact compaction summary text except secret-shaped values; replacement history is counted and hashed.",
            "tool_calls": "Exact tool names and arguments, including patches and commands, except secret-shaped values.",
            "tool_outputs": (
                f"Exact through {EXACT_OUTPUT_LIMIT} bytes; larger outputs retain "
                f"SHA-256, size, first/last {OUTPUT_EDGE_LIMIT} bytes, and up to "
                f"{MAX_DIAGNOSTIC_LINES} diagnostic lines."
            ),
            "reasoning": (
                "Encrypted reasoning bodies are not copied; aggregate byte counts "
                "remain in the inventory because they are not project-readable text."
            ),
            "event_manifest": "Every source line retains raw size, SHA-256, timestamp, type, and subtype.",
        },
        "counts": {
            "top_types": dict(top_type_counts.most_common()),
            "subtypes": dict(subtype_counts.most_common()),
            "roles": dict(role_counts.most_common()),
            "functions": dict(function_counts.most_common()),
            "archive_records": dict(archive_record_counts.most_common()),
        },
        "bytes": {
            "top_types": dict(top_type_bytes.most_common()),
            "subtypes": dict(subtype_bytes.most_common()),
            "role_text": dict(role_text_bytes.most_common()),
            "archive_text": dict(archive_text_bytes.most_common()),
        },
        "scope": {
            "working_directories": dict(cwd_counts.most_common()),
            "patched_files": dict(patch_file_counts.most_common()),
            "command_prefixes": dict(command_prefix_counts.most_common(500)),
        },
        "largest_lines": [
            entry
            for _, _, entry in sorted(
                largest_lines, key=lambda item: (item[0], item[1]), reverse=True
            )
        ],
        "secret_scan": {
            "patterns": sorted(SECRET_PATTERNS),
            "finding_count": len(secret_findings),
            "findings": secret_findings,
            "note": "Potential values are never copied; only location, size, and SHA-256 are recorded.",
        },
        "invalid_json": invalid_json,
        "outputs": {
            "messages": messages_path.name,
            "compactions": compactions_path.name,
            "tool_events": tools_path.name,
            "event_manifest": manifest_path.name,
        },
    }
    inventory_path.write_text(
        json.dumps(inventory, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(inventory["source"], ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
