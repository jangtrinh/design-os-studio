"""A resized copy's Fill-width text is checked against the width it should have, not against its main's.

A text with Fill sizing takes whatever width its container leaves, so a copy of a main resized by `w=` (or filled by its parent) has a text as wide as the main's text plus the
instance's growth: `check_nested` must compare that, or it refuses every resized copy of a field (Input/Search Xs, 224 -> 174: copy text 148, main text 198). Synthetic plan and a fake MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_fill_text
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


def field(fill_text=True, inner_fixed=False):
    """A fixed 224 wide row: a Fill (or hugging) hint, optionally inside a fixed-width box, and a hugging unit."""
    hint = ir.T("Hint", "Search", "Body / Medium", "color/line", sx="fill" if fill_text else "hug")
    body = ir.F("Box", [hint], dir="H", sx="fixed", w=100) if inner_fixed else ir.F("Box", [hint], dir="H", sx="fill")
    return ir.F("Field", [body, ir.T("Unit", "px", "Body / Medium", "color/line")], dir="H", w=224, sx="fixed")


def lowerer(root):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.component("Field", "V", root, 0, 0)
    return low


def host(low, name, *kids, w=300):
    return low.component(name, "V", ir.F("Host", list(kids), dir="V", w=w, sx="fixed"), 0, 0)


class FillPairsTest(unittest.TestCase):
    def test_a_copy_of_a_fill_text_is_paired_with_its_instance_and_the_mains_root(self):
        low = lowerer(field())
        rest = host(low, "Bar", ir.X("A", "Field", "V", w=174))
        inst = rest["children"][0]["id"]
        hint = low.main_texts[("Field", "V")][0][0]
        self.assertEqual(low.fill_pairs[("Bar", "V")], {f"I{inst};{hint}": (inst, low.main_ids[("Field", "V")])})

    def test_a_hugging_text_is_not_paired_because_a_resize_does_not_reach_it(self):
        low = lowerer(field(fill_text=False))
        host(low, "Bar", ir.X("A", "Field", "V", w=174))
        self.assertEqual(low.fill_pairs[("Bar", "V")], {})

    def test_a_fill_text_below_a_fixed_box_is_not_paired(self):
        low = lowerer(field(inner_fixed=True))
        host(low, "Bar", ir.X("A", "Field", "V", w=174))
        self.assertEqual(low.fill_pairs[("Bar", "V")], {})

    def test_the_hosts_fill_instance_passes_the_text_on_to_the_outer_copy(self):
        low = lowerer(field())
        host(low, "Bar", ir.X("A", "Field", "V", sx="fill"))
        outer = host(low, "Panel", ir.X("Inner", "Bar", "V", w=250))
        inst = outer["children"][0]["id"]
        (copy, (got_inst, got_root)), = low.fill_pairs[("Panel", "V")].items()
        self.assertTrue(copy.startswith(f"I{inst};I"))
        self.assertEqual((got_inst, got_root), (inst, low.main_ids[("Bar", "V")]))

    def test_the_hosts_hugging_instance_stops_the_chain(self):
        low = lowerer(field())
        host(low, "Bar", ir.X("A", "Field", "V"))
        host(low, "Panel", ir.X("Inner", "Bar", "V", w=250))
        self.assertEqual(low.fill_pairs[("Panel", "V")], {})


class InfoTest(unittest.TestCase):
    """The layout pass reads the pairs from the info of a set: a plain record (no axes) and a set with variants both carry `fills` and `resized`."""

    def info(self, axes):
        spec = {"name": "Bar", "row": "Regions", "axes": axes, "props": [], "build": lambda v, env: ir.F("Bar", [ir.X("A", "Field", "V", w=174, abs=(0, 0, 174, 24))], w=300, h=100)}
        return core.lower_set(lowerer(field()), None, spec, 1, 0, 0)[2]["variants"][0]

    def test_a_plain_record_and_a_set_both_carry_the_fills_and_the_resized_copies(self):
        for axes in ({}, {"State": ["Default"]}):
            variant = self.info(axes)
            self.assertEqual((len(variant["fills"]), [(w, h) for _, w, h in variant["resized"]]), (1, [(174, None)]), axes)


class FakeMcp:
    def __init__(self, widths):
        self.widths = widths

    def tool_or_die(self, tool, args):
        return {"node": {"size": {"x": self.widths[args["node_id"]], "y": 20.0}}}


def info(fills=None):
    variant = {"name": "State=Default", "mirrors": [("Ic5;c2", "c2")]}
    if fills is not None:
        variant["fills"] = fills
    return [{"name": "Bar", "variants": [variant]}]


class CheckFillTest(unittest.TestCase):
    IDS = {"Ic5;c2": 11, "c2": 12, "c5": 13, "c1": 14}
    FILLS = {"Ic5;c2": ("c5", "c1")}

    def widths(self, copy, main=198.0, inst=174.0, root=224.0):
        return FakeMcp({11: copy, 12: main, 13: inst, 14: root})

    def test_a_copy_that_took_the_instances_growth_passes(self):
        self.assertEqual(layout.check_nested(self.widths(148.0), self.IDS, info(self.FILLS)), 1)

    def test_the_same_copy_without_the_fill_record_is_refused(self):
        with self.assertRaises(SystemExit):
            layout.check_nested(self.widths(148.0), self.IDS, info())

    def test_a_copy_that_missed_the_growth_is_refused_and_names_the_expected_width(self):
        with self.assertRaises(SystemExit) as raised:
            layout.check_nested(self.widths(150.0), self.IDS, info(self.FILLS))
        self.assertIn("148", str(raised.exception))

    def test_a_copy_left_at_its_main_width_is_refused_when_the_instance_shrank(self):
        with self.assertRaises(SystemExit):
            layout.check_nested(self.widths(198.0), self.IDS, info(self.FILLS))

    def test_an_unresized_instance_expects_the_mains_width(self):
        self.assertEqual(layout.check_nested(self.widths(198.0, inst=224.0), self.IDS, info(self.FILLS)), 1)
        with self.assertRaises(SystemExit):
            layout.check_nested(self.widths(148.0, inst=224.0), self.IDS, info(self.FILLS))

    def test_the_tolerance_is_half_a_pixel(self):
        self.assertEqual(layout.check_nested(self.widths(148.5), self.IDS, info(self.FILLS)), 1)
        with self.assertRaises(SystemExit):
            layout.check_nested(self.widths(148.6), self.IDS, info(self.FILLS))


if __name__ == "__main__":
    unittest.main()
