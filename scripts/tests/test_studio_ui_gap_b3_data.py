"""The per-screen data of the popovers, menus and the vector bar (screens/gap_overlays_data.py, gap_overlays_data_menus.py): which Kind each capture's popover is, which menu rows carry the check
column, the vector bar's third cell. The data is plain dicts: no `screens()`, no gate module, and the screen modules do not import it yet.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b3_data
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
from screens import gap_overlays_data as pop  # noqa: E402
from screens import gap_overlays_data_menus as menus  # noqa: E402
from screens import gap_table_citations as gtc  # noqa: E402

PKG = HERE.parent / "studio_ui_file"
REGISTRY = json.loads((HERE.parents[1] / "design/component-registry.json").read_text(encoding="utf-8"))
KINDS = [v.partition("=")[2] for rec in REGISTRY["components"] if rec["name"] == "Popover/Panel" for v in rec["variants"] if v.startswith("Kind=")]
# state: (Kind, x, y): the origin of the overlay in layout.json `overlays` (or the named popover rect), the Kind the capture's popover is
PINS = {"f-edit-style-color": ("EditStyleColor", 960, 412), "edit-style-effect": ("EditStyleEffect", 960, 412), "edit-style-grid": ("EditStyleGrid", 960, 412),
        "edit-style-text": ("EditStyleText", 960, 412), "f-stroke-settings": ("Stroke", 960, 668), "effect-noise": ("Noise", 960, 668), "effect-glass": ("Glass", 960, 636),
        "effect-texture": ("Texture", 960, 732), "f-type-details": ("TypeSettings", 888, 156), "f-prototype-tab": ("Interaction", 952, 105), "ai-image-erase": ("Erase", 944, 640),
        "ai-image-expand": ("Expand", 956, 640), "j-comments": ("CommentThread", 801, 381), "agents-skills": ("Skills", 57, 608), "i-agent-permissions": ("Permissions", 540, 326)}


SIZE = {"EditStyleColor": (240, 184), "EditStyleEffect": (240, 344), "EditStyleGrid": (240, 344), "EditStyleText": (240, 280), "CommentThread": (300, 212), "Skills": (320, 192)}


def items(state):
    return [r for panel in menus.MENU[state] for r in panel["slots"]["Body"] if r["component"] == "Menu/Item"]


class PopoverDataTest(unittest.TestCase):
    def test_each_state_places_the_kind_its_capture_shows_at_the_overlays_origin(self):
        self.assertEqual({s: (e["variant"]["Kind"], e["x"], e["y"]) for s, e in pop.POPOVER.items()}, PINS)

    def test_every_kind_but_default_is_drawn_by_a_state_and_is_a_registry_kind(self):
        used = [e["variant"]["Kind"] for e in pop.POPOVER.values()]
        self.assertEqual(sorted(used), sorted(KINDS[1:]))
        self.assertEqual({e["variant"]["State"] for e in pop.POPOVER.values()}, {"Default"})

    def test_the_two_style_popovers_whose_captures_differ_are_different_drawings(self):
        self.assertNotEqual(pop.POPOVER["edit-style-grid"]["variant"], pop.POPOVER["edit-style-effect"]["variant"])

    def test_the_ten_popover_states_and_the_three_overlay_states_of_the_gap_table_are_covered(self):
        for state in (*gtc.POPOVERS, "ai-image-erase", "ai-image-expand", "j-comments", "agents-skills", "i-agent-permissions"):
            self.assertIn(state, pop.POPOVER, state)

    def test_an_entry_sets_a_kind_and_nothing_else_so_no_size_or_close_extras_remain(self):
        for state, e in pop.POPOVER.items():
            self.assertEqual(set(e), {"name", "component", "variant", "x", "y"}, state)

    def test_the_names_are_the_captures_widget_names_where_it_names_the_popover(self):
        names = {s: e["name"] for s, e in pop.POPOVER.items()}
        self.assertEqual(names["f-prototype-tab"], "proto-popover")
        self.assertEqual(names["ai-image-erase"], "ai-erase-popover")
        self.assertEqual(names["ai-image-expand"], "ai-expand-popover")
        self.assertEqual(names["i-agent-permissions"], "agent-permissions-modal")
        self.assertEqual(names["agents-skills"], "agents-skills-picker-popover")

    def test_the_replaced_texts_are_listed_as_rects_inside_their_overlay(self):
        overlays = {s: (x, y, *SIZE[k]) for s, (k, x, y) in PINS.items() if k in SIZE}
        overlays["micro-layer-picker"] = (680, 410, 220, 294)
        self.assertEqual(set(pop.TEXTS), {"f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "j-comments", "agents-skills", "micro-layer-picker"})
        for state, texts in pop.TEXTS.items():
            ox, oy, ow, oh = overlays[state]
            for (rx, ry, rw, rh), what, neutral in texts:
                self.assertTrue(ox <= rx and rx + rw <= ox + ow and oy <= ry and ry + rh <= oy + oh, f"{state}/{what}")
                self.assertTrue(what)

    def test_the_agents_skills_and_layer_picker_lists_name_every_row(self):
        self.assertEqual(len(pop.TEXTS["agents-skills"]), 7)  # three names, three descriptions, the path
        self.assertEqual(len(pop.TEXTS["micro-layer-picker"]), len(items("micro-layer-picker")) - 1)  # every row but the caption


class MenuDataTest(unittest.TestCase):
    def test_the_nine_menu_states_are_covered(self):
        self.assertEqual(set(menus.MENU), set(gtc.MENUS) | {"micro-layer-picker"})

    def test_the_menus_with_a_checkable_row_show_the_check_column_on_every_item_that_is_not_selected(self):
        for state in ("d-menu-main-file", "d-menu-shape", "d-menu-zoom", "grid-styles", "f-effect-styles"):
            panel = menus.MENU[state][-1]
            self.assertTrue(items(state), state)
            for r in panel["slots"]["Body"]:
                if r["component"] != "Menu/Item":
                    continue
                if r["variant"]["State"] == "Selected":
                    self.assertNotIn("Show check column", r["props"], f"{state}/{r['name']}")
                else:
                    self.assertTrue(r["props"].get("Show check column"), f"{state}/{r['name']}")

    def test_the_menus_without_a_check_column_have_none(self):
        for state in ("d-menu-main", "f-effects-menu", "ai-image-menu", "micro-layer-picker"):
            self.assertFalse([r for r in items(state) if r["props"].get("Show check column")], state)
        self.assertFalse([r for r in items("d-menu-main-file") if "Back to files" == r["name"] and r["props"].get("Show check column")])

    def test_the_ai_menu_keeps_the_32_px_rows_of_popup_menu(self):
        self.assertEqual({r["variant"]["Size"] for r in items("ai-image-menu")}, {"Xs"})
        self.assertEqual([r["name"] for r in items("ai-image-menu")], ["Remove background", "Isolate subject", "Expand...", "Erase..."])

    def test_the_effects_menu_carries_its_glyphs_and_the_soon_hint_as_data(self):
        swaps = [o["swap"] for r in items("f-effects-menu") for o in r.get("overrides", []) if "swap" in o]
        self.assertEqual(swaps, [f"Icon/{g}" for g in ("InnerShadow", "DropShadow", "LayerBlur", "BackgroundBlur", "Noise", "Texture", "Glass", "Shader")])
        shader = items("f-effects-menu")[-1]
        self.assertEqual((shader["variant"]["State"], shader["overrides"][0]["text"]), ("Disabled", "Soon"))

    def test_the_effect_style_list_has_four_disabled_captions_and_twenty_three_styles_with_the_first_selected(self):
        rows = items("f-effect-styles")
        self.assertEqual([r["variant"]["State"] for r in rows].count("Disabled"), 4)
        styles = [r for r in rows if r["variant"]["State"] != "Disabled"]
        self.assertEqual((len(styles), styles[0]["name"], styles[0]["variant"]["State"], {r["variant"]["State"] for r in styles[1:]}), (23, "Style 1", "Selected", {"Default"}))

    def test_the_layer_picker_is_a_caption_and_eight_icon_rows_in_the_captures_220_x_294_overlay(self):
        panel = menus.MENU["micro-layer-picker"][0]
        self.assertEqual((panel["x"], panel["y"], panel["w"], panel["h"]), (680, 410, 220, 294))
        rows = items("micro-layer-picker")
        self.assertEqual((rows[0]["name"], rows[0]["variant"]["State"]), ("Select layer", "Disabled"))
        icons = [o["swap"] for r in rows[1:] for o in r["overrides"]]
        self.assertEqual((len(rows[1:]), icons[0], icons[2]), (8, "Icon/Text", "Icon/AutoLayoutH"))
        self.assertEqual([r["variant"]["State"] for r in rows[1:]].count("Hover"), 1)

    def test_the_zoom_menu_draws_the_separator_under_its_input_as_production_does(self):
        body = menus.MENU["d-menu-zoom"][0]["slots"]["Body"]
        self.assertEqual([r["component"] for r in body[:3]], ["Input/Field", "Menu/Separator", "Menu/Item"])
        self.assertEqual([r["name"] for r in body if r["component"] == "Menu/Separator"], ["Separator 1", "Separator 2", "Separator 3"])

    def test_a_separator_spans_its_panel_and_an_item_is_the_panel_less_the_pill_inset(self):
        for state, panels in menus.MENU.items():
            for panel in panels:
                for r in panel["slots"]["Body"]:
                    self.assertEqual(r["w"], panel["w"] if r["component"] == "Menu/Separator" else panel["w"] - 16, f"{state}/{r['name']}")


class VectorDataTest(unittest.TestCase):
    def test_the_vector_bar_is_a_three_cell_strip_at_the_rect_of_its_widgets(self):
        e = pop.VECTOR["micro-vector-edit"]
        self.assertEqual((e["component"], e["x"], e["y"], e["w"], e["h"]), ("Tabs/Segmented", 618, 800, 252, 28))
        self.assertEqual(e["props"], {"First label": "Move", "Second label": "Bend", "Third label": "Paint bucket", "Show cell 3": True})


class ModuleShapeTest(unittest.TestCase):
    def test_the_data_modules_define_no_screens_and_are_no_gate_modules(self):
        for module in (pop, menus):
            self.assertFalse(hasattr(module, "screens"), module.__name__)
            self.assertFalse(module.__name__.endswith("_gate"), module.__name__)

    def test_the_new_files_keep_lines_under_200_characters(self):
        files = [PKG / "screens/gap_overlays_data.py", PKG / "screens/gap_overlays_data_menus.py", *sorted((PKG / "components").glob("popover_kinds*.py")),
                 HERE / "test_studio_ui_gap_b3_kinds.py", HERE / "test_studio_ui_gap_b3_parts.py", HERE / "test_studio_ui_gap_b3_load.py",
                 HERE / "test_studio_ui_gap_b3_data.py"]
        self.assertEqual([(p.name, i) for p in files for i, l in enumerate(p.read_text().splitlines(), 1) if len(l) >= 200], [])
        self.assertEqual([p.name for p in files if len(p.read_text().splitlines()) >= 200], [])


if __name__ == "__main__":
    unittest.main()
