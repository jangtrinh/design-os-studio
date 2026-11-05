"""Foreground figures for the component compare, without Pillow: an image is a flat row-major list of RGB tuples.

A whole-box mean difference cannot see a glyph drawn at the wrong size (a 14 px Close X against production's 10 px scored 1.41 of 255), so the
compare also measures the foreground, the glyph or label on the control's surface:

* the background is the surface: the colour most pixels of the image have (a control's fill, or the page under a label), so a pill's rounded end
  still has the pill as its surface. The foreground is searched in the image inset by `inset`, so rounded corners, a border ring and a focus band
  are never the foreground;
* a pixel is foreground when its distance from that background exceeds half of the interior's largest distance, so the extent does not depend on
  the glyph's colour; two boxes mismatch when their width or height differ by more than 1 px.

The pass/fail decision of a row lives here (`measure_pixels`; the verdict of a run, `bad_rows`, `exit_code`, `pending_note`, `fail_line` and the accepted findings, is `crop_report`): every entry checks its foreground
box, its surface colour and its strongest-pixel colour, and fails closed ("ambiguous background") when the surface it picked cannot be trusted:
the glyph or label is the majority colour, a mark covers the whole searched interior so there is no box, or two colours split the crop.
`colour_only` (a custom painter draws the glyph) exempts the box and loosens the strongest-pixel tolerance; `fg_pending` (a swapped 20 px icon the
engine resets to 16 px, bug #1427) exempts the box and both colours, only while the issue is open.
"""
import collections
import pathlib
import sys

PKG = str(pathlib.Path(__file__).resolve().parents[1])  # scripts/studio_ui_file, where swap_check lives
for path in (PKG, str(pathlib.Path(__file__).resolve().parent)):  # and this folder, where crop_windows lives
    if path not in sys.path:
        sys.path.insert(0, path)
import swap_check  # noqa: E402
from crop_windows import fg_window, max_diff, pad_image  # noqa: E402,F401  (re-exported: the compare and the tests reach them as crop_metrics.<name>)
from crop_gradient import gradient_fit  # noqa: E402,F401
from crop_report import CHECKS, bad_rows, exit_code, fail_line, pending_note, report_lines  # noqa: E402,F401  (the verdict of a run: crop_report)

MIN_PEAK = 16  # below this largest distance an image has no foreground
SURFACE_TOL = 6  # per channel: a control's surface is one flat theme colour in both images; the nearest two theme surfaces are 13 apart
PEAK_TOL = 8  # per channel: the strongest pixel of the same drawing differs only by anti-aliasing (every measured entry differs by 0 or 1)
PEAK_TOL_CUSTOM = 24  # `colour_only`: a custom painter's glyph is another drawing, so the anti-aliasing of its strokes differs more
AMBIGUOUS_LEAD = 0.2  # the most common colour must lead the second by this share of the pixels; the 26 real rows lead by at least 0.375
FLAT_LEAD = 0.98  # an image with no foreground box is a plain surface only when its surface colour owns this share of the pixels (lead over the second)


def _dist(p, bg):
    return max(abs(p[0] - bg[0]), abs(p[1] - bg[1]), abs(p[2] - bg[2]))


def surface_colour(pixels, w, h):
    """The colour most pixels of the image have, or None for an empty image."""
    return collections.Counter(pixels[:w * h]).most_common(1)[0][0] if pixels and w * h else None


