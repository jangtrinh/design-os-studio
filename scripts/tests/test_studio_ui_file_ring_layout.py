"""The layout pass sizes a Focus/Ring instance from its parent, the node it rings, not from the variant it sits somewhere inside:
the parent grown by SPACE_FACT on every side, at (-SPACE_FACT, -SPACE_FACT). A checkbox rings its 16 px box, not its control rect
(`kit/choice.rs:104-111`), so a ring inside a box inside a row must be 16 + 8 wide whatever the row's width. A fake MCP records the ops.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_ring_layout
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_core as core  # noqa: E402
import library_layout as layout  # noqa: E402

IDS = {"s0": 1, "v0": 2, "lab": 3, "sec": 4, "ring": 5, "box": 6, "text": 7}


class FakeMcp:
    """designos_list_nodes of the set (its variants), designos_get_node by native id, designos_apply_batch (ops recorded)."""

    def __init__(self, variant, sizes):
        self.variant, self.sizes, self.ops = variant, sizes, []

    def tool_or_die(self, tool, args):
        if tool == "designos_list_nodes":
            w, h = self.sizes[IDS["v0"]]
            return {"nodes": [{"name": self.variant, "width": w, "height": h}], "next_offset": None}
        if tool == "designos_get_node":
            w, h = self.sizes[args["node_id"]]
            return {"node": {"size": {"x": w, "y": h}}}
        assert tool == "designos_apply_batch", tool
        self.ops += args["ops"]
        return {}


def info(rings):
    return [{"id": "s0", "name": "Checkbox", "label": "lab", "cols": 1, "section": "sec",
             "variants": [{"id": "v0", "name": "State=Focus", "texts": [], "rings": rings}]}]


class RingSizeTest(unittest.TestCase):
    def ring_op(self, rings, sizes):
        mcp = FakeMcp("State=Focus", sizes)
        layout.place(mcp, IDS, info(rings), {"sec": ["s0"]})
        return next(op for op in mcp.ops if op["node"] == IDS["ring"])

    def test_a_ring_directly_in_the_variant_is_the_variant_grown_by_space_fact_on_every_side(self):
        op = self.ring_op([{"id": "ring", "parent": "v0"}], {IDS["v0"]: (100.0, 40.0)})
        self.assertEqual((op["x"], op["y"], op["width"], op["height"]), (-4, -4, 108.0, 48.0))

    def test_a_ring_in_a_nested_box_is_sized_from_that_box_not_from_the_variant(self):
        """The row is 240 x 32 and the box in it is 16 x 16: the ring wraps the box (24 x 24), not the row (248 x 40)."""
        op = self.ring_op([{"id": "ring", "parent": "box"}], {IDS["v0"]: (240.0, 32.0), IDS["box"]: (16.0, 16.0)})
        self.assertEqual((op["x"], op["y"], op["width"], op["height"]), (-4, -4, 24.0, 24.0))

    def test_every_ring_of_a_variant_gets_an_op(self):
        mcp = FakeMcp("State=Focus", {IDS["v0"]: (100.0, 40.0), IDS["box"]: (16.0, 16.0)})
        layout.place(mcp, IDS, info([{"id": "ring", "parent": "v0"}, {"id": "text", "parent": "box"}]), {"sec": ["s0"]})
        sized = {op["node"]: (op["width"], op["height"]) for op in mcp.ops if "width" in op and op["node"] in (IDS["ring"], IDS["text"])}
        self.assertEqual(sized, {IDS["ring"]: (108.0, 48.0), IDS["text"]: (24.0, 24.0)})


class RingInfoTest(unittest.TestCase):
    def test_a_ring_is_found_with_the_id_of_the_node_it_sits_in(self):
        rest = {"id": "v", "type": "COMPONENT", "children": [
            {"id": "box", "type": "FRAME", "name": "Box", "children": [{"id": "r", "type": "INSTANCE", "name": "Focus ring", "children": []}]},
            {"id": "r2", "type": "INSTANCE", "name": "Focus ring", "children": []},
            {"id": "i", "type": "INSTANCE", "name": "Icon", "children": []}]}
        self.assertEqual(core.rings_of(rest), [{"id": "r", "parent": "box"}, {"id": "r2", "parent": "v"}])


if __name__ == "__main__":
    unittest.main()
