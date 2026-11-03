"""Per-corner radii in the IR: `F(..., radius=[tl, tr, br, bl])`, each entry a Semantic name, a `lit(...)` or None (a square corner). They are written through the same path as a uniform radius:
the REST node carries `rectangleCornerRadii` (the importer reads it before `cornerRadius`, designos-fig rest_import.rs:1272), each rounded corner is bound (or allow-listed as a literal) on its own
corner key, and a square corner is neither (G2 only reads a corner that is not 0). `size_overrides` already flags a copy whose corners differ from its main's by value, per corner, as the editor
does (`record_override`: `corner_radii !=`). Synthetic plans for the IR, the real modules for the two uses (the starter banner, the grouped hex and opacity field).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_corner_radii
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import rest_ir as ir  # noqa: E402
import size_overrides as so  # noqa: E402
from rest_lower import CORNERS, Lowerer  # noqa: E402
from test_studio_ui_file_color_picker import ENV as PICKER_ENV, part as picker_part, SPEC as PICKER_SPEC  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402
from test_studio_ui_file_size_overrides_editor_rule import RING, RING_MAIN, doc, flags  # noqa: E402

REASON = "a computed radius: no variable holds it"


def lower(radius, **kw):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    rest = low.lower(ir.F("Box", dir="H", fill="color/bg", radius=radius, w=40, h=20, sx="fixed", sy="fixed", **kw), 0, 0, ptype="COMPONENT")
    return low, rest


def keys(low):
    return sorted((i["key"], i["var"]) for i in low.intents if i["key"] in CORNERS)


class LoweringTest(unittest.TestCase):
    def test_a_uniform_radius_is_unchanged(self):
        low, rest = lower("radius/control")
        self.assertEqual(rest["cornerRadius"], low.value("radius/control"))
        self.assertNotIn("rectangleCornerRadii", rest)
        self.assertEqual(keys(low), sorted((k, "radius/control") for k in CORNERS))

    def test_four_entries_write_the_rest_corner_list_in_corner_order_and_no_uniform_radius(self):
        low, rest = lower([None, "radius/control", ir.lit(5, REASON), None])
        self.assertEqual(rest["rectangleCornerRadii"], [0, low.value("radius/control"), 5.0, 0])
        self.assertNotIn("cornerRadius", rest)

    def test_each_rounded_corner_binds_its_own_key_and_a_square_corner_binds_nothing(self):
        low, _ = lower(["radius/control", None, None, "radius/control"])
        self.assertEqual(keys(low), [("bottomLeftRadius", "radius/control"), ("topLeftRadius", "radius/control")])
        self.assertEqual([a for a in low.allow if a["property"] in CORNERS], [])
        low, _ = lower([None, "radius/control", None, None])
        self.assertEqual(keys(low), [("topRightRadius", "radius/control")])
        low, _ = lower([None, None, "radius/control", None])
        self.assertEqual(keys(low), [("bottomRightRadius", "radius/control")])

    def test_a_literal_corner_is_an_allow_list_entry_with_its_reason_and_value_on_that_corner_only(self):
        low, rest = lower([ir.lit(11, REASON), ir.lit(11, REASON), None, None])
        self.assertEqual(rest["rectangleCornerRadii"], [11.0, 11.0, 0, 0])
        self.assertEqual([(a["property"], a["value"], a["reason"]) for a in low.allow if a["property"] in CORNERS],
                         [("topLeftRadius", 11.0, REASON), ("topRightRadius", 11.0, REASON)])
        self.assertEqual(keys(low), [])

    def test_names_and_literals_mix(self):
        low, rest = lower(["radius/control", ir.lit(11, REASON), None, None])
        self.assertEqual(rest["rectangleCornerRadii"], [low.value("radius/control"), 11.0, 0, 0])
        self.assertEqual(keys(low), [("topLeftRadius", "radius/control")])
        self.assertEqual([a["property"] for a in low.allow if a["property"] in CORNERS], ["topRightRadius"])

    def test_a_nested_copy_takes_its_mains_per_corner_radii_and_records_no_binding_of_its_own(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.prop_keys = {"Label": "Label#set0:0"}
        low.component("Chip", "V", ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label")], dir="H", radius=["radius/control", None, None, "radius/control"]), 0, 0)
        before = len(low.intents)
        host = low.lower(ir.F("Host", [ir.X("Copy", "Chip", "V")], dir="V", w=100, sx="fixed"), 0, 0, ptype="COMPONENT")
        self.assertEqual(host["children"][0]["rectangleCornerRadii"], [low.value("radius/control"), 0, 0, low.value("radius/control")])
        self.assertEqual([i for i in low.intents[before:] if i["sid"] == host["children"][0]["id"]], [])


class ValidationTest(unittest.TestCase):
    def test_a_list_has_four_entries(self):
        for bad in (["radius/control"] * 3, ["radius/control"] * 5, []):
            with self.assertRaises(ValueError, msg=bad):
                ir.F("Box", radius=bad)

    def test_an_entry_is_a_name_a_literal_or_none(self):
        for bad in (8, 8.0, ("x", 1.0, "r"), {"v": 1}, ""):
            with self.assertRaises(ValueError, msg=bad):
                ir.F("Box", radius=[bad, None, None, None])

    def test_a_zero_literal_corner_is_a_square_corner_written_as_none(self):
        with self.assertRaises(ValueError):
            ir.F("Box", radius=[ir.lit(0, REASON), None, None, None])

    def test_an_all_square_list_is_refused(self):
        with self.assertRaises(ValueError):
            ir.F("Box", radius=[None, None, None, None])


class RadiiDifferTest(unittest.TestCase):
    """`size_overrides._radii_differ` compares the four radii by position, as the editor's `corner_radii !=` does."""

    def differ(self, a, b):
        return so._radii_differ({"corner_radii": a}, {"corner_radii": b})

    def test_top_corners_against_all_corners_differ(self):
        self.assertTrue(self.differ([11.0, 11.0, 0.0, 0.0], [11.0] * 4))

    def test_the_same_corners_do_not(self):
        self.assertFalse(self.differ([11.0, 11.0, 0.0, 0.0], [11.0, 11.0, 0.0, 0.0]))

    def test_top_against_bottom_corners_differ_though_sum_max_and_sorted_values_agree(self):
        self.assertTrue(self.differ([0.0, 0.0, 11.0, 11.0], [11.0, 11.0, 0.0, 0.0]))

    def test_one_corner_alone_differs(self):
        for i in range(4):
            b = [8.0] * 4
            b[i] = 6.0
            self.assertTrue(self.differ([8.0] * 4, b), i)

    def test_a_missing_key_is_four_square_corners(self):
        self.assertTrue(self.differ([11.0, 11.0, 0.0, 0.0], None))
        self.assertFalse(self.differ([0.0] * 4, None))

    def test_an_instance_with_top_corners_only_is_reported_and_then_flagged_corner_radii(self):
        d = doc(ring=(16.0, 16.0))
        d["nodes"][str(RING_MAIN)]["corner_radii"] = [11.0] * 4
        d["nodes"][str(RING)]["corner_radii"] = [11.0, 11.0, 0.0, 0.0]
        out = [m for m in so.missing(d) if m["id"] == RING]
        self.assertIn("CornerRadii", out[0]["problem"])
        self.assertIn("[11.0, 11.0, 0.0, 0.0]", out[0]["problem"])
        so.record(d)
        self.assertIn("CornerRadii", flags(d, RING))

    def test_an_instance_whose_corners_equal_its_mains_needs_no_flag(self):
        d = doc(ring=(16.0, 16.0))
        d["nodes"][str(RING_MAIN)]["corner_radii"] = [11.0, 11.0, 0.0, 0.0]
        d["nodes"][str(RING)]["corner_radii"] = [11.0, 11.0, 0.0, 0.0]
        self.assertEqual([m for m in so.missing(d) if m["id"] == RING and "CornerRadii" in m["problem"]], [])


