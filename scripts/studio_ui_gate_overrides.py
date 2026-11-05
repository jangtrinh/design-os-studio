"""G2 overrides: an instance, and every node inside one, differs from its main only in what the editor lets an instance own.

VSF rule S5 ("no style overrides on instances; instances take data only") with the exceptions the Studio file needs. A node that mirrors a
main node (`component.source`) is compared with that node by value and by variable binding, never by the `component.overrides` flags
(3 of the 25 Focus rings pin a corner radius that differs from their main's, yet the import flags only `Layout`). The non-style fields never count (`OverrideField`,
designos-doc/src/node.rs): Name, Visible, Locked, Size, Transform, Characters, MainComponent and SlotContent. `layout_child`, `constraints` and
the own sizing modes of an instance (`primary_sizing`, `counter_sizing`, written by `set_sizing` next to a Fill `layout_child`; studio_ui_gate_overrides_view) are placement, so they are not
compared either, provided the instance carries the `Layout` flag that `record_override` raises for them (components.rs:481-485): a copy that sizes itself with no flag fails. A
ring's absolute position and a hugging child's sizing are no style. Everything else is a failure:

    Opacity, Blend, Fills, Stroke, Effects, CornerRadii, TextStyle (style, runs, `style_refs`), Layout (the frame's gap, padding,
    direction, alignment, clipping, grids), and a variable binding on a style key that the main does not carry.

Exceptions, each checked against the file, not assumed:

    * paint 0's solid colour (and its opacity: a Disabled glyph is dimmed) and the `fills/0` binding of the `Glyph` that is the first child of
      an `Icon/*` instance, when that fill is bound to a Semantic colour (an icon swap or a recoloured glyph);
    * the corner radii of a Focus/Ring instance and their bindings: the ring gate owns their value;
    * paint 0's solid colour and the `fills/0` binding of the layer named `Color` inside a Swatch/Color instance (the swatch's colour is content).

    The two fill exceptions stop at paint 0 (studio_ui_gate_overrides_paint): the paint count, paint 0's other fields, every other paint and
    every other fill binding stay the main's, and a text layer named `Color` gets no exemption.

    fails, counts = check_overrides(doc)    counts: {"instances", "nodes"} checked on pages 02 and 03
"""
from __future__ import annotations

from typing import List, Optional, Tuple

from studio_ui_gate_bound import CORNERS
from studio_ui_gate_doc import GateInputError, collection_names, walk
from studio_ui_gate_overrides_paint import without_first_colour
from studio_ui_gate_overrides_view import _equal, own_sizing_differs, style_view
from studio_ui_gate_ring import ring_mains
from studio_ui_theme_policy import SEMANTIC

PAGES = ("02", "03")
SWATCH_SET = "Swatch/Color"
SWATCH_LAYER = "Color"


def _bindings(node: dict) -> dict:
    return (node.get("variables") or {}).get("bindings") or {}


def _key_field(key: str, is_text: bool) -> str:
    if key.startswith("fills/"):
        return "TextStyle" if is_text else "Fills"
    if key.startswith("strokes/") or (key.startswith("stroke") and key.endswith("Weight")):
        return "Stroke"
    if key in CORNERS or key == "cornerRadius":
        return "CornerRadii"
    if key.startswith("effects/"):
        return "Effects"
    if key in ("opacity", "blend"):
        return key.capitalize()
    if key.startswith("font") or key.startswith(("letter", "line", "paragraph")):
        return "TextStyle"
    return "Layout"


def _sets(doc: dict, name: str) -> set:
    return {int(c) for n in doc["nodes"].values() if n["name"] == name and isinstance(n["kind"], dict) and "ComponentSet" in n["kind"]
            for c in n.get("children", [])}


def _instance_of(node: dict) -> Optional[int]:
    inst = node["kind"].get("Instance") if isinstance(node["kind"], dict) else None
    return int(inst["component"]) if inst and inst.get("component") is not None else None