def _inset(w, h, inset):
    return min(4, w // 4, h // 4) if inset is None else inset


def surface_lead(pixels, w, h):
    """The share of the pixels by which the most common colour leads the second most common (the whole share when there is only one)."""
    top = collections.Counter(pixels[:w * h]).most_common(2)
    return 0.0 if not top else (top[0][1] - (top[1][1] if len(top) > 1 else 0)) / (w * h)


def ambiguous_surface(pixels, w, h, inset=None):
    """True when the most common colour cannot be trusted as the surface: it leads the second by less than AMBIGUOUS_LEAD of the pixels, the
    foreground fills the searched interior on both axes (the glyph or label is then the majority colour, whatever the lead), or the searched
    interior is one flat colour while the image is not (a mark that covers the whole interior has no box to compare: only a plain surface, a
    lead of FLAT_LEAD or more, is a clear image without a foreground)."""
    lead = surface_lead(pixels, w, h)
    if lead < AMBIGUOUS_LEAD:
        return True
    inset = _inset(w, h, inset)
    box = fg_bbox(pixels, w, h, inset=inset)
    if box is None:
        return lead < FLAT_LEAD
    return bbox_size(box) == (w - 2 * inset, h - 2 * inset)


def _scan(pixels, w, h, inset):
    """(interior cells, background, largest distance from it)."""
    inset = _inset(w, h, inset)
    cells = [(x, y, pixels[y * w + x]) for y in range(inset, h - inset) for x in range(inset, w - inset)]
    if not cells:
        return cells, None, 0
    bg = surface_colour(pixels, w, h)
    return cells, bg, max(_dist(p, bg) for _, _, p in cells)


def fg_bbox(pixels, w, h, inset=None):
    """(x0, y0, x1, y1) of the foreground, x1 and y1 exclusive, or None when the interior is one flat colour."""
    cells, bg, peak = _scan(pixels, w, h, inset)
    if peak < MIN_PEAK:
        return None
    hot = [(x, y) for x, y, p in cells if _dist(p, bg) > peak / 2]
    xs, ys = [x for x, _ in hot], [y for _, y in hot]
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def peak_colour(pixels, w, h, inset=None):
    """The colour of the interior pixel farthest from the background (a glyph's strongest pixel), or None without a foreground. For a
    glyph that a custom painter draws, where only the colour can be compared."""
    cells, bg, peak = _scan(pixels, w, h, inset)
    return None if peak < MIN_PEAK else next(p for _, _, p in cells if _dist(p, bg) == peak)


def bbox_size(box):
    return None if box is None else (box[2] - box[0], box[3] - box[1])


def mismatch(render_box, prod_box):
    """{"d_width", "d_height", "ok"}: the render's box minus production's; ok when both differ by at most 1 px (None, None: both empty)."""
    a, b = bbox_size(render_box), bbox_size(prod_box)
    if a is None and b is None:
        return {"d_width": 0, "d_height": 0, "ok": True}
    if a is None or b is None:
        return {"d_width": None, "d_height": None, "ok": False}
    dw, dh = a[0] - b[0], a[1] - b[1]
    return {"d_width": dw, "d_height": dh, "ok": abs(dw) <= 1 and abs(dh) <= 1}


def _channel_diff(a, b):
    return None if a is None or b is None else max(abs(p - q) for p, q in zip(a, b))


def issue_open():
    """True while the engine bug behind the `fg_pending` entries (#1427) is open: swap_check.SWAP_CHECK_FATAL is still False. That constant
    flips in the commit that merges the fix, and the exemption lifts with it."""
    return not swap_check.SWAP_CHECK_FATAL


def measure_pixels(render, prod, entry, pending_open=None):
    """The foreground box, surface and strongest-pixel figures of two (pixels, w, h) images and the verdict for the crops.json `entry`.

    `pending_open` (default: `issue_open()`): #1427 is open, so an `fg_pending` entry is exempt from the box, both colours and the background
    check (its `pending` is the issue id, and the output says so); once the issue is closed the exemption is ignored and the entry is checked
    like any other. Returns the
    boxes (`box_*`) and their sizes (`fg_*`), `fg` (mismatch), `surface_diff`, `peak_diff`, `ambiguous_render` and `ambiguous_prod` (the
    surface pick of that image is not trustworthy) and `fg_fails`, `surface_fails`, `peak_fails`, `ambiguous_fails`, `pending`, `bad`."""
    if entry.get("gradient"):  # a gradient has no surface and no mark: compared pixel by pixel (crop_gradient)
        fit = gradient_fit(render, prod)
        return {"box_render": None, "box_prod": None, "fg_render": None, "fg_prod": None, "fg": {"d_width": None, "d_height": None, "ok": True}, "surface_render": None, "surface_prod": None,
                "surface_diff": None, "peak_render": None, "peak_prod": None, "peak_diff": None, "fg_fails": False, "surface_fails": False, "peak_fails": False, "ambiguous_render": False,
                "ambiguous_prod": False, "ambiguous_fails": False, "pending": None, "gradient": True, **fit, "fit_fails": not fit["fit_ok"], "bad": not fit["fit_ok"]}
    inset = entry.get("fg_inset")
    rbox, pbox = fg_bbox(*render, inset=inset), fg_bbox(*prod, inset=inset)
    fg = mismatch(rbox, pbox)
    peaks = (peak_colour(*render), peak_colour(*prod))
    surfaces = (surface_colour(*render), surface_colour(*prod))
    surface_diff, peak_diff = _channel_diff(*surfaces), _channel_diff(*peaks)
    custom = bool(entry.get("colour_only"))
    pending = entry.get("fg_pending") if (issue_open() if pending_open is None else pending_open) else None
    peak_tol = PEAK_TOL_CUSTOM if custom else PEAK_TOL
    fg_fails = not fg["ok"] and not custom and not pending
    surface_fails = (surface_diff is None or surface_diff > SURFACE_TOL) and not pending
    peak_fails = (peaks[0] is None) != (peaks[1] is None) or (peak_diff is not None and peak_diff > peak_tol)
    peak_fails = peak_fails and not pending
    ambiguous = (ambiguous_surface(*render, inset), ambiguous_surface(*prod, inset))
    ambiguous_fails = any(ambiguous) and not pending
    return {"box_render": rbox, "box_prod": pbox, "fg_render": bbox_size(rbox), "fg_prod": bbox_size(pbox), "fg": fg, "surface_render": surfaces[0], "surface_prod": surfaces[1],
            "surface_diff": surface_diff, "peak_render": peaks[0], "peak_prod": peaks[1], "peak_diff": peak_diff, "fg_fails": fg_fails,
            "surface_fails": surface_fails, "peak_fails": peak_fails, "ambiguous_render": ambiguous[0], "ambiguous_prod": ambiguous[1],
            "ambiguous_fails": ambiguous_fails, "pending": pending, "bad": fg_fails or surface_fails or peak_fails or ambiguous_fails}
