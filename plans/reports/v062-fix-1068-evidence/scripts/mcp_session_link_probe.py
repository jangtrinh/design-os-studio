"""Does designos-mcp write or read through a link (or hang on a FIFO) planted in its session folder?

Usage: python3 -I mcp_session_link_probe.py <designos-mcp binary> <scratch dir> [--json <out.json>]

One fresh scratch session folder and one fresh server process per case. Each case plants one hazard at one
session-file name the way the PR #1067 review probe did, drives the server over MCP stdio
(new_document, get_document, two idempotent create_node calls), then compares the victim.
No agent CLI runs. Prints only names, sizes and hashes, never environment values.
"""
import hashlib
import json
import os
import select
import signal
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


LINK_SENTENCE = " is a link, not a file. Delete the link and try again."
PLAIN_SENTENCE = " is not a plain file. Delete it and try again."


def tell(text):
    """Which of the two sentences an agent is to read a message carries (the path in front of it is not printed)."""
    if LINK_SENTENCE in text:
        return "link-sentence"
    if PLAIN_SENTENCE in text:
        return "not-plain-sentence"
    return None


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
        return f"error[{tell(msg)}]" if tell(msg) else f"error: {msg[:70]}"
    warn = sc.get("session_file_warnings") or []
    told = [tell(w) for w in warn if tell(w)]
    return f"ok+warning[{','.join(told)}]" if told else ("ok+warning[other]" if warn else "ok")


def call(server, rid, name, args):
    server.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call", "params": {"name": name, "arguments": args}})
    return server.recv()


def drive(server):
    """Returns (replies, tool_count) for the fixed call sequence; stops early on a hang."""
    server.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0"}}})
    if server.recv() == "HANG":
        return {"initialize": "HANG"}, None
    server.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    server.send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    listed = server.recv()
    tool_count = len(((listed or {}).get("result") or {}).get("tools", [])) if listed != "HANG" else None
    replies = {}
    r = call(server, 3, "designos_new_document", {"name": "Probe", "discard_changes": True})
    replies["new_document"] = summarize(r)
    if r == "HANG":
        return replies, tool_count
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
    return replies, tool_count


VICTIM_TEXT = "USER CONTENT, KEEP ME\n"
MCP_CONFIG_VICTIM = '{"designos": {"enabled_tools": ["designos_get_document"]}}\n'


def plant(kind, session, victim, name):
    path = os.path.join(session, name)
    if kind == "symlink":
        os.symlink(victim, path)
    elif kind == "dangling":
        os.symlink(victim, path)
    elif kind == "hardlink":
        os.link(victim, path)
    elif kind == "fifo":
        os.mkfifo(path)
    return path


# (label, kind, name template, what a pass means)
CASES = [
    ("append via link at doc-patch.jsonl", "symlink", "doc-patch.jsonl", "victim"),
    ("overwrite via canvas.tmp-<pid>", "symlink", "canvas.tmp-{pid}", "victim"),
    ("overwrite via doc-sync.tmp-<pid>", "symlink", "doc-sync.tmp-{pid}", "victim"),
    ("overwrite via session-options.tmp-<pid>", "symlink", "session-options.tmp-{pid}", "victim"),
    ("truncate+append via link at idempotency-ledger.jsonl", "symlink", "idempotency-ledger.jsonl", "victim"),
    ("create via dangling link at doc-patch.jsonl", "dangling", "doc-patch.jsonl", "victim-absent"),
    ("append via hard link at doc-patch.jsonl", "hardlink", "doc-patch.jsonl", "victim"),
    ("read through link at mcp-config.json", "symlink", "mcp-config.json", "no-readthrough"),
    ("FIFO at doc-patch.jsonl (write name)", "fifo", "doc-patch.jsonl", "no-hang"),
    ("FIFO at context.json (read name)", "fifo", "context.json", "no-hang"),
]

rows = []
for idx, (label, kind, name_tpl, check) in enumerate(CASES):
    root = os.path.join(scratch, f"case-{idx}")
    session, workspace = os.path.join(root, "lib", "agent-sessions", "file-1"), os.path.join(root, "workspace")
    home, tmp, vdir = os.path.join(root, "home"), os.path.join(root, "tmp"), os.path.join(root, "victim")
    for d in (session, workspace, home, tmp, vdir):
        os.makedirs(d, exist_ok=True)
    victim = os.path.join(vdir, "user-file.txt")
    if kind != "dangling":
        with open(victim, "w") as f:
            f.write(MCP_CONFIG_VICTIM if name_tpl == "mcp-config.json" else VICTIM_TEXT)
    before = state(victim)
    server = Server(session, workspace, home, tmp)
    name = name_tpl.format(pid=server.proc.pid)
    plant(kind, session, victim, name)
    t0 = time.monotonic()
    replies, tool_count = drive(server)
    elapsed = time.monotonic() - t0
    hung = "HANG" in replies.values()
    server.stop()
    after = state(victim)
    if check == "victim":
        ok = after == before
        detail = f"victim {before} -> {after}"
    elif check == "victim-absent":
        ok = after == "absent"
        detail = f"victim {before} -> {after}"
    elif check == "no-readthrough":
        ok = tool_count is not None and tool_count > 1
        detail = f"tools listed={tool_count} (1 means the victim's JSON was read through the link)"
    else:
        ok = not hung and elapsed < 3.0
        detail = f"{'HANG (killed after %.0fs)' % HANG_SECONDS if hung else 'returned in %.2fs' % elapsed}"
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
