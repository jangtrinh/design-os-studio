"""A screen's instance carries what apply_spec honours on 0.7.5 (probed): `props`, `overrides`, `slots`, and a size as `width` / `height`. A screen module writes `w` / `h`; a key it does not set is
absent from the spec, so the first screen's spec is unchanged. A slot holds instances of registry records only, checked before the engine is called.

An instance that is resized does not move its absolutely placed children (a 312 wide Popover/Panel keeps its close button at x = 240 - 32): a screen sizes an instance only where the main
places its children in flow, and says so where it does not.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_instance
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

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
ITEM = {"name": "Item 1", "component": "Menu/Item", "variant": {"Size": "Xs", "State": "Default"}, "props": {"Label": "File"}}
MENU = {"name": "Menu/Panel", "component": "Menu/Panel", "variant": None, "x": 12, "y": 44, "w": 210, "props": {"Title": "x"}, "overrides": [{"path": "Body/Item 1", "text": "File"}],
        "slots": {"Body": [ITEM, {"name": "Separator 1", "component": "Menu/Separator", "variant": {"State": "Default"}}]}}
SPEC = {"name": "Screen/EditorDesign/d-menu-main", "w": 1440, "h": 900, "state": "d-menu-main", "canvas": None, "place": [MENU]}


class InstanceTest(unittest.TestCase):
    def test_an_instance_with_no_extra_key_is_the_spec_the_first_screen_always_had(self):
        p = {"name": "Navigation/Rulers", "component": "Navigation/Rulers", "variant": None, "x": 290, "y": 32}
        self.assertEqual(screens.instance(p), {"kind": "instance", "name": "Navigation/Rulers", "component": "Navigation/Rulers", "x": 290, "y": 32})

    def test_props_overrides_and_the_size_pass_through_with_w_and_h_named_width_and_height(self):
        spec = screens.instance(dict(MENU, h=581))
        self.assertEqual((spec["props"], spec["overrides"], spec["width"], spec["height"]), ({"Title": "x"}, [{"path": "Body/Item 1", "text": "File"}], 210, 581))
        self.assertNotIn("w", spec)
        self.assertNotIn("h", spec)

    def test_a_size_set_to_none_is_not_passed(self):
        self.assertNotIn("width", screens.instance(dict(MENU, w=None)))

    def test_a_slot_holds_instance_specs_built_the_same_way_without_a_position(self):
        slot = screens.instance(MENU)["slots"]["Body"]
        self.assertEqual(slot[0], {"kind": "instance", "name": "Item 1", "component": "Menu/Item", "variant": {"Size": "Xs", "State": "Default"}, "props": {"Label": "File"}})
        self.assertEqual([s["kind"] for s in slot], ["instance", "instance"])
        self.assertNotIn("x", slot[0])

    def test_a_slot_set_to_false_stays_false_to_empty_it(self):
        self.assertIs(screens.instance(dict(MENU, slots={"Body": False}))["slots"]["Body"], False)

    def test_the_spec_is_a_copy_the_caller_can_keep_using(self):
        spec = screens.instance(MENU)
        spec["props"]["Title"] = "changed"
        spec["slots"]["Body"][0]["props"]["Label"] = "changed"
        self.assertEqual((MENU["props"]["Title"], ITEM["props"]["Label"]), ("x", "File"))

    def test_the_frame_places_the_sized_instance_with_its_slot(self):
        kid = screens.frame_spec(SPEC, 0)["children"][0]
        self.assertEqual((kid["width"], kid["x"], kid["y"], len(kid["slots"]["Body"])), (210, 12, 44, 2))


class CheckSlotsTest(unittest.TestCase):
    def test_a_slot_of_registry_instances_passes(self):
        self.assertEqual(screens.check_specs([SPEC], REGISTRY), [SPEC])

    def test_a_slot_entry_that_is_not_a_registry_record_fails_naming_the_path(self):
        bad = dict(MENU, slots={"Body": [dict(ITEM, component="Made/Up")]})
        with self.assertRaisesRegex(ValueError, "Made/Up"):
            screens.check_specs([dict(SPEC, place=[bad])], REGISTRY)

    def test_a_slot_entry_two_levels_down_is_read_too(self):
        deep = dict(ITEM, slots={"Body": [dict(ITEM, component="Not/There")]})
        with self.assertRaisesRegex(ValueError, "Not/There"):
            screens.check_specs([dict(SPEC, place=[dict(MENU, slots={"Body": [deep]})])], REGISTRY)

    def test_a_slot_that_holds_a_local_drawing_fails(self):
        for entry in ({"kind": "frame", "name": "Box"}, {"kind": "text", "name": "Label", "text": "x"}):
            with self.assertRaises(ValueError, msg=entry):
                screens.check_specs([dict(SPEC, place=[dict(MENU, slots={"Body": [entry]})])], REGISTRY)

    def test_a_slot_value_that_is_not_a_list_or_false_fails(self):
        with self.assertRaises(ValueError):
            screens.check_specs([dict(SPEC, place=[dict(MENU, slots={"Body": ITEM})])], REGISTRY)


if __name__ == "__main__":
    unittest.main()
