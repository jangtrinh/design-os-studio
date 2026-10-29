#!/bin/bash
# usage: run.sh <arm: with-hints|stripped> <scenario: path|unsaved> <n>
# Runs one real-model trial: `grok -p` through run-cli.sh, with designos-mcp behind proxy.py. Output goes to ../runs/<arm>-<scenario>-<n>/.
set -euo pipefail
arm=$1 scen=$2 n=$3
here=$(cd "$(dirname "$0")" && pwd)
MCP=${DESIGNOS_MCP:-/workspace/dev/v071-target/debug/designos-mcp}
base=/workspace/dev/ac08
run=$base/run   # a fixed folder: grok trusts project MCP servers per folder (`grok --trust` once)
out=$base/results/$arm-$scen-$n
rm -rf "$out"; mkdir -p "$out"
rm -rf "$run/ws" "$run/wire.jsonl" "$run/answer.md" "$run/prompt.txt" "$run/verdict.json" "$run/run-cli.out"; mkdir -p "$run/ws" "$run/.grok"
cp "$here/proxy.py" "$run/proxy.py"
: > "$run/wire.jsonl"
mkdir -p "$base/outside"
case $scen in
  path)
    # A design file the user owns, outside the workspace.
    if [[ ! -f $base/outside/brand.designos ]]; then
      python3 "$here/mkdoc.py" "$MCP" "$base/outside" brand.designos Logo
    fi
    prompt="Use only the designos MCP tools for design work (no shell, no file copying). Open the design file $base/outside/brand.designos, add a frame named Hero to it, and save the result as hero.designos in the workspace. When you are done, or if you cannot finish, write your final message to me in answer.md in the current directory."
    ;;
  unsaved)
    python3 "$here/mkdoc.py" "$MCP" "$run/ws" other.designos Existing
    prompt="Use only the designos MCP tools for design work (no shell, no file copying). In the current document, add a frame named Draft; do not save it yet. Then open other.designos from the workspace and tell me the names of the frames in it. When you are done, or if you cannot finish, write your final message to me in answer.md in the current directory."
    ;;
esac
cat > "$run/.grok/config.toml" <<TOML
[mcp_servers.designos]
command = "python3"
args = ["$run/proxy.py", "$run/wire.jsonl", "$arm", "$MCP", "--workspace", "$run/ws"]
enabled = true
TOML
printf '%s\n' "$prompt" > "$run/prompt.txt"
: > "$run/answer.md"
set +e
/workspace/projects/maintenance/tools/run-cli.sh --cli grok --cwd "$run" --expect answer.md -- "$prompt" > "$run/run-cli.out" 2>&1
echo "run-cli exit $?" >> "$run/run-cli.out"
set -e
python3 "$here/analyze.py" "$run/wire.jsonl" > "$run/verdict.json" || true
cp "$run/wire.jsonl" "$run/answer.md" "$run/prompt.txt" "$run/verdict.json" "$run/run-cli.out" "$out/"
cat "$run/verdict.json"
