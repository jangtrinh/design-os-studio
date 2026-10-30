#!/usr/bin/env python3
"""Trace a Studio render into the six hairline drawings of the homepage "anatomy" scene.

  python3 scripts/anatomy_trace.py RENDER.png --out docs/images/v2/anatomy \\
      [--source TEXT] [--webp FILE] [--log FILE] [--evidence DIR --chrome PATH]

RENDER.png is a 2880x1800 `designos-studio --screenshot --scale 2` capture of a 1440x900 window.
Output: file.svg canvas.svg layers.svg inspector.svg rail.svg chrome.svg plus anatomy.json (the files
docs/assets/v2-anatomy.js loads). Needs Python 3 and Pillow; numpy is not used. The run is
deterministic: the same render gives byte-identical files.

Method
  1. Edges. Two detectors on the colour channels. Thick: the 3x3 range of each channel, so a step edge is
     2 px wide and a 2 px border 4 px. Thin: a pixel differs from its right or lower neighbour, so a step
     is 1 px and two features 2 px apart stay apart. Thick is used everywhere except inside the document
     (the viewport interior, minus the tool bar and the selection overlay), which is drawn at 17% zoom, where
     text, borders and fields are 1-2 px and thick edges would fuse a label to its field. An edge counts at
     >= EDGE_T (12 levels). A fainter edge (>= FAINT_T, 8: the 245 page against a 255 frame) counts only
     when its skeleton runs >= FAINT_MIN px, so frame outlines survive and antialiasing halos do not.
  2. Structure map. Each edge map is thinned to a 1 px, 8-connected skeleton (Zhang-Suen, then an L-corner
     pass, both 3x3 LUTs run by Pillow). A 2 px border thins to its centre line. A skeleton pixel (x, y)
     becomes the lattice vertex (x + 1, y + 1): the centre of an even-width band is a pixel boundary. Every
     vertex is an integer source px, i.e. a multiple of 0.5 logical px.
  3. Splits. Borders are measured, never hard-coded: a 2 px line whose two edges cross >= 85% of the window
     is a border. The four full-height ones are, left to right, the rail, the Layers panel, the vertical
     ruler and the inspector; the full-width ones are the tab bar bottom and the horizontal ruler bottom.
     A border belongs to the panel whose edge it is.
  4. Layers by vertex position: chrome (tab bar), rail, layers (Pages and Layers), inspector, canvas
     (rulers, tool bar, selection overlay) and file (the document inside the viewport). Inside the
     viewport a stroke is canvas when it lies within CYAN_R px of the overlay cyan (selection box, handles,
     frame names, size pill) or in the tool bar, else file. Every skeleton pixel is assigned once, so the
     six maps never overlap.
  5. Text. Per layer, straight runs >= LINE_MIN (200) px (borders, dividers) are set aside so a label touching a
     border is still found. A skeleton component is a text candidate when it fits a line of text (<= 34 px
     across, <= 200 along; <= 12 across in the document), is not an icon (a near-square blob >= 22 px, or
     >= 30 x 28), a tick (thin and >= 24 long) or an outlined field (skeleton on all four sides of its box).
     Candidates on one line, no further apart than 0.95 of the glyph height (0.6 in the document), join
     into a run. Two or more candidates, or one component at least 2.2x as long as it is tall, are a text
     run. The vertical ruler's rotated numbers are grouped along y. A run is ONE bar at its pixel-weighted
     centre line, from its first to its last ink. A lone glyph or icon stays an outline.
  6. Shapes. The remaining skeleton is traced into chains that meet at junctions (a pixel whose 8-ring
     has >= 3 runs), simplified with Douglas-Peucker (0.5 source px; 0.8 in the document) and axis snapped
     (a segment whose cross-axis step is <= 1 px becomes exactly horizontal or vertical). Components under
     3 px are dropped.
  7. Gate. Recall and precision of the rasterised drawing against the structure map (text runs collapsed
     to their bars), with a 2 px Chebyshev tolerance, as specified; the same with the drawing shifted 6 px,
     to show the metric reacts; and an independent check against a Sobel gradient of the render.
     Layers never overlap: the maps by construction, the rasterised strokes by measurement.

Coordinates in the SVGs are logical px (viewBox 0 0 1440 900), relative `l` segments, stroke 1.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageMorph

SRC = 2                                   # source px per logical px
W, H = 2880, 1800
VIEW = (1440, 900)
EDGE_T = 12                               # channel step (thick: 3x3 range) that makes an edge pixel
FAINT_T, FAINT_MIN = 8, 60                # a fainter edge (the 245 vs 255 page and frame fill) counts only as a long line
CYAN = (6, 182, 212)                      # the selection overlay colour (#06B6D4)
CYAN_TOL, CYAN_R = 40, 3
DP_EPS, DOC_DP_EPS = 0.5, 0.8              # Douglas-Peucker tolerance, source px; the 17% document needs no round corners
TOL = 2                                   # recall/precision tolerance, source px (Chebyshev)
SHIFT = 6                                 # shifted-drawing sensitivity test
SOBEL_STRONG, SOBEL_VISIBLE = 15, 6       # independent check: a strong edge (recall) and any visible edge (precision), grey levels
GLYPH_MAX, WORD_MAX, ICON_ACROSS, ICON_ALONG = 34, 200, 30, 28    # candidate size limits, source px
ICON_SQUARE, ICON_ASPECT = 22, 1.25                                # a squarish blob this big is an icon
BOX_FRAC, BOX_ALONG, TICK_ACROSS, WORD_ASPECT, WORD_ACROSS = 0.8, 40, 24, 2.2, 4
GAP_RATIO, GAP_MIN, GAP_SCAN = 0.95, 4, 40
DOC_GLYPH_MAX, DOC_GAP_RATIO = 12, 0.6          # the document is drawn at 17% zoom: its text is under 12 px tall
LINE_MIN = 200                            # a straight run this long is a border or divider, not part of a glyph
BAND = 16                                 # line bucket height for the run search
SPECK = 3                                 # skeleton components below this are dropped
LAYERS = ("file", "canvas", "layers", "inspector", "rail", "chrome")  # bottom to top, as v2-anatomy.js
LABELS = {"file": "The .fig document", "canvas": "Canvas", "layers": "Layers panel",
          "inspector": "Inspector", "rail": "Agents rail", "chrome": "Window chrome"}
COLOURS = {"file": "#ffd60a", "canvas": "#30d5c8", "layers": "#ff453a",
           "inspector": "#0a84ff", "rail": "#bf5af2", "chrome": "#32d74b"}

OFFS = [(dx, dy) for dy in (-1, 0, 1) for dx in (-1, 0, 1) if dx or dy]
OFFS.sort(key=lambda d: (abs(d[0]) + abs(d[1]) != 1, d[1], d[0]))   # orthogonal steps first
OFF8 = [(dy << 12) + dx for dx, dy in OFFS]
DIR = {(dy << 12) + dx: (dx, dy) for dx, dy in OFFS}


# ---------------------------------------------------------------- structure map
def edge_band(im: Image.Image, threshold: int) -> Image.Image:
    """Thick edges: the 3x3 range of each channel. A step edge is 2 px wide, a 2 px line 4 px."""
    grad = None
    for band in im.split():
        g = ImageChops.subtract(band.filter(ImageFilter.MaxFilter(3)), band.filter(ImageFilter.MinFilter(3)))
        grad = g if grad is None else ImageChops.lighter(grad, g)
    return _frame_off(grad.point([255 if v >= threshold else 0 for v in range(256)]))


def edge_marks(im: Image.Image, threshold: int) -> Image.Image:
    """Thin edges: a pixel is marked when it differs from its right or lower neighbour, so a step edge is 1 px wide
    and two features 2 px apart stay apart. Used for the document at 17% zoom, where everything is 1-2 px."""
    step = None
    for band in im.split():
        for dx, dy in ((-1, 0), (0, -1)):
            d = ImageChops.difference(band, ImageChops.offset(band, dx, dy))
            step = d if step is None else ImageChops.lighter(step, d)
    return _frame_off(step.point([255 if v >= threshold else 0 for v in range(256)]))


def _frame_off(mask: Image.Image) -> Image.Image:
    ImageDraw.Draw(mask).rectangle((0, 0, W - 1, H - 1), outline=0)    # the morph LUT skips the frame
    return mask


def _bits(i):
    b = [(i >> j) & 1 for j in range(9)]       # row-major 3x3, bit 4 is the centre
    return b[4], [b[1], b[2], b[5], b[8], b[7], b[6], b[3], b[0]]      # centre, N NE E SE S SW W NW


def _zhang_suen_luts():
    luts = (bytearray(512), bytearray(512))
    for i in range(512):
        c, n = _bits(i)
        b = sum(n)
        a = sum(1 for k in range(8) if n[k] == 0 and n[(k + 1) % 8] == 1)
        ok = c and 2 <= b <= 6 and a == 1
        d1 = ok and n[0] * n[2] * n[4] == 0 and n[2] * n[4] * n[6] == 0
        d2 = ok and n[0] * n[2] * n[6] == 0 and n[0] * n[4] * n[6] == 0
        luts[0][i], luts[1][i] = (0 if d1 else c), (0 if d2 else c)
    return luts


def _corner_lut():
    lut = bytearray(512)
    for i in range(512):
        c, n = _bits(i)
        only = [k for k in range(8) if n[k]]
        lut[i] = 0 if c and len(only) == 2 and set(only) in ({0, 2}, {2, 4}, {4, 6}, {0, 6}) else c
    return lut


def skeletonize(mask: Image.Image) -> Image.Image:
    a, b = (ImageMorph.MorphOp(lut=l) for l in _zhang_suen_luts())
    corner = ImageMorph.MorphOp(lut=_corner_lut())
    while True:
        n1, mask = a.apply(mask)
        n2, mask = b.apply(mask)
        if n1 + n2 == 0:
            break
    while True:
        n, mask = corner.apply(mask)
        if n == 0:
            return mask


def keys_of(mask: Image.Image) -> list[int]:
    return [(m.start() // W << 12) | (m.start() % W) for m in re.finditer(b"\xff", mask.tobytes())]


def skeleton_map(im: Image.Image, edges) -> Image.Image:
    """Skeleton of the edges >= EDGE_T, plus faint edges (>= FAINT_T, away from strong ones) that run >= FAINT_MIN px."""
    strong = edges(im, EDGE_T)
    faint = ImageChops.subtract(edge_band(im, FAINT_T), strong.filter(ImageFilter.MaxFilter(5)))
    out = skeletonize(strong)
    long_faint = Image.new("L", (W, H), 0)
    px = long_faint.load()
    for comp in components(keys_of(skeletonize(faint))):
        if len(comp) >= FAINT_MIN:
            for k in comp:
                px[k & 4095, k >> 12] = 255
    return ImageChops.lighter(out, long_faint)


def structure_keys(im: Image.Image, zone: Image.Image) -> list[int]:
    """The structure map as flat keys (y << 12 | x): thin edges inside the document zone, thick edges everywhere else."""
    thick, thin = skeleton_map(im, edge_band), skeleton_map(im, edge_marks)
    return keys_of(ImageChops.lighter(ImageChops.subtract(thick, zone), ImageChops.darker(thin, zone)))


# ---------------------------------------------------------------- splits
def _border_pairs(counts: list[int], lo: int, hi: int, frac: float) -> list[int]:
    thr = frac * 255
    return [e for e in range(lo, hi - 2) if counts[e] >= thr and counts[e + 2] >= thr]


def _edge_counts(im: Image.Image, axis: str, box) -> list[int]:
    """Per row ('y') or column ('x'): the share of the box, 0-255, where a pixel differs from the one before."""
    prev = ImageChops.offset(im, 0, 1) if axis == "y" else ImageChops.offset(im, 1, 0)
    d = ImageChops.difference(im, prev)
    m = ImageChops.lighter(ImageChops.lighter(*d.split()[:2]), d.split()[2]).point([255 if v >= EDGE_T else 0 for v in range(256)])
    m = m.crop(box)
    m = m.resize((1, m.height), Image.BOX) if axis == "y" else m.resize((m.width, 1), Image.BOX)
    return list(m.tobytes())


def measure_splits(im: Image.Image) -> dict:
    rows = [0] * H
    for y, v in enumerate(_edge_counts(im, "y", (0, 0, W, H))):
        rows[y] = v
    chrome = _border_pairs(rows, 20, 400, 0.85)
    if not chrome:
        raise SystemExit("no full-width tab bar border found")
    cb = chrome[0] + 2
    cols = [0] * W
    for x, v in enumerate(_edge_counts(im, "x", (0, cb, W, H))):
        cols[x] = v
    vert = _border_pairs(cols, 0, W, 0.85)
    groups = [e for i, e in enumerate(vert) if i == 0 or e - vert[i - 1] > 2]   # one entry per line
    if len(groups) != 4:
        raise SystemExit(f"expected 4 full-height borders (rail, layers, ruler, inspector), found {groups}")
    rail, layers, ruler, insp = groups
    vp = [0] * H
    for y, v in enumerate(_edge_counts(im, "y", (ruler + 2, 0, insp, H))):
        vp[y] = v
    ruler_rows = [e for e in _border_pairs(vp, cb, 400, 0.85) if e > cb]
    if not ruler_rows:
        raise SystemExit("no horizontal ruler border found")
    return {"chrome_bottom": cb, "rail_right": rail + 2, "layers_right": layers + 2,
            "ruler_left_right": ruler + 2, "ruler_top_bottom": ruler_rows[0] + 2, "inspector_left": insp}


def toolbar_box(im: Image.Image, sp: dict):
    """The dark tool bar pill at the foot of the viewport, grown by 2 px."""
    x0, y0, x1 = sp["ruler_left_right"], H - 400, sp["inspector_left"]
    dark = im.crop((x0, y0, x1, H)).convert("L").point([255 if v < 80 else 0 for v in range(256)])
    b = dark.getbbox()
    if not b:
        raise SystemExit("no tool bar found")
    return (x0 + b[0] - 2, y0 + b[1] - 2, x0 + b[2] + 2, y0 + b[3] + 2)


def cyan_mask(im: Image.Image) -> Image.Image:
    m = None
    for band, c in zip(im.split(), CYAN):
        one = band.point([255 if abs(v - c) <= CYAN_TOL else 0 for v in range(256)])
        m = one if m is None else ImageChops.darker(m, one)
    return m


def size_pill(cy: Image.Image, sp: dict):
    """Bounding box of the filled cyan blob nearest the viewport's foot (the "W x H" pill)."""
    sub = cy.crop((sp["ruler_left_right"], sp["ruler_top_bottom"], sp["inspector_left"], H - 200))
    data, w = sub.tobytes(), sub.width
    best = None
    for y in range(sub.height - 1, 3, -1):
        row = data[y * w:(y + 1) * w]
        run = re.search(b"\xff{40,}", row)       # a fill, not a 2 px stroke
        if run and all(data[(y - k) * w + run.start() + 8] for k in range(1, 4)):
            best = (y, run)
            break
    if not best:
        return None
    y, run = best
    x = run.start() + 8
    y0 = y
    while y0 > 0 and data[(y0 - 1) * w + x]:
        y0 -= 1
    x0 = x
    while x0 > 0 and data[y * w + x0 - 1]:
        x0 -= 1
    x1 = x
    while x1 < w - 1 and data[y * w + x1 + 1]:
        x1 += 1
    ox, oy = sp["ruler_left_right"], sp["ruler_top_bottom"]
    return (ox + x0, oy + y0, ox + x1 + 1, oy + y + 1)


