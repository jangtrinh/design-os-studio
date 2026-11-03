"""MCP `has-hardcoded('cornerRadius')` sees a node only when its four radii are equal (probed on designos-mcp 0.7.5 with a REST file of eight frames and Number variables holding 11 and 8: it reported
[11, 11, 11, 11] and [8, 8, 8, 8], and none of [11, 11, 0, 0], [11, 0, 0, 0], [0, 0, 11, 11], [8, 8, 0, 0] and [11, 11, 11, 10], held literals all). A per-corner radius with a literal corner (the
starter banner's top corners, 11) is an allow-list entry the file gate needs and MCP never reports, like a literal no variable holds: the row-for-row check does not expect a row for it.
A uniform radius is still expected, and a row MCP does report for a non-uniform node is still a mismatch (the loud direction). Synthetic native files and a fake MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_hardcoded_corners
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
from test_studio_ui_file_hardcoded_check import CORNERS, FakeMcp  # noqa: E402


def doc(radii):
    """A page `03 Studio` with one frame `Banner` of the given corner radii and a Number variable that holds 11."""
    node = {"id": 10, "name": "Banner", "kind": "Frame", "parent": 3, "children": [], "corner_radii": radii}
    return {"pages": [1, 3], "variable_collections": {"cs": {"default_mode": "m"}},
            "variables": {"v": {"id": "v", "name": "size/11", "collection": "cs", "kind": "Number", "values": {"m": {"Number": 11.0}}}},
            "nodes": {"1": {"id": 1, "name": "01 Foundations", "kind": "Page", "parent": None, "children": []},
                      "3": {"id": 3, "name": "03 Studio", "kind": "Page", "parent": None, "children": [10]}, "10": node}}


def allow(*corners):
    return [{"path": "03 Studio/Banner", "property": CORNERS[i], "reason": "concentric", "value": 11.0} for i in corners]


def expected(radii, entries):
    d = doc(radii)
    return hc.expected(entries, hc.paths_by_id(d, "03 Studio")[1], hc.held_numbers(d))["cornerRadius"]


class CornerBlindnessTest(unittest.TestCase):
    def test_a_uniform_radius_with_literal_corners_is_expected_as_a_row(self):
        self.assertEqual(expected([11.0] * 4, allow(0, 1, 2, 3)), {"03 Studio/Banner"})

    def test_top_corners_only_is_not_expected_as_a_row(self):
        self.assertEqual(expected([11.0, 11.0, 0.0, 0.0], allow(0, 1)), set())

    def test_every_non_uniform_shape_is_blind(self):
        for radii, corners in (([11.0, 0.0, 0.0, 0.0], (0,)), ([0.0, 0.0, 11.0, 11.0], (2, 3)), ([11.0, 11.0, 11.0, 10.0], (0, 1, 2)), ([0.0, 11.0, 0.0, 0.0], (1,))):
            with self.subTest(radii=radii):
                self.assertEqual(expected(radii, allow(*corners)), set())

    def test_only_the_corner_query_is_blind_the_other_literals_of_the_node_are_still_expected(self):
        d = doc([11.0, 11.0, 0.0, 0.0])
        entries = allow(0, 1) + [{"path": "03 Studio/Banner", "property": "paddingLeft", "reason": "r", "value": 11.0}]
        want = hc.expected(entries, hc.paths_by_id(d, "03 Studio")[1], hc.held_numbers(d))
        self.assertEqual((want["cornerRadius"], want["paddingLeft"]), (set(), {"03 Studio/Banner"}))

    def test_a_node_with_no_corner_radii_key_is_uniform_zero(self):
        d = doc([])
        d["nodes"]["10"].pop("corner_radii")
        self.assertEqual(hc.expected(allow(0), hc.paths_by_id(d, "03 Studio")[1], hc.held_numbers(d))["cornerRadius"], {"03 Studio/Banner"})


class CheckTest(unittest.TestCase):
    def check(self, radii, entries, rows):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "f.designos"
            f.write_text(json.dumps(doc(radii)))
            return hc.check(FakeMcp(rows), f, entries)

    def test_a_non_uniform_banner_with_no_row_passes(self):
        self.assertEqual(self.check([11.0, 11.0, 0.0, 0.0], allow(0, 1), {}), [])

    def test_a_row_mcp_would_report_for_the_non_uniform_banner_is_a_mismatch(self):
        out = self.check([11.0, 11.0, 0.0, 0.0], allow(0, 1), {"cornerRadius": [10]})
        self.assertEqual(len(out), 1)
        self.assertIn("MCP reports a literal on 03 Studio/Banner that the allow-list does not name", out[0])

    def test_a_uniform_banner_with_no_row_is_a_mismatch(self):
        out = self.check([11.0] * 4, allow(0, 1, 2, 3), {})
        self.assertEqual(len(out), 1)
        self.assertIn("allow-listed but MCP reports no literal there", out[0])


if __name__ == "__main__":
    unittest.main()
