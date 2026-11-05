"""G2 stray: on pages 01-03 no node holds a variable binding on a property of G2's own families that the node does not set.

G2 (`studio_ui_gate_bound`) walks the properties a node sets and checks each against its binding; it never walks the `bindings` map the other way. A binding whose property is unset (a side with
a stored weight of 0, a padding of 0, a second fill the node lacks) passes unseen, and the engine still draws it: designos-mcp 0.7.5 draws a `strokeBottomWeight` bound to a 4 px variable
on a node whose bottom side stores 0 at 4 px (probed on a copy of Bar/Presentation: render rows 64-79 at 4x). Per-side strokes make that reachable, so it is a finding here, and no allow-list
entry waives it.

The families are the ones `node_props` lists: `fills/*`, `strokes/*`, `effects/*/color`, `strokeWeight` and the four `stroke<Side>Weight`, the four corner keys, the four paddings,
`itemSpacing` and `counterAxisSpacing`. A size binding (`width`, `height`) is outside them: 16 sit on the committed file and are legitimate. A uniform `cornerRadius` binding is outside too:
0.7.5 ignores it and G2 already counts the four corner keys of such a node as unbound.

    check_stray(doc) -> (failures, bindings read)
"""
from __future__ import annotations

import re
from typing import List, Tuple

from studio_ui_gate_bound import CORNERS, GATED_PAGES, PADS, SIDES, node_props
from studio_ui_gate_doc import GateInputError, walk

KEYS = frozenset(SIDES + PADS + tuple(CORNERS) + ("strokeWeight", "itemSpacing", "counterAxisSpacing"))
PREFIXES = ("fills/", "strokes/")
EFFECT_COLOR = re.compile(r"^effects/\d+/color$")


def in_families(key: str) -> bool:
    return key in KEYS or key.startswith(PREFIXES) or bool(EFFECT_COLOR.match(key))


def check_stray(doc: dict) -> Tuple[List[str], int]:
    fails: List[str] = []
    read = 0
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None:
            raise GateInputError(f"dangling page id {pid}")
        if not page["name"].startswith(GATED_PAGES):
            continue
        seen: set = {str(pid)}
        for child in page.get("children", []):
            for node, path in walk(doc, child, (page["name"],), seen):
                bindings = (node.get("variables") or {}).get("bindings") or {}
                read += len(bindings)
                listed = {key for key, _ in node_props(node)}
                fails += [f"{path}: {key} is a binding on a property the node does not set: the engine draws it at the variable's value"
                          for key in sorted(bindings) if key not in listed and in_families(key)]
    return fails, read
