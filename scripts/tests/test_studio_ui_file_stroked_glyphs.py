"""The glyphs egui strokes with `line_segment` are stroked open paths, not filled outlines or a stand-in icon.

Probed on designos-mcp 0.7.5: an open VECTOR with a stroke and no fill renders as a line (see test_studio_ui_file_strokes). The Find panel's step chevrons (find/panel.rs:132-153: two segments,
4.5 by 2.5 around the centre of a 24 px box, `Stroke::new(1.5, col)`), the Variables window's collapse arrows (variables/widgets.rs:81-93: three segments per corner arrow, `Stroke::new(1.2,
col)`) and the Spec reader's send arrow (specs.rs:2864-2866: a stem and two legs, `Stroke::new(1.6, WHITE)`) are the three callers. Each keeps the rect the capture gave it.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_stroked_glyphs
"""
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import studio_ui_regions_fixtures as fx  # noqa: E402


def subpaths(node):
    """[[(x, y), ...]] of a path made of `M x y L x y ...` subpaths."""
    return [[(float(x), float(y)) for x, y in re.findall(r"([-\d.]+) ([-\d.]+)", chunk)] for chunk in node["path"].split("M")[1:]]


class StrokedGlyphTest(unittest.TestCase):
    def check_stroke(self, node, colour, weight, rect):
        self.assertEqual((node["t"], node["fill"], node["stroke"], node["sw"][:2], node["abs"]), ("V", None, colour, ("lit", weight), rect))
        self.assertTrue(node["sw"][2], "the weight carries the line of the Rust that names it")

    def test_the_find_step_chevrons_are_two_segments_around_the_centre_of_their_boxes(self):
        region = fx.build("Panel/FindReplace")
        up, down = fx.part(region, "Previous"), fx.part(region, "Next")
        self.check_stroke(up, "color/icon-secondary", 1.5, (184.0, 84.0, 24.0, 24.0))
        self.check_stroke(down, "color/icon-secondary", 1.5, (208.0, 84.0, 24.0, 24.0))
        self.assertEqual(subpaths(up), [[(7.5, 14.5), (12.0, 9.5)], [(12.0, 9.5), (16.5, 14.5)]])
        self.assertEqual(subpaths(down), [[(7.5, 9.5), (12.0, 14.5)], [(12.0, 14.5), (16.5, 9.5)]])
        self.assertEqual((up["w"], up["h"]), (24.0, 24.0))

    def test_the_collapse_arrows_are_six_segments_in_a_16_px_box_with_a_1_2_stroke(self):
        region = fx.build("Window/Variables")
        arrows = fx.part(region, "Collapse")
        self.check_stroke(arrows, "color/icon", 1.2, (1293, 16, 16, 16))
        self.assertEqual(len(subpaths(arrows)), 6)
        ends = {p for seg in subpaths(arrows) for p in seg}
        self.assertLessEqual({(6.5, 9.5), (9.5, 6.5)}, ends, "both arrow tips sit 1.5 px from the centre, as `arrows_glyph(.., inward)` puts them")

    def test_the_send_arrow_is_a_stem_and_two_legs_in_white(self):
        region = fx.build("Panel/SpecReader")
        arrow = fx.part(region, "Send arrow")
        self.check_stroke(arrow, "color/icon", 1.6, (434, 6, 20, 20))
        self.assertEqual(subpaths(arrow), [[(10.0, 15.0), (10.0, 6.0)], [(6.5, 9.0), (10.0, 5.5)], [(13.5, 9.0), (10.0, 5.5)]])


if __name__ == "__main__":
    unittest.main()
