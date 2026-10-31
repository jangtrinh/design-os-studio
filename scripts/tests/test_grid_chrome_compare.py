import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

QA_DIR = Path(__file__).resolve().parents[1] / "qa"
sys.path.insert(0, str(QA_DIR))

from grid_chrome import (  # noqa: E402
    build_batch_probe_html,
    build_screenshot_html,
    compare_boxes,
    find_chrome,
    parse_tailwind_html,
    run_comparison,
    tailwind_token_to_decls,
)


def _load_cli_module():
    spec = importlib.util.spec_from_file_location("grid_chrome_compare_cli", QA_DIR / "grid-chrome-compare.py")
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


class GridChromeCompareTests(unittest.TestCase):
    def test_compare_boxes_fails_when_item_is_missing_or_extra(self) -> None:
        solver = [
            {"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0},
            {"index": 1, "x": 100.0, "y": 0.0, "w": 40.0, "h": 30.0},
        ]
        chrome_missing = [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}]
        max_err, rows = compare_boxes(solver, chrome_missing)
        self.assertEqual(max_err, float("inf"))
        self.assertIn("item count mismatch", rows[0]["error"])

    def test_parse_tailwind_html_extracts_non_div_children_and_checks_count(self) -> None:
        html = (
            '<div class="grid grid-cols-2 box-border p-3 w-[200px] h-[100px]">\n'
            '  <div class="h-5"></div>\n'
            '  <img class="w-10 h-5" />\n'
            "</div>"
        )
        grid_css, item_css = parse_tailwind_html(html, expected_children=2)
        self.assertIn("box-sizing: border-box;", grid_css)
        self.assertEqual(len(item_css), 2)
        self.assertIn("width: 40px;", item_css[1])
        with self.assertRaises(ValueError):
            parse_tailwind_html(html, expected_children=3)

    def test_box_border_and_border_tokens_map_to_css_declarations(self) -> None:
        self.assertEqual(tailwind_token_to_decls("box-border"), ["box-sizing: border-box;"])
        self.assertEqual(tailwind_token_to_decls("border"), ["border-width: 1px;", "border-style: solid;"])
        self.assertEqual(tailwind_token_to_decls("border-2"), ["border-width: 2px;", "border-style: solid;"])
        self.assertEqual(tailwind_token_to_decls("border-[#333333]"), ["border-color: #333333;"])

    def test_probe_and_screenshot_pages_do_not_inject_global_border_box_reset(self) -> None:
        scene = {
            "name": "unequal_tracks",
            "grid_width": 200.0,
            "grid_height": 140.0,
            "grid_css": "display: grid;\nwidth: 200px;\nheight: 140px;\ngrid-template-columns: 100px 40px 60px;\ngap: 10px 0px;\nalign-content: start;",
            "tailwind_html": '<div class="grid grid-cols-[100px_40px_60px] content-start gap-y-2.5 w-[200px] h-[140px]"><div class="w-[100px] h-[30px]"></div></div>',
            "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0, "css": "width: 100px;\nheight: 30px;"}],
        }
        probe_html = build_batch_probe_html([scene])
        shot_html = build_screenshot_html(scene, max_abs_drift=0.0)
        self.assertNotIn("* { box-sizing: border-box", probe_html)
        self.assertNotIn("* { box-sizing: border-box", shot_html)
        self.assertIn("unequal_tracks", shot_html)

    def test_screenshot_subtitle_uses_real_tracks_and_real_drift_and_out_of_flow_labels(self) -> None:
        scene = {
            "name": "n1_explicit_hug_rows_flex_auto",
            "grid_width": 200.0,
            "grid_height": 200.0,
            "grid_css": (
                "display: grid;\nwidth: 200px;\nheight: 200px;\n"
                "grid-template-columns: repeat(2, minmax(0, 1fr));\n"
                "grid-template-rows: auto auto;\ngap: 10px;\nalign-content: start;"
            ),
            "tailwind_html": (
                '<div class="grid grid-cols-2 grid-rows-[auto_auto] content-start gap-2.5 w-[200px] h-[200px]">'
                '<div class="h-5"></div></div>'
            ),
            "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 95.0, "h": 20.0, "css": "height: 20px;"}],
        }
        shot_html = build_screenshot_html(scene, max_abs_drift=75.0)
        self.assertNotIn("100px 40px 60px", shot_html)
        self.assertNotIn("max_abs_drift: 0.00px", shot_html)
        self.assertIn("grid-template-columns: repeat(2, minmax(0, 1fr))", shot_html)
        self.assertIn("grid-template-rows: auto auto", shot_html)
        self.assertIn("max_abs_drift: 75.00px", shot_html)
        self.assertIn("position:absolute; inset:0;", shot_html)

    def test_chrome_bin_missing_does_not_fallback_and_cli_exits_2(self) -> None:
        cli = _load_cli_module()
        with patch.dict(os.environ, {"CHROME_BIN": "/nonexistent/chrome-binary-484"}, clear=False):
            self.assertIsNone(find_chrome())
            err = io.StringIO()
            with redirect_stderr(err):
                rc = cli.main([])
            self.assertEqual(rc, 2)
            self.assertIn("/nonexistent/chrome-binary-484", err.getvalue())

        err2 = io.StringIO()
        with redirect_stderr(err2):
            rc2 = cli.main(["--chrome", "/nonexistent/explicit-chrome-484"])
        self.assertEqual(rc2, 2)
        self.assertIn("/nonexistent/explicit-chrome-484", err2.getvalue())

    def test_tailwind_child_count_mismatch_reports_scene_fail_without_aborting_batch(self) -> None:
        bad_scene = {
            "name": "bad_tw_count",
            "grid_width": 200.0,
            "grid_height": 30.0,
            "grid_css": "display: grid;\nwidth: 200px;\nheight: 30px;\ngrid-template-columns: 100px 100px;",
            "tailwind_html": '<div class="grid grid-cols-[100px_100px] w-[200px] h-[30px]"><div class="w-[100px] h-[30px]"></div></div>',
            "items": [
                {"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0, "css": "width: 100px;\nheight: 30px;"},
                {"index": 1, "x": 100.0, "y": 0.0, "w": 100.0, "h": 30.0, "css": "width: 100px;\nheight: 30px;"},
            ],
        }
        good_scene = {
            "name": "good_after_bad",
            "grid_width": 100.0,
            "grid_height": 30.0,
            "grid_css": "display: grid;\nwidth: 100px;\nheight: 30px;\ngrid-template-columns: 100px;",
            "tailwind_html": '<div class="grid grid-cols-[100px] w-[100px] h-[30px]"><div class="w-[100px] h-[30px]"></div></div>',
            "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0, "css": "width: 100px;\nheight: 30px;"}],
        }
        fake_measured = {
            "bad_tw_count:css": {
                "grid_w": 200.0,
                "grid_h": 30.0,
                "items": [
                    {"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0},
                    {"index": 1, "x": 100.0, "y": 0.0, "w": 100.0, "h": 30.0},
                ],
            },
            "bad_tw_count:tailwind": {
                "grid_w": 200.0,
                "grid_h": 30.0,
                "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}],
            },
            "good_after_bad:css": {
                "grid_w": 100.0,
                "grid_h": 30.0,
                "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}],
            },
            "good_after_bad:tailwind": {
                "grid_w": 100.0,
                "grid_h": 30.0,
                "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}],
            },
        }
        with patch("grid_chrome.screenshot.measure_batch_in_chrome", return_value=fake_measured):
            rc, lines, rep, _ = run_comparison("fake-chrome", fixtures=[bad_scene, good_scene])
        self.assertEqual(rc, 1)
        joined = "\n".join(lines)
        self.assertIn("[FAIL] bad_tw_count", joined)
        self.assertIn("[PASS] good_after_bad", joined)
        self.assertEqual(rep["bad_tw_count"]["tailwind"]["status"], "FAIL")
        self.assertEqual(rep["good_after_bad"]["tailwind"]["status"], "PASS")

    def test_cli_supports_fixtures_json_flag(self) -> None:
        cli = _load_cli_module()
        scene = {
            "name": "single_scene",
            "grid_width": 100.0,
            "grid_height": 30.0,
            "grid_css": "display: grid;\nwidth: 100px;\nheight: 30px;\ngrid-template-columns: 100px;",
            "tailwind_html": '<div class="grid grid-cols-[100px] w-[100px] h-[30px]"><div class="w-[100px] h-[30px]"></div></div>',
            "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0, "css": "width: 100px;\nheight: 30px;"}],
        }
        fake_measured = {
            "single_scene:css": {
                "grid_w": 100.0,
                "grid_h": 30.0,
                "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}],
            },
            "single_scene:tailwind": {
                "grid_w": 100.0,
                "grid_h": 30.0,
                "items": [{"index": 0, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0}],
            },
        }
        with tempfile.TemporaryDirectory() as tmp:
            fjson = Path(tmp) / "fixtures.json"
            fjson.write_text(json.dumps([scene]), encoding="utf-8")
            with patch("grid_chrome.screenshot.measure_batch_in_chrome", return_value=fake_measured), patch(
                "os.path.isfile", return_value=True
            ), patch("os.access", return_value=True):
                out = io.StringIO()
                with redirect_stdout(out):
                    rc = cli.main(["--chrome", "/fake/chrome", "--fixtures-json", str(fjson)])
                self.assertEqual(rc, 0)
                self.assertIn("[PASS] single_scene", out.getvalue())


if __name__ == "__main__":
    unittest.main()
