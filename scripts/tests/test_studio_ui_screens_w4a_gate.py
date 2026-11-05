"""The W4a gate data (screens/w4a_gate.py): every state is declared in full, every cap is the editor-default cap of its region or a pinned value with a cause, the masks name neutral replacements, the
kept rects are the capture's overlays, and the one pinned widget rect is a real key of its capture. Nothing is calibrated here: the numbers come from `evidence screens`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_w4a_gate
"""
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screen_caps as caps  # noqa: E402
import screen_masks as sm  # noqa: E402
import studio_ui_gate_screen_data as gd  # noqa: E402
from screens import w4a_menus  # noqa: E402,F401  (the screens import cleanly with the gate data)
from evidence.capture_dirs import captures_dir  # noqa: E402

GATE = HERE.parent / "studio_ui_file" / "screens" / "w4a_gate.py"
_MOD = gd._module(GATE)  # this batch's module alone: the other batches' states are theirs, and a merged tree holds all four
DATA = {t: dict(getattr(_MOD, t, None) or {}) for t in gd.TABLES}
STATES = sorted(DATA["REGIONS"])
DENY = re.compile(r"untitled|vsf|pcp|olivia|figma|❖", re.I)
MENUS = {"d-menu-main": [(12, 44, 210, 581)], "d-menu-main-file": [(12, 44, 210, 581), (222, 142, 265, 434)], "d-menu-shape": [(619, 636, 246, 208)], "d-menu-zoom": [(1216, 44, 212, 453)],
         "picker-page": [(960, 123, 240, 498)], "micro-layer-picker": [(680, 410, 220, 294)]}
VTOOLS = {"micro-vector-edit": [(618, 800, 252, 28)]}  # the capture's `vtool:Move` + `vtool:Bend` + `vtool:Paint bucket` named rects (618.5, 800, 252, 28), not an `overlays` entry


class DeclaredTest(unittest.TestCase):
    def test_the_nineteen_states_are_declared_in_every_table_the_comparison_reads(self):
        self.assertEqual(len(STATES), 19)
        for s in STATES:
            for table in ("CAPS", "REGIONS", "TEXTS", "CONTENT"):
                self.assertIn(s, DATA[table], (s, table))
            self.assertEqual(set(DATA["CAPS"][s]), {"screen", *DATA["REGIONS"][s]}, s)

    def test_the_states_are_merged_into_the_framework_tables(self):
        for s in STATES:
            self.assertEqual((caps.CAPS[s], sm.regions(s)), (DATA["CAPS"][s], DATA["REGIONS"][s]))
            self.assertTrue(sm.exclusions(s) is not None)

    def test_the_module_is_read_alone_so_another_batch_in_the_folder_does_not_change_it(self):
        import shutil
        with tempfile.TemporaryDirectory() as tmp:
            shutil.copy(GATE, Path(tmp) / "w4a_gate.py")
            shutil.copy(GATE.with_name("gap_wired_texts.json"), Path(tmp) / "gap_wired_texts.json")  # the rects the wired data imply: the module's one data file
            (Path(tmp) / "w9_gate.py").write_text("CAPS = {'x-other': {'screen': 4.6}}\nREGIONS = {'x-other': {}}\n", encoding="utf-8")
            merged = gd.load(tmp)
        self.assertEqual({s for s in merged["REGIONS"] if s in DATA["REGIONS"]}, set(STATES))
        self.assertIn("x-other", merged["CAPS"])

    def test_a_cap_is_the_editor_default_of_its_region_or_a_pinned_value_with_a_cause(self):
        for s in STATES:
            for region, cap in DATA["CAPS"][s].items():
                if cap != caps.DEFAULT_CAPS.get(region):
                    self.assertTrue(DATA["CAUSES"][s].get(region), (s, region, cap))
            if s in DATA["WORST_TILE"]:
                self.assertTrue(DATA["CAUSES"][s].get("worst tile"), s)
                self.assertGreater(DATA["WORST_TILE"][s], caps.WORST_TILE)

    def test_a_pinned_cap_is_a_measured_mean_rounded_up_to_a_hundredth(self):
        for s in STATES:
            for region, cap in DATA["CAPS"][s].items():
                if cap != caps.DEFAULT_CAPS.get(region):
                    self.assertAlmostEqual(cap, round(cap, 2), places=9, msg=(s, region))
                    self.assertGreater(cap, 0, (s, region))

    def test_the_editor_states_report_the_seven_regions_of_the_first_screen(self):
        for s in STATES:
            if s != "h-home":
                self.assertTrue({"tab bar", "rail", "left panel", "right panel", "ruler top strip", "ruler left strip", "toolbar"} <= set(DATA["REGIONS"][s]), s)


