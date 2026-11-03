"""A state screen with a modal dialog draws the app's scrim: ONE local rectangle named exactly `Scrim`, at the window's size, painted above the shell instances and under the
dialog. It is allowed only for a state whose gate data (`SCRIMS`) declares it, in `screens.check_specs`, in `screens.frame_spec` and in the file gate (G3 screens). Where the
Rust names a theme constant that a variable holds, the declaration names the variable and the Scrim's fill must be bound to one; otherwise it is a literal with the Rust line
as its reason.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_scrim
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
from studio_ui_gate_screens import check_screens  # noqa: E402
from test_studio_ui_gate_screens import REGISTRY as GATE_REGISTRY, doc  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
STATE = "h-home-dialog"
DECLARED = {STATE: {"variable": "color/bg-overlay", "reason": "shell.rs: the modal scrim"}}
LITERAL = {STATE: {"variable": None, "reason": "shell.rs:1: Color32::from_black_alpha(160)"}}
SCRIM = {"fill": "var:Semantic/color/bg-overlay", "above": "Panel/HomeMain"}
SPEC = {"name": f"Screen/Home/{STATE}", "w": 1440, "h": 900, "state": STATE, "canvas": None, "scrim": SCRIM,
        "place": [{"name": "Panel/HomeMain", "component": "Panel/HomeMain", "variant": None, "x": 264, "y": 32},
                  {"name": "Dialog/Modal", "component": "Dialog/Modal", "variant": {"Kind": "Libraries", "State": "Default"}, "x": 400, "y": 200}]}


class SpecTest(unittest.TestCase):
    def test_the_scrim_is_a_window_sized_rectangle_between_the_shell_and_the_dialog(self):
        kids = screens.frame_spec(SPEC, 0)["children"]
        self.assertEqual([(k["kind"], k["name"]) for k in kids], [("instance", "Panel/HomeMain"), ("rectangle", "Scrim"), ("instance", "Dialog/Modal")])
        self.assertEqual({k: kids[1][k] for k in ("x", "y", "width", "height", "fill")}, {"x": 0, "y": 0, "width": 1440, "height": 900, "fill": "var:Semantic/color/bg-overlay"})

    def test_a_screen_without_a_scrim_draws_none(self):
        self.assertEqual({k["name"] for k in screens.frame_spec(dict(SPEC, scrim=None), 0)["children"]}, {"Panel/HomeMain", "Dialog/Modal"})

    def test_the_scrim_needs_its_state_declared_in_the_gate_data(self):
        with self.assertRaisesRegex(ValueError, "SCRIMS"):
            screens.check_specs([SPEC], REGISTRY, scrims={})
        self.assertEqual(screens.check_specs([SPEC], REGISTRY, scrims=DECLARED), [SPEC])

    def test_the_scrim_sits_above_an_instance_the_screen_places(self):
        with self.assertRaisesRegex(ValueError, "above"):
            screens.check_specs([dict(SPEC, scrim=dict(SCRIM, above="Nothing"))], REGISTRY, scrims=DECLARED)

    def test_the_scrim_has_a_fill(self):
        with self.assertRaisesRegex(ValueError, "fill"):
            screens.check_specs([dict(SPEC, scrim={"above": "Panel/HomeMain"})], REGISTRY, scrims=DECLARED)


def with_scrim(bound=True, size=(1440, 900), at=(0, 0), extra=0):
    d = doc((21, 22, 23))
    d["nodes"]["10"]["name"], d["nodes"]["10"]["size"] = f"Screen/EditorDesign/{STATE}", {"x": 1440, "y": 900}
    node = {"id": 40, "name": "Scrim", "kind": {"Rectangle": {}}, "parent": 10, "children": [], "transform": {"tx": at[0], "ty": at[1]}, "size": {"x": size[0], "y": size[1]}}
    if bound:
        node["variables"] = {"bindings": {"fills/0": "s.overlay"}}
    d["nodes"]["40"] = node
    d["nodes"]["10"]["children"] += [40] + ([41] if extra else [])
    if extra:
        d["nodes"]["41"] = dict(node, id=41)
    return d


class GateTest(unittest.TestCase):
    def registry(self):
        return GATE_REGISTRY

    def test_a_declared_scrim_at_window_size_passes_and_is_not_a_background(self):
        fails, counts = check_screens(with_scrim(), self.registry(), scrims=DECLARED)
        self.assertEqual((fails, counts["backgrounds"]), ([], 1))

    def test_a_scrim_the_state_does_not_declare_is_local_drawing(self):
        fails, _ = check_screens(with_scrim(), self.registry(), scrims={})
        self.assertEqual(len(fails), 1)
        self.assertIn("Scrim", fails[0])
        self.assertIn("instances only", fails[0])

    def test_a_scrim_that_is_not_the_windows_size_or_position_fails(self):
        for kw in ({"size": (1440, 899)}, {"size": (1000, 900)}, {"at": (1, 0)}, {"at": (0, 32)}):
            fails, _ = check_screens(with_scrim(**kw), self.registry(), scrims=DECLARED)
            self.assertEqual(len(fails), 1, kw)
            self.assertIn("window", fails[0])

    def test_a_second_scrim_fails(self):
        fails, _ = check_screens(with_scrim(extra=1), self.registry(), scrims=DECLARED)
        self.assertTrue(any("one Scrim" in f for f in fails), fails)

    def test_a_scrim_declared_with_a_variable_must_be_bound_and_a_literal_one_must_not_claim_one(self):
        fails, _ = check_screens(with_scrim(bound=False), self.registry(), scrims=DECLARED)
        self.assertEqual(len(fails), 1)
        self.assertIn("fills/0", fails[0])
        self.assertEqual(check_screens(with_scrim(bound=False), self.registry(), scrims=LITERAL)[0], [])
        self.assertEqual(len(check_screens(with_scrim(bound=True), self.registry(), scrims=LITERAL)[0]), 1)

    def test_a_scrim_is_looked_up_by_the_frames_case_not_its_record(self):
        d = with_scrim()
        d["nodes"]["10"]["name"] = "Screen/EditorDesign"
        self.assertEqual(len(check_screens(d, self.registry(), scrims=DECLARED)[0]), 1)


if __name__ == "__main__":
    unittest.main()
