#!/usr/bin/env python3
"""Unit tests for scripts/check_chrome_colour_literals.py."""

from __future__ import annotations

import contextlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_chrome_colour_literals as ccl  # noqa: E402
import design_os_self_evolve as dse  # noqa: E402


class CheckChromeColourLiteralsTests(unittest.TestCase):
    def test_ignores_comments_strings_and_cfg_test(self) -> None:
        src = """
        // Color32::from_rgb(1, 2, 3) in a line comment
        /* Color32::WHITE in a block comment */
        let s = "Color32::from_rgb(4, 5, 6)";
        let transparent = Color32::TRANSPARENT;
        let computed = Color32::from_rgb(r, g, b);

        #[cfg(test)]
        mod tests {
            fn fixture() {
                let _ = Color32::from_rgb(9, 9, 9);
                let _ = Color32::WHITE;
            }
        }
        """
        self.assertEqual(ccl.scan_source(src), [])

    def test_detects_literal_constructors_and_named_colours(self) -> None:
        src = """
        fn paint() {
            let a = Color32::from_rgb(1, 2, 3);
            let b = Color32::from_rgba_unmultiplied(0x18, 0x18, 0x1B, 0xFF);
            let c = Color32::from_gray(40);
            let d = Color32::from_black_alpha(128);
            let e = Color32::WHITE;
        }
        """
        hits = ccl.scan_source(src)
        self.assertEqual(len(hits), 5)

    def test_injected_color32_literal_in_temp_copy_fails_against_baseline(self) -> None:
        """Injecting one Color32::from_rgb(1, 2, 3) into a temp copy of the source tree fails the gate."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp_src = Path(tmp) / "src"
            shutil.copytree(ccl.APP_SRC, tmp_src)
            target = tmp_src / "tabs.rs"
            target.write_text(
                target.read_text(encoding="utf-8")
                + "\npub fn _injected_chrome_literal() -> egui::Color32 { egui::Color32::from_rgb(1, 2, 3) }\n",
                encoding="utf-8",
            )
            exit_code = ccl.main(
                ["--src-root", str(tmp_src), "--baseline", str(ccl.BASELINE_PATH)]
            )
            self.assertEqual(exit_code, 1)

    def test_new_file_not_in_baseline_with_color32_literal_fails(self) -> None:
        """A temp .rs file not in the baseline containing Color32::from_rgb(1, 2, 3) makes the checker exit 1."""
        baseline_files = json.loads(ccl.BASELINE_PATH.read_text(encoding="utf-8")).get(
            "files", {}
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_src = Path(tmp) / "src"
            shutil.copytree(ccl.APP_SRC, tmp_src)
            new_rel = "unbaselined_chrome_surface.rs"
            self.assertNotIn(new_rel, baseline_files)
            new_file = tmp_src / new_rel
            new_file.write_text(
                "pub fn _new_file_chrome_literal() -> egui::Color32 {\n"
                "    Color32::from_rgb(1, 2, 3)\n"
                "}\n",
                encoding="utf-8",
            )
            exit_code = ccl.main(
                ["--src-root", str(tmp_src), "--baseline", str(ccl.BASELINE_PATH)]
            )
            self.assertEqual(exit_code, 1)

    def test_head_passes_against_committed_baseline(self) -> None:
        exit_code = ccl.main(
            ["--src-root", str(ccl.APP_SRC), "--baseline", str(ccl.BASELINE_PATH)]
        )
        self.assertEqual(exit_code, 0)

    def test_g04_is_report_only_when_a11y_chrome_pairs_missing(self) -> None:
        """G04 stays supplementary (passed) and names the missing pairs file."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp_root = Path(tmp)
            (tmp_root / "design").mkdir(parents=True, exist_ok=True)
            with mock.patch.object(dse, "REPO_ROOT", tmp_root):
                ok, summary, _ = dse.check_ui_ds_a11y()
            self.assertTrue(ok)
            self.assertIn("supplementary", summary)
            self.assertIn("design/a11y-chrome-pairs.txt", summary)

    def test_g04_is_report_only_when_a11y_chrome_pairs_empty(self) -> None:
        """G04 stays supplementary (passed) and names an empty pairs file."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp_root = Path(tmp)
            pairs_path = tmp_root / "design" / "a11y-chrome-pairs.txt"
            pairs_path.parent.mkdir(parents=True, exist_ok=True)
            pairs_path.write_text("   \n", encoding="utf-8")
            with mock.patch.object(dse, "REPO_ROOT", tmp_root):
                ok, summary, _ = dse.check_ui_ds_a11y()
            self.assertTrue(ok)
            self.assertIn("supplementary", summary)
            self.assertIn("design/a11y-chrome-pairs.txt", summary)

    def test_run_cli_reports_a_missing_command_instead_of_raising(self) -> None:
        """A runner without the `ui` CLI must get a non-zero code and a note, never an exception."""
        code, out, err = dse.run_cli(["no-such-binary-for-the-design-gates"])
        self.assertEqual(code, 127)
        self.assertEqual(out, "")
        self.assertIn("not runnable", err)
        self.assertIn("no-such-binary-for-the-design-gates", err)

    def test_g04_stays_passed_and_names_the_tool_when_ui_is_not_installed(self) -> None:
        """G04 is report-only: with no `ui` on PATH it still passes and says why it measured nothing."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp_root = Path(tmp)
            (tmp_root / "design").mkdir(parents=True, exist_ok=True)
            (tmp_root / "design" / "a11y-chrome-pairs.txt").write_text(
                "colors.bg-foreground:colors.bg\n", encoding="utf-8"
            )
            with mock.patch.object(dse, "REPO_ROOT", tmp_root), mock.patch.object(
                dse, "build_env", return_value={"PATH": str(tmp_root / "no-bin")}
            ):
                ok, summary, _ = dse.check_ui_ds_a11y()
        self.assertTrue(ok)
        self.assertIn("supplementary", summary)
        self.assertIn("not runnable", summary)

    def test_json_report_on_stdout_is_valid_json(self) -> None:
        """`--json` is the machine-readable form: G01's progress lines go to stderr, stdout parses."""
        stub = mock.Mock(return_value=(True, "stub", {}))
        patched = {
            name: stub
            for name in (
                "check_zero_fake_ui_gate",
                "check_ui_ds_status",
                "check_ui_ds_a11y",
                "check_ui_ds_soul",
                "check_ui_agents",
                "check_ui_gate_specimen",
                "check_design_os_project_audit",
                "run_self_evolution_pulse",
            )
        }
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.multiple(dse, **patched), mock.patch.object(
            sys, "argv", ["design_os_self_evolve.py", "--check", "--json"]
        ), contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            exit_code = dse.main()
        self.assertEqual(exit_code, 0)
        report = json.loads(out.getvalue())
        self.assertEqual(report["total"], 9)
        self.assertIn("purple-band hits: 0", err.getvalue())

    def test_purple_band_flags_the_retired_syntax_keyword(self) -> None:
        """A Color32 constant with hue 255-300 deg and HSL S > 0.25 is flagged; BOARD_* is exempt."""
        hits = dse.purple_band_constants(
            "pub const SYNTAX_KEYWORD: Color32 = rgb(0xC792EA);\n"
            "pub const BOARD_SWATCH: Color32 = rgb(0xD5C3FF);\n"
        )
        self.assertEqual(len(hits), 1)
        self.assertIn("SYNTAX_KEYWORD", hits[0])
        self.assertIn("#C792EA", hits[0])

    def test_rust_constant_parser_catches_explicit_from_rgb(self) -> None:
        """The Purple Ban must cover valid Color32::from_rgb constructor syntax."""
        theme = "pub const BANNED: Color32 = Color32::from_rgb(0x97, 0x47, 0xFF);\n"
        self.assertEqual(maybe := dse.parse_rust_theme_constants(theme), {"BANNED": "#9747ff"})
        self.assertIn("#9747ff", maybe.values())

    def test_rust_constant_parser_accepts_decimal_and_mixed_channels(self) -> None:
        theme = "pub const DECIMAL: Color32 = Color32::from_rgb(151, 71, 255);\n"
        mixed = "pub const MIXED: Color32 = Color32::from_rgb(0x97, 71, 0xFF);\n"
        self.assertEqual(dse.parse_rust_theme_constants(theme + mixed), {"DECIMAL": "#9747ff", "MIXED": "#9747ff"})
        self.assertEqual(len(dse.purple_band_constants(theme + mixed)), 2)

    def test_rust_constant_parser_accepts_leading_zero_decimal_channels(self) -> None:
        theme = "pub const RGB: Color32 = Color32::from_rgb(001, 008, 003);\n"
        self.assertEqual(dse.parse_rust_theme_constants(theme), {"RGB": "#010803"})

    def test_purple_band_covers_rgba_literals(self) -> None:
        theme = "pub const BANNED: Color32 = rgba(0x9747FF, 0xFF);\n"
        self.assertEqual(dse.parse_rust_theme_constants(theme), {"BANNED": "#9747ff"})
        # The alpha argument is separate; the RGB literal remains the audited colour.
        self.assertEqual(dse.purple_band_constants(theme), ["BANNED #9747FF hue 266 deg, HSL S 1.00"])

    def test_purple_band_on_theme_rs_is_empty(self) -> None:
        theme = (SCRIPTS_DIR.parent / "crates/designos-app/src/theme.rs").read_text(encoding="utf-8")
        self.assertEqual(dse.purple_band_constants(theme), [])


if __name__ == "__main__":
    unittest.main()
