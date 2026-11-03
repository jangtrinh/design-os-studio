"""The `scripts/studio_ui_file` CLI plan command and package guard rails (< 200 lines, < 200 chars, docstring indentation, Pillow isolation).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_package_guard
"""
import ast
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
PKG = SCRIPTS / "studio_ui_file"
GATE_MODULES = ["check_studio_ui_file.py"] + sorted(p.name for p in SCRIPTS.glob("studio_ui_*.py"))


class PlanCommandTest(unittest.TestCase):
    """`plan` is the advisory step's command: it must fail loudly, with a message and no traceback, on a theme.rs it cannot read."""

    def run_plan(self, theme_src):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "crates/designos-app/src"
            src.mkdir(parents=True)
            (src / "theme.rs").write_text(theme_src)
            (Path(tmp) / "scripts").symlink_to(SCRIPTS)
            return subprocess.run([sys.executable, "-B", str(PKG), "plan", "--repo", tmp], capture_output=True, text=True)

    def test_an_unresolved_constant_fails_loudly_and_names_it(self):
        out = self.run_plan("pub const BROKEN_ONE: f32 = MISSING * 2.0;\npub const FINE: f32 = 1.0;\n")
        self.assertNotEqual(out.returncode, 0)
        self.assertIn("BROKEN_ONE", out.stderr)
        self.assertNotIn("Traceback", out.stderr)

    def test_a_skip_that_names_no_real_constant_fails_loudly(self):
        out = self.run_plan("pub const FINE: f32 = 1.0;\n")  # none of the SKIPPED constants exist in this theme
        self.assertNotEqual(out.returncode, 0)
        self.assertIn("stale SKIPPED", out.stderr)
        self.assertNotIn("Traceback", out.stderr)


class PackageGuardTest(unittest.TestCase):
    def test_every_script_of_the_package_and_the_gate_is_at_most_200_lines(self):
        files = [p for p in PKG.rglob("*.py")] + [SCRIPTS / n for n in GATE_MODULES] + sorted((SCRIPTS / "tests").glob("test_studio_ui_*.py"))
        self.assertGreater(len(files), 15)
        long = {p.name: len(p.read_text().splitlines()) for p in files if len(p.read_text().splitlines()) >= 200}
        self.assertEqual(long, {})

    def test_regions_screens_and_ir_files_keep_lines_under_200_chars_and_docstrings_indented(self):
        files = (sorted((PKG / "components").glob("regions_*.py")) + sorted((PKG / "screens").glob("*.py"))
                 + sorted(PKG.glob("rest_*.py")) + sorted((PKG / "evidence").glob("screen_*.py"))
                 + sorted((SCRIPTS / "tests").glob("test_studio_ui_file_regions_*.py"))
                 + sorted((SCRIPTS / "tests").glob("test_studio_ui_*screen*.py")))
        over = [(p.name, i, len(l)) for p in files for i, l in enumerate(p.read_text().splitlines(), 1) if len(l) >= 200]
        self.assertEqual(over, [])
        bad_doc = []
        for p in files:
            lines = p.read_text().splitlines()
            for node in ast.walk(ast.parse(p.read_text())):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.body:
                    first = node.body[0]
                    if isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str):
                        for ln in range(first.lineno + 1, first.end_lineno + 1):
                            if lines[ln - 1].strip() and len(lines[ln - 1]) - len(lines[ln - 1].lstrip(" ")) < node.col_offset + 4:
                                bad_doc.append((p.name, ln))
        self.assertEqual(bad_doc, [])

    def test_only_the_evidence_commands_may_import_pillow(self):
        offenders = [p.name for p in PKG.rglob("*.py") if re.search(r"^\s*(import|from)\s+PIL", p.read_text(), re.M) and "evidence" not in p.parts]
        self.assertEqual(offenders, [])

    def test_the_package_runs_by_path_and_lists_its_commands(self):
        out = subprocess.run([sys.executable, "-B", str(PKG), "--help"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stderr)
        for command in ("plan", "foundation", "check", "evidence"):
            self.assertIn(command, out.stdout)

    def test_the_check_command_is_the_gate(self):
        out = subprocess.run([sys.executable, "-B", str(PKG), "check", "--help"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertIn("--allowlist", out.stdout)

    def test_the_components_folder_exists_for_the_next_wave(self):
        self.assertTrue((PKG / "components" / "__init__.py").is_file())


if __name__ == "__main__":
    unittest.main()
