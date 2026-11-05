"""G3 screens: every frame on page `04 Screens` is a registry `screen` record, holds instances of registry records and at most one local rectangle (the canvas background), and nothing else.
An empty page is no failure (a foundation-only file has no screen). Synthetic native documents.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_screens
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_screens import BACKGROUND, check_screens  # noqa: E402

REGISTRY = {"components": [{"name": "Screen/EditorDesign", "category": "screen"}, {"name": "Navigation/LeftRail", "category": "region", "variants": ["Rail=48px"]},
                           {"name": "Navigation/TitleBar", "category": "region"}, {"name": "Button/Icon", "category": "component"}, {"name": "Divider/Plain", "category": "element"}]}


def node(i, name, kind, parent=None, kids=()):
    return {"id": i, "name": name, "kind": kind, "parent": parent, "children": list(kids)}


def doc(kids=(21, 22, 23), screen_name="Screen/EditorDesign", page="04 Screens"):
    """Page 4 holds the screen 10; the set `Navigation/LeftRail` 50 (variant 51) and the plain component `Navigation/TitleBar` 52 are on page 3; 21 and 22 are instances of them, 23 the background."""
    nodes = [node(3, "03 Studio", "Page", None, [50, 52]), node(4, page, "Page", None, [10]), node(10, screen_name, {"Frame": {}}, 4, list(kids)),
             node(50, "Navigation/LeftRail", {"ComponentSet": {}}, 3, [51]), node(51, "Rail=48px", {"Component": {}}, 50), node(52, "Navigation/TitleBar", {"Component": {}}, 3),
             node(21, "Navigation/LeftRail", {"Instance": {"component": 51}}, 10), node(22, "Navigation/TitleBar", {"Instance": {"component": 52}}, 10),
             node(23, BACKGROUND, {"Rectangle": {}}, 10), node(24, "Stray", {"Frame": {}}, 10), node(25, "Label", {"Text": {}}, 10), node(26, BACKGROUND, {"Rectangle": {}}, 10),
             node(27, "Unknown", {"Instance": {"component": 99}}, 10), node(99, "Mystery/Thing", {"Component": {}}, 3)]
    return {"pages": [3, 4], "nodes": {str(n["id"]): n for n in nodes}}


def with_copy(bound=True, styled=True):
    """The screen's instance 21 (of the variant 51) holds a copy 30 of the main's text 53, which binds `fills/0` and carries the text style `Body / Regular`;
    `bound` and `styled` say whether the copy has them."""
    d = doc((21, 22, 23))
    d["nodes"]["51"]["children"] = [53]
    d["nodes"]["53"] = dict(node(53, "Label", {"Text": {}}, 51), variables={"bindings": {"fills/0": "s.text"}}, style_refs={"text": "Body / Regular"})
    d["nodes"]["21"]["children"] = [30]
    copy = dict(node(30, "Label", {"Text": {}}, 21), component={"source": 53})
    if bound:
        copy["variables"] = {"bindings": {"fills/0": "s.text"}}
    if styled:
        copy["style_refs"] = {"text": "Body / Regular"}
    d["nodes"]["30"] = copy
    return d


class CopiesAreBoundTest(unittest.TestCase):
    """The post-pass that gives a copy its main's bindings and text style does not read page 04, so the gate does:
    a screen's copy that lacks a binding or a text style its main has is a literal."""

    def test_a_copy_that_carries_its_mains_binding_and_style_passes(self):
        self.assertEqual(check_screens(with_copy(), REGISTRY)[0], [])

    def test_a_copy_that_lacks_the_binding_fails_naming_the_key(self):
        fails, _ = check_screens(with_copy(bound=False), REGISTRY)
        self.assertEqual(len(fails), 1)
        self.assertIn("fills/0", fails[0])
        self.assertIn("04 Screens/Screen/EditorDesign/Navigation/LeftRail/Label", fails[0])

    def test_a_copy_that_lacks_the_text_style_fails_naming_it(self):
        fails, _ = check_screens(with_copy(styled=False), REGISTRY)
        self.assertEqual(len(fails), 1)
        self.assertIn("text style", fails[0])

    def test_a_binding_the_copy_holds_beyond_its_main_is_no_failure(self):
        d = with_copy()
        d["nodes"]["30"]["variables"]["bindings"]["strokes/0"] = "s.line"
        self.assertEqual(check_screens(d, REGISTRY)[0], [])

    def test_a_copy_two_levels_down_is_read_too(self):
        """The instance holds a frame copy, which holds the text copy: the walk goes below the first level."""
        d = with_copy(bound=False)
        d["nodes"]["51"]["children"], d["nodes"]["53"]["parent"] = [54], 54
        d["nodes"]["54"] = node(54, "Row", {"Frame": {}}, 51, [53])
        d["nodes"]["21"]["children"], d["nodes"]["30"]["parent"] = [31], 31
        d["nodes"]["31"] = dict(node(31, "Row", {"Frame": {}}, 21, [30]), component={"source": 54})
        fails, counts = check_screens(d, REGISTRY)
        self.assertEqual((len(fails), counts["copies"]), (1, 2))
        self.assertIn("Navigation/LeftRail/Row/Label", fails[0])

    def test_the_count_of_copies_checked_is_reported(self):
        self.assertEqual(check_screens(with_copy(), REGISTRY)[1]["copies"], 1)


