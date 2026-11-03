"""R1: the icon-swap round-trip of the library verify. Every screen swaps the `Icon` of a control, so a swap must keep what the
variant draws: the glyph's colour and the variable it is bound to, the Disabled opacity (half alpha on the glyph's own paint), the slot's `Visible`
binding (`Has Leading Icon`) and the slot's size (a 20 px tool, rail or icon-button glyph, which the swap resets to the Icon main's
16 px and re-centres). In 0.7.5 it keeps none of them (engine bug #1427, probes/swap_probe.py of the W1a review).

It also measures the glyph that is drawn, not only the slot's size (the #1427 review's B1: a glyph drawn 1.56x too large past its 20 px slot,
because the engine scaled `fill_geometry` twice, reported SWAP-OK on the slot size alone): the bounds of `fill_geometry` (what the renderer
draws first, curves sampled, glyph_bounds) placed through the glyph's transform must sit inside the slot, must equal the bounds of the glyph's
own Icon main scaled to the glyph box within 1 px (each twin carries its own icon, so two icons' ink is never compared), and the glyph box must be
where the control's is within 1 px. A glyph that draws nothing, and an Icon main found for one twin only, are reported too: silence is right only when both twins lack the data.
Reporting only, like every check here, while SWAP_CHECK_FATAL is False.

    results = run(mcp, workspace)            place a control and a swapped twin of six variants (Tool Selected is no case: its glyph is the Icon colour, so a swap keeps it by accident), save a scratch file, compare
    lines = report(results)                  SWAP-PENDING #1427 with the mismatches (exit 0) while SWAP_CHECK_FATAL is False

`SWAP_CHECK_FATAL` flips to True in the commit that merges #1427 into the pinned designos-mcp; from then on a mismatch stops the
build. The placed instances live in the session and in a scratch file; the committed file is never saved from this session.
"""
import json
import pathlib

import glyph_bounds as gb

SWAP_CHECK_FATAL = False
ISSUE = "#1427"
SCRATCH = "swap-check.designos"
RING = "Focus ring"
EDGE_TOL = 0.01  # px past the slot a glyph may reach: an icon whose ink fills its 16 px box ends exactly on the slot's edge
MATCH_TOL = 1.0  # px: the drawn bounds against the icon's own, and the glyph box against the control's
# (label, set, variant, props of both twins, extra props of the swapped twin)
CASES = [
    ("Button/Tool Hover", "Button/Tool", {"State": "Hover"}, {}, {"Icon": "Icon/Move"}),
    ("Button/Close Xs Hover", "Button/Close", {"Size": "Xs", "State": "Hover"}, {}, {"Icon": "Icon/Search"}),
    ("Button/Icon Xs Disabled", "Button/Icon", {"Size": "Xs", "State": "Disabled"}, {}, {"Icon": "Icon/Check"}),
    ("Button/Primary Md Disabled, leading icon on", "Button/Primary", {"Size": "Md", "State": "Disabled"}, {"Has Leading Icon": True}, {"Icon": "Icon/Check"}),
    ("Button/SecondaryDestructive Sm Hover, leading icon on", "Button/SecondaryDestructive", {"Size": "Sm", "State": "Hover"}, {"Has Leading Icon": True},
     {"Icon": "Icon/Trash"}),
    ("Button/Secondary Sm Hover, leading icon off", "Button/Secondary", {"Size": "Sm", "State": "Hover"}, {"Has Leading Icon": False}, {"Icon": "Icon/Check"}),
]


def _is_instance(node):
    return isinstance(node["kind"], dict) and "Instance" in node["kind"]


def _size(node):
    size = node.get("size") or {}
    return (size["x"], size["y"]) if isinstance(size.get("x"), (int, float)) and isinstance(size.get("y"), (int, float)) else None


