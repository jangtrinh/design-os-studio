#!/usr/bin/env python3
"""usage: mkdoc.py <designos-mcp> <dir> <file name> <frame name> -- saves a one-frame document into <dir> through designos-mcp."""
import json, subprocess, sys
mcp, d, name, frame = sys.argv[1:5]
p = subprocess.Popen([mcp, "--workspace", d], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
def call(i, method, params=None):
    msg = {"jsonrpc": "2.0", "method": method, **({"id": i} if i else {}), "params": params or {}}
    p.stdin.write(json.dumps(msg) + "\n"); p.stdin.flush()
    return json.loads(p.stdout.readline()) if i else None
call(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "mkdoc", "version": "0"}})
call(0, "notifications/initialized")
call(2, "tools/call", {"name": "designos_create_node", "arguments": {"parent_id": 1, "kind": "frame", "name": frame}})
r = call(3, "tools/call", {"name": "designos_save_file", "arguments": {"path": name}})
assert r["result"]["structuredContent"]["saved"], r
p.stdin.close(); p.wait()
