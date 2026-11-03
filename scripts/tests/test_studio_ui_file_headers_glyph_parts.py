"""Panel/Header and Section/Header choose the glyph of an action by parts, not by an exposed property.

A nested instance's glyph cannot be exposed as a property of its host (#1471), so the Layers header (ChevronDown, Search: `collapse_btn` left.rs:228, `fb` :220) and the Page title's action
(Opacity: properties.rs:405) are hidden copies of the action shown by a BOOLEAN; every default leaves the Pages row and the other sections as they were. Synthetic env (the headers test's).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_headers_glyph_parts
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from test_studio_ui_file_headers import SETS, build, named  # noqa: E402


class PanelHeaderGlyphPartsTest(unittest.TestCase):
    def test_the_layers_actions_are_chosen_by_two_booleans_and_the_pages_look_is_the_default(self):
        """Layers draws ChevronDown (`collapse_btn`, left.rs:228) then Search (`fb`, :220), Pages Search then Plus (:117-123). Nested copies cannot be exposed (#1471), so the choice is parts:
        a hidden Collapse action shown by `Show Collapse action`, and the Plus hidden by `Show Add action` (default on): by default the row is the Pages one, as before."""
        row = build("Panel/Header", State="Default")
        collapse, search, plus = (named(row, n)[0] for n in ("Collapse action", "Secondary action", "Primary action"))
        self.assertEqual((collapse["props"], collapse["visible_prop"], collapse["hidden"]), ({"Icon": "ChevronDown"}, "Show Collapse action", True))
        self.assertEqual((search["props"], search.get("visible_prop"), search.get("hidden")), ({}, None, None))
        self.assertEqual((plus["props"], plus["visible_prop"], plus.get("hidden")), ({"Icon": "Plus"}, "Show Add action", None))
        props = {p["name"]: (p["type"], p["default"]) for p in SETS["Panel/Header"]["props"]}
        self.assertEqual(props, {"Title": ("TEXT", "Pages"), "Show Collapse action": ("BOOLEAN", False), "Show Add action": ("BOOLEAN", True)})


class SectionHeaderGlyphPartsTest(unittest.TestCase):
    def test_the_page_section_shows_opacity_through_a_second_hidden_action(self):
        """The Page title's action is `Icon::Opacity` (panels/properties.rs:405), every other section's a Plus: a hidden copy of the action shown by `Show Opacity action` (default off)."""
        row = build("Section/Header", State="Default")
        opacity = named(row, "Opacity action")[0]
        self.assertEqual((opacity["set"], opacity["variant"], opacity["props"], opacity["abs"]), ("Button/Icon", "Size=Xs, State=Default", {"Icon": "Opacity"}, (209.0, 8.0, 24.0, 24.0)))
        self.assertEqual((opacity["visible_prop"], opacity["hidden"]), ("Show Opacity action", True))
        props = {p["name"]: (p["type"], p["default"]) for p in SETS["Section/Header"]["props"]}
        self.assertEqual(props["Show Opacity action"], ("BOOLEAN", False))
        self.assertEqual(named(build("Section/Header", State="Disabled"), "Opacity action")[0]["variant"], "Size=Xs, State=Disabled")


if __name__ == "__main__":
    unittest.main()
