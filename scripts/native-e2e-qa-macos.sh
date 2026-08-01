#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

MODE="all"
ROM_CACHE=""
STAMP=${BR2_E2E_QA_STAMP:-$(date +%Y%m%d-%H%M%S)}
OUT_DIR=${BR2_E2E_QA_OUT_DIR:-"$ROOT_DIR/tmp/native-e2e-qa-$STAMP"}
CLEANUP="never"
BUILD=1
DRY_RUN=0
SMOKE_IPF=${BR2_E2E_QA_SMOKE_IPF:-500000}
LIVE_IPF=${BR2_E2E_QA_LIVE_IPF:-600000}
LIVE_MAX_FRAMES=${BR2_LIVE_QA_MAX_FRAMES:-4200}
GUI_CAPTURE_INTERVAL=${BR2_NATIVE_GUI_CAPTURE_INTERVAL:-15}
SMOKE_WALL_TIMEOUT_SECS=${BR2_NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS:-600}
WINDOW_SCALE=${BR2_E2E_QA_WINDOW_SCALE:-1}
SMOKE_SELECT_CONFIRM_WAIT_FRAMES=${BR2_E2E_QA_SMOKE_SELECT_CONFIRM_WAIT_FRAMES:-420}
SMOKE_GAMEPLAY_WAIT_FRAMES=${BR2_E2E_QA_SMOKE_GAMEPLAY_WAIT_FRAMES:-1500}

usage() {
    cat <<'USAGE'
Usage:
  scripts/native-e2e-qa-macos.sh --rom-cache <cache-rom-dir> [options]
  scripts/native-e2e-qa-macos.sh <cache-rom-dir> [options]

Modes:
  --mode smoke|live|all       Run headless timeline, visible GUI/CoreAudio, or both. Default: all.

Required:
  --rom-cache <dir>           Materialized native ROM cache directory. ROM ZIPs are not copied.

Options:
  --out-dir <dir>             Artifact/summary directory. Default: tmp/native-e2e-qa-<stamp>.
  --cleanup never|on-pass|always
                              Delete bulky artifact files after summary. Default: never.
  --smoke-ipf <n>             native-match-tail-timeline instructions per frame. Default: 500000.
  --live-ipf <n>              native-play instructions per frame. Default: 600000.
  --live-max-frames <n>       Bounded GUI frames. Default: BR2_LIVE_QA_MAX_FRAMES or 4200.
  --capture-interval <n>      GUI capture interval in frames. Default: 15.
  --window-scale <scale>      native-play scale argument. Default: 1.
  --no-build                  Use target/release/bloodyroar2-gym as-is.
  --dry-run                   Print commands only; do not build, run, or write artifacts.
  -h, --help                  Show this help.

Environment overrides:
  BR2_E2E_QA_SMOKE_SCRIPT     Space-separated action:frames script for smoke.
  BR2_E2E_QA_LIVE_SCRIPT      Space-separated action:frames script for live GUI test input.
  BR2_E2E_QA_SMOKE_SELECT_CONFIRM_WAIT_FRAMES
                              Wait after select confirmation before P2 join input. Default: 420.
  BR2_E2E_QA_SMOKE_GAMEPLAY_WAIT_FRAMES
                              Wait before gameplay-only smoke controls. Default: 1500.

Representative checks:
  smoke: PNG stages, P1/P2 input counters, missed_vblank, OTC recovery/model state.
  live: GUI captures, CoreAudio health, GUI input verification, performance, final render gates.
USAGE
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --mode)
            MODE=${2:-}
            shift 2
            ;;
        --mode=*)
            MODE=${1#*=}
            shift
            ;;
        --rom-cache)
            ROM_CACHE=${2:-}
            shift 2
            ;;
        --rom-cache=*)
            ROM_CACHE=${1#*=}
            shift
            ;;
        --out-dir)
            OUT_DIR=${2:-}
            shift 2
            ;;
        --out-dir=*)
            OUT_DIR=${1#*=}
            shift
            ;;
        --cleanup)
            CLEANUP=${2:-}
            shift 2
            ;;
        --cleanup=*)
            CLEANUP=${1#*=}
            shift
            ;;
        --smoke-ipf)
            SMOKE_IPF=${2:-}
            shift 2
            ;;
        --smoke-ipf=*)
            SMOKE_IPF=${1#*=}
            shift
            ;;
        --live-ipf)
            LIVE_IPF=${2:-}
            shift 2
            ;;
        --live-ipf=*)
            LIVE_IPF=${1#*=}
            shift
            ;;
        --live-max-frames)
            LIVE_MAX_FRAMES=${2:-}
            shift 2
            ;;
        --live-max-frames=*)
            LIVE_MAX_FRAMES=${1#*=}
            shift
            ;;
        --capture-interval)
            GUI_CAPTURE_INTERVAL=${2:-}
            shift 2
            ;;
        --capture-interval=*)
            GUI_CAPTURE_INTERVAL=${1#*=}
            shift
            ;;
        --window-scale)
            WINDOW_SCALE=${2:-}
            shift 2
            ;;
        --window-scale=*)
            WINDOW_SCALE=${1#*=}
            shift
            ;;
        --no-build)
            BUILD=0
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            die "unknown option: $1"
            ;;
        *)
            if [ -z "$ROM_CACHE" ]; then
                ROM_CACHE=$1
                shift
            else
                die "unexpected positional argument: $1"
            fi
            ;;
    esac
