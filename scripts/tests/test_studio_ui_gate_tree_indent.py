"""G2 rule for the Tree/Row indent: a layers row at depth n is indented n * LAYER_INDENT (`indent = depth * LAYER_INDENT`, panels/layers.rs:538; `LAYER_INDENT`, theme.rs:545) and the variant
`Depth=n` of the Tree/Row set carries it: its left padding is the Semantic variable of theme::LAYER_INDENT at depth 1 and n times it (an allow-listed literal) beyond. An instance that sets its own
left padding is an ordinary unbound property and fails; so does a variant whose padding is not n * LAYER_INDENT.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_tree_indent
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import frame_kind, node, var, with_node  # noqa: E402
from test_studio_ui_gate_bound import B, BoundBase  # noqa: E402

SET, FIRST, OTHER_SET, OTHER = 60, 61, 70, 71
TREE = "02 Core/Tree/Row"
DEEP = "Size=Xs, State=Default, Depth=2"


class TreeIndentBase(BoundBase):
    """Tree/Row is a component set on 02 Core with the variant Depth=0 (id 61); LAYER_INDENT is 16."""

    def setUp(self):
        super().setUp()
        d = self.doc["nodes"]
        d[str(SET)] = node(SET, "Tree/Row", {"ComponentSet": {}}, parent=3, children=[FIRST])
        d[str(FIRST)] = node(FIRST, "Size=Xs, State=Default, Depth=0", {"Component": {}}, parent=SET)
        d[str(OTHER_SET)] = node(OTHER_SET, "List/Row", {"ComponentSet": {}}, parent=3, children=[OTHER])
        d[str(OTHER)] = node(OTHER, "Size=Xs, State=Default", {"Component": {}}, parent=OTHER_SET)
        d["3"]["children"] += [SET, OTHER_SET]
        self.doc["variables"]["p.16"] = var("p.16", "space/16", "cp", {"Number": 16.0}, kind="Number", hidden=True)
        self.doc["variables"]["s.indent"] = var("s.indent", "layout/layer-indent", "cs", {"Alias": "p.16"}, "theme::LAYER_INDENT", "Number")
        self.doc["variables"]["s.block"] = var("s.block", "space/block", "cs", {"Alias": "p.16"}, "theme::SPACE_BLOCK", "Number")  # another Semantic variable of the same value

    def variant(self, name, left, nid=62, parent=SET):
        d = self.doc["nodes"]
        d[str(nid)] = node(nid, name, frame_kind("Component", padding=[0.0, 0.0, 0.0, left]), parent=parent)
        d[str(parent)]["children"].append(nid)
        return nid

    def instance(self, padding, of=FIRST):
        n = with_node(self.doc, kind=frame_kind("Instance", padding=padding))
        n["kind"]["Instance"]["component"] = of
        return n

    def allow(self, name, value):
        return {"path": f"{TREE}/{name}", "property": "paddingLeft", "reason": "test", "value": value}

    def indent_failures(self, allow=()):
        return [f for f in self.bound(allow) if "LAYER_INDENT" in f]


class InstancePaddingTest(TreeIndentBase):
    def test_an_instance_padding_that_is_a_multiple_of_the_indent_fails_as_unbound(self):
        for left in (16.0, 32.0, 80.0):
            self.instance([0.0, 0.0, 0.0, left])
            self.assertEqual(self.bound(), [f"{B}: paddingLeft is not bound"], left)

    def test_an_instance_padding_bound_to_the_indent_variable_passes_as_a_binding(self):
        self.instance([0.0, 0.0, 0.0, 16.0])
        self.bindings(paddingLeft="s.indent")
        self.assertEqual(self.bound(), [])

    def test_an_instance_padding_bound_to_a_primitive_fails(self):
        self.instance([0.0, 0.0, 0.0, 16.0])
        self.bindings(paddingLeft="p.16")
        self.assertEqual(self.bound(), [f"{B}: paddingLeft is bound to a non-Semantic variable"])

    def test_a_zero_padding_is_no_property(self):
        self.instance([0.0, 0.0, 0.0, 0.0])
        self.assertEqual(self.bound(), [])


class VariantDepthTest(TreeIndentBase):
    def test_depth_two_with_its_product_as_a_recorded_literal_passes(self):
        self.variant(DEEP, 32.0)
        self.assertEqual(self.bound([self.allow(DEEP, 32.0)]), [])

    def test_depth_two_with_a_padding_that_is_not_two_indents_fails(self):
        for left in (16.0, 48.0, 31.0):
            self.variant(DEEP, left, nid=63)
            self.assertEqual(self.bound([self.allow(DEEP, left)]), [f"{TREE}/{DEEP}: paddingLeft stored {left:g} differs from Depth=2 x LAYER_INDENT 32"], left)
            self.doc["nodes"][str(SET)]["children"].remove(63)

    def test_depth_one_bound_to_the_indent_variable_passes(self):
        nid = self.variant("Size=Xs, State=Default, Depth=1", 16.0)
        self.doc["nodes"][str(nid)]["variables"] = {"bindings": {"paddingLeft": "s.indent"}}
        self.assertEqual(self.bound(), [])

    def test_depth_one_bound_to_another_semantic_variable_of_the_same_value_fails(self):
        nid = self.variant("Size=Xs, State=Default, Depth=1", 16.0)
        self.doc["nodes"][str(nid)]["variables"] = {"bindings": {"paddingLeft": "s.block"}}
        self.assertEqual(self.bound(), [f"{TREE}/Size=Xs, State=Default, Depth=1: paddingLeft is bound to space/block, not to the LAYER_INDENT variable"])

    def test_depth_zero_carries_no_padding(self):
        self.variant("Size=Xs, State=Default, Depth=0", 16.0, nid=64)
        self.assertEqual(len(self.indent_failures()), 1)
        self.assertIn("differs from Depth=0 x LAYER_INDENT 0", self.indent_failures()[0])

    def test_a_variant_of_another_set_is_not_held_to_the_rule(self):
        self.variant(DEEP, 8.0, nid=65, parent=OTHER_SET)
        self.assertEqual(self.indent_failures(), [])

    def test_a_tree_row_variant_without_a_depth_is_not_held_to_the_rule(self):
        self.variant("Size=Xs, State=Hover", 8.0, nid=66)
        self.assertEqual(self.indent_failures(), [])

    def test_a_node_named_tree_row_that_is_not_a_component_set_lends_no_rule(self):
        self.doc["nodes"][str(SET)]["kind"] = {"Frame": {}}
        self.variant(DEEP, 8.0, nid=67)
        self.assertEqual(self.indent_failures(), [])

    def test_the_gate_cannot_run_the_rule_without_the_indent_variable(self):
        self.variant(DEEP, 32.0, nid=68)
        del self.doc["variables"]["s.indent"]
        with self.assertRaises(gate.GateInputError):
            self.bound()


if __name__ == "__main__":
    unittest.main()
