"""G2: on pages 01-03 every fill, stroke, stroke weight, corner radius, gap, padding and effect colour that is set is bound
to a Semantic variable of the right kind whose value equals the stored one.

A gradient paint binds per stop (`fills/<i>/stops/<j>`), an image paint has no variable and is not a finding.
Frame fields sit at `kind.<Frame|Component|ComponentSet|...>.auto_layout` and, for an Instance, at
`kind.Instance.frame.auto_layout`. A radius binds through the four corner keys only: 0.7.5 ignores a uniform `cornerRadius` binding
(render, codegen and has-hardcoded skip it), so that key counts as unbound. A Focus/Ring instance never binds a radius (see
studio_ui_gate_ring), and a node named `Focus ring` that is not an instance of a Focus/Ring variant fails (it would skip every ring rule).
"""
from __future__ import annotations

from typing import Any, List, Tuple

from studio_ui_gate_doc import GateInputError, collection_names, is_opacity_entry, same, variable_value, walk
from studio_ui_gate_indent import indent_problem, tree_row_depths
from studio_ui_gate_ring import CORNERS, bound_problem, detached_ring_problem, main_radius, radius_binding, ring_mains, ring_problems
from studio_ui_theme_policy import SEMANTIC

GATED_PAGES = ("01", "02", "03")
SIDES = ("strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight")
PADS = ("paddingTop", "paddingRight", "paddingBottom", "paddingLeft")


def _rgba(c: dict) -> dict:
    return {k: round(c[k] * 255) for k in ("r", "g", "b", "a")}


def _paint_props(prefix: str, i: int, paint: dict) -> List[Tuple[str, Any]]:
    """A solid binds as `<prefix>/<i>`, a gradient per stop as `<prefix>/<i>/stops/<j>`; an image paint has no variable."""
    kind = paint.get("kind") or {}
    if "Solid" in kind:
        return [(f"{prefix}/{i}", _rgba(kind["Solid"]))]
    return [(f"{prefix}/{i}/stops/{j}", _rgba(stop["color"]))
            for shape in kind.values() if isinstance(shape, dict) for j, stop in enumerate(shape.get("stops") or [])]


def _body(node: dict) -> dict:
    kind = node["kind"]
    body = next(iter(kind.values())) if isinstance(kind, dict) else {}
    return body if isinstance(body, dict) else {}


def paints_of(node: dict) -> List[Tuple[str, int, dict]]:
    """(binding prefix, index, paint) of every visible fill and stroke paint of `node`; a text node's fills are its style's."""
    kind = node["kind"]
    fills = (_body(node).get("style") or {}).get("fills") if isinstance(kind, dict) and "Text" in kind else node.get("fills")
    out = [("fills", i, f) for i, f in enumerate(fills or []) if f.get("visible", True)]
    return out + [("strokes", i, p) for i, p in enumerate((node.get("stroke") or {}).get("paints") or []) if p.get("visible", True)]


def node_props(node: dict) -> List[Tuple[str, Any]]:
    """(binding key, stored value) for every property of `node` that must be bound; stored is None when not comparable."""
    body, stroke = _body(node), node.get("stroke") or {}
    out: List[Tuple[str, Any]] = [kv for prefix, i, p in paints_of(node) for kv in _paint_props(prefix, i, p)]
    if stroke.get("paints"):
        if stroke.get("side_weights"):
            out += [(k, w) for k, w in zip(SIDES, stroke["side_weights"]) if w]
        elif stroke.get("weight"):
            out.append(("strokeWeight", stroke["weight"]))
    out += [(k, r) for k, r in zip(CORNERS, node.get("corner_radii") or []) if r]
    al = body.get("auto_layout") or (body.get("frame") or {}).get("auto_layout")
    if al:
        out += [("itemSpacing", al["item_spacing"])] if al.get("item_spacing") else []
        out += [("counterAxisSpacing", al["counter_spacing"])] if al.get("counter_spacing") else []
        out += [(k, p) for k, p in zip(PADS, al.get("padding") or []) if p]
    for i, effect in enumerate(node.get("effects") or []):
        for shape in effect.values() if isinstance(effect, dict) else []:
            if isinstance(shape, dict) and shape.get("visible", True) and shape.get("color"):
                out.append((f"effects/{i}/color", _rgba(shape["color"])))
    return out


def _kind_for(key: str) -> str:
    return "Color" if key.startswith(("fills/", "strokes/")) or key.endswith("/color") else "Number"


def _show(v: Any) -> str:
    return "#{r:02X}{g:02X}{b:02X}/{a}".format(**v) if isinstance(v, dict) else f"{v:g}"


