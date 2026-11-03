"""The swap-paint pass: the MCP write of paint 0 and the post-pass binding and `Fills` flag.

`FakeMcp` plays designos-mcp 0.7.5 as measured: `designos_update_node fills` replaces the node's paints and clears its variable bindings, `designos_save_file` writes
the live document. Synthetic native documents from test_studio_ui_file_swap_paints.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_paints_pass
"""
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_paints as sp  # noqa: E402
from test_studio_ui_file_swap_paints import GREY, NEW_GLYPH, WHITE, doc, fields, paint, reset_glyph  # noqa: E402


class FakeMcp:
    def __init__(self, document):
        self.doc, self.calls = copy.deepcopy(document), []  # the live session: apart from the saved snapshot `write_paints` is given

    def tool_or_die(self, name, args):
        self.calls.append((name, args))
        if name == "designos_update_node":
            n = self.doc["nodes"][str(args["node_id"])]
            n["fills"] = copy.deepcopy(args["fills"])
            n.pop("variables", None)  # measured on 0.7.5: a `fills` write clears the node's binding
        elif name == "designos_save_file":
            Path(self.workspace, args["path"]).write_text(json.dumps(self.doc), encoding="utf-8")
        return {}

    def updates(self):
        return [a for n, a in self.calls if n == "designos_update_node"]


def mcp_for(document, workspace=None):
    m = FakeMcp(document)
    m.workspace = workspace
    return m


class WritePaints(unittest.TestCase):
    def test_paint_0_gets_the_pre_swap_colour_and_opacity_through_one_fills_write(self):
        d = reset_glyph()
        m = mcp_for(d)
        counts = sp.write_paints(m, d)
        self.assertEqual(counts, {"glyphs": 1, "copies": 0, "paints": 1})
        (call,) = m.updates()
        self.assertEqual((call["node_id"], call["lint"]), (NEW_GLYPH, "off"))
        self.assertEqual(call["fills"], [paint(GREY, 0.5)])
        self.assertEqual(fields(d), ["binding", "flag"])  # the write is only the paint; the snapshot now carries it

    def test_the_glyphs_other_paints_visibility_and_blend_stay(self):
        d = reset_glyph()
        n = d["nodes"][str(NEW_GLYPH)]
        n["fills"] = [dict(paint(WHITE, 1.0), visible=True, blend="Multiply"), paint(WHITE, 0.2)]
        m = mcp_for(d)
        sp.write_paints(m, d)
        self.assertEqual(m.updates()[0]["fills"], [dict(paint(GREY, 0.5), blend="Multiply"), paint(WHITE, 0.2)])

    def test_a_glyph_that_already_carries_the_paint_is_not_written(self):
        d = doc()
        m = mcp_for(d)
        self.assertEqual(sp.write_paints(m, d), {"glyphs": 1, "copies": 0, "paints": 0})
        self.assertEqual(m.calls, [])

    def test_a_pre_swap_glyph_with_no_solid_paint_stops_the_build(self):
        d = reset_glyph()
        d["nodes"]["22"]["fills"] = []
        with self.assertRaises(SystemExit) as raised:
            sp.write_paints(mcp_for(d), d)
        self.assertIn("cannot keep the pre-swap paint", str(raised.exception))

    def test_apply_in_session_saves_reads_back_and_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = reset_glyph()
            m = mcp_for(d, tmp)
            self.assertEqual(sp.apply_in_session(m, tmp), {"glyphs": 1, "copies": 0, "paints": 1})
            self.assertEqual([n for n, _ in m.calls], ["designos_save_file", "designos_update_node"])
            self.assertEqual(m.calls[0][1], {"path": sp.SAVED, "overwrite": True})
        self.assertEqual(m.doc["nodes"][str(NEW_GLYPH)]["fills"], [paint(GREY, 0.5)])

    def test_a_glyph_that_copies_a_corrected_glyph_sees_the_corrected_paint(self):
        """Settled in order: the second swap's pre-swap glyph is the first swap's glyph, written a moment earlier in the same snapshot."""
        d = reset_glyph()
        n = d["nodes"]
        n["40"] = {"id": 40, "name": "Icon", "kind": {"Instance": {"frame": {}, "component": 12}}, "parent": 31, "children": [41], "component": {"source": 32}}
        n["41"] = {"id": 41, "name": "Glyph", "kind": "Vector", "parent": 40, "children": [], "component": {"source": 13}, "fills": [paint(WHITE, 1.0)]}
        n["31"]["children"].append(40)
        m = mcp_for(d)
        sp.write_paints(m, d)
        self.assertEqual([a["node_id"] for a in m.updates()], [33, 41])
        self.assertEqual(m.updates()[1]["fills"], [paint(GREY, 0.5)])


