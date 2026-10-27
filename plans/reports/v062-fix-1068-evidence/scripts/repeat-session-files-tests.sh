#!/bin/zsh
# Usage: repeat-session-files-tests.sh <session_files test binary> <runs>
# Runs the compiled test binary <runs> times with a scratch HOME and TMPDIR (removed afterwards) and prints each result line.
set -u
BIN="$1"; RUNS="$2"
S="$(mktemp -d "${TMPDIR:-/tmp}/repeat.XXXXXX")"
for i in $(seq 1 "$RUNS"); do
  env HOME="$S" TMPDIR="$S/" "$BIN" 2>&1 | grep "^test result" | sed "s/^/run $i: /"
done
rm -rf "$S"
echo "scratch_removed=$([ -e "$S" ] && echo no || echo yes)"
