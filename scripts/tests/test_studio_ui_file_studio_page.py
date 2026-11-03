"""Page `03 Studio` and the records that have no variants.

`check_specs` takes the row from the page the registry category puts a record on (`element`/`component` -> `02 Core` rows, `composite`/`region` -> `03 Studio` rows).
`build_core` lowers page 02 first, then page 03, one section per row on each page; a record with no variants is lowered as one plain COMPONENT named after it, nestable as
`X(name, record, "")`. Real registry and plan, patched `discover`; no MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_studio_page
"""
import json
import pathlib
import sys
import unittest
from unittest import mock

HERE = pathlib.Path(__file__).resolve().parent
PKG = HERE.parent / "studio_ui_file"
sys.path.insert(0, str(PKG))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import library_core  # noqa: E402
import text_styles_plan as styles_mod  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from rest_ir import F, T, X  # noqa: E402

REPO = HERE.parents[1]
REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
ONE = {"State": ["Default"]}


def build(values, env):
    return F("Box", [T("Label", "Hello", "Body / Medium", env.c("TEXT"), prop="Label")], dir="H", fill=env.c("BG"), pad=env.c("SPACE_FACT"))


PROPS = [{"name": "Label", "type": "TEXT", "default": "Hello"}]


def spec(name, row, axes, **kw):
    return {"name": name, "row": row, "axes": axes, "build": build, "module": "t", **kw}


class RowsTest(unittest.TestCase):
    def check(self, *specs):
        return components.check_specs(list(specs), REGISTRY)

    def test_a_composite_takes_a_studio_row(self):
        self.assertEqual(len(self.check(spec("Menu/Panel", "Menus & Popovers", ONE))), 1)

    def test_a_composite_on_a_core_row_fails(self):
        with self.assertRaises(ValueError) as e:
            self.check(spec("Menu/Panel", "Menus & Overlays", ONE))
        self.assertIn("03 Studio", str(e.exception))

    def test_a_component_on_a_studio_row_fails(self):
        with self.assertRaises(ValueError):
            self.check(spec("Divider/Hairline", "Menus & Popovers", ONE))

    def test_a_screen_record_has_no_component_rows(self):
        screen = next(r["name"] for r in REGISTRY["components"] if r["category"] == "screen")
        with self.assertRaises(ValueError) as e:
            self.check(spec(screen, "Cards", {}))
        self.assertIn("04 Screens", str(e.exception))

    def test_the_two_row_lists_do_not_overlap(self):
        self.assertEqual(set(components.ROWS) & set(components.STUDIO_ROWS), set())

    def test_a_record_with_no_variants_takes_no_axes(self):
        self.assertEqual(len(self.check(spec("Card/Library", "Cards", {}))), 1)
        with self.assertRaises(ValueError):
            self.check(spec("Card/Library", "Cards", ONE))

    def test_a_record_with_variants_cannot_drop_them(self):
        with self.assertRaises(ValueError):
            self.check(spec("Menu/Panel", "Menus & Popovers", {}))


class BuildTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = plan_mod.build_plan(str(REPO))
        cls.styles, _ = styles_mod.build(cls.plan)
        cls.icons = icon_lib.IconLib(str(REPO))

    def core(self, specs):
        with mock.patch.object(components, "discover", return_value=specs):
            return library_core.build_core(str(REPO), self.plan, self.styles, self.icons)

    def mixed(self):
        return [spec("Menu/Panel", "Menus & Popovers", ONE, props=PROPS), spec("Card/Library", "Cards", {}, props=PROPS),
                spec("Divider/Hairline", "Badges & Status", ONE, props=PROPS)]

    def test_page_02_and_page_03_get_their_own_sections(self):
        core = self.core(self.mixed())
        self.assertEqual([c["name"] for c in core["children"]], ["Badges & Status"])
        self.assertEqual([c["name"] for c in core["studio"]], ["Menus & Popovers", "Cards"])
        self.assertEqual([(s["name"], s["page"]) for s in core["info"]], [("Divider/Hairline", "02 Core"), ("Menu/Panel", "03 Studio"), ("Card/Library", "03 Studio")])

    def test_sections_on_each_page_start_at_x_zero(self):
        core = self.core(self.mixed())
        self.assertEqual((core["children"][0]["absoluteBoundingBox"]["x"], core["studio"][0]["absoluteBoundingBox"]["x"]), (0, 0))
        a, b = (s["absoluteBoundingBox"] for s in core["studio"])
        self.assertEqual(b["x"], a["x"] + a["width"] + library_core.GAP)

    def test_a_plain_component_is_a_component_named_after_the_record_and_no_set(self):
        core = self.core(self.mixed())
        plain = next(k for k in core["studio"][1]["children"] if k["name"] == "Card/Library" and k["type"] == "COMPONENT")
        self.assertEqual(plain["name"], "Card/Library")
        self.assertEqual(list(plain["componentPropertyDefinitions"]), ["Label#set2:0"])
        self.assertNotIn("Card/Library", [v["name"] for v in core["componentSets"].values()])
        self.assertEqual(core["components"][plain["id"]], {"key": "studio-ui/Card/Library", "name": "Card/Library"})
        info = next(s for s in core["info"] if s["name"] == "Card/Library")
        self.assertEqual((info["plain"], info["id"], info["variants"][0]["id"], info["cols"]), (True, plain["id"], plain["id"], 1))

    def test_a_set_keeps_its_variant_properties_and_its_wrapper(self):
        core = self.core(self.mixed())
        wrapper = next(k for k in core["studio"][0]["children"] if k["type"] == "COMPONENT_SET")
        self.assertEqual(wrapper["componentPropertyDefinitions"]["State"]["type"], "VARIANT")
        self.assertNotIn("plain", next(s for s in core["info"] if s["name"] == "Menu/Panel"))

    def test_a_plain_component_can_be_nested_as_an_instance_with_the_empty_variant(self):
        host = spec("Table/Data", "Data & Code", {}, build=lambda v, env: F("Table", [X("Lib", "Card/Library", "", props={"Label": "Hi"})], dir="V"))
        core = self.core([spec("Card/Library", "Cards", {}, props=PROPS), host])
        table = next(k for k in core["studio"][1]["children"] if k["name"] == "Table/Data")
        inst = table["children"][0]
        self.assertEqual((inst["type"], inst["componentProperties"]), ("INSTANCE", {"Label#set0:0": {"type": "TEXT", "value": "Hi"}}))

    def test_a_set_that_nests_a_set_of_a_later_row_fails_loudly(self):
        host = spec("Menu/Panel", "Menus & Popovers", ONE, build=lambda v, env: F("Panel", [X("Lib", "Card/Library", "")], dir="V"))
        with self.assertRaises(KeyError) as e:
            self.core([host, spec("Card/Library", "Cards", {})])
        self.assertIn("earlier row", str(e.exception))

    def test_an_allow_list_entry_on_page_03_carries_the_page_in_its_path(self):
        lit = spec("Menu/Panel", "Menus & Popovers", ONE, build=lambda v, env: F("Panel", [], dir="H", pad=[None, None, None, __import__("rest_ir").lit(6, "r")], fill=env.c("BG")))
        core = self.core([lit])
        self.assertEqual([a["path"] for a in core["allow"]], ["03 Studio/Menus & Popovers/Menu/Panel/State=Default"])

    def test_the_rest_document_carries_page_03(self):
        import assemble_foundation as base
        doc = base.rest_document([], {}, self.core(self.mixed()))
        pages = {c["name"]: c for c in doc["document"]["children"]}
        self.assertEqual([c["name"] for c in pages["03 Studio"]["children"]], ["Menus & Popovers", "Cards"])
        self.assertEqual([c["name"] for c in pages["02 Core"]["children"]], ["Badges & Status"])


if __name__ == "__main__":
    unittest.main()
