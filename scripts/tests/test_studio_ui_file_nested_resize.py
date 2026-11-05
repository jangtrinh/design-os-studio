"""A hug main resized by `w=` / `h=` is fixed-size in its copy, in a parent with auto layout or without, and the layout pass proves it on the saved file.

A region places its parts with `abs` in a frame that has no auto layout, so the copy gets no `layoutSizing*` from a parent: its own frame sizing is what fixes it. The lowerer writes the
copy's own sizing modes (`own_sizing`), logs every resized copy, and `library_layout.check_resized` reads the measured size back, so a copy that kept hugging fails the build
instead of drawing 191 px where the module asked for 224. Synthetic plan and a fake MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_resize
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import library_core as core  # noqa: E402
import library_layout as layout  # noqa: E402
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402


def button():
    """A hugging button: auto layout, hug width, fixed height."""
    return ir.F("Btn", [ir.T("Label", "Save", "Body / Medium", "color/line")], dir="H", fill="color/bg", h=32, sx="hug", sy="fixed")


def lowerer():
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.component("Btn", "V", button(), 0, 0)
    return low


def host(low, kid, dir=None):
    return low.component("Panel", "V", ir.F("Panel", [kid], dir=dir, w=300, h=100, sx="fixed", sy="fixed"), 0, 0)["children"][0]


class OwnSizingTest(unittest.TestCase):
    def test_a_resized_copy_in_a_frame_without_auto_layout_is_fixed_on_both_axes(self):
        copy = host(lowerer(), ir.X("B", "Btn", "V", w=224, abs=(8, 8, 224, 32)))
        self.assertEqual((copy["primaryAxisSizingMode"], copy["counterAxisSizingMode"]), ("FIXED", "FIXED"))
        self.assertEqual(copy["absoluteBoundingBox"]["width"], 224)

    def test_the_same_copy_in_a_row_is_fixed_and_says_so_to_its_parent(self):
        copy = host(lowerer(), ir.X("B", "Btn", "V", w=224), dir="H")
        self.assertEqual((copy["primaryAxisSizingMode"], copy["layoutSizingHorizontal"]), ("FIXED", "FIXED"))

    def test_an_unresized_copy_keeps_the_mains_hug(self):
        copy = host(lowerer(), ir.X("B", "Btn", "V", abs=(8, 8, 60, 32)))
        self.assertEqual(copy["primaryAxisSizingMode"], "AUTO")


class ResizedLogTest(unittest.TestCase):
    def test_every_copy_with_an_own_size_is_logged_with_the_size_it_asked_for(self):
        low = lowerer()
        rest = low.component("Panel", "V", ir.F("Panel", [ir.X("A", "Btn", "V", w=224, abs=(0, 0, 224, 32)), ir.X("B", "Btn", "V", h=40, abs=(0, 40, 60, 40)),
                                                          ir.X("C", "Btn", "V", abs=(0, 90, 60, 32))], w=300, h=130, sx="fixed", sy="fixed"), 0, 0)
        a, b, _ = (k["id"] for k in rest["children"])
        self.assertEqual(low.resized[("Panel", "V")], [(a, 224, None), (b, None, 40)])

    def test_the_layout_info_of_a_set_carries_them(self):
        low = lowerer()
        spec = {"name": "Panel", "row": "Regions", "axes": {}, "props": [], "build": lambda v, env: ir.F("Panel", [ir.X("A", "Btn", "V", w=224, abs=(0, 0, 224, 32))], w=300, h=100)}
        _, _, info = core.lower_set(low, None, spec, 1, 0, 0)
        self.assertEqual([(w, h) for _, w, h in info["variants"][0]["resized"]], [(224, None)])


class FakeMcp:
    def __init__(self, sizes):
        self.sizes = sizes  # native id -> (width, height)

    def tool_or_die(self, tool, args):
        w, h = self.sizes[args["node_id"]]
        return {"node": {"size": {"x": w, "y": h}}}


def info(*resized):
    return [{"name": "Panel", "variants": [{"name": "V", "resized": list(resized)}]}]


class CheckResizedTest(unittest.TestCase):
    IDS = {"c5": 11, "c6": 12}

    def test_a_copy_at_the_size_it_asked_for_passes(self):
        self.assertEqual(layout.check_resized(FakeMcp({11: (224.0, 32.0), 12: (60.0, 40.0)}), self.IDS, info(("c5", 224, None), ("c6", None, 40))), 2)

    def test_a_copy_that_kept_hugging_is_fatal_and_named(self):
        with self.assertRaises(SystemExit) as raised:
            layout.check_resized(FakeMcp({11: (191.0, 32.0)}), self.IDS, info(("c5", 224, None)))
        text = str(raised.exception)
        self.assertIn("Panel / V", text)
        self.assertIn("c5", text)
        self.assertIn("191", text)
        self.assertIn("224", text)

    def test_the_height_is_checked_when_it_was_asked_for(self):
        with self.assertRaises(SystemExit):
            layout.check_resized(FakeMcp({12: (60.0, 32.0)}), self.IDS, info(("c6", None, 40)))

    def test_the_axis_nobody_asked_about_is_not_checked(self):
        self.assertEqual(layout.check_resized(FakeMcp({11: (224.0, 999.0)}), self.IDS, info(("c5", 224, None))), 1)

    def test_the_tolerance_is_half_a_pixel(self):
        self.assertEqual(layout.check_resized(FakeMcp({11: (224.5, 32.0)}), self.IDS, info(("c5", 224, None))), 1)
        with self.assertRaises(SystemExit):
            layout.check_resized(FakeMcp({11: (224.6, 32.0)}), self.IDS, info(("c5", 224, None)))

    def test_no_resized_copy_checks_nothing(self):
        self.assertEqual(layout.check_resized(FakeMcp({}), {}, [{"name": "Panel", "variants": [{"name": "V"}]}]), 0)


if __name__ == "__main__":
    unittest.main()
