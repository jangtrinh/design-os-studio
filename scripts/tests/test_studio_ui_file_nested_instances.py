"""`X(name, set, variant, props=...)`: a nested instance of a variant that has children.

The lowerer emits an INSTANCE with the main's own frame fields, no children (the importer expands it from its main) and the property values the module
gave, keyed by the nested set's `Name#id` keys. It records no bind intent and no allow-list entry for the instance or its children: the post-pass mirrors
the main's bindings (mirror_sources), so a child can never bind differently from its main. A childless main (a focus ring) keeps its re-lowered path.
Synthetic plan only (no MCP, registry or committed file).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_instances
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

PROPS = {"Label": ("Label#set0:0", "TEXT"), "Has Icon": ("Has Icon#set0:1", "BOOLEAN"), "Icon": ("Icon#set0:2", "INSTANCE_SWAP")}


def lowerer():
    """A Lowerer with one lowered main `Chip / V` (a hugging row: an icon, a label) and its property keys registered."""
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {k: v[0] for k, v in PROPS.items()}
    low.set_props["Chip"] = PROPS
    main = ir.F("Chip", [ir.I("Icon", "Plus", 16, "color/line", swap="Icon", visible_prop="Has Icon", hidden=True), ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label")],
                dir="H", gap="space/fact", pad=[None, "inset/button", None, "inset/button"], fill="color/bg", radius="radius/control", h=32, sx="hug", sy="fixed")
    low.component("Chip", "V", main, 0, 0)
    low.intents.clear()
    return low


def nest(low, **kw):
    node = ir.F("Host", [ir.X("Action", "Chip", "V", **kw)], dir="V", w=200, sx="fixed")
    return low.lower(node, 0, 0, ptype="COMPONENT")["children"][0]


class IrTest(unittest.TestCase):
    def test_x_keeps_props_and_defaults_to_none(self):
        self.assertEqual(ir.X("A", "S", "V", abs=(0, 0, 1, 1))["props"], {})
        self.assertEqual(ir.X("A", "S", "V", props={"Label": "Hi"})["props"], {"Label": "Hi"})

    def test_props_must_be_a_dict(self):
        with self.assertRaises(ValueError):
            ir.X("A", "S", "V", props=["Label"])

    def test_estimate_of_a_nested_instance_is_its_mains_size(self):
        low = lowerer()
        node = ir.X("Action", "Chip", "V")
        self.assertEqual(ir.estimate(node, mains=low.main_sizes), low.main_sizes[("Chip", "V")])
        self.assertEqual(ir.estimate(ir.F("Host", [node], dir="V"), mains=low.main_sizes)[1], 32)

    def test_estimate_of_a_nested_instance_without_a_main_fails_loudly(self):
        with self.assertRaises(KeyError):
            ir.estimate(ir.X("Action", "Chip", "V"))

    def test_an_absolute_instance_still_estimates_from_abs(self):
        self.assertEqual(ir.estimate(ir.X("Ring", "R", "V", abs=(-4, -4, 48, 48))), (48, 48))


class LoweringTest(unittest.TestCase):
    def test_the_instance_has_the_mains_id_and_no_children(self):
        low = lowerer()
        rest = nest(low)
        self.assertEqual((rest["type"], rest["componentId"], "children" in rest), ("INSTANCE", low.main_ids[("Chip", "V")], False))

    def test_the_instance_has_the_mains_frame_fields_and_its_own_sizing_in_the_parent(self):
        """The copy keeps the main's frame fields (gap, padding, radius) and writes its own sizing where it asks for another: Fill is `layout_child` Fill with own Fixed (a Fill copy that kept the
        main's own Hug is re-hugged by the engine), Hug is `layout_child` Fixed with own Hug (`set_sizing`, properties.rs:1107-1130). The main hugs its width and fixes its height."""
        rest = nest(lowerer(), sx="fill", sy="hug")
        self.assertEqual((rest["layoutMode"], rest["itemSpacing"], rest["paddingLeft"], rest["cornerRadius"], rest["primaryAxisSizingMode"], rest["counterAxisSizingMode"]),
                         ("HORIZONTAL", 4.0, 12.0, 8.0, "FIXED", "AUTO"))
        self.assertEqual((rest["layoutSizingHorizontal"], rest["layoutSizingVertical"]), ("FILL", "FIXED"))

    def test_text_boolean_and_swap_values_are_keyed_by_the_sets_keys(self):
        rest = nest(lowerer(), props={"Label": "Review", "Has Icon": True, "Icon": "Search"})
        self.assertEqual(rest["componentProperties"], {"Label#set0:0": {"type": "TEXT", "value": "Review"}, "Has Icon#set0:1": {"type": "BOOLEAN", "value": True},
                                                       "Icon#set0:2": {"type": "INSTANCE_SWAP", "value": "icon:0"}})

    def test_no_props_means_no_component_properties(self):
        self.assertNotIn("componentProperties", nest(lowerer()))

    def test_an_unknown_property_fails_with_the_known_ones(self):
        with self.assertRaises(KeyError) as e:
            nest(lowerer(), props={"Title": "x"})
        self.assertIn("Label", str(e.exception))

    def test_a_value_of_the_wrong_type_fails(self):
        for bad in ({"Label": True}, {"Has Icon": "yes"}, {"Icon": 3}):
            with self.assertRaises(ValueError, msg=str(bad)):
                nest(lowerer(), props=bad)

    def test_the_instance_and_its_children_record_no_intent_and_no_allow_entry(self):
        low = lowerer()
        low.allow.clear()
        nest(low, props={"Label": "Review"})
        self.assertEqual(low.intents, [])
        self.assertEqual(low.allow, [])

    def test_a_literal_on_the_main_root_records_no_waiver_for_the_instance(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Chip", "V", ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line")], dir="H", pad=[None, None, None, ir.lit(4, "why")], fill="color/bg"), 0, 0)
        before = len(low.allow)
        low.lower(ir.F("Host", [ir.X("Action", "Chip", "V")], dir="V"), 0, 0)
        self.assertEqual(len(low.allow), before)

    def test_a_childless_main_keeps_the_re_lowered_ring_path(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Ring", "Tone=Brand", ir.F("Focus", [], dir="H", fill="color/bg", radius="radius/control", w=48, h=48, sx="fixed", sy="fixed"), 0, 0)
        low.intents.clear()
        rest = low.lower(ir.F("Host", [ir.X("Ring", "Ring", "Tone=Brand", abs=(-4, -4, 48, 48))], dir="H"), 0, 0)["children"][0]
        self.assertEqual((rest["type"], rest["componentId"], rest["layoutPositioning"]), ("INSTANCE", low.main_ids[("Ring", "Tone=Brand")], "ABSOLUTE"))
        self.assertEqual({i["key"] for i in low.intents}, {"fills/0", "topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"})

    def test_a_childless_main_without_abs_fails_with_the_fix(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Ring", "Tone=Brand", ir.F("Focus", [], fill="color/bg", w=48, h=48), 0, 0)
        with self.assertRaises(ValueError) as e:
            low.lower(ir.F("Host", [ir.X("Ring", "Ring", "Tone=Brand")], dir="H"), 0, 0)
        self.assertIn("abs", str(e.exception))

    def test_props_on_a_childless_main_fail(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Ring", "Tone=Brand", ir.F("Focus", [], fill="color/bg", w=48, h=48), 0, 0)
        with self.assertRaises(ValueError):
            low.lower(ir.F("Host", [ir.X("Ring", "Ring", "Tone=Brand", props={"Label": "x"}, abs=(0, 0, 1, 1))], dir="H"), 0, 0)

    def test_the_instance_is_as_big_as_its_main_before_layout(self):
        low = lowerer()
        rest = nest(low)
        box = rest["absoluteBoundingBox"]
        self.assertEqual((box["width"], box["height"]), low.main_sizes[("Chip", "V")])


if __name__ == "__main__":
    unittest.main()