class Record(unittest.TestCase):
    def written(self, **kw):
        d = doc(binding=None, flag=False, **kw)  # the paint is written, the binding and the flag are not
        return d

    def test_record_writes_the_pre_swap_binding_and_the_flag(self):
        d = self.written()
        counts = sp.record(d)
        self.assertEqual(counts, {"glyphs": 1, "copies": 0, "bindings": 1, "flags": 1})
        n = d["nodes"][str(NEW_GLYPH)]
        self.assertEqual(n["variables"], {"bindings": {"fills/0": "s.quat"}, "modes": {}})
        self.assertEqual(n["component"]["overrides"], ["Fills"])
        self.assertEqual(sp.missing(d), [])

    def test_record_keeps_the_flags_it_finds_and_adds_one(self):
        d = self.written()
        d["nodes"][str(NEW_GLYPH)]["component"]["overrides"] = ["Size"]
        sp.record(d)
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["component"]["overrides"], ["Size", "Fills"])

    def test_record_twice_changes_nothing_the_second_time(self):
        d = self.written()
        sp.record(d)
        once = copy.deepcopy(d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 0, "bindings": 0, "flags": 0})
        self.assertEqual(d, once)

    def test_a_glyph_that_already_carries_paint_binding_and_flag_is_a_no_op(self):
        """An engine that keeps overrides across a swap (#1427 fixed) leaves nothing to write: no MCP call, no change to the document."""
        d = doc()
        before, m = copy.deepcopy(d), mcp_for(d)
        self.assertEqual(sp.write_paints(m, d), {"glyphs": 1, "copies": 0, "paints": 0})
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 0, "bindings": 0, "flags": 0})
        self.assertEqual((m.calls, d), ([], before))

    def test_a_missing_flag_alone_is_written_and_nothing_else(self):
        d = doc(flag=False)
        before = copy.deepcopy(d)
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 0, "bindings": 0, "flags": 1})
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["variables"], before["nodes"][str(NEW_GLYPH)]["variables"])
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["fills"], before["nodes"][str(NEW_GLYPH)]["fills"])

    def test_a_missing_binding_alone_is_written_and_nothing_else(self):
        d = doc(binding="s.icon")
        self.assertEqual(sp.record(d), {"glyphs": 1, "copies": 0, "bindings": 1, "flags": 0})
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["variables"]["bindings"], {"fills/0": "s.quat"})

    def test_a_paint_that_was_not_written_through_mcp_stops_the_build(self):
        d = reset_glyph()
        before = copy.deepcopy(d)
        with self.assertRaises(SystemExit) as raised:
            sp.record(d)
        self.assertIn("03 Studio/State=Default/Action/Icon/Glyph", str(raised.exception))
        self.assertEqual(d["nodes"][str(NEW_GLYPH)]["fills"], before["nodes"][str(NEW_GLYPH)]["fills"])  # never patched into the file

    def test_the_whole_pass_takes_the_reset_glyph_to_complete(self):
        d = reset_glyph()
        self.assertEqual(fields(d), ["paint", "binding", "flag"])
        sp.write_paints(mcp_for(d), d)
        sp.record(d)
        self.assertEqual(sp.missing(d), [])


if __name__ == "__main__":
    unittest.main()