# ---------------------------------------------------------------- layers
def file_zone(sp, tool, near_cyan: Image.Image) -> Image.Image:
    """255 where a stroke is the document: the viewport interior, minus the tool bar and the selection overlay."""
    z = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(z)
    d.rectangle((sp["ruler_left_right"], sp["ruler_top_bottom"], sp["inspector_left"] - 2, H - 1), fill=255)
    d.rectangle((tool[0], tool[1], tool[2] - 1, tool[3] - 1), fill=0)
    return ImageChops.subtract(z, near_cyan)


def assign_layers(keys, sp, zone: bytes) -> dict:
    out = {k: [] for k in LAYERS}
    cb, rr, lr = sp["chrome_bottom"], sp["rail_right"], sp["layers_right"]
    rx, ry, il = sp["ruler_left_right"], sp["ruler_top_bottom"], sp["inspector_left"]
    for k in keys:
        x, y = k & 4095, k >> 12
        vx, vy = x + 1, y + 1                  # the lattice vertex of this skeleton pixel
        if vy <= cb:
            name = "chrome"
        elif vx <= rr:
            name = "rail"
        elif vx <= lr:
            name = "layers"
        elif vx >= il:
            name = "inspector"
        elif vx <= rx or vy <= ry:
            name = "canvas"
        elif zone[y * W + x]:
            name = "file"
        else:
            name = "canvas"
        out[name].append(k)
    return out


