#!/bin/zsh
# Usage: run-isolated.sh <worktree> <log> <cargo args...>
# Runs cargo in the worktree with an isolated HOME/TMPDIR scratch that is removed afterwards.
# No agent CLI is started; the real HOME is never the HOME of the cargo process.
set -u
WT="$1"; LOG="$2"; shift 2
export PATH="$HOME/.cargo/bin:$PATH"
cd "$WT" || exit 9
export CARGO_TARGET_DIR="$WT/out/cargo-1068"
REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo "$@" > "$LOG" 2>&1
rc=$?
rm -rf "$S"
echo "exit=$rc scratch_removed=$([ -e "$S" ] && echo no || echo yes)" >> "$LOG"
exit $rc
