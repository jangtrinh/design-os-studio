"""The layout pass on two pages and on a plain component: the sections of each page start at x 0, and a plain component is its own box (no variant grid, no set to resize).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_place_pages
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_core as core  # noqa: E402
import library_layout as layout  # noqa: E402

IDS = {n: i for i, n in enumerate(["setA", "vA", "labA", "secA", "setB", "labB", "secB", "plain", "ringP", "labP", "secP"], start=1)}


class FakeMcp:
    def __init__(self, sizes):
        self.sizes, self.ops = sizes, []

    def tool_or_die(self, tool, args):
        if tool == "designos_list_nodes":
            w, h = self.sizes[IDS["vA"]]
            return {"nodes": [{"name": "State=Default", "width": w, "height": h}], "next_offset": None}
        if tool == "designos_get_node":
            w, h = self.sizes[args["node_id"]]
            return {"node": {"size": {"x": w, "y": h}}}
        assert tool == "designos_apply_batch", tool
        self.ops += args["ops"]
        return {}

    def op(self, name):
        return next(op for op in self.ops if op["node"] == IDS[name])


def a_set():
    return {"id": "setA", "name": "Menu/Panel", "label": "labA", "cols": 1, "section": "secA", "page": "02 Core",
            "variants": [{"id": "vA", "name": "State=Default", "texts": [], "rings": []}]}


def plain(page="03 Studio", rings=()):
    return {"id": "plain", "name": "Card/Library", "label": "labB", "cols": 1, "plain": True, "section": "secB", "page": page,
            "variants": [{"id": "plain", "name": "", "texts": [], "rings": list(rings)}]}


class PlacePagesTest(unittest.TestCase):
    SIZES = {IDS["vA"]: (100.0, 50.0), IDS["plain"]: (300.0, 120.0), IDS["ringP"]: (0.0, 0.0)}

    def run_place(self, info, sections):
        mcp = FakeMcp(self.SIZES)
        layout.place(mcp, IDS, info, sections)
        return mcp

    def test_the_first_section_of_each_page_starts_at_x_zero(self):
        mcp = FakeMcp(self.SIZES)
        info = [a_set(), dict(a_set(), id="setC", label="labB", section="secB", page="03 Studio")]
        IDS.update(setC=IDS["setB"])
        layout.place(mcp, IDS, info, {"secA": ["setA"], "secB": ["setC"]})
        self.assertEqual((mcp.op("secA")["x"], mcp.op("secB")["x"]), (0, 0))

    def test_sections_of_one_page_still_follow_each_other(self):
        info = [a_set(), dict(a_set(), id="setC", label="labB", section="secB")]
        IDS.update(setC=IDS["setB"])
        mcp = self.run_place(info, {"secA": ["setA"], "secB": ["setC"]})
        self.assertEqual(mcp.op("secB")["x"], mcp.op("secA")["x"] + mcp.op("secA")["width"] + core.GAP)

    def test_a_set_wraps_its_variant_grid_with_32_px_on_each_side(self):
        mcp = self.run_place([a_set()], {"secA": ["setA"]})
        self.assertEqual((mcp.op("setA")["width"], mcp.op("setA")["height"]), (100.0 + 64, 50.0 + 64))

    def test_a_plain_component_is_not_resized_or_wrapped(self):
        mcp = self.run_place([plain()], {"secB": ["plain"]})
        self.assertFalse([op for op in mcp.ops if op["node"] == IDS["plain"] and ("width" in op or "height" in op)])
        place = mcp.op("plain")
        self.assertEqual((place["x"], place["y"]), (core.PAD, core.PAD + core.HEADER + core.LABEL))

    def test_the_section_around_a_plain_component_fits_its_own_size(self):
        mcp = self.run_place([plain()], {"secB": ["plain"]})
        sec = mcp.op("secB")
        self.assertEqual((sec["width"], sec["height"]), (300.0 + 2 * core.PAD, core.PAD + core.HEADER + core.LABEL + 120.0 + core.PAD))

    def test_a_ring_in_a_plain_component_is_sized_from_the_component(self):
        mcp = self.run_place([plain(rings=[{"id": "ringP", "parent": "plain"}])], {"secB": ["plain"]})
        ring = mcp.op("ringP")
        self.assertEqual((ring["x"], ring["y"], ring["width"], ring["height"]), (-4, -4, 308.0, 128.0))

    def test_the_report_names_a_plain_component_by_the_record(self):
        mcp = FakeMcp(self.SIZES)
        report = layout.place(mcp, IDS, [plain()], {"secB": ["plain"]})
        self.assertEqual(report, {"Card/Library": [300.0, 120.0]})


if __name__ == "__main__":
    unittest.main()
