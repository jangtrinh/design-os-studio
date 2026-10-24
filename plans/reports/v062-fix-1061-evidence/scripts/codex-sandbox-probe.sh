#!/bin/bash
# Codex sandbox probe for #1061: an agent in a real Codex workspace-write sandbox (`codex sandbox -P :workspace`, no model turn,
# no login, scratch HOME / CODEX_HOME / TMPDIR) plants a link inside its own session folder, which Studio's test build made.
# Studio's rewrite of the session files is then run (the `probe_*` steps of codex_probe_tests.rs) and must be refused; the victim
# folder the link points at must stay byte for byte as it was.
# Usage: codex-sandbox-probe.sh <test binary: out/cargo-1061/debug/deps/designos-...>
# Scratch lives in the worktree's out/ folder, not under /tmp or the real TMPDIR; paths in the log are redacted to <scratch>.
set -u
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
BIN="$1"
S="$WT/out/probe-1061"; rm -rf "$S"
V="$S/home/.gemini"                              # the victim: stands for the user's ~/.gemini
SESS="$S/lib/agent-sessions/file-1"              # the session folder Studio makes (a library file's, so the test build knows it)
mkdir -p "$V" "$S/codex-home" "$S/test-home" "$S/tmp" "$S/lib/agent-sessions"
printf 'USER-SETTINGS\n' > "$V/settings.json"; printf 'USER-CREDENTIALS\n' > "$V/oauth_creds.json"
red() { sed "s#$S#<scratch>#g; s#$WT#<worktree>#g; s#$HOME#~#g; s#$(id -un)#user#g"; }
victim_state() { (cd "$V" && ls -A | sort | tr '\n' ' '; shasum -a 256 settings.json oauth_creds.json | awk '{print substr($1,1,12)}' | tr '\n' ' '); echo; }
sandboxed() { env HOME="$S/home" CODEX_HOME="$S/codex-home" TMPDIR="$S/tmp/" codex sandbox -P :workspace -C "$SESS" -- "$@"; }
studio() { env HOME="$S/test-home" TMPDIR="$S/tmp/" GEMINI_CLI_HOME="$S/test-home" CODEX_HOME="$S/test-home/.codex" CLAUDE_CONFIG_DIR="$S/test-home/.claude" \
  DESIGNOS_PROBE_SESSION="$SESS" "$BIN" --ignored --exact "agent_session_files::codex_probe_tests::$1" --test-threads=1 --nocapture 2>&1 | grep -o 'PROBE.*'; }
{
echo "tree: ${PROBE_TREE:-$(git -C "$WT" rev-parse HEAD)}  binary: ${BIN##*/}  $(env HOME="$S/home" CODEX_HOME="$S/codex-home" TMPDIR="$S/tmp/" codex --version 2>&1 | head -1)"
before="$(victim_state)"; echo "victim before: $before"
echo "== Studio makes the session folder (test build)"; studio probe_make_session_folder
echo "-- control: the sandboxed agent cannot write the victim"
sandboxed /usr/bin/touch "$V/agent-was-here"; echo "touch victim exit=$? (must not be 0)"
sandboxed /bin/rm "$V/settings.json"; echo "rm victim file exit=$? (must not be 0)"
echo "== A. the agent plants .gemini -> victim in its session folder"
sandboxed /bin/ln -s "$V" "$SESS/.gemini"; echo "ln -s exit=$? (0 = the sandbox allows it)"
ls -la "$SESS" | tail -n +2
echo "== A. Studio rewrites the session files"; studio probe_write_session_files
echo "victim after A: $(victim_state)"
echo "== B. the folder was real first: the agent moves .gemini away and plants the link (after Studio provisioned it)"
rm "$SESS/.gemini"; studio probe_write_session_files
sandboxed /bin/mv "$SESS/.gemini" "$SESS/.gemini.old"; echo "mv exit=$?"
sandboxed /bin/ln -s "$V" "$SESS/.gemini"; echo "ln -s exit=$?"
echo "== B. Studio rewrites the session files"; studio probe_write_session_files
echo "victim after B: $(victim_state)"
echo "== C. the agent plants links at .grok and .agents/plugins too"
rm -f "$SESS/.gemini"
sandboxed /bin/mv "$SESS/.grok" "$SESS/.grok.old"; sandboxed /bin/ln -s "$V" "$SESS/.grok"; echo "ln -s .grok exit=$?"
sandboxed /bin/mv "$SESS/.agents/plugins" "$SESS/.plugins.old"; echo "mv .agents/plugins exit=$? (non-zero: the Codex sandbox keeps .agents read-only)"
mv "$SESS/.agents/plugins" "$SESS/.plugins.old"; ln -s "$V" "$SESS/.agents/plugins"; echo "(planted by the host instead, to see that Studio refuses it too)"
echo "== C. Studio rewrites the session files"; studio probe_write_session_files
echo "victim after C: $(victim_state)"
echo "== D. a hard link to the victim's file under a name Studio writes (links removed, real folders back)"
rm -f "$SESS/.grok" "$SESS/.agents/plugins"; mv "$SESS/.grok.old" "$SESS/.grok"; mv "$SESS/.plugins.old" "$SESS/.agents/plugins"
sandboxed /bin/ln "$V/settings.json" "$SESS/AGENTS.md.hardlink"; echo "hard link into the session folder exit=$? (0 = the sandbox allows a hard link to a file outside it)"
rm -f "$SESS/AGENTS.md"; ln "$V/settings.json" "$SESS/AGENTS.md"
echo "AGENTS.md before Studio's rewrite: $(head -c 14 "$SESS/AGENTS.md" | tr '\n' ' ')"
studio probe_write_session_files
echo "AGENTS.md after Studio's rewrite: $(head -c 14 "$SESS/AGENTS.md" | tr '\n' ' ')..."
echo "victim after D: $(victim_state)"
after="$(victim_state)"
[ "$before" = "$after" ] && echo "RESULT: victim identical before and after (PASS)" || echo "RESULT: victim CHANGED (FAIL)"
} 2>&1 | red
rm -rf "$S"
echo "scratch removed: $([ -e "$S" ] && echo NO || echo yes)"
