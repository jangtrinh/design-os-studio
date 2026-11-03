"""Menu/Item `Show check column` and Tabs/Segmented `Show cell 3` (synthetic env: names are the theme constants; looks from the real kit-states.json).

Menu/Item: the check column of a menu where any row is checkable (widgets/menu.rs:166-199 `Layout::has_check`, :252-259 `label_x`): a check box at `MENU_CHECK_X` 12 and the label at
`MENU_LABEL_X_CHECK` 32, or 60 after an icon (the 4 px gap after the check hides with the icon box). Tabs/Segmented: the vector sub-tool bar's three cells (canvas/vector.rs:1110-1142).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b3_parts
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
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"BORDER_WIDTH": 1.0, "MENU_CHECK_BOX": 16.0, "MENU_ICON_LABEL_GAP": 4.0, "MENU_ICON_BOX": 24.0}


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


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class MenuItemCheckColumnTest(unittest.TestCase):
    def build(self, **values):
        return SETS["Menu/Item"]["build"]({"Size": "Xs", "State": "Default", **values}, FakeEnv())

    def test_the_set_has_a_boolean_show_check_column_that_is_off_by_default(self):
        prop = [p for p in SETS["Menu/Item"]["props"] if p["name"] == "Show check column"]
        self.assertEqual(prop, [{"name": "Show check column", "type": "BOOLEAN", "default": False}])

    def test_the_check_column_is_a_hidden_16_px_box_and_a_4_px_gap_that_shows_only_with_an_icon(self):
        column = named(self.build(), "Check column")[0]
        self.assertEqual((column["visible_prop"], column["hidden"], column["dir"]), ("Show check column", True, "H"))
        box, gap = column["kids"]
        self.assertEqual((box["w"], box["h"]), (16.0, 16.0))
        self.assertEqual((gap["w"], gap["visible_prop"], gap["hidden"]), (4.0, "Has Icon", True))

    def test_a_selected_row_keeps_its_own_check_and_has_no_second_column(self):
        node = self.build(State="Selected")
        self.assertEqual(named(node, "Check column"), [])
        self.assertTrue(named(node, "Check slot"))

    def test_every_other_state_has_the_column_and_no_check_slot(self):
        for state in ("Default", "Hover", "Active", "Focus", "Disabled"):
            node = self.build(State=state)
            self.assertTrue(named(node, "Check column"), state)
            self.assertEqual(named(node, "Check slot"), [], state)

    def test_the_default_row_is_the_drawing_it_was_before_the_column_the_hidden_slot_aside(self):
        node = self.build()
        shown = [k["name"] for k in node["kids"] if not k.get("hidden")]
        self.assertEqual(shown, ["Icon slot", "Label", "Submenu box"])


class SegmentedThirdCellTest(unittest.TestCase):
    def test_the_set_has_a_boolean_show_cell_3_off_by_default_and_a_third_label(self):
        props = {p["name"]: p for p in SETS["Tabs/Segmented"]["props"]}
        self.assertEqual(props["Show cell 3"], {"name": "Show cell 3", "type": "BOOLEAN", "default": False})
        self.assertEqual(props["Third label"]["type"], "TEXT")

    def test_the_third_cell_is_hidden_by_the_property_and_fills_like_the_other_two(self):
        for state in ("Selected", "Default", "Hover", "Focus", "Disabled"):
            node = SETS["Tabs/Segmented"]["build"]({"State": state}, FakeEnv())
            cells = list(node["kids"])
            self.assertEqual([c["name"] for c in cells], ["Cell 1", "Cell 2", "Cell 3"], state)
            third = cells[2]
            self.assertEqual((third["visible_prop"], third["hidden"], third["sx"], third["sy"]), ("Show cell 3", True, "fill", "fill"), state)
            self.assertTrue(all("visible_prop" not in c for c in cells[:2]), state)

    def test_the_third_cell_takes_the_second_cells_state(self):
        for state in ("Selected", "Default", "Hover", "Focus", "Disabled"):
            second, third = SETS["Tabs/Segmented"]["build"]({"State": state}, FakeEnv())["kids"][1:]
            for key in ("fill", "stroke", "sw", "alpha"):
                self.assertEqual(third.get(key), second.get(key), f"{state}: {key}")
            for key in ("color", "alpha"):
                self.assertEqual(named(third, "Label")[0].get(key), named(second, "Label")[0].get(key), f"{state}: label {key}")

    def test_the_first_two_cells_are_unchanged_by_the_third(self):
        node = SETS["Tabs/Segmented"]["build"]({"State": "Selected"}, FakeEnv())
        self.assertEqual([named(c, "Label")[0]["text"] for c in node["kids"][:2]], ["This page", "All pages"])


if __name__ == "__main__":
    unittest.main()
