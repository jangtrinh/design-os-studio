#!/usr/bin/env python3
"""Headless Chrome vs layout_solver::compute_grid geometry harness (Issue #484).

Exit codes:
- 0: All scenes and items match within +/-0.5px in both CSS and Tailwind modes.
- 1: One or more item boxes drifted beyond +/-0.5px, an item count mismatched, or Rust fixture dump failed.
- 2: Headless Chrome/Chromium binary is not installed or configured binary is missing.
"""

import argparse
import json
import os
from pathlib import Path
import sys
from typing import List, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))

from grid_chrome import find_chrome, render_comparison_png, run_comparison  # noqa: E402


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--chrome", type=str, default=None, help="Optional path or name of Chrome/Chromium binary")
    ap.add_argument("--fixtures-json", type=Path, default=None, help="Optional path to pre-dumped fixtures JSON")
    ap.add_argument("--screenshot", type=Path, default=None, help="Optional path to save comparison PNG")
    ap.add_argument("--screenshot-scene", type=str, default="unequal_tracks", help="Scene name for --screenshot")
    ap.add_argument("--json-out", type=Path, default=None, help="Optional path to write full JSON comparison report")
    args = ap.parse_args(argv)

    chrome_bin = find_chrome(args.chrome)
    if not chrome_bin:
        configured = args.chrome if args.chrome is not None else os.environ.get("CHROME_BIN")
        if configured:
            sys.stderr.write(f"SKIP: Configured Chrome/Chromium binary not found: {configured} (exit 2).\n")
        else:
            sys.stderr.write("SKIP: Headless Chrome/Chromium binary not found (exit 2).\n")
        return 2

    repo_root = Path(__file__).resolve().parents[2]
    fixtures = json.loads(args.fixtures_json.read_text(encoding="utf-8")) if args.fixtures_json else None
    rc, lines, report_scenes, fixtures = run_comparison(chrome_bin, fixtures=fixtures, repo_root=repo_root)
    for line in lines:
        print(line)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report_scenes, indent=2), encoding="utf-8")

    if args.screenshot:
        target = next((f for f in fixtures if f["name"] == args.screenshot_scene), fixtures[0])
        scene_rep = report_scenes.get(target["name"], {})
        drift = max(
            (
                max(float(m["max_item_err_px"]), float(m["grid_w_err_px"]), float(m["grid_h_err_px"]))
                for m in scene_rep.values()
            ),
            default=None,
        )
        render_comparison_png(chrome_bin, target, args.screenshot, max_abs_drift=drift)
        print(f"Saved screenshot: {args.screenshot}")

    return rc


if __name__ == "__main__":
    sys.exit(main())
