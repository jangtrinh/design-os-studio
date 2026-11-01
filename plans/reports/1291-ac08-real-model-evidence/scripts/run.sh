#!/bin/bash
# usage: run.sh <model: grok|sonnet> <arm: with-hints|stripped-hints|stripped-hints-rules> <scenario: path|unsaved> <n> [slot]
# One real-model trial with designos-mcp behind proxy.py. Output goes to $RESULTS/<model>/<arm>-<scenario>-<n>/ (default /workspace/dev/ac08/results).
#   grok   : `grok -p` through maintenance/tools/run-cli.sh (bwrap sandbox); the wire log is declared as an expected output of the run.
#   sonnet : `claude -p --model sonnet` with only the designos MCP tools and Write enabled (no shell), project config ignored (--strict-mcp-config).
# `slot` names the scratch folder, so several trials can run side by side (default 1).
set -euo pipefail
model=$1 arm=$2 scen=$3 n=$4 slot=${5:-1}
here=$(cd "$(dirname "$0")" && pwd)
MCP=${DESIGNOS_MCP:-/workspace/dev/v071-target/debug/designos-mcp}
base=/workspace/dev/ac08
RESULTS=${RESULTS:-$base/results}
run=$base/slot-$model-$slot
out=$RESULTS/$model/$arm-$scen-$n
rm -rf "$out"; mkdir -p "$out" "$base/outside"
if [[ ! -d $run/.git ]]; then
  mkdir -p "$run"; git -C "$run" init -q
  printf '.grok/\nws/\nwire.jsonl\nanswer.md\nmcp.json\nclaude.out\n' > "$run/.gitignore"
  cp "$here/proxy.py" "$run/proxy.py"
  git -C "$run" add -A; git -C "$run" -c user.name=ac08 -c user.email=ac08@example.invalid commit -qm init
  # Grok starts project MCP servers only in trusted folders.
  if [[ $model == grok ]] && ! grep -q "\"$run\"" ~/.grok/trusted_folders.toml 2>/dev/null; then
    printf '\n[folders."%s"]\ntrusted = true\ndecided_at = %s\n' "$run" "$(date +%s)" >> ~/.grok/trusted_folders.toml
  fi
fi
cmp -s "$here/proxy.py" "$run/proxy.py" || { cp "$here/proxy.py" "$run/proxy.py"; git -C "$run" add -A; git -C "$run" -c user.name=ac08 -c user.email=ac08@example.invalid commit -qm proxy; }
rm -rf "$run/ws" "$run/wire.jsonl" "$run/answer.md" "$run/claude.out"; mkdir -p "$run/ws" "$run/.grok"
if [[ ! -f $base/outside/brand.designos ]]; then python3 "$here/mkdoc.py" "$MCP" "$base/outside" brand.designos Logo; fi
case $scen in
  path)
    prompt="Use only the designos MCP tools for design work (no shell, no file copying). Open the design file $base/outside/brand.designos, add a frame named Hero to it, and save the result as hero.designos in the workspace. When you are done, or if you cannot finish, write your final message to me in answer.md in the current directory."
    ;;
  unsaved)
    python3 "$here/mkdoc.py" "$MCP" "$run/ws" other.designos Existing
    prompt="Use only the designos MCP tools for design work (no shell, no file copying). In the current document, add a frame named Draft; do not save it yet. Then open other.designos from the workspace and tell me the names of the frames in it. When you are done, or if you cannot finish, write your final message to me in answer.md in the current directory."
    ;;
esac
: > "$run/wire.jsonl"
if [[ $model == grok ]]; then : > "$run/answer.md"; fi   # run-cli.sh needs the expected file to exist; claude may create it itself
printf '%s\n' "$prompt" > "$out/prompt.txt"
set +e
if [[ $model == grok ]]; then
  cat > "$run/.grok/config.toml" <<TOML
[mcp_servers.designos]
command = "python3"
args = ["$run/proxy.py", "$run/wire.jsonl", "$arm", "$MCP", "--workspace", "$run/ws"]
enabled = true
TOML
  /workspace/projects/maintenance/tools/run-cli.sh --cli grok --cwd "$run" --expect answer.md --expect wire.jsonl -- "$prompt" > "$out/run-cli.out" 2>&1
  echo "run-cli exit $?" >> "$out/run-cli.out"
else
  python3 - "$run" "$arm" "$MCP" > "$run/mcp.json" <<'PY'
import json, sys
run, arm, mcp = sys.argv[1:4]
print(json.dumps({"mcpServers": {"designos": {"command": "python3", "args": [run + "/proxy.py", run + "/wire.jsonl", arm, mcp, "--workspace", run + "/ws"]}}}))
PY
  (cd "$run" && timeout 900 claude -p "$prompt" --model sonnet --mcp-config "$run/mcp.json" --strict-mcp-config \
     --tools Write --allowedTools "Write" "mcp__designos" --permission-mode dontAsk --no-session-persistence > "$run/claude.out" 2>&1)
  echo "claude exit $?" >> "$run/claude.out"
  cp "$run/claude.out" "$out/claude.out"
fi
set -e
python3 "$here/analyze.py" "$run/wire.jsonl" > "$out/verdict.json" || true
cp "$run/wire.jsonl" "$out/"; cp "$run/answer.md" "$out/" 2>/dev/null || true
cat "$out/verdict.json"
