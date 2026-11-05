"""Gate G3 structure: every component set on page 02 Core is a `design/component-registry.json` record and carries the
registry's variant values as a full matrix. Synthetic documents and a stub registry only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_registry
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_doc import GateInputError  # noqa: E402
from studio_ui_gate_fixtures import REGISTRY, make_doc, node  # noqa: E402
from studio_ui_gate_registry import check_registry  # noqa: E402


def variant_prop(name, options):
    return {"name": name, "kind": {"Variant": {"options": list(options)}}, "default": {"Variant": options[0]}}


def add_set(doc, nid, name, axes, names=None):
    """A ComponentSet under 02 Core with one Component child per full-matrix combination (or the given names)."""
    import itertools
    combos = names or [", ".join(f"{a}={v}" for a, v in zip(axes, c)) for c in itertools.product(*axes.values())]
    kids = list(range(nid + 1, nid + 1 + len(combos)))
    doc["nodes"][str(nid)] = node(nid, name, {"ComponentSet": {}}, parent=3, children=kids,
                                  component={"props": [variant_prop(a, v) for a, v in axes.items()]})
    for k, n in zip(kids, combos):
        doc["nodes"][str(k)] = node(k, n, {"Component": {}}, parent=nid)
    doc["nodes"]["3"]["children"].append(nid)


class RegistryTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def check(self):
        return check_registry(self.doc, REGISTRY)

    def test_the_fixture_passes(self):
        self.assertEqual(self.check()[0], [])

    def test_a_full_matrix_over_two_axes_passes(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]})
        fails, count = self.check()
        self.assertEqual((fails, count), ([], 3))  # the fixture's Divider/Hairline set and its plain Divider/Plain, and this set

    def test_a_set_that_is_not_a_registry_record_fails(self):
        self.doc["nodes"]["40"]["name"] = "Divider/Line"
        self.assertTrue(any("Divider/Line" in f and "not in the registry" in f for f in self.check()[0]), self.check())

    def test_an_extra_variant_value_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm", "Md"], "State": ["Default", "Hover"]})
        self.assertTrue(any("Button/Icon" in f and "Size" in f and "Md" in f for f in self.check()[0]), self.check())

    def test_a_missing_variant_value_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs"], "State": ["Default", "Hover"]})
        self.assertTrue(any("Button/Icon" in f and "Size" in f and "Sm" in f for f in self.check()[0]), self.check())

    def test_an_option_the_property_lists_without_a_variant_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]})
        self.doc["nodes"]["20"]["component"]["props"][0]["kind"]["Variant"]["options"].append("Md")
        self.assertTrue(any("axis Size values" in f and "differ from the registry" in f for f in self.check()[0]), self.check())

    def test_a_missing_axis_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"]})
        self.assertTrue(any("Button/Icon" in f and "State" in f for f in self.check()[0]), self.check())

    def test_a_hole_in_the_matrix_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]},
                names=["Size=Xs, State=Default", "Size=Xs, State=Hover", "Size=Sm, State=Default"])
        self.assertTrue(any("Button/Icon" in f and "Size=Sm, State=Hover" in f for f in self.check()[0]), self.check())

    def test_a_variant_name_outside_the_matrix_fails(self):
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]})
        self.doc["nodes"]["21"]["name"] = "Size=Xs, State=Pressed"
        self.assertTrue(any("Size=Xs, State=Pressed" in f for f in self.check()[0]), self.check())

    def test_a_duplicated_variant_name_fails(self):
        """The matrix is complete, plus a second variant with a name that already exists."""
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]},
                names=["Size=Xs, State=Default", "Size=Xs, State=Hover", "Size=Sm, State=Default", "Size=Sm, State=Hover",
                       "Size=Xs, State=Hover"])
        self.assertEqual([f for f in self.check()[0] if "twice" in f], ["Button/Icon: a variant name is used twice"], self.check())

    def test_a_permuted_axis_order_in_a_variant_name_fails(self):
        """`State=Hover, Size=Xs` holds the same pairs as `Size=Xs, State=Hover` but is not the name the property order produces."""
        add_set(self.doc, 20, "Button/Icon", {"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]},
                names=["Size=Xs, State=Default", "State=Hover, Size=Xs", "Size=Sm, State=Default", "Size=Sm, State=Hover"])
        fails = self.check()[0]
        self.assertTrue(any("Size=Xs, State=Hover is missing" in f for f in fails), fails)
        self.assertTrue(any("State=Hover, Size=Xs is not in the registry matrix" in f for f in fails), fails)

    def test_two_sets_with_one_name_fail(self):
        add_set(self.doc, 20, "Divider/Hairline", {"State": ["Default"]})
        self.assertTrue(any("twice" in f for f in self.check()[0]), self.check())

    def test_sets_on_other_pages_are_not_checked(self):
        self.doc["nodes"]["2"]["children"].append(30)
        self.doc["nodes"]["30"] = node(30, "Whatever", {"ComponentSet": {}}, parent=2)
        self.assertEqual(self.check()[0], [])

    def test_no_component_set_on_core_is_an_error_not_a_pass(self):
        self.doc["nodes"]["3"]["children"] = []
        with self.assertRaises(GateInputError):
            self.check()

    def test_a_registry_without_components_is_an_error(self):
        with self.assertRaises(GateInputError):
            check_registry(self.doc, {"components": []})
        with self.assertRaises(GateInputError):
            check_registry(self.doc, copy.deepcopy({"version": 1}))


if __name__ == "__main__":
    unittest.main()
