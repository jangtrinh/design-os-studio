#!/bin/bash
# Gates for #1069. Every cargo test and every test binary runs with an isolated HOME
# and a scratch TMPDIR with no double slash. Both bins are run in full (CI runs the suite once per bin), and while one bin's full suite
# runs, the race test is run once in the other bin, so it is measured with the machine loaded.
# Logs go to the evidence folder; the scratch path, the worktree and the home folder are redacted.
set -u
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1069-evidence"
cd "$WT" || exit 1
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$WT/out/cargo-1061"
REAL_HOME="$HOME"
T0="${TMPDIR:-/tmp}"; T0="${T0%/}"
S="$(mktemp -d "$T0/b1061.XXXXXX")"; S="${S%/}"
case "$S" in *//*) echo "scratch path has a double slash"; exit 1;; esac
mkdir -p "$S/home" "$S/race-a" "$S/race-b"
red() { sed "s#$S#<scratch>#g; s#$WT#<worktree>#g; s#$REAL_HOME#~#g; s#$(id -un)#user#g"; }
iso() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" \
          CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$@"; }
isorace() { env HOME="$S/home" TMPDIR="$1/" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "${@:2}"; }
mtimes() {
  date '+%Y-%m-%d %H:%M:%S %Z'
  for f in "$REAL_HOME/.gemini/projects.json" "$REAL_HOME/.gemini/oauth_creds.json" "$REAL_HOME/.gemini" "$REAL_HOME/.codex/tmp/arg0-macos"; do
    echo "$(stat -f '%m' "$f" 2>/dev/null || echo missing) ~${f#$REAL_HOME}"
  done
}
RACE=agent_session_files::inner_link_race_tests::no_byte_lands_in_the_target_of_a_link_swapped_in_while_studio_rewrites_the_session_files
git rev-parse HEAD > "$EV/gate-head.txt"; mtimes > "$EV/gate-mtimes-before.txt"

cargo fmt --all -- --check > "$EV/gate-fmt.log" 2>&1; echo "fmt exit=$?" >> "$EV/gate-fmt.log"
cargo clippy -p designos-app --all-targets --locked -- -D warnings 2>&1 | red > "$EV/gate-clippy.log"; echo "clippy exit=${PIPESTATUS[0]}" >> "$EV/gate-clippy.log"
python3 scripts/zero_fake_ui_gate.py > "$EV/gate-zero-fake-ui.log" 2>&1; echo "zero-fake exit=$?" >> "$EV/gate-zero-fake-ui.log"
iso cargo test -p designos-app --bins --locked --no-run > "$S/build.log" 2>&1
grep "Executable" "$S/build.log" | red > "$EV/gate-test-binaries.txt"
BIN_APP="$(grep -o 'out/cargo-1061/debug/deps/designos-[0-9a-f]*' "$S/build.log" | head -1)"
BIN_STUDIO="$(grep -o 'out/cargo-1061/debug/deps/designos_studio-[0-9a-f]*' "$S/build.log" | head -1)"
iso cargo test -p designos-mcp --locked 2>&1 | red > "$EV/gate-test-mcp.log"; echo "mcp exit=${PIPESTATUS[0]}" >> "$EV/gate-test-mcp.log"

loop_race() { # <binary> <label> <scratch tmp> <log>
  : > "$4"
  for i in $(seq 1 1); do
    out="$(isorace "$3" "$WT/$1" --exact "$RACE" --nocapture 2>&1)"; rc=$?
    echo "$2 run $i exit=$rc $(echo "$out" | grep -o 'race: .*' | head -1) $(echo "$out" | grep -o 'a rewrite failed.*' | head -1 | cut -c1-200)" | red >> "$4"
  done
}

# Phase A: the full suite of designos-studio, with the race test looped in the designos bin
( iso cargo test -p designos-app --bin designos-studio --locked 2>&1 | red > "$EV/gate-test-studio.log"; echo "studio exit=${PIPESTATUS[0]}" >> "$EV/gate-test-studio.log" ) &
SUITE=$!
loop_race "$BIN_APP" designos "$S/race-a" "$EV/gate-race-loop-designos.log"
wait $SUITE
# Phase B: the full suite of designos, with the race test looped in the designos-studio bin
( iso cargo test -p designos-app --bin designos --locked 2>&1 | red > "$EV/gate-test-app.log"; echo "app exit=${PIPESTATUS[0]}" >> "$EV/gate-test-app.log" ) &
SUITE=$!
loop_race "$BIN_STUDIO" designos-studio "$S/race-b" "$EV/gate-race-loop-designos-studio.log"
wait $SUITE
rm -rf "$S"
echo "scratch removed: $([ -e "$S" ] && echo NO || echo yes)" >> "$EV/gate-test-app.log"
mtimes > "$EV/gate-mtimes-after.txt"
echo "gates done: 1069"
