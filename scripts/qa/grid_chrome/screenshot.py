"""Side-by-side screenshot builder and comparison runner for grid Chrome harness."""

from pathlib import Path
import subprocess
import tempfile
from typing import Any, Dict, List, Optional, Tuple

from .fixtures import inline_css, load_fixtures_from_rust
from .probe_page import (
    MAX_DRIFT_PX,
    build_batch_probe_html,
    compare_boxes,
    measure_batch_in_chrome,
    scene_tailwind_errors,
)
from .tailwind_css_map import parse_tailwind_html


def _extract_track_summary(grid_css: str) -> str:
    parts = [
        raw.strip().rstrip(";")
        for raw in grid_css.splitlines()
        if raw.strip().startswith(("grid-template-columns:", "grid-template-rows:"))
    ]
    return " | ".join(parts) if parts else "grid-template-columns: (none)"


def _label_span(it: Dict[str, Any], idx: int) -> str:
    label = f'#{idx} ({int(it["w"])}x{int(it["h"])} @ {int(it["x"])},{int(it["y"])})'
    return (
        '<span style="position:absolute; inset:0; display:flex; align-items:center; '
        "justify-content:center; white-space:nowrap; pointer-events:none; "
        f'font:600 11px/1 -apple-system,sans-serif; color:#e2e8f0;">{label}</span>'
    )


