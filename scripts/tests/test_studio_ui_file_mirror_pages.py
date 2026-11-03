"""`mirror` on page `03 Studio`: a copy there takes its main's waivers like a copy on 02 Core.

Composites sit on 03 Studio and nest sets of 02 Core, so a page-03 copy of a main that holds an allow-listed literal must repeat the entry on its own path, or the file gate
reports the copy's literal as unbound. Synthetic native documents.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_mirror_pages
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import mirror_sources as ms  # noqa: E402
from studio_ui_override_fixtures import BOX, HOST, IBOX, INST, MAIN, base, on_page_03  # noqa: E402

ENTRY = {"path": "02 Core/Card/Main/Box", "property": "paddingLeft", "reason": "r", "value": 6.0}


def doc():
    """`base()` with the main in a set `Card` on 02 Core and the host that holds its copy on page 03."""
    d = base()
    d["nodes"][str(MAIN)]["name"] = "Main"
    d["nodes"][str(INST)]["name"] = "Instance"
    d["nodes"]["80"] = {"id": 80, "name": "Card", "kind": {"ComponentSet": {}}, "parent": 3, "children": [MAIN]}
    d["nodes"][str(MAIN)]["parent"] = 80
    d["nodes"]["3"]["children"] = [40, 42, 43, 80, HOST]
    return on_page_03(d)


class Page03Mirror(unittest.TestCase):
    def test_a_page_03_copy_of_a_page_02_main_gets_the_entry_on_its_own_path(self):
        counts, allow = ms.mirror(doc(), [ENTRY])
        self.assertEqual([a["path"] for a in allow], ["02 Core/Card/Main/Box", "03 Studio/Host/Instance/Box"])
        self.assertEqual({k: v for k, v in allow[1].items() if k != "path"}, {k: v for k, v in ENTRY.items() if k != "path"})
        self.assertEqual(counts["allow"], 1)

    def test_a_page_03_copy_of_a_page_03_main_gets_the_entry_too(self):
        d = doc()
        d["nodes"]["3"]["children"].remove(80)
        d["nodes"]["9"]["children"].append(80)
        d["nodes"]["80"]["parent"] = 9
        entry = dict(ENTRY, path="03 Studio/Card/Main/Box")
        _, allow = ms.mirror(d, [entry])
        self.assertEqual([a["path"] for a in allow], ["03 Studio/Card/Main/Box", "03 Studio/Host/Instance/Box"])

    def test_a_page_03_copy_takes_the_bindings_of_its_main(self):
        d = doc()
        d["nodes"][str(IBOX)].pop("variables", None)
        counts, _ = ms.mirror(d)
        self.assertEqual(d["nodes"][str(IBOX)]["variables"]["bindings"], d["nodes"][str(BOX)]["variables"]["bindings"])
        self.assertGreater(counts["bindings"], 0)


if __name__ == "__main__":
    unittest.main()
