#!/usr/bin/env python3
"""
Captures all 48 REAL sequential Retina 2x (2880x1800) screenshots directly from
`./target/release/designos-studio` across the 8 Figma A-to-Z Micro-Interaction suites
(`m01..m08`, steps `1..6`).
"""

from __future__ import annotations

import concurrent.futures
import os
import sys
import subprocess
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from cargo_target import resolve as resolve_cargo_target

TARGET_DIR = resolve_cargo_target(REPO_ROOT)
BIN = TARGET_DIR / "release" / "designos-studio"
OUT_DIR = REPO_ROOT / "Design" / "marketing" / "screenshots" / "micro"

SUITES = [
    ("m01", "micro-autolayout"),
    ("m02", "micro-css-grid"),
    ("m03", "micro-smart-selection"),
    ("m04", "micro-shape-controls"),
    ("m05", "micro-vector-edit"),
    ("m06", "micro-layer-picker"),
    ("m07", "micro-dev-boxmodel"),
    ("m08", "micro-scale-tool"),
]


def capture_one(task: tuple[int, int, str, str, int]) -> tuple[str, float, int]:
    idx, total, code, panel_base, step = task
    out_path = OUT_DIR / f"{code}-step{step}.png"
    panel_arg = f"{panel_base}:{step}"
    cmd = [
        str(BIN),
        "--fixture",
        "showcase",
        "--select",
        "Home",
        "--panel",
        panel_arg,
        "--size",
        "1440x900",
        "--scale",
        "2",
        "--screenshot",
        str(out_path),
    ]
    t0 = time.time()
    subprocess.run(cmd, cwd=str(REPO_ROOT), check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    dt = time.time() - t0
    if not out_path.exists() or out_path.stat().st_size < 10_000:
        raise RuntimeError(f"Failed or empty screenshot: {out_path}")
    size_kb = out_path.stat().st_size // 1024
    print(f"  [{idx:02d}/{total:02d}] {out_path.name} ({panel_arg}) -> {size_kb} KB in {dt:.2f}s", flush=True)
    return out_path.name, dt, size_kb


def main() -> int:
    if not BIN.exists():
        print(f"ERROR: Binary not found at {BIN}", file=sys.stderr)
        return 1
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tasks: list[tuple[int, int, str, str, int]] = []
    total = len(SUITES) * 6
    idx = 0
    for code, panel_base in SUITES:
        for step in range(1, 7):
            idx += 1
            tasks.append((idx, total, code, panel_base, step))

    print(f"==> Capturing {total} REAL Retina 2x screenshots from {BIN.name} (6 workers)...")
    t_start = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        futures = [pool.submit(capture_one, t) for t in tasks]
        for f in concurrent.futures.as_completed(futures):
            f.result()

    elapsed = time.time() - t_start
    print(f"==> Done: 48/48 real Studio screenshots captured in {elapsed:.1f}s -> {OUT_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
