"""G2 overrides gate, exceptions: what an instance may still own, and what stays forbidden next to each exception.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_overrides_exceptions
Synthetic documents only (studio_ui_override_fixtures).
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import CORNERS, solid  # noqa: E402
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BLUE, IBOX, base, icon_doc, ring_doc, swatch_doc, tree_doc  # noqa: E402


def problems(doc):
    return check_overrides(doc)[0]


class Exceptions(unittest.TestCase):
    def test_glyph_fill_bound_to_a_semantic_colour(self):
        d = icon_doc()
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        d["nodes"]["93"]["variables"]["bindings"]["fills/0"] = "s.accent"
        self.assertEqual(problems(d), [])

    def test_glyph_fill_unbound_fails(self):
        d = icon_doc()
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        d["nodes"]["93"]["variables"]["bindings"] = {}
        self.assertTrue(any("Fills differs" in f for f in problems(d)))

    def test_glyph_fill_bound_to_a_primitive_fails(self):
        d = icon_doc()
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        d["nodes"]["93"]["variables"]["bindings"]["fills/0"] = "p.blue"
        self.assertTrue(any("Fills differs" in f for f in problems(d)))

    def test_glyph_stroke_still_fails(self):
        d = icon_doc()
        d["nodes"]["93"]["stroke"] = {"paints": [solid()], "weight": 1.0}
        self.assertTrue(any("Stroke differs" in f for f in problems(d)))

    def test_the_exception_is_the_first_child_of_an_icon_only(self):
        d = base()
        d["nodes"][str(IBOX)]["fills"] = [solid(BLUE)]
        d["nodes"][str(IBOX)]["variables"]["bindings"]["fills/0"] = "s.accent"
        self.assertTrue(any("Fills differs" in f for f in problems(d)))

    def test_a_second_child_of_an_icon_is_no_glyph(self):
        d = icon_doc()
        d["nodes"]["94"] = dict(d["nodes"]["93"], id=94, name="Dot", component={"source": 91})
        d["nodes"]["92"]["children"].append(94)
        d["nodes"]["94"]["fills"] = [solid(BLUE)]
        d["nodes"]["94"]["variables"] = {"modes": {}, "bindings": {"fills/0": "s.accent"}}
        self.assertTrue(any("Dot: Fills differs" in f for f in problems(d)))

    def test_ring_corners(self):
        d = ring_doc()
        d["nodes"]["73"]["corner_radii"] = [6.0] * 4
        self.assertEqual(problems(d), [])

    def test_ring_corner_binding_may_differ(self):
        d = ring_doc()
        d["nodes"]["73"]["corner_radii"] = [6.0] * 4
        d["nodes"]["73"]["variables"] = {"modes": {}, "bindings": {}}
        d["nodes"]["71"]["variables"] = {"modes": {}, "bindings": {k: "s.fact" for k in CORNERS}}
        self.assertEqual(problems(d), [])

    def test_ring_stroke_still_fails(self):
        d = ring_doc()
        d["nodes"]["73"]["stroke"] = {"paints": [solid(BLUE)], "weight": 3.0}
        self.assertTrue(any("Stroke differs" in f for f in problems(d)))

    def test_corner_radii_of_a_non_ring_still_fail(self):
        d = ring_doc()
        d["nodes"]["74"]["corner_radii"] = [3.0] * 4  # an instance of another main
        self.assertTrue(any("CornerRadii differs" in f for f in problems(d)))

    def test_swatch_colour_layer(self):
        d = swatch_doc()
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        d["nodes"]["93"]["variables"]["bindings"]["fills/0"] = "s.accent"
        self.assertEqual(problems(d), [])

    def test_swatch_colour_layer_unbound_literal(self):
        d = swatch_doc()
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        d["nodes"]["93"]["variables"]["bindings"] = {}
        self.assertEqual(problems(d), [])

    def test_swatch_other_layer_fails(self):
        d = swatch_doc()
        d["nodes"]["93"]["name"] = "Fill"
        d["nodes"]["93"]["fills"] = [solid(BLUE)]
        self.assertTrue(any("Fills differs" in f for f in problems(d)))

    def test_a_layer_named_color_outside_a_swatch_fails(self):
        d = base()
        d["nodes"][str(IBOX)]["name"] = "Color"
        d["nodes"][str(IBOX)]["fills"] = [solid(BLUE)]
        self.assertTrue(any("Fills differs" in f for f in problems(d)))

    def test_the_swatch_instance_itself_is_not_its_colour_layer(self):
        d = swatch_doc()
        d["nodes"]["92"]["name"] = "Color"
        d["nodes"]["92"]["fills"] = [solid(BLUE)]
        self.assertTrue(any("Color: Fills differs" in f for f in problems(d)))

    def test_swatch_opacity_still_fails(self):
        d = swatch_doc()
        d["nodes"]["93"]["opacity"] = 0.4
        self.assertTrue(any("Opacity differs" in f for f in problems(d)))

    def test_a_tree_row_instance_owns_no_indent_the_depth_variant_carries_it(self):
        """The indent is a property of the Depth variant (studio_ui_gate_indent): an instance that sets a multiple of the step itself still differs from its main."""
        for pad in (14.0, 28.0):
            self.assertTrue(any("Layout differs" in f for f in problems(tree_doc(pad))), pad)

    def test_tree_row_indent_not_a_multiple_fails(self):
        self.assertTrue(any("Layout differs" in f for f in problems(tree_doc(20.0))))

    def test_tree_row_zero_indent_is_the_main(self):
        self.assertEqual(problems(tree_doc(0.0)), [])

    def test_tree_row_other_padding_fails(self):
        d = tree_doc(28.0)
        d["nodes"]["82"]["kind"]["Instance"]["frame"]["auto_layout"]["padding"][1] = 9.0
        self.assertTrue(any("Layout differs" in f for f in problems(d)))

    def test_tree_row_indent_bound_to_another_variable_fails(self):
        d = tree_doc(28.0)
        d["nodes"]["82"]["variables"] = {"modes": {}, "bindings": {"paddingLeft": "s.fact"}}
        self.assertTrue(any("paddingLeft is bound to space/fact" in f for f in problems(d)))

    def test_indent_on_a_non_tree_row_instance_fails(self):
        d = tree_doc(28.0)
        d["nodes"]["80"]["name"] = "Other/Row"
        self.assertTrue(any("Layout differs" in f for f in problems(d)))


if __name__ == "__main__":
    unittest.main()
