"""G5 size (studio_ui_gate_size): a projection report, never a failure. Each screen of page `04 Screens` weighs what its nodes add to the saved file (compact JSON of every node of its subtree plus
the quoted id key, the colon and the comma); the projection is the file without its screens plus the planned 74 screens of the mean weight.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_size
"""
import sys
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import studio_ui_gate_size as size  # noqa: E402
from studio_ui_gate_doc import GateInputError  # noqa: E402
from studio_ui_gate_fixtures import make_doc  # noqa: E402


def with_screens(doc, *screens):
    """Page `04 Screens` (id 10) holding `screens`: each (id, name, [child ids]), the children being plain nodes named `a` (so `{"id":12,"name":"a"}` is 20 bytes of JSON)."""
    kids = []
    for sid, name, children in screens:
        doc["nodes"][str(sid)] = {"id": sid, "name": name, "children": list(children)}
        doc["nodes"].update({str(c): {"id": c, "name": "a"} for c in children})
        kids.append(sid)
    doc["nodes"]["10"] = {"id": 10, "name": "04 Screens", "kind": "Page", "children": kids}
    doc["pages"].append(10)
    return doc


class ScreenBytesTest(unittest.TestCase):
    def test_a_screen_weighs_its_subtree_in_compact_json_with_the_key_around_each_node(self):
        """`{"id":11,"name":"S1","children":[12]}` is 37 bytes + 2 digits + 4 = 43; `{"id":12,"name":"a"}` 20 + 6 = 26; `{"id":13,"name":"S2","children":[]}` 35 + 6 = 41."""
        r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", [])), 1000)
        self.assertEqual(r["screens"], [("S1", 2, 69), ("S2", 1, 41)])

    def test_a_name_with_non_ascii_characters_counts_its_utf8_bytes(self):
        """`{"id":14,"name":"é❖","children":[]}` is 38 bytes of UTF-8 (35 characters: é is 2 bytes, ❖ 3) + 2 digits + 4 = 44."""
        doc = with_screens(make_doc(), (14, "é❖", []))
        self.assertEqual(size.check_size(doc, 1000)["screens"], [("é❖", 1, 44)])

    def test_only_page_04_is_read(self):
        doc = with_screens(make_doc(), (11, "S1", []))
        self.assertEqual([s[0] for s in size.check_size(doc, 1000)["screens"]], ["S1"])

    def test_a_dangling_child_is_an_input_error(self):
        doc = with_screens(make_doc(), (11, "S1", [12]))
        del doc["nodes"]["12"]
        with self.assertRaises(GateInputError):
            size.check_size(doc, 1000)


class ProjectionTest(unittest.TestCase):
    def test_the_projection_is_the_base_plus_74_screens_of_the_mean_weight(self):
        r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", [])), 1000)
        self.assertEqual((r["base"], r["mean"], r["projected"]), (890, 55.0, 890 + 74 * 55))

    def test_a_file_with_the_planned_screens_or_more_projects_to_itself(self):
        with mock.patch.object(size, "PLANNED_SCREENS", 2):
            r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", [])), 1000)
        self.assertEqual(r["projected"], 1000)

    def test_a_file_with_more_than_the_planned_screens_does_not_shrink_to_a_smaller_projection(self):
        with mock.patch.object(size, "PLANNED_SCREENS", 1):
            r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", [])), 1000)
            out = size.lines(r)
            second = out[-1]
        self.assertEqual(len(out), 4, "the file line, a line per screen, the closing line")
        self.assertEqual(r["projected"], 1000)
        self.assertEqual(second, "G5 size   : the file already holds 2 screens (planned 1), so it is its own projection: 1,000 bytes (0.0 MB), a report: the size never fails the gate")

    def test_no_screen_leaves_nothing_to_project_and_says_so(self):
        r = size.check_size(make_doc(), 1000)
        self.assertEqual((r["screens"], r["mean"], r["projected"], r["base"]), ([], None, None, 1000))
        self.assertEqual(len(size.lines(r)), 1)
        self.assertIn("no screen on page 04 yet", size.lines(r)[0])

    def test_every_screen_has_a_line_of_its_own_between_the_file_line_and_the_projection(self):
        r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", []), (14, "S3", [])), 1000)
        out = size.lines(r)
        self.assertEqual(len(out), 5)
        self.assertEqual([l.split(": ", 1)[1] for l in out[1:4]], ["  S1: 2 nodes, 69 bytes", "  S2: 1 node, 41 bytes", "  S3: 1 node, 41 bytes"])
        self.assertTrue(out[4].startswith("G5 size   : projection for 74 screens"))

    def test_a_screen_over_a_thousand_bytes_is_written_with_a_thousands_separator(self):
        """`{"id":11,"name":"xx...x","children":[]}` with a 1,200 character name is 1,233 bytes of JSON + 2 digits + 4 = 1,239."""
        r = size.check_size(with_screens(make_doc(), (11, "x" * 1200, [])), 5000)
        self.assertEqual(size.lines(r)[1], f"G5 size   :   {'x' * 1200}: 1 node, 1,239 bytes")

    def test_the_lines_give_the_bytes_per_screen_and_the_projection(self):
        r = size.check_size(with_screens(make_doc(), (11, "S1", [12]), (13, "S2", [])), 6_514_193)
        first, *each, second = size.lines(r)
        self.assertEqual(each, ["G5 size   :   S1: 2 nodes, 69 bytes", "G5 size   :   S2: 1 node, 41 bytes"])
        self.assertIn("the file is 6,514,193 bytes (6.5 MB); 2 screens weigh 55 bytes each on average (2 nodes)", first)
        self.assertIn("the heaviest is S1 at 69", first)
        self.assertIn(f"projection for 74 screens: {6_514_193 - 110:,} bytes without screens + 74 x 55 = {6_514_193 - 110 + 74 * 55:,} bytes (6.5 MB)", second)
        self.assertIn("never fails", second)
        self.assertTrue(first.startswith("G5 size   : ") and second.startswith("G5 size   : projection for 74 screens: "), (first, second))


if __name__ == "__main__":
    unittest.main()
