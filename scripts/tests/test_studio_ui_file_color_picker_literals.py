"""Popover/ColorPicker of components/color_picker.py: the colours the painter draws from inline `Color32` literals, the raw Rust literals that stay `lit`, and the pins on the contrast
row, the thumbs and the blend glyph. The rest of the picker is in test_studio_ui_file_color_picker.py. Read from the IR the builder returns against the real Semantic plan.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_color_picker_literals
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
import components.color_picker  # noqa: E402,F401  (the module under test)
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPEC = next(s for s in components.discover() if s["name"] == "Popover/ColorPicker")
OPAQUE = {v["name"]: v["value"] for v in PLAN["semantic"] if v["kind"] == "Color" and v["value"]["a"] == 255 and v["code_syntax"] == f"theme::{v['const']}"}
# The inline `Color32` literals of `show` that no variable holds, with the theme tone the component draws instead (the controller's rule: the nearest tone by value, even when it is
# a hover role) and its distance (the largest per-channel difference). Kept in step with the table in the `color_picker.py` docstring.
INLINE = {"contrast ink 0xE4E4E7 (:955)": ((0xE4, 0xE4, 0xE7), "color/text-secondary-hover", 2),
          "badge text 0x4CC58B (:960)": ((0x4C, 0xC5, 0x8B), "color/text-success", 25),
          "hue thumb 0xFF0000 at hue 0 (:1056)": ((0xFF, 0x00, 0x00), "color/bg-error-solid", 38),
          "alpha thumb 0xFFFFFF (:1058)": ((0xFF, 0xFF, 0xFF), "color/icon", 0),
          "thumb ring Color32::WHITE (:679)": ((0xFF, 0xFF, 0xFF), "color/icon", 0)}


def distance(rgb, name):
    v = OPAQUE[name]
    return max(abs(rgb[0] - v["r"]), abs(rgb[1] - v["g"]), abs(rgb[2] - v["b"]))


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class ColorPickerLiteralsTest(unittest.TestCase):
    """`show` (widgets/picker.rs:737-1217), the solid mode, origin at the popover's top-left."""

    def setUp(self):
        self.picker = SPEC["build"]({"Kind": "Fill"}, ENV)

    def test_the_contrast_ink_is_one_tone_on_the_ring_the_half_disc_and_the_ratio(self):
        """`fg_col` (:955) paints `circle_stroke` (:956), the half disc (:957) and the ratio text (:958): one colour, so one variable."""
        bar = part(self.picker, "Contrast bar")
        self.assertEqual([part(bar, "Ring")["stroke"], part(bar, "Half")["fill"], part(bar, "Ratio")["color"]], ["color/text-secondary-hover"] * 3)

    def test_the_badge_is_the_aaa_green_tone_on_the_nearest_success_surface(self):
        """AAA (:960): text `0x4CC58B` is TEXT_SUCCESS (25 away; BADGE_SUCCESS_TEXT is 58 away), on `0x17302A`, drawn BG_SUCCESS_PRIMARY (the nearest success surface, 20 away; a neutral
        surface such as BG_TERTIARY is 15 away but loses the green)."""
        badge = part(part(self.picker, "Contrast bar"), "Badge")
        self.assertEqual((badge["fill"], part(badge, "Level")["color"]), ("color/bg-success-primary", "color/text-success"))

    def test_the_thumb_fills_and_rings_take_the_nearest_tone_of_the_hue_the_white_and_the_ring(self):
        """`thumb_ring` (:674-680) paints `Color32::WHITE` for the ring; the hue thumb's fill is the hue at its position (red at hue 0, the capture's state), the alpha thumb's the colour
        with alpha 1 (white)."""
        hue, alpha, sv = part(self.picker, "Hue thumb"), part(self.picker, "Alpha thumb"), part(self.picker, "SV thumb")
        self.assertEqual([part(hue, "Disc")["fill"], part(alpha, "Disc")["fill"]], ["color/bg-error-solid", "color/icon"])
        self.assertEqual([part(t, "Ring")["stroke"] for t in (hue, alpha, sv)], ["color/icon"] * 3)

    def test_every_inline_literal_takes_the_nearest_opaque_theme_tone_by_value(self):
        """The rule, checked on what the IR draws and against every opaque colour Semantic variable, not against the picks: the tone drawn is the table's tone, at the table's distance, and no
        theme tone is nearer (a tie is fine)."""
        bar = part(self.picker, "Contrast bar")
        drawn = {"contrast ink 0xE4E4E7 (:955)": part(bar, "Ratio")["color"], "badge text 0x4CC58B (:960)": part(part(bar, "Badge"), "Level")["color"],
                 "hue thumb 0xFF0000 at hue 0 (:1056)": part(part(self.picker, "Hue thumb"), "Disc")["fill"],
                 "alpha thumb 0xFFFFFF (:1058)": part(part(self.picker, "Alpha thumb"), "Disc")["fill"],
                 "thumb ring Color32::WHITE (:679)": part(part(self.picker, "SV thumb"), "Ring")["stroke"]}
        for what, (rgb, name, expected) in INLINE.items():
            nearest = min(distance(rgb, other) for other in OPAQUE)
            self.assertEqual((drawn[what], distance(rgb, drawn[what]), nearest), (name, expected, expected), what)

    def test_the_docstring_records_every_inline_literal_with_its_tone_and_distance(self):
        """Each mapping is a recorded deviation: the literal, the constant drawn and the distance, so a reader never mistakes a stand-in for the painter's colour."""
        doc = " ".join(components.color_picker.__doc__.split())
        for literal, const, d in (("0xE4E4E7", "TEXT_SECONDARY_HOVER", 2), ("0x4CC58B", "TEXT_SUCCESS", 25), ("0x17302A", "BG_SUCCESS_PRIMARY", 20), ("FF0000", "BG_ERROR_SOLID", 38),
                                  ("FFFFFF", "ICON", 0)):
            self.assertRegex(doc, rf"{literal}[^|]*\| {const} \| {d}\b", (literal, const, d))
        self.assertIn("BG_TERTIARY", doc)  # why the badge surface is not the numerically nearest neutral

    def test_the_blend_button_draws_its_16_px_glyph_in_icon_secondary_at_rest(self):
        """`blend_dropdown` (:1353-1362): `Icon::Blend` 16 px (:1360) in ICON at hover or open and ICON_SECONDARY otherwise, so at rest ICON_SECONDARY (not the Button/Icon look's glyph colour)."""
        blend = part(self.picker, "Blend")
        self.assertEqual([(i["icon"], i["size"], i["color"]) for i in blend["kids"]], [("Blend", 16, "color/icon-secondary")])
        self.assertEqual((blend["radius"], blend["w"], blend["h"]), ("radius/control", 24, 24))

    def test_the_thumbs_are_rings_of_outer_minus_half_the_ring_with_the_fill_inside_the_ring(self):
        """`thumb_ring(p, c, outer, ring, fill)` (:674-680): a stroke of `ring` px centred on the circle of radius `outer - ring / 2` (so the thumb is `2 * outer` across), over a disc of radius
        `outer - ring`. The area's thumb is `thumb_ring(.., 7, 2, None)` (:1016), the strips' are `(.., 8, 3, Some(fill))` (:1056, :1058). The strips' thumbs sit at the hue's and the alpha's
        ends (`left + 6 + t * (w - 12)`)."""
        for name, frame, ring_d, ring_w, disc in (("SV thumb", (9, 115, 14, 14), 12, 2, None), ("Hue thumb", (50, 341, 16, 16), 13, 3, 10), ("Alpha thumb", (210, 369, 16, 16), 13, 3, 10)):
            thumb = part(self.picker, name)
            self.assertEqual(thumb["abs"], frame, name)
            ring = part(thumb, "Ring")
            offset = (frame[2] - ring_d) / 2
            self.assertEqual((ring["w"], ring["h"], ring["abs"], ring["sa"], ring["sw"][:2]), (ring_d, ring_d, (offset, offset, ring_d, ring_d), "center", ("lit", float(ring_w))), name)
            self.assertEqual(ring_d + ring_w, frame[2], f"{name}: the stroke's outer edge is `outer`")
            self.assertIn("picker.rs:674-680", ring["sw"][2])
            discs = [k for k in thumb["kids"] if k["name"] == "Disc"]
            if disc is None:
                self.assertEqual(discs, [], name)
            else:
                self.assertEqual([(k["w"], k["h"], k["abs"]) for k in discs], [(disc, disc, (ring_w, ring_w, disc, disc))], name)

    def test_the_contrast_colours_are_pinned_one_by_one(self):
        """Each colour of the row alone, so changing any single one fails: the three ink parts, the badge's surface and the badge's text."""
        bar = part(self.picker, "Contrast bar")
        badge = part(bar, "Badge")
        self.assertEqual(part(bar, "Ring")["stroke"], "color/text-secondary-hover")
        self.assertEqual(part(bar, "Half")["fill"], "color/text-secondary-hover")
        self.assertEqual(part(bar, "Ratio")["color"], "color/text-secondary-hover")
        self.assertEqual(badge["fill"], "color/bg-success-primary")
        self.assertEqual(part(badge, "Level")["color"], "color/text-success")
        self.assertEqual((part(bar, "Ratio")["style"], part(badge, "Level")["style"]), ("Body / Semibold", "Caption / Semibold"))

    def test_a_raw_literal_of_the_rust_is_a_lit_with_its_line_and_never_an_equal_valued_variable(self):
        """The wave's rule: only a value the Rust takes from a `theme::` constant binds that constant. Every one of these is a bare number in `show` (v0.7.5 widgets/picker.rs), although
        BORDER_WIDTH, INSET_CONTROL, RADIUS_SEGMENT and RADIUS_MARK hold the same values: 8 (the label, hex and opacity pads), 1.0 (the dropdown stroke), 6.0 (the strips' corners), 2.0 (the
        half disc)."""
        def lit_at(value, number, line):
            self.assertEqual((value[0], value[1]), ("lit", float(number)), (value, line))
            self.assertIn(f"picker.rs:{line}", value[2])

        for name in ("Custom tab", "Libraries tab"):
            tab = part(self.picker, name)
            lit_at(pads(tab)[1], 8, "758,777")
            lit_at(pads(tab)[3], 8, "758,777")
        fmt, source = part(self.picker, "Format"), part(self.picker, "Source")
        for dropdown in (fmt, source):
            lit_at(dropdown["sw"], 1, 1293)
            lit_at(pads(dropdown)[3], 8, 1298)
        hex_cell, opacity = part(self.picker, "Hex cell"), part(self.picker, "Opacity cell")
        lit_at(pads(hex_cell)[3], 8, 1093)
        lit_at(pads(opacity)[3], 8, 1436)
        lit_at(pads(opacity)[1], 8, 1438)
        lit_at(part(self.picker, "Hue strip")["radius"], 6, 1043)
        lit_at(part(self.picker, "Alpha strip")["radius"], 6, 1054)
        self.assertEqual(part(self.picker, "SV square")["radius"], "radius/control")  # `theme::RADIUS_CONTROL as f32` (:988): the Rust names it
        lit_at(part(part(self.picker, "Contrast bar"), "Half")["radius"], 2, 957)


if __name__ == "__main__":
    unittest.main()
