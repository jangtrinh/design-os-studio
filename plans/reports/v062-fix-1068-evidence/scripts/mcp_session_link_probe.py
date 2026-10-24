"""Does designos-mcp write or read through a link (or hang on a FIFO) planted in its session folder, and what does it tell the agent?

Usage: python3 -I mcp_session_link_probe.py <designos-mcp binary> <scratch dir> [--json <out.json>]

One fresh scratch session folder and one fresh server process per case. Each case plants one hazard at one
session-file name the way the PR #1067 review probe did, drives the server over MCP stdio
(new_document, get_document, two idempotent create_node calls), then compares the victim. The cases of the second
round add a hard link read, a read-only file, a folder, a socket, and the text of a metadata reply.
No agent CLI runs. Prints only names, sizes and hashes, never environment values.
"""
import hashlib
import json
import os
import select
import signal
import socket
import subprocess
import sys
import time

binary, scratch = sys.argv[1], sys.argv[2]
json_out = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
HANG_SECONDS = 4.0


def sha(data):
    return hashlib.sha256(data).hexdigest()[:10]


def state(path):
    if not os.path.exists(path):
        return "absent"
    data = open(path, "rb").read()
    return f"{len(data)}B/{sha(data)}"


class Server:
    def __init__(self, session, workspace, home, tmp):
        env = {"PATH": "/usr/bin:/bin", "HOME": home, "TMPDIR": tmp + "/", "DESIGNOS_SESSION_DIR": session}
        self.proc = subprocess.Popen([binary, "--workspace", workspace], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, env=env, text=True)

    def send(self, msg):
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def recv(self, timeout=HANG_SECONDS):
        ready, _, _ = select.select([self.proc.stdout], [], [], timeout)
        if not ready:
            return "HANG"
        line = self.proc.stdout.readline()
        return json.loads(line) if line else None

    def stop(self):
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.send_signal(signal.SIGKILL)
            self.proc.wait()


# The sentences of the round-2 copy. The base binary says other things, so a case that needs a sentence fails on it.
SENTENCES = {
    "link-sentence": " is a link, not a file. Delete the link, then try again.",
    "not-plain-sentence": " is not a plain file. Delete it, then try again.",
    "second-name-sentence": " has a second name (a hard link). Delete it, then try again.",
    "cant-sentence": ". Check its permissions and free space, then try again.",
}
SEEN = []  # every message a case's replies carried: error messages, warnings, and the text of a result


def tell(text):
    """Which sentence an agent is to read a message carries (the path in front of it is not printed)."""
    return next((tag for tag, sentence in SENTENCES.items() if sentence in text), None)


def texts_of(res):
    return [c.get("text", "") for c in res.get("content") or [] if c.get("type") == "text"]


def summarize(reply):
    """One short word for what a tool reply told the agent."""
    if reply == "HANG":
        return "HANG"
    if reply is None:
        return "no-reply"
    res = reply.get("result", {})
    sc = res.get("structuredContent") or {}
    if res.get("isError"):
        msg = (sc.get("error") or {}).get("message") or ""
        SEEN.append(msg)
        return f"error[{tell(msg)}]" if tell(msg) else f"error: {msg[:70]}"
    warn = sc.get("session_file_warnings") or []
    SEEN.extend(warn)
    told = [tell(w) for w in warn if tell(w)]
    out = f"ok+warning[{','.join(told)}]" if told else ("ok+warning[other]" if warn else "ok")
    text_told = [tell(t) for t in texts_of(res) if tell(t)]
    SEEN.extend(t for t in texts_of(res) if tell(t))
    return out + (f"+text[{','.join(text_told)}]" if text_told else "")


def call(server, rid, name, args):
    server.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call", "params": {"name": name, "arguments": args}})
    return server.recv()


