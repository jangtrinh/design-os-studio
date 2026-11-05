"""A text style with no line variable (`Ruler / Regular`, `Canvas / *`: the font's own line, `line_height` "Auto") lowers to a REST text whose `lineHeightUnit` is `INTRINSIC_%` and carries no
`lineHeightPx` (designos-fig rest_import.rs:342-360 reads any other unit as Auto); a style with a line keeps `PIXELS` and its px. Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_text_line_auto
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES as BODY  # noqa: E402

AUTO = {"font_family": "Inter", "font_weight": 400, "font_size": 9.0, "line_height": "Auto"}
STYLES = BODY + [{"name": "Ruler / Regular", "text_style": AUTO}]


def style_of(style):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    return low.lower(ir.F("Box", [ir.T("Label", "5000", style, "color/line")], dir="V"), 0, 0, ptype="COMPONENT")["children"][0]["style"]


class LineAutoTest(unittest.TestCase):
    def test_a_style_with_no_line_is_intrinsic_and_has_no_px(self):
        s = style_of("Ruler / Regular")
        self.assertEqual(s["lineHeightUnit"], "INTRINSIC_%")
        self.assertNotIn("lineHeightPx", s)
        self.assertEqual((s["fontSize"], s["fontWeight"]), (9.0, 400))

    def test_a_style_with_a_line_keeps_its_pixels(self):
        s = style_of("Body / Medium")
        self.assertEqual((s["lineHeightUnit"], s["lineHeightPx"]), ("PIXELS", 18.0))


if __name__ == "__main__":
    unittest.main()
