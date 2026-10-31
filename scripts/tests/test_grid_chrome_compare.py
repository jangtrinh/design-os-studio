import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "qa"))

from grid_chrome import (  # noqa: E402
    build_batch_probe_html,
    build_screenshot_html,
    compare_boxes,
    parse_tailwind_html,
    tailwind_token_to_decls,
)


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

    def test_box_border_token_maps_to_border_box(self) -> None:
        self.assertEqual(tailwind_token_to_decls("box-border"), ["box-sizing: border-box;"])

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
        shot_html = build_screenshot_html(scene)
        self.assertNotIn("* { box-sizing: border-box", probe_html)
        self.assertNotIn("* { box-sizing: border-box", shot_html)
        self.assertIn("unequal_tracks", shot_html)


if __name__ == "__main__":
    unittest.main()
