"""One layer: set long lines aside, find text runs, trace what is left."""

from __future__ import annotations

from .config import DOC_DP_EPS, DP_EPS, LINE_MIN, SPECK, SRC
from .skeleton import components
from .text import bar_of, text_runs
from .tracing import polyline, trace_chains


def long_lines(keys: list[int]) -> set[int]:
    """Skeleton px on a perfectly straight horizontal or vertical run of >= LINE_MIN px."""
    live, out = set(keys), set()
    cols: dict[int, list[int]] = {}
    for k in keys:
        cols.setdefault(k & 4095, []).append(k >> 12)
    runs = []
    row = []
    for k in keys:                              # keys are sorted: one row after another, left to right
        if row and k - row[-1] == 1:
            row.append(k)
        else:
            runs.append(row)
            row = [k]
    runs.append(row)
    for x, ys in cols.items():
        col = []
        for y in sorted(ys):
            if col and y - (col[-1] >> 12) == 1:
                col.append((y << 12) | x)
            else:
                runs.append(col)
                col = [(y << 12) | x]
        runs.append(col)
    for r in runs:
        if len(r) >= LINE_MIN:
            out.update(r)
    return out & live


def build_layer(name, keys, vertical_zone):
    """Text runs become bars; everything else is traced into polylines. Long straight lines are set aside
    while looking for text, so a label touching a border is still found."""
    lines = long_lines(keys)
    comps = components([k for k in keys if k not in lines])
    horizontal = [c for c in comps if not vertical_zone(c)]
    vertical = [c for c in comps if vertical_zone(c)]
    bars, taken, runs_all = [], set(), []
    for group, vert in ((horizontal, False), (vertical, True)):
        for run in text_runs(group, vert, doc=name == "file"):
            bars.append(bar_of(group, run, vert))
            runs_all.append([group[i] for i in run])
            taken.update(p for i in run for p in group[i])
    rest = [k for k in keys if k not in taken]
    shape_keys = sorted(p for c in components(rest) if len(c) >= SPECK for p in c)
    eps = DOC_DP_EPS if name == "file" else DP_EPS
    polys = [r for r in (polyline(p, c, eps) for p, c in trace_chains(shape_keys)) if r]
    bar_polys = [([a, b], False) for a, b in bars if a != b]
    return {"name": name, "keys": keys, "shape_keys": shape_keys, "polys": polys, "bars": bar_polys, "runs": runs_all}


def bbox_of(layer):
    xs = [x for pts, _ in layer["polys"] + layer["bars"] for x, _ in pts]
    ys = [y for pts, _ in layer["polys"] + layer["bars"] for _, y in pts]
    return min(xs) / SRC, min(ys) / SRC, max(xs) / SRC, max(ys) / SRC
