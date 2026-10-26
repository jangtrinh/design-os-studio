#!/usr/bin/env python3
"""Unit tests for scripts/check_chrome_colour_literals.py."""

from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_chrome_colour_literals as ccl  # noqa: E402


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
        """Spec A §10.7: injecting one Color32::from_rgb(1, 2, 3) into a temp copy fails."""
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

    def test_head_passes_against_committed_baseline(self) -> None:
        exit_code = ccl.main(
            ["--src-root", str(ccl.APP_SRC), "--baseline", str(ccl.BASELINE_PATH)]
        )
        self.assertEqual(exit_code, 0)


if __name__ == "__main__":
    unittest.main()