def components(keys: list[int]) -> list[list[int]]:
    live, seen, out = set(keys), set(), []
    for k in keys:
        if k in seen:
            continue
        seen.add(k)
        stack, comp = [k], []
        while stack:
            p = stack.pop()
            comp.append(p)
            for o in OFF8:
                q = p + o
                if q in live and q not in seen:
                    seen.add(q)
                    stack.append(q)
        comp.sort()
        out.append(comp)
    return out


# ---------------------------------------------------------------- text runs
def _box(comp):
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
        box = _box(c)
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


# ---------------------------------------------------------------- shape tracing
RING = [(dy << 12) + dx for dx, dy in ((0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1))]


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


# ---------------------------------------------------------------- one layer
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


# ---------------------------------------------------------------- svg
def num(v: float) -> str:
    """A logical-px value (multiple of 0.5) without a leading zero: 3.5, .5, -.5, 12."""
    s = str(int(v)) if v == int(v) else f"{v:.1f}"
    return s.replace("0.", ".", 1) if s.startswith("0.") else s.replace("-0.", "-.", 1)


def path_d(polys) -> str:
    parts = []
    for pts, closed in sorted(polys, key=lambda pc: ([(y, x) for x, y in pc[0]], pc[1])):
        (x0, y0), segs, px, py = pts[0], [], pts[0][0], pts[0][1]
        for x, y in pts[1:]:
            segs += [num((x - px) / SRC), num((y - py) / SRC)]
            px, py = x, y
        parts.append(f"M{num(x0 / SRC)} {num(y0 / SRC)}l{' '.join(segs)}" + ("z" if closed else ""))
    return "".join(parts)


