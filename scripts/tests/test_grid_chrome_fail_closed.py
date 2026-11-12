"""The harness fails closed: a drift that is expected must happen, and a pixel check needs Pillow.

Run from the repo root: python3 scripts/tests/test_grid_chrome_fail_closed.py
"""

from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "qa"))

from grid_chrome import screenshot  # noqa: E402


def _scene(**extra) -> dict:
    item = {"index": 0, "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0, "css": "width: 10px; height: 10px;"}
    scene = {
        "name": "s",
        "grid_width": 10.0,
        "grid_height": 10.0,
        "grid_css": "display: grid; width: 10px; height: 10px;",
        "tailwind_html": '<div class="grid w-[10px] h-[10px]">\n  <div class="w-[10px] h-[10px]"></div>\n</div>',
        "items": [item],
    }
    scene.update(extra)
    return scene


def _run(scene: dict, measured_y: float):
    box = {"grid_w": 10.0, "grid_h": 10.0, "items": [{"index": 0, "x": 0, "y": measured_y, "w": 10, "h": 10}]}
    with mock.patch.object(screenshot, "measure_batch_in_chrome", return_value={"s:css": box, "s:tailwind": box}):
        return screenshot.run_comparison("chrome", fixtures=[scene])


class AcceptedDriftTest(unittest.TestCase):
    def test_expected_drift_that_does_not_happen_fails(self) -> None:
        rc, lines, _, _ = _run(_scene(accepted_drift=4.0), measured_y=0.0)
        self.assertEqual(rc, 1)
        self.assertTrue(any(l.startswith("[FAIL] s") for l in lines), lines)

    def test_expected_drift_that_happens_passes(self) -> None:
        rc, _, _, _ = _run(_scene(accepted_drift=4.0), measured_y=4.0)
        self.assertEqual(rc, 0)

    def test_drift_that_is_not_expected_fails(self) -> None:
        rc, _, _, _ = _run(_scene(), measured_y=4.0)
        self.assertEqual(rc, 1)

    def test_a_different_drift_than_expected_fails(self) -> None:
        rc, _, _, _ = _run(_scene(accepted_drift=4.0), measured_y=2.0)
        self.assertEqual(rc, 1)


class PixelProbeFailsClosedTest(unittest.TestCase):
    def test_missing_pillow_is_a_failure(self) -> None:
        with mock.patch.object(screenshot, "_load_pillow", return_value=None):
            results = screenshot.stroke_pixel_probe("chrome", _scene(studio_edges={"top": [0, 0, 0]}), {})
        self.assertTrue(results and all(not ok for _, ok, _ in results), results)
        self.assertIn("Pillow", results[0][2])

    def test_missing_pillow_fails_the_whole_run(self) -> None:
        box = {"grid_w": 10.0, "grid_h": 10.0, "items": [{"index": 0, "x": 0, "y": 0, "w": 10, "h": 10}]}
        scene = _scene(stroke_probe=True, studio_edges={"top": [0, 0, 0]})
        with mock.patch.object(screenshot, "measure_batch_in_chrome", return_value={"s:css": box, "s:tailwind": box}):
            with mock.patch.object(screenshot, "_load_pillow", return_value=None):
                rc, lines, _, _ = screenshot.run_comparison("chrome", fixtures=[scene])
        self.assertEqual(rc, 1)
        self.assertTrue(any("stroke pixels FAILED" in l for l in lines), lines)

    def test_a_scene_without_studio_colours_is_a_failure(self) -> None:
        with mock.patch.object(screenshot, "_load_pillow", return_value=object()):
            results = screenshot.stroke_pixel_probe("chrome", _scene(), {})
        self.assertEqual([ok for _, ok, _ in results], [False])


if __name__ == "__main__":
    unittest.main()
