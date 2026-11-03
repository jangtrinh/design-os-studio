"""The verdict of a crop that is a gradient, without Pillow (an image is a flat row-major list of RGB tuples).

The foreground box, surface and strongest-pixel checks (crop_metrics) need a flat surface and a mark on it. A gradient strip has neither: every pixel is a colour, no colour most pixels have is a
surface, so those checks read "ambiguous background". A gradient row (`gradient: true` in crops.json) is compared pixel by pixel instead, over the two crops' common size:

* the sizes agree within 1 px on each axis (the window is the strip, not a part of it);
* the mean of the largest channel difference per pixel is at most `MEAN_TOL`, which a wrong stop, a wrong hue or a flat colour in place of the ramp exceeds by a wide margin, and a 1 px shift
  of a 200 px ramp does not (a ramp of 255 levels over 172 px changes by 1.5 per px);
* the 99th percentile of that per-pixel difference is at most `P99_TOL`: the few pixels of a rounded cap where one renderer's anti-aliasing differs from the other's (up to 154 on 18 of 1848
  px, measured on the hue strip) are under 1 %, a wrong drawing is not.

Unlike the other checks this one sees where things are: a ramp drawn 4 px to the side fails.
"""
MEAN_TOL = 2.0  # mean per-pixel largest-channel difference, 0..255 (measured 0.56 to 0.86 on the four gradient rows of crops.json)
P99_TOL = 8  # 99th percentile of the same (measured 1 on all three)
SIZE_TOL = 1


def pixel_diffs(render, prod):
    """The largest channel difference of each pixel over the common top-left w x h of two (pixels, w, h) images, plus that size."""
    (rp, rw, rh), (pp, pw, ph) = render, prod
    w, h = min(rw, pw), min(rh, ph)
    return [max(abs(a - b) for a, b in zip(rp[y * rw + x], pp[y * pw + x])) for y in range(h) for x in range(w)], (w, h)


def gradient_fit(render, prod):
    """{"fit_mean", "fit_p99", "fit_size_ok", "fit_ok"} of two (pixels, w, h) images. An empty common area never fits."""
    diffs, _ = pixel_diffs(render, prod)
    size_ok = abs(render[1] - prod[1]) <= SIZE_TOL and abs(render[2] - prod[2]) <= SIZE_TOL
    if not diffs:
        return {"fit_mean": None, "fit_p99": None, "fit_size_ok": size_ok, "fit_ok": False}
    ordered = sorted(diffs)
    mean, p99 = round(sum(diffs) / len(diffs), 2), ordered[max(0, -(-len(ordered) * 99 // 100) - 1)]
    return {"fit_mean": mean, "fit_p99": p99, "fit_size_ok": size_ok, "fit_ok": size_ok and mean <= MEAN_TOL and p99 <= P99_TOL}