def svg_text(shape_d: str, text_d: str) -> str:
    attr = 'vector-effect="non-scaling-stroke"'
    paths = "".join(f'<path data-kind="{k}" {attr} d="{d}"/>' for k, d in (("shape", shape_d), ("text", text_d)) if d)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEW[0]} {VIEW[1]}"><g fill="none" stroke="currentColor" '
            f'stroke-width="1" {attr} stroke-linecap="round" stroke-linejoin="round">{paths}</g></svg>\n')


# ---------------------------------------------------------------- gate
def raster(polys, shift=(0, 0)) -> Image.Image:
    img = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(img)
    for pts, closed in polys:
        seq = [(x - 1 + shift[0], y - 1 + shift[1]) for x, y in pts]    # vertex -> skeleton pixel
        d.line(seq + ([seq[0]] if closed else []), fill=255, width=1)
    return img


def count(img) -> int:
    return img.histogram()[255]


def near(img) -> Image.Image:
    return img.filter(ImageFilter.MaxFilter(2 * TOL + 1))


def inter(a, b) -> int:
    return count(ImageChops.darker(a, b))


def ratio(a: int, b: int) -> float:
    return round(a / b, 5) if b else 1.0


def structure_map(layer) -> Image.Image:
    img = Image.new("L", (W, H), 0)
    px = img.load()
    for k in layer["shape_keys"]:
        px[k & 4095, k >> 12] = 255
    return ImageChops.lighter(img, raster(layer["bars"]))


