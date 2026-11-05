"""A paint opacity no variable holds: `F(fill="color/x", alpha=lit(0.22, reason))`, a bound fill whose paint carries an opacity of its own, waived in the allow-list with its reason.

The Rust draws `DEV_READY.gamma_multiply(0.22)` (the Ready-for-dev pill, dev/sections.rs:36): the named constant at a raw alpha. The fill stays bound to the constant's Semantic variable (a rebound
theme moves it) and the 0.22 is the paint's opacity, a literal: `fills/0/opacity` in the allow-list with the reason and the value, which G2 opacity reads (half alpha per paint is the Disabled
rule, every other paint carries 1.0 unless an entry says otherwise). A plain `alpha` of 0.5 is still the Disabled half alpha and records nothing. Synthetic plan, no engine.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_paint_opacity
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

WHY = "dev/sections.rs:36 `status_color(s).gamma_multiply(0.22)`: a raw alpha over a named constant"
SIZE = dict(w=40, h=20, sx="fixed", sy="fixed")


def lower(node):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    return low, low.lower(node, 0, 0, ptype="COMPONENT")


def pill(**kw):
    return ir.F("Pill", fill="color/bg", alpha=ir.lit(0.22, WHY), **SIZE, **kw)


class LowerOpacityTest(unittest.TestCase):
    def test_the_fill_is_the_variables_colour_with_the_opacity_on_the_paint(self):
        _, rest = lower(pill())
        paint = rest["fills"][0]
        self.assertEqual(paint["opacity"], 0.22)
        self.assertEqual({k: round(v * 255) for k, v in paint["color"].items()}, {"r": 10, "g": 10, "b": 10, "a": 255})

    def test_the_fill_stays_bound_and_the_opacity_is_one_allow_list_entry_with_its_reason_and_value(self):
        low, rest = lower(pill())
        self.assertEqual([i["var"] for i in low.intents if i["key"] == "fills/0"], ["color/bg"])
        self.assertEqual(low.allow, [{"sid": rest["id"], "property": "fills/0/opacity", "reason": WHY, "value": 0.22}])

    def test_a_disabled_half_alpha_is_a_paint_opacity_and_records_nothing(self):
        low, rest = lower(ir.F("Pill", fill="color/bg", alpha=0.5, **SIZE))
        self.assertEqual((rest["fills"][0]["opacity"], low.allow), (0.5, []))

    def test_a_full_alpha_has_no_opacity_and_no_entry(self):
        low, rest = lower(ir.F("Pill", fill="color/bg", **SIZE))
        self.assertEqual(("opacity" in rest["fills"][0], low.allow), (False, []))


class ValidationTest(unittest.TestCase):
    def refused(self, **kw):
        with self.assertRaises(ValueError):
            ir.F("Pill", **{"fill": "color/bg", **kw})

    def test_an_opacity_outside_the_open_unit_interval_is_refused(self):
        for value in (0, 1, 1.2, -0.1, True):
            with self.subTest(value):
                self.refused(alpha=ir.lit(value, WHY))

    def test_an_opacity_without_a_reason_is_refused(self):
        self.refused(alpha=ir.lit(0.22, ""))

    def test_an_opacity_needs_the_one_fill_and_no_stroke_or_gradient(self):
        self.refused(alpha=ir.lit(0.22, WHY), fill=None)
        self.refused(alpha=ir.lit(0.22, WHY), stroke="color/bg", sw="size/border-width")
        self.refused(alpha=ir.lit(0.22, WHY), fill=None, fills=[ir.grad([(0, (1, 2, 3, 255), "a"), (1, (4, 5, 6, 255), "b")])])

    def test_a_tuple_that_is_not_a_literal_is_refused(self):
        self.refused(alpha=("tint", 0.22, WHY))


if __name__ == "__main__":
    unittest.main()
