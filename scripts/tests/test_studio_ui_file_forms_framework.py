"""Additive framework keywords the Forms & Inputs sets need, each backward compatible (an IR that does not use them lowers as before):

* `X(..., radius=lit(...))`: a nested Focus/Ring instance pins its own corner radius (host radius + SPACE_FACT is 8 on a checkbox box, 6 on a
  swatch, 14 on a toggle track), where the default clone of the main carries 12;
* `F(..., sa="center")`: a stroke centred on the edge (egui's `circle_stroke`, which a slider handle uses), `inside` stays the default;
* `T(..., wrap=True)`: a text that wraps inside its fixed or filled width, lowered as `textAutoResize` HEIGHT, with the estimate sized to `w`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_forms_framework
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import PLAN, STYLES, FakeIcons, lower  # noqa: E402

CORNER_KEYS = {"topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"}


def ring_main(low):
    low.component("Ring", "Tone=Brand", ir.F("Ring", stroke="color/line", sw=ir.lit(2, "a 2 px band"), radius=ir.lit(12, "control + 4"), w=40, h=40), 0, 0)


class PinnedRingRadiusTest(unittest.TestCase):
    def host(self, **kw):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        ring_main(low)
        rest = low.lower(ir.F("Host", [ir.X("Focus ring", "Ring", "Tone=Brand", abs=(-4, -4, 24, 24), **kw)], dir="H", w=16, h=16, sx="fixed", sy="fixed"), 0, 0)
        return low, rest["children"][0]

    def entries(self, low, sid):
        return {a["property"]: a["value"] for a in low.allow if a["sid"] == sid}

    def test_an_instance_without_a_radius_keeps_the_mains_12(self):
        low, ring = self.host()
        self.assertEqual(ring["cornerRadius"], 12.0)
        self.assertEqual({k: v for k, v in self.entries(low, ring["id"]).items() if k in CORNER_KEYS}, {k: 12.0 for k in CORNER_KEYS})

    def test_a_pinned_radius_is_the_instances_literal_and_its_own_allow_list_entries(self):
        low, ring = self.host(radius=ir.lit(8, "checkbox box: SPACE_FACT 4 + 4"))
        self.assertEqual(ring["cornerRadius"], 8.0)
        self.assertEqual({k: v for k, v in self.entries(low, ring["id"]).items() if k in CORNER_KEYS}, {k: 8.0 for k in CORNER_KEYS})
        self.assertTrue(all(a["reason"] == "checkbox box: SPACE_FACT 4 + 4" for a in low.allow if a["sid"] == ring["id"] and a["property"] in CORNER_KEYS))

    def test_the_stroke_still_mirrors_the_mains_binding_and_the_mains_own_radius_is_untouched(self):
        low, ring = self.host(radius=ir.lit(8, "r"))
        self.assertIn(("strokes/0", "color/line"), {(i["key"], i["var"]) for i in low.intents if i["sid"] == ring["id"]})
        self.assertEqual(self.entries(low, ring["id"])["strokeWeight"], 2.0)
        main = low.main_ir[("Ring", "Tone=Brand")]
        self.assertEqual(main["radius"], ("lit", 12.0, "control + 4"))

    def test_a_radius_that_is_not_a_literal_is_refused(self):
        """A ring radius is host + SPACE_FACT, a sum no variable holds: a binding on it would draw the variable's value (0.7.5)."""
        with self.assertRaises(ValueError):
            self.host(radius="radius/control")


class StrokeAlignTest(unittest.TestCase):
    def stroke(self, **kw):
        return lower(ir.F("Dot", stroke="color/line", sw=ir.lit(2, "2 px"), w=16, h=16, sx="fixed", sy="fixed", **kw))[1]

    def test_the_default_is_inside(self):
        self.assertEqual(self.stroke()["strokeAlign"], "INSIDE")

    def test_center_and_outside_are_lowered(self):
        self.assertEqual(self.stroke(sa="center")["strokeAlign"], "CENTER")
        self.assertEqual(self.stroke(sa="outside")["strokeAlign"], "OUTSIDE")

    def test_an_unknown_alignment_is_refused_when_the_node_is_built(self):
        with self.assertRaises(ValueError):
            ir.F("Dot", sa="middle")

    def test_a_node_without_a_stroke_carries_no_alignment(self):
        self.assertNotIn("strokeAlign", lower(ir.F("Box", fill="color/bg"))[1])


class WrappedTextTest(unittest.TestCase):
    LONG = "Launch CLIs with their auto-approve flag. Fastest inside a trusted project."

    def test_a_wrapping_text_resizes_its_height_only(self):
        _, rest = lower(ir.T("Caption", self.LONG, "Body / Medium", "color/line", wrap=True, w=296))
        self.assertEqual(rest["style"]["textAutoResize"], "HEIGHT")

    def test_a_plain_text_keeps_width_and_height(self):
        _, rest = lower(ir.T("Label", "Save", "Body / Medium", "color/line"))
        self.assertEqual(rest["style"]["textAutoResize"], "WIDTH_AND_HEIGHT")

    def test_the_estimate_wraps_at_the_given_width(self):
        w, h = ir.estimate(ir.T("Caption", self.LONG, "Body / Medium", "color/line", wrap=True, w=296, line=18))
        self.assertEqual((w, h), (296, 36))

    def test_the_estimate_of_a_short_wrapping_text_is_one_line(self):
        w, h = ir.estimate(ir.T("Caption", "Short", "Body / Medium", "color/line", wrap=True, w=296, line=18))
        self.assertEqual((w, h), (296, 18))

    def test_a_wrapping_text_without_a_width_is_refused(self):
        with self.assertRaises(ValueError):
            ir.T("Caption", self.LONG, "Body / Medium", "color/line", wrap=True)

    def test_a_filled_wrapping_text_lowers_as_fill_in_an_auto_layout_parent(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        rest = low.lower(ir.F("Row", [ir.T("Caption", self.LONG, "Body / Medium", "color/line", wrap=True, w=296, sx="fill")], dir="V", w=312, sx="fixed"), 0, 0)
        text = rest["children"][0]
        self.assertEqual((text["layoutSizingHorizontal"], text["style"]["textAutoResize"]), ("FILL", "HEIGHT"))


if __name__ == "__main__":
    unittest.main()