class UsesTest(unittest.TestCase):
    def test_the_starter_banner_rounds_its_top_corners_at_concentric_xl_and_one(self):
        from test_studio_ui_file_cards_templates import SPECS, build  # noqa: F401  (the starter card's builder and its specs)
        banner = [k for k in ir.walk(build("Card/StarterTemplate")) if k["name"] == "Banner"][0]
        r = banner["radius"]
        self.assertEqual([c[:2] if c else None for c in r], [("lit", 11.0), ("lit", 11.0), None, None])
        self.assertIn("concentric(theme::RADIUS_XL as f32, 1.0)", r[0][2])
        self.assertEqual(r[0][2], r[1][2])

    def test_each_cell_of_the_hex_and_opacity_field_rounds_its_outer_corners_and_the_group_has_no_fill_of_its_own(self):
        picker = PICKER_SPEC["build"]({"Kind": "Fill"}, PICKER_ENV)
        group, hex_cell, opacity = (picker_part(picker, n) for n in ("Hex field", "Hex cell", "Opacity cell"))
        self.assertEqual((hex_cell["fill"], hex_cell["radius"]), ("color/bg-secondary", ["radius/control", None, None, "radius/control"]))
        self.assertEqual((opacity["fill"], opacity["radius"]), ("color/bg-secondary", [None, "radius/control", "radius/control", None]))
        self.assertNotIn("fill", group)
        self.assertNotIn("radius", group)
        self.assertEqual(picker_part(group, "Gap")["fill"], "color/bg")


if __name__ == "__main__":
    unittest.main()
