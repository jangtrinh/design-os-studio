"""Edge maps (thick and thin) and the structure map made from them."""

from __future__ import annotations

from PIL import Image, ImageChops, ImageFilter

from .config import EDGE_T, FAINT_MIN, FAINT_T, H, W
from .skeleton import components, frame_off, keys_of, skeletonize


def edge_band(im: Image.Image, threshold: int) -> Image.Image:
    """Thick edges: the 3x3 range of each channel. A step edge is 2 px wide, a 2 px line 4 px."""
    grad = None
    for band in im.split():
        g = ImageChops.subtract(band.filter(ImageFilter.MaxFilter(3)), band.filter(ImageFilter.MinFilter(3)))
        grad = g if grad is None else ImageChops.lighter(grad, g)
    return frame_off(grad.point([255 if v >= threshold else 0 for v in range(256)]))


def edge_marks(im: Image.Image, threshold: int) -> Image.Image:
    """Thin edges: a pixel is marked when it differs from its right or lower neighbour, so a step edge is 1 px wide
    and two features 2 px apart stay apart. Used for the document at 17% zoom, where everything is 1-2 px."""
    step = None
    for band in im.split():
        for dx, dy in ((-1, 0), (0, -1)):
            d = ImageChops.difference(band, ImageChops.offset(band, dx, dy))
            step = d if step is None else ImageChops.lighter(step, d)
    return frame_off(step.point([255 if v >= threshold else 0 for v in range(256)]))


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
