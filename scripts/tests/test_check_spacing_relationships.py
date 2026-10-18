"""Proves the spacing ratchet passes on clean code and fails on each violation.

Run: python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
"""

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
SCRIPT = SCRIPTS / "check_spacing_relationships.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures" / "spacing"
EMPTY_BASELINE = FIXTURES / "baseline_empty.json"


def run(src: Path, baseline: Path, *extra: str):
    return subprocess.run(
        [sys.executable, "-B", str(SCRIPT), "--src", str(src), "--baseline", str(baseline), *extra],
        capture_output=True,
        text=True,
    )


def report(src: Path, baseline: Path):
    proc = run(src, baseline, "--json")
    return proc, json.loads(proc.stdout)


class SpacingRatchetTest(unittest.TestCase):
    def test_clean_tree_passes_its_baseline(self):
        proc, rep = report(FIXTURES / "clean", FIXTURES / "baseline_clean.json")
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertTrue(rep["ok"])
        self.assertEqual(rep["totals"]["nonconcentric_pairs"], 0)  # concentric pair is not counted
        self.assertEqual(rep["totals"]["inset_literals"], 1)  # its shrink(8.0)

    def test_dirty_tree_fails_in_every_category(self):
        proc, rep = report(FIXTURES / "dirty", EMPTY_BASELINE)
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        self.assertFalse(rep["ok"])
        got = {(v["file"], v["category"]) for v in rep["violations"]}
        for expected in (
            ("bad_offset.rs", "offset_literals"),
            ("bad_inset.rs", "inset_literals"),
            ("bad_radius.rs", "radius_literals"),
            ("bad_font.rs", "font_literals"),
            ("bad_pair.rs", "nonconcentric_pairs"),
        ):
            self.assertIn(expected, got)
        self.assertNotIn("theme.rs", {f for f, _ in got})  # theme.rs may hold literals

    def test_matching_baseline_passes_and_one_extra_literal_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            src, baseline = tmp / "src", tmp / "baseline.json"
            shutil.copytree(FIXTURES / "dirty", src)
            self.assertEqual(run(src, baseline, "--write-baseline").returncode, 0)
            self.assertEqual(run(src, baseline).returncode, 0)
            with open(src / "bad_offset.rs", "a", encoding="utf-8") as fh:
                fh.write("fn more(rect: Rect) -> f32 {\n    rect.min.y + 4.0\n}\n")
            proc, rep = report(src, baseline)
            self.assertEqual(proc.returncode, 1)
            self.assertEqual(
                rep["violations"],
                [{"file": "bad_offset.rs", "category": "offset_literals", "baseline": 1, "current": 2}],
            )

    def test_new_file_with_a_literal_fails_even_when_totals_would_fall(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            src, baseline = tmp / "src", tmp / "baseline.json"
            shutil.copytree(FIXTURES / "dirty", src)
            run(src, baseline, "--write-baseline")
            (src / "bad_offset.rs").write_text("fn place() {}\n", encoding="utf-8")  # one literal leaves
            (src / "fresh.rs").write_text("fn p(r: Rect) -> f32 { r.min.x + 6.0 }\n", encoding="utf-8")
            proc, rep = report(src, baseline)
            self.assertEqual(proc.returncode, 1)
            self.assertEqual([v["file"] for v in rep["violations"]], ["fresh.rs"])

    def test_write_baseline_refuses_to_raise_a_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            src, baseline = tmp / "src", tmp / "baseline.json"
            shutil.copytree(FIXTURES / "dirty", src)
            run(src, baseline, "--write-baseline")
            before = baseline.read_text(encoding="utf-8")
            with open(src / "bad_font.rs", "a", encoding="utf-8") as fh:
                fh.write("fn b() -> FontId {\n    FontId::proportional(13.0)\n}\n")
            proc = run(src, baseline, "--write-baseline")
            self.assertEqual(proc.returncode, 1)
            self.assertEqual(baseline.read_text(encoding="utf-8"), before)

    def test_missing_baseline_is_a_usage_error_not_a_pass(self):
        proc = run(FIXTURES / "clean", FIXTURES / "does-not-exist.json")
        self.assertEqual(proc.returncode, 2)

    def test_comments_and_tests_are_not_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "src"
            src.mkdir()
            (src / "a.rs").write_text(
                '// rect.min.x + 12.0\nfn f() { let s = "x + 12.0"; }\n'
                "#[cfg(test)]\nmod tests {\n    fn t(r: Rect) -> f32 { r.min.x + 9.0 }\n}\n",
                encoding="utf-8",
            )
            proc, rep = report(src, EMPTY_BASELINE)
            self.assertEqual(proc.returncode, 0, proc.stdout)
            self.assertEqual(sum(rep["totals"].values()), 0)


if __name__ == "__main__":
    unittest.main()