def _entry_value_problem(entry: dict) -> str | None:
    v = entry.get("value")
    ok = isinstance(v, dict) and all(isinstance(v.get(k), (int, float)) for k in "rgba") or (isinstance(v, (int, float)) and not isinstance(v, bool))
    return None if ok else "has no value: an allow-list entry pins the literal it waives"


def _waiver_problem(entry, stored) -> str | None:
    """Why the allow-list entry cannot waive `stored` (None when it can)."""
    if _entry_value_problem(entry):
        return "is not bound and its allow-list entry has no value"
    if "host" in entry:  # the key itself is the finding: "", null, 0 and false are as wrong as a name
        return (f"names a host ({entry['host']!r}): the host of a Focus/Ring instance is its parent node, read from the file, so an entry "
                f"carries only the literal it waives; delete the field")
    if not same(stored, entry["value"]):
        return f"stored {_show(stored)} differs from the allow-listed literal {_show(entry['value'])}"
    return None


def _allow_index(allow: list) -> tuple[dict, list]:
    index, fails = {}, []
    for a in allow:
        k = (a["path"], a["property"])
        if k in index:
            fails.append(f"allow-list entry {a['path']}: {a['property']} is listed twice")
        index.setdefault(k, a)
        if _entry_value_problem(a):
            fails.append(f"allow-list entry {a['path']}: {a['property']} {_entry_value_problem(a)}")
    return index, fails


def check_bound(doc: dict, allow: list) -> tuple[list, dict]:
    """(failures, per-page counts). An allow-list entry is used by exactly one unbound property and every unbound property has one
    (a stale, duplicate, valueless, shared-path or mismatching entry fails), so `allowed` equals `entries` on a page that passes."""
    cols = collection_names(doc)
    allow = [a for a in allow if not is_opacity_entry(a)]  # a paint-opacity entry waives no unbound property: G2 opacity reads it
    allowed, fails = _allow_index(allow)
    mains, used, depths = ring_mains(doc), set(), tree_row_depths(doc)
    per_page, total = {}, 0
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None:
            raise GateInputError(f"dangling page id {pid}")
        if not page["name"].startswith(GATED_PAGES):
            continue
        c = per_page[page["name"]] = {"nodes": 0, "properties": 0, "bound": 0, "allowed": 0, "unbound": 0, "entries": 0}
        seen: set = {str(pid)}
        for child in page.get("children", []):
            for node, path in walk(doc, child, (page["name"],), seen):
                c["nodes"] += 1
                bindings = (node.get("variables") or {}).get("bindings") or {}
                ring = main_radius(node, mains)
                if ring is not None:
                    fails += [f"{path}: {line}" for line in ring_problems(doc, node)]
                elif detached_ring_problem(node):
                    fails.append(f"{path}: {detached_ring_problem(node)}")
                for key, stored in node_props(node):
                    c["properties"] += 1
                    if ring is not None and key in CORNERS and radius_binding(bindings, key):
                        c["unbound"] += 1  # no binding can draw host + SPACE_FACT, and no allow-list entry waives one
                        used.add((path, key))
                        fails.append(f"{path}: {key} {bound_problem(doc, radius_binding(bindings, key))}")
                        continue
                    var = doc["variables"].get(bindings.get(key) or "")
                    problem = None
                    if var is None:
                        problem = "is not bound"
                    elif cols.get(var["collection"]) != SEMANTIC:
                        problem = "is bound to a non-Semantic variable"
                    elif var["kind"] != _kind_for(key):
                        problem = f"is bound to a variable of kind {var['kind']}, expected {_kind_for(key)}"
                    else:
                        value = variable_value(doc, var)
                        if stored is not None and value is not None and not same(stored, value):
                            problem = f"stored {_show(stored)} differs from variable {var['name']} {_show(value)}"
                    depth = indent_problem(doc, node, key, stored, depths)
                    if depth:
                        fails.append(f"{path}: {depth}")
                    if problem is None:
                        c["bound"] += 1
                        continue
                    entry = allowed.get((path, key))
                    waived = None if entry is None else _waiver_problem(entry, stored)
                    if entry is not None and waived is None and (path, key) in used:
                        waived = "shares its path with another node, so one allow-list entry would waive both"
                    if entry is not None and waived is None:
                        c["allowed"] += 1
                    else:
                        c["unbound"] += 1
                        fails.append(f"{path}: {key} {problem if entry is None else waived}")
                    used.add((path, key))
        c["entries"] = sum(1 for a in allow if a["path"].split("/")[0] == page["name"])
        total += c["nodes"]
    fails += [f"allow-list entry {p}: {k} matches no unbound property" for (p, k) in allowed if (p, k) not in used]
    if not per_page or total == 0:
        raise GateInputError("G2 selected no nodes on pages 01-03: refusing to pass an empty selection")
    return fails, per_page
