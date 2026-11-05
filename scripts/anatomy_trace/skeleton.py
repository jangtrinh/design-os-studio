"""Thinning to a 1 px skeleton and connected components, on flat pixel keys (y << 12 | x)."""

from __future__ import annotations

import re

from PIL import Image, ImageDraw, ImageMorph

from .config import H, OFF8, W


def frame_off(mask: Image.Image) -> Image.Image:
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