def sobel(im: Image.Image) -> Image.Image:
    g = im.convert("L")
    kx, ky = (-1, 0, 1, -2, 0, 2, -1, 0, 1), (-1, -2, -1, 0, 0, 0, 1, 2, 1)
    out = None
    for k in (kx, ky):
        for sign in (1, -1):
            f = g.filter(ImageFilter.Kernel((3, 3), [sign * v for v in k], scale=4))
            out = f if out is None else ImageChops.lighter(out, f)
    return _frame_off(out)


def gate(layers: dict, im: Image.Image) -> dict:
    res, maps, drawn = {}, {}, {}
    for name, ly in layers.items():
        maps[name], drawn[name] = structure_map(ly), raster(ly["polys"] + ly["bars"])
    all_map = all_dr = None
    for name in LAYERS:
        all_map = maps[name] if all_map is None else ImageChops.lighter(all_map, maps[name])
        all_dr = drawn[name] if all_dr is None else ImageChops.lighter(all_dr, drawn[name])
    for name, s, d in [(n, maps[n], drawn[n]) for n in LAYERS] + [("ALL", all_map, all_dr)]:
        r = {"recall": ratio(inter(s, near(d)), count(s)), "precision": ratio(inter(d, near(s)), count(d))}
        for axis, sh in (("dx", (SHIFT, 0)), ("dy", (0, SHIFT))):
            ds = ImageChops.offset(d, *sh)
            r[f"shifted_{SHIFT}px_{axis}_recall"] = ratio(inter(s, near(ds)), count(s))
            r[f"shifted_{SHIFT}px_{axis}_precision"] = ratio(inter(ds, near(s)), count(ds))
        res[name] = r
    grad = sobel(im)
    strong = grad.point([255 if v >= SOBEL_STRONG else 0 for v in range(256)])
    visible = grad.point([255 if v >= SOBEL_VISIBLE else 0 for v in range(256)])
    zones, shapes_only = Image.new("L", (W, H), 0), Image.new("L", (W, H), 0)
    zd = ImageDraw.Draw(zones)
    for ly in layers.values():                  # text zones: the box of every run, grown by the tolerance
        for run in ly["runs"]:
            b = [_box(c) for c in run]
            zd.rectangle((min(v[0] for v in b) - TOL, min(v[1] for v in b) - TOL, max(v[2] for v in b) + TOL, max(v[3] for v in b) + TOL), fill=255)
        shapes_only = ImageChops.lighter(shapes_only, raster(ly["polys"]))
    outside = ImageChops.subtract(strong, zones)
    res["source_check"] = {
        "P_src": ratio(inter(shapes_only, near(visible)), count(shapes_only)),
        "R_src_strong15": ratio(inter(strong, near(all_dr)), count(strong)),
        "R_src_strong15_outside_text_zones": ratio(inter(outside, near(all_dr)), count(outside)),
        "R_src_text_zones_widened": ratio(inter(strong, ImageChops.lighter(near(all_dr), zones)), count(strong)),
    }
    res["stroke_overlap"] = sum(inter(drawn[a], drawn[b]) for i, a in enumerate(LAYERS) for b in LAYERS[i + 1:])
    res["map_px"] = {n: len(layers[n]["keys"]) for n in LAYERS}
    res["map_union"] = len({k for n in LAYERS for k in layers[n]["keys"]})
    return res


