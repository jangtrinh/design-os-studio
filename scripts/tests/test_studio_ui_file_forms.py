"""The choice sets of the Forms & Inputs row (components/forms.py): what each variant draws, read from the IR the builders return against the real
Semantic plan, with the Rust the value comes from named in forms.py. Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_forms
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import components  # noqa: E402
import icon_lib  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover() if s["module"] == "forms"}


def build(set_name, **values):
    return SPECS[set_name]["build"](values, ENV)


def ring(node):
    rings = [k for k in walk(node) if k["t"] == "X"]
    return rings[0] if rings else None


class MatrixTest(unittest.TestCase):
    def test_the_variant_counts_follow_the_registry(self):
        want = {"Checkbox/Box": 5, "Toggle/Switch": 5, "Swatch/Color": 4, "Slider/Handle": 5, "Radio/Card": 5}
        for name, n in want.items():
            self.assertEqual(len(components.matrix(SPECS[name]["axes"])), n, name)
        self.assertTrue(all(s["row"] == "Forms & Inputs" for s in SPECS.values()))

    def test_every_set_that_paints_a_ring_uses_the_focus_ring_main(self):
        """Radio/Card draws the live permission rows, which paint no ring (the kit card's ring is the gap #1436)."""
        self.assertEqual({n for n, s in SPECS.items() if s.get("uses") == ["Focus/Ring"]}, {"Checkbox/Box", "Toggle/Switch", "Swatch/Color", "Slider/Handle"})
        self.assertEqual(SPECS["Radio/Card"].get("uses", []), [])


class CheckboxTest(unittest.TestCase):
    def test_the_box_is_16_px_at_space_fact_and_is_the_rings_parent(self):
        box = build("Checkbox/Box", State="Focus")
        self.assertEqual((box["w"], box["h"], box["radius"]), (16, 16, "space/fact"))
        self.assertIn(ring(box), box["kids"])
        self.assertEqual(ring(box)["radius"][1], 8.0)  # SPACE_FACT + SPACE_FACT: the box is painted at 4

    def test_the_looks_come_from_the_kit_table(self):
        sel, focus, off = build("Checkbox/Box", State="Selected"), build("Checkbox/Box", State="Focus"), build("Checkbox/Box", State="Disabled")
        self.assertEqual((sel["fill"], sel.get("stroke")), ("color/bg-brand", None))
        mark = [k for k in sel["kids"] if k["t"] == "I"][0]
        self.assertEqual((mark["icon"], mark["size"], mark["color"]), ("Check", 12, "color/text-on-brand"))
        self.assertEqual((focus["fill"], focus["stroke"]), ("color/bg", "color/border-strong"))
        self.assertEqual((off["fill"], off["alpha"]), ("color/bg-tertiary", 0.5))

    def test_hover_is_the_default_look_and_only_focus_has_a_ring(self):
        default, hover = build("Checkbox/Box", State="Default"), build("Checkbox/Box", State="Hover")
        self.assertEqual(default, hover)
        self.assertIsNone(ring(default))
        self.assertEqual((default["fill"], default["stroke"]), (None, "color/border-strong"))


class ToggleTest(unittest.TestCase):
    def test_the_track_is_36_by_20_with_a_pill_radius_and_a_16_px_knob(self):
        track = build("Toggle/Switch", State="Default")
        knob = [k for k in track["kids"] if k["t"] == "E"][0]
        self.assertEqual((track["w"], track["h"], track["radius"][1], track["pad"]), (36.0, 20.0, 10.0, "inset/segment"))
        self.assertEqual((knob["w"], knob["h"], knob["fill"]), (16, 16, "color/icon-white"))

    def test_the_selected_knob_sits_at_the_end_with_the_brand_fill_and_no_border(self):
        on, off = build("Toggle/Switch", State="Selected"), build("Toggle/Switch", State="Default")
        self.assertEqual((on["align"][0], on["fill"], on.get("stroke")), ("end", "color/bg-brand", None))
        self.assertEqual((off["align"][0], off["stroke"], off["sw"][1]), ("start", "color/border", 0.5))

    def test_the_ring_wraps_the_track_at_14_and_disabled_halves_every_paint(self):
        self.assertEqual(ring(build("Toggle/Switch", State="Focus"))["radius"][1], 14.0)
        off = build("Toggle/Switch", State="Disabled")
        self.assertEqual([off["alpha"]] + [k["alpha"] for k in off["kids"]], [0.5, 0.5])


class SwatchTest(unittest.TestCase):
    def test_the_colour_layer_is_named_color_and_sits_on_a_checker_base(self):
        s = build("Swatch/Color", State="Default")
        layer = s["kids"][0]
        self.assertEqual((s["fill"], s["radius"], s["w"], layer["name"], layer["radius"]), ("color/checker-a", "radius/swatch", 16, "Color", "radius/swatch"))
        self.assertEqual((layer["stroke"], layer["sw"]), ("color/border-translucent", "size/border-width"))

    def test_selected_is_a_2_px_focus_ring_edge_and_focus_adds_the_ring_at_6(self):
        sel, focus = build("Swatch/Color", State="Selected"), build("Swatch/Color", State="Focus")
        self.assertEqual((sel["kids"][0]["stroke"], sel["kids"][0]["sw"][1]), ("color/focus-ring", 2.0))
        self.assertEqual(ring(focus)["radius"][1], 6.0)
        self.assertIsNone(ring(sel))

    def test_only_the_color_layer_carries_the_content_colour(self):
        s = build("Swatch/Color", State="Hover")
        self.assertEqual([k["name"] for k in s["kids"]], ["Color"])
        self.assertEqual(build("Swatch/Color", State="Hover"), build("Swatch/Color", State="Default"))


class SliderTest(unittest.TestCase):
    def test_the_handle_is_a_pill_frame_with_a_centred_2_px_bg_stroke_and_the_icon_fill(self):
        h = build("Slider/Handle", State="Default")
        self.assertEqual((h["t"], h["w"], h["h"], h["radius"][1], h["sa"]), ("F", 16, 16, 8.0, "center"))
        self.assertEqual((h["fill"], h["stroke"], h["sw"][1]), ("color/icon-brand-primary", "color/bg", 2.0))

    def test_the_kit_fill_is_never_painted_and_the_ring_keeps_the_mains_12(self):
        h = build("Slider/Handle", State="Focus")
        self.assertNotEqual(h["fill"], "color/bg-pressed")
        self.assertNotIn("radius", ring(h))  # a pill host of radius 8 gives round(8 + 4) = 12, the main's

    def test_every_state_but_disabled_draws_the_same_and_disabled_is_half_alpha(self):
        look = lambda s: {k: v for k, v in build("Slider/Handle", State=s).items() if k != "kids"}
        self.assertTrue(all(look(s) == look("Default") for s in ("Hover", "Active", "Focus")))
        self.assertEqual(build("Slider/Handle", State="Disabled")["alpha"], 0.5)


class RadioCardTest(unittest.TestCase):
    def row(self, state):
        return build("Radio/Card", State=state)

    def test_the_row_is_312_by_68_at_radius_control_with_a_title_and_a_wrapping_caption(self):
        r = self.row("Default")
        title, caption = r["kids"]
        self.assertEqual((r["w"], r["h"], r["radius"], r["pad"], r["gap"]), (312, 68, "radius/control", ["inset/frame", "inset/frame", None, "inset/frame"], "space/fact"))
        self.assertEqual((title["style"], title["color"], title["prop"]), ("Body / Medium", "color/text", "Title"))
        self.assertEqual((caption["style"], caption["color"], caption["wrap"], caption["w"], caption["sx"]), ("Caption / Regular", "color/text-secondary", True, 296, "fill"))

    def test_fill_and_edge_follow_selected_hovered_or_neither(self):
        got = {s: (self.row(s)["fill"], self.row(s)["stroke"]) for s in ("Selected", "Default", "Hover")}
        self.assertEqual(got, {"Selected": ("color/bg-selected", "color/border-selected"), "Default": ("color/bg-tertiary", "color/border"),
                               "Hover": ("color/bg-hover", "color/border")})

    def test_the_edge_is_one_pixel_even_when_selected_unlike_the_kit_card(self):
        self.assertEqual(self.row("Selected")["sw"], "size/border-width")

    def test_focus_is_the_default_look_and_there_is_no_ring_or_radio_circle(self):
        self.assertEqual(self.row("Focus"), self.row("Default"))
        self.assertIsNone(ring(self.row("Focus")))
        self.assertEqual([k["t"] for k in self.row("Selected")["kids"]], ["T", "T"])

    def test_disabled_is_the_default_look_at_half_alpha_on_every_paint(self):
        off, default = self.row("Disabled"), self.row("Default")
        self.assertEqual([off["alpha"]] + [k["alpha"] for k in off["kids"]], [0.5, 0.5, 0.5])
        strip = lambda n: {k: v for k, v in n.items() if k not in ("alpha", "kids")}
        self.assertEqual(strip(off), strip(default))


if __name__ == "__main__":
    unittest.main()
