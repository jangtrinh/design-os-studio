"""The comparable view of a node for the override gate (studio_ui_gate_overrides): the fields an instance may not override, by the override field that owns them.

A node's frame props sit under `kind.<Frame|Component|ComponentSet|Section>` and, for an Instance, under `kind.Instance.frame`. The frame's own sizing modes (`primary_sizing`, `counter_sizing`) are
placement, like `layout_child` and `constraints`: `set_sizing` (v0.7.5 panels/properties.rs:1107-1130) writes a Fill child's `layout_child` as Fill and its own sizing as Fixed, so a nested copy of a hugging
main that fills its row differs from the main in them. They do not count for an Instance (the copy a module resizes) against its main, provided it carries the `Layout` override
flag: `record_override` raises it for a changed `frame_props` or `layout_child`, and `own_sizing_differs` tells the gate when one is owed. For any other mirrored node, a frame inside
a copy, they do count, as the gap, padding, direction, alignment, clipping and grids of every frame do.
"""
from __future__ import annotations

from typing import Any

FRAME_KINDS = ("Frame", "Component", "ComponentSet", "Instance", "Section")
SIZING_MODES = ("primary_sizing", "counter_sizing")


def _body(node: dict) -> dict:
    kind = node["kind"]
    body = next(iter(kind.values())) if isinstance(kind, dict) else {}
    return body if isinstance(body, dict) else {}


def _frame(node: dict, ignore_sizing: bool = False) -> dict:
    """The frame props (auto layout, clipping, grids) of a frame-like node: an Instance keeps them under `frame`, without its component link; `ignore_sizing` leaves out the own sizing modes.
    Any other kind (text, vector, ellipse, ...) has none."""
    kind = node["kind"]
    name = next(iter(kind)) if isinstance(kind, dict) else None
    if name not in FRAME_KINDS:
        return {}
    body = _body(node)
    frame = body.get("frame") if name == "Instance" else body
    out = {k: v for k, v in (frame or {}).items() if k not in ("component", "component_key")}
    if ignore_sizing and isinstance(out.get("auto_layout"), dict):  # the own sizing modes of a copy are placement (see the module docstring)
        out["auto_layout"] = {k: v for k, v in out["auto_layout"].items() if k not in SIZING_MODES}
    return out


def own_sizing_differs(node: dict, src: dict) -> bool:
    """True when the copy `node` holds an own sizing mode its main `src` does not hold or holds differently (a key only the main holds is no difference)."""
    mine, theirs = (_frame(n).get("auto_layout") or {} for n in (node, src))
    mine, theirs = (a if isinstance(a, dict) else {} for a in (mine, theirs))
    return any(k in mine and (k not in theirs or not _equal(mine[k], theirs[k])) for k in SIZING_MODES)


def style_view(node: dict, ignore_pad_left: bool = False, ignore_sizing: bool = False) -> dict:
    """The fields an instance may not override, by the override field that owns them. `ignore_sizing` is for a node that is itself an instance (and for its main, which it is compared with): only the
    copy a module resizes writes its own sizing, so a frame INSIDE a copy keeps its main frame's."""
    frame = _frame(node, ignore_sizing)
    if ignore_pad_left and isinstance(frame.get("auto_layout"), dict):
        pad = list(frame["auto_layout"].get("padding") or [])
        frame = dict(frame, auto_layout=dict(frame["auto_layout"], padding=pad[:3] + [None] if pad else pad))
    is_text = isinstance(node["kind"], dict) and "Text" in node["kind"]
    text = {k: v for k, v in _body(node).items() if k != "content"} if is_text else {}
    return {"Opacity": node.get("opacity", 1.0), "Blend": node.get("blend", "Normal"), "Fills": node.get("fills") or [],
            "Stroke": node.get("stroke") or {}, "Effects": node.get("effects") or [],
            "CornerRadii": [node.get("corner_radii"), node.get("corner_smoothing") or 0.0],  # smoothing is skipped in the JSON when 0
            "TextStyle": [text, node.get("style_refs") or {}],
            "Layout": frame}


def _equal(a: Any, b: Any) -> bool:
    if isinstance(a, bool) or isinstance(b, bool) or a is None or b is None:
        return a is b or a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= 1e-3 + 1e-6 * abs(b)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_equal(a[k], b[k]) for k in a)
    if isinstance(a, (list, tuple)) and isinstance(b, (list, tuple)):
        return len(a) == len(b) and all(_equal(x, y) for x, y in zip(a, b))
    return a == b
