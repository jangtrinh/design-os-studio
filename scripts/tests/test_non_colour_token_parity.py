"""Non-colour token parity: passes on agreement, fails on injected drift.

Run: python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
"""

import json
import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
from non_colour_token_parity import KNOWN_DRIFT, TOKEN_TO_RUST, check_non_colour_parity  # noqa: E402

REPO = SCRIPTS.parent
THEME = (REPO / "crates/designos-app/src/theme.rs").read_text(encoding="utf-8")
TOKENS = json.loads((REPO / "design/design.tokens.json").read_text(encoding="utf-8"))

MAP = {"radius.radius-xs": ["RADIUS_SMALL"], "spacing.rail-width": ["RAIL_WIDTH"]}


def tokens(radius_xs="2px", rail="56px"):
    return {
        "radius": {"radius-xs": {"$type": "dimension", "$value": radius_xs}},
        "spacing": {"rail-width": {"$type": "dimension", "$value": rail}},
    }


SRC = "pub const RADIUS_SMALL: u8 = 2;\npub const RAIL_WIDTH: f32 = 56.0;\n"


class NonColourParityTest(unittest.TestCase):
    def test_agreement_passes(self):
        r = check_non_colour_parity(SRC, tokens(), MAP, {})
        self.assertTrue(r["ok"], r)
        self.assertEqual(len(r["verified"]), 2)

    def test_rust_drift_fails(self):
        r = check_non_colour_parity(SRC.replace("= 2;", "= 3;"), tokens(), MAP, {})
        self.assertFalse(r["ok"])
        self.assertIn("RADIUS_SMALL (3) != radius.radius-xs (2)", r["drifts"])

    def test_token_drift_fails(self):
        r = check_non_colour_parity(SRC, tokens(rail="57px"), MAP, {})
        self.assertFalse(r["ok"])
        self.assertTrue(any("RAIL_WIDTH" in d for d in r["drifts"]))

    def test_alias_or_expression_constant_fails(self):
        r = check_non_colour_parity("pub const RADIUS_SMALL: u8 = BASE * 2;\npub const RAIL_WIDTH: f32 = 56.0;\n", tokens(), MAP, {})
        self.assertFalse(r["ok"])
        self.assertTrue(any("RADIUS_SMALL" in p for p in r["problems"]))

    def test_missing_token_fails(self):
        r = check_non_colour_parity(SRC, {"radius": {}, "spacing": {}}, MAP, {})
        self.assertFalse(r["ok"])
        self.assertEqual(len(r["problems"]), 2)

    def test_known_drift_passes_only_while_pinned(self):
        pin = {"radius.radius-xs": {"constant": "RADIUS_SMALL", "rust": 3.0, "token": 2.0, "reason": "test"}}
        held = check_non_colour_parity(SRC.replace("= 2;", "= 3;"), tokens(), MAP, pin)
        self.assertTrue(held["ok"], held)
        self.assertEqual(len(held["known_drift"]), 1)
        moved = check_non_colour_parity(SRC.replace("= 2;", "= 4;"), tokens(), MAP, pin)
        self.assertFalse(moved["ok"])
        fixed = check_non_colour_parity(SRC, tokens(), MAP, pin)
        self.assertFalse(fixed["ok"])
        self.assertTrue(fixed["stale_exemptions"])

    def test_title_token_is_not_matched_by_name(self):
        self.assertNotIn("typography.font-size-title", TOKEN_TO_RUST)
        self.assertEqual(TOKEN_TO_RUST["typography.font-size-hero"], ["FONT_SIZE_TITLE"])

    def test_a_new_role_constant_without_a_token_or_a_reason_fails(self):
        src = SRC + "pub const SPACE_NEW: f32 = 20.0;\n"
        r = check_non_colour_parity(src, tokens(), MAP, {})
        self.assertFalse(r["ok"])
        self.assertEqual(r["unguarded_role_constants"], ["SPACE_NEW"])

    def test_every_ladder_inset_and_control_constant_is_guarded_by_a_primitive(self):
        guarded = {n for names in TOKEN_TO_RUST.values() for n in names}
        for name in ("SPACE_FACT", "SPACE_REGION", "INSET_PANEL", "INSET_MODAL", "CONTROL_STANDARD", "CONTROL_ROW", "RADIUS_CONTROL", "FONT_SIZE_CAPTION"):
            self.assertIn(name, guarded)

    def test_a_drifted_ladder_rung_in_the_real_theme_fails(self):
        drifted = THEME.replace("pub const SPACE_BLOCK: f32 = 16.0;", "pub const SPACE_BLOCK: f32 = 20.0;")
        self.assertNotEqual(drifted, THEME)
        r = check_non_colour_parity(drifted, TOKENS)
        self.assertFalse(r["ok"])
        self.assertTrue(any("SPACE_BLOCK (20)" in d for d in r["drifts"]))

    def test_repository_is_in_parity_with_the_pinned_drift_only(self):
        r = check_non_colour_parity(THEME, TOKENS)
        self.assertTrue(r["ok"], r)
        self.assertEqual(len(r["known_drift"]), len(KNOWN_DRIFT))


if __name__ == "__main__":
    unittest.main()