# ---------------------------------------------------------------- outputs
def anatomy_json(layers, sp, tool, pill, gate_res, source, sizes):
    def lg(v):
        return v / SRC
    cb, rr, lr = sp["chrome_bottom"], sp["rail_right"], sp["layers_right"]
    rx, ry, il = sp["ruler_left_right"], sp["ruler_top_bottom"], sp["inspector_left"]
    regions = {"file": (rx, ry, il, H), "canvas": (lr, cb, il, H), "layers": (rr, cb, lr, H),
               "inspector": (il, cb, W, H), "rail": (0, cb, rr, H), "chrome": (0, 0, W, cb)}
    meta = []
    for name in LAYERS:
        ly = layers[name]
        x0, y0, x1, y1 = bbox_of(ly)
        rx0, ry0, rx1, ry1 = regions[name]
        if name == "file":
            anchor, side = [x1, round((y0 + y1) / 2, 1)], "right (ink bbox edge)"
        elif name in ("layers", "canvas"):
            anchor, side = [lg(rx0 + rx1) / 2, VIEW[1]], "bottom"
        elif name == "inspector":
            anchor, side = [VIEW[0], VIEW[1] / 2], "right"
        elif name == "rail":
            anchor, side = [0, VIEW[1] / 2], "left"
        else:
            anchor, side = [VIEW[0] / 2, 0], "top"
        shape_n = sum(1 for _ in re.finditer("M", path_d(ly["polys"])))
        text_n = len(ly["bars"])
        meta.append({"id": name, "file": f"{name}.svg", "bbox": [x0, y0, x1 - x0, y1 - y0],
                     "region": [lg(rx0), lg(ry0), lg(rx1 - rx0), lg(ry1 - ry0)], "anchor": anchor,
                     "anchor_side": side, "label": LABELS[name], "bytes": sizes[name],
                     "path_elements": int(shape_n > 0) + int(text_n > 0), "subpaths": shape_n + text_n,
                     "shape_subpaths": shape_n, "text_bars": text_n})
    for m in meta:
        m["region"] = [int(v) if v == int(v) else v for v in m["region"]]
        m["anchor"] = [int(v) if v == int(v) else round(v, 1) for v in m["anchor"]]
    splits = {"note": "measured from the render (source px, 2 px = 1 logical px); border lines belong to the panel whose edge they are",
              "chrome_bottom_logical": lg(cb), "chrome_bottom_src": f"tab bar bottom border = rows {cb - 2}-{cb - 1}",
              "rail_right_logical": lg(rr), "rail_right_src": f"rail border = cols {rr - 2}-{rr - 1}",
              "layers_right_logical": lg(lr), "layers_right_src": f"left panel border = cols {lr - 2}-{lr - 1}",
              "inspector_left_logical": lg(il), "inspector_left_src": f"inspector border = cols {il}-{il + 1}",
              "ruler_left_right_logical": lg(rx), "ruler_left_right_src": f"vertical ruler right edge = cols {rx - 2}-{rx - 1}",
              "ruler_top_bottom_logical": lg(ry), "ruler_top_bottom_src": f"horizontal ruler bottom edge = rows {ry - 2}-{ry - 1}",
              "toolbar_rect_logical": [lg(tool[0]), lg(tool[1]), lg(tool[2]), lg(tool[3])],
              "toolbar_src": f"dark pill {tool[0] + 2}..{tool[2] - 2} x {tool[1] + 2}..{tool[3] - 2}"}
    if pill:
        splits["size_pill_logical"] = [lg(v) for v in pill]
    splits["canvas_vs_file"] = (f"inside the viewport interior a stroke is canvas when it lies within {CYAN_R} source px of the overlay cyan "
                                "(selection box, handles, frame names, size pill) or in the tool bar, else file")
    gate_json = {"tolerance_px": TOL,
                 "recall_precision": {n: gate_res[n] for n in list(LAYERS) + ["ALL"]},
                 "source_check": gate_res["source_check"],
                 "note": ("recall/precision compare the rasterised SVG with the structure map the strokes were extracted from (text runs "
                          "collapsed to their bars, as specified); source_check is the independent comparison with a Sobel gradient of the "
                          f"render: P_src = drawn shape px within {TOL} px of a gradient >= {SOBEL_VISIBLE}; R_src_strong15 = gradient >= {SOBEL_STRONG} px "
                          "within the tolerance of any drawn px; _outside_text_zones drops the boxes of the text runs; _text_zones_widened "
                          "counts a text-zone px as covered (the run is drawn as a bar)")}
    return {"viewBox": [0, 0, VIEW[0], VIEW[1]], "source": source,
            "scale": "source is 2880x1800 = 2x logical; every stroke vertex is rounded to 0.5 logical px (= 1 source px)",
            "layers": meta, "splits": splits, "gate": gate_json,
            "notes": ["Text runs are one bar each at the run's skeleton-weighted centre line; a glyph or icon with no neighbour on its line stays an outline",
                      "The vertical ruler's rotated numbers are grouped along y and become vertical bars",
                      "Skeleton components under 3 px outside text runs are dropped; tiny doc-matrix icons shrink to short bars or disappear",
                      "Rounded corners are polylines (Douglas-Peucker 0.5 source px = 0.25 logical px; 0.8 in the document), not arcs; straight segments within 1 px of an axis are snapped to it",
                      "The document is traced with thin edges and tighter text rules (it is drawn at 17% zoom); filled icon interiors and low-contrast detail are not reproduced"],
            "stroke_pixel_overlap_between_layers": gate_res["stroke_overlap"]}


