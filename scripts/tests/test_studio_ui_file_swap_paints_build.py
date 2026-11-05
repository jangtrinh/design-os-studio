"""The swap-paint pass inside the build: the native post-pass order, the library verify assertion, and the isolation of the swap check.

Synthetic native documents from test_studio_ui_file_swap_paints.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_paints_build
"""
import ast
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_verify  # noqa: E402
import mirror_sources  # noqa: E402
import native_post_pass as post  # noqa: E402
from test_studio_ui_file_swap_paints import ACTION, CTRL, GREY, HOST, NEW_GLYPH, NEW_SLOT, OLD_GLYPH, OLD_SLOT, PAGE, PLUS, SEARCH_GLYPH, doc, inst, node, paint, reset_glyph  # noqa: E402

STUDIO = HERE.parent / "studio_ui_file"


class PostPass(unittest.TestCase):
    def run_post_pass(self, document, plan=None, intents=()):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp) / "in.designos", Path(tmp) / "out.designos"
            src.write_text(json.dumps(document), encoding="utf-8")
            counts = post.post_pass(src, dst, plan or {"semantic": []}, {}, intents)
            return counts, json.loads(dst.read_text(encoding="utf-8"))

    def test_the_swap_record_runs_after_the_bind_intents(self):
        """The pre-swap glyph is bound by a bind intent: the file the post-pass reads carries no binding of its own on it (only its source's `s.icon`). `record` copies that binding
        onto the swapped glyph, so it must run after `apply_intents`; before them it sees `s.icon` on both glyphs, writes nothing, and the mirror then stamps `s.icon`."""
        old = node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[paint(GREY, 0.5)], flags=["Fills"])
        old["source_id"] = "rest:old-glyph"
        d = doc(binding=None, flag=False, old=old)
        d["variables"] = {"s.quat": {"id": "s.quat"}}  # post_pass writes the code syntax of every semantic variable of the plan
        plan = {"semantic": [{"id": "s.quat", "name": "color/icon-quaternary", "code_syntax": "theme::ICON_QUATERNARY"}]}
        counts, out = self.run_post_pass(d, plan, [{"sid": "rest:old-glyph", "key": "fills/0", "var": "color/icon-quaternary"}])
        self.assertEqual(counts["bindings"], 1)  # the intent
        self.assertEqual(counts["swap_paints"], {"glyphs": 1, "copies": 0, "bindings": 1, "flags": 1})
        self.assertEqual(out["nodes"][str(OLD_GLYPH)]["variables"]["bindings"], {"fills/0": "s.quat"})
        self.assertEqual(out["nodes"][str(NEW_GLYPH)]["variables"]["bindings"], {"fills/0": "s.quat"})

    def test_the_post_pass_records_the_swap_before_the_mirror_stamps_the_icon_main_s_binding(self):
        counts, out = self.run_post_pass(doc(binding=None, flag=False))
        self.assertEqual(counts["swap_paints"], {"glyphs": 1, "copies": 0, "bindings": 1, "flags": 1})
        glyph = out["nodes"][str(NEW_GLYPH)]
        self.assertEqual(glyph["variables"]["bindings"], {"fills/0": "s.quat"})  # mirror_sources only fills a key the node lacks
        self.assertEqual(glyph["component"]["overrides"], ["Fills"])

    def test_a_nested_copy_of_the_swapped_glyph_ends_with_the_corrected_binding(self):
        """A composite nests HOST: its copy of the swapped glyph is a chain copy whose paint is already written. Its binding is the corrected glyph's, which the mirror stamps."""
        d = doc(binding=None, flag=False)
        n = d["nodes"]
        n["50"] = node(50, "State=Default", {"Component": {}}, PAGE, [51])
        n["51"] = node(51, "Header", inst(HOST), 50, [52], source=HOST)
        n["52"] = node(52, "Action", inst(CTRL), 51, [53], source=ACTION)
        n["53"] = node(53, "Icon", inst(PLUS), 52, [54], source=NEW_SLOT)
        n["54"] = node(54, "Glyph", "Vector", 53, source=NEW_GLYPH, fills=[paint(GREY, 0.5)])
        n[str(PAGE)]["children"].append(50)
        _, out = self.run_post_pass(d)
        self.assertEqual(out["nodes"]["54"]["variables"]["bindings"], {"fills/0": "s.quat"})
        self.assertEqual(out["nodes"][str(NEW_GLYPH)]["variables"]["bindings"], {"fills/0": "s.quat"})

    def test_without_the_swap_pass_the_mirror_would_have_stamped_s_icon(self):
        d = doc(binding=None, flag=False)
        mirror_sources.mirror(d)
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["variables"]["bindings"], {"fills/0": "s.icon"})

    def test_a_file_with_no_swapped_glyph_reports_zero(self):
        d = doc()
        d["nodes"]["32"]["component"]["source"] = 10
        counts, _ = self.run_post_pass(d)
        self.assertEqual(counts["swap_paints"], {"glyphs": 0, "copies": 0, "bindings": 0, "flags": 0})


