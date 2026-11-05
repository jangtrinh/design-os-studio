"""Text runs: which skeleton components are text, and the bar each run becomes."""

from __future__ import annotations

from .config import BAND, BOX_ALONG, BOX_FRAC, DOC_GAP_RATIO, DOC_GLYPH_MAX, GAP_MIN, GAP_RATIO, GAP_SCAN, GLYPH_MAX, ICON_ACROSS, ICON_ALONG, ICON_ASPECT, ICON_SQUARE, TICK_ACROSS, WORD_ACROSS, WORD_ASPECT, WORD_MAX


def comp_box(comp):
    xs, ys = [p & 4095 for p in comp], [p >> 12 for p in comp]
    return min(xs), min(ys), max(xs), max(ys)


def _is_box(comp, box) -> bool:
    """An outlined rectangle: skeleton px sit within 2 px of all four sides of the bounding box, along >= BOX_FRAC of each."""
    x0, y0, x1, y1 = box
    top, bottom, left, right = set(), set(), set(), set()
    for p in comp:
        x, y = p & 4095, p >> 12
        if y - y0 <= 2:
            top.add(x)
        if y1 - y <= 2:
            bottom.add(x)
        if x - x0 <= 2:
            left.add(y)
        if x1 - x <= 2:
            right.add(y)
    w, h = x1 - x0 + 1, y1 - y0 + 1
    return min(len(top) / w, len(bottom) / w, len(left) / h, len(right) / h) >= BOX_FRAC


def text_runs(comps: list[list[int]], vertical: bool, doc: bool = False) -> list[list[int]]:
    """Indices of components that form text runs (see the module docstring, step 5). `doc`: micro text of the document."""
    glyph_max, gap_ratio = (DOC_GLYPH_MAX, DOC_GAP_RATIO) if doc else (GLYPH_MAX, GAP_RATIO)
    cand = []                                  # (u0, u1, v0, v1, index): u along the line, v across it
    for i, c in enumerate(comps):
        box = comp_box(c)
        x0, y0, x1, y1 = box
        u0, u1, v0, v1 = (y0, y1, x0, x1) if vertical else (x0, x1, y0, y1)
        along, across = u1 - u0 + 1, v1 - v0 + 1
        if across > glyph_max or along > WORD_MAX or (across >= ICON_ACROSS and along >= ICON_ALONG):
            continue                           # too big for a line of text, or an icon-sized square
        if across >= ICON_SQUARE and along >= ICON_SQUARE and along <= ICON_ASPECT * across:
            continue                           # a smaller icon
        if along <= 2 and across >= TICK_ACROSS:
            continue                           # a tick or divider, not a stem
        if not vertical and along >= BOX_ALONG and across >= 8 and _is_box(c, box):
            continue                           # an outlined field or pill
        cand.append((u0, u1, v0, v1, i))
    cand.sort()
    buckets: dict[int, list] = {}
    for n, c in enumerate(cand):
        buckets.setdefault((c[2] + c[3]) // 2 // BAND, []).append(n)
    parent = list(range(len(cand)))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    for n, (u0, u1, v0, v1, _) in enumerate(cand):
        for b in range((v0 + v1) // 2 // BAND - 1, (v0 + v1) // 2 // BAND + 2):
            for m in buckets.get(b, ()):
                if m <= n:
                    continue
                c = cand[m]
                if c[0] > u1 + 1 + GAP_SCAN:
                    break
                across_a, across_b = v1 - v0 + 1, c[3] - c[2] + 1
                overlap = min(v1, c[3]) - max(v0, c[2]) + 1
                gap = c[0] - u1 - 1
                if overlap >= 0.5 * min(across_a, across_b) and gap <= max(GAP_MIN, gap_ratio * max(across_a, across_b)):
                    parent[find(m)] = find(n)
    groups: dict[int, list] = {}
    for n in range(len(cand)):
        groups.setdefault(find(n), []).append(cand[n])
    runs = []
    for g in groups.values():
        if len(g) >= 2:
            runs.append(sorted(c[4] for c in g))
            continue
        u0, u1, v0, v1, i = g[0]               # one component alone is a word only when it is wide and tall enough to be one
        along, across = u1 - u0 + 1, v1 - v0 + 1
        if across >= WORD_ACROSS and along >= WORD_ASPECT * across:
            runs.append([i])
    return sorted(runs)


def bar_of(comps, run, vertical):
    """((x0, y0), (x1, y1)) vertices of the run's bar: ends at the ink extremes, centre at the pixel-weighted mean."""
    pix = [p for i in run for p in comps[i]]
    xs, ys = [(p & 4095) + 1 for p in pix], [(p >> 12) + 1 for p in pix]
    if vertical:
        c = round(sum(xs) / len(xs))
        return (c, min(ys)), (c, max(ys))
    c = round(sum(ys) / len(ys))
    return (min(xs), c), (max(xs), c)
