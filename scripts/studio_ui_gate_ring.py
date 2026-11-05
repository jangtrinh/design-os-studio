"""G2 rule for Focus/Ring instances: a ring rings the node it sits in, and that node is its host (`kit/focus.rs` paint_ring).

Production draws the ring around the host's rect grown by SPACE_FACT on every side, at the host's radius + SPACE_FACT, rounded
(`focus.rs:36-38`: `rect.expand(SPACE_FACT)`, `(radius + SPACE_FACT).round()`, one `CornerRadius::same`). So the file proves it from the
instance's parent, with no second source to drift:

* the ring is its parent grown by SPACE_FACT on every side: size + 2 * SPACE_FACT, at (-SPACE_FACT, -SPACE_FACT), no scale or rotation;
* each corner is `round(parent radius + SPACE_FACT)`, half away from zero like Rust's `f32::round`. The parent's radius is what it draws:
  clamped to half its smaller side, so a pill host (radius at or above half its height) gives `round(height / 2) + SPACE_FACT`
  (`cr_pill`, theme.rs:432-434). The main's 12 is only right on a RADIUS_CONTROL 8 host: a ring that keeps it inside a 2, 4 or 6 px
  host fails, inherited or pinned;
* the four corners are equal (Rust draws one `CornerRadius::same`), whatever the host's corners are;
* 0.7.5 draws a bound radius at the variable's value (render, codegen and has-hardcoded read the four corner keys; a uniform
  `cornerRadius` binding is ignored), so no binding can express "host radius + SPACE_FACT": any radius binding on a ring instance
  fails and every corner that is not bound is a literal whose allow-list entry pins the value. An entry names no `host`: it would be a
  second source for what the parent already says, so one that carries it fails.

A node named `Focus ring` that is not an instance of a Focus/Ring variant is a finding (`detached_ring_problem`): it would skip all of the above.

What the file cannot know is which node Rust rings: a ring around a whole checkbox row that is the right size for that row passes. The
checkbox rings its 16 px box, not its control rect (`choice.rs:104-111`), so a checkbox variant must make the box the ring's parent
(written on `00 Start Here`).
"""
from __future__ import annotations

import math
from typing import Dict, List, Optional

from studio_ui_gate_doc import GateInputError, same, variable_value

RING_SET = "Focus/Ring"
RING_NODE_NAME = "Focus ring"  # what the generator names a ring instance (components/buttons.py with_ring); the set itself is `Focus/Ring`
OFFSET_CODE = "theme::SPACE_FACT"
CORNERS = ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius")


def rust_round(x: float) -> float:
    """f32::round: halves round away from zero (Python's round() is banker's: round(14.5) is 14, Rust gives 15). x is never negative here."""
    return float(math.floor(x + 0.5))


def ring_mains(doc: dict) -> Dict[int, float]:
    """{node id of a Focus/Ring variant: its corner radius} for every component set named Focus/Ring."""
    out: Dict[int, float] = {}
    for node in doc["nodes"].values():
        if node["name"] == RING_SET and isinstance(node["kind"], dict) and "ComponentSet" in node["kind"]:
            for c in node.get("children", []):
                radii = (doc["nodes"].get(str(c)) or {}).get("corner_radii") or [0.0]
                out[int(c)] = float(radii[0])
    return out


def main_radius(node: dict, mains: Dict[int, float]) -> Optional[float]:
    """The main's radius when `node` is an instance of a Focus/Ring variant, else None."""
    kind = node["kind"]
    inst = kind.get("Instance") if isinstance(kind, dict) else None
    return mains.get(int(inst["component"])) if inst and inst.get("component") is not None else None


def detached_ring_problem(node: dict) -> Optional[str]:
    """For a node that is not an instance of a Focus/Ring variant (the caller has checked): the finding when it is named like a ring. A frame or
    an instance of another component called `Focus ring` would skip every rule here, so the name alone makes it a finding."""
    if node.get("name") != RING_NODE_NAME:
        return None
    return (f"is named {RING_NODE_NAME!r} but is not an instance of a Focus/Ring variant, so none of the ring rules (size, position, radius) "
            f"apply to it: place the Focus/Ring instance or rename the node")


def constant_value(doc: dict, code: Optional[str]) -> Optional[float]:
    """The value of the Semantic variable whose code syntax is `code` (a theme.rs constant), or None when no variable has it."""
    for var in doc["variables"].values():
        if code and (var.get("code_syntax") or {}).get("WEB") == code:
            value = variable_value(doc, var)
            return value if isinstance(value, float) else None
    return None


