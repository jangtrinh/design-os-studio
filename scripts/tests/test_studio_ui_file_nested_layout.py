"""Nested instances are laid out like their mains: the layout pass measures the texts that exist only after the import.

A copy is as wide as the estimate it was cloned at (0.56 x size x characters) and `designos_update_node` on a main does not sync its instances, so the layout
pass sets every copied text to a placeholder and back, as it does for a main's. The lowerer lists those texts (REST ids `I<instance>;<main node>`) with the content the
module's `props` gave, and the copies that keep their main's content, whose measured width must equal the main's. Synthetic plan and a fake MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_layout
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

LABEL = ("Label#set0:0", "TEXT")


def chip():
    return ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label"), ir.T("Fixed", "Fixed text", "Body / Medium", "color/line")],
                dir="H", fill="color/bg", h=32, sx="hug", sy="fixed")


def lowerer():
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": LABEL[0]}
    low.set_props["Chip"] = {"Label": LABEL}
    low.component("Chip", "V", chip(), 0, 0)
    return low


def host(low, name, *kids):
    low.prop_keys = {}
    return low.component(name, "V", ir.F("Host", list(kids), dir="V", w=200, sx="fixed"), 0, 0)


class NestedTextsTest(unittest.TestCase):
    def test_a_main_lists_its_own_texts_with_their_property_keys(self):
        low = lowerer()
        got = low.main_texts[("Chip", "V")]
        self.assertEqual([(c, k) for _, c, k in got], [("Label", LABEL[0]), ("Fixed text", None)])

    def test_a_nested_instance_adds_its_copied_texts_after_the_hosts_own(self):
        low = lowerer()
        rest = host(low, "Bar", ir.X("A", "Chip", "V"))
        got = low.main_texts[("Bar", "V")]
        a = rest["children"][0]["id"]
        main_ids = [sid for sid, _, _ in low.main_texts[("Chip", "V")]]
        self.assertEqual([sid for sid, _, _ in got], [f"I{a};{m}" for m in main_ids])
        self.assertEqual([c for _, c, _ in got], ["Label", "Fixed text"])

    def test_a_text_property_value_replaces_the_content_of_the_text_it_drives_only(self):
        low = lowerer()
        host(low, "Bar", ir.X("A", "Chip", "V", props={"Label": "A longer label"}))
        self.assertEqual([c for _, c, _ in low.main_texts[("Bar", "V")]], ["A longer label", "Fixed text"])

    def test_only_the_copies_that_keep_the_mains_content_are_paired_with_it(self):
        low = lowerer()
        rest = host(low, "Bar", ir.X("A", "Chip", "V"), ir.X("B", "Chip", "V", props={"Label": "Review"}))
        a, b = (k["id"] for k in rest["children"])
        main_label, main_fixed = (sid for sid, _, _ in low.main_texts[("Chip", "V")])
        self.assertEqual(low.mirror_pairs[("Bar", "V")], [(f"I{a};{main_label}", main_label), (f"I{a};{main_fixed}", main_fixed), (f"I{b};{main_fixed}", main_fixed)])

    def test_a_host_nested_in_a_host_passes_the_texts_on_with_the_inner_content(self):
        low = lowerer()
        host(low, "Bar", ir.X("A", "Chip", "V", props={"Label": "Review"}))
        outer = host(low, "Panel", ir.X("Inner", "Bar", "V"))
        inner = outer["children"][0]["id"]
        bar_texts = low.main_texts[("Bar", "V")]
        got = low.main_texts[("Panel", "V")]
        self.assertEqual([(sid, c) for sid, c, _ in got], [(f"I{inner};{sid}", c) for sid, c, _ in bar_texts])
        self.assertEqual(got[0][1], "Review")
        self.assertTrue(all(k is None for _, _, k in got))

    def test_the_layout_info_of_a_set_carries_the_nested_texts_and_pairs(self):
        low = lowerer()
        spec = {"name": "Bar", "row": "Navigation", "axes": {"State": ["Default"]}, "props": [], "build": lambda v, env: ir.F("Bar", [ir.X("A", "Chip", "V")], dir="H")}
        _, _, info = core.lower_set(low, None, spec, 1, 0, 0)
        v = info["variants"][0]
        self.assertEqual([c for _, c in v["texts"]], ["Label", "Fixed text"])
        self.assertEqual(len(v["mirrors"]), 2)
        self.assertTrue(all(sid.startswith("I") for sid, _ in v["texts"]))


class FakeMcp:
    def __init__(self, widths):
        self.widths = widths  # native id -> width

    def tool_or_die(self, tool, args):
        assert tool == "designos_get_node"
        return {"node": {"size": {"x": self.widths[args["node_id"]], "y": 20.0}}}


def info(*pairs):
    return [{"name": "Bar", "variants": [{"name": "State=Default", "mirrors": list(pairs)}]}]


class CheckNestedTest(unittest.TestCase):
    IDS = {"Ic1;c2": 11, "c2": 12, "Ic1;c3": 13, "c3": 14}

    def test_a_copy_as_wide_as_its_main_passes(self):
        self.assertEqual(layout.check_nested(FakeMcp({11: 36.75, 12: 36.75, 13: 50.0, 14: 50.3}), self.IDS, info(("Ic1;c2", "c2"), ("Ic1;c3", "c3"))), 2)

    def test_a_copy_left_at_its_estimate_is_fatal_and_named(self):
        with self.assertRaises(SystemExit) as raised:
            layout.check_nested(FakeMcp({11: 39.2, 12: 36.75}), self.IDS, info(("Ic1;c2", "c2")))
        text = str(raised.exception)
        self.assertIn("Bar / State=Default", text)
        self.assertIn("Ic1;c2", text)
        self.assertIn("39.2", text)

    def test_the_tolerance_is_half_a_pixel(self):
        self.assertEqual(layout.check_nested(FakeMcp({11: 37.25, 12: 36.75}), self.IDS, info(("Ic1;c2", "c2"))), 1)
        with self.assertRaises(SystemExit):
            layout.check_nested(FakeMcp({11: 37.3, 12: 36.75}), self.IDS, info(("Ic1;c2", "c2")))

    def test_no_pairs_checks_nothing(self):
        self.assertEqual(layout.check_nested(FakeMcp({}), {}, [{"name": "Bar", "variants": [{"name": "V"}]}]), 0)


if __name__ == "__main__":
    unittest.main()