def _geometry(nodes, slot, glyph):
    """In slot space: the bounds the glyph draws, the bounds its own Icon main would draw scaled to the glyph box, and the glyph box. Each is
    None when the file has no such data (a synthetic host without geometry or without the main)."""
    box_size, t = _size(glyph), glyph.get("transform")
    box = gb.placed((0.0, 0.0, *box_size), t) if box_size else None
    inst = slot["kind"]["Instance"] if isinstance(slot["kind"], dict) else {}
    main = nodes.get(str(inst.get("component")))
    main_glyph = nodes.get(str(main["children"][0])) if main and main.get("children") else None
    main_size = _size(main_glyph) if main_glyph else None
    expected = None
    if main_glyph and main_size and box_size and min(main_size) > 0:
        expected = gb.placed(gb.scaled(gb.geometry_bounds(main_glyph), box_size[0] / main_size[0], box_size[1] / main_size[1]), t)
    return gb.placed(gb.geometry_bounds(glyph), t), expected, box


def snapshot(doc, host_id):
    """What the host's icon slot draws: the glyph's fill, its binding and opacity, the slot's visibility, `Visible` binding and size, and the
    glyph's drawn bounds, its own icon's bounds at that scale and its box (all in slot space)."""
    nodes = doc["nodes"]
    host = nodes[str(host_id)]
    slot = next((nodes[str(c)] for c in host.get("children", []) if _is_instance(nodes[str(c)]) and nodes[str(c)]["name"] != RING), None)
    glyph = nodes[str(slot["children"][0])] if slot and slot.get("children") else None
    if glyph is None or not glyph.get("fills"):
        raise SystemExit(f"swap check: {host['name']} has no icon slot with a glyph fill")
    paint = glyph["fills"][0]
    solid = paint["kind"]["Solid"]
    drawn, expected, box = _geometry(nodes, slot, glyph)
    return {"fill": {k: round(solid[k] * 255) for k in "rgba"}, "opacity": paint.get("opacity", 1.0),
            "binding": ((glyph.get("variables") or {}).get("bindings") or {}).get("fills/0"),
            "visible_binding": ((slot.get("component") or {}).get("bindings") or {}).get("Visible"), "visible": slot.get("visible", True),
            "size": (slot.get("size") or {}).get("x"), "slot": _size(slot), "drawn": drawn, "expected": expected, "box": box}


def _hex(c):
    return "#{r:02X}{g:02X}{b:02X}".format(**c) + ("" if c["a"] == 255 else f"/{c['a']}")


def _bounds(b):
    return "({}, {})-({}, {})".format(*(f"{round(v, 2):g}" for v in b))


def geometry_problems(snap, who):
    """Why the glyph that `snap` draws is wrong: past its slot, or not the size its own icon has at the glyph box."""
    out, drawn, expected, slot = [], snap["drawn"], snap["expected"], snap["slot"]
    if drawn and slot and (drawn[0] < -EDGE_TOL or drawn[1] < -EDGE_TOL or drawn[2] > slot[0] + EDGE_TOL or drawn[3] > slot[1] + EDGE_TOL):
        out.append(f"{who} glyph is drawn outside its {slot[0]:g} x {slot[1]:g} slot: bounds {_bounds(drawn)}")
    if drawn and expected and any(abs(a - b) > MATCH_TOL for a, b in zip(drawn, expected)):
        out.append(f"{who} glyph is drawn at {_bounds(drawn)} but its icon scaled to the glyph box is {_bounds(expected)}")
    return out


def silence_problems(control, swapped):
    """What a missing measurement hides: a glyph that draws nothing, or an Icon main found for one twin only. Silence is right only when both
    twins lack the data (a synthetic host with no geometry or no main), never when just one does."""
    out = []
    if (control["drawn"] is None) != (swapped["drawn"] is None):
        blank, other = ("swapped", control) if swapped["drawn"] is None else ("control", swapped)
        out.append(f"{blank} glyph draws nothing: its fill geometry and paths have no bounds, the other twin's glyph is at {_bounds(other['drawn'])}")
    if (control["expected"] is None) != (swapped["expected"] is None):
        lost = "swapped" if swapped["expected"] is None else "control"
        out.append(f"{lost} glyph's Icon main was not found (or has no geometry): its drawn bounds cannot be checked against its own icon")
    return out


