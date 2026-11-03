"""`T(..., align_h="left"|"center"|"right"|"justified")` writes the text's `textAlignHorizontal` (designos-fig rest_import.rs:306-312 reads LEFT, CENTER, RIGHT, JUSTIFIED; a missing key is left). A text
without it carries no key, so every text drawn before this keyword is byte-identical. Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_text_align
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402


def lower(**kw):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Body": "Body#set0:0"}
    return low.lower(ir.F("Box", [ir.T("Body", "A long body that wraps onto a second line", "Body / Medium", "color/line", prop="Body", **kw)], dir="V", w=200, sx="fixed"), 0, 0, ptype="COMPONENT")["children"][0]


class AlignTest(unittest.TestCase):
    def test_each_alignment_is_the_rest_enum(self):
        for word, rest in (("left", "LEFT"), ("center", "CENTER"), ("right", "RIGHT"), ("justified", "JUSTIFIED")):
            with self.subTest(word=word):
                self.assertEqual(lower(align_h=word)["style"]["textAlignHorizontal"], rest)

    def test_a_text_without_it_carries_no_key(self):
        self.assertNotIn("textAlignHorizontal", lower()["style"])

    def test_a_wrapping_text_keeps_its_width_and_auto_height_with_it(self):
        t = lower(align_h="center", wrap=True, w=120)
        self.assertEqual((t["style"]["textAutoResize"], t["absoluteBoundingBox"]["width"], t["style"]["textAlignHorizontal"]), ("HEIGHT", 120, "CENTER"))

    def test_an_unknown_alignment_fails_loudly(self):
        for bad in ("middle", "CENTER", "", None, 1):
            with self.assertRaises(ValueError, msg=bad):
                ir.T("Body", "x", "Body / Medium", "color/line", align_h=bad)

    def test_the_other_style_fields_are_untouched(self):
        plain, centred = lower(), lower(align_h="center")
        self.assertEqual({k: v for k, v in centred["style"].items() if k != "textAlignHorizontal"}, plain["style"])


if __name__ == "__main__":
    unittest.main()
