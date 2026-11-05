"""Constants shared by the tracer modules (thresholds, layer names, 8-neighbour offsets)."""

from __future__ import annotations

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
RING = [(dy << 12) + dx for dx, dy in ((0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1))]
