# Codex Session Archive Manifest

## Identity

| Field | Value |
| --- | --- |
| Project | `BloodyRoar2-rust` |
| Session ID | `019ed623-4296-79f2-a751-ff3cec538edc` |
| Original path | `/Users/jinminseong/.codex/sessions/2026/06/18/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl` |
| Original bytes | `16,252,314,249` |
| Original lines | `845,307` |
| Original SHA-256 | `34d9f9e49e7d5bdea008c792ddc1a322ff736bc9842e5aaff40ef48fde212da6` |
| First event | `2026-06-17T15:14:23.970Z` |
| Last event | `2026-08-07T14:08:10.676Z` |
| Invalid JSON records | `0` |

## Lossless Archive

```text
.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl.zst
```

| Field | Value |
| --- | --- |
| Compressed bytes | `634,119,614` |
| SHA-256 | `f25546b39bbfe183e1b3e9149912793727a2a39f225b0641a2d475e6d004e757` |
| Filesystem mode | `0600` |
| Zstandard frames | `1` |
| Zstandard checksum | `XXH64` |
| Decompressed size | `15.1 GiB` |
| Decompressed SHA matches source | `true` |
| Archive directory mode | `0700` |
| Archive file mode | `0600` |
| Git tracked | `false` |

This file is the information-loss prevention layer. It is byte-for-byte
restorable and supersedes any omission or simplification in the human summary.

## Semantic Archive

All files are mode `0600` and live under:

```text
.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/semantic/
```

| File | Bytes | Records | SHA-256 |
| --- | --- | --- | --- |
| `messages.jsonl.gz` | `17,633,268` | `132,724` | `5a7eddd269459e4b3195e84149f21719eae6c836393dd928452d321a2e629d1a` |
| `compactions.jsonl.gz` | `4,187,722` | `2,611` | `f5af6aaf1577ac94969cc3a60d15c8e2ab8e7f0a91410b21c6229446303dd1d6` |
| `tool-events.jsonl.gz` | `170,539,979` | `456,349` | `6c67726565b59f7420beb7e8d7568b3a8d692cc59398c93dc6243a42ccb68de3` |
| `event-manifest.tsv.gz` | `42,854,499` | `845,308` including header | `2b1049784646e9824e3eca4b07880675c602c51db9aeb38d9e8f37d7195ff0d1` |

Preservation rules:

- User and assistant messages are retained with source line, timestamp, size,
  and original text hash.
- Compaction summary text is retained; replacement histories are counted and
  hashed.
- Tool names and arguments, including commands and patches, are retained.
- Tool outputs up to 8,192 bytes are retained in full. Larger outputs retain
  byte size, SHA-256, first/last 2,048 bytes, and up to 64 diagnostic lines.
- Every original line has a timestamp/type/subtype/size/SHA-256 entry.
- Encrypted reasoning bodies are not copied into the semantic archive; their
  aggregate byte count remains in `inventory.json`. They remain present in the
  exact lossless archive.
- The same secret-shaped value appeared in three original tool outputs. The
  exact archive retains it; semantic files redact it with type, size, and hash.
  A post-redaction scan found zero secret-pattern matches.

## Inventory Highlights

| Event category | Count |
| --- | --- |
| Total JSONL events | `845,307` |
| `response_item` | `636,724` |
| `event_msg` | `201,631` |
| `turn_context` | `2,828` |
| `compacted` | `2,611` |
| `world_state` | `1,512` |
| Function calls | `218,819` |
| Function call outputs | `218,624` |
| Reasoning records | `109,769` |
| Response messages | `67,912` |
| `exec_command` calls | `171,241` |
| `apply_patch` calls | `9,453` |

The source has dual logging for some visible messages, so archive message
records must not be interpreted as a count of unique conversational turns.

## Preserved Runtime Evidence

| Artifact | SHA-256 |
| --- | --- |
| `.runtime-cache/bin/bloodyroar2-gym` | `1f9955883abd6c2e9894cb09ab9d41b839109212c4e58d3c1a304dd2d5a200c9` |
| `qa-summary-20260808-213347.json` | `b895ebd8917510f4fcbf1eca48bd1f36e675012e6db08a7e67d4655b6f7593d9` |
| `qa-summary-20260808-224406.json` | `6f43251ddd6c290cdfeda52fe9aa5f59b6bac138397f324045f60e40c3a78f0a` |
| `scripts/codex-session-archive.py` before documentation edits | `61053966df00cc8760e98755ddf7f517c74917eea34912a999c2a0b4dfdeeeb3` |

