"""G2 rule for the Tree/Row indent (split out of studio_ui_gate_bound to keep that file under its line cap).

A layers row at depth n is indented n * LAYER_INDENT (`indent = depth * LAYER_INDENT`, panels/layers.rs:538), and the variant `Depth=n` of the Tree/Row component set carries it, so the padding
is a property of the variant. `tree_row_depths(doc)` maps every such variant's node id to n; `indent_problem` is the failure text when a variant's left padding is not n * the Semantic
variable of theme::LAYER_INDENT, else None. An instance that sets its own left padding is no longer waived: it is an ordinary unbound property.
"""
from __future__ import annotations

import re
from typing import Any, Optional

from studio_ui_gate_doc import GateInputError
from studio_ui_gate_ring import constant_value

TREE_ROW_SET, INDENT_CODE = "Tree/Row", "theme::LAYER_INDENT"
DEPTH = re.compile(r"(?:^|, )Depth=(\d+)(?:,|$)")


def tree_row_depths(doc: dict) -> dict:
    """{node id: n} of every variant of a Tree/Row component set whose name carries `Depth=n`."""
    out = {}
    for n in doc["nodes"].values():
        if n["name"] == TREE_ROW_SET and isinstance(n["kind"], dict) and "ComponentSet" in n["kind"]:
            for c in n.get("children", []):
                m = DEPTH.search(doc["nodes"][str(c)]["name"])
                if m:
                    out[int(c)] = int(m.group(1))
    return out


def indent_problem(doc: dict, node: dict, key: str, stored: Any, depths: dict) -> Optional[str]:
    """The failure when a Tree/Row variant `Depth=n` does not indent its row by n * the Semantic variable of theme::LAYER_INDENT (`indent = depth * LAYER_INDENT`, panels/layers.rs:538;
    `LAYER_INDENT`, theme.rs:545), else None; a padding bound to a variable must be the LAYER_INDENT one, not another variable of the same value. Only
    the variant carries the indent: an instance that sets its own left padding is an ordinary unbound property, and so is any other side or node."""
    if key != "paddingLeft" or depths.get(int(node.get("id", -1))) is None:
        return None
    step = constant_value(doc, INDENT_CODE)
    if step is None:
        raise GateInputError(f"no Semantic variable has the code syntax {INDENT_CODE}: a Tree/Row indent cannot be checked")
    want = depths[int(node["id"])] * step
    if not (isinstance(stored, (int, float)) and abs(stored - want) < 1e-3):
        return f"paddingLeft stored {_num(stored)} differs from Depth={depths[int(node['id'])]} x LAYER_INDENT {_num(want)}"
    bound = ((node.get("variables") or {}).get("bindings") or {}).get(key)
    var = doc["variables"].get(bound) if bound is not None else None
    if bound is not None and (var is None or (var.get("code_syntax") or {}).get("WEB") != INDENT_CODE):
        return f"paddingLeft is bound to {var['name'] if var else bound}, not to the LAYER_INDENT variable"
    return None


def _num(v: Any) -> str:
    return f"{v:g}" if isinstance(v, (int, float)) else repr(v)