def compare(control, swapped):
    """Mismatches between the unswapped control and its swapped twin; empty when the swap lost nothing."""
    out = []
    if any(abs(control["fill"][k] - swapped["fill"][k]) > 1 for k in "rgba"):
        out.append(f"glyph fill lost: {_hex(control['fill'])} before the swap, {_hex(swapped['fill'])} after")
    if control["binding"] != swapped["binding"]:
        out.append(f"glyph fill binding lost: {control['binding']} before the swap, {swapped['binding']} after")
    if abs(control["opacity"] - swapped["opacity"]) > 1e-3:
        out.append(f"glyph opacity lost: {control['opacity']:g} before the swap, {swapped['opacity']:g} after")
    if control["size"] is not None and swapped["size"] is not None and abs(control["size"] - swapped["size"]) > 0.01:
        out.append(f"glyph size lost: {control['size']:g} before the swap, {swapped['size']:g} after")
    if control["visible_binding"] != swapped["visible_binding"]:
        out.append(f"slot visibility binding lost: {control['visible_binding']!r} before the swap, {swapped['visible_binding']!r} after")
    if control["visible"] != swapped["visible"]:
        out.append(f"slot visibility changed: {control['visible']} before the swap, {swapped['visible']} after")
    out += geometry_problems(control, "control") + geometry_problems(swapped, "swapped") + silence_problems(control, swapped)
    if control["box"] and swapped["box"] and any(abs(a - b) > MATCH_TOL for a, b in zip(control["box"], swapped["box"])):
        out.append(f"glyph box differs from the control's: {_bounds(swapped['box'])} after the swap, {_bounds(control['box'])} before")
    return out


def run(mcp, workspace):
    """Place the control and swapped twin of every case on `99 Archive`, save a scratch file and compare the twins."""
    pages = {p["name"]: p["node_id"] for p in mcp.tool_or_die("designos_get_document", {})["pages"]}
    archive = next(i for n, i in pages.items() if n.startswith("99"))
    for i, (_, comp, variant, both, extra) in enumerate(CASES):
        for row, (kind, props) in enumerate((("control", both), ("swap", {**both, **extra}))):
            mcp.tool_or_die("designos_apply_spec", {"parent_id": archive, "lint": "off", "spec": {
                "kind": "instance", "name": f"swapck-{i}-{kind}", "component": comp, "variant": variant, "props": props, "x": i * 240, "y": row * 120}})
    mcp.tool_or_die("designos_save_file", {"path": SCRATCH, "overwrite": True})
    doc = json.loads((pathlib.Path(workspace) / SCRATCH).read_text(encoding="utf-8"))
    ids = {n["name"]: int(i) for i, n in doc["nodes"].items() if n["name"].startswith("swapck-")}
    return [{"case": label, "mismatches": compare(snapshot(doc, ids[f"swapck-{i}-control"]), snapshot(doc, ids[f"swapck-{i}-swap"]))}
            for i, (label, *_) in enumerate(CASES)]


def report(results, fatal=None):
    """Lines to print. A mismatch while `fatal` (default SWAP_CHECK_FATAL) stops the build; while pending it only reports."""
    fatal = SWAP_CHECK_FATAL if fatal is None else fatal
    bad = [r for r in results if r["mismatches"]]
    n = sum(len(r["mismatches"]) for r in bad)
    if not bad:
        return [f"SWAP-OK {ISSUE}: {len(results)} swapped variants keep colour, binding, opacity, visibility and the drawn glyph's bounds; "
                f"{'SWAP_CHECK_FATAL is on' if fatal else 'SWAP_CHECK_FATAL can flip to True'}"]
    lines = [f"{'SWAP-FAIL' if fatal else 'SWAP-PENDING'} {ISSUE}: {n} mismatches in {len(bad)} of {len(results)} swapped variants"
             f"{'' if fatal else ' (reporting only: SWAP_CHECK_FATAL is False until the engine fix merges)'}"]
    lines += [f"  {r['case']}: {m}" for r in bad for m in r["mismatches"]]
    if fatal:
        raise SystemExit("\n".join(lines))
    return lines