class VerifyAndIsolation(unittest.TestCase):
    def test_the_build_refuses_a_swapped_glyph_that_lost_its_paint_on_the_saved_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "f.designos"
            path.write_text(json.dumps(reset_glyph()), encoding="utf-8")
            with self.assertRaises(SystemExit) as raised:
                library_verify.verify(None, path, {"semantic": []}, [])
        text = str(raised.exception)
        self.assertIn("03 Studio/State=Default/Action/Icon/Glyph", text)
        self.assertIn("pre-swap glyph", text)

    def refusal(self, document, *recorded):
        """The text of the SystemExit that `library_verify.verify` raises on `document` saved as the reopened file."""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "f.designos"
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaises(SystemExit) as raised:
                library_verify.verify(None, path, {"semantic": []}, [], *recorded)
        return str(raised.exception)

    def test_the_build_refuses_a_swapped_glyph_that_lost_only_its_fills_flag(self):
        """The flag is read nowhere else: a reopen that dropped it would let the editor's next main sync reset the glyph."""
        text = self.refusal(doc(flag=False))
        self.assertIn("(flag)", text)
        self.assertNotIn("(paint)", text)
        self.assertNotIn("(binding)", text)

    def test_the_build_refuses_a_swapped_glyph_bound_to_another_variable(self):
        text = self.refusal(doc(binding="s.icon"))
        self.assertIn("(binding)", text)
        self.assertIn("s.quat", text)
        self.assertNotIn("(paint)", text)
        self.assertNotIn("(flag)", text)

    def test_the_build_passes_a_complete_swapped_glyph_and_counts_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "f.designos"
            path.write_text(json.dumps(doc()), encoding="utf-8")
            self.assertEqual(library_verify.verify(None, path, {"semantic": []}, [])["swap_paints_verified"], 1)
            self.assertEqual(library_verify.verify(None, path, {"semantic": []}, [], {"glyphs": 1, "copies": 0})["swap_paints_verified"], 1)

    def test_the_build_refuses_a_file_that_verifies_fewer_swapped_glyphs_than_it_recorded(self):
        """A reopen that lost the `component.source` link the swap pass reads leaves no swapped glyph to check, and `missing` is then empty: the recorded count catches it."""
        lost = doc()
        del lost["nodes"][str(NEW_SLOT)]["component"]
        text = self.refusal(lost, {"glyphs": 1, "copies": 0})
        self.assertIn("0 swapped glyphs", text)
        self.assertIn("recorded 1", text)

    def test_the_build_refuses_a_file_that_verifies_more_swapped_glyphs_than_it_recorded(self):
        self.assertIn("1 swapped glyphs", self.refusal(doc(), {"glyphs": 0, "copies": 0}))

    def test_the_build_hands_the_recorded_counts_to_the_verify(self):
        tree = ast.parse((STUDIO / "assemble_library.py").read_text(encoding="utf-8"))
        (call,) = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and ast.unparse(n.func) == "library_verify.verify"]
        self.assertEqual(len(call.args) + len(call.keywords), 5)
        self.assertEqual(ast.unparse(call.args[4]), "stats['swap_paints']")  # the build's own count, not None: a None would skip the cross-check

    def test_the_build_writes_the_swap_paints_through_mcp_before_the_save_the_post_pass_reads(self):
        tree = ast.parse((STUDIO / "assemble_library.py").read_text(encoding="utf-8"))
        calls = [(n.lineno, ast.unparse(n.func)) for n in ast.walk(tree) if isinstance(n, ast.Call)]
        lines = {name: min(line for line, who in calls if who == name) for name in ("swap_paints.apply_in_session", "base.finish")}
        self.assertLess(lines["swap_paints.apply_in_session"], lines["base.finish"])

    def test_the_swap_check_never_reads_the_swap_pass(self):
        """swap_check places its own twins on 99 Archive and compares what the engine's swap kept: its evidence must not come from the pass that repairs the swap."""
        tree = ast.parse((STUDIO / "swap_check.py").read_text(encoding="utf-8"))
        imported = {a.name.split(".")[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        imported |= {n.module.split(".")[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom) and n.module}
        self.assertFalse(imported & {"swap_paints", "swap_targets", "native_post_pass", "library_verify", "mirror_sources"}, imported)


if __name__ == "__main__":
    unittest.main()