class MasksTest(unittest.TestCase):
    def test_every_replaced_text_names_what_it_replaces_and_a_neutral_replacement(self):
        for s in STATES:
            for rect, what, neutral in DATA["TEXTS"][s]:
                self.assertTrue(what and neutral and len(rect) == 4, (s, what))
                self.assertIsNone(DENY.search(f"{what} {neutral}"), (s, what))
                x, y, w, h = rect
                self.assertTrue(0 <= x and 0 <= y and x + w <= 1440 and y < 900, (s, rect))  # the last layer row is half below the window, as in the first screen

    def test_a_text_an_open_menu_covers_is_not_masked(self):
        for s, menus in MENUS.items():
            if s == "picker-page":
                continue
            for rect, what, _ in DATA["TEXTS"][s]:
                if s == "micro-layer-picker" and what.startswith("layer name "):
                    continue  # the picker lists the file's own layer names: its rows are the text the screen replaces
                for mx, my, mw, mh in menus:
                    self.assertFalse(rect[0] < mx + mw and mx < rect[0] + rect[2] and rect[1] < my + mh and my < rect[1] + rect[3], (s, what, rect))

    def test_the_kept_rects_are_the_captures_overlays_and_each_has_its_region(self):
        for s in STATES:
            self.assertEqual([tuple(r) for r in DATA["KEEP"].get(s, [])], MENUS.get(s, []) + VTOOLS.get(s, []), s)
            for rect in DATA["KEEP"].get(s, []):
                self.assertIn(tuple(rect), [tuple(r) for r in DATA["REGIONS"][s].values()], s)

    def test_the_overlay_rects_equal_the_captures_layout(self):
        crops = json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))
        folder = captures_dir(crops["captures"], None)
        if not (folder / "d-editor-default.png").exists():
            self.skipTest("the 0.7.5 captures are not in this checkout")
        for s, menus in MENUS.items():
            layout = json.loads((folder / f"{s}.layout.json").read_text(encoding="utf-8"))
            got = [tuple(round(v) for v in o["rect"]) for o in layout["overlays"]]
            self.assertEqual(got, menus, s)

    def test_the_vector_tools_rect_is_the_union_of_the_three_named_cells_of_the_capture(self):
        crops = json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))
        folder = captures_dir(crops["captures"], None)
        if not (folder / "d-editor-default.png").exists():
            self.skipTest("the 0.7.5 captures are not in this checkout")
        named = json.loads((folder / "micro-vector-edit.layout.json").read_text(encoding="utf-8"))["named"]
        cells = [named[f"vtool:{n}"]["rect"] for n in ("Move", "Bend", "Paint bucket")]
        x0, x1 = min(c[0] for c in cells), max(c[0] + c[2] for c in cells)
        self.assertEqual((round(x0), round(cells[0][1]), round(x1 - x0), round(cells[0][3])), VTOOLS["micro-vector-edit"][0])

    def test_home_has_no_canvas_content_and_masks_the_file_thumbnail_only(self):
        self.assertIsNone(DATA["CONTENT"]["h-home"])
        self.assertEqual([what for _, what, _ in DATA["TEXTS"]["h-home"]], ["file thumbnail"])

    def test_every_editor_state_masks_the_layer_name_column_only_and_never_a_whole_region(self):
        for s in STATES:
            if s == "h-home":
                continue
            names = [tuple(rect) for rect, what, _ in DATA["TEXTS"][s] if what == "layer name"]
            default = [(100, 345 + 32 * i, 82, 32) for i in range(18)]
            self.assertTrue(all(r[0] >= 100 and r[0] + r[2] <= 233 and r[3] <= 32 for r in names), s)  # a tree row's name starts at its indent and ends at the row's text column
            self.assertEqual(len([r for r in default if r in names]) < 18, s in ("d-menu-main", "d-menu-main-file"), s)  # only the two main-menu screens hide layer rows under their menu
            for rect, what, _ in DATA["TEXTS"][s]:
                self.assertLess(rect[2] * rect[3], 224 * 32 + 1, (s, what, "no text mask is a region's whole area"))

    def test_a_selection_state_with_a_tree_meets_the_default_left_panel_cap_except_the_two_whose_rows_the_data_cannot_place(self):
        for s in ("d-editor-selected", "kit-editor", "micro-smart-selection"):
            self.assertEqual(DATA["CAPS"][s]["left panel"], caps.DEFAULT_CAPS["left panel"], s)  # the capture's tree is drawn
        self.assertIn("scrolled", DATA["CAUSES"]["e-layers-selected"]["left panel"])  # a slot cannot express a scroll offset
        self.assertIn("child of a selected row", DATA["CAUSES"]["micro-css-grid"]["left panel"])
        for s in ("e-layers-selected", "micro-css-grid"):
            self.assertNotEqual(DATA["CAPS"][s]["left panel"], caps.DEFAULT_CAPS["left panel"], s)


