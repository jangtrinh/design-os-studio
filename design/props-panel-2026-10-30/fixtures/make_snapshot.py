#!/usr/bin/env python3
"""Snapshot producer for props_panel_check.py (snapshot mode). Design artifact, not product code.
usage: make_snapshot.py layout.json fixture.json > snapshot.json
layout.json = `designos <fixture> --select fig:1:2 ... --layout-json layout.json` (named records: instance-swap, go-to-main,
instance-more, instance-source, instance-description, prop:<name>). fixture.json = the Figma REST JSON the layout was rendered from.
Items are emitted in visual (y) order. `component_description` is the resolved text the panel is fed (spec D5): the main
component's own description, else its ComponentSet's, whitespace-trimmed; "" when undescribed, so the checker drops the row."""
import json, sys
named = json.load(open(sys.argv[1]))["named"]
fx = json.load(open(sys.argv[2]))
def find(n, pred):
    if pred(n): return n
    for c in n.get("children", []):
        r = find(c, pred)
        if r: return r
inst = find(fx["document"], lambda n: n.get("id") == "1:2")
cid = inst["componentId"]
own = fx["components"][cid].get("description", "")
cset = find(fx["document"], lambda n: n.get("type") == "COMPONENT_SET")
setdesc = fx["componentSets"].get(cset["id"], {}).get("description", "") if cset else ""
desc = " ".join((own if own.strip() else setdesc).split())
defs = (cset or {}).get("componentPropertyDefinitions", {})
TYPE = {"VARIANT": ("variant", "dropdown"), "BOOLEAN": ("boolean", "toggle"), "TEXT": ("text", "text_field")}
tmap = {k.split("#")[0]: TYPE[v["type"]] for k, v in defs.items() if v["type"] in TYPE}
swap = named["instance-swap"]["rect"]
items = []
y = lambda r: r["rect"][1]
items.append({"kind": "header", "name": inst["name"], "chevron": "instance-swap" in named,
              "go_to_main": "go-to-main" in named, "more": "instance-more" in named, "_y": y(named["instance-swap"])})
if "instance-source" in named:
    items.append({"kind": "source", "text": "From this file", "_y": y(named["instance-source"])})
if "instance-description" in named:
    items.append({"kind": "description", "text": desc, "_y": y(named["instance-description"])})
for k, v in named.items():
    if k.startswith("prop:"):
        t, c = tmap.get(k[5:], (None, None))
        r = v["rect"]
        items.append({"kind": "prop", "label": k[5:], "type": t, "control": c, "grid": "%gpx %gpx" % (r[0] - swap[0], r[2]),
                      "label_x": swap[0], "control_x": r[0], "_y": r[1]})
items.sort(key=lambda i: i.pop("_y") if False else i["_y"])
for i in items: i.pop("_y")
json.dump({"items": items, "component_description": desc}, sys.stdout, indent=1)
