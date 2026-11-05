"""R1: the icon-swap round-trip check of the library verify. An `Icon` swap must keep the variant's glyph colour, the colour's
variable binding, the Disabled opacity and the slot's `Visible` binding (the engine bug is #1427). Synthetic native documents only.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_check
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_check as swap  # noqa: E402

GREY = {"r": 163 / 255, "g": 163 / 255, "b": 163 / 255, "a": 1.0}
WHITE = {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}


def host(doc, nid, fill=GREY, binding="s.color.icon-quaternary", opacity=1.0, visible_binding="Has Leading Icon", visible=True, ring=False, size=20.0):
    """A host instance (nid) with an optional focus ring, a nested icon instance (nid+2) and its glyph (nid+3)."""
    n = doc["nodes"]
    kids = [nid + 1] if ring else []
    kids += [nid + 2]
    n[str(nid)] = {"id": nid, "name": f"host{nid}", "kind": {"Instance": {}}, "children": kids}
    if ring:
        n[str(nid + 1)] = {"id": nid + 1, "name": "Focus ring", "kind": {"Instance": {}}, "children": [nid + 4]}
        n[str(nid + 4)] = {"id": nid + 4, "name": "Ring", "kind": "Rectangle", "children": [], "fills": []}
    slot = {"id": nid + 2, "name": "Icon", "kind": {"Instance": {}}, "children": [nid + 3], "component": {"bindings": {}}, "size": {"x": size, "y": size}}
    if visible_binding:
        slot["component"]["bindings"]["Visible"] = visible_binding
    if not visible:
        slot["visible"] = False
    n[str(nid + 2)] = slot
    n[str(nid + 3)] = {"id": nid + 3, "name": "Glyph", "kind": "Vector", "children": [],
                       "fills": [{"kind": {"Solid": fill}, "opacity": opacity, "visible": True}],
                       "variables": {"bindings": {"fills/0": binding} if binding else {}}}
    return nid


def doc():
    return {"nodes": {}}


class CompareTest(unittest.TestCase):
    def pair(self, **swapped):
        d = doc()
        host(d, 10)
        host(d, 20, **swapped)
        return swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))

    def test_a_swap_that_keeps_everything_has_no_mismatch(self):
        self.assertEqual(self.pair(), [])

    def test_a_lost_fill_is_found(self):
        out = self.pair(fill=WHITE)
        self.assertEqual(len(out), 1, out)
        self.assertIn("glyph fill lost", out[0])
        self.assertIn("#A3A3A3", out[0])
        self.assertIn("#FFFFFF", out[0])

    def test_the_tool_loss_of_26_levels_is_found(self):
        """#1427's own Tool loss is #E5E5E5 -> #FFFFFF (26 levels per channel): a tolerance wide enough to hide it would report a clean swap."""
        grey = {"r": 229 / 255, "g": 229 / 255, "b": 229 / 255, "a": 1.0}
        d = doc()
        host(d, 10, fill=grey)
        host(d, 20, fill=WHITE)
        out = swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))
        self.assertEqual(len(out), 1, out)
        self.assertIn("#E5E5E5", out[0])

    def test_one_level_is_rounding_and_two_levels_is_a_loss(self):
        near = lambda n: {"r": n / 255, "g": 163 / 255, "b": 163 / 255, "a": 1.0}
        for level, lost in ((164, False), (165, True)):
            d = doc()
            host(d, 10)
            host(d, 20, fill=near(level))
            self.assertEqual(bool(swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))), lost, level)

    def test_an_alpha_only_difference_is_a_lost_fill(self):
        """The same colour at half alpha is not the same glyph: the alpha channel is compared like the three colour channels."""
        d = doc()
        host(d, 10)
        host(d, 20, fill=dict(GREY, a=0.5))
        out = swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))
        self.assertEqual(len(out), 1, out)
        self.assertIn("glyph fill lost", out[0])
        self.assertIn("/128", out[0])

    def test_a_lost_binding_is_found(self):
        out = self.pair(binding="s.color.icon")
        self.assertEqual(len(out), 1, out)
        self.assertIn("binding lost", out[0])
        self.assertIn("s.color.icon-quaternary", out[0])

    def test_an_unbound_glyph_after_the_swap_is_a_lost_binding(self):
        self.assertTrue(any("binding lost" in m for m in self.pair(binding=None)))

    def test_a_lost_disabled_opacity_is_found(self):
        d = doc()
        host(d, 10, opacity=0.5)
        host(d, 20, opacity=1.0)
        out = swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))
        self.assertEqual(len(out), 1, out)
        self.assertIn("opacity lost", out[0])

    def test_a_lost_visibility_binding_is_found(self):
        out = self.pair(visible_binding=None)
        self.assertEqual(len(out), 1, out)
        self.assertIn("visibility binding lost", out[0])

    def test_a_hidden_icon_that_becomes_visible_is_found(self):
        d = doc()
        host(d, 10, visible=False)
        host(d, 20, visible=True)
        out = swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))
        self.assertTrue(any("visibility changed" in m for m in out), out)

    def test_a_slot_reset_to_the_icon_mains_size_is_found(self):
        """A 20 px tool glyph becomes the Icon main's 16 px: every tool, rail and icon button on a screen would be drawn small."""
        out = self.pair(size=16.0)
        self.assertEqual(len(out), 1, out)
        self.assertIn("glyph size lost: 20 before the swap, 16 after", out[0])

    def test_all_four_losses_are_reported_together(self):
        d = doc()
        host(d, 10, opacity=0.5)
        host(d, 20, fill=WHITE, binding="s.color.icon", opacity=1.0, visible_binding=None)
        kinds = [m.split(" lost")[0] for m in swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))]
        self.assertEqual(sorted(kinds), ["glyph fill", "glyph fill binding", "glyph opacity", "slot visibility binding"])
        d = doc()
        host(d, 10)
        host(d, 20, size=16.0)
        self.assertEqual([m.split(" lost")[0] for m in swap.compare(swap.snapshot(d, 10), swap.snapshot(d, 20))], ["glyph size"])

    def test_the_focus_ring_instance_is_not_the_icon_slot(self):
        d = doc()
        host(d, 10, ring=True)
        self.assertEqual(swap.snapshot(d, 10)["fill"], {"r": 163, "g": 163, "b": 163, "a": 255})


