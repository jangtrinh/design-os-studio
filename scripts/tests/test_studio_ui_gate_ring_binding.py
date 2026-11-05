"""Focus/Ring radius is a literal: 0.7.5 draws a bound radius at the variable's value, so no binding can express host radius + SPACE_FACT and any
radius binding on a ring instance fails (per corner or the uniform `cornerRadius` the engine ignores). An allow-list entry carries the literal
it waives and no `host`: the parent already says what the host is, so a second source that can drift is refused.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_ring_binding
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import CORNERS, frame_kind, with_node  # noqa: E402
from test_studio_ui_gate_allowlist import entry  # noqa: E402
from test_studio_ui_gate_ring import RING, RING_MAIN, RingHostBase  # noqa: E402


class RingBindingTest(RingHostBase):
    def bind(self, var, keys=CORNERS):
        self.bindings().update({k: var for k in keys})

    def test_a_radius_bound_to_the_host_constant_fails(self):
        """0.7.5 draws a bound radius at the variable's value (4), not 4 + SPACE_FACT: the ring would be 4 px too small."""
        self.put(host=4.0, bind="s.fact", ring=8.0)
        self.bind("s.fact")
        fails = self.bound(self.entries(8.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("bound to space/fact on a Focus/Ring instance" in f for f in self.corner_fails(fails)), fails)

    def test_a_uniform_corner_radius_binding_on_a_ring_fails_too(self):
        self.put(host=4.0, bind="s.fact", ring=8.0)
        self.bind("s.fact", ("cornerRadius",))
        fails = self.bound(self.entries(8.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("on a Focus/Ring instance" in f for f in self.corner_fails(fails)), fails)

    def test_a_radius_binding_on_an_inheriting_ring_fails(self):
        self.bind("s.inline")
        self.assertTrue(any("topLeftRadius" in f and "on a Focus/Ring instance" in f for f in self.bound(self.entries(12.0))))

    def test_an_entry_that_names_a_host_fails(self):
        """The parent says what the host is: a `host` in the entry is a second source that can drift, so the field itself is refused."""
        for host in ("theme::RADIUS_CONTROL", "theme::SPACE_FACT", "pill"):
            fails = self.bound(self.entries(12.0, host=host))
            self.assertEqual(len(self.corner_fails(fails)), 4, (host, fails))
            self.assertTrue(all("names a host" in f and "parent node" in f for f in self.corner_fails(fails)), fails)

    def test_a_host_on_an_entry_that_is_not_a_ring_radius_fails(self):
        with_node(self.doc, kind=frame_kind(padding=[0.0, 12.0, 0.0, 0.0]), corner_radii=[0.0] * 4)
        fails = self.bound([entry(RING, "paddingRight", 12.0, host="theme::SPACE_FACT")])
        self.assertTrue(any("names a host" in f and "paddingRight" in f for f in fails), fails)

    def test_the_rule_is_only_for_ring_instances(self):
        self.doc["nodes"]["4"].update(kind="Rectangle", corner_radii=[8.0] * 4, size={"x": 3.0, "y": 3.0}, transform=None, parent=2)
        self.doc["nodes"]["2"]["children"] = [5, 4]
        self.doc["nodes"]["5"]["children"] = []
        self.bind("s.inline")
        self.assertEqual(self.bound([e for e in self.entries(12.0) if e["path"] == RING_MAIN]), [])


if __name__ == "__main__":
    unittest.main()
