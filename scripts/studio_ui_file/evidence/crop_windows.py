"""Pixel windows for the component compare, without Pillow (an image is a flat row-major list of RGB tuples): the largest channel difference of two
images, a window round a foreground box, and a margin of surface colour added round a bare control. Re-exported by crop_metrics."""


def max_diff(a, b, w, h):
    """The largest channel difference over the first w * h pixels of two equally wide images."""
    return max((max(abs(p[i] - q[i]) for i in range(3)) for p, q in zip(a[:w * h], b[:w * h])), default=0)


def fg_window(pixels, w, h, box, pad, fill):
    """(pixels, w, h) of the foreground `box` plus `pad` px on every side, taken from the image padded with `fill` (its own surface): a
    label that starts at the edge still gets `pad` px of surface on that side, so two images compare without a 1 px misregistration.
    `box` None returns the image."""
    if box is None:
        return pixels, w, h
    x0, y0, x1, y1 = box[0] - pad, box[1] - pad, box[2] + pad, box[3] + pad
    out = [pixels[y * w + x] if 0 <= x < w and 0 <= y < h else fill for y in range(y0, y1) for x in range(x0, x1)]
    return out, x1 - x0, y1 - y0


def pad_image(pixels, w, h, pad, fill):
    """(pixels, w, h) of the image with `pad` = (left, top, right, bottom) px of `fill` added around it. A render of a bare control gets the
    surface its production crop shows around it (crops.json `render_pad`), so both images have a surface to measure the control against."""
    left, top, right, bottom = pad
    if min(pad) < 0:
        raise ValueError(f"a pad margin cannot be negative: {tuple(pad)}")
    if not any(pad):
        return pixels, w, h
    nw, nh = w + left + right, h + top + bottom
    out = [fill] * (nw * nh)
    for y in range(h):
        out[(y + top) * nw + left:(y + top) * nw + left + w] = pixels[y * w:(y + 1) * w]
    return out, nw, nh
