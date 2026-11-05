"""Command line entry: render in, six SVGs, anatomy.json, webp, log and evidence out."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageFilter

from . import __doc__ as package_doc
from .config import CYAN_R, H, LAYERS, SHIFT, TOL, W
from .edges import structure_keys
from .layer import build_layer
from .metrics import gate
from .outputs import anatomy_json, evidence
from .splits import assign_layers, cyan_mask, file_zone, measure_splits, size_pill, toolbar_box
from .svg import path_d, svg_text
from .text import comp_box


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=package_doc.split("\n")[0])
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
        x0, y0, x1, y1 = comp_box(comp)
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
