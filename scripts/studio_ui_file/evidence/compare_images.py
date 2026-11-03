"""Pillow side of the component compare: crops, background composite, the score of one render against one production crop, sheets.

`measure(render, prod, entry)` returns the whole-box mean and the max absolute RGB difference, the heat map, and the figures and verdict
of crop_metrics.measure_pixels (foreground boxes, surface colour, strongest-pixel colour; the decision is there, without Pillow, so
the tests reach it). Entry keys that change it: `mask_glyph` (ignore the centre 24 px for the mean and max: the icon of an icon-only
control differs from the set's default), `render_rect` [x, y, w, h] (compare only that part of the render), `render_pad` [left, top, right, bottom] (add that many px of the entry's `bg` around the
render, applied after `render_rect`: a bare control against the surface its production crop shows), `align` "fg" (crop both
images to their foreground plus 2 px of their own surface first: for a label whose crop rect is only approximate), `colour_only` (the
glyph is a custom painter: the box is exempt, the strongest pixel gets a wider tolerance) and `fg_pending` (exempt while #1427 is open).
"""
import json
import math

from PIL import Image, ImageChops, ImageDraw, ImageOps

from evidence import crop_metrics as cm

BG = (10, 10, 10)
ZOOM = 4


def crop_of(entry, captures):
    d = json.loads((captures / f"{entry['case']}.layout.json").read_text())
    x, y, w, h = entry["rect"] if "rect" in entry else d["named"][entry["named"]]["rect"]
    im = Image.open(captures / f"{entry['case']}.png").convert("RGB")
    return im.crop((math.floor(x), math.floor(y), math.ceil(x + w), math.ceil(y + h))), (w, h)


def over_bg(path, bg=BG):
    im = Image.open(path).convert("RGBA")
    return Image.alpha_composite(Image.new("RGBA", im.size, tuple(bg) + (255,)), im).convert("RGB")


def _px(im):
    return list(im.getdata())


def _fg_crop(im, box, pad=2):
    """The foreground box plus `pad` px of the image's own surface on every side (a label at the edge is not clamped on one side only)."""
    pixels, w, h = cm.fg_window(_px(im), im.width, im.height, box, pad, cm.surface_colour(_px(im), im.width, im.height))
    if box is None:
        return im
    out = Image.new("RGB", (w, h))
    out.putdata(pixels)
    return out


def measure(render, prod, entry):
    if entry.get("render_rect"):
        x, y, w, h = entry["render_rect"]
        render = render.crop((x, y, x + w, y + h))
    if entry.get("render_pad"):  # the surface round a bare control: its production crop shows it, so the render gets it too
        px, w, h = cm.pad_image(_px(render), render.width, render.height, entry["render_pad"], tuple(entry.get("bg", BG)))
        render = Image.new("RGB", (w, h))
        render.putdata(px)
    m = cm.measure_pixels((_px(render), *render.size), (_px(prod), *prod.size), entry)
    if entry.get("align") == "fg":
        render, prod = _fg_crop(render, m["box_render"]), _fg_crop(prod, m["box_prod"])
    w, h = min(render.width, prod.width), min(render.height, prod.height)
    a, b = render.crop((0, 0, w, h)), prod.crop((0, 0, w, h))
    diff = ImageChops.difference(a, b)
    mask = Image.new("L", (w, h), 255)
    if entry.get("mask_glyph") or entry.get("colour_only"):
        ImageDraw.Draw(mask).rectangle((w // 2 - 12, h // 2 - 12, w // 2 + 12, h // 2 + 12), fill=0)
    masked = [p for p, m_ in zip(_px(diff), mask.getdata()) if m_]
    mean = sum(sum(p) for p in masked) / (3 * len(masked)) if masked else 0.0
    heat = ImageOps.colorize(ImageOps.autocontrast(diff.convert("L").point(lambda v: min(255, v * 4)), cutoff=0), (0, 0, 0), (255, 140, 0), (200, 40, 0))
    return {**m, "mean": mean, "max": max((max(p) for p in masked), default=0), "render": render, "heat": heat.resize((w * ZOOM, h * ZOOM), Image.NEAREST)}


def sheet(rows, path):
    zoom = lambda im: im.resize((im.width * ZOOM, im.height * ZOOM), Image.NEAREST)
    panels = [(label, [zoom(r), zoom(p), h]) for label, r, p, h in rows]
    ruler = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    width = max(max(sum(i.width for i in ims) + 24 for _, ims in panels), max(int(ruler.textlength(label)) + 8 for label, _ in panels))
    height = sum(max(i.height for i in ims) + 28 for _, ims in panels)
    out = Image.new("RGB", (width, height), (30, 30, 30))
    y = 0
    for label, ims in panels:
        ImageDraw.Draw(out).text((4, y + 4), label, fill=(255, 255, 255))
        x = 0
        for im in ims:
            out.paste(im, (x, y + 24))
            x += im.width + 12
        y += max(i.height for i in ims) + 28
    out.save(path)
