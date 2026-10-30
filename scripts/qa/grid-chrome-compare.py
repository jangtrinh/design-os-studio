#!/usr/bin/env python3
"""Compare designos-export CSS and Tailwind grid layouts against the layout solver in headless Chrome."""

from __future__ import annotations

import argparse
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any

DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
TOLERANCE_PX = 0.5

# Tailwind default spacing scale (unit -> px, where 1 unit = 4px).
TAILWIND_SCALE_PX: dict[str, float] = {
    "0": 0.0,
    "px": 1.0,
    "0.5": 2.0,
    "1": 4.0,
    "1.5": 6.0,
    "2": 8.0,
    "2.5": 10.0,
    "3": 12.0,
    "3.5": 14.0,
    "4": 16.0,
    "5": 20.0,
    "6": 24.0,
    "7": 28.0,
    "8": 32.0,
    "9": 36.0,
    "10": 40.0,
    "11": 44.0,
    "12": 48.0,
    "14": 56.0,
    "16": 64.0,
    "20": 80.0,
    "24": 96.0,
    "28": 112.0,
    "32": 128.0,
    "36": 144.0,
    "40": 160.0,
    "44": 176.0,
    "48": 192.0,
    "52": 208.0,
    "56": 224.0,
    "60": 240.0,
    "64": 256.0,
    "72": 288.0,
    "80": 320.0,
    "96": 384.0,
}


def fmt_px(val: float) -> str:
    if abs(val - round(val)) < 1e-4:
        return f"{int(round(val))}px"
    return f"{val:.2f}".rstrip("0").rstrip(".") + "px"


def resolve_spacing_token(token: str) -> str:
    if token.startswith("[") and token.endswith("]"):
        return token[1:-1].replace("_", " ")
    if token == "px":
        return "1px"
    if token in TAILWIND_SCALE_PX:
        return fmt_px(TAILWIND_SCALE_PX[token])
    if re.fullmatch(r"\d+(\.\d+)?", token):
        return fmt_px(float(token) * 4.0)
    raise ValueError(f"Unsupported Tailwind spacing/size token: {token!r}")


def tailwind_class_to_css(cls: str) -> list[str]:
    if cls == "grid":
        return ["display: grid;"]
    if cls == "content-start":
        return ["align-content: start;"]
    if cls == "content-center":
        return ["align-content: center;"]
    if cls == "content-end":
        return ["align-content: end;"]
    if cls == "content-between":
        return ["align-content: space-between;"]
    if cls == "justify-self-start":
        return ["justify-self: start;"]
    if cls == "self-start":
        return ["align-self: start;"]
    if cls == "self-center":
        return ["align-self: center;"]
    if cls == "self-end":
        return ["align-self: end;"]
    if cls == "self-stretch":
        return ["align-self: stretch;"]

    m = re.fullmatch(r"grid-cols-(\d+)", cls)
    if m:
        return [f"grid-template-columns: repeat({m.group(1)}, minmax(0, 1fr));"]
    m = re.fullmatch(r"grid-cols-\[(.+)\]", cls)
    if m:
        tracks = m.group(1).replace(",", ", ").replace("_", " ")
        return [f"grid-template-columns: {tracks};"]

    m = re.fullmatch(r"grid-rows-(\d+)", cls)
    if m:
        return [f"grid-template-rows: repeat({m.group(1)}, minmax(0, 1fr));"]
    m = re.fullmatch(r"grid-rows-\[(.+)\]", cls)
    if m:
        tracks = m.group(1).replace(",", ", ").replace("_", " ")
        return [f"grid-template-rows: {tracks};"]

    if cls.startswith("gap-x-"):
        return [f"column-gap: {resolve_spacing_token(cls[len('gap-x-'):])};"]
    if cls.startswith("gap-y-"):
        return [f"row-gap: {resolve_spacing_token(cls[len('gap-y-'):])};"]
    if cls.startswith("gap-"):
        return [f"gap: {resolve_spacing_token(cls[len('gap-'):])};"]

    if cls.startswith("p-"):
        return [f"padding: {resolve_spacing_token(cls[2:])};"]
    if cls.startswith("px-"):
        v = resolve_spacing_token(cls[3:])
        return [f"padding-left: {v};", f"padding-right: {v};"]
    if cls.startswith("py-"):
        v = resolve_spacing_token(cls[3:])
        return [f"padding-top: {v};", f"padding-bottom: {v};"]
    if cls.startswith("pt-"):
        return [f"padding-top: {resolve_spacing_token(cls[3:])};"]
    if cls.startswith("pr-"):
        return [f"padding-right: {resolve_spacing_token(cls[3:])};"]
    if cls.startswith("pb-"):
        return [f"padding-bottom: {resolve_spacing_token(cls[3:])};"]
    if cls.startswith("pl-"):
        return [f"padding-left: {resolve_spacing_token(cls[3:])};"]

    if cls == "w-full":
        return ["width: 100%;"]
    if cls == "w-max":
        return ["width: max-content;"]
    if cls.startswith("w-"):
        return [f"width: {resolve_spacing_token(cls[2:])};"]

    if cls == "h-full":
        return ["height: 100%;"]
    if cls == "h-max":
        return ["height: max-content;"]
    if cls.startswith("h-"):
        return [f"height: {resolve_spacing_token(cls[2:])};"]

    m = re.fullmatch(r"col-start-(\d+)", cls) or re.fullmatch(r"col-start-\[(\d+)\]", cls)
    if m:
        return [f"grid-column-start: {m.group(1)};"]
    m = re.fullmatch(r"col-span-(\d+)", cls) or re.fullmatch(r"col-\[span_(\d+)_/_span_\d+\]", cls)
    if m:
        return [f"grid-column-end: span {m.group(1)};"]

    m = re.fullmatch(r"row-start-(\d+)", cls) or re.fullmatch(r"row-start-\[(\d+)\]", cls)
    if m:
        return [f"grid-row-start: {m.group(1)};"]
    m = re.fullmatch(r"row-span-(\d+)", cls) or re.fullmatch(r"row-\[span_(\d+)_/_span_\d+\]", cls)
    if m:
        return [f"grid-row-end: span {m.group(1)};"]

    raise ValueError(f"Unmapped Tailwind class in grid harness: {cls!r}")


