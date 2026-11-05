"""The pixel difference of a screen render against its capture, without Pillow (an image is a flat row-major list of RGB tuples), over the pixels no mask excludes.

A screen is compared with the 0.7.5 capture it draws. Two kinds of pixel are excluded, because they are not the chrome being drawn: the document's own content (the canvas area between the
rulers and the toolbar: a kit's frames in the capture, nothing in the screen) and the texts a screen replaces with neutral content (a kit's names). Every other pixel counts:

    mean      the mean over counted pixels of the mean absolute channel difference (0..255)
    p99       the 99th percentile of the largest channel difference per counted pixel
    share     the fraction of counted pixels whose largest channel difference is above `NOTICE` (24)
    worst     the 40 x 40 tile (at least half of it counted) with the highest mean, with its rect

    mask = exclusion_mask(w, h, rects)                       a bytearray of w * h, 1 = excluded; a rect is (x, y, w, h), clipped to the image
    diffs = pixel_diffs(render, capture)                     (largest channel difference per pixel, mean channel difference per pixel) as two flat lists
    stats = region_stats(diffs, w, rect, mask)               {"pixels", "mean", "p99", "share"} over the rect's counted pixels (rect None: the whole image)
    tile = worst_tile(diffs, w, h, mask)                     {"rect", "mean"} of the worst 40 x 40 tile
"""
NOTICE = 24
TILE = 40


def exclusion_mask(w, h, rects):
    mask = bytearray(w * h)
    for x, y, rw, rh in rects:
        x0, y0, x1, y1 = max(0, int(x)), max(0, int(y)), min(w, int(x + rw + 0.999)), min(h, int(y + rh + 0.999))
        for row in range(y0, y1):
            if x1 > x0:
                mask[row * w + x0:row * w + x1] = b"\x01" * (x1 - x0)
    return mask


def pixel_diffs(render, capture):
    """Two flat lists over the pixels of two equal-sized (pixels, w, h) images: the largest channel difference and the mean channel difference of each."""
    (rp, rw, rh), (cp, cw, ch) = render, capture
    if (rw, rh) != (cw, ch):
        raise ValueError(f"the render is {rw} x {rh}, the capture {cw} x {ch}: a screen is compared at the capture's size")
    big, mean = [], []
    for a, b in zip(rp, cp):
        d = (abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))
        big.append(max(d))
        mean.append((d[0] + d[1] + d[2]) / 3)
    return big, mean


def _indices(w, rect, mask, h):
    x, y, rw, rh = rect if rect else (0, 0, w, h)
    for row in range(max(0, int(y)), min(h, int(y + rh))):
        base = row * w
        for col in range(max(0, int(x)), min(w, int(x + rw))):
            if not mask[base + col]:
                yield base + col


def region_stats(diffs, w, rect, mask):
    big, mean = diffs
    h = len(big) // w
    idx = list(_indices(w, rect, mask, h))
    if not idx:
        return {"pixels": 0, "mean": None, "p99": None, "share": None}
    ordered = sorted(big[i] for i in idx)
    return {"pixels": len(idx), "mean": round(sum(mean[i] for i in idx) / len(idx), 3), "p99": ordered[max(0, -(-len(ordered) * 99 // 100) - 1)],
            "share": round(sum(1 for v in ordered if v > NOTICE) / len(ordered), 4)}


def worst_tile(diffs, w, h, mask):
    best = {"rect": None, "mean": -1.0}
    for ty in range(0, h, TILE):
        for tx in range(0, w, TILE):
            rect = (tx, ty, min(TILE, w - tx), min(TILE, h - ty))
            idx = list(_indices(w, rect, mask, h))
            if len(idx) * 2 < rect[2] * rect[3]:
                continue
            m = sum(diffs[1][i] for i in idx) / len(idx)
            if m > best["mean"]:
                best = {"rect": list(rect), "mean": round(m, 3)}
    return best
