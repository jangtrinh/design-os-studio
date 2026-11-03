"""G3 and G2 opacity on page `03 Studio`, and on a record that has no variants.

Plan S6: `element` and `component` records live on `02 Core`, `composite` and `region` records on `03 Studio`; a set on the other page fails. A record with no variants
is ONE plain Component named after the record (no variant property); a set for it, or a plain Component for a record that has variants, fails. Synthetic documents only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_registry_pages
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_doc import GateInputError  # noqa: E402
from studio_ui_gate_fixtures import GREY, REGISTRY, make_doc, node  # noqa: E402
from studio_ui_gate_opacity import check_opacity  # noqa: E402
from studio_ui_gate_registry import check_registry  # noqa: E402
from test_studio_ui_gate_registry import add_set, variant_prop  # noqa: E402

REG = {"components": REGISTRY["components"] + [
    {"name": "Menu/Panel", "category": "composite", "variants": ["State=Default"]},
    {"name": "Panel/Left", "category": "region", "variants": ["State=Default"]},
    {"name": "Card/Library", "category": "composite"},
    {"name": "Divider/Rule", "category": "element"},
    {"name": "Button/Pill", "category": "component", "variants": ["State=Default", "State=Disabled"]}]}
STUDIO = 9  # the fixture's `03 Studio` page


def put_set(doc, nid, name, page=STUDIO, states=("Default",)):
    doc["nodes"][str(nid)] = node(nid, name, {"ComponentSet": {}}, parent=page, children=[nid + 1 + i for i in range(len(states))],
                                  component={"props": [variant_prop("State", states)]})
    for i, s in enumerate(states):
        doc["nodes"][str(nid + 1 + i)] = node(nid + 1 + i, f"State={s}", {"Component": {}}, parent=nid)
    doc["nodes"][str(page)].setdefault("children", []).append(nid)


def put_plain(doc, nid, name, page=STUDIO, **extra):
    doc["nodes"][str(nid)] = node(nid, name, {"Component": {}}, parent=page, **extra)
    doc["nodes"][str(page)].setdefault("children", []).append(nid)


def fails(doc):
    return check_registry(doc, REG)[0]


class PageTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def test_a_composite_set_on_the_studio_page_passes(self):
        put_set(self.doc, 20, "Menu/Panel")
        self.assertEqual(check_registry(self.doc, REG), ([], 3))  # the fixture's set and plain component, and this set

    def test_a_region_set_on_the_studio_page_passes(self):
        put_set(self.doc, 20, "Panel/Left")
        self.assertEqual(fails(self.doc), [])

    def test_a_composite_set_on_the_core_page_fails(self):
        add_set(self.doc, 20, "Menu/Panel", {"State": ["Default"]})
        self.assertEqual(fails(self.doc), ["Menu/Panel: a record of this category belongs on 03 Studio, it is on 02 Core"])

    def test_a_component_set_on_the_studio_page_fails(self):
        put_set(self.doc, 20, "Button/Pill", states=("Default", "Disabled"))
        self.assertEqual(fails(self.doc), ["Button/Pill: a record of this category belongs on 02 Core, it is on 03 Studio"])

    def test_the_same_name_on_both_pages_fails(self):
        put_set(self.doc, 20, "Menu/Panel")
        add_set(self.doc, 30, "Menu/Panel", {"State": ["Default"]})
        self.assertTrue(any("appears twice" in f for f in fails(self.doc)), fails(self.doc))

    def test_a_set_that_is_no_record_fails_on_the_studio_page_too(self):
        put_set(self.doc, 20, "Whatever/Set")
        self.assertIn("Whatever/Set: component set is not in the registry", fails(self.doc))

    def test_a_document_with_a_set_on_neither_page_is_an_error(self):
        for pid in ("3", "9"):
            self.doc["nodes"][pid]["children"] = []
        with self.assertRaises(GateInputError) as e:
            check_registry(self.doc, REG)
        self.assertIn("02 Core or 03 Studio", str(e.exception))


class PlainComponentTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def test_a_plain_component_for_a_record_with_no_variants_passes_on_its_page(self):
        put_plain(self.doc, 20, "Card/Library")
        put_plain(self.doc, 21, "Divider/Rule", page=3)
        self.assertEqual(check_registry(self.doc, REG), ([], 4))

    def test_a_plain_component_for_a_record_that_has_variants_fails(self):
        put_plain(self.doc, 20, "Menu/Panel")
        self.assertEqual(len(fails(self.doc)), 1)
        self.assertIn("so it is a component set, not a plain component", fails(self.doc)[0])

    def test_a_set_for_a_record_with_no_variants_fails(self):
        put_set(self.doc, 20, "Card/Library")
        self.assertEqual(fails(self.doc), ["Card/Library: the registry gives it no variants, so it is one plain component, not a component set"])

    def test_a_plain_component_that_is_no_record_fails(self):
        put_plain(self.doc, 20, "Card/Invented")
        self.assertEqual(fails(self.doc), ["Card/Invented: plain component is not in the registry"])

    def test_a_plain_component_with_a_variant_property_fails(self):
        put_plain(self.doc, 20, "Card/Library", component={"props": [variant_prop("State", ["Default"])]})
        self.assertEqual(fails(self.doc), ["Card/Library: a record with no variants has no variant property"])

    def test_a_plain_component_on_the_wrong_page_fails(self):
        put_plain(self.doc, 20, "Card/Library", page=3)
        self.assertEqual(fails(self.doc), ["Card/Library: a record of this category belongs on 03 Studio, it is on 02 Core"])

    def test_a_plain_component_alone_is_a_selection(self):
        doc = make_doc()
        doc["nodes"]["3"]["children"] = []
        put_plain(doc, 20, "Card/Library")
        self.assertEqual(check_registry(doc, REG), ([], 1))


def paint(opacity=1.0):
    return {"kind": {"Solid": GREY}, "opacity": opacity, "visible": True, "blend": "Normal"}


class OpacityPagesTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def test_a_disabled_variant_on_the_studio_page_is_read_like_one_on_core(self):
        put_set(self.doc, 20, "Panel/Left", states=("Default", "Disabled"))
        self.doc["nodes"]["22"]["fills"] = [paint(1.0)]
        self.assertEqual(check_opacity(self.doc)[0], ["03 Studio/Panel/Left/State=Disabled: fills/0 has opacity 1, a Disabled variant carries 0.5 on every visible paint"])

    def test_a_plain_component_is_a_variant_that_is_not_disabled(self):
        put_plain(self.doc, 20, "Card/Library", fills=[paint(0.5)])
        self.assertEqual(check_opacity(self.doc)[0], ["03 Studio/Card/Library: fills/0 has opacity 0.5, a non-Disabled variant carries 1 on every visible paint"])

    def test_a_plain_component_with_full_opacity_passes_and_counts_as_a_variant(self):
        before = check_opacity(self.doc)[1]["variants"]
        put_plain(self.doc, 20, "Card/Library", fills=[paint(1.0)])
        fails_, counts = check_opacity(self.doc)
        self.assertEqual((fails_, counts["variants"]), ([], before + 1))

    def test_a_node_opacity_inside_a_plain_component_fails(self):
        put_plain(self.doc, 20, "Card/Library", children=[21])
        self.doc["nodes"]["21"] = node(21, "Body", "Rectangle", parent=20, opacity=0.5)
        self.assertEqual(len(check_opacity(self.doc)[0]), 1)
        self.assertIn("03 Studio/Card/Library/Body: has node opacity 0.5", check_opacity(self.doc)[0][0])


if __name__ == "__main__":
    unittest.main()