def tailwind_classes_to_css(class_str: str) -> str:
    decls: list[str] = []
    for token in class_str.split():
        decls.extend(tailwind_class_to_css(token))
    return "\n".join(decls)


class TailwindDivExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.div_classes: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "div":
            cls = dict(attrs).get("class") or ""
            self.div_classes.append(cls)


def parse_tailwind_scene(tailwind_html: str) -> tuple[str, list[str]]:
    parser = TailwindDivExtractor()
    parser.feed(tailwind_html)
    if not parser.div_classes:
        raise ValueError(f"No <div> found in Tailwind markup:\n{tailwind_html}")
    grid_css = tailwind_classes_to_css(parser.div_classes[0])
    item_css = [tailwind_classes_to_css(c) for c in parser.div_classes[1:]]
    return grid_css, item_css


def build_batch_probe_html(cases: list[tuple[str, str, list[str]]]) -> str:
    rules: list[str] = []
    containers: list[str] = []
    for case_id, grid_css, item_css_list in cases:
        rules.append(f"#grid-{case_id} {{\n{grid_css}\n}}")
        item_divs = []
        for i, icss in enumerate(item_css_list):
            rules.append(f"#grid-{case_id}-item-{i} {{\n{icss}\n}}")
            item_divs.append(f'    <div class="item" id="grid-{case_id}-item-{i}"></div>')
        items_block = "\n".join(item_divs)
        containers.append(f'  <div class="probe-grid" data-case="{case_id}" id="grid-{case_id}">\n{items_block}\n  </div>')

    css_block = "\n".join(rules)
    html_containers = "\n".join(containers)
    return f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
html, body {{ margin: 0; padding: 0; }}
{css_block}
</style>
</head>
<body>
{html_containers}
  <pre id="rects"></pre>
  <script>
    (function () {{
      const out = {{}};
      for (const gridEl of document.querySelectorAll('.probe-grid')) {{
        const caseId = gridEl.getAttribute('data-case');
        const g = gridEl.getBoundingClientRect();
        out[caseId] = Array.from(gridEl.querySelectorAll(':scope > .item')).map((el, index) => {{
          const r = el.getBoundingClientRect();
          return {{
            index,
            x: r.left - g.left,
            y: r.top - g.top,
            w: r.width,
            h: r.height
          }};
        }});
      }}
      document.getElementById('rects').textContent = JSON.stringify(out);
    }})();
  </script>
