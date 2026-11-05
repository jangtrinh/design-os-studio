"""Every crops.json entry with a `render_rect` names the node it is cut from (`render_node`) and how far it is trimmed from that node's edges (`render_trim`): the compare is position-blind inside a
window, so a 2 px shift of one only shows here. Synthetic native documents for the mechanics, then the committed pair and crops.json for the data, with the three shifts the W2c review made by hand.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_geometry
"""
import copy
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_geometry as geo  # noqa: E402


def node(i, name, kind, parent=None, children=(), at=(0.0, 0.0), size=(10.0, 10.0)):
    return {"id": i, "name": name, "kind": kind, "parent": parent, "children": list(children), "transform": {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": at[0], "ty": at[1]},
            "size": {"x": size[0], "y": size[1]}}


def doc():
    """Section 1 holds the set 2 (variant 3 `State=Default`, 200 x 100: node 4 `Row` at (8, 10) 184 x 32 holding node 5 `Label` at (12, 4) 60 x 20, and two nodes named `Dup`)
    and the plain component 9 `Plain` (50 x 40, a child `Cell` at (5, 5) 20 x 20)."""
    nodes = [node(1, "Section", {"Section": {}}, children=[2, 9]), node(2, "Menu/Item", {"ComponentSet": {}}, 1, [3]),
             node(3, "State=Default", {"Component": {}}, 2, [4, 6, 7], size=(200.0, 100.0)), node(4, "Row", {"Frame": {}}, 3, [5], at=(8.0, 10.0), size=(184.0, 32.0)),
             node(5, "Label", {"Text": {}}, 4, at=(12.0, 4.0), size=(60.0, 20.0)), node(6, "Dup", {"Frame": {}}, 3, at=(0.0, 50.0)), node(7, "Dup", {"Frame": {}}, 3, at=(0.0, 60.0)),
             node(9, "Plain", {"Component": {}}, 1, [10], size=(50.0, 40.0)), node(10, "Cell", {"Frame": {}}, 9, at=(5.0, 5.0), size=(20.0, 20.0))]
    return {"nodes": {str(n["id"]): n for n in nodes}}


def entry(**kw):
    return {"set": "Menu/Item", "variant": "State=Default", "case": "d-menu", "named": "x", **kw}


def problems(*entries, d=None):
    return geo.check(d or doc(), list(entries))


class ExactNodeTest(unittest.TestCase):
    def test_a_rect_that_is_the_nodes_bounds_passes(self):
        self.assertEqual(problems(entry(render_rect=[8, 10, 184, 32], render_node="Row")), [])

    def test_the_bounds_add_up_the_parents_offsets(self):
        self.assertEqual(geo.bounds(doc(), doc()["nodes"]["3"], "Row/Label"), (20.0, 14.0, 60.0, 20.0))

    def test_the_variant_itself_is_the_empty_path(self):
        self.assertEqual(problems(entry(render_rect=[0, 0, 200, 100], render_node="")), [])

    def test_a_plain_component_is_its_own_variant_named_empty(self):
        e = {"set": "Plain", "variant": "", "case": "f", "named": "p", "render_rect": [5, 5, 20, 20], "render_node": "Cell"}
        self.assertEqual(problems(e), [])

    def test_two_px_off_in_x_or_in_y_fails_and_names_the_edges(self):
        for rect, side in (([10, 10, 184, 32], "left"), ([8, 12, 184, 32], "top"), ([6, 10, 184, 32], "left"), ([8, 8, 184, 32], "top")):
            with self.subTest(rect=rect):
                out = problems(entry(render_rect=rect, render_node="Row"))
                self.assertEqual(len(out), 2, out)  # a shift moves both edges of its axis
                self.assertIn(f"its {side} edge", out[0])
                self.assertIn("Menu/Item State=Default d-menu/x", out[0])

    def test_two_px_off_in_size_fails_on_the_far_edges(self):
        self.assertIn("right edge", problems(entry(render_rect=[8, 10, 186, 32], render_node="Row"))[0])
        self.assertIn("bottom edge", problems(entry(render_rect=[8, 10, 184, 30], render_node="Row"))[0])

    def test_the_tolerance_is_half_a_pixel(self):
        self.assertEqual(problems(entry(render_rect=[8.25, 10, 183.75, 32], render_node="Row")), [])
        self.assertEqual(len(problems(entry(render_rect=[8.75, 10, 183.25, 32], render_node="Row"))), 1)


class WindowTest(unittest.TestCase):
    def test_a_window_is_the_node_trimmed_and_pinned_at_its_top_left(self):
        e = entry(render_rect=[8, 10, 100, 32], render_node="Row", render_trim=[0, 0, 84, 0])
        self.assertEqual(problems(e), [])

    def test_a_free_edge_is_not_pinned_but_the_origin_is(self):
        e = entry(render_rect=[8, 10, 100, 20], render_node="Row", render_trim=[0, 0, None, None])
        self.assertEqual(problems(e), [])
        self.assertEqual(len(problems(dict(e, render_rect=[10, 10, 100, 20]))), 1)
        self.assertEqual(len(problems(dict(e, render_rect=[8, 12, 100, 20]))), 1)

    def test_a_window_trimmed_from_the_left_is_where_the_trim_puts_it(self):
        e = entry(render_rect=[8 + 124, 10, 60, 32], render_node="Row", render_trim=[124, 0, 0, 0])
        self.assertEqual(problems(e), [])
        self.assertEqual(len(problems(dict(e, render_rect=[8 + 126, 10, 58, 32]))), 1)

    def test_a_window_in_a_nested_node(self):
        self.assertEqual(problems(entry(render_rect=[22, 16, 30, 10], render_node="Row/Label", render_trim=[2, 2, None, None])), [])

    def test_the_trim_has_four_numbers_or_nulls(self):
        for trim in ([0, 0, 0], "0,0,0,0", [0, 0, 0, "x"], [True, 0, 0, 0]):
            with self.subTest(trim=trim):
                self.assertIn("render_trim must be", problems(entry(render_rect=[8, 10, 184, 32], render_node="Row", render_trim=trim))[0])

    def test_a_null_left_or_top_trim_is_one_problem_and_never_a_crash(self):
        for trim in ([None, 0, 0, 0], [0, None, 0, 0], [None, None, None, None]):
            with self.subTest(trim=trim):
                out = problems(entry(render_rect=[8, 10, 184, 32], render_node="Row", render_trim=trim))
                self.assertEqual(len(out), 1, out)
                self.assertIn("left and top", out[0])

    def test_a_free_far_edge_stays_inside_its_node(self):
        e = entry(render_node="Row", render_trim=[0, 0, None, None])
        self.assertEqual(problems(dict(e, render_rect=[8, 10, 184.5, 32.5])), [])  # the node's own far edges, within the tolerance
        wide, tall = problems(dict(e, render_rect=[8, 10, 224, 20])), problems(dict(e, render_rect=[8, 10, 100, 72]))
        self.assertEqual((len(wide), len(tall)), (1, 1), (wide, tall))
        self.assertIn("right edge", wide[0])
        self.assertIn("bottom edge", tall[0])


class NamingTest(unittest.TestCase):
    def test_a_rect_with_no_node_fails(self):
        self.assertIn("names no render_node", problems(entry(render_rect=[8, 10, 184, 32]))[0])
        self.assertIn("names no render_node", problems(entry(render_rect=[8, 10, 184, 32], render_node=None))[0])

    def test_a_node_with_no_rect_fails(self):
        self.assertIn("has no render_rect", problems(entry(render_node="Row"))[0])
        self.assertIn("has no render_rect", problems(entry(render_trim=[0, 0, 0, 0]))[0])

    def test_an_entry_that_cuts_nothing_needs_nothing(self):
        self.assertEqual(problems(entry()), [])

    def test_a_missing_name_fails_and_lists_the_children(self):
        out = problems(entry(render_rect=[0, 0, 1, 1], render_node="Nope"))
        self.assertIn("not a child", out[0])
        self.assertIn("'Row'", out[0])

    def test_an_ambiguous_name_fails(self):
        self.assertIn("ambiguous", problems(entry(render_rect=[0, 0, 1, 1], render_node="Dup"))[0])

    def test_an_unknown_variant_fails(self):
        self.assertIn("the file has no", problems(entry(variant="State=Hover", render_rect=[0, 0, 1, 1], render_node=""))[0])


class CommittedDataTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.doc = json.loads((REPO / "design" / "studio-ui.designos").read_text(encoding="utf-8"))
        cls.entries = json.loads((HERE.parent / "studio_ui_file" / "evidence" / "crops.json").read_text(encoding="utf-8"))["entries"]

    def test_every_rect_is_where_its_node_puts_it(self):
        self.assertEqual(geo.check(self.doc, self.entries), [])

    def test_every_rect_names_its_node(self):
        cut = [e for e in self.entries if "render_rect" in e]
        self.assertGreaterEqual(len(cut), 41)
        self.assertTrue(all(isinstance(e.get("render_node"), str) for e in cut))

    def shifted(self, match, dx=0, dy=0):
        entries = copy.deepcopy(self.entries)
        hit = [e for e in entries if match(e)]
        self.assertEqual(len(hit), 1)
        hit[0]["render_rect"][0] += dx
        hit[0]["render_rect"][1] += dy
        return geo.check(self.doc, entries)

    def test_a_picker_tab_window_two_px_to_the_right_fails(self):
        out = self.shifted(lambda e: e.get("named") == "picker-tab:Custom", dx=2)
        self.assertTrue(out and "Popover/ColorPicker" in out[0], out)

    def test_the_table_header_window_two_px_down_fails(self):
        out = self.shifted(lambda e: e["set"] == "Table/Data" and e.get("rect") == [290, 81, 482, 41], dy=2)
        self.assertTrue(out and "Table/Data" in out[0], out)

    def test_the_file_thumbnail_strip_two_px_up_fails(self):
        out = self.shifted(lambda e: e["set"] == "Card/FileThumbnail", dy=-2)
        self.assertTrue(out and "Card/FileThumbnail" in out[0], out)

    def test_the_tag_chip_window_widened_by_forty_px_fails(self):
        entries = copy.deepcopy(self.entries)
        hit = [e for e in entries if e["set"] == "Tag/Chip" and e.get("render_trim") == [0, 0, None, 0]]
        self.assertEqual(len(hit), 1)
        hit[0]["render_rect"][2] += 40
        out = geo.check(self.doc, entries)
        self.assertEqual(len(out), 1, out)
        self.assertIn("Tag/Chip", out[0])

    def test_every_window_two_px_off_fails(self):
        for i, e in enumerate(self.entries):
            if "render_rect" not in e:
                continue
            for dx, dy in ((2, 0), (-2, 0), (0, 2), (0, -2)):
                entries = copy.deepcopy(self.entries)
                entries[i]["render_rect"][0] += dx
                entries[i]["render_rect"][1] += dy
                with self.subTest(i=i, dx=dx, dy=dy):
                    self.assertTrue(geo.check(self.doc, entries), "a 2 px shift of the window must fail")


if __name__ == "__main__":
    unittest.main()
