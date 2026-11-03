"""Native Slot properties (S5): a SLOT property definition on the set, and `F(..., slot="Body")` marking the container frame whose children are the default content.

designos-mcp 0.7.5 keeps both through the REST import (`isSlot` on the frame, `componentPropertyReferences.slotContent`, a SLOT `componentPropertyDefinitions` entry): the component
holds a `Slot` property with an empty default, the container is `is_slot` and bound with `SlotContent`, and an instance expands the neutral default content with `source` links.
Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_slot_props
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import library_core as core  # noqa: E402
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

PROPS = [{"name": "Title", "type": "TEXT", "default": "Card title"}, {"name": "Body", "type": "SLOT"}]


def card(values, env):
    body = ir.F("Body", [ir.T("Neutral content", "Content", "Body / Medium", "color/line")], dir="V", slot="Body", sx="fill")
    return ir.F("Card", [ir.T("Title", "Card title", "Body / Medium", "color/line", prop="Title"), body], dir="V", fill="color/bg", w=200, sx="fixed")


def lower(build=card, props=PROPS):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    spec = {"name": "Card/Library", "row": "Cards", "axes": {}, "props": props, "build": build}
    node, comps, info = core.lower_set(low, None, spec, 0, 0, 0)
    return low, node, info


class SlotTest(unittest.TestCase):
    def test_the_set_defines_a_slot_property_with_no_default_content_of_its_own(self):
        _, node, _ = lower()
        self.assertEqual(node["componentPropertyDefinitions"]["Body#set0:1"], {"type": "SLOT", "defaultValue": ""})

    def test_the_container_frame_is_marked_and_bound_to_the_property(self):
        _, node, _ = lower()
        body = next(c for c in node["children"] if c["name"] == "Body")
        self.assertEqual((body["isSlot"], body["componentPropertyReferences"]), (True, {"slotContent": "Body#set0:1"}))

    def test_the_default_content_stays_in_the_container(self):
        _, node, _ = lower()
        body = next(c for c in node["children"] if c["name"] == "Body")
        self.assertEqual([(c["name"], c["characters"]) for c in body["children"]], [("Neutral content", "Content")])

    def test_a_frame_that_is_no_slot_carries_no_slot_marks(self):
        _, node, _ = lower()
        title = next(c for c in node["children"] if c["name"] == "Title")
        self.assertNotIn("isSlot", title)
        self.assertEqual(title["componentPropertyReferences"], {"characters": "Title#set0:0"})

    def test_a_slot_the_set_does_not_define_fails_loudly(self):
        with self.assertRaises(KeyError) as e:
            lower(props=PROPS[:1])
        self.assertIn("Body", str(e.exception))

    def test_a_slot_frame_without_the_marker_is_no_slot(self):
        plain = lambda v, env: ir.F("Card", [ir.F("Body", [], dir="V")], dir="V", fill="color/bg")  # noqa: E731
        _, node, _ = lower(build=plain)
        self.assertNotIn("isSlot", next(c for c in node["children"] if c["name"] == "Body"))

    def test_an_instance_cannot_fill_a_slot_through_props(self):
        low, _, _ = lower()
        with self.assertRaises(ValueError) as e:
            low.lower(ir.F("Host", [ir.X("Inner", "Card/Library", "", props={"Body": "x"})], dir="V"), 0, 0)
        self.assertIn("SLOT", str(e.exception))

    def test_a_nested_instance_still_takes_the_other_properties(self):
        low, _, _ = lower()
        rest = low.lower(ir.F("Host", [ir.X("Inner", "Card/Library", "", props={"Title": "Library"})], dir="V"), 0, 0)["children"][0]
        self.assertEqual(rest["componentProperties"], {"Title#set0:0": {"type": "TEXT", "value": "Library"}})
        self.assertNotIn("children", rest)

    def test_the_slot_container_text_is_measured_like_any_text(self):
        low, _, info = lower()
        self.assertEqual([c for _, c in info["variants"][0]["texts"]], ["Card title", "Content"])
        self.assertEqual([k for _, _, k in low.main_texts[("Card/Library", "")]], ["Title#set0:0", None])


if __name__ == "__main__":
    unittest.main()
