"""`X(..., sx=, sy=, w=, h=)` on a nested variant sets the copy's own sizing next to its `layout_child`, as the editor does (`set_sizing`, v0.7.5 panels/properties.rs:1107-1130):

    Fill  -> `layout_child` Fill  and the node's own `primary_sizing` / `counter_sizing` FIXED
    Fixed -> `layout_child` Fixed and both FIXED
    Hug   -> `layout_child` Fixed and own HUG

The engine re-hugs a frame whose own sizing says Hug (designos-doc `layout_apply.rs:177,192,196`), so a Fill copy of a hugging main that kept the main's own Hug stayed as wide as its label: 60.75 px
in a 241 px card, where the editor's shape fills it (216). `w=` / `h=` give the copy a fixed size on that axis. A copy that asks for what its main already has changes nothing. Synthetic plan only
(no MCP; the engine half is test_studio_ui_file_nested_fill_engine).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_own_sizing
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


def lowerer(direction="H", **main_sizing):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": "Label#set0:0"}
    low.component("Chip", "V", ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label")], dir=direction, **main_sizing), 0, 0)
    return low


def copy_of(low, parent_dir="V", **kw):
    host = low.lower(ir.F("Host", [ir.X("Action", "Chip", "V", **kw)], dir=parent_dir, w=241, sx="fixed"), 0, 0, ptype="COMPONENT")
    return host["children"][0]


def modes(rest):
    return (rest["layoutSizingHorizontal"], rest["layoutSizingVertical"], rest["primaryAxisSizingMode"], rest["counterAxisSizingMode"])


class FillTest(unittest.TestCase):
    def test_a_fill_copy_of_a_hugging_main_gets_its_own_primary_sizing_fixed(self):
        """The generator's bug: layout_child Fill with the main's own Hug, which the engine re-hugs."""
        rest = copy_of(lowerer(sx="hug", sy="fixed"), sx="fill")
        self.assertEqual(modes(rest), ("FILL", "FIXED", "FIXED", "FIXED"))

    def test_the_counter_axis_of_a_vertical_main_is_its_horizontal_sizing(self):
        rest = copy_of(lowerer("V", sx="hug", sy="hug"), sx="fill")
        self.assertEqual(modes(rest), ("FILL", "HUG", "AUTO", "FIXED"))

    def test_a_copy_that_fills_an_axis_its_main_already_fixes_changes_nothing(self):
        rest = copy_of(lowerer(sx="fixed", sy="fixed"), sx="fill")
        self.assertEqual(modes(rest), ("FILL", "FIXED", "FIXED", "FIXED"))

    def test_only_the_axis_that_differs_is_written(self):
        rest = copy_of(lowerer(sx="hug", sy="hug"), sx="fill")
        self.assertEqual((rest["primaryAxisSizingMode"], rest["counterAxisSizingMode"]), ("FIXED", "AUTO"))


class FixedAndSizeTest(unittest.TestCase):
    def test_a_fixed_copy_of_a_hugging_main_is_layout_fixed_and_own_fixed(self):
        self.assertEqual(modes(copy_of(lowerer(sx="hug", sy="hug"), sx="fixed", w=120)), ("FIXED", "HUG", "FIXED", "AUTO"))

    def test_a_width_alone_makes_the_axis_fixed_at_that_size(self):
        rest = copy_of(lowerer(sx="hug", sy="hug"), w=230)
        self.assertEqual(modes(rest), ("FIXED", "HUG", "FIXED", "AUTO"))
        self.assertEqual(rest["absoluteBoundingBox"]["width"], 230)

    def test_a_height_alone_makes_the_other_axis_fixed(self):
        rest = copy_of(lowerer(sx="hug", sy="hug"), h=40)
        self.assertEqual(modes(rest), ("HUG", "FIXED", "AUTO", "FIXED"))
        self.assertEqual(rest["absoluteBoundingBox"]["height"], 40)

    def test_a_size_goes_into_the_estimate_a_parent_places_the_next_sibling_by(self):
        low = lowerer(sx="hug", sy="hug")
        main_w, main_h = low.main_sizes[("Chip", "V")]
        self.assertEqual(ir.estimate(ir.X("A", "Chip", "V", w=230), mains=low.main_sizes), (230, main_h))
        self.assertEqual(ir.estimate(ir.X("A", "Chip", "V", h=40), mains=low.main_sizes), (main_w, 40))
        self.assertEqual(ir.estimate(ir.X("A", "Chip", "V"), mains=low.main_sizes), (main_w, main_h))

    def test_the_size_does_not_change_the_mains_estimate_or_a_sibling_copy(self):
        low = lowerer(sx="hug", sy="hug")
        before = low.main_sizes[("Chip", "V")]
        host = low.lower(ir.F("Host", [ir.X("A", "Chip", "V", w=230), ir.X("B", "Chip", "V")], dir="V", w=241, sx="fixed"), 0, 0, ptype="COMPONENT")
        self.assertEqual(low.main_sizes[("Chip", "V")], before)
        self.assertEqual(host["children"][1]["absoluteBoundingBox"]["width"], before[0])

    def test_a_width_next_to_fill_or_hug_is_a_contradiction(self):
        for sx in ("fill", "hug"):
            with self.assertRaises(ValueError, msg=sx):
                ir.X("A", "Chip", "V", sx=sx, w=100)
            with self.assertRaises(ValueError, msg=sx):
                ir.X("A", "Chip", "V", sy=sx, h=100)

    def test_a_size_is_a_positive_number(self):
        for bad in (0, -4, "230", True):
            with self.assertRaises(ValueError, msg=bad):
                ir.X("A", "Chip", "V", w=bad)


class HugTest(unittest.TestCase):
    def test_a_hug_copy_of_a_fixed_main_is_layout_fixed_and_own_hug(self):
        """`set_sizing(Hug)` writes `layout_child` Fixed and the own sizing Hug."""
        self.assertEqual(modes(copy_of(lowerer(sx="fixed", sy="fixed"), sx="hug")), ("FIXED", "FIXED", "AUTO", "FIXED"))

    def test_a_hug_copy_of_a_hugging_main_keeps_the_mains_layout_child(self):
        self.assertEqual(modes(copy_of(lowerer(sx="hug", sy="hug"), sx="hug")), ("HUG", "HUG", "AUTO", "AUTO"))


class UnchangedTest(unittest.TestCase):
    def test_a_copy_that_asks_for_nothing_keeps_every_mode_of_its_main(self):
        for main in ({"sx": "hug", "sy": "fixed"}, {"sx": "fixed", "sy": "hug"}, {}):
            with self.subTest(main=main):
                self.assertEqual(modes(copy_of(lowerer(**main))), (("HUG" if main.get("sx", "hug") == "hug" else "FIXED"), ("FIXED" if main.get("sy", "hug") == "fixed" else "HUG"),
                                                                    "AUTO" if main.get("sx", "hug") == "hug" else "FIXED", "AUTO" if main.get("sy", "hug") == "hug" else "FIXED"))

    def test_a_copy_that_asks_for_its_mains_own_modes_changes_nothing(self):
        low = lowerer(sx="hug", sy="fixed")
        self.assertEqual(modes(copy_of(low, sx="hug", sy="fixed")), modes(copy_of(low)))

    def test_the_copy_records_no_binding_of_its_own_for_its_sizing(self):
        low = lowerer(sx="hug", sy="hug")
        before = len(low.intents)
        copy_of(low, sx="fill")
        self.assertEqual(len(low.intents), before + 0)


if __name__ == "__main__":
    unittest.main()
