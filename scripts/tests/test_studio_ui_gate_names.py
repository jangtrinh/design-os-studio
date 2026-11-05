"""G1 naming: Primitive colour names follow the objective-step rule, and never take a Tailwind v3.4.17 name that holds another value.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_names
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import make_doc, var  # noqa: E402


def colour(r, g, b, a=255):
    return {"Color": {"r": r / 255, "g": g / 255, "b": b / 255, "a": a / 255}}


class PrimitiveNameTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def add(self, vid, name, rgba):
        self.doc["variables"][vid] = var(vid, name, "cp", colour(*rgba), hidden=True)

    def names(self):
        return gate.check_primitive_names(self.doc)

    def test_clean_document_passes(self):
        self.assertEqual(self.names(), [])

    def test_a_palette_colour_must_carry_its_palette_name(self):
        self.doc["variables"]["p.grey"]["name"] = "neutral/899"
        self.assertTrue(any("neutral/899" in f and "neutral/900" in f for f in self.names()), self.names())

    def test_an_off_palette_colour_never_takes_a_tailwind_name_that_holds_another_value(self):
        self.add("p.orange", "amber/200", (255, 166, 41))  # amber-200 is #fde68a in Tailwind v3.4.17
        self.assertTrue(any("amber/200" in f and "amber/200-2" in f for f in self.names()), self.names())

    def test_the_next_free_name_is_accepted(self):
        self.add("p.orange", "amber/200-2", (255, 166, 41))
        self.assertEqual(self.names(), [])

    def test_a_name_by_hex_fails(self):
        self.add("p.hex", "orange/ffa629", (255, 166, 41))
        self.assertTrue(self.names())

    def test_an_alpha_colour_shares_the_base_of_its_opaque_twin(self):
        self.add("p.a", "amber/200-2", (255, 166, 41))
        self.add("p.b", "amber/200-2-a50", (255, 166, 41, 128))
        self.assertEqual(self.names(), [])

    def test_number_primitives_are_not_named_by_this_rule(self):
        self.doc["variables"]["p.4"]["name"] = "space/4"
        self.assertEqual(self.names(), [])


if __name__ == "__main__":
    unittest.main()
