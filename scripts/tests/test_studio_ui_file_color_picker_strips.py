"""The picker's three strips as real gradients (components/color_picker_strips.py), at the capture's state (hue h = 0, saturation 0, value 1, alpha 1), read from the IR the builder returns.

* SV square (`sv_area`, picker.rs:637-656): the colour at (s, v) is `v * lerp(white, hue colour, s)`, the product of two linear terms, so two layers draw it exactly: white to red left to right, and over
  it black from alpha 0 at the top to alpha 1 at the bottom.
* Hue strip (`hue_bar`, :658-671): seven stops, `Hsva(i / 6, 1, 1)`, red at both ends.
* Alpha strip (`checker` then `quad_mesh`, :612-635): a CHECKER_A base (`color/checker-a`), CHECKER_B cells (`color/checker-b`) of 4 px where `(i + j) % 2 == 1`, and white from alpha 0 to alpha 1 over them, clipped at the strip's radius.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_color_picker_strips
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
from test_studio_ui_file_color_picker import ENV, SPEC, part  # noqa: E402

RED, WHITE = (255, 0, 0, 255), (255, 255, 255, 255)
HUE = [(255, 0, 0), (255, 255, 0), (0, 255, 0), (0, 255, 255), (0, 0, 255), (255, 0, 255), (255, 0, 0)]


def stops(node, layer=0):
    return [(p, c) for p, c, _ in node["fills"][layer]["stops"]]


class StripsTest(unittest.TestCase):
    def setUp(self):
        self.picker = SPEC["build"]({"Kind": "Fill"}, ENV)

    def test_the_sv_square_is_white_to_red_under_a_transparent_to_black_layer(self):
        sv = part(self.picker, "SV square")
        self.assertEqual(len(sv["fills"]), 2)
        self.assertEqual(stops(sv, 0), [(0, WHITE), (1, RED)])
        self.assertEqual((sv["fills"][0]["start"], sv["fills"][0]["end"]), ((0.0, 0.5), (1.0, 0.5)))
        self.assertEqual(stops(sv, 1), [(0, (0, 0, 0, 0)), (1, (0, 0, 0, 255))])
        self.assertEqual((sv["fills"][1]["start"], sv["fills"][1]["end"]), ((0.5, 0.0), (0.5, 1.0)))
        self.assertEqual((sv["radius"], sv["abs"], "fill" in sv), ("radius/control", (16, 122, 208, 208), False))

    def test_the_hue_strip_has_the_seven_hsv_stops_red_at_both_ends(self):
        hue = part(self.picker, "Hue strip")
        self.assertEqual([(round(p * 6), c[:3]) for p, c in stops(hue)], list(enumerate(HUE)))
        self.assertTrue(all(c[3] == 255 for _, c in stops(hue)))
        self.assertEqual((hue["abs"], hue["radius"][:2]), ((52, 343, 172, 12), ("lit", 6.0)))

    def test_every_stop_says_where_its_colour_comes_from(self):
        strip = part(self.picker, "Alpha strip")
        for node in (part(self.picker, "SV square"), part(self.picker, "Hue strip"), part(strip, "Alpha ramp")):
            for fill in node["fills"]:
                for _, _, reason in fill["stops"]:
                    self.assertIn("picker.rs", reason, node["name"])

    def test_the_alpha_strip_is_a_clipped_checker_under_a_white_ramp_from_alpha_0_to_1(self):
        strip = part(self.picker, "Alpha strip")
        self.assertEqual((strip["abs"], strip["fill"], strip["clip"], strip["radius"][:2]), ((52, 371, 172, 12), "color/checker-a", True, ("lit", 6.0)))
        ramp = part(strip, "Alpha ramp")
        self.assertEqual((stops(ramp), ramp["abs"]), ([(0, (255, 255, 255, 0)), (1, WHITE)], (0, 0, 172, 12)))
        self.assertEqual(strip["kids"][-1]["name"], "Alpha ramp", "the ramp is the top layer")

    def test_the_checker_cells_are_4_px_where_i_plus_j_is_odd_over_43_by_3_cells(self):
        cells = [k for k in part(self.picker, "Alpha strip")["kids"] if k["name"].startswith("Checker")]
        self.assertEqual(len(cells), 64, "43 x 3 = 129 cells, 64 of them odd")
        self.assertTrue(all(k["fill"] == "color/checker-b" and k["w"] == 4 and k["h"] == 4 for k in cells))
        self.assertEqual({k["abs"][:2] for k in cells}, {(4 * i, 4 * j) for j in range(3) for i in range(43) if (i + j) % 2 == 1})

    def test_the_strips_keep_their_painter_order_before_the_thumbs(self):
        names = [k["name"] for k in self.picker["kids"]]
        self.assertLess(names.index("SV square"), names.index("SV thumb"))
        self.assertLess(names.index("Hue strip"), names.index("Hue thumb"))
        self.assertLess(names.index("Alpha strip"), names.index("Alpha thumb"))


if __name__ == "__main__":
    unittest.main()
