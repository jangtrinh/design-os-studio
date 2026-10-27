#!/usr/bin/env python3
"""Measure left and right pixel gaps around button labels in a rendered PNG.

Used by issue #1154 (US-18, AC-02) to verify that hug-sized auto-layout buttons
labelled "Delete", "Export", and "Share" have symmetric left/right glyph gaps
(differing by <= 0.5 px at 1x) when sized via NodeSpec.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from PIL import Image


BUTTON_SPECS = [
    ("Delete", (0xDC, 0x26, 0x26)),
    ("Export", (0x25, 0x63, 0xEB)),
    ("Share", (0x16, 0xA3, 0x4A)),
]


def color_close(rgb: tuple[int, int, int], target: tuple[int, int, int], tol: int = 18) -> bool:
    return (
        abs(rgb[0] - target[0]) <= tol
        and abs(rgb[1] - target[1]) <= tol
        and abs(rgb[2] - target[2]) <= tol
    )


def measure_button(
    img: Image.Image,
    label: str,
    fill_rgb: tuple[int, int, int],
    scale: float,
    x_range: tuple[int, int],
    y_range: tuple[int, int],
) -> dict:
    px = img.load()
    x0, x1 = x_range
    y0, y1 = y_range

    xs: list[int] = []
    ys: list[int] = []
    for y in range(y0, y1):
        for x in range(x0, x1):
            c = px[x, y][:3]
            if color_close(c, fill_rgb):
                xs.append(x)
                ys.append(y)

    if not xs:
        raise RuntimeError(f"Button {label!r} with fill {fill_rgb} not found in image region")

    top_y = min(ys)
    bot_y = max(ys)
    mid_y = (top_y + bot_y) // 2
    band_half = max(2, (bot_y - top_y) // 6)

    # Measure button outer left/right edges across the vertical middle band (avoiding rounded corners).
    band_xs = [
        x
        for y in range(mid_y - band_half, mid_y + band_half + 1)
        for x in range(x0, x1)
        if color_close(px[x, y][:3], fill_rgb)
    ]
    btn_left = min(band_xs)
    btn_right = max(band_xs)

    # Find white glyph ink pixels inside the button's interior using the 50% coverage midpoint rule.
    margin_y = max(2, int(round(3 * scale)))
    margin_x = max(2, int(round(2 * scale)))
    ink_xs: list[int] = []
    for y in range(top_y + margin_y, bot_y - margin_y + 1):
        for x in range(btn_left + margin_x, btn_right - margin_x + 1):
            rgb = px[x, y][:3]
            cov = sum((c - f) / (255.0 - f) for c, f in zip(rgb, fill_rgb) if 255 > f) / 3.0
            if cov >= 0.50:
                ink_xs.append(x)

    if not ink_xs:
        raise RuntimeError(f"No glyph ink pixels found inside button {label!r}")

    ink_left = min(ink_xs)
    ink_right = max(ink_xs)

    left_gap_dev = ink_left - btn_left
    right_gap_dev = btn_right - ink_right
    left_gap_1x = round(left_gap_dev / scale, 2)
    right_gap_1x = round(right_gap_dev / scale, 2)
    diff_1x = round(abs(left_gap_1x - right_gap_1x), 2)
    btn_width_1x = round((btn_right - btn_left + 1) / scale, 2)
    ink_width_1x = round((ink_right - ink_left + 1) / scale, 2)

    return {
        "label": label,
        "button_px": [btn_left, top_y, btn_right, bot_y],
        "button_width_1x": btn_width_1x,
        "ink_width_1x": ink_width_1x,
        "left_gap_1x": left_gap_1x,
        "right_gap_1x": right_gap_1x,
        "gap_diff_1x": diff_1x,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("png", type=Path, help="Path to rendered screenshot PNG")
    parser.add_argument("--scale", type=float, default=3.0, help="Effective canvas zoom/scale in pixels per 1x logical unit (default: 3.0)")
    parser.add_argument("--max-diff", type=float, default=None, help="Fail with exit code 1 if any button's |left_gap - right_gap| exceeds this threshold (in 1x px)")
    args = parser.parse_args()

    img = Image.open(args.png).convert("RGBA")
    w, h = img.size
    # If full Studio window (e.g. 1440x900), restrict scan to the canvas viewport so toolbar/inspector chrome is ignored.
    if w >= 1000 and h >= 600:
        x_range = (int(w * 0.18), int(w * 0.80))
        y_range = (int(h * 0.08), int(h * 0.90))
    else:
        x_range = (0, w)
        y_range = (0, h)

    results = []
    for label, fill_rgb in BUTTON_SPECS:
        results.append(measure_button(img, label, fill_rgb, args.scale, x_range, y_range))

    out = {"png": str(args.png), "scale": args.scale, "buttons": results}
    print(json.dumps(out, indent=2))

    if args.max_diff is not None:
        failed = [r for r in results if r["gap_diff_1x"] > args.max_diff]
        if failed:
            for r in failed:
                print(
                    f"FAIL: {r['label']} gap_diff_1x={r['gap_diff_1x']} > {args.max_diff} "
                    f"(left={r['left_gap_1x']}, right={r['right_gap_1x']})",
                    file=sys.stderr,
                )
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
