"""MCP `has-hardcoded` is the linter's `hardcoded-value` finding, and for a number it fires only when some Number variable holds exactly that
literal (designos-doc/src/lint/token_match.rs `find_number`: a literal is "hard-coded" when a variable could replace it). A literal no variable
holds, such as a toggle track's 0.5 px border (`W * 0.5`, kit/choice.rs:38), is an allow-list entry the file gate needs but MCP never reports.
The row-for-row check therefore expects a row only for an entry whose value some Number variable holds.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_hardcoded_unmatched
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
from test_studio_ui_file_hardcoded_check import FakeMcp, node  # noqa: E402


def doc(*numbers, visible=None):
    """A page `02 Core` with one frame `Track` (hidden when `visible` is False), and one Semantic Number variable per value in `numbers` (plus a colour that never counts)."""
    variables = {f"v{i}": {"id": f"v{i}", "name": f"size/{i}", "collection": "cs", "kind": "Number", "values": {"m": {"Number": float(x)}}}
                 for i, x in enumerate(numbers)}
    variables["c"] = {"id": "c", "name": "color/x", "collection": "cs", "kind": "Color", "values": {"m": {"Color": {"r": 0.5, "g": 0.5, "b": 0.5, "a": 1.0}}}}
    return {"pages": [1, 3], "variable_collections": {"cs": {"default_mode": "m"}}, "variables": variables,
            "nodes": {"1": node(1, "01 Foundations", "Page", None), "3": node(3, "02 Core", "Page", None, [10]), "10": node(10, "Track", "Frame", 3, visible=visible)}}


ALLOW = [{"path": "02 Core/Track", "property": "strokeWeight", "reason": "W * 0.5", "value": 0.5}]


class UnmatchedLiteralTest(unittest.TestCase):
    def expected(self, d, allow=ALLOW):
        return hc.expected(allow, hc.paths_by_id(d)[1], hc.held_numbers(d))["strokeWeight"]

    def test_a_literal_no_number_variable_holds_is_not_expected_as_a_row(self):
        self.assertEqual(self.expected(doc(1, 2, 4)), set())

    def test_a_literal_a_number_variable_holds_is_expected_as_a_row(self):
        self.assertEqual(self.expected(doc(0.5, 2)), {"02 Core/Track"})

    def test_a_colour_variable_never_holds_a_number(self):
        self.assertEqual(hc.held_numbers(doc(2)), {2.0})

    def test_the_value_is_compared_as_a_number_with_f32_noise(self):
        self.assertEqual(self.expected(doc(0.5000001)), {"02 Core/Track"})
        self.assertEqual(self.expected(doc(0.51)), set())

    def test_check_passes_when_mcp_reports_nothing_for_an_unheld_literal(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(1, 2)))
            self.assertEqual(hc.check(FakeMcp({}), f, ALLOW), [])

    def test_check_fails_when_mcp_reports_a_held_literal_the_allow_list_lacks(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(0.5)))
            self.assertEqual(hc.check(FakeMcp({"strokeWeight": [10]}), f, []),
                             ["has-hardcoded('strokeWeight'): MCP reports a literal on 02 Core/Track that the allow-list does not name"])

    def test_check_still_fails_when_a_held_literal_is_allow_listed_but_not_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(0.5)))
            self.assertEqual(hc.check(FakeMcp({}), f, ALLOW),
                             ["has-hardcoded('strokeWeight'): 02 Core/Track is allow-listed but MCP reports no literal there"])

    def test_a_document_without_variables_keeps_every_entry_expected(self):
        """The older fixtures have no variable table: nothing to filter by, so the previous behaviour stands."""
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(doc(1))[1])["strokeWeight"], {"02 Core/Track"})


class HiddenOwnerTest(unittest.TestCase):
    """The linter never checks a node that is not visible (designos-doc/src/lint/traverse.rs, `SingleNodeLinter::lint` and the tree walk: `!n.visible`), so MCP
    reports no `has-hardcoded` row for it even when a Number variable holds its literal: its entry is allow-list-only, guarded by the file gate."""

    def expected(self, d):
        return hc.expected(ALLOW, hc.paths_by_id(d)[1], hc.held_numbers(d))["strokeWeight"]

    def test_a_held_literal_on_a_hidden_node_is_not_expected_as_a_row(self):
        self.assertEqual(self.expected(doc(0.5, visible=False)), set())

    def test_a_held_literal_on_a_visible_node_still_is(self):
        self.assertEqual(self.expected(doc(0.5, visible=True)), {"02 Core/Track"})
        self.assertEqual(self.expected(doc(0.5)), {"02 Core/Track"})

    def test_paths_by_id_records_the_nodes_own_visibility(self):
        self.assertFalse(hc.paths_by_id(doc(0.5, visible=False))[1][10][3])
        self.assertTrue(hc.paths_by_id(doc(0.5))[1][10][3])

    def test_check_passes_when_mcp_reports_nothing_for_a_held_literal_on_a_hidden_node(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(0.5, visible=False)))
            self.assertEqual(hc.check(FakeMcp({}), f, ALLOW), [])

    def test_check_still_fails_when_mcp_reports_a_hidden_node_the_allow_list_cannot_expect(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(0.5, visible=False)))
            self.assertEqual(hc.check(FakeMcp({"strokeWeight": [10]}), f, ALLOW),
                             ["has-hardcoded('strokeWeight'): MCP reports a literal on 02 Core/Track that the allow-list does not name"])


class HiddenAncestorTest(unittest.TestCase):
    """Only a node's own `visible` flag stops the linter: `has-hardcoded` runs `SingleNodeLinter::lint` on the node, which returns at its own `!n.visible`
    (designos-doc/src/lint/traverse.rs:382); only the full-tree walk skips a hidden subtree (:262). The query reaches the node directly, so a held literal on a
    VISIBLE child of a HIDDEN parent is a row (probed on designos-mcp 0.7.5: the child, and a grandchild below a hidden grandparent, are reported; the hidden node itself is not)."""

    ALLOW = [{"path": "02 Core/Group/Track", "property": "strokeWeight", "reason": "W * 0.5", "value": 0.5}]

    def doc(self, parent_visible, child_visible=None):
        d = doc(0.5)
        d["nodes"]["3"]["children"] = [9]
        d["nodes"]["9"] = node(9, "Group", "Frame", 3, [10], visible=parent_visible)
        d["nodes"]["10"] = node(10, "Track", "Frame", 9, visible=child_visible)
        return d

    def expected(self, d):
        return hc.expected(self.ALLOW, hc.paths_by_id(d)[1], hc.held_numbers(d))["strokeWeight"]

    def test_a_held_literal_on_a_visible_child_of_a_hidden_parent_is_expected_as_a_row(self):
        self.assertEqual(self.expected(self.doc(False)), {"02 Core/Group/Track"})

    def test_the_child_is_visible_by_its_own_flag_whatever_its_parent_is(self):
        by_id = hc.paths_by_id(self.doc(False))[1]
        self.assertFalse(by_id[9][3])
        self.assertTrue(by_id[10][3])

    def test_a_hidden_child_of_a_visible_parent_is_not_a_row(self):
        self.assertEqual(self.expected(self.doc(True, child_visible=False)), set())

    def test_check_fails_when_mcp_misses_the_row_of_a_visible_child_of_a_hidden_parent(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(self.doc(False)))
            self.assertEqual(hc.check(FakeMcp({}), f, self.ALLOW),
                             ["has-hardcoded('strokeWeight'): 02 Core/Group/Track is allow-listed but MCP reports no literal there"])
            self.assertEqual(hc.check(FakeMcp({"strokeWeight": [10]}), f, self.ALLOW), [])


if __name__ == "__main__":
    unittest.main()
