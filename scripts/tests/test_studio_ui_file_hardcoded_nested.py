"""A literal inside a component that another component nests is owned by the main only: every copy of it, at any depth, inherits it and is no `has-hardcoded` row.

Two shapes failed `library` before a region could nest its parts: a set whose children carry literals (Table/Data's pads, gaps and radii, nested in Window/Variables) and a literal
whose value some Number variable holds (a 28 px disc with a radius of 14, nested in a composer, nested in a panel). The discriminator is `component.source`, which the engine sets on
every node an instance copies from its main: a node a user added to an instance (slot content) has none, owns its literal, and is a row. Synthetic native file and a fake MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_hardcoded_nested
"""
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import hardcoded_check as hc  # noqa: E402
from test_studio_ui_file_hardcoded_check import DOC, FakeMcp, node  # noqa: E402


def entry(path, prop="topLeftRadius", value=14.0):
    return {"path": path, "property": prop, "reason": "r", "value": value}


def corners(path):
    """The four corner entries of a node whose radius is a literal 14 (the engine reports one `cornerRadius` row for them)."""
    return [entry(path, k) for k in hc.QUERIES["cornerRadius"]]


def two_levels():
    """Composer (main, 40) holds the Send disc (41, radius 14). Panel (50) nests Composer (51, a copy 52 of Send); Window (60) nests Panel (61 > 62 Composer > 63 Send)."""
    d = copy.deepcopy(DOC)
    d["nodes"]["3"]["children"] += [40, 50, 60]
    d["nodes"].update({
        "40": node(40, "Composer", "Frame", 3, [41]), "41": node(41, "Send", "Frame", 40, radius=14.0),
        "50": node(50, "Panel", "Frame", 3, [51]), "51": node(51, "Composer", {"Instance": {"component": 40}}, 50, [52]),
        "52": dict(node(52, "Send", "Frame", 51, radius=14.0), component={"source": 41}),
        "60": node(60, "Window", "Frame", 3, [61]), "61": node(61, "Panel", {"Instance": {"component": 50}}, 60, [62]),
        "62": dict(node(62, "Composer", {"Instance": {"component": 40}}, 61, [63]), component={"source": 51}),
        "63": dict(node(63, "Send", "Frame", 62, radius=14.0), component={"source": 52})})
    return d


ALLOW = [e for p in ("02 Core/Composer/Send", "02 Core/Panel/Composer/Send", "02 Core/Window/Panel/Composer/Send") for e in corners(p)]


class NestedLiteralTest(unittest.TestCase):
    def rows(self, doc, allow):
        return hc.expected(allow, hc.paths_by_id(doc)[1])

    def test_only_the_main_owns_the_literal_however_deep_the_copies_are(self):
        self.assertEqual(self.rows(two_levels(), ALLOW)["cornerRadius"], {"02 Core/Composer/Send"})

    def test_the_check_passes_when_mcp_reports_the_main_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(two_levels()))
            self.assertEqual(hc.check(FakeMcp({"cornerRadius": [41]}), f, ALLOW), [])

    def test_a_row_mcp_reports_on_a_copy_is_a_finding_not_a_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(two_levels()))
            self.assertEqual(hc.check(FakeMcp({"cornerRadius": [41, 63]}), f, ALLOW),
                             ["has-hardcoded('cornerRadius'): MCP reports a literal on 02 Core/Window/Panel/Composer/Send that the allow-list does not name"])

    def test_a_node_added_to_an_instance_has_no_source_and_owns_its_literal(self):
        d = two_levels()
        d["nodes"]["51"]["children"].append(53)
        d["nodes"]["53"] = node(53, "Badge", "Frame", 51, radius=14.0)
        allow = ALLOW + corners("02 Core/Panel/Composer/Badge")
        self.assertEqual(self.rows(d, allow)["cornerRadius"], {"02 Core/Composer/Send", "02 Core/Panel/Composer/Badge"})

    def test_a_held_padding_and_a_gap_on_a_copy_are_inherited_too(self):
        allow = ALLOW + [entry(p, "paddingLeft", 6.0) for p in ("02 Core/Composer/Send", "02 Core/Panel/Composer/Send")] + [entry("02 Core/Panel/Composer/Send", "itemSpacing", 4.0)]
        got = self.rows(two_levels(), allow)
        self.assertEqual((got["paddingLeft"], got["itemSpacing"]), ({"02 Core/Composer/Send"}, set()))


if __name__ == "__main__":
    unittest.main()
