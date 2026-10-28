#!/bin/bash
# Loops, in parallel, the four agent_sessions tests that bind a second window with a fallback folder id, to see whether any of them fails.
# Before the fix they shared the id "p1-w2-tab-1"; macos-14 failed second_window_does_not_share_the_folder because of it.
# Usage: fallback-tests-loop.sh <test binary> <label> <runs>. Isolated HOME and a scratch TMPDIR for every run.
set -u
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1061-evidence"
BIN="$1"; TAG="$2"; RUNS="${3:-60}"
REAL_HOME="$HOME"
T0="${TMPDIR:-/tmp}"; T0="${T0%/}"
S="$(mktemp -d "$T0/x.XXXXXX")"; S="${S%/}"
mkdir -p "$S/home"
LOG="$EV/round4-fallback-loop-$TAG.log"
: > "$LOG"
fails=0
for i in $(seq 1 "$RUNS"); do
  out="$(env HOME="$S/home" TMPDIR="$S/" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    "$BIN" agent_sessions::tests::second_window_does_not_share_the_folder agent_sessions::tests::restart_resumes_the_previous_session \
    agent_sessions::tests::save_to_library_as_with_open_agent_tab_binds_stable_dir_and_keeps_session_on_close \
    agent_sessions::tests::save_to_library_as_with_a_tab_on_a_disk_file_keeps_that_files_lock --test-threads=4 2>&1)"
  rc=$?
  [ $rc -ne 0 ] && fails=$((fails+1))
  echo "run $i exit=$rc $(echo "$out" | grep -o 'test result: .*' | head -1 | cut -c1-70) $(echo "$out" | grep -m1 -o '^---- .* stdout') $(echo "$out" | grep -m1 -E 'panicked|left:' | sed "s#$S#<scratch>#g" | cut -c1-200)" >> "$LOG"
done
echo "$TAG: $fails of $RUNS runs failed" | tee -a "$LOG"
rm -rf "$S"
