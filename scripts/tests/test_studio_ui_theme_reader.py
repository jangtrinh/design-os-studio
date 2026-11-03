"""theme.rs reader: resolves aliases, arithmetic and colours; fails loudly on anything it cannot resolve.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_theme_reader
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import studio_ui_theme_reader as theme  # noqa: E402


def read(src):
    return theme.read_theme_constants(src)


class ResolveTest(unittest.TestCase):
    def test_aliases_expressions_alpha_and_premultiplied_colours_resolve(self):
        c = read("pub const A: f32 = 8.0;\npub const B: f32 = A * 0.5;\npub const C: Color32 = rgba(0xFFFFFF, 0x1A);\n"
                 "pub const D: Color32 = Color32::from_rgba_premultiplied(4, 49, 81, 89);\npub const E: Color32 = rgb(0x0A0A0A);\n"
                 "pub const F: [Color32; 2] = [C, E];\n")
        self.assertEqual((c["B"].value, c["C"].value["a"], c["D"].value), (4.0, 26, {"r": 11, "g": 140, "b": 232, "a": 89}))
        self.assertEqual([r["name"] for r in theme.flatten(c) if r["const"] == "F"], ["F[0]", "F[1]"])

    def test_unresolvable_constant_fails_loudly(self):
        with self.assertRaises(theme.ThemeError) as ctx:
            read("pub const A: f32 = MISSING * 2.0;\npub const B: f32 = 1.0;\n")
        self.assertIn("A", str(ctx.exception))

    def test_integer_constants_use_integer_division(self):
        c = read("pub const A: u8 = 7;\npub const B: u8 = A / 2;\npub const C: f32 = 7.0 / 2.0;\n")
        self.assertEqual((c["B"].value, c["C"].value), (3, 3.5))

    def test_pub_crate_constants_are_read(self):
        self.assertEqual(read("pub(crate) const A: f32 = 2.0;\n")["A"].value, 2.0)

    def test_an_indented_or_nested_pub_const_is_reported_not_skipped(self):
        with self.assertRaises(theme.ThemeError) as ctx:
            read("pub const A: f32 = 1.0;\nmod inner {\n    pub const B: f32 = 2.0;\n}\n")
        self.assertIn("B", str(ctx.exception))


class U32ColourTest(unittest.TestCase):
    def test_a_u32_whose_doc_comment_says_colour_is_a_colour(self):
        c = read("/// Canvas colour of a board.\npub const BOARD_X: u32 = 0xF4F4F4;\n")
        row = theme.flatten(c)[0]
        self.assertEqual((row["kind"], row["value"]["r"]), ("colour", 0xF4))

    def test_a_u32_array_documented_as_0xrrggbb_is_a_colour_list(self):
        c = read("/// Palette (`0xRRGGBB`).\npub const PAL: [u32; 2] = [0xFF0000, 0x00FF00];\n")
        self.assertEqual([r["kind"] for r in theme.flatten(c)], ["colour", "colour"])

    def test_an_undecidable_u32_fails_loudly(self):
        with self.assertRaises(theme.ThemeError) as ctx:
            read("pub const FLAGS: u32 = 0x0F;\n")
        self.assertIn("FLAGS", str(ctx.exception))


# The real theme.rs is deliberately not read here: whether it resolves, and whether every skip names a real constant, is
# the advisory `python3 scripts/studio_ui_file plan` step of .github/workflows/design-gates.yml, so a theme.rs edit
# can never turn the default suite (which release automation runs) red.


if __name__ == "__main__":
    unittest.main()