def drive(server, first=None):
    """Returns (replies, tool_count, doc_name) for the fixed call sequence; stops early on a hang."""
    server.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0"}}})
    if server.recv() == "HANG":
        return {"initialize": "HANG"}, None, None
    server.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    server.send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    listed = server.recv()
    tool_count = len(((listed or {}).get("result") or {}).get("tools", [])) if listed != "HANG" else None
    replies, doc_name = {}, None
    if first == "metadata":
        r = call(server, 9, "designos_get_metadata", {})
        replies["get_metadata"] = summarize(r)
    elif first == "document":
        r = call(server, 10, "designos_get_document", {})
        replies["get_document"] = summarize(r)
        doc_name = ((r or {}).get("result", {}).get("structuredContent") or {}).get("name") if r != "HANG" else None
        return replies, tool_count, doc_name
    r = call(server, 3, "designos_new_document", {"name": "Probe", "discard_changes": True})
    replies["new_document"] = summarize(r)
    if r == "HANG":
        return replies, tool_count, doc_name
    r = call(server, 4, "designos_get_document", {})
    replies["get_document"] = summarize(r)
    page = None
    try:
        page = r["result"]["structuredContent"]["pages"][0]["node_id"]
    except (KeyError, IndexError, TypeError):
        pass
    if page is not None:
        for i in (5, 6):
            r = call(server, i, "designos_create_node", {"kind": "frame", "parent_id": page, "name": f"P{i}", "idempotency_key": f"probe-{i}"})
            replies[f"create_node_{i - 4}"] = summarize(r)
            if r == "HANG":
                break
    return replies, tool_count, doc_name


VICTIM_TEXT = "USER CONTENT, KEEP ME\n"
MCP_CONFIG_VICTIM = '{"designos": {"enabled_tools": ["designos_get_document"]}}\n'


def victim_document_bytes():
    """A valid design file of this binary's own making, named VictimDocName."""
    root = os.path.join(scratch, "victim-doc")
    session, workspace, home, tmp = (os.path.join(root, n) for n in ("session", "workspace", "home", "tmp"))
    for d in (session, workspace, home, tmp):
        os.makedirs(d, exist_ok=True)
    server = Server(session, workspace, home, tmp)
    server.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0"}}})
    server.recv()
    server.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    call(server, 2, "designos_new_document", {"name": "VictimDocName", "discard_changes": True})
    server.stop()
    return open(os.path.join(session, "canvas.designos"), "rb").read()


def plant(kind, session, victim, name):
    path = os.path.join(session, name)
    if kind in ("symlink", "dangling"):
        os.symlink(victim, path)
    elif kind == "hardlink":
        os.link(victim, path)
    elif kind == "fifo":
        os.mkfifo(path)
    elif kind == "readonly":
        with open(path, "w") as f:
            f.write(VICTIM_TEXT)
        os.chmod(path, 0o444)
    elif kind == "dir":
        os.mkdir(path)
        with open(os.path.join(path, "inside.txt"), "w") as f:
            f.write(VICTIM_TEXT)
    elif kind == "socket":
        short = os.path.join("/tmp", f"ds-probe-{os.getpid()}.sock")  # a socket path is limited to ~100 bytes
        s = socket.socket(socket.AF_UNIX)
        s.bind(short)
        s.close()
        os.rename(short, path)
    return path


# (label, kind, name template, check, expected sentence or None, first call or None)
# check: victim = unchanged | victim-absent | no-readthrough | no-hang | not-loaded (the planted document's name must not show)
CASES = [
    ("append via link at doc-patch.jsonl", "symlink", "doc-patch.jsonl", "victim", None, None),
    ("overwrite via canvas.tmp-<pid>", "symlink", "canvas.tmp-{pid}", "victim", None, None),
    ("overwrite via doc-sync.tmp-<pid>", "symlink", "doc-sync.tmp-{pid}", "victim", None, None),
    ("overwrite via session-options.tmp-<pid>", "symlink", "session-options.tmp-{pid}", "victim", None, None),
    ("truncate+append via link at idempotency-ledger.jsonl", "symlink", "idempotency-ledger.jsonl", "victim", None, None),
    ("create via dangling link at doc-patch.jsonl", "dangling", "doc-patch.jsonl", "victim-absent", None, None),
    ("append via hard link at doc-patch.jsonl", "hardlink", "doc-patch.jsonl", "victim", "second-name-sentence", None),
    ("read through link at mcp-config.json", "symlink", "mcp-config.json", "no-readthrough", None, None),
    ("FIFO at doc-patch.jsonl (write name)", "fifo", "doc-patch.jsonl", "no-hang", None, None),
    ("FIFO at context.json (read name)", "fifo", "context.json", "no-hang", None, None),
    ("read through hard link at canvas.designos", "hardlink-doc", "canvas.designos", "not-loaded", "second-name-sentence", "document"),
    ("0444 file at doc-patch.jsonl", "readonly", "doc-patch.jsonl", "victim", "cant-sentence", None),
    ("folder at canvas.designos", "dir", "canvas.designos", "victim-inside", "not-plain-sentence", None),
    ("socket at context.json", "socket", "context.json", "no-hang", "not-plain-sentence", None),
    ("warning in the model's text (link at session-options.json, get_metadata first)", "symlink", "session-options.json", "victim", "link-sentence-in-text", "metadata"),
]

