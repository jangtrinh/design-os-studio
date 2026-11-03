"""evidence/crops.json is the one list of production crops: the rows each set's builder brought stay in it, no row is there twice, and `follows` names real sets only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_crops_union
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import components  # noqa: E402

CROPS = json.loads((HERE.parent / "studio_ui_file" / "evidence" / "crops.json").read_text(encoding="utf-8"))
# the fewest rows each set had when its crops were folded into the file
MIN_ROWS = {"Dialog/CommandPalette": 6, "Dialog/Modal": 3, "Menu/Panel": 3, "Panel/Header": 2, "Popover/Panel": 1, "Section/Header": 1, "Tabs/Segmented": 2,
            "Card/FileThumbnail": 1, "Card/Library": 1, "Popover/ColorPicker": 11, "Table/Data": 5}
FOLLOWED = ("Bar/Contextual", "Dialog/CommandPalette", "Dialog/Modal", "Menu/Context", "Menu/Panel", "Panel/Header", "Popover/Panel", "Section/Header", "Tabs/Segmented",
            "Toast/Default", "Card/FileThumbnail", "Card/Library", "Card/StarterTemplate", "Code/Block", "EmptyState/Default", "Popover/ColorPicker", "Table/Data")


def key(e):
    return (e["set"], e["variant"], e["case"], e.get("named"), tuple(e["rect"]) if e.get("rect") else None)


class CropsUnionTest(unittest.TestCase):
    def test_each_set_keeps_the_rows_its_builder_brought(self):
        have = {}
        for e in CROPS["entries"]:
            have[e["set"]] = have.get(e["set"], 0) + 1
        short = {s: (have.get(s, 0), n) for s, n in MIN_ROWS.items() if have.get(s, 0) < n}
        self.assertEqual(short, {}, "set: (rows in crops.json, rows it had)")

    def test_every_followed_set_says_which_rust_it_follows(self):
        self.assertEqual([s for s in FOLLOWED if not CROPS["follows"].get(s)], [])

    def test_no_row_is_in_the_list_twice(self):
        keys = [key(e) for e in CROPS["entries"]]
        self.assertEqual(len(keys), len(set(keys)))

    def test_only_the_rows_the_swap_pass_cannot_repair_are_exempt_from_the_box_and_colour_checks(self):
        """The picker's swapped icons are part of the generated Popover/ColorPicker, so the swap pass writes their colour, opacity, binding and flag and they match their captures: enforced.
        A row with an `icon` key is rendered by the compare itself, which places an instance on `99 Archive` and swaps the icon in a scratch copy, so the generated file's pass never
        sees it. The five Button/Icon and Button/Tool rows are such rows: they differ in box size (a swapped 20 px icon is reset to 16 px, #1427) and, in four of them, in the
        strongest pixel, so they keep `fg_pending`."""
        self.assertEqual(sorted({e["set"] for e in CROPS["entries"] if "fg_pending" in e}), ["Button/Icon", "Button/Tool"])
        self.assertEqual(sum("fg_pending" in e for e in CROPS["entries"]), 5)

    def test_follows_names_real_sets_only(self):
        names = {s["name"] for s in components.discover()}
        self.assertEqual(sorted(set(CROPS["follows"]) - names), [])


if __name__ == "__main__":
    unittest.main()
