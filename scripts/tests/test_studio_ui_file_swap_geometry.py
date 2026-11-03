"""The icon-swap check measures the glyph that is drawn, not only the slot's size. `snapshot` once read `slot.size` alone, so a glyph drawn 1.56x too
large past its 20 px slot (#1427's engine head scaled `fill_geometry` twice) reported SWAP-OK. The renderer draws `fill_geometry` first and the
kind's paths only without it (`designos-render/src/geom.rs:416-420`), so the drawn bounds are the bounds of `fill_geometry` (curves sampled, not
their control points), placed through the glyph's transform. Three checks, each a reporting check while SWAP_CHECK_FATAL is False:

* the drawn bounds sit inside the slot;
* they equal the bounds of the glyph's own Icon main scaled to the glyph box (each twin carries its own icon, so one icon's ink is never compared
  with another's), within 1 px;
* the glyph box (the em box the icon is drawn in) is where the control's is, within 1 px.

The numbers below are the review's measurements of the #1427 head build (a 20 px slot) and of its fix-trial, in glyph space.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_geometry
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import glyph_bounds as gb  # noqa: E402
import swap_check as swap  # noqa: E402

GREY = {"r": 163 / 255, "g": 163 / 255, "b": 163 / 255, "a": 1.0}


def poly(x0, y0, x1, y1):
    """A closed path whose bounds are exactly (x0, y0)-(x1, y1)."""
    return {"cmds": [{"MoveTo": {"x": x0, "y": y0}}, {"LineTo": {"x": x1, "y": y0}}, {"LineTo": {"x": x1, "y": y1}}, {"LineTo": {"x": x0, "y": y1}}, "Close"],
            "fill_rule": "NonZero"}


def vector(nid, parent, size, bounds, fill_geometry=None, transform=None, name="Glyph"):
    node = {"id": nid, "name": name, "kind": {"Vector": {"paths": [poly(*bounds)]}}, "parent": parent, "children": [], "size": {"x": size, "y": size},
            "fills": [{"kind": {"Solid": GREY}, "opacity": 1.0, "visible": True}], "variables": {"bindings": {"fills/0": "s.color.icon"}}}
    if fill_geometry is not None:
        node["fill_geometry"] = [poly(*fill_geometry)]
    if transform is not None:
        node["transform"] = transform
    return node


def add_twin(doc, base, main_id, slot, glyph, paths, drawn, transform=None, name="Icon"):
    """A host, its icon slot (an instance of the main `main_id`, `slot` px) and the slot's glyph (`glyph` px, `paths` bounds, `drawn` fill_geometry bounds)."""
    n = doc["nodes"]
    n[str(base)] = {"id": base, "name": f"host{base}", "kind": {"Instance": {}}, "children": [base + 1], "parent": None}
    n[str(base + 1)] = {"id": base + 1, "name": name, "kind": {"Instance": {"component": main_id}}, "parent": base, "children": [base + 2],
                        "component": {"bindings": {"Visible": "Has Leading Icon"}}, "size": {"x": slot, "y": slot}}
    n[str(base + 2)] = vector(base + 2, base + 1, glyph, paths, drawn, transform)
    return base


def doc_with_mains(**mains):
    """{"nodes": ...} with one Component per name in `mains` (id, 16 px bounds): its single Glyph child is the icon at 16 px."""
    doc = {"nodes": {}}
    for i, (name, bounds) in enumerate(mains.items()):
        nid = 900 + 10 * i
        doc["nodes"][str(nid)] = {"id": nid, "name": f"Icon/{name}", "kind": {"Component": {}}, "children": [nid + 1], "size": {"x": 16.0, "y": 16.0}}
        doc["nodes"][str(nid + 1)] = vector(nid + 1, nid, 16.0, bounds, bounds)
    return doc