rows = []
doc_bytes = None
for idx, (label, kind, name_tpl, check, expected, first) in enumerate(CASES):
    root = os.path.join(scratch, f"case-{idx}")
    session, workspace = os.path.join(root, "lib", "agent-sessions", "file-1"), os.path.join(root, "workspace")
    home, tmp, vdir = os.path.join(root, "home"), os.path.join(root, "tmp"), os.path.join(root, "victim")
    for d in (session, workspace, home, tmp, vdir):
        os.makedirs(d, exist_ok=True)
    victim = os.path.join(vdir, "user-file.txt")
    if kind == "hardlink-doc":
        doc_bytes = doc_bytes or victim_document_bytes()
        with open(victim, "wb") as f:
            f.write(doc_bytes)
    elif kind not in ("dangling", "readonly", "dir", "socket"):
        with open(victim, "w") as f:
            f.write(MCP_CONFIG_VICTIM if name_tpl == "mcp-config.json" else VICTIM_TEXT)
    server = Server(session, workspace, home, tmp)
    name = name_tpl.format(pid=server.proc.pid)
    planted = plant("hardlink" if kind == "hardlink-doc" else kind, session, victim, name)
    watched = {"readonly": planted, "dir": os.path.join(planted, "inside.txt")}.get(kind, victim)
    before = state(watched)
    SEEN.clear()
    t0 = time.monotonic()
    replies, tool_count, doc_name = drive(server, first)
    elapsed = time.monotonic() - t0
    hung = "HANG" in replies.values()
    server.stop()
    after = state(watched)
    expected_tag = (expected or "").removesuffix("-in-text")
    told = expected is None or any(SENTENCES[expected_tag] in text for text in SEEN)
    if expected and expected.endswith("-in-text"):
        told = replies.get("get_metadata", "").find("text[" + expected_tag) >= 0
    if check in ("victim", "victim-inside"):
        ok = after == before and told
        detail = f"victim {before} -> {after}"
    elif check == "victim-absent":
        ok = after == "absent"
        detail = f"victim {before} -> {after}"
    elif check == "no-readthrough":
        ok = tool_count is not None and tool_count > 1
        detail = f"tools listed={tool_count} (1 means the victim's JSON was read through the link)"
    elif check == "not-loaded":
        ok = after == before and doc_name is not None and doc_name != "VictimDocName" and told
        detail = f"document name shown: {doc_name!r} (VictimDocName means it was read through the second name)"
    else:
        ok = not hung and elapsed < 3.0 and told
        detail = f"{'HANG (killed after %.0fs)' % HANG_SECONDS if hung else 'returned in %.2fs' % elapsed}"
    if expected and not told:
        detail += f"; the expected sentence ({expected}) was not told"
    rows.append({"case": label, "verdict": "PASS" if ok else "FAIL", "detail": detail, "replies": replies})

width = max(len(r["case"]) for r in rows)
print(f"{'case':<{width}}  verdict  detail")
for r in rows:
    print(f"{r['case']:<{width}}  {r['verdict']:<7}  {r['detail']}")
    print(f"{'':<{width}}           replies: " + "; ".join(f"{k}={v}" for k, v in r["replies"].items()))
print(f"\nSUMMARY: {sum(r['verdict'] == 'PASS' for r in rows)} PASS, {sum(r['verdict'] == 'FAIL' for r in rows)} FAIL of {len(rows)}")
if json_out:
    with open(json_out, "w") as f:
        json.dump(rows, f, indent=1)
