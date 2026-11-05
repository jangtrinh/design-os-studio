"""The compare finds the sets of both pages and gives a plain component one variant, named "".

`set_variants` reads the node kind (a ComponentSet has the variants as children, a plain Component is its own variant), never the name: a plain component's children are its layers.
Pages `02 Core` and `03 Studio`; a section's `Label ...` text and its title are not sets. A fake MCP stands in for the engine.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_compare_pages
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
from evidence import node_paths  # noqa: E402

PAGES = {"02 Core": 100, "03 Studio": 200, "99 Archive": 900}
# id -> (name, kind, children)
NODES = {
    100: ("02 Core", "Page", [101]), 101: ("Actions", "Section", [102, 103, 110]), 102: ("Section title", "Text", []), 103: ("Label Button/Icon", "Text", []),
    110: ("Button/Icon", "Component set", [111, 112]), 111: ("Size=Xs, State=Default", "Component", []), 112: ("Size=Xs, State=Hover", "Component", []),
    200: ("03 Studio", "Page", [201, 202]), 201: ("Cards", "Section", [203, 210, 220]), 203: ("Label Card/Library", "Text", []),
    210: ("Card/Library", "Component", [211, 212]), 211: ("Title", "Text", []), 212: ("Body", "Frame", []),
    220: ("Menu/Panel", "Component set", [221]), 221: ("State=Default", "Component", []),
    202: ("Menus & Popovers", "Section", []),
    900: ("99 Archive", "Page", [901]), 901: ("Archive/Thing", "Component set", [902]), 902: ("State=Default", "Component", []),
}


class FakeMcp:
    def tool_or_die(self, tool, args):
        assert tool == "designos_list_nodes", tool
        return {"nodes": [{"name": NODES[c][0], "node_id": c, "kind": NODES[c][1]} for c in NODES[args["parent_id"]][2]], "next_offset": None}


class SetVariantsTest(unittest.TestCase):
    def setUp(self):
        self.out = node_paths.set_variants(FakeMcp(), PAGES)

    def test_sets_of_both_pages_are_found(self):
        self.assertEqual(set(self.out), {"Button/Icon", "Menu/Panel", "Card/Library"})

    def test_a_set_has_its_variants(self):
        self.assertEqual(self.out["Button/Icon"], {"Size=Xs, State=Default": 111, "Size=Xs, State=Hover": 112})
        self.assertEqual(self.out["Menu/Panel"], {"State=Default": 221})

    def test_a_plain_component_is_its_own_single_variant_named_empty(self):
        self.assertEqual(self.out["Card/Library"], {"": 210})

    def test_labels_titles_other_pages_and_empty_sections_are_not_sets(self):
        self.assertFalse({n for n in self.out if n.startswith("Label ") or n == "Section title" or n.startswith("Archive")})

    def test_a_page_the_file_lacks_is_skipped(self):
        self.assertEqual(set(node_paths.set_variants(FakeMcp(), {"02 Core": 100})), {"Button/Icon"})

    def test_a_plain_component_with_a_slashless_name_is_still_found_by_kind(self):
        NODES[211] = ("Plain", "Component", [])
        NODES[201] = ("Cards", "Section", [210, 211])
        try:
            self.assertEqual(node_paths.set_variants(FakeMcp(), PAGES)["Plain"], {"": 211})
        finally:
            NODES[211] = ("Title", "Text", [])
            NODES[201] = ("Cards", "Section", [203, 210, 220])

    def test_a_set_with_a_slashless_name_is_found_by_kind_too(self):
        NODES[102] = ("Section title", "Component set", [111])
        try:
            self.assertEqual(node_paths.set_variants(FakeMcp(), PAGES)["Section title"], {"Size=Xs, State=Default": 111})
        finally:
            NODES[102] = ("Section title", "Text", [])

    def test_the_compare_reads_its_sets_through_it(self):
        src = (HERE.parent / "studio_ui_file" / "evidence" / "component_compare.py").read_text()
        self.assertIn("node_paths.set_variants(mcp, pages)", src)
        self.assertNotIn('pages["02 Core"]', src)


if __name__ == "__main__":
    unittest.main()
