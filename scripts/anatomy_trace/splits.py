"""Borders and zones measured from the render, and the assignment of skeleton pixels to layers."""

from __future__ import annotations

import re

from PIL import Image, ImageChops, ImageDraw

from .config import CYAN, CYAN_TOL, EDGE_T, H, LAYERS, W


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
