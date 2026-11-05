"""Layout pass over the live MCP session, after the REST import (which keeps the sizes it was given, estimated):

    ids = source_ids(saved_native_file)      REST id -> native node id
    relayout(mcp, ids, info)                 make the engine measure every text, so hug widths are real
    check_nested(mcp, ids, info)             a copied text that keeps its main's content is as wide as the main's measured text (±0.5)
    check_resized(mcp, ids, info)            a nested copy a module sized itself has that size on the saved file (±0.5)
    place(mcp, ids, info, sections)          real variant sizes -> grid positions, focus-ring sizes, set and section sizes

A text is measured when its content changes, so each one is set to a placeholder and back; that is the only route found
(an update with the same content does not measure). Sizes then survive save and reopen.
"""
import json
import pathlib

import library_core as core

RING_OUT = 4  # the ring band starts 2 px and ends 4 px outside its host: its box is the host's, grown by SPACE_FACT (4) on each side


def source_ids(path):
    doc = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    return {n["source_id"]: int(i) for i, n in doc["nodes"].items() if n.get("source_id")}


def batches(mcp, ops, n=200):
    for i in range(0, len(ops), n):
        mcp.tool_or_die("designos_apply_batch", {"ops": ops[i:i + n], "lint": "off"})


def relayout(mcp, ids, info):
    texts = [(ids[sid], content) for s in info for v in s["variants"] for sid, content in v["texts"]]
    batches(mcp, [{"op": "update", "node": nid, "text": "x"} for nid, _ in texts])
    batches(mcp, [{"op": "update", "node": nid, "text": c} for nid, c in texts])
    return len(texts)


WIDTH_TOL = 0.5


def text_width(mcp, nid):
    return mcp.tool_or_die("designos_get_node", {"node_id": nid})["node"]["size"]["x"]


def check_nested(mcp, ids, info):
    """Every text copied into a nested instance that keeps its main's content must measure as wide as the main's own text: a copy the layout pass skipped keeps the
    width it was cloned at (the REST estimate, 0.56 x size x characters), which is not the main's laid-out width. A longer label has no main to compare with: the
    engine measured it the same way (relayout), so it hugs it. A Fill-width text (`fills`: {copy: (instance, the main's root)}) is as wide as its main's plus the growth of the
    instance over its main's root, so a copy resized by `w=` is compared with that. Returns the number of pairs checked; any difference is fatal."""
    pairs = [(s["name"], v["name"], c, m, (v.get("fills") or {}).get(c)) for s in info for v in s["variants"] for c, m in v.get("mirrors", [])]
    bad = []
    for set_name, variant, copy_sid, main_sid, fill in pairs:
        a, b = text_width(mcp, ids[copy_sid]), text_width(mcp, ids[main_sid])
        note = ""
        if fill:
            grew = text_width(mcp, ids[fill[0]]) - text_width(mcp, ids[fill[1]])
            note = f" (its Fill text moves with the instance {fill[0]}, {grew:+g} px against its main {fill[1]})"
            b += grew
        if abs(a - b) > WIDTH_TOL:
            bad.append(f"{set_name} / {variant}: the copied text {copy_sid} is {a:g} px wide, its main text {main_sid} {b:g} px{note}")
    if bad:
        raise SystemExit(f"{len(bad)} nested texts were not measured like their mains:\n" + "\n".join(bad[:25]))
    return len(pairs)


def check_resized(mcp, ids, info):
    """Every nested copy a module sized itself (`w=` / `h=`) has the size it asked for on the saved file: a hug main resized in a frame that gives it no `layoutSizing*` keeps hugging
    unless its own frame sizing is fixed, and that would draw the main's width where the capture has another. Only the axes the module named are compared. Returns the number of copies
    checked; any difference is fatal."""
    rows = [(s["name"], v["name"], sid, w, h) for s in info for v in s["variants"] for sid, w, h in v.get("resized", [])]
    bad = []
    for set_name, variant, sid, w, h in rows:
        size = mcp.tool_or_die("designos_get_node", {"node_id": ids[sid]})["node"]["size"]
        for axis, want, got in (("width", w, size["x"]), ("height", h, size["y"])):
            if want and abs(got - want) > WIDTH_TOL:
                bad.append(f"{set_name} / {variant}: the copy {sid} is {got:g} px {'wide' if axis == 'width' else 'high'}, the module asked for {want:g}")
    if bad:
        raise SystemExit(f"{len(bad)} resized copies did not take the size they were given:\n" + "\n".join(bad[:25]))
    return len(rows)