MAINS = {"Plus": (2.0, 2.0, 14.0, 14.0), "Check": (1.95, 3.95, 14.55, 13.0), "Move": (1.936, 1.936, 14.016, 14.0), "Trash": (2.0, 1.0, 14.0, 14.0),
         "Full": (0.0, 0.0, 16.0, 16.0)}
ID = {name: 900 + 10 * i for i, name in enumerate(MAINS)}
PLUS20, CHECK20 = (2.5, 2.5, 17.5, 17.5), (2.44, 4.94, 18.19, 16.25)
CHECK20_HEAD = (3.05, 6.17, 22.73, 20.31)  # fill_geometry scaled twice: the paths above, 1.25x again


def pair(control, swapped, **kw):
    """compare() of a control twin (Plus, 20 px glyph in a 20 px slot) with a swapped twin; `control` and `swapped` override add_twin arguments."""
    doc = doc_with_mains(**MAINS)
    c = dict(main_id=ID["Plus"], slot=20.0, glyph=20.0, paths=PLUS20, drawn=PLUS20)
    s = dict(main_id=ID["Check"], slot=20.0, glyph=20.0, paths=CHECK20, drawn=CHECK20)
    add_twin(doc, 10, **{**c, **control})
    add_twin(doc, 20, **{**s, **swapped})
    return swap.compare(swap.snapshot(doc, 10), swap.snapshot(doc, 20))