class ReportTest(unittest.TestCase):
    LOST = [{"case": "Tool Selected", "mismatches": ["glyph fill lost: #A3A3A3 before the swap, #FFFFFF after"]}]
    CLEAN = [{"case": "Tool Selected", "mismatches": []}]

    def test_while_pending_it_prints_swap_pending_with_the_mismatches_and_does_not_raise(self):
        lines = swap.report(self.LOST, fatal=False)
        self.assertTrue(lines[0].startswith("SWAP-PENDING #1427"), lines)
        self.assertTrue(any("Tool Selected" in line and "glyph fill lost" in line for line in lines), lines)

    def test_when_fatal_a_mismatch_stops_the_build(self):
        with self.assertRaises(SystemExit) as e:
            swap.report(self.LOST, fatal=True)
        self.assertIn("#1427", str(e.exception))
        self.assertIn("glyph fill lost", str(e.exception))

    def test_a_clean_run_says_the_constant_can_flip(self):
        lines = swap.report(self.CLEAN, fatal=False)
        self.assertTrue(lines[0].startswith("SWAP-OK #1427") and "SWAP_CHECK_FATAL" in lines[0], lines)
        self.assertEqual(swap.report(self.CLEAN, fatal=True)[0][:7], "SWAP-OK")

    def test_the_constant_is_off_until_the_engine_fix_merges(self):
        self.assertIs(swap.SWAP_CHECK_FATAL, False)
        self.assertEqual(swap.ISSUE, "#1427")


if __name__ == "__main__":
    unittest.main()
