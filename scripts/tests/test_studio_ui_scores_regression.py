"""The regression differ over two `compare-scores.json` runs (scores_regression): the rows are compared on every field but `note` and the `finding` text, the `finding_pin` included, so a pin that
moves while its score holds is listed instead of counted as unchanged.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_scores_regression
"""
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import scores_regression as rg  # noqa: E402

PIN = {"mean_abs_rgb_diff": 1.42}


def row(set_name="Table/Data", variant="Size=Sm", capture="k-variables", label="[322,204,449,41]", icon=None, **kw):
    return {"set": set_name, "variant": variant, "capture": capture, "label": label, "icon": icon, "bad": False, "mean_abs_rgb_diff": 1.4, "note": "n", "finding": None,
            "finding_pin": None, **kw}


class RegressTest(unittest.TestCase):
    def test_equal_runs_are_all_identical(self):
        r = rg.regress([row(), row(label="b")], [row(), row(label="b")])
        self.assertEqual((r["identical"], r["changed"], r["missing"], r["pass_to_fail"], r["new"]), (2, [], [], [], 0))

    def test_a_pin_that_moves_while_the_score_holds_is_a_changed_row(self):
        """The Table/Data pin gained a field; the score, the verdict and the finding text are the same."""
        base, head = row(finding="why", finding_pin={"fg_render_px": [10, 10]}), row(finding="why", finding_pin={"fg_render_px": [10, 10], **PIN})
        r = rg.regress([base], [head])
        self.assertEqual((r["identical"], len(r["changed"])), (0, 1))
        self.assertEqual(r["changed"][0][1], {"finding_pin": ({"fg_render_px": [10, 10]}, {"fg_render_px": [10, 10], **PIN})})

    def test_a_pin_added_to_a_row_that_had_none_is_a_changed_row(self):
        r = rg.regress([row()], [row(finding_pin=PIN)])
        self.assertEqual([d for _, d in r["changed"]], [{"finding_pin": (None, PIN)}])

    def test_the_note_and_the_finding_text_are_words_and_never_a_change(self):
        r = rg.regress([row(note="a", finding="one", finding_pin=PIN)], [row(note="b", finding="two", finding_pin=PIN)])
        self.assertEqual((r["identical"], r["changed"]), (1, []))

    def test_a_changed_score_is_listed_with_both_numbers(self):
        r = rg.regress([row()], [row(mean_abs_rgb_diff=2.3)])
        self.assertEqual(r["changed"], [(("Table/Data", "Size=Sm", "k-variables", "[322,204,449,41]", None), {"mean_abs_rgb_diff": (1.4, 2.3)})])

    def test_a_base_row_missing_at_head_and_a_row_new_at_head_are_counted(self):
        r = rg.regress([row(), row(label="b")], [row(), row(label="c")])
        self.assertEqual((len(r["missing"]), r["new"], r["identical"]), (1, 1, 1))

    def test_a_row_that_passed_at_base_and_fails_at_head_is_a_pass_to_fail(self):
        r = rg.regress([row(), row(label="b", bad=True)], [row(bad=True), row(label="b", bad=False)])
        self.assertEqual(r["pass_to_fail"], [("Table/Data", "Size=Sm", "k-variables", "[322,204,449,41]", None)])

    def test_two_rows_with_one_key_are_refused(self):
        with self.assertRaises(ValueError):
            rg.regress([row(), row()], [row()])

    def test_the_label_and_the_icon_are_part_of_the_key(self):
        self.assertEqual(rg.regress([row(icon="a"), row(icon="b")], [row(icon="a"), row(icon="b")])["identical"], 2)


class CommandTest(unittest.TestCase):
    def run_main(self, base, head):
        with tempfile.TemporaryDirectory() as tmp:
            paths = []
            for name, rows in (("base.json", base), ("head.json", head)):
                paths.append(str(Path(tmp, name)))
                Path(tmp, name).write_text(json.dumps(rows))
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                code = rg.main(paths)
        return code, buf.getvalue()

    def test_a_changed_pin_is_printed_and_does_not_fail_the_run(self):
        code, text = self.run_main([row()], [row(finding_pin=PIN)])
        self.assertEqual(code, 0)
        self.assertIn("identical 0, changed 1", text)
        self.assertIn("finding_pin None -> {'mean_abs_rgb_diff': 1.42}", text)

    def test_a_missing_row_or_a_pass_to_fail_exits_1(self):
        self.assertEqual(self.run_main([row()], [])[0], 1)
        self.assertEqual(self.run_main([row()], [row(bad=True)])[0], 1)

    def test_equal_runs_exit_0(self):
        code, text = self.run_main([row()], [row()])
        self.assertEqual(code, 0)
        self.assertIn("rows passing at base but failing at head: 0", text)


class DispatchTest(unittest.TestCase):
    def test_evidence_regress_runs_the_differ_through_the_package_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name, rows in (("base.json", [row()]), ("head.json", [row(finding_pin=PIN)])):
                Path(tmp, name).write_text(json.dumps(rows))
            run = subprocess.run([sys.executable, "-B", str(HERE.parent / "studio_ui_file"), "evidence", "regress", str(Path(tmp, "base.json")), str(Path(tmp, "head.json"))],
                                 capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn("identical 0, changed 1", run.stdout)


if __name__ == "__main__":
    unittest.main()