The QA summaries are post-source continuation evidence dated
`2026-08-08`; they are not events from the archived session.

## Verification Commands

Run from the repository root:

```bash
ARCHIVE=.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl.zst
zstd -t "$ARCHIVE"
shasum -a 256 "$ARCHIVE"
```

Verify the decompressed identity without writing a 15GB copy:

```bash
zstd -d --stdout "$ARCHIVE" | shasum -a 256
```

Expected decompressed SHA-256:

```text
34d9f9e49e7d5bdea008c792ddc1a322ff736bc9842e5aaff40ef48fde212da6
```

Verify semantic gzip containers:

```bash
gzip -t .runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/semantic/messages.jsonl.gz
gzip -t .runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/semantic/compactions.jsonl.gz
gzip -t .runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/semantic/tool-events.jsonl.gz
gzip -t .runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/semantic/event-manifest.tsv.gz
```

## Restoration

Restore to a temporary inspection path:

```bash
ARCHIVE=.runtime-cache/session-archives/019ed623-4296-79f2-a751-ff3cec538edc/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl.zst
RESTORED=/private/tmp/rollout-019ed623-4296-79f2-a751-ff3cec538edc.jsonl
zstd -d --stdout "$ARCHIVE" > "$RESTORED"
chmod 600 "$RESTORED"
shasum -a 256 "$RESTORED"
```

Do not restore into `~/.codex/sessions` while any Codex process might open or
append to that path. If an exact in-place restoration is ever required, first
verify the destination is absent and unopened, then install the verified
temporary file with mode `0600`.

## Deletion Guard and Audit

Authorized deletion target:

```text
/Users/jinminseong/.codex/sessions/2026/06/18/rollout-2026-06-18T00-11-35-019ed623-4296-79f2-a751-ff3cec538edc.jsonl
```

Required controls:

- Confirm `lsof` has no open handle to the exact target.
- Record a sorted path/inode manifest for every other
  `/Users/jinminseong/.codex/sessions` file.
- Delete with one exact absolute path after `--`; never use a wildcard.
- Confirm the source path is absent.
- Re-verify the lossless archive and decompressed source hash.
- Recreate the other-session manifest and require every preexisting path/inode
  entry to remain. A byte-exact match is expected only if Codex creates no new
  session files during the audit.

## Completed Deletion Audit

| Field | Value |
| --- | --- |
| Status | `completed` |
| Verified absent UTC | `2026-08-08T16:07:33Z` |
| Delete command scope | One exact absolute path after `--`; no wildcard |
| Target open handles before deletion | `0` |
| Target original inode | `232250270` |
| Other session files before deletion | `6,180` |
| Other session files in immediate post-delete snapshot | `6,182` |
| Preexisting paths missing or inode-changed | `0` |
| New session files in immediate post-delete snapshot | `2` |
| Post-delete archive SHA match | `true` |
| Post-delete decompressed SHA match | `true` |

Audit evidence is Git-ignored, mode `0600`, and stored beside the archive:

| File | SHA-256 |
| --- | --- |
| `other-sessions-before.tsv` | `174ce92a10a8eb4a70b8d058ac2c0492c95feffa7091f003e73f36677c015a7e` |
| `other-sessions-after.tsv` | `731cf094d127931c9af1cf7376126f3813de7b24ec97978b96258f5736a269a5` |
| `other-sessions-added.tsv` | `cd33ee7775d328938b3195db2e1fb9ee7b4550f4f9a06ab0052a0d32967822d9` |
| `other-sessions-missing-or-changed.tsv` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The missing-or-changed manifest is empty. The two immediate additions were
created by Codex and were not modified or deleted. Subsequent low-memory,
serial verification also found zero missing or inode-changed preexisting
sessions; the runtime continued to create additional new session files, which
were likewise left untouched.

## Git Secret Audit

- The exact and semantic archives are ignored by `.gitignore` and absent from
  `git ls-files`.
- The sanitized semantic archives contain zero matches for the configured
  OpenAI, GitHub, bearer, AWS, and private-key patterns.
- The tracked worktree, every reachable Git commit/ref, and every local Git
  blob including unreachable objects contain zero matching key patterns.
- The live remote has only `refs/heads/main` at
  `0566d19afd4c27662a069419eb92855c0384bafd`, matching the locally scanned
  `origin/main`.

Deletion status: completed and verified.
