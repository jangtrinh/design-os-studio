"""The MCP `has-hardcoded` rows on `02 Core` equal the allow-list, node for node (a fake MCP and a synthetic native file).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_hardcoded_check
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import hardcoded_check as hc  # noqa: E402

CORNERS = ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius")


def node(nid, name, kind, parent, children=(), radius=None, visible=None):
    return {"id": nid, "name": name, "kind": kind, "parent": parent, "children": list(children), **({"corner_radii": [radius] * 4} if radius else {}),
            **({} if visible is None else {"visible": visible})}


# A Focus/Ring set (node 98) whose main (node 99) has radius 12; Ring inherits it, Ring8 keeps a radius of its own.
DOC = {"pages": [1, 3], "nodes": {
    "1": node(1, "01 Foundations", "Page", None), "3": node(3, "02 Core", "Page", None, [10, 11, 12, 13]),
    "10": node(10, "Chip", "Frame", 3), "11": node(11, "Ring", {"Instance": {"component": 99}}, 3, radius=12.0), "12": node(12, "Bar", "Frame", 3),
    "13": node(13, "Ring8", {"Instance": {"component": 99}}, 3, radius=8.0),
    "98": node(98, "Focus/Ring", {"ComponentSet": {}}, None, [99]), "99": node(99, "Tone=Brand", {"Component": {}}, 98, radius=12.0)}}
ALLOW = [{"path": f"02 Core/Chip", "property": k, "reason": "r", "value": 12.0} for k in CORNERS] + [
    {"path": "02 Core/Ring", "property": "topLeftRadius", "reason": "r", "value": 12.0},  # an instance: inherited, no MCP row
    {"path": "02 Core/Bar", "property": "paddingLeft", "reason": "r", "value": 6.0},
    # a Focus/Ring instance whose radius differs from its main's: it owns the literal, so MCP reports a row (read from the file, no `host`)
    {"path": "02 Core/Ring8", "property": "topLeftRadius", "reason": "r", "value": 8.0},
    {"path": "02 Core/Ring8", "property": "strokeWeight", "reason": "r", "value": 2.0}]  # a ring's own stroke weight is inherited: no row


class FakeMcp:
    def __init__(self, rows, done=True):
        self.rows = rows  # {query key: [node ids]}
        self.done = done

    def tool_or_die(self, tool, args):
        assert tool == "designos_query_xpath"
        key = args["selector"].split("'")[1]
        return {"columns": ["id", "name"], "rows": [[i, "n"] for i in self.rows.get(key, [])], "done": self.done}


class HardcodedCheckTest(unittest.TestCase):
    def check(self, rows, done=True):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(DOC))
            return hc.check(FakeMcp(rows, done), f, ALLOW)

    def test_rows_equal_to_the_non_instance_allow_list_nodes_pass(self):
        self.assertEqual(self.check({"cornerRadius": [10, 13], "paddingLeft": [12]}), [])

    def test_an_inheriting_instance_entry_is_not_expected_as_a_row(self):
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(DOC)[1])["cornerRadius"], {"02 Core/Chip", "02 Core/Ring8"})

    def test_a_frame_copied_into_an_instance_inherits_its_main_literal_and_is_no_row(self):
        """A frame inside an instance mirrors a main node (`component.source`): the copy holds the main's literal without owning it, so the entry the sync repeats on it is allow-list-only
        (designos-mcp 0.7.5 reports no `has-hardcoded` row on the `Icon slot` copies of the Tree/Row instances of a region). The main's own entry is still a row."""
        d = json.loads(json.dumps(DOC))
        d["nodes"]["3"]["children"] += [20, 21]
        d["nodes"]["20"] = node(20, "Row", {"Instance": {"component": 98}}, 3, [22])
        d["nodes"]["22"] = dict(node(22, "Slot", "Frame", 20), component={"source": 12})
        d["nodes"]["21"] = node(21, "Slot", "Frame", 3)
        allow = ALLOW + [{"path": "02 Core/Row/Slot", "property": "paddingLeft", "reason": "r", "value": 6.0}, {"path": "02 Core/Slot", "property": "paddingLeft", "reason": "r", "value": 6.0}]
        self.assertEqual(hc.expected(allow, hc.paths_by_id(d)[1])["paddingLeft"], {"02 Core/Bar", "02 Core/Slot"})

    def test_only_the_corner_radius_of_a_pinned_ring_is_a_row(self):
        """Ring8's strokeWeight entry is on an instance that inherits the stroke from its main: MCP reports no row for it."""
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(DOC)[1])["strokeWeight"], set())

    def test_the_pinned_ring_comes_from_the_file_not_from_the_entry(self):
        """A ring whose stored radius differs from the main's owns the literal, whatever its entry says; one that inherits does not."""
        pinned = {path for path, _, pin, *_ in hc.paths_by_id(DOC)[1].values() if pin}
        self.assertEqual(pinned, {"02 Core/Ring8"})

    def test_a_hidden_pinned_ring_is_no_row(self):
        """The linter skips a node that is not visible, so a hidden ring's pinned corner is allow-list-only."""
        hidden = json.loads(json.dumps(DOC))
        hidden["nodes"]["13"]["visible"] = False
        self.assertEqual(hc.expected(ALLOW, hc.paths_by_id(hidden)[1])["cornerRadius"], {"02 Core/Chip"})

    def test_a_ring_instance_that_pins_a_literal_radius_is_expected_as_a_row(self):
        self.assertEqual(self.check({"cornerRadius": [10], "paddingLeft": [12]}),
                         ["has-hardcoded('cornerRadius'): 02 Core/Ring8 is allow-listed but MCP reports no literal there"])

    def test_an_inheriting_ring_instance_that_MCP_reports_fails(self):
        self.assertEqual(self.check({"cornerRadius": [10, 13, 11], "paddingLeft": [12]}),
                         ["has-hardcoded('cornerRadius'): MCP reports a literal on 02 Core/Ring that the allow-list does not name"])

    def test_a_truncated_reply_is_refused_not_compared(self):
        """`done` goes false when the server cuts the reply at the limit: comparing a partial row set would pass a missing literal."""
        out = self.check({"cornerRadius": [10, 13], "paddingLeft": [12]}, done=False)
        self.assertEqual(out, ["has-hardcoded('cornerRadius'): more than 500 rows, the check cannot compare them"])

    def test_a_literal_the_allow_list_does_not_name_fails(self):
        out = self.check({"cornerRadius": [10, 12, 13], "paddingLeft": [12]})
        self.assertEqual(out, ["has-hardcoded('cornerRadius'): MCP reports a literal on 02 Core/Bar that the allow-list does not name"])

    def test_a_stale_allow_list_entry_fails(self):
        out = self.check({"cornerRadius": [10, 13]})
        self.assertEqual(out, ["has-hardcoded('paddingLeft'): 02 Core/Bar is allow-listed but MCP reports no literal there"])

    def test_the_four_corners_are_one_query_and_an_unlisted_key_is_not_checked(self):
        self.assertEqual(hc.QUERIES["cornerRadius"], CORNERS)
        self.assertIn("strokeWeight", hc.QUERIES)


if __name__ == "__main__":
    unittest.main()
