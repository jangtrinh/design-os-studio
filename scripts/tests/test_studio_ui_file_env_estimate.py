"""`env.estimate(node)`: the size of a tree before layout, with the sizes of the mains lowered so far, for a module that holds a nested instance.

A module cannot size a tree that holds an `X` without `abs` through the bare `rest_ir.estimate` (it has no mains), and the error it got named the row order, not the
missing argument. The Env keeps a reference to `Lowerer.main_sizes` (never a copy: the dict is empty when the Env is built and fills as sets are lowered), `buttons.with_ring`
sizes its ring through it, and no module under `components/` may call the bare estimate again. Synthetic plan only (no MCP or committed file).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_env_estimate
"""
import ast
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
COMPONENTS = HERE.parent / "studio_ui_file" / "components"
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import rest_ir as ir  # noqa: E402
from components import buttons  # noqa: E402
from components.env import Env  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

KEY = ("Chip", "V")


def chip():
    return ir.F("Chip", [ir.T("Label", "Label", "Body / Medium", "color/line", prop="Label")], dir="H", h=32, sx="hug", sy="fixed")


def lowerer():
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": "Label#set0:0"}
    return low


def row():
    return ir.F("Row", [ir.T("Title", "Title", "Body / Medium", "color/line"), ir.X("Action", "Chip", "V")], dir="H")


class EnvEstimateTest(unittest.TestCase):
    def test_a_row_with_a_nested_instance_is_the_sum_with_the_mains_size(self):
        low = lowerer()
        env = Env(str(REPO), PLAN, FakeIcons(), mains=low.main_sizes)
        low.component("Chip", "V", chip(), 0, 0)
        main_w, main_h = low.main_sizes[KEY]
        text_w, text_h = ir.estimate(ir.T("Title", "Title", "Body / Medium", "color/line"))
        self.assertEqual(env.estimate(row()), (text_w + main_w, max(text_h, main_h)))

    def test_a_main_added_after_the_env_was_built_is_seen_not_copied(self):
        low = lowerer()
        env = Env(str(REPO), PLAN, FakeIcons(), mains=low.main_sizes)
        self.assertEqual(low.main_sizes, {})
        with self.assertRaises(KeyError):
            env.estimate(ir.X("Action", "Chip", "V"))
        low.component("Chip", "V", chip(), 0, 0)
        self.assertEqual(env.estimate(ir.X("Action", "Chip", "V")), low.main_sizes[KEY])

    def test_an_env_without_mains_knows_none(self):
        env = Env(str(REPO), PLAN, FakeIcons())
        self.assertEqual(env.estimate(ir.X("Ring", "R", "V", abs=(0, 0, 8, 9))), (8, 9))
        with self.assertRaises(KeyError) as ctx:
            env.estimate(ir.X("Action", "Chip", "V"))
        self.assertIn("earlier row", str(ctx.exception))


class EstimateErrorTest(unittest.TestCase):
    def test_the_bare_estimate_without_mains_names_env_estimate(self):
        with self.assertRaises(KeyError) as ctx:
            ir.estimate(row())
        self.assertIn("env.estimate(node)", str(ctx.exception))
        self.assertNotIn("earlier row", str(ctx.exception))

    def test_an_unknown_main_in_a_given_dict_keeps_the_row_hint(self):
        for mains in ({}, {("Other", "V"): (10, 10)}):
            with self.assertRaises(KeyError) as ctx:
                ir.estimate(row(), mains=mains)
            self.assertIn("earlier row", str(ctx.exception), mains)
            self.assertNotIn("env.estimate", str(ctx.exception), mains)


class WithRingTest(unittest.TestCase):
    def test_a_ring_over_a_root_with_a_nested_instance_is_sized_from_the_mains_size(self):
        low = lowerer()
        env = Env(str(REPO), PLAN, FakeIcons(), mains=low.main_sizes)
        low.component("Chip", "V", chip(), 0, 0)
        w, h = low.main_sizes[KEY]
        root = buttons.with_ring(ir.F("Host", [ir.X("Action", "Chip", "V")], dir="H"), {"ring": "FOCUS_RING"}, env, False)
        ring = root["kids"][-1]
        self.assertEqual((ring["t"], ring["set"], ring["variant"], ring["abs"]), ("X", "Focus/Ring", "Tone=Brand", (-4, -4, w + 8, h + 8)))

    def test_a_ring_over_a_root_without_a_nested_instance_is_unchanged(self):
        env = Env(str(REPO), PLAN, FakeIcons())
        root = buttons.with_ring(ir.F("Box", [ir.I("Icon", "Plus", 16, "color/line")], dir="H", w=32, h=32, sx="fixed", sy="fixed"), {"ring": "FOCUS_RING"}, env, True)
        self.assertEqual((root["kids"][-1]["variant"], root["kids"][-1]["abs"]), ("Tone=Error", (-4, -4, 40, 40)))


def bare_estimate_uses(source):
    """Lines of `source` that reach `rest_ir.estimate` directly: `from rest_ir import estimate` (or `*`), `import rest_ir [as m]` then `m.estimate`."""
    tree, modules, lines = ast.parse(source), set(), []
    for n in ast.walk(tree):
        if isinstance(n, ast.Import):
            modules |= {a.asname or a.name for a in n.names if a.name == "rest_ir"}
        if isinstance(n, ast.ImportFrom) and n.module == "rest_ir" and any(a.name in ("estimate", "*") for a in n.names):
            lines.append(n.lineno)
    for n in ast.walk(tree):
        if isinstance(n, ast.Attribute) and n.attr == "estimate" and isinstance(n.value, ast.Name) and n.value.id in modules:
            lines.append(n.lineno)
    return sorted(lines)


class BareEstimateScanTest(unittest.TestCase):
    def test_the_scan_catches_every_way_to_reach_the_bare_estimate(self):
        self.assertEqual(bare_estimate_uses("from rest_ir import F, estimate\n"), [1])
        self.assertEqual(bare_estimate_uses("from rest_ir import *\n"), [1])
        self.assertEqual(bare_estimate_uses("import rest_ir as ir\nx = ir.estimate(n)\n"), [2])
        self.assertEqual(bare_estimate_uses("import rest_ir\nx = rest_ir.estimate(n)\n"), [2])
        self.assertEqual(bare_estimate_uses("from rest_ir import F\nx = env.estimate(n)\n"), [])

    def test_no_component_module_but_env_reaches_the_bare_estimate(self):
        modules = sorted(p for p in COMPONENTS.glob("*.py") if p.name != "env.py")
        self.assertGreater(len(modules), 5)
        offenders = {p.name: bare_estimate_uses(p.read_text(encoding="utf-8")) for p in modules}
        self.assertEqual({k: v for k, v in offenders.items() if v}, {})


if __name__ == "__main__":
    unittest.main()