class ScreensTest(unittest.TestCase):
    def test_instances_and_one_background_pass(self):
        fails, counts = check_screens(doc(), REGISTRY)
        self.assertEqual((fails, counts), ([], {"screens": 1, "instances": 2, "backgrounds": 1, "copies": 0}))

    def test_an_empty_page_is_no_failure(self):
        d = doc()
        d["nodes"]["4"]["children"] = []
        self.assertEqual(check_screens(d, REGISTRY), ([], {"screens": 0, "instances": 0, "backgrounds": 0, "copies": 0}))

    def test_a_file_without_the_page_is_no_failure(self):
        d = doc(page="05 Other")
        self.assertEqual(check_screens(d, REGISTRY)[1]["screens"], 0)

    def test_local_drawing_fails_naming_the_node(self):
        for stray in (24, 25):
            with self.subTest(stray=stray):
                fails, _ = check_screens(doc((21, stray)), REGISTRY)
                self.assertEqual(len(fails), 1)
                self.assertIn("04 Screens/Screen/EditorDesign", fails[0])
                self.assertIn("instances only", fails[0])

    def test_a_second_background_fails(self):
        fails, _ = check_screens(doc((21, 23, 26)), REGISTRY)
        self.assertEqual(len(fails), 1)
        self.assertIn("one background", fails[0])

    def test_a_rectangle_not_named_as_the_background_fails(self):
        d = doc((21, 23))
        d["nodes"]["23"]["name"] = "Box"
        self.assertEqual(len(check_screens(d, REGISTRY)[0]), 1)

    def test_a_screen_that_is_not_a_registry_screen_fails(self):
        fails, _ = check_screens(doc(screen_name="Navigation/LeftRail"), REGISTRY)
        self.assertTrue(any("not a `screen` record" in f for f in fails), fails)
        fails, _ = check_screens(doc(screen_name="Screen/Made up"), REGISTRY)
        self.assertTrue(any("not a `screen` record" in f for f in fails), fails)

    def test_an_instance_of_a_component_the_registry_lacks_fails(self):
        fails, _ = check_screens(doc((21, 27)), REGISTRY)
        self.assertEqual(len(fails), 1)
        self.assertIn("Mystery/Thing", fails[0])

    def test_a_screen_with_nothing_in_it_fails(self):
        fails, _ = check_screens(doc(()), REGISTRY)
        self.assertTrue(any("empty" in f for f in fails), fails)

    def test_a_dangling_instance_fails(self):
        d = copy.deepcopy(doc((21,)))
        d["nodes"]["21"]["kind"] = {"Instance": {"component": 777}}
        fails, _ = check_screens(d, REGISTRY)
        self.assertEqual(len(fails), 1)

    def test_screens_grouped_into_sections_pass_and_an_empty_section_fails(self):
        d = doc()
        d["nodes"]["4"]["children"] = [60]
        d["nodes"]["60"] = node(60, "Editor shell", {"Section": {}}, 4, [61, 62, 10])
        d["nodes"]["61"] = node(61, "Section title", {"Text": {}}, 60)
        d["nodes"]["62"] = node(62, "Label Screen/EditorDesign", {"Text": {}}, 60)
        d["nodes"]["10"]["parent"] = 60
        self.assertEqual(check_screens(d, REGISTRY), ([], {"screens": 1, "instances": 2, "backgrounds": 1, "copies": 0}))
        d["nodes"]["60"]["children"] = [61]
        fails, _ = check_screens(d, REGISTRY)
        self.assertEqual(len(fails), 1)
        self.assertIn("Editor shell", fails[0])


if __name__ == "__main__":
    unittest.main()