def evidence(out: Path, render: Path, svg_dir: Path, chrome: str):
    """overlay-075.png: the six drawings in one colour over the render at 0.6; layers-colored-075.png: one colour per layer."""
    svgs = {n: (svg_dir / f"{n}.svg").read_text() for n in LAYERS}
    css = "html,body{margin:0;background:#000}.stage{position:relative;width:1440px;height:900px;overflow:hidden}.stage>*{position:absolute;left:0;top:0}.stage svg{width:1440px;height:900px;display:block}"

    def stack(colours):
        return "".join(f'<div style="color:{colours[n]}">{svgs[n]}</div>' for n in LAYERS)

    pages = {
        "overlay-075.png": f'<img src="{render.resolve().as_uri()}" width="1440" height="900" style="opacity:.6">' + stack({n: "#ff2d55" for n in LAYERS}),
        "layers-colored-075.png": stack(COLOURS),
    }
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for png, body in pages.items():
            page = Path(tmp) / (png + ".html")
            page.write_text(f'<!doctype html><meta charset=utf-8><style>{css}</style><div class="stage">{body}</div>')
            subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                            "--window-size=1440,900", f"--screenshot={out / png}", page.as_uri()], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("render", type=Path)
    ap.add_argument("--out", type=Path, required=True, help="directory that receives the six SVGs and anatomy.json")
    ap.add_argument("--source", default="", help="text for anatomy.json `source` (the sha256 of the render is appended)")
    ap.add_argument("--webp", type=Path, help="also write the render as a 1600x1000 webp here")
    ap.add_argument("--webp-quality", type=int, default=80)
    ap.add_argument("--log", type=Path, help="also write the printout here")
    ap.add_argument("--evidence", type=Path, help="write overlay-075.png and layers-colored-075.png here (needs --chrome)")
    ap.add_argument("--chrome", help="path of a headless-capable Chrome binary")
    a = ap.parse_args(argv)

    lines = []

    def say(s=""):
        print(s)
        lines.append(s)

    raw = a.render.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    im = Image.open(a.render).convert("RGB")
    if im.size != (W, H):
        raise SystemExit(f"expected a {W}x{H} render, got {im.size}")
    sp = measure_splits(im)
    tool = toolbar_box(im, sp)
    cy = cyan_mask(im)
    pill = size_pill(cy, sp)
    zone = file_zone(sp, tool, cy.filter(ImageFilter.MaxFilter(2 * CYAN_R + 1)))
    parts = assign_layers(structure_keys(im, zone), sp, zone.tobytes())

    def in_vertical_ruler(comp):                # the vertical ruler's numbers are rotated, so its runs go along y
        x0, y0, x1, y1 = _box(comp)
        return x1 + 1 <= sp["ruler_left_right"] and y0 + 1 > sp["ruler_top_bottom"]

    layers = {n: build_layer(n, parts[n], in_vertical_ruler if n == "canvas" else (lambda c: False)) for n in LAYERS}

    a.out.mkdir(parents=True, exist_ok=True)
    sizes = {}
    for n in LAYERS:
        ly = layers[n]
        svg = svg_text(path_d(ly["polys"]), path_d(ly["bars"]))
        (a.out / f"{n}.svg").write_text(svg)
        sizes[n] = len(svg.encode())
    res = gate(layers, im)
    source = (a.source + "; " if a.source else "") + f"sha256:{sha}"
    meta = anatomy_json(layers, sp, tool, pill, res, source, sizes)
    (a.out / "anatomy.json").write_text(json.dumps(meta, indent=1) + "\n")

    say(f"render {a.render.name} sha256 {sha}")
    say(f"splits (source px) {sp}")
    say(f"toolbar src box {tool}  size pill {pill}")
    say(f"map px per layer {res['map_px']} sum {sum(res['map_px'].values())} union {res['map_union']} pairwise overlap px {sum(res['map_px'].values()) - res['map_union']}")
    say(f"stroke pixel overlap between layers (rasterised drawings, 1 px) {res['stroke_overlap']}")
    total = sum(sizes.values()) + (a.out / "anatomy.json").stat().st_size
    say(f"deliverables {a.out} total bytes {total} = {total / 1024:.1f} KB (svgs {sum(sizes.values())} B)")
    for m in meta["layers"]:
        say(f"{m['id']} {m['bytes']} B paths {m['path_elements']} subpaths(M) {m['subpaths']} bars {m['text_bars']} bbox {m['bbox']} anchor {m['anchor']}")
    say(f"gate tolerance {TOL} px (Chebyshev)")
    for n in list(LAYERS) + ["ALL"]:
        r = res[n]
        say(f"  {n:9s} recall {r['recall'] * 100:6.2f}% precision {r['precision'] * 100:6.2f}%  "
            f"shifted {SHIFT}px dx R {r[f'shifted_{SHIFT}px_dx_recall'] * 100:5.1f}% P {r[f'shifted_{SHIFT}px_dx_precision'] * 100:5.1f}%  "
            f"dy R {r[f'shifted_{SHIFT}px_dy_recall'] * 100:5.1f}% P {r[f'shifted_{SHIFT}px_dy_precision'] * 100:5.1f}%")
    say(f"source check (Sobel) {res['source_check']}")

    if a.webp:
        Image.open(a.render).convert("RGB").resize((1600, 1000), Image.LANCZOS).save(a.webp, "WEBP", quality=a.webp_quality, method=6)
        say(f"webp {a.webp} {a.webp.stat().st_size} B")
    if a.evidence:
        if not a.chrome:
            raise SystemExit("--evidence needs --chrome")
        evidence(a.evidence, a.render, a.out, a.chrome)
        say(f"evidence {a.evidence}")
    if a.log:
        a.log.parent.mkdir(parents=True, exist_ok=True)
        a.log.write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
