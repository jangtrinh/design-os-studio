"""G2 opacity with an allow-listed paint opacity: a paint carries 1.0 (0.5 inside a Disabled variant) unless an exact allow-list entry `{path, property: "fills/<i>/opacity", value, reason}` names
another opacity, as the Ready-for-dev pill's `DEV_READY.gamma_multiply(0.22)` needs. The entry is compared like the literal entries of G2 bound: an unlisted opacity fails, a listed one passes only
when the stored opacity is the listed value, and a stale, duplicate or valueless entry fails. In a Disabled variant the expected opacity is 0.5 times the listed value.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_opacity_listed
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import make_doc, node  # noqa: E402
from studio_ui_gate_opacity import check_opacity  # noqa: E402
from test_studio_ui_gate_opacity import paint  # noqa: E402
from test_studio_ui_gate_registry import add_set  # noqa: E402

DEFAULT, DISABLED = "02 Core/Button/Icon/State=Default", "02 Core/Button/Icon/State=Disabled"
WHY = "dev/sections.rs:36 `status_color(s).gamma_multiply(0.22)`"


def entry(path=DEFAULT, value=0.22, prop="fills/0/opacity", **kw):
    return {"path": path, "property": prop, "reason": WHY, "value": value, **kw}


class ListedOpacityTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()
        add_set(self.doc, 20, "Button/Icon", {"State": ["Default", "Disabled"]})  # variants 21 (Default) and 22 (Disabled)

    def put(self, nid, **extra):
        self.doc["nodes"][str(nid)].update(extra)

    def check(self, *allow):
        return check_opacity(self.doc, list(allow))

    def test_an_unlisted_0_22_fails(self):
        self.put(21, fills=[paint(0.22)])
        self.assertEqual(self.check()[0], [f"{DEFAULT}: fills/0 has opacity 0.22, a non-Disabled variant carries 1 on every visible paint"])

    def test_a_listed_0_22_passes(self):
        self.put(21, fills=[paint(0.22)])
        fails, counts = self.check(entry())
        self.assertEqual(fails, [])
        self.assertEqual(counts["listed"], 1)

    def test_a_listed_0_2_against_a_stored_0_22_fails_and_names_both(self):
        self.put(21, fills=[paint(0.22)])
        out = self.check(entry(value=0.2))[0]
        self.assertEqual(len(out), 1)
        self.assertIn("fills/0 has opacity 0.22", out[0])
        self.assertIn("allow-list pins 0.2", out[0])

    def test_an_entry_on_a_paint_whose_opacity_is_1_fails(self):
        self.put(21, fills=[paint(1.0)])
        out = self.check(entry())[0]
        self.assertEqual(len(out), 1)
        self.assertIn("fills/0 has opacity 1", out[0])
        self.assertIn("allow-list pins 0.22", out[0])

    def test_a_disabled_variant_expects_half_the_listed_value(self):
        self.put(22, fills=[paint(0.11)])
        self.assertEqual(self.check(entry(DISABLED))[0], [])
        for stored in (0.22, 0.5, 1.0):
            with self.subTest(stored):
                self.put(22, fills=[paint(stored)])
                self.assertEqual(len(self.check(entry(DISABLED))[0]), 1)

    def test_a_listed_paint_inside_an_instance_of_a_disabled_variant_expects_half_too(self):
        """Sticky downward: the nested pill is dimmed by the Disabled instance above it."""
        self.put(21, kind={"Component": {}}, children=[23], fills=[])
        self.doc["nodes"]["23"] = node(23, "Pill", {"Instance": {"frame": {}, "component": 22}}, parent=21, fills=[paint(0.11)])
        self.assertEqual(self.check(entry(f"{DEFAULT}/Pill"))[0], [])

    def test_a_stale_entry_fails(self):
        self.put(21, fills=[paint(1.0)])
        self.assertEqual(self.check(entry(f"{DEFAULT}/Nothing", prop="fills/0/opacity"))[0],
                         [f"allow-list entry {DEFAULT}/Nothing: fills/0/opacity matches no paint"])
        self.assertEqual(self.check(entry(prop="strokes/0/opacity"))[0], [f"allow-list entry {DEFAULT}: strokes/0/opacity matches no paint"])

    def test_a_duplicate_entry_fails(self):
        self.put(21, fills=[paint(0.22)])
        self.assertIn(f"allow-list entry {DEFAULT}: fills/0/opacity is listed twice", self.check(entry(), entry())[0])

    def test_a_valueless_or_out_of_range_entry_fails(self):
        self.put(21, fills=[paint(0.22)])
        for bad in (None, "0.22", True, 0, 1.0, 1.5, -0.1):
            with self.subTest(bad):
                self.assertTrue(any("has no value" in f for f in self.check(entry(value=bad))[0]), bad)
        missing = entry()
        del missing["value"]
        self.assertTrue(any("has no value" in f for f in self.check(missing)[0]))

    def test_a_listed_stroke_opacity_is_read_like_a_fill(self):
        self.put(21, stroke={"paints": [paint(0.22)], "weight": 1.0})
        self.assertEqual(self.check(entry(prop="strokes/0/opacity"))[0], [])
        self.assertEqual(len(self.check()[0]), 1)

    def test_the_other_allow_list_entries_are_not_opacity_entries(self):
        self.put(21, fills=[paint(1.0)])
        grey = {"r": 23, "g": 23, "b": 23, "a": 255}
        other = {"path": DEFAULT, "property": "fills/0", "reason": "x", "value": grey}
        self.assertEqual(self.check(other)[0], [])

    def test_the_bound_gate_leaves_the_opacity_entries_to_this_one(self):
        """G2 bound compares an entry with the unbound property it waives; an opacity entry waives none, and must not read as stale or shared there."""
        fails, per_page = gate.check_bound(self.doc, [entry("01 Foundations/Swatch")])
        self.assertEqual(fails, [])
        self.assertEqual(per_page["01 Foundations"]["entries"], 0)


if __name__ == "__main__":
    unittest.main()
