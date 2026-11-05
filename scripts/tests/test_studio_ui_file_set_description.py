"""A set's description is read from the node (the importer takes `symbolDescription` or `description` of the COMPONENT_SET or plain COMPONENT node, designos-fig rest_import.rs, v0.7.5 :1147-1149),
never from the `components` map, which holds only the key and the name (:627-635, :773-781). `lower_set` and `lower_plain` copy an optional spec `description` onto that node; a spec without one
carries no key. Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_set_description
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components as comps_pkg  # noqa: E402
import library_core as core  # noqa: E402
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

TEXT = "State=Disabled is the whole strip disabled: the app never draws a disabled segmented"


def build(values, env):
    return ir.F("Box", [ir.T("Label", "Label", "Body / Medium", "color/line")], dir="H", fill="color/bg")


def lower(axes, **extra):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    spec = {"name": "Tabs/Segmented", "row": "Headers & Tabs", "axes": axes, "build": build, **extra}
    return core.lower_set(low, None, spec, 0, 0, 0)


class SetDescriptionTest(unittest.TestCase):
    def test_a_set_with_a_description_lowers_to_a_component_set_node_that_carries_it(self):
        node, _, _ = lower({"State": ["Default", "Disabled"]}, description=TEXT)
        self.assertEqual((node["type"], node["description"]), ("COMPONENT_SET", TEXT))

    def test_a_plain_component_with_a_description_carries_it_on_its_node(self):
        node, _, _ = lower({}, description=TEXT)
        self.assertEqual((node["type"], node["description"]), ("COMPONENT", TEXT))

    def test_a_set_without_a_description_carries_no_key(self):
        self.assertNotIn("description", lower({"State": ["Default"]})[0])
        self.assertNotIn("description", lower({})[0])

    def test_an_empty_description_carries_no_key_either(self):
        self.assertNotIn("description", lower({"State": ["Default"]}, description="")[0])
        self.assertNotIn("description", lower({}, description="")[0])

    def test_the_components_map_does_not_change(self):
        _, with_text, _ = lower({"State": ["Default", "Disabled"]}, description=TEXT)
        _, without, _ = lower({"State": ["Default", "Disabled"]})
        self.assertEqual(with_text, without)
        self.assertTrue(all(set(v) == {"key", "name"} for v in with_text.values()))
        _, plain, _ = lower({}, description=TEXT)
        self.assertTrue(all(set(v) == {"key", "name"} for v in plain.values()))

    def test_the_variants_do_not_carry_the_sets_description(self):
        node, _, _ = lower({"State": ["Default", "Disabled"]}, description=TEXT)
        self.assertTrue(all("description" not in v for v in node["children"]))


class ModuleDescriptionsTest(unittest.TestCase):
    def specs(self):
        return {s["name"]: s for s in comps_pkg.discover()}

    def test_tabs_segmented_says_why_disabled_is_the_whole_strip(self):
        d = self.specs()["Tabs/Segmented"]["description"]
        self.assertIn("State=Disabled is the whole strip disabled", d)
        self.assertIn("state_of(.., true)", d)
        self.assertIn("widgets/kit/tabs.rs:101", d)

    def test_bar_contextual_says_it_is_a_plain_component(self):
        d = self.specs()["Bar/Contextual"]["description"]
        self.assertIn("no variants", d)
        self.assertIn("toolbar.rs", d)

    def test_a_description_is_one_line_of_text(self):
        for name in ("Tabs/Segmented", "Bar/Contextual"):
            d = self.specs()[name]["description"]
            self.assertTrue(isinstance(d, str) and d.strip() == d and "\n" not in d, name)


if __name__ == "__main__":
    unittest.main()