done

case "$MODE" in
    smoke|live|all) ;;
    *) die "--mode must be smoke, live, or all" ;;
esac
case "$CLEANUP" in
    never|on-pass|always) ;;
    *) die "--cleanup must be never, on-pass, or always" ;;
esac
[ -n "$ROM_CACHE" ] || die "--rom-cache <cache-rom-dir> is required"
if [ "$DRY_RUN" -eq 0 ] && [ ! -d "$ROM_CACHE" ]; then
    die "ROM cache path must be an existing directory: $ROM_CACHE"
fi

BIN="$ROOT_DIR/target/release/bloodyroar2-gym"
ARTIFACT_ROOT="$OUT_DIR/artifacts"
COMMANDS_LOG="$OUT_DIR/commands.txt"
BUILD_STDOUT="$ARTIFACT_ROOT/cargo-build.stdout.log"
BUILD_STDERR="$ARTIFACT_ROOT/cargo-build.stderr.log"
SMOKE_DIR="$ARTIFACT_ROOT/smoke"
SMOKE_PREFIX="$SMOKE_DIR/timeline"
SMOKE_STDOUT="$SMOKE_DIR/native-match-tail-timeline.stdout.json"
SMOKE_STDERR="$SMOKE_DIR/native-match-tail-timeline.stderr.log"
LIVE_DIR="$ARTIFACT_ROOT/live"
LIVE_CAPTURE_DIR="$LIVE_DIR/captures"
LIVE_STDOUT="$LIVE_DIR/native-play.stdout.json"
LIVE_STDERR="$LIVE_DIR/native-play.stderr.log"

SMOKE_SEGMENTS=(
    coin:18 noop:24 start:24 noop:240
    punch:36 "noop:${SMOKE_SELECT_CONFIRM_WAIT_FRAMES}"
    p2+coin:18 noop:24 p2+start:24 "noop:${SMOKE_GAMEPLAY_WAIT_FRAMES}"
    right:24 noop:18 down:24 noop:18 left:24 noop:18 up:24 noop:18
    punch:36 noop:60 kick:36 noop:60 beast:36 noop:240 guard:36 noop:120
    p2+right:24 noop:18 p2+down:24 noop:18 p2+left:24 noop:18 p2+up:24 noop:18
    p2+punch:36 noop:60 p2+kick:36 noop:60 p2+beast:36 noop:240 p2+guard:36 noop:180
)
LIVE_SEGMENTS=("${SMOKE_SEGMENTS[@]}")

