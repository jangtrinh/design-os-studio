"""The engine half of `X(.., sx="fill")` on a hugging main (needs the installed designos-mcp; skipped where it is absent). A copy that keeps the main's own Hug next to `layout_child` Fill is
re-hugged by the engine and stays as wide as its label whatever the parent does (the W2c review's probe B: 60.75 px in a 241 px card, 54.88 px here). The copy that writes its own Fixed fills the parent and keeps
filling when the parent is resized, and its frame differs from the main's, which the post-pass records as the `Layout` override the editor would.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_fill_engine
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import mcp_client  # noqa: E402
import rest_ir as ir  # noqa: E402
import scratch_workspace as scratch  # noqa: E402
import size_overrides as so  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

HAVE_MCP = Path(mcp_client.INSTALLED_MCP).exists()


def rest_file(copy_kw, own_sizing):
    """A REST file with a hugging main `Chip` and a 241 px wide column `Host` holding one copy of it; `own_sizing` False reproduces the generator's old shape (the main's own sizing kept)."""
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": "Label#set0:0"}
    main = low.component("Chip", "", ir.F("Chip", [ir.T("Label", "A label", "Body / Medium", "color/line", prop="Label")], dir="H", fill="color/bg", h=32, sx="hug", sy="fixed"), 0, 0)
    if not own_sizing:
        low.own_sizing = lambda node, m: {}
    host = low.lower(ir.F("Host", [ir.X("Copy", "Chip", "", **copy_kw)], dir="V", pad=None, w=241, h=100, sx="fixed", sy="fixed", align=("start", "start")), 400, 0, ptype="COMPONENT")
    main["name"], host["name"] = "Chip", "Host"
    canvas = {"id": "page:0", "type": "CANVAS", "name": "Page", "children": [main, host]}
    return {"name": "t", "document": {"id": "0:0", "type": "DOCUMENT", "children": [canvas]},
            "components": {main["id"]: {"key": "t/Chip", "name": "Chip"}, host["id"]: {"key": "t/Host", "name": "Host"}}, "componentSets": {}}


def children(mcp, nid):
    return {n["name"]: n["node_id"] for n in mcp.tool_or_die("designos_list_nodes", {"parent_id": nid, "limit": 500})["nodes"]}


def probe(ws, copy_kw, own_sizing):
    """(width of the copy after the host is resized to 251, after it is resized to 301, the native document after save): a layout runs when a size changes, not on open."""
    (ws / "t.json").write_text(json.dumps(rest_file(copy_kw, own_sizing)))
    mcp = mcp_client.Mcp(str(ws))
    try:
        mcp.tool_or_die("designos_open_file", {"path": "t.json", "discard_changes": True})
        page = mcp.tool_or_die("designos_get_document", {})["pages"][0]["node_id"]
        host = children(mcp, page)["Host"]
        copy = children(mcp, host)["Copy"]
        width = lambda: mcp.tool_or_die("designos_get_node", {"node_id": copy})["node"]["size"]["x"]
        mcp.tool_or_die("designos_update_node", {"node_id": host, "width": 251})
        first = width()
        mcp.tool_or_die("designos_update_node", {"node_id": host, "width": 301})
        second = width()
        mcp.tool_or_die("designos_save_file", {"path": "t.designos", "overwrite": True})
    finally:
        mcp.close()
    return first, second, json.loads((ws / "t.designos").read_text())


@unittest.skipUnless(HAVE_MCP, "the installed designos-mcp is needed")
class EngineTest(unittest.TestCase):
    def test_the_old_shape_stays_as_wide_as_its_label_and_the_new_one_fills_and_keeps_filling(self):
        with scratch.workspace(None, prefix="studio-ui-fill-engine-") as ws:
            old_first, old_second, _ = probe(ws, {"sx": "fill"}, own_sizing=False)
            new_first, new_second, doc = probe(ws, {"sx": "fill"}, own_sizing=True)
        self.assertEqual(old_first, old_second, "the re-hugged copy ignores its parent")
        self.assertLess(old_first, 100)
        self.assertEqual((new_first, new_second), (251.0, 301.0))
        self.assertIsNotNone(doc)

    def test_the_copy_differs_from_its_main_only_by_layout_and_the_post_pass_records_it(self):
        with scratch.workspace(None, prefix="studio-ui-fill-engine-") as ws:
            _, _, doc = probe(ws, {"sx": "fill"}, own_sizing=True)
        missing = [m for m in so.missing(doc) if m["name"] == "Copy"]
        self.assertEqual(sorted(("Layout" in m["problem"], "Size" in m["problem"]) for m in missing), [(False, True), (True, False)], "Size (301 against the main's width) and Layout (its own sizing)")
        so.record(doc)
        self.assertEqual([m for m in so.missing(doc) if m["name"] == "Copy"], [])
        copy = next(n for n in doc["nodes"].values() if n["name"] == "Copy")
        self.assertIn("Layout", copy["component"]["overrides"])
        self.assertEqual(copy["kind"]["Instance"]["frame"]["auto_layout"]["primary_sizing"], "Fixed")


if __name__ == "__main__":
    unittest.main()