</body>
</html>
"""


class PreRectsParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self._in_pre = False
        self.chunks: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "pre" and dict(attrs).get("id") == "rects":
            self._in_pre = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "pre" and self._in_pre:
            self._in_pre = False

    def handle_data(self, data: str) -> None:
        if self._in_pre:
            self.chunks.append(data)


def measure_batch_in_chrome(chrome_bin: str, html_doc: str) -> dict[str, list[dict[str, float]]]:
    with tempfile.TemporaryDirectory(prefix="grid-chrome-") as tmpdir:
        html_path = Path(tmpdir) / "scene.html"
        html_path.write_text(html_doc, encoding="utf-8")
        proc = subprocess.run(
            [
                chrome_bin,
                "--headless=new",
                "--disable-gpu",
                "--dump-dom",
                str(html_path),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        parser = PreRectsParser()
        parser.feed(proc.stdout)
        raw = "".join(parser.chunks).strip()
        if not raw:
            raise RuntimeError(
                f"Missing <pre id=\"rects\"> payload in Chrome DOM dump (rc={proc.returncode}):\n{proc.stderr}\n{proc.stdout}"
            )
        return json.loads(raw)


def render_unequal_tracks_screenshot(chrome_bin: str, scene: dict[str, Any], out_path: Path) -> None:
    grid_w = float(scene["grid_width"])
    grid_h = float(scene["grid_height"])
    grid_css = scene["grid_css"]
    items = scene["items"]

    palette = ["#2563eb", "#0891b2", "#0d9488", "#4f46e5", "#0284c7", "#059669"]
    item_rules = []
    chrome_divs = []
    solver_boxes = []
    overlay_boxes = []

    for i, item in enumerate(items):
        color = palette[i % len(palette)]
        icss = item["css"]
        x, y, w, h = item["x"], item["y"], item["w"], item["h"]
        label = f"#{i} {int(round(w))}×{int(round(h))}" if w >= 55 else f"{int(round(w))}×{int(round(h))}"
        item_rules.append(
            f"#chrome-item-{i} {{\n{icss}\nbackground: {color}26;\nborder: 1.5px solid {color};\n"
            "display: flex; align-items: center; justify-content: center;\n"
            "font: 600 10px/1 ui-monospace, SFMono-Regular, Menlo, monospace; color: #0f172a;\n}"
        )
        chrome_divs.append(f'        <div id="chrome-item-{i}">{label}</div>')
        solver_boxes.append(
            f'        <div class="solver-box" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;">'
            f"<span>{label}</span></div>"
        )
        overlay_boxes.append(
            f'        <div class="solver-outline" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;"></div>'
        )

    html_doc = f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: #f8fafc;
  color: #0f172a;
  padding: 20px;
}}
h1 {{
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 14px;
  color: #1e293b;
}}
.panels {{
  display: flex;
  gap: 24px;
  align-items: flex-start;
}}
.panel {{
  background: #ffffff;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  padding: 14px;
}}
.panel-title {{
  font-size: 12px;
  font-weight: 600;
  color: #475569;
  margin-bottom: 10px;
}}
#chrome-grid {{
{grid_css}
  position: relative;
  background: #f1f5f9;
  outline: 1px dashed #94a3b8;
}}
.solver-stage {{
  position: relative;
  width: {grid_w}px;
  height: {grid_h}px;
  background: #f1f5f9;
  outline: 1px dashed #94a3b8;
}}
.solver-box {{
  position: absolute;
  border: 2px dashed #dc2626;
  background: rgba(220, 38, 38, 0.06);
  display: flex;
  align-items: center;
  justify-content: center;
  font: 600 10px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  color: #991b1b;
}}
.solver-outline {{
  position: absolute;
  border: 2px dashed #dc2626;
  pointer-events: none;
}}
{chr(10).join(item_rules)}
</style>
</head>
<body>
  <h1>Issue #484 — unequal_tracks (100px / 40px / 60px): Chrome CSS Grid vs Layout Solver Boxes</h1>
  <div class="panels">
    <div class="panel">
      <div class="panel-title">1. Chrome Render + Solver Outlines (dashed red)</div>
      <div id="chrome-grid">
{chr(10).join(chrome_divs)}
{chr(10).join(overlay_boxes)}
      </div>
    </div>
    <div class="panel">
      <div class="panel-title">2. Solver Boxes Only (dashed red outlines)</div>
      <div class="solver-stage">
{chr(10).join(solver_boxes)}
      </div>
    </div>
  </div>
</body>
</html>
"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="grid-shot-") as tmpdir:
        html_path = Path(tmpdir) / "screenshot.html"
        html_path.write_text(html_doc, encoding="utf-8")
        proc = subprocess.run(
            [
                chrome_bin,
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--window-size=560,310",
                f"--screenshot={out_path.resolve()}",
                str(html_path),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if not out_path.is_file():
            raise RuntimeError(f"Failed to capture Chrome screenshot ({proc.returncode}):\n{proc.stderr}")


def load_scenes(repo_root: Path, fixtures_json: str | None) -> list[dict[str, Any]]:
    if fixtures_json:
        return json.loads(Path(fixtures_json).read_text(encoding="utf-8"))
    env = os.environ.copy()
    cargo_bin_dir = str(Path.home() / ".cargo" / "bin")
    if cargo_bin_dir not in env.get("PATH", ""):
        env["PATH"] = f"{cargo_bin_dir}:{env.get('PATH', '')}"
    cargo_exe = str(Path.home() / ".cargo" / "bin" / "cargo")
    if not os.path.isfile(cargo_exe):
        cargo_exe = "cargo"
    env.setdefault("CARGO_TARGET_DIR", str(repo_root / "out" / "cargo"))
    env["GRID_CHROME_DUMP_JSON"] = "1"
    proc = subprocess.run(
        [
            cargo_exe,
            "test",
            "--quiet",
            "--locked",
            "-p",
            "designos-export",
            "--test",
            "grid_chrome_fixtures",
            "equal_tracks_css_and_tailwind",
            "--",
            "--exact",
            "--nocapture",
        ],
        cwd=str(repo_root),
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"cargo test --test grid_chrome_fixtures failed ({proc.returncode}):\n{proc.stderr}")
    begin = "===GRID_CHROME_JSON_BEGIN==="
    end = "===GRID_CHROME_JSON_END==="
    out = proc.stdout
    start_idx = out.find(begin)
    end_idx = out.find(end)
    if start_idx == -1 or end_idx == -1 or end_idx <= start_idx:
        raise RuntimeError(f"Failed to locate JSON markers in test stdout:\n{out}")
    payload = out[start_idx + len(begin) : end_idx].strip()
    return json.loads(payload)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--chrome", default=os.environ.get("CHROME_BIN", DEFAULT_CHROME), help="Path to Google Chrome binary")
    ap.add_argument("--fixtures-json", default=None, help="Optional pre-generated JSON from grid_chrome_fixtures")
    ap.add_argument("--screenshot", default=None, help="Write PNG screenshot of unequal_tracks Chrome vs solver boxes")
    args = ap.parse_args()

    chrome_bin = args.chrome
    if not (os.path.isfile(chrome_bin) and os.access(chrome_bin, os.X_OK)):
        print(f"ERROR: Google Chrome binary not found or not executable at {chrome_bin!r}", file=sys.stderr)
        return 2

    repo_root = Path(__file__).resolve().parents[2]
    scenes = load_scenes(repo_root, args.fixtures_json)

    batch_cases: list[tuple[str, str, list[str]]] = []
    for scene in scenes:
        name = scene["name"]
        solver_items = scene["items"]
        css_grid_decl = scene["grid_css"]
        css_item_decls = [it["css"] for it in solver_items]
        tw_grid_decl, tw_item_decls = parse_tailwind_scene(scene["tailwind_html"])
        batch_cases.append((f"{name}__CSS", css_grid_decl, css_item_decls))
        batch_cases.append((f"{name}__Tailwind", tw_grid_decl, tw_item_decls))

    html_doc = build_batch_probe_html(batch_cases)
    measured = measure_batch_in_chrome(chrome_bin, html_doc)

    any_failed = False
    for scene in scenes:
        name = scene["name"]
        solver_items = scene["items"]
        for mode in ("CSS", "Tailwind"):
            case_id = f"{name}__{mode}"
            chrome_items = measured[case_id]
            print(f"=== Scene: {name} [{mode}] ===")
            print(f"{'item':<6} {'solver (x, y, w, h)':<28} {'chrome (x, y, w, h)':<28} {'delta (dx, dy, dw, dh)':<28} {'max_diff':<8} status")
            scene_failed = False
            for s_it, c_it in zip(solver_items, chrome_items):
                dx = c_it["x"] - s_it["x"]
                dy = c_it["y"] - s_it["y"]
                dw = c_it["w"] - s_it["w"]
                dh = c_it["h"] - s_it["h"]
                max_d = max(abs(dx), abs(dy), abs(dw), abs(dh))
                ok = max_d <= TOLERANCE_PX
                if not ok:
                    scene_failed = True
                    any_failed = True
                s_str = f"({s_it['x']:.1f}, {s_it['y']:.1f}, {s_it['w']:.1f}, {s_it['h']:.1f})"
                c_str = f"({c_it['x']:.2f}, {c_it['y']:.2f}, {c_it['w']:.2f}, {c_it['h']:.2f})"
                d_str = f"({dx:+.2f}, {dy:+.2f}, {dw:+.2f}, {dh:+.2f})"
                status = "PASS" if ok else "FAIL"
                print(f"#{s_it['index']:<5} {s_str:<28} {c_str:<28} {d_str:<28} {max_d:<8.2f} {status}")
            print(f"Result [{name} / {mode}]: {'FAIL' if scene_failed else 'PASS'}\n")

    if args.screenshot:
        unequal = next((s for s in scenes if s["name"] == "unequal_tracks"), None)
        if unequal is None:
            raise RuntimeError("unequal_tracks scene not found for --screenshot")
        shot_path = Path(args.screenshot)
        render_unequal_tracks_screenshot(chrome_bin, unequal, shot_path)
        print(f"Wrote screenshot: {shot_path}")

    return 1 if any_failed else 0


if __name__ == "__main__":
    sys.exit(main())
