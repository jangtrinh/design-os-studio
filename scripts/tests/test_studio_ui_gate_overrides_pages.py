"""G2 overrides gate on page `03 Studio`, and on corner smoothing.

Composites live on 03 Studio and nest the sets of 02 Core, so the gate must walk both pages: a style override on a page-03 instance fails, and the instances there are
counted. `corner_smoothing` is part of the CornerRadii field (`record_override`, designos-doc/src/components.rs:465): a copy that smooths its corners differently from its main
fails, except on a Focus/Ring, whose corners the ring gate owns.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_overrides_pages
Synthetic documents only (studio_ui_override_fixtures).
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BOX, IBOX, ITEXT, base, on_page_03, ring_doc  # noqa: E402


class Page03(unittest.TestCase):
    def test_a_forbidden_override_on_a_page_03_instance_fails_with_its_path(self):
        d = on_page_03(base())
        d["nodes"][str(IBOX)]["opacity"] = 0.5
        fails = check_overrides(d)[0]
        self.assertEqual(len(fails), 1)
        self.assertTrue(fails[0].startswith("03 Studio/Host/"), fails[0])
        self.assertIn("Opacity differs", fails[0])

    def test_a_clean_page_03_instance_passes_and_is_counted(self):
        base_counts = check_overrides(base())[1]
        fails, counts = check_overrides(on_page_03(base()))
        self.assertEqual((fails, counts), ([], base_counts))

    def test_a_page_03_text_style_override_fails_too(self):
        d = on_page_03(base())
        d["nodes"][str(ITEXT)]["style_refs"] = {"text": "Text lg / Bold"}
        self.assertTrue(any(f.startswith("03 Studio/") and "TextStyle differs" in f for f in check_overrides(d)[0]))


class CornerSmoothing(unittest.TestCase):
    def test_a_different_smoothing_is_a_corner_override(self):
        d = base()
        d["nodes"][str(IBOX)]["corner_smoothing"] = 0.6
        self.assertTrue(any("CornerRadii differs" in f for f in check_overrides(d)[0]))

    def test_the_same_smoothing_passes(self):
        d = base()
        for nid in (IBOX, BOX):
            d["nodes"][str(nid)]["corner_smoothing"] = 0.6
        self.assertEqual(check_overrides(d)[0], [])

    def test_no_smoothing_and_zero_smoothing_are_the_same(self):
        d = base()
        d["nodes"][str(IBOX)]["corner_smoothing"] = 0.0
        self.assertEqual(check_overrides(d)[0], [])

    def test_a_ring_may_smooth_its_corners_differently(self):
        d = ring_doc()
        d["nodes"]["73"]["corner_smoothing"] = 0.6
        self.assertEqual(check_overrides(d)[0], [])


if __name__ == "__main__":
    unittest.main()
