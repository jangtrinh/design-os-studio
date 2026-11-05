"""What the data of Panel/RightInspector (screens/gap_right_panel_data.py) draws against the named rects of the capture each screen is: the selection-colour rows, the fields that show a bound
variable and the Alignment and Rotation rows. The captures are untracked, so a checkout without them skips these tests; the folder is the one `evidence/crops.json` names.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b1_capture
"""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import REPO  # noqa: E402
from test_studio_ui_gap_b1_data import resolve, root_of  # noqa: E402
from test_studio_ui_gap_b1_right_inspector import top  # noqa: E402

sys.path.insert(0, str(REPO / "scripts/studio_ui_file"))
from evidence.capture_dirs import captures_dir  # noqa: E402
from screens import gap_right_panel_data as data  # noqa: E402

CAPTURES = json.loads((REPO / "scripts/studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))["captures"]
CONTENT_TOP, FOOTER_TOP = 113, 864  # capture px: the right panel's content area (panel y 81 + 32) down to the footer (832 + 32)
DISPLACED = {"micro-scale-tool": 32, "micro-inspector-math": 32}  # the Copy row of a frame without auto layout is not in the Layers state (a recorded gap)
BOUND_KEY_FIELD = {"bound:paddingLeft": "padding-h", "bound:paddingTop": "padding-v", "bound:itemSpacing": "gap"}  # the layout key of a bound field -> the field of `frame()`
GLYPH_FIELD = {data.AL + "Flow group/Flow row/Gap field/Glyph": "gap", data.AL + "Padding group/Padding row/Horizontal padding/Glyph": "padding-h",
               data.AL + "Padding group/Padding row/Vertical padding/Glyph": "padding-v"}


def named(state):
    """The `named` map of a capture's layout.json (the captures are untracked: a checkout without them skips)."""
    path = Path(captures_dir(CAPTURES, None)) / f"{state}.layout.json"
    if not path.exists():
        raise unittest.SkipTest(f"no capture folder: {path}")
    return json.loads(path.read_text())["named"]


def in_view(rect):
    return CONTENT_TOP <= rect[1] < FOOTER_TOP


def colour_rows_shown(state):
    """The selection-colour rows the data leaves visible in a state's variant."""
    entry, root = data.RIGHT_PANEL[state], root_of(state)
    if resolve(root, "Content/Selection colors section") is None:
        return 0
    off = {o["path"] for o in entry["overrides"] if o.get("visible") is False}
    if "Content/Selection colors section" in off:
        return 0
    rows = resolve(root, "Content/Selection colors section/Selection colors body/Colour rows")["kids"]
    return len([r for r in rows if f"Content/Selection colors section/Selection colors body/Colour rows/{r['name']}" not in off])


def fields_the_data_binds(state):
    """The fields whose glyph the data swaps to the variable icon."""
    paths = [o["path"] for o in data.RIGHT_PANEL[state]["overrides"] if o.get("swap") == data.VARIABLE]
    return {GLYPH_FIELD[p] for p in paths}


def fields_the_capture_binds(state):
    """The fields the capture's layout names as bound to a variable inside the panel's content area."""
    keys = [k for k, v in named(state).items() if k.startswith("bound:") and in_view(v["rect"])]
    return {BOUND_KEY_FIELD[k] for k in keys}


class CaptureTest(unittest.TestCase):
    def test_the_selection_colour_rows_shown_are_the_ones_the_capture_draws(self):
        for state in data.RIGHT_PANEL:
            rows = [k for k, v in named(state).items() if k.startswith("selsw:") and in_view(v["rect"])]
            self.assertEqual(colour_rows_shown(state), len(rows), state)

    def test_a_field_that_shows_a_bound_variable_carries_its_name_not_a_number(self):
        for state, entry in data.RIGHT_PANEL.items():
            texts = {o["path"]: o["text"] for o in entry["overrides"] if "text" in o}
            for o in entry["overrides"]:
                if o.get("swap") == data.VARIABLE:
                    value = texts[o["path"].rsplit("/", 1)[0] + "/Value"]
                    self.assertFalse(value.replace(".", "").isdigit(), f"{state}: {o['path']} shows {value!r}")

    def test_the_fields_the_data_binds_are_the_fields_the_capture_names_as_bound(self):
        """`bound:paddingLeft`, `bound:paddingTop` and `bound:itemSpacing` are the padding-h, padding-v and gap fields; a bound key of another field in the content area joins the table."""
        for state in data.RIGHT_PANEL:
            keys = {k for k, v in named(state).items() if k.startswith("bound:") and in_view(v["rect"])}
            self.assertLessEqual(keys, set(BOUND_KEY_FIELD), f"{state}: a bound key the table does not map")
            self.assertEqual(fields_the_data_binds(state), fields_the_capture_binds(state), state)

    def test_the_alignment_and_transform_rows_sit_at_the_capture_rects(self):
        """`distribute` is the Alignment row's top and `xform:0` the Rotation row's: the variant's rows are where the capture puts them, but for the frames without auto layout."""
        checked = 0
        for state in data.RIGHT_PANEL:
            rects = {k: v["rect"][1] for k, v in named(state).items() if k in ("distribute", "xform:0")}
            if not rects:
                continue
            root = root_of(state)
            c = [k for k in root["kids"] if k["name"] == "Content"][0]
            body = "Position section/Position body/"
            for rect, row in (("distribute", "Alignment group/Alignment row"), ("xform:0", "Rotation group/Rotation row")):
                self.assertEqual(rects[rect] - CONTENT_TOP - top(c, body + row), DISPLACED.get(state, 0), f"{state} {rect}")
            checked += 1
        self.assertGreater(checked, 30)


if __name__ == "__main__":
    unittest.main()