class CausesTest(unittest.TestCase):
    """A pinned cap with another state's cause is not a cause: the causes of the overlay states name the overlay."""
    NAMED = {("d-menu-zoom", "right panel"): "zoom menu", ("d-menu-main-file", "ruler left strip"): "File submenu", ("d-menu-zoom", "menu 1"): "zoom menu",
             ("d-menu-shape", "menu 1"): "selected row", ("d-menu-main-file", "menu 2"): "File submenu"}

    def test_the_causes_of_the_overlay_states_name_the_overlay_not_the_selection_inspector(self):
        for (state, region), word in self.NAMED.items():
            cause = DATA["CAUSES"][state][region]
            self.assertIn(word, cause, (state, region))
            self.assertNotIn("selection inspector", cause, (state, region))
            self.assertNotIn("selection band", cause, (state, region))

    def test_the_selection_screens_keep_the_selection_causes(self):
        self.assertIn("no auto layout", DATA["CAUSES"]["micro-scale-tool"]["right panel"])
        self.assertIn("Vertex section", DATA["CAUSES"]["micro-vector-edit"]["right panel"])
        self.assertIn("selection band", DATA["CAUSES"]["micro-css-grid"]["ruler top strip"])
        for s in ("d-editor-selected", "kit-editor", "e-layers-selected"):
            self.assertNotIn("right panel", DATA["CAUSES"][s], f"{s}: the inspector draws the capture's state, so it meets its default cap")

    def test_no_pinned_cap_repeats_a_cause_that_names_a_region_the_state_does_not_pin(self):
        for s in STATES:
            for region, cause in DATA["CAUSES"][s].items():
                if "right panel and the ruler strips" in cause:
                    self.assertTrue({"right panel", "ruler top strip", "ruler left strip"} <= set(DATA["CAPS"][s]) and
                                    all(DATA["CAPS"][s][r] != caps.DEFAULT_CAPS[r] for r in ("right panel", "ruler top strip", "ruler left strip")), (s, region))


class PinsTest(unittest.TestCase):
    def test_no_screen_of_this_module_keeps_a_geometry_pin_the_screen_has_outgrown(self):
        """The Home tab bar's `+` is where the capture puts it, so no named widget of these screens is off by more than 1 px (the other modules pin their own)."""
        self.assertEqual(DATA["GEOMETRY"], {})


if __name__ == "__main__":
    unittest.main()
