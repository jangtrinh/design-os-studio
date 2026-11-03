"""Two component modules may fill one page row: W1b's `forms` and W1c's `forms_fields` both belong to `Forms & Inputs`. The assembler groups sets
by their `row`, never by module, so both land in one section in module order (dependencies first), with every label, set and ring id present.
A characterisation of what the framework already does, so a later change to `discover` or `build_core` cannot drop it unseen.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_two_modules_one_row
"""
import importlib
import pathlib
import sys
import tempfile
import textwrap
import unittest
from unittest import mock

HERE = pathlib.Path(__file__).resolve().parent
PKG = HERE.parent / "studio_ui_file"
sys.path.insert(0, str(PKG))
import components  # noqa: E402
import icon_lib  # noqa: E402
import library_core  # noqa: E402
import text_styles_plan as styles_mod  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from rest_ir import F  # noqa: E402

REPO = HERE.parents[1]
ROW = "Forms & Inputs"


def box(values, env):
    return F("Box", fill=env.c("BG"), w=16, h=16, sx="fixed", sy="fixed")


def spec(name, axes, **kw):
    return {"name": name, "row": ROW, "axes": axes, "build": box, **kw}


STATE = {"State": ["Default", "Hover", "Focus", "Disabled"]}
SIZE_STATE = {"Size": ["Xs", "Sm"], **STATE}


def two_module_specs(second_uses_first=False, reverse=False):
    a = dict(spec("Checkbox/Box", {"State": ["Selected", "Default", "Hover", "Focus", "Disabled"]}), module="forms")
    b = dict(spec("Input/Field", SIZE_STATE, **({"uses": ["Checkbox/Box"]} if second_uses_first else {})), module="forms_fields")
    return [b, a] if reverse else [a, b]


class BuildCoreTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = plan_mod.build_plan(str(REPO))
        cls.styles, _ = styles_mod.build(cls.plan)
        cls.icons = icon_lib.IconLib(str(REPO))

    def core(self, specs):
        with mock.patch.object(components, "discover", return_value=specs):
            return library_core.build_core(str(REPO), self.plan, self.styles, self.icons)

    def test_the_sets_of_two_modules_share_one_section(self):
        core = self.core(two_module_specs())
        self.assertEqual([c["name"] for c in core["children"]], [ROW])
        self.assertEqual([s["name"] for s in core["info"]], ["Checkbox/Box", "Input/Field"])
        self.assertEqual({s["section"] for s in core["info"]}, {f"sec:{ROW}"})
        section = core["children"][0]
        names = [k["name"] for k in section["children"]]
        self.assertEqual(names, ["Section title", "Label Checkbox/Box", "Checkbox/Box", "Label Input/Field", "Input/Field"])
        self.assertEqual(len(core["componentSets"]), 2)

    def test_the_second_set_sits_below_the_first_in_the_same_section(self):
        sets = [k for k in self.core(two_module_specs())["children"][0]["children"] if k["type"] == "COMPONENT_SET"]
        first, second = (s["absoluteBoundingBox"] for s in sets)
        self.assertGreater(second["y"], first["y"] + first["height"])
        self.assertEqual(second["x"], first["x"])

    def test_the_order_of_the_modules_is_the_order_of_the_sets(self):
        self.assertEqual([s["name"] for s in self.core(two_module_specs(reverse=True))["info"]], ["Input/Field", "Checkbox/Box"])

    def test_a_set_may_use_a_set_of_the_other_module(self):
        """A dependency across modules is built first (the assembler orders by `uses`, then module, then declaration)."""
        specs = two_module_specs(second_uses_first=True)
        self.assertEqual([s["name"] for s in self.core(specs)["info"]], ["Checkbox/Box", "Input/Field"])


class DiscoverTest(unittest.TestCase):
    """The real `discover` over two real module files placed in the package folder's search path."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        components.__path__.append(self.dir.name)
        self.addCleanup(lambda: components.__path__.remove(self.dir.name))
        for mod in ("zz_forms_a", "zz_forms_b"):
            self.addCleanup(lambda m=mod: sys.modules.pop(f"components.{m}", None))
        importlib.invalidate_caches()

    def write(self, mod, body):
        (pathlib.Path(self.dir.name) / f"{mod}.py").write_text(textwrap.dedent(body))

    def test_two_modules_in_one_row_are_both_discovered_in_module_order(self):
        self.write("zz_forms_a", f"""
            def component_sets():
                return [{{"name": "ZZ/A", "row": "{ROW}", "axes": {{"State": ["Default"]}}, "build": lambda v, e: None}}]
        """)
        self.write("zz_forms_b", f"""
            def component_sets():
                return [{{"name": "ZZ/B", "row": "{ROW}", "axes": {{"State": ["Default"]}}, "uses": ["ZZ/A"], "build": lambda v, e: None}}]
        """)
        mine = [s for s in components.discover() if s["name"].startswith("ZZ/")]
        self.assertEqual([(s["name"], s["module"], s["row"]) for s in mine], [("ZZ/A", "zz_forms_a", ROW), ("ZZ/B", "zz_forms_b", ROW)])

    def test_the_same_set_in_two_modules_is_refused(self):
        body = f"""
            def component_sets():
                return [{{"name": "ZZ/A", "row": "{ROW}", "axes": {{"State": ["Default"]}}, "build": lambda v, e: None}}]
        """
        self.write("zz_forms_a", body)
        self.write("zz_forms_b", body)
        with self.assertRaisesRegex(ValueError, "defined twice"):
            components.discover()


if __name__ == "__main__":
    unittest.main()
