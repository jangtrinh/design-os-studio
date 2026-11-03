"""Where a vector glyph is drawn: the bounds of its geometry, placed through its transform. Used by the icon-swap check (swap_check).

    path_bounds(paths)      (x0, y0, x1, y1) of native paths (`cmds`: MoveTo, LineTo, QuadTo, CubicTo, Close); curves are sampled, so the
                            handles that stick out of an arc do not widen the bounds. None when nothing is drawn
    geometry_bounds(node)   the bounds of what the renderer draws for a vector node: `fill_geometry` first, the kind's paths without it
                            (`designos-render/src/geom.rs:416-420`)
    placed(bounds, t)       the bounds of the box after the node's transform {a, b, c, d, tx, ty} (None: identity)
    scaled(bounds, sx, sy)  the bounds of a glyph resized by (sx, sy)
"""
STEPS = 32  # samples per curve: the error of a sampled extremum is far below a hundredth of a pixel on a 16 px icon


def _point(v):
    return float(v["x"]), float(v["y"])


def _mix(points, t):
    """De Casteljau: the point at `t` of the Bezier whose control points are `points` (2 for a line, 3 quadratic, 4 cubic)."""
    while len(points) > 1:
        points = [((1 - t) * a[0] + t * b[0], (1 - t) * a[1] + t * b[1]) for a, b in zip(points, points[1:])]
    return points[0]


def path_bounds(paths):
    pts = []
    for path in paths or []:
        cur = start = None
        for cmd in path.get("cmds", []):
            if cmd == "Close":
                cur = start
                continue
            (kind, v), = cmd.items()
            if kind in ("MoveTo", "LineTo"):
                cur = _point(v)
                start = cur if kind == "MoveTo" else start
                pts.append(cur)
            elif kind in ("QuadTo", "CubicTo") and cur is not None:
                ctrl = [_point(p) for p in v]
                pts += [_mix([cur, *ctrl], i / STEPS) for i in range(1, STEPS + 1)]
                cur = ctrl[-1]
    return (min(p[0] for p in pts), min(p[1] for p in pts), max(p[0] for p in pts), max(p[1] for p in pts)) if pts else None


def geometry_bounds(node):
    """The bounds of what the renderer draws for the vector `node`, in the node's own space."""
    own = node.get("fill_geometry")
    kind = node.get("kind")
    return path_bounds(own if own else (kind.get("Vector") or {}).get("paths") if isinstance(kind, dict) else None)


def placed(bounds, t):
    if bounds is None or not t:
        return bounds
    x0, y0, x1, y1 = bounds
    corners = [(t["a"] * x + t["c"] * y + t["tx"], t["b"] * x + t["d"] * y + t["ty"]) for x in (x0, x1) for y in (y0, y1)]
    return min(c[0] for c in corners), min(c[1] for c in corners), max(c[0] for c in corners), max(c[1] for c in corners)


def scaled(bounds, sx, sy):
    return None if bounds is None else (bounds[0] * sx, bounds[1] * sy, bounds[2] * sx, bounds[3] * sy)