def children(mcp, nid):
    out, off = [], 0
    while True:
        r = mcp.tool_or_die("designos_list_nodes", {"parent_id": nid, "offset": off, "limit": 500})
        out += r["nodes"]
        if r["next_offset"] is None:
            return out
        off = r["next_offset"]


def host_size(mcp, ids, ring, variant, size):
    """The measured size of the node a ring sits in: the variant's own (already read) or, for a nested host such as a checkbox's box, its node's."""
    if ring["parent"] == variant["id"]:
        return size
    host = mcp.tool_or_die("designos_get_node", {"node_id": ids[ring["parent"]]})["node"]["size"]
    return host["x"], host["y"]


def place(mcp, ids, info, sections):
    """Variants onto a grid by their real sizes, rings to their host's size + 8 (the host is the node a ring sits in), sets and sections
    fitted, the sections of each page in one row. A plain component (a record with no variants) is its own box: nothing wraps it."""
    ops, set_sizes, report, labels = [], {}, {}, {s["id"]: s["label"] for s in info}
    page_of = {s["section"]: s.get("page", "02 Core") for s in info}
    for s in info:
        if s.get("plain"):
            node = mcp.tool_or_die("designos_get_node", {"node_id": ids[s["id"]]})["node"]["size"]
            v, w, h = s["variants"][0], node["x"], node["y"]
            for ring in v["rings"]:
                hw, hh = host_size(mcp, ids, ring, v, (w, h))
                ops.append({"op": "update", "node": ids[ring["id"]], "x": -RING_OUT, "y": -RING_OUT, "width": hw + 2 * RING_OUT, "height": hh + 2 * RING_OUT})
            report[s["name"]] = [w, h]
            set_sizes[s["id"]] = (w, h)
            continue
        kids = {k["name"]: k for k in children(mcp, ids[s["id"]])}
        sizes = [(kids[v["name"]]["width"], kids[v["name"]]["height"]) for v in s["variants"]]
        xs, ys, w, h = core._grid(sizes, s["cols"])
        for v, (vw, vh), x, y in zip(s["variants"], sizes, xs, ys):
            ops.append({"op": "update", "node": ids[v["id"]], "x": 32 + x, "y": 32 + y})
            for ring in v["rings"]:
                hw, hh = host_size(mcp, ids, ring, v, (vw, vh))
                ops.append({"op": "update", "node": ids[ring["id"]], "x": -RING_OUT, "y": -RING_OUT, "width": hw + 2 * RING_OUT, "height": hh + 2 * RING_OUT})
            report[f"{s['name']} / {v['name']}"] = [vw, vh]
        ops.append({"op": "update", "node": ids[s["id"]], "width": w + 64, "height": h + 64})
        set_sizes[s["id"]] = (w + 64, h + 64)
    batches(mcp, ops)
    x, page = 0, None
    for sec_id, sec_sets in sections.items():
        if page_of[sec_id] != page:
            page, x = page_of[sec_id], 0
        sec = ids[sec_id]
        y = core.PAD + core.HEADER
        ops = []
        for sid in sec_sets:
            ops.append({"op": "update", "node": ids[labels[sid]], "x": core.PAD, "y": y})
            ops.append({"op": "update", "node": ids[sid], "x": core.PAD, "y": y + core.LABEL})
            y += core.LABEL + set_sizes[sid][1] + core.SET_GAP
        w = max(set_sizes[sid][0] for sid in sec_sets)
        ops.append({"op": "update", "node": sec, "x": x, "y": 0, "width": w + 2 * core.PAD, "height": y - core.SET_GAP + core.PAD})
        batches(mcp, ops)
        x += w + 2 * core.PAD + core.GAP
    return report
