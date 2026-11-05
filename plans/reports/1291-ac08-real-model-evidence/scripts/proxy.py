#!/usr/bin/env python3
"""Stdio proxy between an agent CLI and designos-mcp. Logs every JSON-RPC line to a wire file.

usage: proxy.py <wire.jsonl> <arm: with-hints|stripped-hints|stripped-hints-rules> <designos-mcp> --workspace DIR

Arm `with-hints` forwards every answer unchanged. Arm `stripped-hints` is the negative control: for an UNSAVED_CHANGES or
PATH_OUTSIDE_WORKSPACE answer it removes `next_step`, `error_kind`, `safe_to_retry` and `corrected_example`, and words the
message neutrally, so the agent gets the same refusal without the machine-readable stop instruction.
Arm `stripped-hints-rules` does that and also removes the stop guidance from the rules text the server hands the agent: the
error-contract sentences in the `initialize` instructions, and, in every other answer (guidelines, app state, resources, prompts),
each line that mentions `needs_human` or is part of the error-recovery section. Tool descriptions and schemas are left as built.
"""
import re
import json, subprocess, sys, threading, time

wire_path, arm, *cmd = sys.argv[1:]
NEUTRAL = {
    "UNSAVED_CHANGES": "The open document has unsaved changes.",
    "PATH_OUTSIDE_WORKSPACE": "The path is outside the configured workspace.",
}
STRIP = ("next_step", "error_kind", "safe_to_retry", "corrected_example")
RULE_LINE = re.compile(r"needs_human|Errors & retries|Structured errors|Retry contract|safe_to_retry|error_kind|Error Recovery|next_step")
INSTR_CUT = re.compile(r"\s*Tool errors return structuredContent\.error.*$", re.S)
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

def strip_rules_text(node, count):
    """Drops rule lines from every multi-line string under `node` (in place); `count` is a one-item list of removed lines."""
    if isinstance(node, dict):
        for k, v in list(node.items()):
            if isinstance(v, str) and "\n" in v:
                kept = [ln for ln in v.split("\n") if not RULE_LINE.search(ln)]
                count[0] += len(v.split("\n")) - len(kept)
                node[k] = "\n".join(kept)
            else:
                strip_rules_text(v, count)
    elif isinstance(node, list):
        for i, v in enumerate(node):
            if isinstance(v, str) and "\n" in v:
                kept = [ln for ln in v.split("\n") if not RULE_LINE.search(ln)]
                count[0] += len(v.split("\n")) - len(kept)
                node[i] = "\n".join(kept)
            else:
                strip_rules_text(v, count)

def rewrite(line):
    if arm == "with-hints":
        return line, ""
    try:
        msg = json.loads(line)
    except ValueError:
        return line, ""
    res = msg.get("result")
    if not isinstance(res, dict):
        return line, ""
    notes = []
    if arm == "stripped-hints-rules":
        if isinstance(res.get("instructions"), str):
            cut = INSTR_CUT.sub("", res["instructions"])
            if cut != res["instructions"]:
                res["instructions"] = cut
                notes.append("instructions-cut")
        count = [0]
        # text inside tool results / resources is a JSON string or plain text; handle plain multi-line text and nested JSON text
        for item in res.get("content", []) if isinstance(res.get("content"), list) else []:
            if item.get("type") == "text" and isinstance(item.get("text"), str) and item["text"][:1] in "{[":
                try:
                    inner = json.loads(item["text"])
                except ValueError:
                    inner = None
                if inner is not None:
                    strip_rules_text(inner, count)
                    item["text"] = json.dumps(inner, sort_keys=True, separators=(",", ":"))
            elif item.get("type") == "text" and isinstance(item.get("text"), str) and "\n" in item["text"]:
                kept = [ln for ln in item["text"].split("\n") if not RULE_LINE.search(ln)]
                count[0] += item["text"].count("\n") + 1 - len(kept)
                item["text"] = "\n".join(kept)
        for item in res.get("contents", []) if isinstance(res.get("contents"), list) else []:
            if isinstance(item.get("text"), str):
                kept = [ln for ln in item["text"].split("\n") if not RULE_LINE.search(ln)]
                count[0] += item["text"].count("\n") + 1 - len(kept)
                item["text"] = "\n".join(kept)
        for m in res.get("messages", []) if isinstance(res.get("messages"), list) else []:
            c = m.get("content")
            if isinstance(c, dict) and isinstance(c.get("text"), str):
                kept = [ln for ln in c["text"].split("\n") if not RULE_LINE.search(ln)]
                count[0] += c["text"].count("\n") + 1 - len(kept)
                c["text"] = "\n".join(kept)
        if isinstance(res.get("structuredContent"), (dict, list)):
            strip_rules_text(res["structuredContent"], count)
        if count[0]:
            notes.append("rule-lines-removed:%d" % count[0])
    sc = res.get("structuredContent")
    if res.get("isError") and isinstance(sc, dict) and strip_error(sc):
        for item in res.get("content", []):
            if item.get("type") == "text":
                try:
                    inner = json.loads(item["text"])
                except ValueError:
                    continue
                strip_error(inner)
                item["text"] = json.dumps(inner, sort_keys=True, separators=(",", ":"))
        notes.append("hints-stripped")
    if not notes:
        return line, ""
    return json.dumps(msg), ",".join(notes)

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
    log("mcp->agent", line, "as-sent[" + note + "]: " + out if note else "")
    sys.stdout.write(out + "\n")
    sys.stdout.flush()
child.wait()
