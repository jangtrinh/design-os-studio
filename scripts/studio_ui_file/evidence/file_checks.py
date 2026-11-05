"""Structural checks of a native `.designos` for the Foundations rules that the gate does not read (evidence tool, stdlib only).

    python3 scripts/studio_ui_file evidence structure <file.designos> <allowlist.json>      exit 0 when every check passes

Checks: Primitive colour names follow the palette-exact / hue-step rule (no hex), array names are 0-based, inset tiles are
concentric, popover and modal samples sit in radius 16, badges are 6 (large 8), the contrast section has its pairs and
named exceptions, and the 4 px card inset is a recorded allow-list exception.
"""
import json
import pathlib
import re
import sys

import colour_names as names_mod


def main(argv=None):
    path, allow_path = (sys.argv[1:3] if argv is None else argv)[:2]
    doc = json.loads(pathlib.Path(path).read_text())
    nodes, variables = doc["nodes"], doc["variables"]
    cols = {c["id"]: c["name"] for c in doc["variable_collections"].values()}
    results = []

    def check(label, ok, detail=""):
        results.append((label, ok, detail))

    prims = [v for v in variables.values() if cols[v["collection"]] == "Primitives" and v["kind"] == "Color"]
    bad = [v["name"] for v in prims if re.search(r"[0-9a-f]{6}", v["name"])]
    check("M4 no Primitive colour is named by hex", not bad, f"{len(bad)} hex-named, e.g. {bad[:3]}")
    wrong = []
    for v in prims:
        c = v["values"]["dark"]["Color"]
        rgba = tuple(round(c[k] * 255) for k in ("r", "g", "b", "a"))
        base = re.sub(r"(-\d+)?$", "", re.sub(r"-a\d+(?=(-\d+)?$)", "", v["name"]))
        if base != names_mod.opaque_name(*rgba[:3]):
            wrong.append((v["name"], names_mod.opaque_name(*rgba[:3])))
    check("M4 every Primitive follows the palette-exact / hue-step rule", not wrong, f"{len(wrong)} differ, e.g. {wrong[:2]}")
    check("M4 #fefce8 is yellow/50", any(v["name"] == "yellow/50" for v in prims))
    arr = [(v["name"], v["code_syntax"]["WEB"]) for v in variables.values() if re.search(r"\[\d+\]", v.get("code_syntax", {}).get("WEB", ""))]
    off = [n for n, cs in arr if not n.endswith("-" + re.search(r"\[(\d+)\]", cs).group(1))]
    check("minor array names are 0-based like their code syntax", arr and not off, f"{len(off)} off, e.g. {off[:2]}")

    by_name = {}
    for n in nodes.values():
        by_name.setdefault(n["name"], []).append(n)

    def radius(node):
        return (node.get("corner_radii") or [0])[0]

    insets = [n for n in nodes.values() if n["name"].startswith("Inset inset/")]
    ok_inset, detail = bool(insets), []
    for col in insets:
        frame = next(nodes[str(c)] for c in col["children"] if nodes[str(c)]["name"] == "Frame")
        tile = nodes[str(frame["children"][0])]
        pad = frame["kind"]["Frame"]["auto_layout"]["padding"][0]
        want = max(0.0, radius(frame) - pad)
        if abs(radius(tile) - want) > 1e-6:
            ok_inset = False
            detail.append((col["name"], radius(tile), want))
    check("P4 inset tiles are concentric (inner = outer - inset)", ok_inset, str(detail[:3]))
    outer = {n["name"]: radius(next(nodes[str(c)] for c in n["children"] if nodes[str(c)]["name"] == "Frame")) for n in insets}
    check("P4 popover and modal samples sit in radius 16", outer.get("Inset inset/popover") == 16 and outer.get("Inset inset/modal") == 16, str(outer))
    ctl = {n["name"]: radius(next(nodes[str(c)] for c in n["children"] if nodes[str(c)]["name"] == "Control"))
           for n in nodes.values() if n["name"].startswith("Control control/")}
    check("P4 badges are 6 (large 8)", ctl.get("Control control/badge-sm") == 6 and ctl.get("Control control/badge-md") == 6
          and ctl.get("Control control/badge-lg") == 8, str({k: v for k, v in ctl.items() if "badge" in k}))
    pairs = [n for n in nodes.values() if n["name"].startswith("Pair ")]
    exc = [n for n in nodes.values() if isinstance(n["kind"], dict) and str(n["kind"].get("Text", {}).get("content", "")).startswith("Exception R")]
    check("S6 contrast pairs with ratios and 4 named exceptions", len(pairs) >= 45 and len(exc) == 4, f"{len(pairs)} pairs, {len(exc)} exceptions")
    allow = json.loads(pathlib.Path(allow_path).read_text())["allow"]
    check("P4 the 4 px card inset is a recorded allow-list exception", len(allow) == 4 and all("Card holds control" in a["path"] for a in allow), f"{len(allow)} entries")
    for label, ok, detail in results:
        print(("PASS " if ok else "FAIL ") + label + ("" if ok else f"  [{detail}]"))
    return 0 if all(ok for _, ok, _ in results) else 1
