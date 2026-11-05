"""Text styles of the Studio UI file (S3): one per type role of theme.rs, bound by constant, not by value.

`Text sm / Semibold` is `font_text(FONT_SIZE_TEXT_SM, Semibold)` (every kit button label, `kit/button.rs`) and `Text xs / Medium` is
`font_text(FONT_SIZE_TEXT_XS, Medium)` (every kit badge, tag and status label, `kit/badge.rs`). Synthetic plan only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_styles
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import text_styles_plan as styles  # noqa: E402


def plan():
    names = {n for r in styles.ROLES for n in (r[2], r[3], r[4], r[5]) if n}
    return {"semantic": [{"name": n, "id": f"id:{n}", "value": 400.0 if "weight" in n else ("Inter" if "family" in n else 12.0)} for n in sorted(names)]}


class TextStyleTest(unittest.TestCase):
    def setUp(self):
        self.styles, self.bindings = styles.build(plan())
        self.by_name = {s["name"]: s for s in self.styles}

    def test_text_sm_semibold_binds_the_text_sm_variables_not_large(self):
        self.assertEqual(self.bindings["Text sm / Semibold"], {"fontSize": "id:text/text-sm", "lineHeight": "id:text/line-text-sm",
                                                                "fontWeight": "id:text/weight-strong", "fontFamily": "id:text/font-family-ui"})
        self.assertEqual(self.by_name["Text sm / Semibold"]["description"], "theme.rs font_text(FONT_SIZE_TEXT_SM, Semibold)")

    def test_text_xs_medium_binds_the_text_xs_variables_not_caption(self):
        self.assertEqual(self.bindings["Text xs / Medium"], {"fontSize": "id:text/text-xs", "lineHeight": "id:text/line-text-xs",
                                                              "fontWeight": "id:text/weight-medium", "fontFamily": "id:text/font-family-ui"})

    def test_large_semibold_keeps_font_size_large(self):
        self.assertEqual(self.bindings["Large / Semibold"]["fontSize"], "id:text/large")

    def test_mono_caption_binds_the_caption_variables_in_the_mono_family(self):
        """Progress counts are FontId::new(FONT_SIZE_CAPTION, Monospace) (panels/check_designs.rs:569): the style binds the constant the Rust names."""
        self.assertEqual(self.bindings["Mono / Caption"], {"fontSize": "id:text/caption", "lineHeight": "id:text/line-caption",
                                                            "fontWeight": "id:text/weight-mono", "fontFamily": "id:text/font-family-mono"})
        self.assertEqual(self.bindings["Mono / Value"]["fontSize"], "id:text/body", "font_mono is FONT_SIZE_BODY")

    def test_a_regular_text_sm_style_exists_because_a_set_uses_it(self):
        """Text sm Regular is used by the Home sidebar row (home/mod.rs:1237-1241), the Home search field and the dialog field (home/dialog.rs:246)."""
        self.assertEqual(self.bindings["Text sm / Regular"], {"fontSize": "id:text/text-sm", "lineHeight": "id:text/line-text-sm",
                                                              "fontWeight": "id:text/weight-regular", "fontFamily": "id:text/font-family-ui"})
        self.assertEqual(self.by_name["Text sm / Regular"]["description"], "theme.rs font_text(FONT_SIZE_TEXT_SM, Regular)")

    def test_text_xs_regular_and_semibold_bind_the_text_xs_variables_not_caption(self):
        """Text xs Regular is the Card/Library meta and the starter card description, Semibold the starter card badge and the Tabs/Segmented label (`label_font`, kit/tabs.rs:64-66)."""
        for label, weight in (("Regular", "regular"), ("Semibold", "strong")):
            self.assertEqual(self.bindings[f"Text xs / {label}"], {"fontSize": "id:text/text-xs", "lineHeight": "id:text/line-text-xs",
                                                                   "fontWeight": f"id:text/weight-{weight}", "fontFamily": "id:text/font-family-ui"})
            self.assertEqual(self.by_name[f"Text xs / {label}"]["description"], f"theme.rs font_text(FONT_SIZE_TEXT_XS, {label})")

    def test_text_sm_medium_binds_the_text_sm_variables_at_the_medium_weight(self):
        """The toast message is `font_text(FONT_SIZE_TEXT_SM, Medium)` (shell.rs:1240 on v0.7.5): 14 / 20 at weight 500, not Regular's 400."""
        self.assertEqual(self.bindings["Text sm / Medium"], {"fontSize": "id:text/text-sm", "lineHeight": "id:text/line-text-sm",
                                                             "fontWeight": "id:text/weight-medium", "fontFamily": "id:text/font-family-ui"})
        self.assertEqual(self.by_name["Text sm / Medium"]["description"], "theme.rs font_text(FONT_SIZE_TEXT_SM, Medium)")

    def test_ruler_regular_binds_the_ruler_size_with_no_line_height(self):
        """The rulers' tick labels are `font_ruler` (canvas/rulers.rs:211, theme.rs:793): `RULER_FONT_SIZE` (an alias of FONT_SIZE_CANVAS), proportional, regular, the font's own line."""
        self.assertEqual(self.bindings["Ruler / Regular"], {"fontSize": "id:text/ruler", "fontWeight": "id:text/weight-regular", "fontFamily": "id:text/font-family-ui"})
        self.assertEqual(self.by_name["Ruler / Regular"]["description"], "theme.rs font_ruler")
        self.assertEqual(self.by_name["Ruler / Regular"]["text_style"]["line_height"], "Auto")

    def test_text_xl_semibold_binds_the_text_xl_variables_not_title(self):
        """The Home breadcrumb is `font_text(FONT_SIZE_TEXT_XL, Semibold)` (home/mod.rs:1476): 20 / 30 / Semibold by the constants it names, as `Title` is by `FONT_SIZE_TITLE`."""
        self.assertEqual(self.bindings["Text xl / Semibold"], {"fontSize": "id:text/text-xl", "lineHeight": "id:text/line-text-xl",
                                                               "fontWeight": "id:text/weight-strong", "fontFamily": "id:text/font-family-ui"})
        self.assertEqual(self.by_name["Text xl / Semibold"]["description"], "theme.rs font_text(FONT_SIZE_TEXT_XL, Semibold)")

    def test_text_xl_semibold_takes_its_size_and_line_from_theme_rs(self):
        import variables_plan  # noqa: E402  (the real plan: every value is read from theme.rs)
        real = variables_plan.build_plan(str(HERE.parents[1]))
        built = {s["name"]: s["text_style"] for s in styles.build(real)[0]}["Text xl / Semibold"]
        self.assertEqual((built["font_size"], built["line_height"], built["font_weight"]), (20.0, {"Pixels": 30.0}, 600))


if __name__ == "__main__":
    unittest.main()
