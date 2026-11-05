"""Proves Z16 accepts the Figma command names on its allowlist and still flags every other long label.

Run: python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
"""

import sys
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import ux_writing_labels as labels  # noqa: E402

MENU_FILE = "panels/menus.rs"

ALLOWLISTED = ("Paste and match style", "Swap fill and stroke")


def menu_source(*menu_labels: str) -> str:
    return "\n".join(f'    leaf("{label}", "", Action::Noop),' for label in menu_labels)


def scan(files):
    return labels.scan_labels(files, lambda rel: files[rel])


class FigmaCommandLabelTests(unittest.TestCase):
    def test_the_two_figma_command_names_pass(self):
        files = {MENU_FILE: menu_source(*ALLOWLISTED)}
        self.assertEqual(scan(files), Counter())
        ok, detail = labels.check_ux_writing(files, lambda rel: files[rel], baseline=Counter())
        self.assertTrue(ok, detail)

    def test_an_unlisted_four_word_menu_label_still_fails(self):
        unlisted = "Duplicate every selected layer"
        files = {MENU_FILE: menu_source(*ALLOWLISTED, unlisted)}
        self.assertEqual(scan(files), Counter({(MENU_FILE, labels.WORDS_KIND, unlisted): 1}))
        ok, detail = labels.check_ux_writing(files, lambda rel: files[rel], baseline=Counter())
        self.assertFalse(ok)
        self.assertIn(unlisted, detail)

    def test_the_allowance_is_for_that_menu_file_only(self):
        other = "panels/context.rs"
        files = {other: "\n".join(f'    MenuItem::new("{label}"),' for label in ALLOWLISTED)}
        self.assertEqual(scan(files), Counter({(other, labels.WORDS_KIND, label): 1 for label in ALLOWLISTED}))

    def test_a_count_in_an_allowlisted_label_still_fails(self):
        counted = "Swap fill and stroke (3)"
        files = {MENU_FILE: menu_source(counted)}
        self.assertIn(labels.COUNT_KIND, {kind for (_f, kind, _l) in scan(files)})


if __name__ == "__main__":
    unittest.main()
