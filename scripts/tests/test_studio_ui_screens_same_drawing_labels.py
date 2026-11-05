"""Same-drawing labels on page `04 Screens`: a label names one of two reasons (an identical 0.7.5 capture, an open gap-table row), every table entry is a duplicate today, and the built row
label is the recorded one.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_same_drawing_labels
"""
import json
import sys
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402
from screens import gap_table_citations as gtc, same_drawing  # noqa: E402

LABELS = {"d-window-chrome": "#3 same drawing as #2: same 0.7.5 capture", "e-no-selection": "#6 same drawing as #2: same 0.7.5 capture",
          "i-terminal": "#12 same drawing as #11: same 0.7.5 capture"}
GAP_ROWS = {}  # {state: gap-table row}: the duplicates that name an open gap row, written out so no relabelling of one goes unseen
CAPTURES = captures_dir(json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))["captures"], None)


class FakeMcp:
    def __init__(self):
        self.calls = []

    def tool_or_die(self, tool, args):
        self.calls.append((tool, args))
        return {"root_node_id": 100 + len(self.calls)}


class SameDrawingLabelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.specs = screens.discover()
        cls.same = screens.same_drawings(cls.specs)

    def test_the_duplicates_are_the_three_identical_captures_with_the_label_each_carries(self):
        self.assertEqual({state: r["label"] for state, r in self.same.items()}, LABELS)
        self.assertEqual({state: r["of_state"] for state, r in self.same.items()},
                         {"d-window-chrome": "d-editor-default", "e-no-selection": "d-editor-default", "i-terminal": "i-agents-picker"})

    def test_a_gap_named_label_sits_on_a_screen_its_gap_row_lists_and_the_rows_are_pinned(self):
        named = {state: r for state, r in self.same.items() if r["gap"]}
        rows = {}
        for state, r in named.items():
            found = [e for e in gtc.GAP_TABLE if e["title"] == r["gap"]]
            self.assertEqual(len(found), 1, r["gap"])
            self.assertIn(state, found[0]["screens"], f"#{r['pos']} {state} names gap row {found[0]['row']} ({r['gap']}), which does not list it")
            rows[state] = found[0]["row"]
        self.assertEqual(rows, GAP_ROWS)

    def test_every_entry_of_a_table_is_a_duplicate_of_the_screens_today(self):
        for table in (same_drawing.GAP_NAMES, same_drawing.CAPTURE_IDENTICAL):
            self.assertEqual(sorted(set(table) - set(self.same)), [])
        self.assertEqual(set(same_drawing.CAPTURE_IDENTICAL) | set(same_drawing.GAP_NAMES), set(self.same))

    def test_a_state_has_one_reason_never_two(self):
        tables = (set(same_drawing.GAP_NAMES), set(same_drawing.CAPTURE_IDENTICAL))
        self.assertEqual(sum(len(t) for t in tables), len(set().union(*tables)))

    def test_a_dead_gap_entry_is_caught_by_the_table_test(self):
        with mock.patch.dict(same_drawing.GAP_NAMES, {"kit-agents": same_drawing.GAP_NAMES.get("kit-agents", "Agents & Terminal left panel states (Panel/AgentTerminal)")}):
            self.assertEqual(sorted(set(same_drawing.GAP_NAMES) - set(self.same)), ["kit-agents"])

    def test_a_gap_named_duplicate_carries_the_title_of_its_row_in_the_label(self):
        first = self.specs[1]
        dup = dict(first, name="Screen/EditorDesign/x-gap", state="x-gap")
        title = gtc.GAP_TABLE[1]["title"]
        with mock.patch.dict(same_drawing.GAP_NAMES, {"x-gap": title}):
            record = screens.same_drawings([first, dup])["x-gap"]
        self.assertEqual((record["gap"], record["reason"], record["label"]), (title, title, f"#2 same drawing as #1: {title}"))

    def test_the_built_label_of_every_duplicate_is_the_recorded_text(self):
        mcp = FakeMcp()
        screens.build(mcp, 7, self.specs)
        by_label = {c[1]["spec"]["name"]: c[1]["spec"]["text"] for c in mcp.calls if c[1]["spec"]["kind"] == "text"}
        for s in self.specs:
            if s["state"] in LABELS:
                self.assertEqual(by_label[f"Label {s['name']}"], LABELS[s["state"]])

    def test_a_duplicate_with_no_reason_fails_loudly(self):
        first = self.specs[1]
        with self.assertRaises(ValueError) as caught:
            screens.same_drawings([first, dict(first, name="Screen/EditorDesign/x-unnamed", state="x-unnamed")])
        self.assertIn("x-unnamed", str(caught.exception))

    @unittest.skipUnless(CAPTURES.exists(), "the 0.7.5 captures are untracked")
    def test_the_captures_of_the_identical_states_are_pixel_for_pixel_the_screen_they_repeat(self):
        try:
            from PIL import Image, ImageChops
        except ImportError:
            self.skipTest("Pillow is not installed")
        for state in same_drawing.CAPTURE_IDENTICAL:
            a, b = (Image.open(CAPTURES / f"{name}.png").convert("RGB") for name in (state, self.same[state]["of_state"]))
            self.assertIsNone(ImageChops.difference(a, b).getbbox(), state)


if __name__ == "__main__":
    unittest.main()
