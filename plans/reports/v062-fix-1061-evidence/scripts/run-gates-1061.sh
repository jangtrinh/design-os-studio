#!/bin/bash
# Final gates for #1061: isolated HOME and a scratch TMPDIR (no double slash) for every cargo test run.
# Logs go to the evidence folder; env values are never printed and the scratch path is redacted to <scratch>.
# Usage: run-gates-1061.sh [fmt|clippy|zero-fake|mcp|app|all]   (default all)
set -u
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1061-evidence"
cd "$WT" || exit 1
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$WT/out/cargo-1061"
REAL_HOME="$HOME"
T0="${TMPDIR:-/tmp}"; T0="${T0%/}"
S="$(mktemp -d "$T0/b1061.XXXXXX")"; S="${S%/}"
case "$S" in *//*) echo "scratch path has a double slash"; exit 1;; esac
mkdir -p "$S/home"
red() { sed "s#$S#<scratch>#g; s#$WT#<worktree>#g; s#$HOME#~#g; s#$(id -un)#user#g"; }
iso() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" \
          CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$@"; }
mtimes() { date '+%Y-%m-%d %H:%M:%S %Z'; for f in "$REAL_HOME/.gemini/projects.json" "$REAL_HOME/.gemini/oauth_creds.json"; do echo "$(stat -f '%m' "$f" 2>/dev/null || echo missing) ~/.gemini/${f##*/}"; done; }
WHAT="${1:-all}"
[ "$WHAT" = all ] && { git rev-parse HEAD > "$EV/gate-head.txt"; mtimes > "$EV/mtimes-before.txt"; }
if [ "$WHAT" = all ] || [ "$WHAT" = fmt ]; then
  cargo fmt --all -- --check > "$EV/gate-fmt.log" 2>&1; echo "fmt exit=$?" >> "$EV/gate-fmt.log"
fi
if [ "$WHAT" = all ] || [ "$WHAT" = clippy ]; then
  cargo clippy -p designos-app --all-targets --locked -- -D warnings > "$EV/gate-clippy.log" 2>&1; echo "clippy exit=$?" >> "$EV/gate-clippy.log"
fi
if [ "$WHAT" = all ] || [ "$WHAT" = zero-fake ]; then
  python3 scripts/zero_fake_ui_gate.py > "$EV/gate-zero-fake-ui.log" 2>&1; echo "zero-fake exit=$?" >> "$EV/gate-zero-fake-ui.log"
fi
if [ "$WHAT" = all ] || [ "$WHAT" = mcp ]; then
  iso cargo test -p designos-mcp --locked 2>&1 | red > "$EV/gate-test-mcp.log"; echo "mcp exit=${PIPESTATUS[0]}" >> "$EV/gate-test-mcp.log"
fi
if [ "$WHAT" = all ] || [ "$WHAT" = app ]; then
  iso cargo test -p designos-app --bin designos --locked 2>&1 | red > "$EV/gate-test-app.log"; echo "app exit=${PIPESTATUS[0]}" >> "$EV/gate-test-app.log"
  echo "scratch test root after the run: $(ls -1 "$S/designos test data" 2>/dev/null | tr '\n' ' ')" >> "$EV/gate-test-app.log"
fi
rm -rf "$S"
echo "scratch removed: $([ -e "$S" ] && echo NO || echo yes)" >> "$EV/gate-test-app.log"
[ "$WHAT" = all ] && mtimes > "$EV/mtimes-after.txt"
echo "gates done: $WHAT"
