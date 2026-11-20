#!/bin/sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 <caller-owned-binary> [output_dir]" >&2
    exit 2
fi

target=$1
if [ ! -f "$target" ]; then
    echo "binary is not a regular file: $target" >&2
    exit 2
fi
if ! command -v rea >/dev/null 2>&1; then
    echo "REA is not installed; install a caller-selected REA provider first" >&2
    exit 127
fi

if [ "$#" -eq 2 ]; then
    output_dir=$2
else
    base=$(basename "$target")
    stamp=$(date -u +%Y%m%dT%H%M%SZ)
    output_dir="tmp/rea-analysis/${base}.${stamp}"
fi
mkdir -p "$output_dir"

run_json() {
    name=$1
    shift
    echo "[rea] $name" >&2
    rea "$@" --json >"$output_dir/$name.json"
}

run_json file_headers inspect-with-objdump "$target" file_headers
run_json section_headers inspect-with-objdump "$target" section_headers
run_json symbols inspect-with-objdump "$target" symbols
run_json disassemble inspect-with-objdump "$target" disassemble
run_json rizin_info execute-rizin-command "$target" iI

cat <<EOF
{"target":"$target","output_dir":"$output_dir","status":"complete","tracked":false}
EOF
