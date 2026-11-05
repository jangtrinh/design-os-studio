"""The MCP `has-hardcoded` rows are compared on `03 Studio` as well as `02 Core`: an entry under one page is expected as a row on that page only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_hardcoded_pages
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import hardcoded_check as hc  # noqa: E402
from test_studio_ui_file_hardcoded_check import node  # noqa: E402

DOC = {"pages": [1, 3, 4], "variable_collections": {}, "variables": {}, "nodes": {
    "1": node(1, "01 Foundations", "Page", None), "3": node(3, "02 Core", "Page", None, [10]), "4": node(4, "03 Studio", "Page", None, [20]),
    "10": node(10, "Chip", "Frame", 3), "20": node(20, "Panel", "Frame", 4)}}
ALLOW = [{"path": "02 Core/Chip", "property": "paddingLeft", "reason": "r", "value": 6.0},
         {"path": "03 Studio/Panel", "property": "paddingLeft", "reason": "r", "value": 8.0}]


class PagedMcp:
    """Answers each `has-hardcoded` query with the rows of the page it is scoped to."""

    def __init__(self, rows_by_page):
        self.rows, self.asked = rows_by_page, []

    def tool_or_die(self, tool, args):
        assert tool == "designos_query_xpath"
        self.asked.append(args["page_id"])
        key = args["selector"].split("'")[1]
        return {"columns": ["id", "name"], "rows": [[i, "n"] for i in self.rows.get((args["page_id"], key), [])], "done": True}


def run(rows, doc=DOC, allow=ALLOW):
    with tempfile.TemporaryDirectory() as tmp:
        f = Path(tmp) / "f.designos"
        f.write_text(json.dumps(doc))
        mcp = PagedMcp(rows)
        return hc.check(mcp, f, allow), mcp


class PagesTest(unittest.TestCase):
    def test_both_pages_are_queried_and_their_rows_matched_to_their_entries(self):
        problems, mcp = run({(3, "paddingLeft"): [10], (4, "paddingLeft"): [20]})
        self.assertEqual(problems, [])
        self.assertEqual(set(mcp.asked), {3, 4})

    def test_a_missing_row_on_the_studio_page_is_reported(self):
        problems, _ = run({(3, "paddingLeft"): [10]})
        self.assertEqual(problems, ["has-hardcoded('paddingLeft'): 03 Studio/Panel is allow-listed but MCP reports no literal there"])

    def test_a_problem_on_the_core_page_is_kept_when_the_studio_page_is_clean(self):
        problems, _ = run({(4, "paddingLeft"): [20]})
        self.assertEqual(problems, ["has-hardcoded('paddingLeft'): 02 Core/Chip is allow-listed but MCP reports no literal there"])

    def test_a_row_on_the_studio_page_the_allow_list_lacks_is_reported(self):
        problems, _ = run({(3, "paddingLeft"): [10], (4, "paddingLeft"): [20]}, allow=ALLOW[:1])
        self.assertEqual(problems, ["has-hardcoded('paddingLeft'): MCP reports a literal on 03 Studio/Panel that the allow-list does not name"])

    def test_an_entry_under_the_other_page_is_not_expected_on_this_one(self):
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(DOC, "02 Core")[1])["paddingLeft"], {"02 Core/Chip"})
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(DOC, "03 Studio")[1])["paddingLeft"], {"03 Studio/Panel"})

    def test_a_file_without_a_studio_page_is_checked_on_core_alone(self):
        doc = json.loads(json.dumps(DOC))
        doc["pages"] = [1, 3]
        problems, mcp = run({(3, "paddingLeft"): [10]}, doc=doc, allow=ALLOW[:1])
        self.assertEqual((problems, set(mcp.asked)), ([], {3}))


if __name__ == "__main__":
    unittest.main()