def build_screenshot_html(scene: Dict[str, Any], max_abs_drift: Optional[float] = None) -> str:
    scene_name, gw, gh = scene["name"], scene["grid_width"], scene["grid_height"]
    palette = ["#38bdf8", "#a78bfa", "#34d399", "#fbbf24", "#f87171", "#f472b6"]
    css_grid_style = inline_css(scene["grid_css"])
    css_items, solver_items, tw_items = [], [], []
    for idx, it in enumerate(scene["items"]):
        color = palette[idx % len(palette)]
        span = _label_span(it, idx)
        box_fx = f"overflow:hidden; background:{color}33; box-shadow: inset 0 0 0 2px {color};"
        css_items.append(f'<div style="{inline_css(it["css"])} position:relative; {box_fx}">{span}</div>')
        solver_items.append(
            f'<div style="position:absolute; left:{it["x"]}px; top:{it["y"]}px; '
            f'width:{it["w"]}px; height:{it["h"]}px; {box_fx}">{span}</div>'
        )
    if scene_tailwind_errors([scene]):
        tw_grid_decls, tw_item_decls = "", []
    else:
        tw_grid_decls, tw_item_decls = parse_tailwind_html(scene["tailwind_html"])
    for idx, decl in enumerate(tw_item_decls):
        color = palette[idx % len(palette)]
        it = scene["items"][idx] if idx < len(scene["items"]) else {"w": 0, "h": 0, "x": 0, "y": 0}
        box_fx = f"overflow:hidden; background:{color}33; box-shadow: inset 0 0 0 2px {color};"
        tw_items.append(f'<div style="{decl} position:relative; {box_fx}">{_label_span(it, idx)}</div>')

    tracks_summary = _extract_track_summary(scene["grid_css"])
    drift_val = max_abs_drift if max_abs_drift is not None else scene.get("max_abs_drift")
    drift_text = f"{float(drift_val):.2f}px" if drift_val is not None else "unmeasured"

    return f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body {{ margin: 0; padding: 0; background: #0f172a; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, sans-serif; }}
  .wrap {{ padding: 28px 32px; display: flex; flex-direction: column; gap: 20px; }}
  h1 {{ margin: 0; font-size: 16px; font-weight: 600; color: #f1f5f9; }}
  .sub {{ margin: 4px 0 0 0; font-size: 12px; color: #94a3b8; font-family: ui-monospace, SFMono-Regular, monospace; }}
  .cols {{ display: flex; gap: 28px; align-items: flex-start; }}
  .card {{ background: #1e293b; border: 1px solid #334155; border-radius: 10px; padding: 16px; }}
  .card h2 {{ margin: 0 0 12px 0; font-size: 13px; font-weight: 600; color: #cbd5e1; }}
  .frame-shell {{ outline: 1px dashed #64748b; background: #0b1120; position: relative; }}
</style>
</head>
<body>
<div class="wrap">
  <div>
    <h1>Issue #484 — {scene_name}: Chrome CSS Grid vs Tailwind Grid vs layout_solver::compute_grid</h1>
    <p class="sub">{tracks_summary} | frame: {int(gw)}x{int(gh)}px | max_abs_drift: {drift_text}</p>
  </div>
  <div class="cols">
    <div class="card">
      <h2>1. Chrome CSS Grid (css_for)</h2>
      <div class="frame-shell" style="{css_grid_style}">{"".join(css_items)}</div>
    </div>
    <div class="card">
      <h2>2. Chrome Tailwind Grid (tailwind_for)</h2>
      <div class="frame-shell" style="{tw_grid_decls}">{"".join(tw_items)}</div>
    </div>
    <div class="card">
      <h2>3. layout_solver::compute_grid reference</h2>
      <div class="frame-shell" style="width:{gw}px; height:{gh}px;">{"".join(solver_items)}</div>
    </div>
  </div>
</div>
</body>
</html>
"""


def render_comparison_png(
    chrome_bin: str, scene: Dict[str, Any], out_png: Path, max_abs_drift: Optional[float] = None
) -> None:
    if max_abs_drift is None:
        _, _, rep, _ = run_comparison(chrome_bin, fixtures=[scene])
        scene_rep = rep.get(scene["name"], {})
        max_abs_drift = max(
            (
                max(float(m["max_item_err_px"]), float(m["grid_w_err_px"]), float(m["grid_h_err_px"]))
                for m in scene_rep.values()
            ),
            default=0.0,
        )
    out_png.parent.mkdir(parents=True, exist_ok=True)
    html_doc = build_screenshot_html(scene, max_abs_drift=max_abs_drift)
    win_w = max(960, int(float(scene["grid_width"])) * 3 + 220)
    win_h = max(360, int(float(scene["grid_height"])) + 180)
    with tempfile.TemporaryDirectory(prefix="grid-shot-") as tmpdir:
        html_path = Path(tmpdir) / "shot.html"
        profile_dir = Path(tmpdir) / "profile"
        profile_dir.mkdir(parents=True, exist_ok=True)
        html_path.write_text(html_doc, encoding="utf-8")
        cmd = [
            chrome_bin,
            "--headless",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--hide-scrollbars",
            f"--user-data-dir={profile_dir}",
            f"--window-size={win_w},{win_h}",
            f"--screenshot={out_png}",
            html_path.as_uri(),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=30)
        if proc.returncode != 0 or not out_png.is_file():
            raise RuntimeError(f"Chrome screenshot failed (exit {proc.returncode}):\n{proc.stdout}\n{proc.stderr}")


def run_comparison(
    chrome_bin: str,
    fixtures: Optional[List[Dict[str, Any]]] = None,
    repo_root: Optional[Path] = None,
) -> Tuple[int, List[str], Dict[str, Any], List[Dict[str, Any]]]:
    if fixtures is None:
        if repo_root is None:
            repo_root = Path(__file__).resolve().parents[2]
        fixtures = load_fixtures_from_rust(repo_root)
    probe_html = build_batch_probe_html(fixtures)
    measured = measure_batch_in_chrome(chrome_bin, probe_html)

    tailwind_errors = scene_tailwind_errors(fixtures)
    failed = False
    lines: List[str] = []
    report_scenes: Dict[str, Any] = {}
    for scene in fixtures:
        name = scene["name"]
        report_scenes[name] = {}
        for mode in ("css", "tailwind"):
            key = f"{name}:{mode}"
            if mode == "tailwind" and name in tailwind_errors:
                failed = True
                lines.append(f"[FAIL] {name:30s} ({mode:8s}) {tailwind_errors[name]}")
                report_scenes[name][mode] = {
                    "status": "FAIL",
                    "error": tailwind_errors[name],
                    "max_item_err_px": float("inf"),
                    "grid_w_err_px": float("inf"),
                    "grid_h_err_px": float("inf"),
                    "items": [],
                }
                continue
            if key not in measured:
                failed = True
                lines.append(f"[FAIL] {key}: missing measurement in Chrome output")
                continue
            chrome_box = measured[key]
            max_err, rows = compare_boxes(scene["items"], chrome_box["items"])
            gw_err = abs(float(chrome_box["grid_w"]) - float(scene["grid_width"]))
            gh_err = abs(float(chrome_box["grid_h"]) - float(scene["grid_height"]))
            overall_err = max(max_err, gw_err, gh_err)
            accepted = float(scene.get("accepted_drift", 0.0))
            # An accepted drift is a measured expectation: the scene fails when it drifts less as well as more.
            ok = abs(overall_err - accepted) <= MAX_DRIFT_PX if accepted > 0.0 else overall_err <= MAX_DRIFT_PX
            if not ok:
                failed = True
            status = "PASS" if ok else "FAIL"
            note = f" accepted_drift={accepted:g}px (a visible stroke wins over exact geometry)" if accepted > 0.0 else ""
            lines.append(
                f"[{status}] {name:30s} ({mode:8s}) max_item_err={max_err:.2f}px "
                f"grid_err=({gw_err:.2f}px, {gh_err:.2f}px){note}"
            )
            if not ok:
                for r in rows:
                    if "error" in r:
                        lines.append(f"       {r['error']}")
                    elif r["max_abs"] > MAX_DRIFT_PX:
                        s, c = r["solver"], r["chrome"]
                        lines.append(
                            f"       item[{r['index']}] solver=({s['x']:.1f},{s['y']:.1f},{s['w']:.1f},{s['h']:.1f}) "
                            f"chrome=({c['x']:.1f},{c['y']:.1f},{c['w']:.1f},{c['h']:.1f}) max_abs={r['max_abs']:.2f}px"
                        )
            report_scenes[name][mode] = {
                "status": status,
                "max_item_err_px": max_err,
                "grid_w_err_px": gw_err,
                "grid_h_err_px": gh_err,
                "items": rows,
            }
    for scene in fixtures:
        if scene.get("stroke_probe"):
            for mode, ok, detail in stroke_pixel_probe(chrome_bin, scene, tailwind_errors):
                failed = failed or not ok
                lines.append(f"[{'PASS' if ok else 'FAIL'}] {scene['name']:30s} ({mode:8s}) stroke pixels {detail}")
    return (1 if failed else 0), lines, report_scenes, fixtures


PIXEL_TOLERANCE = 3


def _load_pillow() -> Any:
    """Pillow's Image module, or None when it is not installed."""
    try:
        from PIL import Image
    except ImportError:
        return None
    return Image


def stroke_pixel_probe(
    chrome_bin: str, scene: Dict[str, Any], tailwind_errors: Dict[str, str]
) -> List[Tuple[str, bool, str]]:
    """Renders the exported grid in Chrome with every child filled blue and compares the colour at the middle
    of each edge with what Studio's renderer shows for the same scene (`studio_edges`: a child colour where a
    child covers the stroke, the stroke colour where not). Fails closed: no Pillow, or no Studio colours for the
    scene, is a FAIL."""
    Image = _load_pillow()
    if Image is None:
        return [("css", False, "FAILED (Pillow is required for the stroke pixel check: pip install pillow)")]
    studio = scene.get("studio_edges")
    if not studio:
        return [("css", False, "FAILED (the scene has no Studio edge colours to compare with)")]
    gw, gh = int(float(scene["grid_width"])), int(float(scene["grid_height"]))
    edges = {"top": (gw // 2, 0), "bottom": (gw // 2, gh - 1), "left": (0, gh // 2), "right": (gw - 1, gh // 2)}
    results: List[Tuple[str, bool, str]] = []
    modes = [("css", inline_css(scene["grid_css"]), [inline_css(it["css"]) for it in scene["items"]])]
    if scene["name"] not in tailwind_errors:
        tw_grid, tw_items = parse_tailwind_html(scene["tailwind_html"])
        modes.append(("tailwind", tw_grid, tw_items))
    for mode, grid_style, item_styles in modes:
        kids = "".join(f'<div style="{st} background:#0000ff;"></div>' for st in item_styles)
        page = f'<!DOCTYPE html><html><body style="margin:0;background:#fff"><div style="{grid_style}">{kids}</div></body></html>'
        with tempfile.TemporaryDirectory(prefix="grid-pixels-") as tmp:
            html_path, png = Path(tmp) / "p.html", Path(tmp) / "p.png"
            html_path.write_text(page, encoding="utf-8")
            (Path(tmp) / "profile").mkdir()
            subprocess.run(
                [chrome_bin, "--headless", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                 f"--user-data-dir={Path(tmp) / 'profile'}", f"--window-size={gw + 40},{gh + 40}",
                 f"--screenshot={png}", html_path.as_uri()],
                capture_output=True, text=True, check=False, timeout=30,
            )
            img = Image.open(png).convert("RGB")
            bad = []
            for edge, xy in edges.items():
                got, want = img.getpixel(xy), tuple(studio[edge])
                if max(abs(a - b) for a, b in zip(got, want)) > PIXEL_TOLERANCE:
                    bad.append(f"{edge} chrome={got} studio={want}")
        results.append((mode, not bad, "match Studio on all four edges" if not bad else "differ from Studio: " + "; ".join(bad)))
    return results
