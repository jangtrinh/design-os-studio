"""A state screen is named `Screen/<registry screen>/<case>`: the registry screen record the capture shows, then the capture's file name. A bare registry screen name stays valid
(`Screen/EditorDesign`). Both the generator (`screens.check_specs`) and the file gate (G3 screens) apply the one rule `screen_record`; a duplicate full name fails on both sides.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_state_names
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from studio_ui_gate_screens import check_screens, screen_record  # noqa: E402
from test_studio_ui_gate_screens import REGISTRY as GATE_REGISTRY, doc  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
CATEGORY = {r["name"]: r.get("category", "component") for r in REGISTRY["components"]}
SPEC = {"name": "Screen/Home/h-home", "w": 1440, "h": 900, "state": "h-home", "canvas": None, "place": [{"name": "Panel/HomeMain", "component": "Panel/HomeMain", "variant": None, "x": 264, "y": 32}]}


class ScreenRecordTest(unittest.TestCase):
    def test_a_bare_registry_screen_name_is_its_own_record_with_no_case(self):
        self.assertEqual(screen_record("Screen/EditorDesign", CATEGORY), ("Screen/EditorDesign", None))

    def test_a_state_name_is_a_registry_screen_and_a_capture_case(self):
        for name, want in (("Screen/Home/h-home", ("Screen/Home", "h-home")), ("Screen/EditorDev/g-dev-react", ("Screen/EditorDev", "g-dev-react")),
                           ("Screen/EditorDesign/d-menu-main", ("Screen/EditorDesign", "d-menu-main")), ("Screen/Present/x-present", ("Screen/Present", "x-present"))):
            self.assertEqual(screen_record(name, CATEGORY), want, name)

    def test_every_other_name_has_no_record(self):
        for name in ("Screen/Panels/j-find", "Screen/Home/", "Screen/Home/H_Home", "Screen/Home/h home", "Screen/Home/h-home/extra", "Screen/Home/-x", "Screen/Home/x-",
                     "Navigation/LeftRail", "Navigation/LeftRail/d-menu-main", "Screen/Made up", "Screen//h-home", "Screen/Home/h--home", ""):
            self.assertIsNone(screen_record(name, CATEGORY), name)


class CheckSpecsTest(unittest.TestCase):
    def test_a_state_screen_whose_state_is_its_case_passes(self):
        self.assertEqual(screens.check_specs([SPEC], REGISTRY), [SPEC])

    def test_a_state_screen_whose_state_is_another_capture_fails(self):
        with self.assertRaisesRegex(ValueError, "state"):
            screens.check_specs([dict(SPEC, state="h-home-empty")], REGISTRY)

    def test_a_flow_that_is_not_a_registry_screen_fails(self):
        for name in ("Screen/Panels/h-home", "Navigation/LeftRail/h-home", "Screen/Home/H_HOME"):
            with self.assertRaises(ValueError, msg=name):
                screens.check_specs([dict(SPEC, name=name)], REGISTRY)

    def test_two_cases_of_one_screen_are_two_screens_and_one_case_twice_is_a_duplicate(self):
        other = dict(SPEC, name="Screen/Home/h-home-empty", state="h-home-empty")
        self.assertEqual(len(screens.check_specs([SPEC, other], REGISTRY)), 2)
        import types
        mod = types.SimpleNamespace(screens=lambda: [SPEC])
        orig = screens.importlib.import_module, screens.pkgutil.iter_modules
        screens.importlib.import_module = lambda name: mod
        screens.pkgutil.iter_modules = lambda path: [types.SimpleNamespace(name="a"), types.SimpleNamespace(name="b")]
        try:
            with self.assertRaisesRegex(ValueError, "twice"):
                screens.discover()
        finally:
            screens.importlib.import_module, screens.pkgutil.iter_modules = orig


class GateTest(unittest.TestCase):
    def registry(self):
        return {"components": GATE_REGISTRY["components"] + [{"name": "Screen/Home", "category": "screen"}]}

    def test_a_state_frame_and_a_bare_frame_pass_the_gate(self):
        for name in ("Screen/EditorDesign", "Screen/EditorDesign/d-menu-main", "Screen/Home/h-home"):
            self.assertEqual(check_screens(doc(screen_name=name), self.registry(), scrims={})[0], [], name)

    def test_a_frame_named_after_a_flow_that_is_not_a_registry_screen_fails(self):
        for name in ("Screen/Panels/j-find", "Screen/EditorDesign/D_MENU", "Navigation/LeftRail/d-menu-main", "Screen/EditorDesign/"):
            fails, _ = check_screens(doc(screen_name=name), self.registry(), scrims={})
            self.assertTrue(any("not a `screen` record" in f for f in fails), (name, fails))

    def test_two_frames_with_one_full_name_fail_and_two_cases_of_one_screen_pass(self):
        d = doc()
        d["nodes"]["4"]["children"] = [10, 11]
        d["nodes"]["11"] = dict(d["nodes"]["10"], id=11, children=[21])
        fails, _ = check_screens(d, self.registry(), scrims={})
        self.assertTrue(any("twice" in f for f in fails), fails)
        d["nodes"]["10"]["name"], d["nodes"]["11"]["name"] = "Screen/EditorDesign/d-menu-main", "Screen/EditorDesign/d-menu-zoom"
        self.assertEqual(check_screens(d, self.registry(), scrims={})[0], [])


if __name__ == "__main__":
    unittest.main()
