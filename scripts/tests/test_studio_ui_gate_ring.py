"""G2 rule for Focus/Ring instances: a ring rings the node it sits in (its parent, the host). It is that parent grown by SPACE_FACT on
every side, at the parent's radius + SPACE_FACT (`kit/focus.rs:36-38`), so a ring that keeps the main's 12 inside a 2, 4 or 6 px host
fails. 0.7.5 draws a bound radius at the variable's value, so no binding can express the sum and any radius binding on a ring instance
fails (test_studio_ui_gate_ring_binding, with the allow-list entry that carries the literal and no `host`). Pill hosts and rounding:
test_studio_ui_gate_ring_rounding.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_ring
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import CORNERS, node, var, with_node  # noqa: E402
from test_studio_ui_gate_allowlist import entry  # noqa: E402
from test_studio_ui_gate_bound import B, BoundBase  # noqa: E402

RING_MAIN = "02 Core/Focus/Ring/Tone=Brand"
HOST, RING = "01 Foundations/Host", "01 Foundations/Host/Swatch"
UPRIGHT = {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0}


class RingHostBase(BoundBase):
    """The Swatch node is a Focus/Ring instance (a variant of a `Focus/Ring` set on 02 Core whose main radius is 12) inside a `Host` frame."""

    def setUp(self):
        super().setUp()
        d = self.doc["nodes"]
        d["50"] = node(50, "Focus/Ring", {"ComponentSet": {}}, parent=3, children=[51])
        d["51"] = node(51, "Tone=Brand", {"Component": {}}, parent=50, corner_radii=[12.0] * 4)
        d["3"]["children"].append(50)
        for vid, name, code, prim in (("s.control", "radius/control", "theme::RADIUS_CONTROL", "p.8"), ("s.mark", "radius/mark", "theme::RADIUS_MARK", "p.2"),
                                      ("s.swatch", "radius/swatch", "theme::SWATCH_RADIUS", "p.2")):
            self.doc["variables"][vid] = var(vid, name, "cs", {"Alias": prim}, code, "Number")
        self.doc["variables"]["p.2"] = var("p.2", "space/2", "cp", {"Number": 2.0}, kind="Number", hidden=True)
        self.put(host=8.0, bind="s.control")

    def put(self, host, bind=None, size=(40.0, 40.0), ring=12.0, ring_size=None, at=(-4.0, -4.0), **ring_extra):
        """The Host frame (size, radius `host`, bound to variable `bind` or a literal) with the ring (radius `ring`) inside it."""
        d, bindings = self.doc["nodes"], {k: bind for k in CORNERS} if bind else {}
        d["5"] = node(5, "Host", "Rectangle", parent=2, children=[4], size={"x": size[0], "y": size[1]}, corner_radii=[float(host)] * 4,
                      variables={"modes": {}, "bindings": bindings})
        d["2"]["children"], d["4"]["parent"] = [5], 5
        rs = ring_size or (size[0] + 8.0, size[1] + 8.0)
        with_node(self.doc, kind={"Instance": {"frame": {}, "component": 51, "component_key": "k"}}, fills=[], corner_radii=[float(ring)] * 4,
                  size={"x": rs[0], "y": rs[1]}, transform={**UPRIGHT, "tx": at[0], "ty": at[1]}, **ring_extra)
        self.bindings().clear()
        self.host_pinned = None if bind else float(host)

    def entries(self, ring, host_entries=None, **extra):
        """The main's literals, the ring's four corners pinned to `ring`, and the host's four corners when the host is a literal."""
        out = [entry(RING_MAIN, k, 12.0) for k in CORNERS] + [entry(RING, k, ring, **extra) for k in CORNERS]
        if self.host_pinned is not None:
            out += [entry(HOST, k, self.host_pinned if host_entries is None else host_entries) for k in CORNERS]
        return out

    def ring_fails(self, fails):
        return [f for f in fails if f.startswith(RING)]

    def corner_fails(self, fails):
        return [f for f in self.ring_fails(fails) if "Radius" in f]