class DrawnGlyphTest(unittest.TestCase):
    def test_a_correct_swap_has_no_geometry_mismatch(self):
        """The fix-trial's numbers: the Check is drawn at its own bounds x 1.25 inside the 20 px slot, where the Plus control is."""
        self.assertEqual(pair({}, {}), [])

    def test_the_1427_head_geometry_fails_outside_the_slot_and_against_its_icon(self):
        """fill_geometry scaled twice: (3.05, 6.17)-(22.73, 20.31) in a 20 px slot. The size check read 20 and reported SWAP-OK."""
        out = pair({}, {"drawn": CHECK20_HEAD})
        self.assertEqual(len(out), 2, out)
        self.assertTrue(any("drawn outside its 20 x 20 slot" in m and "(3.05, 6.17)-(22.73, 20.31)" in m and m.startswith("swapped glyph") for m in out), out)
        self.assertTrue(any("scaled to the glyph box" in m and "(2.44, 4.94)-(18.19, 16.25)" in m for m in out), out)

    def test_a_glyph_that_is_too_large_but_still_inside_the_slot_fails_the_match(self):
        out = pair({}, {"drawn": (2.44, 4.94, 19.0, 17.0)})  # 0.8 px wide, 0.75 high of overshoot: within 1 px
        self.assertEqual(out, [])
        out = pair({}, {"drawn": (2.44, 4.94, 19.5, 17.0)})  # 1.3 px of overshoot
        self.assertEqual(len(out), 1, out)
        self.assertIn("scaled to the glyph box", out[0])

    def test_the_slot_boundary_is_exact_to_a_hundredth_of_a_pixel(self):
        """An icon whose ink fills its 16 px box reaches exactly 20 in a 20 px slot: that is inside. Half a pixel more is outside (and still
        within 1 px of the icon's own bounds, so only the slot check speaks)."""
        full = {"main_id": ID["Full"]}
        self.assertEqual(pair({}, {**full, "paths": (0.0, 0.0, 20.0, 20.0), "drawn": (0.0, 0.0, 20.0, 20.0)}), [])
        self.assertEqual(pair({}, {**full, "paths": (0.0, 0.0, 20.0, 20.0), "drawn": (0.0, 0.0, 20.005, 20.0)}), [])
        out = pair({}, {**full, "paths": (0.0, 0.0, 20.0, 20.0), "drawn": (0.0, 0.0, 20.5, 20.0)})
        self.assertEqual(len(out), 1, out)
        self.assertIn("outside its 20 x 20 slot", out[0])
        out = pair({}, {**full, "paths": (0.0, 0.0, 20.0, 20.0), "drawn": (-0.5, 0.0, 20.0, 20.0)})
        self.assertEqual(len(out), 1, out)

    def test_each_edge_of_the_slot_is_checked_on_its_own(self):
        full = {"main_id": ID["Full"], "paths": (0.0, 0.0, 20.0, 20.0)}
        for label, drawn in (("left", (-0.5, 0.0, 20.0, 20.0)), ("top", (0.0, -0.5, 20.0, 20.0)), ("right", (0.0, 0.0, 20.5, 20.0)), ("bottom", (0.0, 0.0, 20.0, 20.5))):
            out = pair({}, {**full, "drawn": drawn})
            self.assertEqual(len(out), 1, (label, out))
            self.assertIn("outside its 20 x 20 slot", out[0], label)

    def test_each_of_the_four_bounds_is_matched_against_the_icon_on_its_own(self):
        """Check at the glyph box is (2.44, 4.94)-(18.19, 16.25): one edge 1.5 px off, still inside the slot, fails the match."""
        for label, drawn in (("left", (0.5, 4.94, 18.19, 16.25)), ("top", (2.44, 3.0, 18.19, 16.25)), ("right", (2.44, 4.94, 19.9, 16.25)), ("bottom", (2.44, 4.94, 18.19, 17.9))):
            out = pair({}, {"drawn": drawn})
            self.assertEqual(len(out), 1, (label, out))
            self.assertIn("scaled to the glyph box", out[0], label)

    def test_the_four_head_cases_the_review_measured_fail_and_their_fix_trial_passes(self):
        """(control icon, swapped icon, the swapped glyph's paths at 20 px, head fill_geometry): the head build's numbers for the swaps to Move,
        Check and Trash (the control icon is the Plus: the geometry check compares each twin with its own icon, so the control's does not matter)."""
        cases = [("Plus", "Move", (2.42, 2.42, 17.52, 17.5), (3.02, 3.02, 21.9, 21.88)), ("Plus", "Check", CHECK20, CHECK20_HEAD),
                 ("Plus", "Trash", (2.5, 1.25, 17.5, 17.5), (3.12, 1.56, 21.88, 21.88))]
        for control, icon, paths, head in cases:
            args = {"main_id": ID[icon], "paths": paths}
            self.assertEqual(pair({"main_id": ID[control]}, {**args, "drawn": paths}), [], icon)
            self.assertGreaterEqual(len(pair({"main_id": ID[control]}, {**args, "drawn": head})), 1, icon)

    def test_a_control_glyph_that_is_wrong_is_named_the_control(self):
        out = pair({"drawn": (2.5, 2.5, 22.0, 17.5)}, {})
        self.assertTrue(out and all(m.startswith("control glyph") for m in out), out)

    def test_a_curve_whose_handles_leave_the_slot_is_not_flagged(self):
        arc = {"cmds": [{"MoveTo": {"x": 2, "y": 10}}, {"CubicTo": [{"x": 2, "y": -2}, {"x": 18, "y": -2}, {"x": 18, "y": 10}]}], "fill_rule": "NonZero"}
        doc = doc_with_mains(**MAINS)
        add_twin(doc, 10, ID["Plus"], 20.0, 20.0, PLUS20, PLUS20)
        add_twin(doc, 20, ID["Check"], 20.0, 20.0, CHECK20, CHECK20)
        doc["nodes"]["22"]["fill_geometry"] = [arc]
        doc["nodes"]["22"]["kind"]["Vector"]["paths"] = [arc]
        doc["nodes"][str(ID["Check"] + 1)]["fill_geometry"] = [{"cmds": [{"MoveTo": {"x": 1.6, "y": 8}}, {"CubicTo": [{"x": 1.6, "y": -1.6}, {"x": 14.4, "y": -1.6}, {"x": 14.4, "y": 8}]}],
                                                              "fill_rule": "NonZero"}]
        out = swap.compare(swap.snapshot(doc, 10), swap.snapshot(doc, 20))
        self.assertFalse(any("outside its" in m for m in out), out)

    def test_the_kind_paths_are_drawn_only_without_fill_geometry(self):
        """The renderer draws fill_geometry first: a glyph with a doubled fill_geometry is wrong however right its paths are."""
        doc = doc_with_mains(**MAINS)
        add_twin(doc, 20, ID["Check"], 20.0, 20.0, CHECK20, None)  # no fill_geometry: the paths are drawn
        self.assertEqual(swap.snapshot(doc, 20)["drawn"], CHECK20)
        add_twin(doc, 30, ID["Check"], 20.0, 20.0, CHECK20, CHECK20_HEAD)
        self.assertEqual(swap.snapshot(doc, 30)["drawn"], CHECK20_HEAD)

    def test_a_translated_glyph_is_placed_through_its_transform(self):
        """Drawn bounds are in slot space: a glyph at (6, 6) with geometry to 18 reaches 24, past a 20 px slot."""
        moved = {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 6.0, "ty": 6.0}
        out = pair({}, {"transform": moved})
        self.assertTrue(any("outside its 20 x 20 slot" in m for m in out), out)

    def test_the_icons_own_bounds_move_with_the_glyph_so_a_shifted_glyph_is_not_a_wrong_size(self):
        """Both the drawn bounds and the icon's own bounds go through the glyph's transform: a glyph shifted by 1.5 px matches its icon, and only
        the glyph box (1.5 px from the control's) is reported."""
        shifted = {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 1.5, "ty": 0.0}
        out = pair({}, {"transform": shifted})
        self.assertEqual(len(out), 1, out)
        self.assertIn("glyph box differs", out[0])

    def test_nothing_is_reported_without_geometry_or_a_main(self):
        """The synthetic hosts of the colour tests carry no geometry and no icon main: the geometry checks stay silent there."""
        doc = {"nodes": {}}
        add_twin(doc, 10, 999, 20.0, 20.0, PLUS20, PLUS20)
        del doc["nodes"]["12"]["fill_geometry"]
        doc["nodes"]["12"]["kind"] = "Vector"
        self.assertEqual(swap.compare(swap.snapshot(doc, 10), swap.snapshot(doc, 10)), [])


