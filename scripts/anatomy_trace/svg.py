"""Path data and SVG text in the format docs/assets/v2-anatomy.js loads."""

from __future__ import annotations

from .config import SRC, VIEW


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