if [ -n "${BR2_E2E_QA_SMOKE_SCRIPT:-}" ]; then
    # shellcheck disable=SC2206
    SMOKE_SEGMENTS=(${BR2_E2E_QA_SMOKE_SCRIPT})
fi
if [ -n "${BR2_E2E_QA_LIVE_SCRIPT:-}" ]; then
    # shellcheck disable=SC2206
    LIVE_SEGMENTS=(${BR2_E2E_QA_LIVE_SCRIPT})
fi

script_required_frames() {
    local total=1
    local segment frames
    for segment in "$@"; do
        frames=${segment##*:}
        case "$frames" in
            ''|*[!0-9]*)
                die "invalid scripted input segment: $segment"
                ;;
        esac
        total=$((total + frames))
    done
    printf '%s\n' "$total"
}

LIVE_REQUIRED_FRAMES=$(script_required_frames "${LIVE_SEGMENTS[@]}")
if { [ "$MODE" = "live" ] || [ "$MODE" = "all" ]; } \
    && [ "$LIVE_MAX_FRAMES" -lt "$LIVE_REQUIRED_FRAMES" ]; then
    die "--live-max-frames must be at least $LIVE_REQUIRED_FRAMES for the configured GUI input script"
fi

quote_command() {
    local part
    for part in "$@"; do
        printf '%q ' "$part"
    done
    printf '\n'
}

record_command() {
    local label=$1
    shift
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '[dry-run] %s: ' "$label"
        quote_command "$@"
    else
        printf '%s: ' "$label" >>"$COMMANDS_LOG"
        quote_command "$@" >>"$COMMANDS_LOG"
    fi
}

run_logged() {
    local label=$1
    local stdout_log=$2
    local stderr_log=$3
    shift 3
    record_command "$label" "$@"
    if [ "$DRY_RUN" -eq 1 ]; then
        return 0
    fi
    mkdir -p "$(dirname -- "$stdout_log")" "$(dirname -- "$stderr_log")"
    "$@" >"$stdout_log" 2>"$stderr_log"
}

if [ "$DRY_RUN" -eq 1 ]; then
    printf 'mode: %s\nrom_cache: %s\nout_dir: %s\ncleanup: %s\n' \
        "$MODE" "$ROM_CACHE" "$OUT_DIR" "$CLEANUP"
    if [ "$BUILD" -eq 1 ]; then
        record_command "cargo-build" cargo build --release
    fi
    if [ "$MODE" = "smoke" ] || [ "$MODE" = "all" ]; then
        record_command "smoke" \
            env "BR2_NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS=$SMOKE_WALL_TIMEOUT_SECS" \
            "$BIN" native-match-tail-timeline "$ROM_CACHE" "$SMOKE_IPF" "$SMOKE_PREFIX" \
            "${SMOKE_SEGMENTS[@]}"
        printf '[dry-run] smoke stdout -> %s\n' "$SMOKE_STDOUT"
        printf '[dry-run] smoke stderr -> %s\n' "$SMOKE_STDERR"
    fi
    if [ "$MODE" = "live" ] || [ "$MODE" = "all" ]; then
        record_command "live" \
            env \
            "BR2_NATIVE_GUI_CAPTURE_DIR=$LIVE_CAPTURE_DIR" \
            "BR2_NATIVE_GUI_CAPTURE_INTERVAL=$GUI_CAPTURE_INTERVAL" \
            "BR2_NATIVE_GUI_DEEP_CAPTURE=${BR2_NATIVE_GUI_DEEP_CAPTURE:-1}" \
            "BR2_NATIVE_TRACE_GUI_INPUT=${BR2_NATIVE_TRACE_GUI_INPUT:-1}" \
            "BR2_NATIVE_GUI_TEST_EXCLUSIVE_INPUT=${BR2_NATIVE_GUI_TEST_EXCLUSIVE_INPUT:-1}" \
            "BR2_NATIVE_GUI_WALL_TIMEOUT_SECS=${BR2_NATIVE_GUI_WALL_TIMEOUT_SECS:-300}" \
            "$BIN" native-play "$ROM_CACHE" "$LIVE_IPF" "$WINDOW_SCALE" "$LIVE_MAX_FRAMES" \
            --gui-test-input "${LIVE_SEGMENTS[@]}"
        printf '[dry-run] live stdout -> %s\n' "$LIVE_STDOUT"
        printf '[dry-run] live stderr -> %s\n' "$LIVE_STDERR"
        printf '[dry-run] live captures -> %s\n' "$LIVE_CAPTURE_DIR"
    fi
    exit 0
