"""G1 token half: a mapped constant names its token key (failure); unmapped tokens and value differences are INFO only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_tokens
The token map and the tokens are passed in; the committed G01 map is never read.
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import ROWS, make_doc  # noqa: E402


class TokenHalfTest(unittest.TestCase):
    KEYS = {"BG_SECONDARY": "colors.surface", "SPACE_FACT": "spacing.space-1"}
    TOKENS = {"colors": {"surface": {"$type": "color", "$value": "#171717"}, "card": {"$type": "color", "$value": "#101010"}},
              "spacing": {"space-1": {"$type": "dimension", "$value": "4px"}, "space-fact": {"$type": "dimension", "$value": "5px"}}}

    def setUp(self):
        self.doc = make_doc()
        self.doc["variables"]["s.bg"]["description"] = "token colors.surface"
        self.doc["variables"]["s.fact"]["description"] = "token spacing.space-1"

    def tokens(self):
        return gate.check_tokens(self.doc, ROWS, self.TOKENS, self.KEYS)

    def test_mapped_constants_naming_their_token_pass(self):
        fails, infos = self.tokens()
        self.assertEqual(fails, [])
        self.assertTrue(any("colors.card" in i and "no mapped constant" in i for i in infos), infos)

    def test_a_missing_token_key_in_the_description_fails(self):
        self.doc["variables"]["s.bg"]["description"] = ""
        self.assertTrue(any("colors.surface" in f and "BG_SECONDARY" in f for f in self.tokens()[0]))

    def test_a_token_that_differs_from_theme_is_info_not_a_failure(self):
        tokens = json.loads(json.dumps(self.TOKENS))
        tokens["colors"]["surface"]["$value"] = "#181818"
        fails, infos = gate.check_tokens(self.doc, ROWS, tokens, self.KEYS)
        self.assertEqual(fails, [])
        self.assertTrue(any("colors.surface" in i and "#181818" in i and "#171717" in i for i in infos), infos)

    def test_a_name_match_with_a_different_value_is_info(self):
        _, infos = self.tokens()
        self.assertTrue(any("spacing.space-fact" in i and "5" in i and "4" in i for i in infos), infos)


if __name__ == "__main__":
    unittest.main()
