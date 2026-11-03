"""The exit decision of `evidence screens` (screen_compare.main), where the verdict over every screen is made: 0 when every cap holds and every named widget rect is within 1 px, 1 when a
cap fails (a `CAP` line says which) or a widget rect is off, 2 when no screen was compared. `screen_caps.exit_code` is tested on its own; this drives `main` with the render, the MCP session
and the scratch workspace replaced by fixed values, so a `main` that stops handing the caps' verdict on, or stops printing the `CAP` lines, fails here instead of passing silently.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_compare_main
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
PKG = HERE.parent / "studio_ui_file"
sys.path.insert(0, str(PKG))
from evidence import screen_compare as sc  # noqa: E402
from test_studio_ui_screen_caps import scores  # noqa: E402

SPEC = {"name": "Screen/EditorDesign", "state": "d-editor-default"}


def entry(**kw):
    """One entry of screens-scores.json: the caps test's measured numbers plus what `lines` prints (the widget rects and the excluded pixel count)."""
    geometry = {"matched": ["rail-File"], "mismatched": kw.pop("mismatched", []), "not_named": [], "keys": 1}
    return dict(scores(**kw), geometry=geometry, excluded_pixels=0)


class FakeMcp:
    ALL_SPECS = list(sc.screens_pkg.discover())

    def __init__(self, workspace):
        self.workspace = workspace

    def tool_or_die(self, name, args):
        nodes = [{"name": s["name"], "node_id": i} for i, s in enumerate(self.ALL_SPECS, 2)]
        return {"designos_get_document": {"pages": [{"name": "04 Screens", "node_id": 1}]},
                "designos_list_nodes": {"nodes": nodes}}.get(name, {})

    def close(self):
        pass


class MainExitTest(unittest.TestCase):
    def run_main(self, specs, result):
        """(exit code, stdout, the screens-scores.json written) of `main` with `result` as the one comparison it gets."""
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        (root / "ui.designos").write_text("{}")
        out, buf = root / "out", io.StringIO()
        workspace = mock.MagicMock()
        workspace.return_value.__enter__.return_value = root
        with mock.patch.object(sc.screens_pkg, "discover", return_value=specs), mock.patch.object(sc, "compare", return_value=result), \
                mock.patch.object(sc.mcpc, "Mcp", FakeMcp), mock.patch.object(sc.scratch, "workspace", workspace), contextlib.redirect_stdout(buf):
            code = sc.main(["--file", str(root / "ui.designos"), "--captures", str(root), "--out", str(out)])
        written = out / "screens-scores.json"
        return code, buf.getvalue(), json.loads(written.read_text()) if written.exists() else None

    def test_every_cap_holding_exits_0_and_prints_no_cap_line(self):
        code, text, written = self.run_main([SPEC], entry())
        self.assertEqual(code, 0)
        self.assertNotIn("CAP", text)
        self.assertEqual(written[0]["screen"], SPEC["name"])

    def test_a_failing_cap_exits_1_and_prints_a_cap_line_naming_the_region_and_both_numbers(self):
        code, text, _ = self.run_main([SPEC], entry(means={"rail": 2.9}))
        self.assertEqual(code, 1)
        cap_lines = [line for line in text.splitlines() if line.startswith("  CAP ")]
        self.assertEqual(len(cap_lines), 1, text)
        self.assertIn("region 'rail' mean 2.9 is over its cap 2.6", cap_lines[0])

    def test_a_worst_tile_over_its_cap_exits_1_even_with_every_region_mean_holding(self):
        code, text, _ = self.run_main([SPEC], entry(tile=40.0))
        self.assertEqual(code, 1)
        self.assertIn("worst tile", text)

    def test_a_widget_rect_off_by_more_than_a_pixel_exits_1_with_every_cap_holding(self):
        off = {"key": "rail-File", "layout": [0, 56, 47, 56], "node": [0, 60, 47, 56]}
        code, text, _ = self.run_main([SPEC], entry(mismatched=[off]))
        self.assertEqual(code, 1)
        self.assertIn("OFF rail-File", text)
        self.assertIn("CAP Screen/EditorDesign (d-editor-default): widget rect 'rail-File'", text)

    def test_a_screen_with_no_caps_exits_1(self):
        state = "g-spec-reader"
        self.assertNotIn(state, sc.caps.CAPS, f"{state!r} now has caps: pick an uncapped state")
        code, text, _ = self.run_main([SPEC], entry(state=state))
        self.assertEqual(code, 1)
        self.assertIn("no caps for this screen", text)

    def test_no_screen_to_compare_exits_2_and_writes_nothing(self):
        code, text, written = self.run_main([], entry())
        self.assertEqual(code, 2)
        self.assertIn("no screen to compare", text)
        self.assertIsNone(written)

    def test_duplicate_specs_print_one_info_same_drawing_line_each(self):
        dup = dict(SPEC, name="Screen/EditorDesign/d-window-chrome", state="d-window-chrome")
        code, text, _ = self.run_main([SPEC, dup], entry())
        self.assertEqual(code, 0)
        info = [line for line in text.splitlines() if line.startswith("INFO same-drawing ")]
        self.assertEqual(info, ["INFO same-drawing d-window-chrome = d-editor-default"])

    def test_main_over_discover_prints_the_3_same_drawing_info_lines(self):
        specs = sc.screens_pkg.discover()
        _, text, _ = self.run_main(specs, entry())
        info = [line for line in text.splitlines() if line.startswith("INFO same-drawing ")]
        self.assertEqual(len(info), 3)
        self.assertEqual(info, sc.screens_pkg.same_drawing_info_lines(specs))


if __name__ == "__main__":
    unittest.main()