fi

mkdir -p "$ARTIFACT_ROOT"
: >"$COMMANDS_LOG"

BUILD_STATUS=0
if [ "$BUILD" -eq 1 ]; then
    set +e
    run_logged "cargo-build" "$BUILD_STDOUT" "$BUILD_STDERR" cargo build --release
    BUILD_STATUS=$?
    set -e
fi

SMOKE_STATUS=-1
LIVE_STATUS=-1
if [ "$BUILD_STATUS" -eq 0 ] && { [ "$MODE" = "smoke" ] || [ "$MODE" = "all" ]; }; then
    set +e
    run_logged "smoke" "$SMOKE_STDOUT" "$SMOKE_STDERR" \
        env "BR2_NATIVE_PLAY_SNAPSHOT_WALL_TIMEOUT_SECS=$SMOKE_WALL_TIMEOUT_SECS" \
        "$BIN" native-match-tail-timeline "$ROM_CACHE" "$SMOKE_IPF" "$SMOKE_PREFIX" \
        "${SMOKE_SEGMENTS[@]}"
    SMOKE_STATUS=$?
    set -e
fi

if [ "$BUILD_STATUS" -eq 0 ] && { [ "$MODE" = "live" ] || [ "$MODE" = "all" ]; }; then
    mkdir -p "$LIVE_CAPTURE_DIR"
    set +e
    run_logged "live" "$LIVE_STDOUT" "$LIVE_STDERR" \
        env \
        "BR2_NATIVE_GUI_CAPTURE_DIR=$LIVE_CAPTURE_DIR" \
        "BR2_NATIVE_GUI_CAPTURE_INTERVAL=$GUI_CAPTURE_INTERVAL" \
        "BR2_NATIVE_GUI_DEEP_CAPTURE=${BR2_NATIVE_GUI_DEEP_CAPTURE:-1}" \
        "BR2_NATIVE_TRACE_GUI_INPUT=${BR2_NATIVE_TRACE_GUI_INPUT:-1}" \
        "BR2_NATIVE_GUI_TEST_EXCLUSIVE_INPUT=${BR2_NATIVE_GUI_TEST_EXCLUSIVE_INPUT:-1}" \
        "BR2_NATIVE_GUI_WALL_TIMEOUT_SECS=${BR2_NATIVE_GUI_WALL_TIMEOUT_SECS:-300}" \
        "$BIN" native-play "$ROM_CACHE" "$LIVE_IPF" "$WINDOW_SCALE" "$LIVE_MAX_FRAMES" \
        --gui-test-input "${LIVE_SEGMENTS[@]}"
    LIVE_STATUS=$?
    set -e
fi

set +e
python3 "$ROOT_DIR/scripts/native_e2e_qa_summary.py" \
    --mode "$MODE" \
    --out-dir "$OUT_DIR" \
    --artifact-root "$ARTIFACT_ROOT" \
    --cleanup "$CLEANUP" \
    --build-status "$BUILD_STATUS" \
    --smoke-status "$SMOKE_STATUS" \
    --smoke-stdout "$SMOKE_STDOUT" \
    --smoke-stderr "$SMOKE_STDERR" \
    --smoke-prefix "$SMOKE_PREFIX" \
    --live-status "$LIVE_STATUS" \
    --live-stdout "$LIVE_STDOUT" \
    --live-stderr "$LIVE_STDERR" \
    --live-capture-dir "$LIVE_CAPTURE_DIR" \
    --commands-log "$COMMANDS_LOG"
SUMMARY_STATUS=$?
set -e
exit "$SUMMARY_STATUS"
