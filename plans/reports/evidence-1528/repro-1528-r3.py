#!/usr/bin/env python3
"""Round-3 probes: rename-after-open (verify_handle), list cursor stability, id gates, fenced value passed back.

Usage: python3 -I repro-1528-r3.py <path/to/designos-mcp> <workdir>
"""
import json, os, sys, pathlib, threading, time, importlib.util

spec = importlib.util.spec_from_file_location("r2", str(pathlib.Path(__file__).with_name("repro-1528-r2-lib.py")))
binary = pathlib.Path(sys.argv[1])
work = pathlib.Path(sys.argv[2])
PID = "a1b2c3"

import subprocess


class Session:
    def __init__(self, lib, ws):
        env = dict(os.environ, DESIGNOS_LIBRARY=str(lib))
        self.p = subprocess.Popen([str(binary), "--workspace", str(ws)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env=env, text=True)
        self.n = 0
        self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "rv", "version": "1"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def rpc(self, method, params):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.n:
                return m

    def call(self, name, args):
        return self.rpc("tools/call", {"name": name, "arguments": args})["result"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def project(lib, pid=PID, name="Checkout"):
    proj = lib / "projects" / pid
    (proj / "docs").mkdir(parents=True, exist_ok=True)
    (proj / "project.json").write_text(json.dumps({"id": pid, "name": name, "folders": [], "created": 1, "modified": 1, "files": []}))
    return proj / "docs"


def manifest(pages, pins=None):
    ids = ["overview", "spec", "screens", "states", "discovery"]
    return json.dumps({"schema_version": 1, "groups": [{"id": g, "pages": pages.get(g, [])} for g in ids], "pins": pins or [], "linked_specs": []})


# R1: a page rewritten by atomic rename (how Studio's write_atomic saves) while it is being read
base = work / "R1"
lib = base / "library"
d = project(lib)
(d / "x.md").write_text("# v0\n\nbody\n")
(d / "docs-manifest.json").write_text(manifest({"overview": ["x.md"]}))
stop = False


def saver():
    i = 0
    while not stop:
        tmp = d / f".x.md.{i}.tmp"
        tmp.write_text(f"# v{i}\n\nbody\n")
        os.replace(tmp, d / "x.md")
        i += 1


t = threading.Thread(target=saver, daemon=True)
s = Session(lib, base)
t.start()
codes = {}
msgs = set()
N = 3000
for _ in range(N):
    r = s.call("designos_docs_read", {"page": "x.md"})
    if r.get("isError"):
        e = r["structuredContent"]["error"]
        codes[e["code"]] = codes.get(e["code"], 0) + 1
        msgs.add(e["message"][:110])
    else:
        codes["ok"] = codes.get("ok", 0) + 1
stop = True
t.join()
s.close()
print(f"[R1 atomic-save during read] reads={N} outcomes={codes} messages={sorted(msgs)}")

# R2: list cursor when the manifest changes between calls
base = work / "R2"
lib = base / "library"
d = project(lib)
pages = [f"p{i:03d}.md" for i in range(300)]
for rel in pages:
    (d / rel).write_text(f"---\ntitle: \"{'T' * 450}\"\n---\n")
(d / "docs-manifest.json").write_text(manifest({"overview": pages}))
s = Session(lib, base)
r1 = s.call("designos_docs_list", {})["structuredContent"]
cur = r1["next_cursor"]
got1 = [p["path"]["value"] for p in r1["groups"][0]["pages"]]
new = [f"new{i}.md" for i in range(5)]
for rel in new:
    (d / rel).write_text("# n\n")
(d / "docs-manifest.json").write_text(manifest({"overview": new + pages}))
# Round 3 change: the cursor is "<12 hex of manifest_hash>:<index>"; a cursor from an earlier manifest must be refused.
for label, listing in (("insert before cursor", new + pages), ("delete before cursor", pages[10:])):
    (d / "docs-manifest.json").write_text(manifest({"overview": listing}))
    r = s.call("designos_docs_list", {"cursor": cur})["structuredContent"]
    err = r.get("error") or {}
    print(f"[R2 {label}] cursor={cur} code={err.get('code')} path={err.get('path')} message={err.get('message')!r} pages_returned={len(r.get('groups', []))}")
r_again = s.call("designos_docs_list", {})["structuredContent"]
print(f"[R2 list again from the start] next_cursor={r_again['next_cursor']} pages={len(r_again['groups'][0]['pages'])}")
s.close()

# R3: file_id / node_id gates
base = work / "R3"
lib = base / "library"
d = project(lib)
pins = [
    {"id": "a", "file_id": "deadbeefdecafbadc0ffeefacade0001", "node_id": "000000000000000000042", "name": "n", "group": "screens"},
    {"id": "b", "file_id": "acceded", "node_id": "18446744073709551616", "name": "n", "group": "screens"},
    {"id": "c", "file_id": "ffffffffffffffffffffffffffffffff0", "node_id": "0042", "name": "n", "group": "screens"},
]
(d / "docs-manifest.json").write_text(manifest({}, pins))
s = Session(lib, base)
out = s.call("designos_docs_list", {})["structuredContent"]["pins"]
print("[R3 id gates]", [(p["file_id"] if isinstance(p["file_id"], str) else "FENCED", p["node_id"] if isinstance(p["node_id"], str) else "FENCED") for p in out])
s.close()

# R4: a fenced value passed back to read, and the whole envelope passed back
base = work / "R4"
lib = base / "library"
d = project(lib)
(d / "spec").mkdir()
(d / "spec" / "a b.md").write_text("# ok\n")
(d / "docs-manifest.json").write_text(manifest({"spec": ["spec/a b.md"]}))
s = Session(lib, base)
p = s.call("designos_docs_list", {})["structuredContent"]["groups"][1]["pages"][0]["path"]
r = s.call("designos_docs_read", {"page": p["value"]})
print(f"[R4 value back] isError={r.get('isError')} page_echo_fenced={isinstance(r['structuredContent'].get('page'), dict)}")
r = s.call("designos_docs_read", {"page": p})
e = r["structuredContent"].get("error", {})
print(f"[R4 envelope back] isError={r.get('isError')} code={e.get('code')} msg={e.get('message','')[:120]!r} received={json.dumps(e.get('received'))[:120]}")
for bad in ["spec/a b.md\u0000", "spec/" + "a" * 600 + ".md", "spec/a b.md/../../x.md"]:
    r = s.call("designos_docs_read", {"page": bad})
    e = r["structuredContent"].get("error", {})
    print(f"[R4 hostile arg {bad[:20]!r}...] code={e.get('code')} msg_has_arg={bad[:10] in e.get('message','')} received_fenced={isinstance(e.get('received'), dict)}")
s.close()
