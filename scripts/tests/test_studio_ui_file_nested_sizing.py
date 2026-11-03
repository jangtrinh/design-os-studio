"""An `X` that nests a variant with children takes its main's sizing in an auto-layout parent unless the module gives `sx`/`sy`.

The importer expands the instance from its main, so the main's `layout_child` is what the instance has until someone resizes it. An X that defaulted to hug/hug was a
Layout override the editor would record (`record_override` flags any `layout_child` difference, designos-doc/src/components.rs:482-486) and a sync of an unflagged one
resets it. A childless main (the focus ring) keeps its own default. Synthetic plan only (no MCP, registry or committed file).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_sizing
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


def lowerer(**main_sizing):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": "Label#set0:0"}
    low.component("Chip", "V", ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label")], dir="H", h=32, **main_sizing), 0, 0)
    return low


def sizing(low, parent_dir="V", **kw):
    rest = low.lower(ir.F("Host", [ir.X("Action", "Chip", "V", **kw)], dir=parent_dir, w=200, sx="fixed"), 0, 0, ptype="COMPONENT")["children"][0]
    return rest["layoutSizingHorizontal"], rest["layoutSizingVertical"]


class NestedSizingTest(unittest.TestCase):
    def test_an_x_without_sx_and_sy_takes_its_mains(self):
        self.assertEqual(sizing(lowerer(sx="hug", sy="fixed")), ("HUG", "FIXED"))
        self.assertEqual(sizing(lowerer(sx="fixed", sy="hug")), ("FIXED", "HUG"))

    def test_a_main_that_gives_none_is_hug_hug_as_before(self):
        self.assertEqual(sizing(lowerer()), ("HUG", "HUG"))

    def test_each_axis_the_module_gives_wins_over_the_mains(self):
        low = lowerer(sx="hug", sy="fixed")
        self.assertEqual(sizing(low, sx="fill"), ("FILL", "FIXED"))
        self.assertEqual(sizing(low, sy="hug"), ("HUG", "FIXED"))  # a Hug the main does not have is the editor's shape: layout_child Fixed, own sizing Hug (test_studio_ui_file_nested_own_sizing)
        self.assertEqual(sizing(low, sx="fill", sy="fill"), ("FILL", "FILL"))

    def test_the_same_in_a_horizontal_parent(self):
        self.assertEqual(sizing(lowerer(sx="hug", sy="fixed"), parent_dir="H"), ("HUG", "FIXED"))

    def test_a_childless_main_keeps_the_ring_default(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Ring", "Tone=Brand", ir.F("Focus", [], dir="H", fill="color/bg", radius="radius/control", w=48, h=48, sx="fixed", sy="fixed"), 0, 0)
        rest = low.lower(ir.F("Host", [ir.X("Ring", "Ring", "Tone=Brand", abs=(-4, -4, 48, 48))], dir="H"), 0, 0)["children"][0]
        self.assertEqual((rest["layoutSizingHorizontal"], rest["layoutSizingVertical"]), ("HUG", "HUG"))

    def test_an_icon_is_still_fixed_and_a_text_still_hugs(self):
        low = lowerer()
        host = low.lower(ir.F("Host", [ir.I("Icon", "Plus", 16, "color/line"), ir.T("Title", "Title", "Body / Medium", "color/line")], dir="H"), 0, 0)["children"]
        self.assertEqual([(c["layoutSizingHorizontal"], c["layoutSizingVertical"]) for c in host], [("FIXED", "FIXED"), ("HUG", "HUG")])


if __name__ == "__main__":
    unittest.main()
