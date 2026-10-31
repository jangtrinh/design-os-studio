"""Headless Chrome batch probe page builder and box comparator for grid scenes."""

from html.parser import HTMLParser
import json
from pathlib import Path
import subprocess
import tempfile
from typing import Any, Dict, List, Optional, Tuple

from .fixtures import inline_css
from .tailwind_css_map import parse_tailwind_html

MAX_DRIFT_PX = 0.5


def build_batch_probe_html(fixtures: List[Dict[str, Any]]) -> str:
    blocks: List[str] = []
    for scene in fixtures:
        name = scene["name"]
        solver_items = scene["items"]
        css_grid_style = inline_css(scene["grid_css"])
        css_children = "\n".join(
            f'      <div class="probe-item" data-idx="{it["index"]}" style="{inline_css(it["css"])}"></div>'
            for it in solver_items
        )
        blocks.append(
            f'  <div class="probe-grid" data-scene="{name}" data-mode="css" style="{css_grid_style}">\n'
            f"{css_children}\n  </div>"
        )
        tw_grid_decls, tw_item_decls = parse_tailwind_html(scene["tailwind_html"])
        tw_children = "\n".join(
            f'      <div class="probe-item" data-idx="{idx}" style="{decl}"></div>'
            for idx, decl in enumerate(tw_item_decls)
        )
        blocks.append(
            f'  <div class="probe-grid" data-scene="{name}" data-mode="tailwind" style="{tw_grid_decls}">\n'
            f"{tw_children}\n  </div>"
        )

    body_grids = "\n".join(blocks)
    return f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body {{ margin: 0; padding: 0; }}
  .probe-stage {{ position: relative; padding: 24px; }}
</style>
</head>
<body>
<div class="probe-stage">
{body_grids}
</div>
<pre id="probe-results"></pre>
<script>
(() => {{
  const out = {{}};
  for (const grid of document.querySelectorAll('.probe-grid')) {{
    const key = grid.dataset.scene + ':' + grid.dataset.mode;
    const gRect = grid.getBoundingClientRect();
    const items = [];
    for (const child of grid.querySelectorAll(':scope > .probe-item')) {{
      const r = child.getBoundingClientRect();
      items.push({{
        index: Number(child.dataset.idx),
        x: r.left - gRect.left,
        y: r.top - gRect.top,
        w: r.width,
        h: r.height,
      }});
    }}
    out[key] = {{ grid_w: gRect.width, grid_h: gRect.height, items }};
  }}
  document.getElementById('probe-results').textContent = JSON.stringify(out);
}})();
</script>
</body>
</html>
"""


class _PreProbeExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self._in_pre = False
        self.chunks: List[str] = []

    def handle_starttag(self, tag: str, attrs: List[Tuple[str, Optional[str]]]) -> None:
        if tag == "pre" and dict(attrs).get("id") == "probe-results":
            self._in_pre = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "pre" and self._in_pre:
            self._in_pre = False

    def handle_data(self, data: str) -> None:
        if self._in_pre:
            self.chunks.append(data)


def measure_batch_in_chrome(chrome_bin: str, html_doc: str) -> Dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="grid-chrome-") as tmpdir:
        html_path = Path(tmpdir) / "probe.html"
        profile_dir = Path(tmpdir) / "profile"
        profile_dir.mkdir(parents=True, exist_ok=True)
        html_path.write_text(html_doc, encoding="utf-8")
        cmd = [
            chrome_bin,
            "--headless",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            f"--user-data-dir={profile_dir}",
            "--window-size=1400,2400",
            "--dump-dom",
            html_path.as_uri(),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=30)
        if proc.returncode != 0:
            raise RuntimeError(
                f"Chrome --dump-dom failed (exit {proc.returncode}):\nstdout={proc.stdout}\nstderr={proc.stderr}"
            )
        extractor = _PreProbeExtractor()
        extractor.feed(proc.stdout)
        raw = "".join(extractor.chunks).strip()
        if not raw:
            raise RuntimeError(
                f"Chrome --dump-dom did not populate #probe-results:\nstdout={proc.stdout}\nstderr={proc.stderr}"
            )
        return json.loads(raw)


def compare_boxes(
    solver_items: List[Dict[str, Any]], chrome_items: List[Dict[str, Any]]
) -> Tuple[float, List[Dict[str, Any]]]:
    if len(chrome_items) != len(solver_items):
        missing = abs(len(solver_items) - len(chrome_items))
        return float("inf"), [
            {
                "index": -1,
                "solver": {"count": len(solver_items)},
                "chrome": {"count": len(chrome_items)},
                "dx": float("inf"),
                "dy": float("inf"),
                "dw": float("inf"),
                "dh": float("inf"),
                "max_abs": float("inf"),
                "error": f"item count mismatch: solver={len(solver_items)} vs chrome={len(chrome_items)} (diff={missing})",
            }
        ]
    max_err = 0.0
    rows: List[Dict[str, Any]] = []
    for s_it, c_it in zip(solver_items, chrome_items):
        dx = abs(float(c_it["x"]) - float(s_it["x"]))
        dy = abs(float(c_it["y"]) - float(s_it["y"]))
        dw = abs(float(c_it["w"]) - float(s_it["w"]))
        dh = abs(float(c_it["h"]) - float(s_it["h"]))
        item_err = max(dx, dy, dw, dh)
        max_err = max(max_err, item_err)
        rows.append(
            {
                "index": s_it["index"],
                "solver": {"x": s_it["x"], "y": s_it["y"], "w": s_it["w"], "h": s_it["h"]},
                "chrome": {"x": c_it["x"], "y": c_it["y"], "w": c_it["w"], "h": c_it["h"]},
                "dx": dx,
                "dy": dy,
                "dw": dw,
                "dh": dh,
                "max_abs": item_err,
            }
        )
    return max_err, rows
