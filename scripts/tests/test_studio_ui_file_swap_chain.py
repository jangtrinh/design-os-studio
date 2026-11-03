"""Chain copies of the swap-paint pass: what `missing` names about them, and how `write_paints` and `record` settle them (which glyphs count as copies: ..._swap_chain_discovery).

A composite that nests a composite holding a swapped glyph keeps that glyph's paint from before the pass corrected it (the REST import expands once): the copy must get the same
paint 0 and `fills/0` binding as the glyph it copies, at any depth, and never a `Fills` flag: the editor clears every override flag on a linked copy and flags only the node a
user edits, so a flag on a copy would freeze it against its main. Synthetic documents from swap_chain_fixtures.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_chain
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_paints as sp  # noqa: E402
from swap_chain_fixtures import chain, settle  # noqa: E402
from test_studio_ui_file_swap_paints import GREY, NEW_GLYPH, OLD_GLYPH, OLD_SLOT, SEARCH_GLYPH, WHITE, doc, fields, node, paint  # noqa: E402
from test_studio_ui_file_swap_paints_pass import mcp_for  # noqa: E402


def pairs(found):
    return [(g["id"], p["id"]) for g, p in found]


class Missing(unittest.TestCase):
    def test_a_stale_copy_names_the_dropped_paint_and_nothing_else(self):
        d, (c,) = chain()
        out = sp.missing(d)
        self.assertEqual([(m["id"], m["field"]) for m in out], [(c, "paint")])  # its effective binding is the corrected glyph's, through the source; the flag is no gap
        self.assertEqual({m["what"] for m in out}, {"chain copy"})
        self.assertIn("Level 0/Header/Action/Icon/Glyph", out[0]["problem"])
        self.assertIn("paint 0 is #FFFFFF at opacity 1, the glyph it copies 33 has #737373 at opacity 0.5", out[0]["problem"])

    def test_a_copy_with_a_binding_of_its_own_that_differs_names_the_binding(self):
        d, (c,) = chain()
        settle(d, c, binding="s.icon")
        self.assertEqual(fields(d), ["binding"])
        self.assertIn("fills/0 is bound to s.icon, the glyph it copies 33 has s.quat", sp.missing(d)[0]["problem"])

    def test_a_wrong_colour_alone_and_a_wrong_opacity_alone(self):
        d, (c,) = chain()
        settle(d, c, binding="s.quat")
        d["nodes"][str(c)]["fills"] = [paint(WHITE, 0.5)]
        self.assertEqual(fields(d), ["paint"])
        d["nodes"][str(c)]["fills"] = [paint(GREY, 1.0)]
        self.assertEqual(fields(d), ["paint"])

    def test_an_unflagged_copy_of_a_flagged_glyph_with_the_same_paint_and_binding_is_complete(self):
        """The shape the editor makes when a user places the composite (`create_instance` -> `copy_rec`): the glyph carries `Fills`, its copies carry none. Nothing is missing."""
        d, copies = chain(2)
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["component"]["overrides"], ["Fills"])
        for c in copies:
            settle(d, c, binding="s.quat")
            self.assertNotIn("overrides", d["nodes"][str(c)]["component"])
        self.assertEqual(sp.missing(d), [])

    def test_a_copy_of_a_swapped_glyph_that_carries_no_flag_needs_none(self):
        """A swap between icons that paint alike leaves the glyph equal to its source and unflagged: its copy is the same, and nothing is missing."""
        old = node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[paint(WHITE, 1.0)], binding="s.icon")
        d, (c,) = chain(corrected=False, base=doc(kind=WHITE, opacity=1.0, binding="s.icon", flag=False, old=old))
        self.assertEqual((pairs(sp.chain_copies(d)), sp.missing(d)), ([(c, NEW_GLYPH)], []))


class WriteAndRecord(unittest.TestCase):
    def test_each_stale_copy_gets_the_paint_of_the_glyph_it_copies_through_one_fills_write(self):
        d, (c1, c2) = chain(2)
        m = mcp_for(d)
        self.assertEqual(sp.write_paints(m, d), {"glyphs": 1, "copies": 2, "paints": 2})
        self.assertEqual([(a["node_id"], a["fills"]) for a in m.updates()], [(c1, [paint(GREY, 0.5)]), (c2, [paint(GREY, 0.5)])])

    def test_a_copy_settles_after_the_swapped_glyph_it_copies_in_the_same_snapshot(self):
        d, (c1, c2) = chain(2, corrected=False)
        m = mcp_for(d)
        sp.write_paints(m, d)
        self.assertEqual([a["node_id"] for a in m.updates()], [NEW_GLYPH, c1, c2])
        self.assertEqual([a["fills"] for a in m.updates()], [[paint(GREY, 0.5)]] * 3)

    def test_the_other_paints_visibility_and_blend_of_a_copy_stay(self):
        d, (c,) = chain()
        d["nodes"][str(c)]["fills"] = [dict(paint(WHITE, 1.0), blend="Multiply"), paint(WHITE, 0.2)]
        m = mcp_for(d)
        sp.write_paints(m, d)
        self.assertEqual(m.updates()[0]["fills"], [dict(paint(GREY, 0.5), blend="Multiply"), paint(WHITE, 0.2)])

    def test_a_copy_that_already_carries_the_paint_is_not_written(self):
        d, (c1, c2) = chain(2)
        settle(d, c1)
        settle(d, c2)
        m = mcp_for(d)
        self.assertEqual(sp.write_paints(m, d), {"glyphs": 1, "copies": 2, "paints": 0})
        self.assertEqual(m.calls, [])

    def test_record_writes_the_differing_binding_of_a_copy_and_never_a_flag(self):
        d, (c1, c2) = chain(2)
        settle(d, c1, binding="s.icon")
        settle(d, c2)
        before = copy.deepcopy(d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 2, "bindings": 1, "flags": 0})
        self.assertEqual(d["nodes"][str(c1)]["variables"]["bindings"], {"fills/0": "s.quat"})
        self.assertEqual([d["nodes"][str(c)]["component"].get("overrides") for c in (c1, c2)], [None, None])
        self.assertEqual(d["nodes"][str(c1)]["fills"], before["nodes"][str(c1)]["fills"])
        self.assertEqual(sp.missing(d), [])
        again = copy.deepcopy(d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 2, "bindings": 0, "flags": 0})
        self.assertEqual(d, again)

    def test_record_leaves_a_settled_unflagged_copy_unflagged(self):
        d, (c,) = chain()
        settle(d, c, binding="s.quat")
        before = copy.deepcopy(d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 1, "bindings": 0, "flags": 0})
        self.assertEqual(d, before)

    def test_a_copy_whose_paint_was_not_written_stops_the_build(self):
        d, _ = chain()
        with self.assertRaises(SystemExit) as raised:
            sp.record(d)
        self.assertIn("Level 0/Header/Action/Icon/Glyph", str(raised.exception))

    def test_the_whole_pass_takes_every_copy_of_a_three_level_chain_from_stale_to_complete(self):
        d, copies = chain(3, corrected=False)
        self.assertEqual([m["id"] for m in sp.missing(d)], [NEW_GLYPH] * 3)  # the swapped glyph alone: while it is still reset the copies equal it
        sp.write_paints(mcp_for(d), d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 3, "bindings": 1, "flags": 1})  # the swapped glyph's flag alone
        self.assertEqual(sp.missing(d), [])
        self.assertTrue(all(d["nodes"][str(c)]["fills"] == [paint(GREY, 0.5)] for c in copies))
        self.assertTrue(all("overrides" not in d["nodes"][str(c)]["component"] for c in copies))
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["component"]["overrides"], ["Fills"])

    def test_an_engine_that_keeps_the_swap_leaves_nothing_to_write(self):
        """Every copy already carries paint and binding: no MCP call, the document is deep-equal, and the counts only count."""
        d, copies = chain(3)
        for c in copies:
            settle(d, c, binding="s.quat")
        before, m = copy.deepcopy(d), mcp_for(d)
        self.assertEqual(sp.write_paints(m, d), {"glyphs": 1, "copies": 3, "paints": 0})
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 3, "bindings": 0, "flags": 0})
        self.assertEqual((m.calls, d), ([], before))


if __name__ == "__main__":
    unittest.main()
