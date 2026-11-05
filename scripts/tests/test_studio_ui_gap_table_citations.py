"""Unit tests for the open rows of the Production <-> File Gap Table and their v0.7.5 Rust citations."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
for p in (ROOT / "scripts" / "studio_ui_file", ROOT / "scripts"):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))

from studio_ui_file.screens import gap_table_citations as gtc  # noqa: E402


class GapTableCitationsTest(unittest.TestCase):
    def test_every_gap_table_row_has_valid_v075_rust_citations(self):
        self.assertEqual(len(gtc.GAP_TABLE), 20)
        total_cites = sum(len(entry["citations"]) for entry in gtc.GAP_TABLE)
        self.assertEqual(total_cites, 32)
        fails, report = gtc.check_citations()
        self.assertEqual(fails, [], "\n".join(fails + report))
        self.assertIn("20 gap rows, 32 v0.7.5 Rust citations checked, 0 failures, 0 truncated screen lists", report[-1])

    def test_only_the_open_rows_are_listed_and_each_names_screens_that_exist(self):
        from studio_ui_file import screens as screens_pkg
        self.assertEqual([e["row"] for e in gtc.GAP_TABLE], [1, 2, 3, 5, 6, 7, 8, 9, 11, 12, 13, 15, 16, 18, 19, 20, 21, 22, 24, 25])  # 4, 10, 14, 17 and 23 are drawn
        for entry in gtc.GAP_TABLE:
            self.assertTrue(entry["screens"], entry["row"])
            self.assertEqual(sorted(set(entry["screens"]) - set(screens_pkg.ORDER)), [], entry["row"])
            self.assertEqual(len(set(entry["screens"])), len(entry["screens"]), entry["row"])

    def test_rows_two_and_three_list_the_menu_captures_no_data_covers_and_row_three_the_two_scrolled_trees(self):
        rows = {e["row"]: set(e["screens"]) for e in gtc.GAP_TABLE}
        self.assertEqual(set(gtc.STYLE_MENUS), {"f-effects-menu", "f-effect-styles", "grid-styles"})
        self.assertTrue(set(gtc.STYLE_MENUS) <= rows[2] and set(gtc.STYLE_MENUS) <= rows[3])
        self.assertTrue({"edit-style-text", "f-type-details"} <= rows[3])

    def test_row_eight_names_only_the_two_screens_that_still_show_its_gap(self):
        row8 = next(e for e in gtc.GAP_TABLE if e["row"] == 8)
        self.assertEqual(row8["screens"], ("g-dev-compare", "micro-dev-boxmodel"))
        self.assertIn("Changed", row8["title"])
        self.assertIn("playground", row8["title"])

    def test_rows_24_and_25_list_their_screens_and_source(self):
        rows = {e["row"]: e for e in gtc.GAP_TABLE}
        self.assertEqual(rows[24]["screens"], (*gtc.DEV, *gtc.UNSAVED_DOT))
        self.assertEqual(rows[24]["citations"], (("crates/designos-app/src/tabs.rs", 936, 1279, "pub fn show_top_tab_bar_ui"),))
        self.assertEqual(rows[25]["screens"], ("f-effect-styles",))
        self.assertEqual(rows[25]["citations"], (("crates/designos-app/src/panels/style_picker.rs", 217, 292, "pub(crate) fn style_button"),))

    def test_the_zoom_menu_row_cites_the_function_that_paints_its_input_row(self):
        row7 = next(e for e in gtc.GAP_TABLE if e["row"] == 7)
        self.assertEqual(row7["citations"], (("crates/designos-app/src/panels/menus.rs", 704, 801, "fn show_zoom_menu"),))
        self.assertEqual(gtc.SNAPSHOT["files"]["crates/designos-app/src/panels/menus.rs"]["items"]["704"]["item_end"], 801)
        self.assertNotIn("tool_menu_pos", repr(gtc.GAP_TABLE))

    def test_snapshot_matches_v075_git_tag_when_present(self):
        if not gtc.has_tag("v0.7.5"):
            self.skipTest("v0.7.5 git tag not present in shallow/tagless clone")
        fails, report = gtc.verify_snapshot_against_tag()
        self.assertEqual(fails, [], "\n".join(fails + report))

    def test_tuple_names_match_current_citation_counts(self):
        for present, expected in (("RULERS_51", 51), ("RIGHT_46", 46), ("LEFT_39", 39)):
            self.assertTrue(hasattr(gtc, present), f"missing {present}")
            self.assertEqual(len(getattr(gtc, present)), expected)
        for stale in ("RULERS_54", "RIGHT_49", "LEFT_42"):
            self.assertFalse(hasattr(gtc, stale), f"stale {stale} still present")
        for entry in gtc.GAP_TABLE:
            self.assertNotIn("…", entry["title"])
            self.assertNotIn("...", entry["title"])
            for s in entry["screens"]:
                self.assertNotIn("…", s)
                self.assertNotIn("...", s)

    def test_checker_rejects_stale_line_range_exceeding_v075_file_length(self):
        fake = ({
            "row": 99,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/history/banner.rs", 9, 44, "pub fn show"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("outside 1..40", fails[0])

    def test_checker_rejects_range_exceeding_item_end(self):
        fake = ({
            "row": 1,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/canvas/rulers.rs", 100, 215, "pub fn strips"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("does not end on item closing line 106", fails[0])

    def test_checker_rejects_range_stopping_short_of_item_end(self):
        fake = ({
            "row": 1,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/canvas/rulers.rs", 100, 101, "pub fn strips"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("does not end on item closing line 106", fails[0])

    def test_checker_accepts_only_the_item_closing_line_and_rejects_one_line_past_it(self):
        def run(end):
            return gtc.check_citations(({"row": 1, "title": "fake", "screens": ("h-home",),
                                         "citations": (("crates/designos-app/src/canvas/rulers.rs", 100, end, "pub fn strips"),)},))[0]
        self.assertEqual(run(106), [])
        for end in (105, 107):
            fails = run(end)
            self.assertEqual(len(fails), 1, end)
            self.assertIn("does not end on item closing line 106", fails[0])

    def test_checker_rejects_padded_start_line(self):
        fake = ({
            "row": 21,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/panels/context.rs", 1, 241, "fn show_layer_picker"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("start line 1 is not a snapshotted item start", fails[0])

    def test_checker_rejects_missing_symbol_in_range(self):
        fake = ({
            "row": 99,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/history/banner.rs", 9, 40, "fn nonexistent_symbol"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("symbol 'fn nonexistent_symbol' not found on start line", fails[0])

    def test_checker_rejects_missing_file_at_v075(self):
        fake = ({
            "row": 99,
            "title": "fake",
            "screens": ("h-home",),
            "citations": (("crates/designos-app/src/does_not_exist.rs", 1, 10, "fn show"),),
        },)
        fails, _ = gtc.check_citations(fake)
        self.assertEqual(len(fails), 1)
        self.assertIn("does not exist at tag v0.7.5", fails[0])


if __name__ == "__main__":
    unittest.main()
