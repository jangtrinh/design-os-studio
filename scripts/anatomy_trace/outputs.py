"""anatomy.json and the evidence images."""

from __future__ import annotations

import re
import subprocess
import tempfile
from pathlib import Path

from .config import COLOURS, CYAN_R, H, LABELS, LAYERS, SOBEL_STRONG, SOBEL_VISIBLE, SRC, TOL, VIEW, W
from .layer import bbox_of
from .svg import path_d


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