class RingHostTest(RingHostBase):
    def test_the_main_12_on_a_radius_control_host_passes(self):
        self.assertEqual(self.bound(self.entries(12.0)), [])

    def test_a_pinned_literal_on_a_host_that_needs_it_passes(self):
        self.put(host=4.0, bind="s.fact", ring=8.0)  # the checkbox box (SPACE_FACT 4): 4 + 4
        self.assertEqual(self.bound(self.entries(8.0)), [])
        self.put(host=2.0, bind="s.mark", ring=6.0)  # a swatch or mark (RADIUS_MARK 2): 2 + 4
        self.assertEqual(self.bound(self.entries(6.0)), [])

    def test_a_default_12_ring_in_a_radius_mark_host_fails(self):
        self.put(host=2.0, bind="s.mark")
        fails = self.bound(self.entries(12.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("is 12" in f and "draws radius 2" in f and "drawn at 6" in f for f in self.corner_fails(fails)), fails)

    def test_a_default_12_ring_in_a_swatch_radius_host_fails(self):
        self.put(host=2.0, bind="s.swatch")
        fails = self.bound(self.entries(12.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("drawn at 6" in f for f in self.corner_fails(fails)), fails)

    def test_a_default_12_ring_in_a_space_fact_host_fails(self):
        self.put(host=4.0, bind="s.fact")
        fails = self.bound(self.entries(12.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("drawn at 8" in f for f in self.corner_fails(fails)), fails)

    def test_a_pinned_literal_that_is_not_host_plus_space_fact_fails(self):
        self.put(host=4.0, bind="s.fact", ring=6.0)
        fails = self.bound(self.entries(6.0))
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("is 6" in f and "drawn at 8" in f for f in self.corner_fails(fails)), fails)

    def test_a_ring_whose_host_corners_differ_is_checked_corner_by_corner(self):
        self.doc["nodes"]["5"]["corner_radii"] = [8.0, 8.0, 4.0, 8.0]
        self.doc["nodes"]["5"]["variables"]["bindings"]["bottomRightRadius"] = "s.fact"
        self.doc["nodes"]["4"]["corner_radii"] = [12.0, 12.0, 12.0, 12.0]
        fails = self.bound(self.entries(12.0))
        self.assertEqual([f.split(": ")[1].split(" ")[0] for f in self.corner_fails(fails)], ["bottomRightRadius"], fails)

    def test_a_zero_corner_is_checked_too(self):
        """node_props skips a zero radius, but Rust draws round(0 + 4) on a square host: a ring left square is wrong."""
        self.put(host=0.0, bind=None, ring=0.0)
        fails = self.bound([e for e in self.entries(0.0) if e["path"] != RING and e["path"] != HOST])
        self.assertEqual(len(self.corner_fails(fails)), 4, fails)
        self.assertTrue(all("is 0" in f and "drawn at 4" in f for f in self.corner_fails(fails)), fails)

    def test_an_entry_whose_value_differs_from_the_stored_radius_fails(self):
        self.put(host=4.0, bind="s.fact", ring=8.0)
        fails = self.bound(self.entries(12.0))
        self.assertEqual(len(self.ring_fails(fails)), 4, fails)
        self.assertTrue(all("allow-listed literal" in f for f in fails), fails)


class RingBoundingBoxTest(RingHostBase):
    """The ring is its parent grown by SPACE_FACT on every side: size, position, and no scale or rotation."""

    def test_a_ring_that_is_not_grown_on_every_side_fails(self):
        for label, size in (("narrower", (44.0, 48.0)), ("shorter", (48.0, 44.0)), ("the parent's own size", (40.0, 40.0)), ("wider", (56.0, 48.0))):
            self.put(host=8.0, bind="s.control", ring_size=size)
            fails = self.ring_fails(self.bound(self.entries(12.0)))
            self.assertEqual(len(fails), 1, (label, fails))
            self.assertIn("grown by SPACE_FACT on every side (48 x 48)", fails[0], label)

    def test_a_ring_at_the_right_size_but_the_wrong_position_fails(self):
        for at in ((0.0, 0.0), (-4.0, 0.0), (0.0, -4.0), (-2.0, -2.0), (4.0, 4.0)):
            self.put(host=8.0, bind="s.control", at=at)
            fails = self.ring_fails(self.bound(self.entries(12.0)))
            self.assertEqual(len(fails), 1, (at, fails))
            self.assertIn("puts it at (-4, -4)", fails[0])

    def test_a_scaled_or_rotated_ring_fails(self):
        for extra in ({"a": 1.25}, {"d": 0.8}, {"b": 0.1}, {"c": -0.1}):
            self.put(host=8.0, bind="s.control")
            self.doc["nodes"]["4"]["transform"].update(extra)
            fails = self.ring_fails(self.bound(self.entries(12.0)))
            self.assertEqual(len(fails), 1, (extra, fails))
            self.assertIn("scaled or rotated", fails[0])

    def test_a_ring_with_no_transform_or_size_fails_closed(self):
        self.put(host=8.0, bind="s.control")
        del self.doc["nodes"]["4"]["transform"]
        self.assertTrue(any("has no transform" in f for f in self.bound(self.entries(12.0))))
        self.put(host=8.0, bind="s.control")
        del self.doc["nodes"]["4"]["size"]
        self.assertTrue(any("has no size" in f for f in self.bound(self.entries(12.0))))

    def test_a_ring_that_sits_in_a_page_has_no_host(self):
        """A page has no size: the ring rings nothing, so it fails instead of passing against a radius it never had."""
        d = self.doc["nodes"]
        d["2"]["children"], d["4"]["parent"] = [4], 2
        fails = [f for f in self.bound([e for e in self.entries(12.0) if e["path"] == RING_MAIN]) if f.startswith(B)]
        self.assertEqual(len([f for f in fails if "has no size" in f]), 1, fails)
        self.assertIn("has no size: a Focus/Ring instance rings the node it sits in", fails[0])
        self.assertEqual(len(fails), 5, fails)  # the geometry line only: the four corners are left unchecked against a host that is not one, so just "not bound"


if __name__ == "__main__":
    unittest.main()
