"""The gate: recall and precision against the structure map, and the independent Sobel check."""

from __future__ import annotations

from PIL import Image, ImageChops, ImageDraw, ImageFilter

from .config import H, LAYERS, SHIFT, SOBEL_STRONG, SOBEL_VISIBLE, TOL, W
from .skeleton import frame_off
from .text import comp_box


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
    return frame_off(out)


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
            b = [comp_box(c) for c in run]
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
