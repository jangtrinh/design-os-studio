"""The exit code of a compare run: 0 when every row passes or carries an accepted finding, 1 when any row is a problem or any `render_rect` is not where its node puts it (a GEOMETRY line), 2 when
nothing was compared. The decision lives in `crop_report.exit_code(scores, geometry)` so it is tested without Pillow or MCP; `component_compare.run` must hand it the GEOMETRY lines it prints
(checked on its source, because the run itself needs Pillow and the engine and never runs in CI).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_exit
"""
import ast
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parent / "studio_ui_file" / "evidence"
sys.path.insert(0, str(EVIDENCE))
import crop_report as report  # noqa: E402

PASSING = {"set": "S", "variant": "", "capture": "c", "bad": False, "pending": None, "finding": None, "finding_pin": None}
ACCEPTED = dict(PASSING, bad=True, fg_ok=False, fg_render_px=(10, 10), fg_production_px=(12, 10), background_ok=True, finding="why",
                finding_pin={"fg_render_px": [10, 10], "fg_production_px": [12, 10]})
LINE = "Menu/Item State=Default d-menu/x: render_rect [8, 10, 184, 32] has its left edge at 10, but 'Row' trimmed by [0, 0, 0, 0] puts it at 8"


class GeometryExitTest(unittest.TestCase):
    def test_no_geometry_lines_leave_the_verdict_to_the_rows(self):
        self.assertEqual(report.exit_code([PASSING, ACCEPTED]), 0)
        self.assertEqual(report.exit_code([PASSING, ACCEPTED], geometry=()), 0)
        self.assertEqual(report.exit_code([PASSING, ACCEPTED], geometry=[]), 0)
        self.assertEqual(report.exit_code([]), 2)

    def test_a_geometry_line_exits_1_with_every_row_passing(self):
        self.assertEqual(report.exit_code([PASSING], geometry=[LINE]), 1)

    def test_no_accepted_finding_excuses_a_geometry_line(self):
        self.assertEqual(report.exit_code([PASSING, ACCEPTED], geometry=[LINE]), 1)

    def test_a_geometry_line_is_a_failure_even_when_nothing_was_compared(self):
        self.assertEqual(report.exit_code([], geometry=[LINE]), 1)

    def test_a_failing_row_still_exits_1_with_no_geometry_line(self):
        self.assertEqual(report.exit_code([dict(ACCEPTED, finding=None, finding_pin=None)], geometry=()), 1)


class CompareRunPassesItsGeometryTest(unittest.TestCase):
    def test_run_returns_exit_code_of_the_scores_and_the_geometry_lines_it_printed(self):
        tree = ast.parse((EVIDENCE / "component_compare.py").read_text(encoding="utf-8"))
        run = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run")
        last = run.body[-1]
        self.assertIsInstance(last, ast.Return)
        call = last.value
        self.assertTrue(isinstance(call, ast.Call) and getattr(call.func, "attr", None) == "exit_code", ast.dump(last))
        self.assertEqual([getattr(a, "id", None) for a in call.args], ["scores", "where"], "the exit code takes the scored rows and the GEOMETRY lines")


if __name__ == "__main__":
    unittest.main()