def _is_glyph(doc: dict, node: dict) -> bool:
    parent = doc["nodes"].get(str(node.get("parent"))) or {}
    comp = _instance_of(parent) if parent else None
    kids = parent.get("children") or []
    return comp is not None and str(doc["nodes"].get(str(comp), {}).get("name", "")).startswith("Icon/") and bool(kids) and kids[0] == node["id"]


def _in_swatch(doc: dict, node: dict, swatches: set) -> bool:
    cur = node
    while cur is not None:
        comp = _instance_of(cur)
        if comp is not None and comp in swatches:
            return node["name"] == SWATCH_LAYER and cur is not node
        cur = doc["nodes"].get(str(cur.get("parent")))
    return False


def _semantic_colour(doc: dict, node: dict, cols: dict) -> bool:
    var = doc["variables"].get(_bindings(node).get("fills/0") or "")
    return var is not None and cols.get(var["collection"]) == SEMANTIC and var["kind"] == "Color"


def check_overrides(doc: dict) -> Tuple[List[str], dict]:
    """(failures, {"instances", "nodes"}) over every node that mirrors a main node on the pages named 02... and 03..."""
    cols, rings, swatches = collection_names(doc), ring_mains(doc), _sets(doc, SWATCH_SET)
    fails: List[str] = []
    counts = {"instances": 0, "nodes": 0}
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None:
            raise GateInputError(f"dangling page id {pid}")
        if not page["name"].startswith(PAGES):
            continue
        for child in page.get("children", []):
            for node, path in walk(doc, child, (page["name"],), {str(pid)}):
                src_id = (node.get("component") or {}).get("source")
                if src_id is None:
                    continue
                src = doc["nodes"].get(str(src_id))
                if src is None:
                    fails.append(f"{path}: mirrors node {src_id}, which does not exist")
                    continue
                counts["nodes"] += 1
                counts["instances"] += _instance_of(node) is not None
                fails += [f"{path}: {line}" for line in _node_problems(doc, node, src, cols, rings, swatches)]
    if counts["nodes"] == 0:
        raise GateInputError("the override gate found no instance on pages 02 and 03: refusing to pass an empty selection")
    return fails, counts


def _node_problems(doc, node, src, cols, rings, swatches) -> List[str]:
    is_text = isinstance(node["kind"], dict) and "Text" in node["kind"]
    comp = _instance_of(node)
    ok_fields, ok_keys = set(), set()
    if comp in rings:
        ok_fields.add("CornerRadii")
        ok_keys |= {*CORNERS, "cornerRadius"}
    free_fill = not is_text and (_in_swatch(doc, node, swatches) or (_is_glyph(doc, node) and _semantic_colour(doc, node, cols)))
    if free_fill:
        ok_keys.add("fills/0")
    mine, theirs = style_view(node, ignore_sizing=comp is not None), style_view(src, ignore_sizing=comp is not None)  # `comp`: the node is itself an instance, the copy that may size itself
    unflagged = comp is not None and own_sizing_differs(node, src) and "Layout" not in ((node.get("component") or {}).get("overrides") or [])
    if free_fill:
        mine["Fills"], theirs["Fills"] = without_first_colour(mine["Fills"], theirs["Fills"])
    out = [f"{f} differs from its main ({src['name']!r}): an instance may only change text, visibility, size, position and swaps"
           for f in mine if f not in ok_fields and not _equal(mine[f], theirs[f])]
    if unflagged:
        out.append(f"its own sizing modes differ from its main's ({src['name']!r}) but it carries no Layout override flag: the editor records one when a copy is resized (`set_sizing`)")
    have, want = _bindings(node), _bindings(src)
    for key in sorted(set(have) | set(want)):
        if key in ok_keys or have.get(key) == want.get(key):
            continue
        out.append(f"{key} is bound to {_var(doc, have.get(key))}, its main ({src['name']!r}) has {_var(doc, want.get(key))}: "
                   f"a binding on a style key is a style override ({_key_field(key, is_text)})")
    return out


def _var(doc: dict, vid: Optional[str]) -> str:
    return "nothing" if vid is None else (doc["variables"].get(vid) or {}).get("name", vid)
