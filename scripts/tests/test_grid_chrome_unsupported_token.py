"""An untranslatable Tailwind token must fail only its own scene, never abort the run.

Run from the repo root: python3 scripts/tests/test_grid_chrome_unsupported_token.py
"""

from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "qa"))

from grid_chrome import probe_page, screenshot  # noqa: E402
from grid_chrome.tailwind_css_map import UnsupportedTailwindToken, tailwind_token_to_decls  # noqa: E402


def _scene(name: str, tailwind_html: str) -> dict:
    item = {"index": 0, "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0, "css": "width: 10px; height: 10px;"}
    return {
        "name": name,
        "grid_width": 10.0,
        "grid_height": 10.0,
        "grid_css": "display: grid; width: 10px; height: 10px;",
        "tailwind_html": tailwind_html,
        "items": [item],
    }


class UnsupportedTokenTest(unittest.TestCase):
    def test_unknown_token_raises_specific_error(self) -> None:
        with self.assertRaises(UnsupportedTailwindToken):
            tailwind_token_to_decls("backdrop-blur-[4px]")

    def test_scene_with_unknown_token_fails_without_aborting_others(self) -> None:
        good = _scene("good", '<div class="grid w-[10px] h-[10px]">\n  <div class="w-[10px] h-[10px]"></div>\n</div>')
        bad = _scene("bad", '<div class="grid backdrop-blur-[4px]">\n  <div class="w-[10px] h-[10px]"></div>\n</div>')
        box = {"grid_w": 10.0, "grid_h": 10.0, "items": [{"index": 0, "x": 0, "y": 0, "w": 10, "h": 10}]}
        measured = {"good:css": box, "good:tailwind": box, "bad:css": box}
        with mock.patch.object(screenshot, "measure_batch_in_chrome", return_value=measured):
            rc, lines, report, _ = screenshot.run_comparison("chrome", fixtures=[good, bad])
        self.assertEqual(rc, 1)
        self.assertTrue(any(l.startswith("[PASS] good") and "tailwind" in l for l in lines), lines)
        self.assertTrue(any(l.startswith("[FAIL] bad") and "backdrop-blur" in l for l in lines), lines)
        self.assertEqual(report["bad"]["tailwind"]["status"], "FAIL")
        self.assertEqual(report["bad"]["css"]["status"], "PASS")
        self.assertNotIn('data-scene="bad" data-mode="tailwind"', probe_page.build_batch_probe_html([good, bad]))


if __name__ == "__main__":
    unittest.main()
