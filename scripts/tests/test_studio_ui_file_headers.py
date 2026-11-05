"""Panel/Header, Section/Header and Tabs/Segmented against the painters 0.7.5 calls (synthetic env: names are the theme constants; the looks come from the
real kit-states.json).

Panel/Header: `kit::panel_header` (widgets/kit/rows.rs:114) via `section_title` (panels/left.rs:244), the row and its action cluster from `panels/left.rs:31-37, 113-122,
211-231`. Section/Header: `panels/properties.rs:207-226` `title` (the kit's `section_header`, rows.rs:121, has no caller). Tabs/Segmented: `kit::segmented`
(widgets/kit/tabs.rs:76-113), look `segmented_look` (:42-50), through `comments/ui.rs:134`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_headers
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"LEFT_PANEL_WIDTH": 240.0, "RIGHT_PANEL_WIDTH": 240.0, "PANEL_TITLE_ROW_HEIGHT": 40.0, "SECTION_TITLE_HEIGHT": 40.0, "PANEL_ACTION_BUTTON": 24.0,
       "SECTION_TRAILING_X": 209.0, "COMMENT_FILTER_H": 24.0, "BORDER_WIDTH": 1.0, "SPACE_FACT": 4.0, "INSET_SEGMENT": 2.0, "RADIUS_SEGMENT": 6.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out


SETS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SETS[name]["build"](values, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class PanelHeaderTest(unittest.TestCase):
    def test_the_set_is_one_default_variant_in_the_headers_row_nesting_button_icon(self):
        s = SETS["Panel/Header"]
        self.assertEqual((s["row"], s["axes"], s["uses"]), ("Headers & Tabs", {"State": ["Default"]}, ["Button/Icon"]))

    def test_the_row_is_the_panel_wide_and_title_row_high_with_the_title_at_inset_panel(self):
        row = build("Panel/Header", State="Default")
        self.assertEqual((row["w"], row["h"], row["sx"], row["sy"]), (240.0, 40.0, "fixed", "fixed"))
        self.assertEqual(ir.pads(row), [None, "PANEL_ACTION_RIGHT_PAD", None, "INSET_PANEL"], "left.rs:33 right - PANEL_ACTION_RIGHT_PAD; kit rows.rs:115 left + INSET_PANEL")
        self.assertNotIn("fill", row)
        self.assertNotIn("stroke", row)

    def test_the_title_is_text_sm_semibold_in_text_and_takes_the_free_width(self):
        title = named(build("Panel/Header", State="Default"), "Title")[0]
        self.assertEqual((title["style"], title["color"], title["sx"], title["prop"]), ("Text sm / Semibold", "TEXT", "fill", "Title"))

    def test_two_xs_icon_buttons_36_px_apart_with_plus_on_the_right(self):
        row = build("Panel/Header", State="Default")
        actions = [k for k in row["kids"] if k["t"] == "X" and not k.get("hidden")]
        self.assertEqual([(a["set"], a["variant"]) for a in actions], [("Button/Icon", "Size=Xs, State=Default")] * 2)
        self.assertEqual([a["props"] for a in actions], [{}, {"Icon": "Plus"}], "Pages: Search at the left (the set's own glyph), Plus rightmost")
        self.assertEqual(row["gap"], "SPACE_FACT", "LEFT_ACTION_PITCH = 32 + SPACE_FACT")
        self.assertEqual([k["name"] for k in row["kids"]], ["Title", "Collapse action", "Secondary action", "Primary action"])


class SectionHeaderTest(unittest.TestCase):
    def test_the_set_has_a_default_and_a_disabled_variant(self):
        s = SETS["Section/Header"]
        self.assertEqual((s["row"], s["axes"], s["uses"]), ("Headers & Tabs", {"State": ["Default", "Disabled"]}, ["Button/Icon"]))

    def test_the_row_is_239_by_40_and_the_title_sits_at_section_content_x(self):
        row = build("Section/Header", State="Default")
        self.assertEqual((row["w"], row["h"]), (239.0, 40.0))
        self.assertEqual(ir.pads(row), [None, None, None, "SECTION_CONTENT_X"])
        title = named(row, "Title")[0]
        self.assertEqual((title["style"], title["color"], title["alpha"] if "alpha" in title else 1.0), ("Body / Semibold", "TEXT", 1.0))
        self.assertNotIn("sx", title, "the action is absolute, so the title is only its text: nothing makes it fill the row (properties.rs:210-216 draws text at x)")

    def test_the_action_is_a_24_px_icon_button_at_x_209_y_8_shown_by_has_action(self):
        action = named(build("Section/Header", State="Default"), "Action")[0]
        self.assertEqual((action["set"], action["variant"], action["props"]), ("Button/Icon", "Size=Xs, State=Default", {"Icon": "Plus"}))
        self.assertEqual((action["abs"], action["visible_prop"]), ((209.0, 8.0, 24.0, 24.0), "Has Action"))
        self.assertNotIn("hidden", action, "the registry's own picture shows the action: it is on by default")

    def test_disabled_dims_the_title_and_nests_the_disabled_icon_button(self):
        row = build("Section/Header", State="Disabled")
        self.assertEqual(named(row, "Title")[0]["alpha"], 0.5, "section_header_look(Disabled) halves every paint (kit/mod.rs:134-162)")
        action = named(row, "Action")[0]
        self.assertEqual((action["variant"], action["props"]), ("Size=Xs, State=Disabled", {"Icon": "Plus"}))

    def test_the_disabled_arm_is_said_on_the_set_itself(self):
        """The instance panel shows the set's description, so the deviation lives on the node and not only in the module docstring (merge review LOW 7)."""
        text = SETS["Section/Header"].get("description", "")
        self.assertIn("section_header_look(Disabled)", text)
        self.assertIn("no disabled path", text)
        self.assertNotRegex(text, r"(?i)\b(ruling|W\d|review|phase)\b")

    def test_the_kit_hairline_the_painter_never_draws_is_not_here(self):
        self.assertEqual([k["name"] for k in build("Section/Header", State="Default")["kids"]], ["Title", "Action", "Opacity action"])


