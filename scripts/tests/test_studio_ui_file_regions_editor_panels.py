"""The left layers and right inspector regions (page `03 Studio`, row Regions) of components/regions_editor_left.py and regions_editor_right.py, read from the IR the builders return
against the real Semantic plan: the record, the rect of the capture it is sized to, the place of the widgets and rows layout.json and properties.rs give, and the neutral default
content.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_editor_panels
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import DENY, SIZE, SPECS, STYLES, build, by_name, default_build, rect  # noqa: E402,F401
from rest_ir import walk  # noqa: E402

AXES = {"Panel/LeftLayers": {}, "Panel/RightInspector": {"State": ["Page", "Frame", "Layers", "Text", "Instance", "Prototype", "Dev"]}}


class RecordsTest(unittest.TestCase):
    def test_every_region_is_a_record_of_the_row_regions_with_the_registrys_axes(self):
        for name, axes in AXES.items():
            self.assertEqual((SPECS[name]["row"], SPECS[name]["axes"]), ("Regions", axes), name)

    def test_each_region_is_sized_to_the_rect_of_the_capture(self):
        for name in AXES:
            root = default_build(name)
            self.assertEqual((root["w"], root["h"]), tuple(float(v) for v in SIZE[name]), name)

    def test_the_regions_nest_only_what_they_declare_and_place_their_children_absolutely(self):
        for name in AXES:
            root = default_build(name)
            self.assertLessEqual({n["set"] for n in walk(root) if n["t"] == "X"}, set(SPECS[name].get("uses", [])), name)
            if not root.get("dir"):  # the left panel's body is one auto layout column
                self.assertTrue(all(k.get("abs") for k in root["kids"]), name)

    def test_every_style_exists_and_no_default_text_names_a_kit_client_or_library(self):
        for name in AXES:
            root = default_build(name)
            self.assertLessEqual({n["style"] for n in walk(root) if n["t"] == "T"}, STYLES, name)
            texts = [n["text"] for n in walk(root) if n["t"] == "T"] + [str(v) for n in walk(root) if n["t"] == "X" for v in n["props"].values()]
            self.assertEqual([t for t in texts if DENY.search(t)], [], name)


class PanelsTest(unittest.TestCase):
    def test_the_left_panel_is_one_auto_layout_column_whose_heights_add_up_to_left_rs(self):
        """panels/left.rs:111-152 walks a running `y`; the flow here is header 65, `pages-toggle` 40, the Pages list (5 rows of 32), the 8 px separator gap and the 40 px Layers header, so the Layers
        list starts at 313 and fills to the panel's bottom (868): the placed rects are checked in `evidence screens` (G3 geometry: `pages-list`, `layers-list`, `pages-toggle`)."""
        root = build("Panel/LeftLayers")
        panel = by_name(root)
        self.assertEqual((root["dir"], root["w"], root["h"]), ("V", 241.0, 868.0))
        self.assertEqual([k["name"] for k in root["kids"]], ["Header", "pages-toggle", "pages-list", "scrollbar-track", "Pages and layers separator", "Layers header", "layers-list", "Right border"])
        flow = [panel[n] for n in ("Header", "Pages and layers separator")]
        self.assertEqual([k["h"] for k in flow], [65.0, 8.0])
        self.assertEqual(rect(panel["file-header"]), (8.0, 8.0, 184.0, 24.0))
        self.assertEqual(65 + 40 + 5 * 32 + 8 + 40, 313)
        self.assertEqual((panel["pages-list"]["sy"], panel["layers-list"]["sy"], panel["layers-list"]["clip"]), ("hug", "fill", True))
        self.assertEqual((panel["pages-list"]["slot"], panel["Layers rows"]["slot"]), ("Pages", "Layers"))

    def test_five_page_rows_and_eighteen_layer_rows_flow_in_their_slots_inset_8(self):
        root = build("Panel/LeftLayers")
        pages = [n for n in walk(root) if n["name"].startswith("page-")]
        layers = [n for n in walk(root) if n["name"].startswith("layer-")]
        self.assertEqual((len(pages), len(layers)), (5, 18))
        self.assertTrue(all(not n.get("abs") for n in pages + layers))
        self.assertTrue(all(n["set"] == "List/Row" for n in pages) and all(n["set"] == "Tree/Row" for n in layers))
        self.assertEqual({n["variant"] for n in layers}, {"Size=Xs, State=Default, Depth=0"})
        slots = [n for n in walk(root) if n.get("slot")]
        self.assertEqual({(n["slot"], n["dir"], n["pad"][3]) for n in slots}, {("Pages", "V", "size/page-row-inset"), ("Layers", "V", "size/page-row-inset")})

    def test_the_eye_button_is_the_icon_button_component_resized_to_24_not_a_local_frame(self):
        """`widgets::icon_button` is Button/Icon Xs with a 16 px glyph (widgets/mod.rs:144-150); Section/Header already nests it at 24 (`PANEL_ACTION_BUTTON`)."""
        eye = by_name(build("Panel/RightInspector"))["Content"]
        eye = next(n for n in walk(eye) if n["name"] == "Eye button")
        self.assertEqual((eye["t"], eye["set"], eye["props"], rect(eye)), ("X", "Button/Icon", {"Icon": "Eye"}, (209.0, 44.0, 24.0, 24.0)))
        self.assertIn("Button/Icon", SPECS["Panel/RightInspector"]["uses"])

    def test_the_panel_borders_are_the_capture_columns_240_in_the_left_panel_and_0_in_the_right(self):
        """d-editor-default: the left panel's 1 px border is the screen column 289 (49 + 240), the right panel's the column 1199 (its own first)."""
        left, right = by_name(build("Panel/LeftLayers")), by_name(build("Panel/RightInspector"))
        self.assertEqual(rect(left["Right border"]), (240.0, 0.0, 1, 868.0))
        self.assertEqual(rect(right["Left border"]), (0.0, 0.0, 1, 868.0))

    def test_the_layer_kinds_follow_the_capture_two_instances_then_columns_and_rows_of_frames(self):
        """d-editor-default lists two instance rows (no chevron), then frames: rows 4, 5, 8 and 9 are laid out in rows (the stacked-boxes icon), the others in columns."""
        layers = [n for n in walk(build("Panel/LeftLayers")) if n["name"].startswith("layer-")]
        icons = [n["props"]["Icon"] for n in layers]
        self.assertEqual(icons[:2], ["Instance", "Instance"])
        self.assertEqual([i for i, icon in enumerate(icons) if icon == "AutoLayoutV"], [4, 5, 8, 9])
        self.assertEqual([n["props"]["Has Children"] for n in layers[:3]], [False, False, True])
        self.assertEqual({n["props"]["Type label"] for n in layers[2:]}, {"Auto layout"})

    def test_the_style_sizes_follow_the_type_ramp_one_per_folder(self):
        content = [n for n in by_name(build("Panel/RightInspector"))["Content"]["kids"] if n["name"].startswith(("folder-", "style-"))]
        sizes, folder = [], -1
        for row in content:
            if row["name"].startswith("folder-"):
                folder += 1
            else:
                sizes.append((folder, [k["text"] for k in row["kids"] if k["name"] == "Size"][0]))
        self.assertEqual(sorted(set(sizes))[:4], [(0, "72/90"), (1, "60/72"), (2, "48/60"), (3, "36/44")])

    def test_the_two_lists_clip_and_the_last_layer_row_is_cut(self):
        panel = by_name(build("Panel/LeftLayers"))
        self.assertTrue(panel["layers-list"]["clip"])
        self.assertGreater(32.0 * 17 + 32, 868.0 - 313.0)  # 18 rows are 576 px under a 555 px list: the last one is cut

    def test_the_right_panel_widgets(self):
        panel = by_name(build("Panel/RightInspector"))
        self.assertEqual(rect(panel["right-top-avatar"]), (8.0, 8.0, 38.0, 24.0))
        self.assertEqual((rect(panel["present-button"]), rect(panel["present-chevron"]), rect(panel["share-button"])), ((139.0, 8.0, 24.0, 24.0), (163.0, 8.0, 16.0, 24.0), (185.0, 8.0, 48.0, 24.0)))
        self.assertEqual((rect(panel["right-tab-Design"])[:2], rect(panel["right-tab-Prototype"])[:2]), ((9.0, 48.0), (69.25, 48.0)))
        self.assertEqual(rect(panel["help-button"]), (207.0, 836.0, 28.0, 28.0))

    def test_the_page_field_and_the_style_rows_follow_properties_rs(self):
        panel = build("Panel/RightInspector")
        content = by_name(panel)["Content"]
        inner = by_name(content)
        self.assertEqual(rect(inner["Page field"]), (17.0, 44.0, 184.0, 32.0))
        rows = [n for n in content["kids"] if n["name"].startswith(("folder-", "style-"))]
        self.assertEqual(rect(rows[0])[1], 168.0)  # y 249 in the panel, 81 below the content's top
        self.assertEqual([r["abs"][1] - rows[0]["abs"][1] for r in rows[:3]], [0.0, 32.0, 64.0])
        self.assertEqual([r["name"].split("-")[0] for r in rows[:6]], ["folder", "style", "style", "style", "style", "folder"])
        self.assertTrue(content["clip"])



if __name__ == "__main__":
    unittest.main()
