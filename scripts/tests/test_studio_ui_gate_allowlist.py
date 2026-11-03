"""G2 allow-list logic: an entry pins the literal it waives and an entry that waives nothing is stale. The Focus/Ring radius rule is in
test_studio_ui_gate_ring.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_allowlist
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import frame_kind, with_node  # noqa: E402
from test_studio_ui_gate_bound import B, BoundBase  # noqa: E402


def entry(path, prop, value, reason="test", **extra):
    return {"path": path, "property": prop, "reason": reason, "value": value, **extra}


class LiteralPinTest(BoundBase):
    def setUp(self):
        super().setUp()
        with_node(self.doc, kind=frame_kind(padding=[0.0, 12.0, 0.0, 0.0]))

    def pad(self, value, **extra):
        return {"path": B, "property": "paddingRight", "reason": "a 12 px literal", **({"value": value} if value is not None else {}), **extra}

    def test_an_entry_waives_the_literal_it_names(self):
        self.assertEqual(self.bound([self.pad(12)]), [])

    def test_the_stored_literal_must_equal_the_entry_value(self):
        fails = self.bound([self.pad(3)])
        self.assertEqual(len(fails), 1, fails)
        self.assertIn("paddingRight", fails[0])
        self.assertIn("allow-listed literal", fails[0])

    def test_an_entry_without_a_value_waives_nothing(self):
        fails = self.bound([self.pad(None)])
        self.assertTrue(any("no value" in f and "paddingRight" in f for f in fails), fails)

    def test_a_boolean_is_not_a_literal(self):
        """`True == 1` in Python: a padding of 1 would match an entry whose value is JSON true, so the type is checked."""
        with_node(self.doc, kind=frame_kind(padding=[0.0, 1.0, 0.0, 0.0]))
        self.assertEqual(self.bound([self.pad(1)]), [])
        fails = self.bound([self.pad(True)])
        self.assertTrue(any("no value" in f and "paddingRight" in f for f in fails), fails)

    def test_a_colour_literal_is_pinned_too(self):
        del self.bindings()["fills/0"]
        grey = {"r": 23, "g": 23, "b": 23, "a": 255}
        self.assertEqual(self.bound([self.pad(12), entry(B, "fills/0", grey)]), [])
        self.assertTrue(any("allow-listed literal" in f for f in self.bound([self.pad(12), entry(B, "fills/0", dict(grey, r=0))])))


class StaleEntryTest(BoundBase):
    def test_an_entry_that_waives_nothing_fails(self):
        fails = self.bound([entry(B, "paddingRight", 12.0)])
        self.assertEqual(len(fails), 1, fails)
        self.assertIn("matches no unbound property", fails[0])

    def test_an_entry_listed_twice_fails(self):
        with_node(self.doc, kind=frame_kind(padding=[0.0, 12.0, 0.0, 0.0]))
        fails = self.bound([entry(B, "paddingRight", 12.0), entry(B, "paddingRight", 12.0)])
        self.assertTrue(any("listed twice" in f for f in fails), fails)

    def test_the_page_count_equals_the_entries_on_the_page(self):
        with_node(self.doc, kind=frame_kind(padding=[0.0, 12.0, 0.0, 12.0]))
        fails, per_page = gate.check_bound(self.doc, [entry(B, "paddingRight", 12.0), entry(B, "paddingLeft", 12.0)])
        self.assertEqual((fails, per_page["01 Foundations"]["allowed"], per_page["01 Foundations"]["entries"]), ([], 2, 2))

    def test_two_nodes_that_share_a_path_cannot_share_one_entry(self):
        with_node(self.doc, kind=frame_kind(padding=[0.0, 12.0, 0.0, 0.0]))
        self.doc["nodes"]["2"]["children"].append(5)
        self.doc["nodes"]["5"] = dict(self.doc["nodes"]["4"], id=5)
        fails = self.bound([entry(B, "paddingRight", 12.0)])
        self.assertTrue(any("shares its path" in f for f in fails), fails)


if __name__ == "__main__":
    unittest.main()
