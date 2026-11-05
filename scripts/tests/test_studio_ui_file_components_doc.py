"""The component package's module docstring is the authoring contract of every set module: the rules a module needs to know before it draws are written there.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_components_doc
"""
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import components  # noqa: E402


class AuthoringContractDoc(unittest.TestCase):
    def text(self):
        return re.sub(r"\s+", " ", components.__doc__)

    def test_an_abs_placed_auto_layout_frame_with_children_needs_fixed_sizing(self):
        """Found building Dialog/CommandPalette: the abs-placed `Image button` frame hugged its 14 px glyph, which sat at the frame's corner, 6 px up and 7 px left of the capture."""
        self.assertIn("An abs-placed auto-layout frame with children needs fixed sizing.", self.text())


if __name__ == "__main__":
    unittest.main()
