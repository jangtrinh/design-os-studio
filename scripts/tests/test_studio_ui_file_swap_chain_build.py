"""The chain copies inside the build: the library verify refuses a copy that dropped its paint or binding, passes a copy that carries no flag (the editor makes none), and refuses
a count that differs from the recorded one; the native post-pass settles the copies, and every swapped glyph, before the mirror runs. Synthetic documents from swap_chain_fixtures.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_chain_build
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_verify  # noqa: E402
import native_post_pass as post  # noqa: E402
import swap_paints as sp  # noqa: E402
from swap_chain_fixtures import chain, settle  # noqa: E402
from test_studio_ui_file_swap_paints import GREY, NEW_GLYPH, WHITE, paint  # noqa: E402


def saved(document):
    tmp = tempfile.TemporaryDirectory()
    path = Path(tmp.name) / "f.designos"
    path.write_text(json.dumps(document), encoding="utf-8")
    return tmp, path


def verify(document, *recorded):
    tmp, path = saved(document)
    with tmp:
        return library_verify.verify(None, path, {"semantic": []}, [], *recorded)


def refusal(test, document, *recorded):
    with test.assertRaises(SystemExit) as raised:
        verify(document, *recorded)
    return str(raised.exception)


class Verify(unittest.TestCase):
    def one_copy(self):
        d, (c,) = chain()
        settle(d, c, binding="s.quat")
        return d, c

    def test_the_build_refuses_a_chain_copy_that_kept_the_stale_paint(self):
        d, c = self.one_copy()
        d["nodes"][str(c)]["fills"] = [paint(WHITE, 1.0)]
        text = refusal(self, d)
        self.assertIn(f"chain copy {c} (paint)", text)
        self.assertNotIn("(binding)", text)
        self.assertNotIn("(flag)", text)

    def test_the_build_refuses_a_chain_copy_bound_to_another_variable(self):
        d, c = self.one_copy()
        settle(d, c, binding="s.icon")
        text = refusal(self, d)
        self.assertIn(f"chain copy {c} (binding)", text)
        self.assertNotIn("(paint)", text)
        self.assertNotIn("(flag)", text)

    def test_the_build_passes_a_chain_copy_that_carries_no_fills_flag(self):
        """The editor's own copy of a flagged glyph is unflagged (`copy_rec`): the verify must not call it broken, or it would refuse a file a user can make in Studio."""
        d, c = self.one_copy()
        self.assertNotIn("overrides", d["nodes"][str(c)]["component"])
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["component"]["overrides"], ["Fills"])
        out = verify(d, {"glyphs": 1, "copies": 1})
        self.assertEqual((out["swap_paints_verified"], out["chain_copies_verified"]), (1, 1))

    def test_the_build_passes_complete_chain_copies_and_counts_them(self):
        d, copies = chain(2)
        for c in copies:
            settle(d, c, binding="s.quat")
        out = verify(d, {"glyphs": 1, "copies": 2})
        self.assertEqual((out["swap_paints_verified"], out["chain_copies_verified"]), (1, 2))

    def test_the_build_refuses_a_file_that_verifies_a_different_number_of_chain_copies_than_it_recorded(self):
        d, copies = chain(2)
        for c in copies:
            settle(d, c, binding="s.quat")
        self.assertIn("2 chain copies", refusal(self, d, {"glyphs": 1, "copies": 3}))
        self.assertIn("recorded 1 and 1", refusal(self, d, {"glyphs": 1, "copies": 1}))

    def test_a_copy_whose_source_link_was_lost_leaves_nothing_to_check_and_the_count_catches_it(self):
        d, c = self.one_copy()
        del d["nodes"][str(d["nodes"][str(c)]["parent"])]["component"]
        self.assertEqual(sp.missing(d), [])
        self.assertIn("0 chain copies", refusal(self, d, {"glyphs": 1, "copies": 1}))


class PostPass(unittest.TestCase):
    def run_post_pass(self, document):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp) / "in.designos", Path(tmp) / "out.designos"
            src.write_text(json.dumps(document), encoding="utf-8")
            counts = post.post_pass(src, dst, {"semantic": []}, {})
            return counts, json.loads(dst.read_text(encoding="utf-8"))

    def test_the_post_pass_settles_the_copies_of_a_swapped_glyph_without_flagging_them(self):
        """The copies' paint was written through MCP; the pass adds no flag to a copy, and the mirror then stamps the binding the corrected glyph carries."""
        d, copies = chain(2)
        for c in copies:
            settle(d, c)
        counts, out = self.run_post_pass(d)
        self.assertEqual(counts["swap_paints"], {"glyphs": 1, "copies": 2, "bindings": 0, "flags": 0})
        for c in copies:
            glyph = out["nodes"][str(c)]
            self.assertEqual((glyph["component"].get("overrides"), glyph["variables"]["bindings"]), (None, {"fills/0": "s.quat"}))
            self.assertEqual(glyph["fills"], [paint(GREY, 0.5)])
        self.assertEqual(out["nodes"][str(NEW_GLYPH)]["component"]["overrides"], ["Fills"])
        self.assertEqual(sp.missing(out), [])

    def test_the_swap_pass_has_settled_every_glyph_when_the_mirror_runs(self):
        """The mirror only fills a binding a node lacks and would stamp the Icon main's `s.icon` on the swapped glyph and on every copy: the record must have run before it."""
        d, copies = chain(2)
        swapped = d["nodes"][str(NEW_GLYPH)]
        swapped.pop("variables")
        swapped["component"].pop("overrides")  # the pass has not recorded the swapped glyph yet; the copies' paint is written
        for c in copies:
            settle(d, c)
        self.assertNotEqual(sp.missing(d), [])  # the pass has work to do before the mirror
        seen = []
        real = post.mirror_sources.mirror

        def spy(doc, allow=()):
            seen.append(sp.missing(doc))
            return real(doc, allow)

        with mock.patch.object(post.mirror_sources, "mirror", spy):
            self.run_post_pass(d)
        self.assertEqual(seen, [[]])


if __name__ == "__main__":
    unittest.main()