def ring_offset(doc: dict) -> float:
    """The value of the Semantic variable of theme::SPACE_FACT: what a ring adds to its host on every side."""
    value = constant_value(doc, OFFSET_CODE)
    if value is None:
        raise GateInputError(f"no Semantic variable has the code syntax {OFFSET_CODE}: a Focus/Ring cannot be checked")
    return value


def radius_binding(bindings: dict, key: str) -> Optional[str]:
    """The variable id bound to the corner `key`, or to the uniform `cornerRadius` (which the engine ignores): both are forbidden on a ring."""
    return bindings.get(key) or bindings.get("cornerRadius")


def bound_problem(doc: dict, var_id: str) -> str:
    name = (doc["variables"].get(var_id) or {}).get("name", var_id)
    return (f"is bound to {name} on a Focus/Ring instance: 0.7.5 draws a bound radius at the variable's value, not host + SPACE_FACT; "
            f"keep a literal radius")


def _xy(v) -> Optional[tuple]:
    ok = isinstance(v, dict) and all(isinstance(v.get(k), (int, float)) for k in ("x", "y"))
    return (float(v["x"]), float(v["y"])) if ok else None


def _geometry_problems(ring: dict, parent: dict, offset: float) -> List[str]:
    """Why the ring is not its parent grown by `offset` on every side (size, position, no scale or rotation)."""
    who, pname = ring.get("name"), parent.get("name")
    psize, rsize = _xy(parent.get("size")), _xy(ring.get("size"))
    if psize is None or rsize is None:
        return [f"sits in {pname!r}, which has no size: a Focus/Ring instance rings the node it sits in, its host"]
    out = []
    want = (psize[0] + 2 * offset, psize[1] + 2 * offset)
    if not (same(rsize[0], want[0]) and same(rsize[1], want[1])):
        out.append(f"size is {rsize[0]:g} x {rsize[1]:g}, not its parent {pname!r} ({psize[0]:g} x {psize[1]:g}) grown by SPACE_FACT on "
                   f"every side ({want[0]:g} x {want[1]:g}): the parent is the host the ring wraps")
    t = ring.get("transform") or {}
    if not all(isinstance(t.get(k), (int, float)) for k in ("a", "b", "c", "d", "tx", "ty")):
        return out + [f"has no transform: it must sit at (-{offset:g}, -{offset:g}) in {pname!r}"]
    if not (same(t["tx"], -offset) and same(t["ty"], -offset)):
        out.append(f"is at ({t['tx']:g}, {t['ty']:g}) in {pname!r}: grown by SPACE_FACT on every side puts it at (-{offset:g}, -{offset:g})")
    if not (same(t["a"], 1.0) and same(t["d"], 1.0) and same(t["b"], 0.0) and same(t["c"], 0.0)):
        out.append("is scaled or rotated: a ring is its host's rect grown by SPACE_FACT, as drawn")
    return out


def host_radii(parent: dict) -> List[float]:
    """The four corner radii the parent draws: stored, clamped to half its smaller side (the engine and epaint clamp the same way)."""
    size = _xy(parent.get("size")) or (0.0, 0.0)
    half = min(size) / 2
    stored = [float(r) for r in (parent.get("corner_radii") or [])][:4]
    return [min(r, half) for r in stored + [0.0] * (4 - len(stored))]


def ring_problems(doc: dict, node: dict) -> List[str]:
    """Every way the Focus/Ring instance `node` disagrees with the node it sits in, as lines that start with the property they concern.
    Independent of the allow-list and of any binding: a zero corner is checked too."""
    parent = doc["nodes"].get(str(node.get("parent"))) or {}
    offset = ring_offset(doc)
    out = _geometry_problems(node, parent, offset)
    if _xy(parent.get("size")) is None or _xy(node.get("size")) is None:
        return out  # the parent is no host: its radius means nothing
    stored = [float(r) for r in (node.get("corner_radii") or [])][:4]
    stored += [0.0] * (4 - len(stored))
    if len({*stored}) > 1:
        out.append(f"corners are not equal ({', '.join(f'{r:g}' for r in stored)}): Rust draws one CornerRadius::same(round(radius + SPACE_FACT)) "
                   f"(kit/focus.rs:38), so a ring has a single radius")
    for key, have, host in zip(CORNERS, stored, host_radii(parent)):
        want = rust_round(host + offset)
        if not same(have, want):
            out.append(f"{key} is {have:g} but its host {parent.get('name')!r} draws radius {host:g}: the ring is drawn at {want:g} "
                       f"(host radius + SPACE_FACT, rounded, kit/focus.rs:38)")
    return out
