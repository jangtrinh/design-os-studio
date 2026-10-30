"""Skeleton pixels to chains to simplified polylines."""

from __future__ import annotations

from .config import DIR, OFF8, RING


def trace_chains(keys: list[int]) -> list[tuple[list[int], bool]]:
    """Chains of 8-connected skeleton px, as (px list, closed). Chains meet at junctions: px whose 8-ring has >= 3
    runs of set px (the crossing number). A 2 px thick diagonal has crossing number 2, so it is one chain."""
    live = set(keys)
    cross = {}
    for p in keys:
        ring = [(p + o) in live for o in RING]
        cross[p] = sum(1 for k in range(8) if not ring[k] and ring[(k + 1) % 8])
    nodes = {p for p in keys if cross[p] >= 3}
    seen: set[int] = set()
    chains = []

    def walk(start, first=None):
        path = [start]
        seen.add(start)
        cur, prev = start, None
        if first is not None:
            path.append(first)
            seen.add(first)
            prev, cur = DIR[first - start], first
        while first is None and cur == start or cur not in nodes:
            cands = [o for o in OFF8 if cur + o in live and cur + o not in seen]
            if not cands:
                break
            o = cands[0] if prev is None else max(cands, key=lambda c: DIR[c][0] * prev[0] + DIR[c][1] * prev[1])
            cur += o
            prev = DIR[o]
            path.append(cur)
            seen.add(cur)
        closed = False
        if cur not in nodes:
            for o in OFF8:
                r = cur + o
                if len(path) >= 4 and r == path[0]:
                    closed = True
                    break
                if r in nodes and r not in path[-3:] and r != path[0]:
                    path.append(r)
                    break
        if len(path) >= 2:
            chains.append((path, closed))

    for p in keys:
        if cross[p] <= 1 and p not in seen:
            walk(p)
    for j in sorted(nodes):
        for o in OFF8:
            q = j + o
            if q in live and q not in seen and q not in nodes:
                walk(j, q)
    cover = {p + o for p in seen for o in OFF8}        # px already within 1 px of a chain: leftovers there are shadows
    for p in keys:
        if p in seen:
            continue
        if p in cover:
            seen.add(p)
            continue
        n = len(chains)
        walk(p)
        if len(chains) > n:
            cover.update(q + o for q in chains[-1][0] for o in OFF8)
    return chains


def simplify(pts, eps):
    n = len(pts)
    keep = [False] * n
    keep[0] = keep[-1] = True
    stack = [(0, n - 1)]
    while stack:
        i, j = stack.pop()
        (ax, ay), (bx, by) = pts[i], pts[j]
        dx, dy = bx - ax, by - ay
        den = dx * dx + dy * dy
        far, at = eps * eps, -1
        for k in range(i + 1, j):
            px, py = pts[k]
            if den == 0:
                d = (px - ax) ** 2 + (py - ay) ** 2
            else:
                t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / den))
                d = (px - ax - t * dx) ** 2 + (py - ay - t * dy) ** 2
            if d > far:
                far, at = d, k
        if at >= 0:
            keep[at] = True
            stack += [(i, at), (at, j)]
    return [p for p, k in zip(pts, keep) if k]


def snap(pts):
    out = [pts[0]]
    for x, y in pts[1:]:
        px, py = out[-1]
        if abs(x - px) <= 1 and abs(y - py) >= 4:
            x = px
        elif abs(y - py) <= 1 and abs(x - px) >= 4:
            y = py
        if (x, y) != out[-1]:
            out.append((x, y))
    return out


def polyline(path, closed, eps):
    """Vertices (source lattice px) of one traced chain, or None when it collapses to a point."""
    pts = [((p & 4095) + 1, (p >> 12) + 1) for p in path]
    if closed:
        pts.append(pts[0])
    pts = snap(simplify(pts, eps))
    if closed:
        if len(pts) > 1 and pts[-1] == pts[0]:
            pts.pop()
        if len(pts) < 3:
            closed = False
    if not closed and (pts[-1][1], pts[-1][0]) < (pts[0][1], pts[0][0]):
        pts.reverse()
    if len(pts) < 2:
        return None
    if closed:
        r = min(range(len(pts)), key=lambda i: (pts[i][1], pts[i][0]))
        pts = pts[r:] + pts[:r]
    return pts, closed
