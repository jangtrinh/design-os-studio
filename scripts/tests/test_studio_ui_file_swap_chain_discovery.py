"""Which glyphs the swap-paint pass counts as chain copies: structure alone, at any depth, settled after the glyph each copies. Synthetic documents from swap_chain_fixtures.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_chain_discovery
"""
import itertools
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_paints as sp  # noqa: E402
import swap_targets as tg  # noqa: E402
from swap_chain_fixtures import chain, nest, settle  # noqa: E402
from test_studio_ui_file_swap_paints import CTRL, NEW_GLYPH, NEW_SLOT, OLD_GLYPH, PAGE, PLUS, SEARCH, SEARCH_GLYPH, WHITE, inst, node, paint, reset_glyph  # noqa: E402
from test_studio_ui_file_swap_paints_pass import mcp_for  # noqa: E402


def pairs(found):
    return [(g["id"], p["id"]) for g, p in found]


class Discovery(unittest.TestCase):
    def test_a_copy_of_a_swapped_glyph_is_a_chain_copy_and_not_a_swap(self):
        d, (c,) = chain()
        self.assertEqual(pairs(sp.chain_copies(d)), [(c, NEW_GLYPH)])
        self.assertEqual(pairs(sp.swapped(d)), [(NEW_GLYPH, OLD_GLYPH)])

    def test_a_chain_copy_is_found_at_any_depth_after_the_glyph_it_copies(self):
        d, (c1, c2, c3) = chain(3)
        self.assertEqual(pairs(sp.chain_copies(d)), [(c1, NEW_GLYPH), (c2, c1), (c3, c2)])
        self.assertEqual([g["id"] for g, _, _ in tg.targets(d)], [NEW_GLYPH, c1, c2, c3])

    def test_a_copy_is_settled_after_the_glyph_it_copies_whatever_their_node_ids_are(self):
        """The outer copies get the lower node ids here, so a walk in id order meets a copy before its source: the pass must still write the source first."""
        d, (c1, c2, c3) = chain(3, corrected=False, descending=True)
        self.assertLess(c3, c2)
        self.assertLess(c2, c1)
        self.assertEqual([g["id"] for g, _, _ in tg.targets(d)], [NEW_GLYPH, c1, c2, c3])
        m = mcp_for(d)
        sp.write_paints(m, d)
        self.assertEqual([a["node_id"] for a in m.updates()], [NEW_GLYPH, c1, c2, c3])
        sp.record(d)
        self.assertEqual(sp.missing(d), [])

    def test_a_plain_copy_of_a_glyph_that_is_no_target_is_left_alone(self):
        """CTRL's own slot is a plain copy of the Icon main's (OLD_SLOT draws Icon/Search directly): nesting CTRL elsewhere copies a glyph the pass never touches."""
        d = reset_glyph()
        d["nodes"]["100"] = node(100, "Level 0", {"Component": {}}, PAGE)
        nest(d, CTRL, 100, itertools.count(101))
        self.assertEqual(sp.chain_copies(d), [])
        self.assertEqual(pairs(sp.swapped(d)), [(NEW_GLYPH, OLD_GLYPH)])

    def test_a_file_with_no_swap_has_no_chain_copy(self):
        d, _ = chain(2, corrected=False)
        d["nodes"][str(NEW_SLOT)]["component"]["source"] = PLUS  # a slot a module draws directly: no pre-swap node, so no swap, so nothing to copy
        self.assertEqual((sp.swapped(d), sp.chain_copies(d), sp.missing(d)), ([], [], []))

    def test_a_copy_is_found_by_structure_whatever_it_carries(self):
        """A settled copy is still a chain copy: a reopened file must keep listing it, or the verify would pass without checking it."""
        d, (c,) = chain()
        settle(d, c, binding="s.quat")
        self.assertEqual(pairs(sp.chain_copies(d)), [(c, NEW_GLYPH)])
        self.assertEqual(sp.missing(d), [])

    def test_a_copy_of_a_copy_that_reaches_no_swap_does_not_loop(self):
        d = reset_glyph()
        for sid, other in ((40, 42), (42, 40)):  # two slots of the same icon that copy each other
            d["nodes"][str(sid)] = node(sid, "Icon", inst(SEARCH), PAGE, [sid + 1], source=other)
            d["nodes"][str(sid + 1)] = node(sid + 1, "Glyph", "Vector", sid, source=SEARCH_GLYPH, fills=[paint(WHITE, 1.0)])
        self.assertEqual(sp.chain_copies(d), [])

    def test_a_re_swap_inside_a_chain_copy_is_settled_after_the_copy(self):
        """Composite 2 nests composite 1 and swaps the icon of the copied slot back to Search: its pre-swap glyph is the chain copy, so the copy comes first."""
        d, (c1,) = chain()
        comp2 = 200
        d["nodes"][str(comp2)] = node(comp2, "Level 1", {"Component": {}}, PAGE)
        made = nest(d, 100, comp2, itertools.count(201))
        slot = d["nodes"][str(made[d["nodes"][str(c1)]["parent"]])]
        slot["kind"] = inst(SEARCH)
        r = made[c1]
        self.assertEqual([(g["id"], swap) for g, _, swap in tg.targets(d)], [(NEW_GLYPH, True), (c1, False), (r, True)])
        self.assertEqual(pairs(sp.swapped(d))[-1], (r, c1))


if __name__ == "__main__":
    unittest.main()
