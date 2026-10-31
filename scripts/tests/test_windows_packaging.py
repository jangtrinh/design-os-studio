"""Regression checks for Windows PowerShell 5.1 packaging compatibility."""
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[2]
POWERSHELL_SCRIPTS = (
    REPO_ROOT / "packaging/windows/install.ps1",
    REPO_ROOT / "packaging/windows/uninstall.ps1",
)


class WindowsPackagingTests(unittest.TestCase):
    def test_powerShell_scripts_are_ascii_source(self):
        """Windows PowerShell 5.1 parses scripts using the system code page."""
        for script in POWERSHELL_SCRIPTS:
            raw = script.read_bytes()
            self.assertTrue(raw.isascii(), f"{script} must remain ASCII source for PowerShell 5.1")

    def test_installer_constructs_unicode_shortcut_description_at_runtime(self):
        installer = (REPO_ROOT / "packaging/windows/install.ps1").read_text()
        self.assertIn('[char]0x2014', installer)
        self.assertNotIn("—", installer)


if __name__ == "__main__":
    unittest.main()
