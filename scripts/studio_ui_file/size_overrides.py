"""The `Size`, `CornerRadii` and `Layout` overrides the editor records on the instances inside a main, in a saved native `.designos`.

The editor flags what an edit changed on every node that mirrors a main node, so later syncs keep it (`record_overrides`, crates/designos-app/src/comp_ops.rs:60-106;
`record_override`, designos-doc/src/components.rs:465-485). A top instance (an Instance whose `component.source` is its own component) drops only Transform,
Visible and Locked: a size that differs is `Size`, corners that differ are `CornerRadii`. A node inside an instance drops Size when it is derived, so a
text that sizes itself and the children of a hugging auto layout carry none; only a text's `Characters` is recorded. A sync copies every field an instance
does not override (components.rs:1069-1131): without the flags a main sync would reset a resized ring to its main's size, a pinned ring to its main's 12 and a
nested Menu/Item that fills its row to the main's own sizing (`copy_field` Layout copies `layout_child`, :1032-1036).
The REST import records none, so the generator does.

    counts = record(doc)      flag every top instance inside a main whose size, corners, layout_child or frame props differ from its main's, and the glyph of a
                              resized `Icon/*` slot: {"slots", "glyphs", "instances", "radii", "layout"}. `slots` are the Icon/* instances, `instances`
                              every other top instance flagged Size, `radii` the CornerRadii flags added, `layout` the Layout flags added
    problems = missing(doc)   what still lacks a flag ([] once `record` has run); the library build asserts this stays empty

An icon slot's glyph (its first child, against the first child of the Icon main) keeps the pairing designos #1427 needs: a 20 px tool, rail or icon-button
glyph is the 16 px slot resized, the editor records Size on the slot and on its glyph, and without it a swap resets the slot to 16 or needs the engine to
promote Min/Min vector constraints to Scale. A top instance is only checked inside a Component (a main), not as a specimen on a page.
"""
OVERRIDE = "Size"
RADII = "CornerRadii"
LAYOUT = "Layout"
TOL = 0.01
FRAME_KINDS = ("Frame", "Component", "ComponentSet", "Instance", "Section")


def _instance(n):
    kind = n.get("kind")
    inst = kind.get("Instance") if isinstance(kind, dict) else None
    return inst if inst and inst.get("component") is not None else None


def _in_component(nodes, n):
    parent = n.get("parent")
    while parent is not None:
        p = nodes.get(str(parent))
        if p is None:
            return False
        if isinstance(p.get("kind"), dict) and "Component" in p["kind"]:
            return True
        parent = p.get("parent")
    return False


def _size_differs(a, b):
    sa, sb = a.get("size") or {}, b.get("size") or {}
    return any(abs((sa.get(k) or 0.0) - (sb.get(k) or 0.0)) > TOL for k in ("x", "y"))


def _radii(n):
    r = [float(x) for x in (n.get("corner_radii") or [])][:4]
    return r + [0.0] * (4 - len(r))


def _smoothing(n):
    """`corner_smoothing` is skipped in the JSON when it is zero (designos-doc node.rs), so a missing key is 0.0."""
    return float(n.get("corner_smoothing") or 0.0)


def _radii_differ(a, b):
    """The editor's CornerRadii test: the four radii or the corner smoothing differ (`record_override`, components.rs:465)."""
    return any(abs(x - y) > TOL for x, y in zip(_radii(a), _radii(b))) or abs(_smoothing(a) - _smoothing(b)) > TOL


def _frame_props(n):
    """The frame props of a frame-like node (the editor's `kind.frame_props()`): an Instance keeps them under `frame`, a Component or Frame in its body, without the component link. {} for a node with none."""
    k = n.get("kind")
    name = next(iter(k)) if isinstance(k, dict) else None
    body = k[name] if name else None
    frame = (body.get("frame") if name == "Instance" else body) if isinstance(body, dict) and name in FRAME_KINDS else None
    return {a: v for a, v in (frame or {}).items() if a not in ("component", "component_key")}


def _layout_differs(a, b):
    """The editor's Layout test: `kind.frame_props()` or `layout_child` differ (`record_override`, components.rs:481-486); a copy that sets its own sizing differs in the frame."""
    return a.get("layout_child") != b.get("layout_child") or _frame_props(a) != _frame_props(b)


def _first_child(nodes, n):
    kids = n.get("children") or []
    return nodes.get(str(kids[0])) if kids else None


def entries(doc):
    """(instance, main, glyph or None, the glyph of the Icon main or None) for every top instance inside a main, in node id order. The glyphs
    are only given for an `Icon/*` main."""
    nodes = doc["nodes"]
    for _, n in sorted(nodes.items(), key=lambda kv: int(kv[0])):
        inst = _instance(n)
        main = nodes.get(str(inst["component"])) if inst else None
        if main is None or (n.get("component") or {}).get("source") != inst["component"] or not _in_component(nodes, n):
            continue
        icon = str(main.get("name", "")).startswith("Icon/")
        yield n, main, (_first_child(nodes, n) if icon else None), (_first_child(nodes, main) if icon else None)


def _has(n, field):
    return field in ((n.get("component") or {}).get("overrides") or [])


def _gaps(doc):
    """(node, field, what, name, problem) for every flag the editor would have recorded and the file lacks."""
    for n, main, glyph, main_glyph in entries(doc):
        what = "slot" if str(main.get("name", "")).startswith("Icon/") else "instance"
        if _size_differs(n, main) and not _has(n, OVERRIDE):
            s, m = n["size"], main["size"]
            yield (n, OVERRIDE, what, n["name"],
                   f"is {s['x']:g} x {s['y']:g}, its {main['name']} main is {m['x']:g} x {m['y']:g}, and it has no {OVERRIDE} override")
        if _radii_differ(n, main) and not _has(n, RADII):
            yield (n, RADII, what, n["name"],
                   f"has corners {_radii(n)} (smoothing {_smoothing(n):g}), its {main['name']} main has {_radii(main)} (smoothing {_smoothing(main):g}), and it has no {RADII} override")
        if _layout_differs(n, main) and not _has(n, LAYOUT):
            yield (n, LAYOUT, what, n["name"],
                   f"has layout_child {n.get('layout_child')} and frame {_frame_props(n)}, its {main['name']} main has {main.get('layout_child')} and {_frame_props(main)}, and it has no {LAYOUT} override")
        if glyph is not None and main_glyph is not None and _size_differs(glyph, main_glyph) and not _has(glyph, OVERRIDE):
            g, m = glyph["size"], main_glyph["size"]
            yield (glyph, OVERRIDE, "glyph", f"{n['name']}/{glyph['name']}",
                   f"is {g['x']:g} x {g['y']:g}, its {main['name']} glyph is {m['x']:g} x {m['y']:g}, and it has no {OVERRIDE} override")


def missing(doc):
    return [{"id": n["id"], "what": what, "name": name, "problem": problem} for n, _, what, name, problem in _gaps(doc)]


def record(doc):
    counts = {"slots": 0, "glyphs": 0, "instances": 0, "radii": 0, "layout": 0}
    for n, field, what, _, _ in list(_gaps(doc)):
        n.setdefault("component", {}).setdefault("overrides", []).append(field)
        counts[{RADII: "radii", LAYOUT: "layout"}.get(field) or {"slot": "slots", "glyph": "glyphs", "instance": "instances"}[what]] += 1
    return counts
