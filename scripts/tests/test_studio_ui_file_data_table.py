"""Table/Data of components/data_table.py (row `Data & Code` of page `03 Studio`), read from the IR the builder returns against the real Semantic plan, with the Rust each
value comes from named in the module. Structure only: the compare crops the header, a group row and a colour row of `k-variables`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_data_table
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
import components.data_table  # noqa: E402,F401  (the module under test)
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover() if s["module"] == "data_table"}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def at(node, *names):
    """The descendant reached by child names; each step must name exactly one child."""
    for name in names:
        found = [k for k in node["kids"] if k["name"] == name]
        assert len(found) == 1, (name, [k["name"] for k in node["kids"]])
        node = found[0]
    return node


class TableTest(unittest.TestCase):
    """`table` (variables/view.rs:810-953) with `header_row` (:984-1030), `variable_row` (:1031-1127) and `value_cell` (:1283-1415)."""

    def setUp(self):
        self.spec = SPECS["Table/Data"]
        self.table = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_record_on_the_data_and_code_row_that_uses_no_other_set(self):
        self.assertEqual((self.spec["axes"], self.spec["row"], self.spec.get("uses")), ({}, "Data & Code", None))
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Mode", "TEXT"), ("Rows", "SLOT")])

    def test_the_columns_are_the_name_column_one_mode_column_and_the_add_column_with_a_grid_line_after_each(self):
        """200 + 1 + `mode_w` 280 + 1 + `VAR_ADD_COL` 39 = 521 (the capture's name column, one mode, and the add-mode cell)."""
        row = at(self.table, "Header", "Cells")
        widths = [k["w"] for k in row["kids"] if k["t"] == "F"]
        self.assertEqual(widths, [200, 1, 280, 1, 39])
        self.assertEqual([k["name"] for k in row["kids"]], ["Name column", "Grid line", "Mode column", "Grid line", "Add column"])
        self.assertEqual({k["fill"] for k in row["kids"] if k["name"] == "Grid line"}, {"color/border"})

    def test_the_header_is_40_high_in_bg_secondary_closed_by_a_one_px_border_line(self):
        header = at(self.table, "Header")
        cells = at(header, "Cells")
        self.assertEqual((cells["h"], cells["fill"]), (40, "color/bg-secondary"))
        self.assertEqual((at(header, "Row line")["fill"], at(header, "Row line")["h"]), ("color/border", 1))

    def test_the_header_labels_are_body_semibold_in_text_tertiary_16_in(self):
        for column, text in (("Name column", "Name"), ("Mode column", "Value")):
            cell = at(self.table, "Header", "Cells", column)
            label = [k for k in cell["kids"] if k["t"] == "T"][0]
            self.assertEqual((label["text"], label["style"], label["color"]), (text, "Body / Semibold", "color/text-tertiary"), column)
            self.assertEqual(pads(cell)[3][1], 16.0, column)
        self.assertEqual([k for k in at(self.table, "Header", "Cells", "Mode column")["kids"] if k["t"] == "T"][0]["prop"], "Mode")

    def test_the_add_mode_cell_is_the_16_px_plus_in_icon_colour(self):
        cell = at(self.table, "Header", "Cells", "Add column")
        self.assertEqual([(k["icon"], k["size"], k["color"]) for k in cell["kids"] if k["t"] == "I"], [("Plus", 16, "color/icon")])

    def test_the_rows_are_a_slot_of_the_captures_41_px_rows_four_groups_and_14_colours(self):
        rows = at(self.table, "Rows")
        names = [f"Group row {i}" if i in (1, 2, 6, 18) else f"Colour row {i}" for i in range(1, 19)]
        self.assertEqual((rows["slot"], [k["name"] for k in rows["kids"]]), ("Rows", names))
        for row in rows["kids"]:
            self.assertEqual((at(row, "Cells")["h"], at(row, "Row line")["h"]), (40, 1), row["name"])

    def test_a_group_row_is_bg_secondary_with_a_12_px_chevron_and_a_semibold_label_in_text_tertiary(self):
        name = at(self.table, "Rows", "Group row 1", "Cells", "Name column")
        self.assertEqual(at(self.table, "Rows", "Group row 1", "Cells")["fill"], "color/bg-secondary")
        self.assertEqual([(k["icon"], k["size"], k["color"]) for k in walk(name) if k["t"] == "I"], [("ChevronDown", 12, "color/text-tertiary")])
        label = [k for k in walk(name) if k["t"] == "T"][0]
        self.assertEqual((label["style"], label["color"]), ("Body / Semibold", "color/text-tertiary"))

    def test_a_variable_row_is_bg_with_a_kind_glyph_and_the_name_in_body_text(self):
        for row in ("Colour row 3", "Colour row 17"):
            cells = at(self.table, "Rows", row, "Cells")
            self.assertEqual(cells["fill"], "color/bg", row)
            name = at(cells, "Name column")
            label = [k for k in name["kids"] if k["t"] == "T"][0]
            self.assertEqual((label["style"], label["color"]), ("Body / Regular", "color/text"), row)
            self.assertEqual(at(name, "Kind glyph")["w"], 16, row)

    def test_the_colour_cell_is_its_own_swatch_and_the_hex_text_not_the_swatch_color_set(self):
        """`vars-swatch` is `variables/view.rs:367`, radius `round(w * 0.2)` = 3 and a `BORDER_TRANSLUCENT` inside stroke: not Swatch/Color's painter."""
        cell = at(self.table, "Rows", "Colour row 3", "Cells", "Mode column")
        swatch, hex_text = at(cell, "Swatch"), [k for k in cell["kids"] if k["t"] == "T"][0]
        self.assertEqual((swatch["w"], swatch["h"], swatch["radius"][1], swatch["stroke"]), (16, 16, 3.0, "color/border-translucent"))
        self.assertEqual(swatch["sw"][:2], ("lit", 1.0))
        self.assertIn("view.rs:376", swatch["sw"][2])  # `Stroke::new(1.0, theme::BORDER_TRANSLUCENT)`: a raw 1.0, no BORDER_WIDTH
        self.assertEqual((hex_text["style"], hex_text["color"], hex_text["text"]), ("Body / Regular", "color/text", "FFFFFF"))  # the capture's first colour variable (white)
        self.assertNotIn("Swatch/Color", {k.get("set") for k in walk(self.table)})

    def test_the_colour_cell_puts_the_swatch_17_in_and_the_hex_3_after_it(self):
        """`sw` is at `cell.left() + 17.0` (:1320), 16 wide, and the hex text at `cell.left() + 36.0` (:1334): 36 - 17 - 16 = 3 between them. Both are literals of the Rust (the lit's value is what
        the file carries), so the cell's left pad and its gap are the two numbers."""
        cell = at(self.table, "Rows", "Colour row 3", "Cells", "Mode column")
        self.assertEqual((cell["dir"], pads(cell)[3][:2], cell["gap"][:2]), ("H", ("lit", 17.0), ("lit", 3.0)))
        self.assertIn("view.rs:1320", pads(cell)[3][2])
        self.assertIn("view.rs:1320,1334", cell["gap"][2])
        swatch, hex_text = at(cell, "Swatch"), at(cell, "Hex")
        self.assertEqual(pads(cell)[3][1] + swatch["w"] + cell["gap"][1], 36.0)  # the hex text's x in the cell
        self.assertEqual(cell["kids"], [swatch, hex_text])

    def test_the_default_rows_nest_no_toggle_switch_the_capture_shows_no_boolean_variable(self):
        self.assertEqual([k["set"] for k in walk(self.table) if k["t"] == "X"], [])

    def test_the_last_row_is_a_group_header_cut_by_the_scrollbar_as_in_the_capture(self):
        rows = at(self.table, "Rows")["kids"]
        self.assertEqual((rows[-1]["name"], len(rows)), ("Group row 18", 18))
        label = [k for k in walk(rows[-1]) if k["t"] == "T"][0]
        self.assertEqual((label["text"], label["style"]), ("Group D", "Body / Semibold"))

    def test_every_style_exists(self):
        self.assertLessEqual({k["style"] for k in walk(self.table) if k["t"] == "T"}, STYLES)


if __name__ == "__main__":
    unittest.main()