class GlyphBoxTest(unittest.TestCase):
    def test_a_glyph_box_that_moved_more_than_a_pixel_is_reported(self):
        """Engine 0.7.5 resets the slot's glyph to the icon main's 16 px and re-centres it: the box is (2, 2)-(18, 18), not (0, 0)-(20, 20)."""
        recentred = {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 2.0, "ty": 2.0}
        out = pair({}, {"glyph": 16.0, "transform": recentred, "paths": (1.95, 3.95, 14.55, 13.0), "drawn": (1.95, 3.95, 14.55, 13.0)})
        self.assertTrue(any("glyph box" in m and "(2, 2)-(18, 18)" in m and "(0, 0)-(20, 20)" in m for m in out), out)

    def test_a_box_within_a_pixel_of_the_control_passes(self):
        self.assertEqual(pair({}, {"glyph": 20.5, "paths": (2.44 * 1.025, 4.94 * 1.025, 18.19 * 1.025, 16.25 * 1.025),
                                    "drawn": (2.44 * 1.025, 4.94 * 1.025, 18.19 * 1.025, 16.25 * 1.025)}), [])

    def test_a_box_more_than_a_pixel_larger_is_reported(self):
        out = pair({}, {"glyph": 22.0, "paths": (2.44 * 1.1, 4.94 * 1.1, 18.19 * 1.1, 16.25 * 1.1), "drawn": (2.44 * 1.1, 4.94 * 1.1, 18.19 * 1.1, 16.25 * 1.1)})
        self.assertTrue(any("glyph box" in m for m in out), out)


if __name__ == "__main__":
    unittest.main()
