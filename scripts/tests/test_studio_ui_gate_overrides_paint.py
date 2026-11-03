"""G2 overrides gate: the glyph and Swatch exceptions exempt paint 0's colour and its binding, not the whole Fills field.

An icon swap or a recoloured glyph changes the glyph's first paint (its solid colour, bound to a Semantic colour; a Disabled button dims it to opacity 0.5, which G2 opacity
owns) and a Swatch/Color instance's `Color` layer is content. Everything else about the fills stays the main's: the paint count, paint 0's visibility and blend, every
other paint and every other fill binding. A text layer named `Color` gets no exemption at all: its fill lives in the TextStyle field with the font, size and runs.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_overrides_paint
Synthetic documents only (studio_ui_override_fixtures).
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import solid  # noqa: E402
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BLUE, TEXT_STYLE, icon_doc, set_text, swatch_doc  # noqa: E402

GLYPH, SWATCH_LAYER = "93", "93"


def problems(doc):
    return check_overrides(doc)[0]


def recoloured(d, nid=GLYPH):
    d["nodes"][nid]["fills"] = [solid(BLUE)]
    d["nodes"][nid]["variables"]["bindings"]["fills/0"] = "s.accent"
    return d


def has(d, text):
    return any(text in f for f in problems(d))


class Glyph(unittest.TestCase):
    def test_the_colour_and_its_semantic_binding_pass(self):
        self.assertEqual(problems(recoloured(icon_doc())), [])

    def test_the_dimming_of_a_disabled_glyph_passes(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"][0]["opacity"] = 0.5
        self.assertEqual(problems(d), [])

    def test_a_second_paint_fails_even_when_paint_0_stays_semantic(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"].append(solid(BLUE))
        self.assertTrue(has(d, "Fills differs"))

    def test_a_second_paint_bound_to_a_semantic_colour_fails_both_ways(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"].append(solid(BLUE))
        d["nodes"][GLYPH]["variables"]["bindings"]["fills/1"] = "s.accent"
        fails = problems(d)
        self.assertTrue(any("Fills differs" in f for f in fails))
        self.assertTrue(any("fills/1 is bound to color/bg-brand" in f for f in fails))

    def test_a_hidden_first_paint_fails(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"][0]["visible"] = False
        self.assertTrue(has(d, "Fills differs"))

    def test_a_blend_on_the_first_paint_fails(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"][0]["blend"] = "Multiply"
        self.assertTrue(has(d, "Fills differs"))

    def test_a_first_paint_that_is_no_longer_solid_fails(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"][0]["kind"] = {"LinearGradient": {"stops": []}}
        self.assertTrue(has(d, "Fills differs"))

    def test_a_dropped_paint_fails(self):
        d = recoloured(icon_doc())
        d["nodes"][GLYPH]["fills"] = []
        self.assertTrue(has(d, "Fills differs"))

    def test_the_main_may_have_two_paints_if_the_glyph_keeps_the_second(self):
        d = icon_doc()
        for nid in ("91", GLYPH):
            d["nodes"][nid]["fills"] = [solid(), solid()]
        recoloured(d)
        d["nodes"][GLYPH]["fills"] = [solid(BLUE), solid()]
        self.assertEqual(problems(d), [])


class Swatch(unittest.TestCase):
    def test_the_colour_layer_fill_and_its_binding_pass(self):
        self.assertEqual(problems(recoloured(swatch_doc(), SWATCH_LAYER)), [])

    def test_a_second_paint_on_the_colour_layer_fails(self):
        d = recoloured(swatch_doc(), SWATCH_LAYER)
        d["nodes"][SWATCH_LAYER]["fills"].append(solid(BLUE))
        self.assertTrue(has(d, "Fills differs"))

    def test_a_blend_on_the_colour_layer_paint_fails(self):
        d = recoloured(swatch_doc(), SWATCH_LAYER)
        d["nodes"][SWATCH_LAYER]["fills"][0]["blend"] = "Multiply"
        self.assertTrue(has(d, "Fills differs"))

    def test_a_second_fill_binding_on_the_colour_layer_fails(self):
        d = recoloured(swatch_doc(), SWATCH_LAYER)
        d["nodes"][SWATCH_LAYER]["variables"]["bindings"]["fills/1"] = "s.accent"
        self.assertTrue(has(d, "fills/1 is bound to color/bg-brand"))

    def swatch_text(self):
        """The `Color` layer is a text in the main and in the copy (the exception must not reach a text)."""
        d = swatch_doc()
        for nid in ("82", SWATCH_LAYER):
            d["nodes"][nid]["kind"] = {"Text": {"content": "Color", "style": copy.deepcopy(TEXT_STYLE), "auto_resize": "WidthAndHeight"}}
            d["nodes"][nid]["fills"] = []
        return d

    def test_a_colour_text_with_the_mains_style_passes(self):
        self.assertEqual(problems(self.swatch_text()), [])

    def test_a_colour_text_with_another_font_size_fails(self):
        d = self.swatch_text()
        set_text(d, SWATCH_LAYER, font_size=20.0)
        self.assertTrue(has(d, "TextStyle differs"))

    def test_a_colour_text_with_another_text_fill_fails(self):
        d = self.swatch_text()
        set_text(d, SWATCH_LAYER, fills=[solid(BLUE)])
        self.assertTrue(has(d, "TextStyle differs"))

    def test_a_colour_text_with_another_fill_binding_fails(self):
        d = self.swatch_text()
        d["nodes"][SWATCH_LAYER]["variables"]["bindings"]["fills/0"] = "s.accent"
        self.assertTrue(has(d, "fills/0 is bound to color/bg-brand"))


if __name__ == "__main__":
    unittest.main()
