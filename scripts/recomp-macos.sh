#!/bin/sh
set -eu

# Run the upstream BloodyRoar2Recomp local code-generation flow without
# copying a disc image or generated game C into this repository.  The script
# is intentionally usable on macOS and Linux; the upstream CMake project owns
# the host-specific compiler/linker details.

usage() {
    cat >&2 <<'EOF'
usage: scripts/recomp-macos.sh [--dry-run] --project-root PATH --disc PATH \
       [--build-dir PATH] [--config FILE] [--target TARGET] \
       [--disc-us PATH] [--disc-japan PATH]

The project root must be a caller-owned BloodyRoar2Recomp checkout.  The disc
image and generated C remain there (or in another caller-owned path), never in
BloodyRoar2-gym or its GitHub artifacts.

The default config follows the target: game.toml for psx-runtime,
game_us.toml for psx-runtime-us, and game_japan.toml for psx-runtime-japan.
The psx-runtime-japan target enables BR2_BUILD_JAPAN automatically. The
psx-runtime-universal target generates all three regions in one run; --disc is
the EU image and --disc-us/--disc-japan provide the USA and Japan images.
EOF
    exit 2
}

normalize_path() {
    path=$1
    case "$path" in
        /*) ;;
        *) path="$(pwd -P)/$path" ;;
    esac
    if [ -e "$path" ]; then
        if command -v realpath >/dev/null 2>&1; then
            realpath "$path"
        else
            python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$path"
        fi
        return
    fi
    parent=$(dirname "$path")
    base=$(basename "$path")
    if [ -d "$parent" ]; then
        printf '%s/%s\n' "$(CDPATH= cd -- "$parent" && pwd -P)" "$base"
    else
        printf '%s\n' "$path"
    fi
}

dry_run=0
project_root=
disc=
disc_us=
disc_japan=
build_dir=
config=
target=psx-runtime

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run) dry_run=1 ;;
        --project-root)
            [ "$#" -ge 2 ] || usage
            project_root=$2
            shift
            ;;
        --disc)
            [ "$#" -ge 2 ] || usage
            disc=$2
            shift
            ;;
        --disc-us)
            [ "$#" -ge 2 ] || usage
            disc_us=$2
            shift
            ;;
        --disc-japan)
            [ "$#" -ge 2 ] || usage
            disc_japan=$2
            shift
            ;;
        --build-dir)
            [ "$#" -ge 2 ] || usage
            build_dir=$2
            shift
            ;;
        --config)
            [ "$#" -ge 2 ] || usage
            config=$2
            shift
            ;;
        --target)
            [ "$#" -ge 2 ] || usage
            target=$2
            shift
            ;;
        *) usage ;;
    esac
    shift
done

[ -n "$project_root" ] && [ -n "$disc" ] || usage
exe_basename=BloodyRoar2_Recompiled
case "$target" in
    psx-runtime)
        exe_basename=BloodyRoar2_Recompiled
        [ -n "$config" ] || config=game.toml
        ;;
    psx-runtime-us)
        exe_basename=BloodyRoar2_Recompiled_USA
        [ -n "$config" ] || config=game_us.toml
        ;;
    psx-runtime-japan)
        exe_basename=BloodyRoar2_Recompiled_Japan
        [ -n "$config" ] || config=game_japan.toml
        ;;
    psx-runtime-universal)
        exe_basename=BloodyRoar2_Recompiled
        [ -n "$config" ] || config=game.toml
        [ -n "$disc_us" ] && [ -n "$disc_japan" ] || {
            echo "psx-runtime-universal requires --disc-us and --disc-japan in addition to the EU --disc" >&2
            exit 2
        }
        ;;
    *)
        echo "unsupported Recomp target: $target" >&2
        exit 2
        ;;
esac

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd -P)
project_root=$(normalize_path "$project_root")
if [ -n "$build_dir" ]; then
    case "$build_dir" in
        /*) ;;
        *) build_dir="$project_root/$build_dir" ;;
    esac
else
    build_dir="$project_root/build-release"
fi
build_dir=$(normalize_path "$build_dir")
case "$config" in
    /*) config_path=$config ;;
    *) config_path="$project_root/$config" ;;
esac
config_path=$(normalize_path "$config_path")
if [ "$project_root" = "$repo_root" ] ||
   case "$project_root" in "$repo_root"/*) true ;; *) false ;; esac ||
   [ "$build_dir" = "$repo_root" ] ||
   case "$build_dir" in "$repo_root"/*) true ;; *) false ;; esac; then
    echo "refusing to generate proprietary code in bloodyroar2-gym itself" >&2
    exit 2
fi

case "$(printf '%s' "$disc" | tr '[:upper:]' '[:lower:]')" in
    *.bin|*.cue|*.iso|*.chd) ;;
    *) echo "disc must end in .bin, .cue, .iso, or .chd: $disc" >&2; exit 2 ;;
esac

disc=$(normalize_path "$disc")
if [ "$target" = "psx-runtime-universal" ]; then
    for regional_disc in "$disc_us" "$disc_japan"; do
        case "$(printf '%s' "$regional_disc" | tr '[:upper:]' '[:lower:]')" in
            *.bin|*.cue|*.iso|*.chd) ;;
            *) echo "regional disc must end in .bin, .cue, .iso, or .chd: $regional_disc" >&2; exit 2 ;;
        esac
    done
    disc_us=$(normalize_path "$disc_us")
    disc_japan=$(normalize_path "$disc_japan")
fi

reject_repo_path() {
    candidate=$1
    case "$candidate" in
        "$repo_root"|"$repo_root"/*) echo "path must stay outside bloodyroar2-gym: $candidate" >&2; exit 2 ;;
    esac
}

reject_repo_path "$disc"
if [ "$target" = "psx-runtime-universal" ]; then
    reject_repo_path "$disc_us"
    reject_repo_path "$disc_japan"
fi

if [ "$dry_run" -eq 0 ]; then
    [ -d "$project_root" ] || { echo "project root is not a directory: $project_root" >&2; exit 2; }
    [ -f "$project_root/psxrecomp/psxrecomp_cli.py" ] || {
        echo "missing upstream psxrecomp CLI under $project_root" >&2
        exit 2
    }
    [ -f "$disc" ] || { echo "disc is not a regular file: $disc" >&2; exit 2; }
    if [ "$target" = "psx-runtime-universal" ]; then
        [ -f "$disc_us" ] || { echo "USA disc is not a regular file: $disc_us" >&2; exit 2; }
        [ -f "$disc_japan" ] || { echo "Japan disc is not a regular file: $disc_japan" >&2; exit 2; }
    fi
fi

run() {
    if [ "$dry_run" -eq 1 ]; then
        printf '+ '
        printf '%s ' "$@"
        printf '\n'
    else
        echo "+ $*" >&2
        "$@"
    fi
}

run git -C "$project_root" submodule update --init --recursive
if [ "$dry_run" -eq 1 ]; then
    printf '+ (cd '
    printf '%s' "$project_root"
    printf ' && ./psxrecomp/tools/ci/build_emitters.sh)\n'
else
    (
        cd "$project_root"
        run ./psxrecomp/tools/ci/build_emitters.sh
    )
fi
run python3 "$project_root/psxrecomp/psxrecomp_cli.py" verify-disc \
    --config "$config_path" --project-root "$project_root" --disc "$disc"
run python3 "$project_root/psxrecomp/psxrecomp_cli.py" generate \
    --config "$config_path" --project-root "$project_root" --disc "$disc"
if [ "$target" = "psx-runtime-universal" ]; then
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" verify-disc \
        --config "$project_root/game_us.toml" --project-root "$project_root" --disc "$disc_us"
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" generate \
        --config "$project_root/game_us.toml" --project-root "$project_root" --disc "$disc_us"
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" verify-disc \
        --config "$project_root/game_japan.toml" --project-root "$project_root" --disc "$disc_japan"
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" generate \
        --config "$project_root/game_japan.toml" --project-root "$project_root" --disc "$disc_japan"
fi

if [ "$target" = "psx-runtime-japan" ]; then
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" rebuild \
        --config "$config_path" --project-root "$project_root" \
        --build-dir "$build_dir" --target "$target" \
        --exe-basename "$exe_basename" --no-pgo \
        --cmake-extra=-DPSX_DEBUG_TOOLS=ON \
        --cmake-extra=-DBR2_BUILD_JAPAN=ON
elif [ "$target" = "psx-runtime-universal" ]; then
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" rebuild \
        --config "$config_path" --project-root "$project_root" \
        --build-dir "$build_dir" --target "$target" \
        --exe-basename "$exe_basename" --no-pgo \
        --cmake-extra=-DPSX_DEBUG_TOOLS=ON \
        --cmake-extra=-DBR2_BUILD_UNIVERSAL=ON
else
    run python3 "$project_root/psxrecomp/psxrecomp_cli.py" rebuild \
        --config "$config_path" --project-root "$project_root" \
        --build-dir "$build_dir" --target "$target" \
        --exe-basename "$exe_basename" --no-pgo \
        --cmake-extra=-DPSX_DEBUG_TOOLS=ON
fi

if [ "$dry_run" -eq 1 ]; then
    printf '%s\n' '{"status":"dry-run","tracked_outputs":false,"disc_not_copied":true,"generated_code_not_copied":true}'
else
    printf '%s\n' "{\"status\":\"complete\",\"project_root\":\"$project_root\",\"build_dir\":\"$build_dir\",\"executable\":\"$exe_basename\",\"tracked_outputs\":false}"
fi