class SegmentedTest(unittest.TestCase):
    def cells(self, state):
        strip = build("Tabs/Segmented", State=state)
        return strip, named(strip, "Cell 1")[0], named(strip, "Cell 2")[0]

    def test_the_set_is_the_five_registry_states_with_a_focus_ring_dependency(self):
        s = SETS["Tabs/Segmented"]
        self.assertEqual((s["row"], s["axes"], s["uses"]), ("Headers & Tabs", {"State": ["Selected", "Default", "Hover", "Focus", "Disabled"]}, ["Focus/Ring"]))

    def test_the_shell_is_216_by_24_in_bg_secondary_with_a_2_px_inset_and_two_equal_cells(self):
        strip, one, two = self.cells("Default")
        self.assertEqual((strip["w"], strip["h"], strip["fill"], strip["radius"]), (216.0, 24.0, "BG_SECONDARY", "RADIUS_CONTROL"))
        self.assertEqual(ir.pads(strip), ["INSET_SEGMENT"] * 4)
        self.assertEqual([(c["sx"], c["sy"], c["radius"]) for c in (one, two)], [("fill", "fill", "RADIUS_SEGMENT")] * 2)

    def test_the_label_is_text_xs_semibold_the_style_label_font_names(self):
        for state in ("Selected", "Focus", "Disabled"):
            for label in named(self.cells(state)[0], "Label"):
                self.assertEqual(label["style"], "Text xs / Semibold", state)
                self.assertIn(label["style"], STYLES)

    def test_selected_first_cell_is_the_selected_tile_and_the_second_the_default_look(self):
        _, one, two = self.cells("Selected")
        self.assertEqual((one["fill"], one["stroke"], one["sw"]), ("BG_SELECTED", "BORDER_STRONG", "BORDER_WIDTH"))
        self.assertEqual(named(one, "Label")[0]["color"], "TEXT_SECONDARY")
        self.assertNotIn("fill", two)
        self.assertNotIn("stroke", two)
        self.assertEqual(named(two, "Label")[0]["color"], "TEXT_QUATERNARY")

    def test_default_and_hover_have_no_tile_and_hover_lights_only_the_first_label(self):
        _, one, two = self.cells("Default")
        self.assertEqual([c.get("fill") for c in (one, two)], [None, None])
        self.assertEqual([named(c, "Label")[0]["color"] for c in (one, two)], ["TEXT_QUATERNARY"] * 2)
        _, one, two = self.cells("Hover")
        self.assertEqual([named(c, "Label")[0]["color"] for c in (one, two)], ["TEXT_SECONDARY", "TEXT_QUATERNARY"])
        self.assertEqual([c.get("fill") for c in (one, two)], [None, None])

    def test_focus_is_the_focused_look_and_a_ring_on_the_first_cell_only(self):
        _, one, two = self.cells("Focus")
        self.assertEqual(one["fill"], "BG")
        rings = [k for c in (one, two) for k in c["kids"] if k["t"] == "X"]
        self.assertEqual([(r["set"], r["variant"]) for r in rings], [("Focus/Ring", "Tone=Brand")])
        self.assertIs(rings[0], one["kids"][-1], "a ring rings the node it sits in: the cell")
        self.assertEqual(rings[0]["radius"][1], 10.0, "round(RADIUS_SEGMENT 6 + SPACE_FACT 4), kit/focus.rs:38")

    def test_no_other_state_draws_a_ring(self):
        for state in ("Selected", "Default", "Hover", "Disabled"):
            self.assertEqual([k for k in ir.walk(build("Tabs/Segmented", State=state)) if k["t"] == "X"], [], state)

    def test_disabled_is_the_whole_strip_at_half_alpha_with_the_disabled_cell_look(self):
        strip, one, two = self.cells("Disabled")
        self.assertEqual((strip["alpha"], strip["fill"]), (0.5, "BG_SECONDARY"))
        for cell in (one, two):
            self.assertNotIn("fill", cell)
            label = named(cell, "Label")[0]
            self.assertEqual((label["color"], label["alpha"]), ("TEXT_QUATERNARY", 0.5))

    def test_every_other_state_has_every_paint_at_full_alpha(self):
        for state in ("Selected", "Default", "Hover", "Focus"):
            self.assertEqual({n.get("alpha", 1.0) for n in ir.walk(build("Tabs/Segmented", State=state))}, {1.0}, state)


class EveryStyleExistsTest(unittest.TestCase):
    def test_every_text_the_header_sets_draw_uses_a_defined_text_style(self):
        for name in ("Panel/Header", "Section/Header", "Tabs/Segmented"):
            for values in components.matrix(SETS[name]["axes"]):
                for t in (k for k in ir.walk(SETS[name]["build"](values, FakeEnv())) if k["t"] == "T"):
                    self.assertIn(t["style"], STYLES, f"{name} {values}")


if __name__ == "__main__":
    unittest.main()
