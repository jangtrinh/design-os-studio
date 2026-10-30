#!/usr/bin/env python3
"""Pixel-fidelity harness for menus and the no-selection inspector.

Screenshots the debug app in matching states, crops the same regions out of the
Figma reference captures and ours, and runs `designos-diff compare` on each pair.

    cargo build -p designos-app -p designos-diff
    python scripts/measure_ui.py [--refs DIR] [--tolerance 8] [--keep]

The references (gitignored) default to <repo>/reference/screens, falling back to the main
checkout when run from a worktree. Crops and diff visualisations land in out/measure/.
"""
import argparse
import json
import os
import subprocess
import sys

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
from cargo_target import resolve as resolve_cargo_target

TARGET_DIR = str(resolve_cargo_target(ROOT))
EXE = os.path.join(TARGET_DIR, "debug", "designos.exe" if os.name == "nt" else "designos")
DIFF = os.path.join(TARGET_DIR, "debug", "designos-diff.exe" if os.name == "nt" else "designos-diff")

# name -> (reference file, our CLI args, crop rect x,y,w,h)
CASES = {
    "main-menu": ("figma-800x600-main-menu.png", ["--menu", "main"], (8, 40, 218, 384)),
    "main-menu-panel": ("figma-800x600-main-menu.png", ["--menu", "main"], (12, 44, 210, 380)),
    "shape-menu": ("figma-800x600-shape-menu.png", ["--menu", "shape"], (290, 352, 250, 192)),
    "shape-menu-panel": ("figma-800x600-shape-menu.png", ["--menu", "shape"], (299, 360, 232, 184)),
    "no-selection": ("figma-800x600-no-selection.png", ["--fixture", "no-selection"], (560, 81, 240, 245)),
    "toolbar": ("figma-800x600-no-selection.png", ["--fixture", "no-selection"], (144, 540, 512, 48)),
}


def find_refs(arg):
    for d in [arg, os.path.join(ROOT, "reference", "screens")]:
        if d and os.path.isdir(d):
            return d
    # worktree: <main>/.claude/worktrees/<name> -> <main>
    main = os.path.abspath(os.path.join(ROOT, "..", "..", ".."))
    d = os.path.join(main, "reference", "screens")
    if os.path.isdir(d):
        return d
    sys.exit("reference screenshots not found; pass --refs DIR")


def shoot(args, out):
    subprocess.run([EXE, "--screenshot", out, "--size", "800x600", "--select", "none", *args], check=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refs")
    ap.add_argument("--tolerance", type=int, default=8)
    ap.add_argument("--only", help="comma separated case names")
    ap.add_argument("--exe", help="override the app binary (for before/after runs)")
    ap.add_argument("--tag", default="", help="suffix for output files")
    a = ap.parse_args()
    global EXE
    if a.exe:
        EXE = os.path.abspath(a.exe)
    refs = find_refs(a.refs)
    out = os.path.join(ROOT, "out", "measure")
    os.makedirs(out, exist_ok=True)
    only = a.only.split(",") if a.only else list(CASES)
    shots = {}
    rows = []
    for name in only:
        ref_name, args, (x, y, w, h) = CASES[name]
        key = tuple(args)
        if key not in shots:
            path = os.path.join(out, f"ours{a.tag}-{len(shots)}.png")
            shoot(args, path)
            shots[key] = path
        box = (x, y, x + w, y + h)
        ref_crop = os.path.join(out, f"{name}-ref.png")
        our_crop = os.path.join(out, f"{name}{a.tag}-ours.png")
        diff_png = os.path.join(out, f"{name}{a.tag}-diff.png")
        Image.open(os.path.join(refs, ref_name)).convert("RGBA").crop(box).save(ref_crop)
        Image.open(shots[key]).convert("RGBA").crop(box).save(our_crop)
        r = subprocess.run([DIFF, "compare", ref_crop, our_crop, "--tolerance", str(a.tolerance), "--out", diff_png, "--json"], capture_output=True, text=True)
        try:
            rep = json.loads(r.stdout)
            rows.append((name, rep["ratio"], rep.get("mismatched", rep.get("diff_pixels", "?")), w * h))
        except Exception:
            rows.append((name, None, r.stdout.strip() or r.stderr.strip(), w * h))
    print(f"{'case':20} {'mismatch':>9}  pixels")
    for name, ratio, n, total in rows:
        print(f"{name:20} {('%.2f%%' % (ratio * 100)) if ratio is not None else 'error':>9}  {n}/{total}")


if __name__ == "__main__":
    main()
