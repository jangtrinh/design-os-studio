"""Popover/ColorPicker of components/color_picker.py (row `Menus & Popovers` of page `03 Studio`), the solid-paint picker (`f-picker`), read from the IR the builder
returns against the real Semantic plan, with the Rust each value comes from named in the module. Structure only: pixels are compared in `evidence compare` on the parts
that are not gradients.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_color_picker
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
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
import components.color_picker  # noqa: E402,F401  (the module under test)
from components.env import Env  # noqa: E402
from rest_ir import walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPEC = next(s for s in components.discover() if s["name"] == "Popover/ColorPicker")
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class ColorPickerTest(unittest.TestCase):
    """`show` (widgets/picker.rs:737-1217), the solid mode, origin at the popover's top-left."""

    def setUp(self):
        self.picker = SPEC["build"]({"Kind": "Fill"}, ENV)

    def test_it_is_a_plain_record_on_the_menus_and_popovers_row_nesting_the_icon_and_close_buttons(self):
        self.assertEqual((SPEC["axes"], SPEC["row"]), ({"Kind": ["Fill", "Page", "Gradient"]}, "Menus & Popovers"))
        self.assertEqual(sorted(SPEC["uses"]), ["Button/Close", "Button/Icon", "Divider/Hairline"])

    def test_the_frame_is_240_by_538_in_bg_with_a_one_px_elevation_ring_at_radius_frame(self):
        """`rect_filled(rect, RADIUS_FRAME, BG)` and a 1 px inside `ELEVATION_400_RING` stroke (:748-749); `elevation_400` adds a shadow whose colour is alpha 0 in theme.rs."""
        p = self.picker
        self.assertEqual((p["w"], p["h"], p["fill"], p["stroke"], p["sw"][:2], p["radius"]), (240, 538, "color/bg", "color/elevation-400-ring", ("lit", 1.0), "radius/frame"))
        self.assertIn("picker.rs:749", p["sw"][2])  # `Stroke::new(1.0, ELEVATION_400_RING)`: a raw 1.0

    def test_the_tabs_are_local_painters_24_high_at_8_with_the_active_one_filled(self):
        """:754-787, not Tabs/Pill (a different painter): `Custom` active (BG_SECONDARY pill, TEXT), `Libraries` plain (TEXT_SECONDARY), both `font_body_strong`."""
        custom, libs = part(self.picker, "Custom tab"), part(self.picker, "Libraries tab")
        self.assertEqual((custom["abs"][:2], custom["h"], custom["fill"], custom["radius"]), ((8, 8), 24, "color/bg-secondary", "radius/control"))
        self.assertEqual((libs["abs"][:2], libs["h"], libs.get("fill")), ((73, 8), 24, None))
        self.assertEqual([(t["text"], t["style"], t["color"]) for t in walk(custom) if t["t"] == "T"], [("Custom", "Body / Semibold", "color/text")])
        self.assertEqual([(t["text"], t["color"]) for t in walk(libs) if t["t"] == "T"], [("Libraries", "color/text-secondary")])

    def test_add_style_and_close_are_the_icon_and_close_buttons_resized_to_24(self):
        add, close = part(self.picker, "Add style"), part(self.picker, "Close")
        self.assertEqual((add["set"], add["variant"], add["props"], add["abs"]), ("Button/Icon", "Size=Xs, State=Default", {"Icon": "Plus"}, (180, 8, 24, 24)))
        self.assertEqual((close["set"], close["variant"], close["abs"]), ("Button/Close", "Size=Xs, State=Default", (208, 8, 24, 24)))

    def test_the_four_rules_are_divider_hairline_instances_across_the_frame(self):
        """`hline(p, rect.left(), rect.right(), y, BORDER)` at header 39, paint row 79, contrast bar 105 and the swatch separator 441 (:799, :843, :972, :1175)."""
        lines = [k for k in self.picker["kids"] if k["t"] == "X" and k["set"] == "Divider/Hairline"]
        self.assertEqual(sorted(k["abs"] for k in lines), [(0, 39, 240, 1), (0, 79, 240, 1), (0, 105, 240, 1), (0, 441, 240, 1)])

    def test_the_paint_row_has_six_local_mode_buttons_a_blend_button_and_the_remove_button(self):
        modes = [k for k in self.picker["kids"] if k["name"].startswith("Mode ")]
        self.assertEqual([k["abs"] for k in modes], [(8 + 28 * i, 48, 24, 24) for i in range(6)])
        self.assertEqual([k["fill"] if k.get("fill") else None for k in modes], ["color/bg-secondary"] + [None] * 5)
        self.assertEqual([i["icon"] for k in modes for i in k["kids"]], ["PaintSolid", "PaintLinear", "PaintRadial", "PaintAngular", "PaintDiamond", "PaintImage"])
        self.assertEqual([i["color"] for k in modes for i in k["kids"]], ["color/icon"] + ["color/icon-secondary"] * 5)
        self.assertEqual(part(self.picker, "Blend")["abs"], (180, 48, 24, 24))
        none = part(self.picker, "Remove paint")
        self.assertEqual((none["set"], none["props"], none["abs"]), ("Button/Icon", {"Icon": "NoPaint"}, (208, 48, 24, 24)))

    def test_the_contrast_bar_is_208_by_20_at_16_86_with_a_badge_at_its_right_end(self):
        bar = part(self.picker, "Contrast bar")
        self.assertEqual(bar["abs"], (16, 86, 208, 20))
        badge = part(bar, "Badge")
        self.assertEqual((badge["abs"], badge["radius"][1]), ((164, 0, 44, 20), 4.0))

    def test_the_three_strips_are_real_gradients_at_the_production_rects(self):
        for name, rect in (("SV square", (16, 122, 208, 208)), ("Hue strip", (52, 343, 172, 12)), ("Alpha strip", (52, 371, 172, 12))):
            self.assertEqual(part(self.picker, name)["abs"], rect, name)
        self.assertEqual([k["name"] for k in walk(self.picker) if "gradient" in k["name"]], [], "no frame is named a framework gap any more")

    def test_the_eyedropper_is_the_icon_button_between_the_two_strips(self):
        eye = part(self.picker, "Eyedropper")
        self.assertEqual((eye["set"], eye["props"], eye["abs"]), ("Button/Icon", {"Icon": "Eyedropper"}, (16, 351, 24, 24)))

    def test_the_fields_row_is_a_format_dropdown_and_one_grouped_hex_and_opacity_field(self):
        fmt, group = part(self.picker, "Format"), part(self.picker, "Hex field")
        self.assertEqual((fmt["abs"], fmt["fill"], fmt["stroke"]), ((16, 397, 55, 24), "color/bg", "color/border"))
        self.assertEqual((group["abs"], group.get("fill"), group.get("radius")), ((80, 397, 144, 24), None, None))
        self.assertEqual([(k["fill"], k["radius"]) for k in (part(group, "Hex cell"), part(group, "Opacity cell"))],
                         [("color/bg-secondary", ["radius/control", None, None, "radius/control"]), ("color/bg-secondary", [None, "radius/control", "radius/control", None])])
        self.assertEqual([k["w"] for k in group["kids"]], [89, 1, 54])
        self.assertEqual([t["text"] for t in walk(group) if t["t"] == "T"], ["FFFFFF", "100", "%"])

    def test_the_hex_and_opacity_field_has_no_edge_at_rest(self):
        """`grouped_hex_text` (:1398-1408) and `opacity_cell` (:1424-1440) fill BG_SECONDARY and stroke only `if focused` (BORDER_SELECTED) `else if` the pointer is over the cell (BORDER): at rest
        there is no stroke, unlike `picker_dropdown` (:1288-1295), which strokes BORDER at rest. Hover and focus have no registry state: gaps."""
        group = part(self.picker, "Hex field")
        self.assertNotIn("stroke", group)
        self.assertNotIn("sw", group)
        self.assertTrue(all("stroke" not in k for k in walk(group)), "no cell of the group draws an edge either")
        self.assertEqual(part(self.picker, "Format")["stroke"], "color/border")  # the control: the dropdown does stroke BORDER at rest

    def test_the_source_dropdown_and_two_rows_of_swatches_follow_the_painters_pitch(self):
        source = part(self.picker, "Source")
        self.assertEqual(source["abs"], (16, 450, 208, 24))
        swatches = [k for k in self.picker["kids"] if k["name"].startswith("Swatch ")]
        self.assertEqual(len(swatches), 11)
        self.assertEqual([k["abs"][:2] for k in swatches], [(16 + 24 * (i % 9), 482 + 24 * (i // 9)) for i in range(11)])
        self.assertEqual({(k["abs"][2], k["abs"][3], k["radius"][1]) for k in swatches}, {(16, 16, 3.0)})

    def test_swatch_color_is_not_nested_because_the_picker_never_calls_widgets_swatch(self):
        self.assertNotIn("Swatch/Color", {k.get("set") for k in walk(self.picker)})

    def test_every_style_exists(self):
        self.assertLessEqual({k["style"] for k in walk(self.picker) if k["t"] == "T"}, STYLES)


if __name__ == "__main__":
    unittest.main()
