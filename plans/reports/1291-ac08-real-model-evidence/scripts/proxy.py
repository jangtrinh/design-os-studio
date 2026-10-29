#!/usr/bin/env python3
"""Stdio proxy between an agent CLI and designos-mcp. Logs every JSON-RPC line to a wire file.

usage: proxy.py <wire.jsonl> <arm: with-hints|stripped> <designos-mcp> --workspace DIR

Arm `with-hints` forwards every answer unchanged. Arm `stripped` is the negative control: for an UNSAVED_CHANGES or
PATH_OUTSIDE_WORKSPACE answer it removes `next_step`, `error_kind`, `safe_to_retry` and `corrected_example`, and words the
message neutrally, so the agent gets the same refusal without the machine-readable stop instruction.
"""
import json, subprocess, sys, threading, time

wire_path, arm, *cmd = sys.argv[1:]
NEUTRAL = {
    "UNSAVED_CHANGES": "The open document has unsaved changes.",
    "PATH_OUTSIDE_WORKSPACE": "The path is outside the configured workspace.",
}
STRIP = ("next_step", "error_kind", "safe_to_retry", "corrected_example")
wire = open(wire_path, "a", buffering=1)
lock = threading.Lock()
t0 = time.time()

def log(direction, raw, note=""):
    with lock:
        wire.write(json.dumps({"t_ms": int((time.time() - t0) * 1000), "dir": direction, "note": note, "line": raw.rstrip("\n")}) + "\n")

def strip_error(obj):
    code = obj.get("code")
    if code not in NEUTRAL:
        return False
    for k in STRIP:
        obj.pop(k, None)
    obj["message"] = NEUTRAL[code]
    if isinstance(obj.get("error"), dict):
        strip_error(obj["error"])
    return True

def rewrite(line):
    if arm != "stripped":
        return line, ""
    try:
        msg = json.loads(line)
    except ValueError:
        return line, ""
    res = msg.get("result")
    if not isinstance(res, dict) or not res.get("isError"):
        return line, ""
    sc = res.get("structuredContent")
    if not isinstance(sc, dict) or not strip_error(sc):
        return line, ""
    for item in res.get("content", []):
        if item.get("type") == "text":
            try:
                inner = json.loads(item["text"])
            except ValueError:
                continue
            strip_error(inner)
            item["text"] = json.dumps(inner, sort_keys=True, separators=(",", ":"))
    return json.dumps(msg), "stripped"

child = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)

def pump_in():
    for line in sys.stdin:
        log("agent->mcp", line)
        child.stdin.write(line)
        child.stdin.flush()
    child.stdin.close()

threading.Thread(target=pump_in, daemon=True).start()
for line in child.stdout:
    out, note = rewrite(line.rstrip("\n"))
    log("mcp->agent", line, "as-sent: " + out if note else "")
    sys.stdout.write(out + "\n")
    sys.stdout.flush()
child.wait()
