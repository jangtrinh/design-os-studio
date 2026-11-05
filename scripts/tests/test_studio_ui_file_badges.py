"""Badge/StatusCount against `kit::count_badge` (Sm, widgets/kit/badge.rs:31-38, 75-80) and `kit::status_tag` (Md, :68-73), synthetic env: no theme.rs, kit-states.json or MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_badges
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402

NUM = {"BADGE_SM": 22.0, "BADGE_MD": 24.0}
TONES = ("Neutral", "Success", "Warning", "Error")


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)


SET = {s["name"]: s for s in components.discover()}["Badge/StatusCount"]


def build(size="Sm", tone="Neutral"):
    return SET["build"]({"Size": size, "Tone": tone, "State": "Default"}, FakeEnv())


class BadgeTest(unittest.TestCase):
    def test_the_set_is_the_registry_matrix_in_the_badges_row(self):
        self.assertEqual(SET["row"], "Badges & Status")
        self.assertEqual(len(components.matrix(SET["axes"])), 8)

    def test_sm_is_the_count_badge_pill_22_high(self):
        """`kit::count_badge` (badge.rs:75-80): `radius_full(height)`, padded by the caller (Home: SPACE_FACT each side)."""
        for tone in ("Neutral", "Success"):
            node = build("Sm", tone)
            self.assertEqual((node["h"], node["radius"][:2]), (22.0, ("lit", 11.0)), tone)
            self.assertEqual(ir.pads(node), [None, "SPACE_FACT", None, "SPACE_FACT"], tone)

    def test_md_is_the_status_tag_chip_24_high_with_the_control_radius(self):
        """`kit::status_tag` (badge.rs:68-73), the Libraries dialog's New / Changed / Removed chips (libraries/modal.rs:136-139): BADGE_MD high,
        RADIUS_CONTROL, text + 2 * SPACE_INLINE wide."""
        for tone in TONES:
            node = build("Md", tone)
            self.assertEqual((node["h"], node["radius"]), (24.0, "RADIUS_CONTROL"), tone)
            self.assertEqual(ir.pads(node), [None, "SPACE_INLINE", None, "SPACE_INLINE"], tone)

    def test_each_tone_is_its_tinted_triple(self):
        want = {"Neutral": ("BADGE_NEUTRAL_BG", "BADGE_NEUTRAL_BORDER", "BADGE_NEUTRAL_TEXT"),
                "Success": ("BG_SUCCESS_PRIMARY", "BADGE_SUCCESS_BORDER", "BADGE_SUCCESS_TEXT"),
                "Warning": ("BG_WARNING_PRIMARY", "BADGE_WARNING_BORDER", "BADGE_WARNING_TEXT"),
                "Error": ("BG_ERROR_PRIMARY", "BADGE_ERROR_BORDER", "BADGE_ERROR_TEXT")}
        for tone, (fill, border, text) in want.items():
            node = build(tone=tone)
            label = node["kids"][0]
            self.assertEqual((node["fill"], node["stroke"], label["color"]), (fill, border, text), tone)
            self.assertEqual(node["sw"], "BORDER_WIDTH")

    def test_the_label_is_text_xs_medium_at_both_sizes(self):
        for size in ("Sm", "Md"):
            node = build(size)
            self.assertEqual((node["kids"][0]["style"], node["kids"][0]["prop"]), ("Text xs / Medium", "Label"), size)
        self.assertEqual((node["sx"], node["sy"]), ("hug", "fixed"))


if __name__ == "__main__":
    unittest.main()
