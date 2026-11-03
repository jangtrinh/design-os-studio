"""Three strictness rules of the Focus/Ring gate (the W1a fix-3 review, MINORs 4, 6 and 7):

* an allow-list entry that carries a `host` key fails whatever its value (`""`, `null`, `0` and `false` used to pass a truthy test);
* a ring's four corners are equal: Rust draws one `CornerRadius::same` (`kit/focus.rs:38`), so four different radii are never what production
  draws, even when each matches the host corner above it;
* a node named `Focus ring` that is not an instance of a `Focus/Ring` variant fails: a detached frame of the same name would skip every ring
  rule. (The controller notes say "named `Focus/Ring`"; that is the component set's name, and the instances are named `Focus ring`.)

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_ring_strictness
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import CORNERS, with_node  # noqa: E402
from test_studio_ui_gate_ring import HOST, RING, RING_MAIN, RingHostBase  # noqa: E402


class HostFieldTest(RingHostBase):
    def test_an_entry_without_a_host_key_passes(self):
        self.assertEqual(self.bound(self.entries(12.0)), [])

    def test_a_host_key_fails_whatever_its_value(self):
        for value in ("", None, 0, False, "theme::RADIUS_CONTROL"):
            fails = self.bound(self.entries(12.0, host=value))
            self.assertEqual(len(self.ring_fails(fails)), 4, (value, fails))
            self.assertTrue(all("names a host" in f for f in self.ring_fails(fails)), (value, fails))


class EqualCornersTest(RingHostBase):
    def non_uniform_host(self, radii, bound_to):
        """A host whose corners differ, and a ring whose corners match them one by one (each is host + SPACE_FACT)."""
        self.doc["nodes"]["5"]["corner_radii"] = list(radii)
        self.doc["nodes"]["5"]["variables"]["bindings"].update(bound_to)
        self.doc["nodes"]["4"]["corner_radii"] = [r + 4.0 for r in radii]

    def entries_for(self, ring_radii):
        return [e for e in self.entries(12.0) if e["path"] != RING] + [
            {"path": RING, "property": k, "reason": "test", "value": v} for k, v in zip(CORNERS, ring_radii)]

    def test_a_ring_matching_a_non_uniform_host_corner_by_corner_still_fails(self):
        self.non_uniform_host([8.0, 8.0, 4.0, 4.0], {"bottomRightRadius": "s.fact", "bottomLeftRadius": "s.fact"})
        fails = self.ring_fails(self.bound(self.entries_for([12.0, 12.0, 8.0, 8.0])))
        self.assertEqual(len(fails), 1, fails)
        self.assertIn("corners are not equal", fails[0])
        self.assertIn("one CornerRadius::same", fails[0])

    def test_one_odd_corner_on_a_uniform_host_fails_on_both_rules(self):
        self.doc["nodes"]["4"]["corner_radii"] = [12.0, 12.0, 12.0, 0.0]
        fails = self.ring_fails(self.bound(self.entries_for([12.0, 12.0, 12.0, 0.0])))
        self.assertTrue(any("corners are not equal" in f for f in fails), fails)
        self.assertTrue(any("bottomLeftRadius is 0" in f for f in fails), fails)

    def test_four_equal_corners_pass(self):
        self.assertEqual(self.bound(self.entries(12.0)), [])


class DetachedRingTest(RingHostBase):
    def setUp(self):
        super().setUp()
        self.ring_path = "01 Foundations/Host/Focus ring"

    def entries_here(self, ring=12.0):
        return [dict(e, path=self.ring_path) if e["path"] == RING else e for e in self.entries(ring)]

    def test_an_instance_of_a_ring_variant_named_focus_ring_passes(self):
        with_node(self.doc, name="Focus ring")
        self.assertEqual(self.bound(self.entries_here()), [])

    def test_a_frame_named_focus_ring_is_not_a_ring_and_fails(self):
        """The review's G1 case: a detached frame of radius 12 in a RADIUS_MARK host passed with its allow-list entries."""
        self.put(host=2.0, bind="s.mark", ring=12.0)
        with_node(self.doc, name="Focus ring", kind="Rectangle")
        fails = [f for f in self.bound(self.entries_here()) if "Focus ring" in f]
        self.assertTrue(any("is not an instance of a Focus/Ring variant" in f for f in fails), fails)

    def test_an_instance_of_another_component_named_focus_ring_fails(self):
        with_node(self.doc, name="Focus ring", kind={"Instance": {"frame": {}, "component": 41, "component_key": "k"}})
        fails = [f for f in self.bound(self.entries_here()) if "Focus ring" in f]
        self.assertTrue(any("is not an instance of a Focus/Ring variant" in f for f in fails), fails)

    def test_a_detached_node_with_another_name_is_not_a_ring_and_is_not_flagged(self):
        """Only the name decides: a frame called `Hairline` is no ring and no finding."""
        with_node(self.doc, name="Hairline", kind="Rectangle")
        self.assertFalse([f for f in self.bound(self.entries_here()) if "is not an instance of a Focus/Ring" in f])

    def test_the_component_set_named_focus_ring_slash_is_not_flagged(self):
        """`Focus/Ring` (the set, and its variants) is the main, not an instance named `Focus ring`."""
        self.assertFalse([f for f in self.bound(self.entries(12.0)) if "is not an instance" in f and RING_MAIN in f])


if __name__ == "__main__":
    unittest.main()
