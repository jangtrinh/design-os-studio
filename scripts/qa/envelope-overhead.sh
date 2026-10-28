#!/bin/sh
# Substitute for designos-eval. No model. Tokenizer: chars/4.
# Usage:
#   DESIGNOS_MCP_BIN=target/debug/designos-mcp scripts/qa/envelope-overhead.sh

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
mcp_bin=${DESIGNOS_MCP_BIN:-$here/target/debug/designos-mcp}
if [ ! -x "$mcp_bin" ]; then
    echo "envelope-overhead: DESIGNOS_MCP_BIN is missing or not executable: $mcp_bin" >&2
    exit 2
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/designos-1289-overhead.XXXXXX")

python3 - "$mcp_bin" "$work" <<'PY'
import json, os, subprocess, sys

mcp_bin, root = sys.argv[1], sys.argv[2]
tools = [
    ("designos_get_document", {}),
    ("designos_list_nodes", {}),
    ("designos_get_metadata", {}),
    ("designos_list_comments", {}),
]

def run(flag):
    tag = "off" if flag == "0" else "on"
    ws = os.path.join(root, tag, "ws")
    sess = os.path.join(root, tag, "session")
    os.makedirs(ws, exist_ok=True)
    os.makedirs(sess, exist_ok=True)
    env = os.environ.copy()
    env["DESIGNOS_SESSION_DIR"] = sess
    if flag is None:
        env.pop("DESIGNOS_MCP_UNTRUSTED_ENVELOPE", None)
    else:
        env["DESIGNOS_MCP_UNTRUSTED_ENVELOPE"] = flag
    p = subprocess.Popen(
        [mcp_bin, "--workspace", ws],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        text=True,
        bufsize=1,
    )
    i = 0

    def rpc(method, params=None):
        nonlocal i
        i += 1
        req = {"jsonrpc": "2.0", "id": i, "method": method}
        if params is not None:
            req["params"] = params
        p.stdin.write(json.dumps(req) + "\n")
        p.stdin.flush()
        while True:
            line = p.stdout.readline()
            if not line:
                raise SystemExit("mcp eof: " + p.stderr.read()[:2000])
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("id") == i:
                if "error" in msg:
                    raise SystemExit("mcp error: " + json.dumps(msg["error"]))
                return msg.get("result")

    def call(name, args=None):
        return rpc("tools/call", {"name": name, "arguments": args or {}})

    rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "envelope-overhead", "version": "1"}})
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    p.stdin.flush()

    doc = call("designos_get_document", {})["structuredContent"]
    page = doc["pages"][0]["node_id"]
    created = call("designos_create_node", {"parent_id": page, "kind": "text", "name": "Hero title", "text": "Welcome to the product. " * 40, "width": 320, "height": 80})
    nid = created["structuredContent"]["node"]["id"]
    call("designos_add_comment", {"node_id": nid, "text": "Designer note: check spacing on this frame."})
    get_node = ("designos_get_node", {"node_id": nid})
    export = ("designos_export_spec", {"node_id": nid})
    rows = []
    for name, args in tools + [get_node, export]:
        res = call(name, args)
        sc = res.get("structuredContent")
        structured = json.dumps(sc, separators=(",", ":")) if sc is not None else ""
        text = "".join(b.get("text", "") for b in res.get("content") or [] if b.get("type") == "text")
        rows.append((name, len(structured), len(text)))
    p.stdin.close()
    p.wait(timeout=20)
    return rows

off = {name: (sj, t) for name, sj, t in run("0")}
on = {name: (sj, t) for name, sj, t in run(None)}
print("substitute envelope overhead (no model). tokenizer: chars/4 heuristic.")
print(f"{'tool':<28} {'sj_off':>8} {'sj_on':>8} {'sj_d':>8} {'tok_d':>8} {'tx_off':>8} {'tx_on':>8} {'tx_d':>8} {'tx_tok':>8}")
sum_sj = sum_tx = 0
for name in off:
    sj0, t0 = off[name]
    sj1, t1 = on[name]
    dsj, dtx = sj1 - sj0, t1 - t0
    sum_sj += dsj
    sum_tx += dtx
    print(f"{name:<28} {sj0:8d} {sj1:8d} {dsj:8d} {dsj/4:8.1f} {t0:8d} {t1:8d} {dtx:8d} {dtx/4:8.1f}")
print(f"{'TOTAL':<28} {'':>8} {'':>8} {sum_sj:8d} {sum_sj/4:8.1f} {'':>8} {'':>8} {sum_tx:8d} {sum_tx/4:8.1f}")
PY
