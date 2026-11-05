"""G3 geometry of a screen: every named widget rect of the capture's layout.json is found, by its name, on the screen's node of the same name within 1 px; the keys a screen cannot name
  are listed with
the reason. A synthetic native document and layout (no Pillow, no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_geometry
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import screen_geometry as sg  # noqa: E402


def node(i, name, parent, kids=(), at=(0.0, 0.0), size=(10.0, 10.0), kind="Frame"):
    return {"id": i, "name": name, "kind": {kind: {}}, "parent": parent, "children": list(kids), "transform": {"tx": at[0], "ty": at[1]}, "size": {"x": size[0], "y": size[1]}}


def doc():
    """Page 90 holds the screen 1 (1440 x 900) at (0, 0): the region `Navigation/LeftRail` 2 at (0, 32) holding `rail-File` 3 at (0, 56) 47 x 56 and two `dup` 4 and 5;
    the region `Navigation/TopTabBar` 6 at (0, 0) holding `tab-bar-home` 7 at (8, 4) 72 x 24."""
    nodes = [node(90, "04 Screens", None, [1], kind="Page"), node(1, "Screen/EditorDesign", 90, [2, 6], size=(1440.0, 900.0)),
             node(2, "Navigation/LeftRail", 1, [3, 4, 5], at=(0.0, 32.0), size=(49.0, 868.0), kind="Instance"), node(3, "rail-File", 2, at=(0.0, 56.0), size=(47.0, 56.0)),
             node(4, "dup", 2, at=(0.0, 200.0), size=(5.0, 5.0)), node(5, "dup", 2, at=(0.0, 300.0), size=(5.0, 5.0)),
             node(6, "Navigation/TopTabBar", 1, [7], size=(1440.0, 32.0), kind="Instance"), node(7, "tab-bar-home", 6, at=(8.0, 4.0), size=(72.0, 24.0))]
    return {"pages": [90], "nodes": {str(n["id"]): n for n in nodes}}


def layout(**named):
    return {"size": [1440, 900], "named": {k: {"region": "r", "rect": list(v), "count": 1} for k, v in named.items()}}


class RectsTest(unittest.TestCase):
    def test_a_node_rect_is_the_sum_of_its_ancestors_offsets_in_the_screen(self):
        self.assertEqual(sg.node_rects(doc(), "Screen/EditorDesign")["rail-File"], [(0.0, 88.0, 47.0, 56.0)])

    def test_a_name_two_nodes_share_has_both_rects(self):
        self.assertEqual(len(sg.node_rects(doc(), "Screen/EditorDesign")["dup"]), 2)

    def test_an_unknown_screen_fails_loudly(self):
        with self.assertRaises(LookupError):
            sg.node_rects(doc(), "Screen/Nope")


class CheckTest(unittest.TestCase):
    def check(self, **named):
        return sg.check(doc(), "Screen/EditorDesign", layout(**named))

    def test_a_rect_within_one_px_matches(self):
        r = self.check(**{"rail-File": (0, 88, 47, 56), "tab-bar-home": (8.9, 4.9, 72.9, 24)})
        self.assertEqual((r["matched"], r["mismatched"]), (["rail-File", "tab-bar-home"], []))

    def test_a_rect_two_px_off_is_a_mismatch_with_both_rects(self):
        r = self.check(**{"rail-File": (0, 90, 47, 56)})
        self.assertEqual(r["matched"], [])
        self.assertEqual(r["mismatched"][0]["key"], "rail-File")
        self.assertEqual((r["mismatched"][0]["layout"], r["mismatched"][0]["node"]), ([0, 90, 47, 56], [0.0, 88.0, 47.0, 56.0]))

    def test_each_edge_and_the_size_is_checked(self):
        for rect in ((2, 88, 47, 56), (0, 88, 49, 56), (0, 88, 47, 58)):
            with self.subTest(rect=rect):
                self.assertEqual(len(self.check(**{"rail-File": rect})["mismatched"]), 1)

    def test_a_name_with_several_nodes_matches_when_one_of_them_does(self):
        self.assertEqual(self.check(dup=(0, 332, 5, 5))["matched"], ["dup"])
        self.assertEqual(self.check(dup=(0, 400, 5, 5))["mismatched"][0]["key"], "dup")

    def test_a_key_with_no_node_is_not_named_with_the_reason(self):
        r = self.check(nothing=(10, 10, 20, 20))
        self.assertEqual(r["not_named"], [{"key": "nothing", "reason": sg.NO_NODE}])

    def test_the_reasons_a_screen_cannot_name_a_key(self):
        keys = {"tab-bar-brand": (0, 0, 0, 0), "tool-chevron-hit-0": (504, 848, 24, 32), "left-panel-resize-band": (278, 32, 15, 868), "folder:Group/One": (1207, 281, 223, 32),
                "style:12:3": (1207, 313, 223, 32), "far-away": (1207, 3714, 223, 32), "pages-layers-separator": (49, 301, 240, 6)}
        got = {n["key"]: n["reason"] for n in self.check(**keys)["not_named"]}
        self.assertEqual(got, {"tab-bar-brand": sg.ZERO_SIZE, "tool-chevron-hit-0": sg.HIT_ONLY, "left-panel-resize-band": sg.HIT_ONLY, "folder:Group/One": sg.KIT_NAMED,
                               "style:12:3": sg.KIT_NAMED, "far-away": sg.OFF_WINDOW, "pages-layers-separator": sg.HIT_ONLY})

    def test_two_keys_on_one_rect_are_one_widget(self):
        r = self.check(**{"rail-File": (0, 88, 47, 56), "rail-File-again": (0, 88, 47, 56)})
        self.assertEqual(r["matched"], ["rail-File"])
        self.assertEqual(r["not_named"], [{"key": "rail-File-again", "reason": sg.SAME_RECT + "rail-File"}])

    def test_the_twin_may_come_later_in_the_layout(self):
        r = self.check(**{"rail-File-first": (0, 88, 47, 56), "rail-File": (0, 88, 47, 56)})
        self.assertEqual(r["matched"], ["rail-File"])
        self.assertEqual(r["not_named"], [{"key": "rail-File-first", "reason": sg.SAME_RECT + "rail-File"}])

    def test_an_alias_names_a_widget_by_its_path_below_the_screen(self):
        d = doc()
        r = sg.check(d, "Screen/EditorDesign", layout(**{"layers-search": (0, 88, 47, 56)}), aliases={"layers-search": "Navigation/LeftRail > rail-File"})
        self.assertEqual(r["matched"], ["layers-search"])

    def test_an_alias_to_a_missing_path_fails_loudly(self):
        with self.assertRaises(LookupError):
            sg.check(doc(), "Screen/EditorDesign", layout(a=(0, 0, 1, 1)), aliases={"a": "Navigation/LeftRail > missing"})

    def test_the_counts_add_up(self):
        r = self.check(**{"rail-File": (0, 88, 47, 56), "tab-bar-home": (8, 4, 72, 24), "nothing": (1, 1, 1, 1)})
        self.assertEqual((len(r["matched"]), len(r["mismatched"]), len(r["not_named"]), r["keys"]), (2, 0, 1, 3))


if __name__ == "__main__":
    unittest.main()
