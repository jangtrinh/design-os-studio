"""Where a vector glyph is drawn: path bounds (curves sampled, not their control points), the geometry the renderer draws, and its placement.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_glyph_bounds
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import glyph_bounds as gb  # noqa: E402


def poly(x0, y0, x1, y1):
    return {"cmds": [{"MoveTo": {"x": x0, "y": y0}}, {"LineTo": {"x": x1, "y": y0}}, {"LineTo": {"x": x1, "y": y1}}, {"LineTo": {"x": x0, "y": y1}}, "Close"],
            "fill_rule": "NonZero"}


class BoundsTest(unittest.TestCase):
    def test_a_polygon_has_its_vertices_as_bounds(self):
        self.assertEqual(gb.path_bounds([poly(1, 2, 7, 9)]), (1.0, 2.0, 7.0, 9.0))

    def test_a_curve_is_sampled_not_bounded_by_its_control_points(self):
        """An arc from (2, 10) to (18, 10) with handles at y -2: its top is y 1, inside a 20 px slot, though the handles reach outside."""
        arc = {"cmds": [{"MoveTo": {"x": 2, "y": 10}}, {"CubicTo": [{"x": 2, "y": -2}, {"x": 18, "y": -2}, {"x": 18, "y": 10}]}], "fill_rule": "NonZero"}
        x0, y0, x1, y1 = gb.path_bounds([arc])
        self.assertEqual((x0, x1, y1), (2.0, 18.0, 10.0))
        self.assertAlmostEqual(y0, 1.0, places=2)

    def test_a_quadratic_curve_is_sampled_too(self):
        arc = {"cmds": [{"MoveTo": {"x": 0, "y": 10}}, {"QuadTo": [{"x": 5, "y": -10}, {"x": 10, "y": 10}]}], "fill_rule": "NonZero"}
        self.assertAlmostEqual(gb.path_bounds([arc])[1], 0.0, places=2)  # the apex of a quadratic with this handle is y 0

    def test_nothing_drawn_has_no_bounds(self):
        self.assertIsNone(gb.path_bounds([]))
        self.assertIsNone(gb.path_bounds([{"cmds": ["Close"], "fill_rule": "NonZero"}]))

    def test_the_transform_places_the_bounds(self):
        moved = {"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 6.0, "ty": 2.0}
        self.assertEqual(gb.placed((0.0, 0.0, 10.0, 10.0), moved), (6.0, 2.0, 16.0, 12.0))
        scaled = {"a": 2.0, "b": 0.0, "c": 0.0, "d": 0.5, "tx": 0.0, "ty": 0.0}
        self.assertEqual(gb.placed((1.0, 2.0, 3.0, 4.0), scaled), (2.0, 1.0, 6.0, 2.0))
        self.assertEqual(gb.placed((1.0, 2.0, 3.0, 4.0), None), (1.0, 2.0, 3.0, 4.0))

    def test_a_rotation_or_shear_moves_the_corners_not_just_the_origin(self):
        quarter_turn = {"a": 0.0, "b": 1.0, "c": -1.0, "d": 0.0, "tx": 0.0, "ty": 0.0}  # (x, y) -> (-y, x)
        self.assertEqual(gb.placed((0.0, 0.0, 10.0, 5.0), quarter_turn), (-5.0, 0.0, 0.0, 10.0))
        shear = {"a": 1.0, "b": 0.0, "c": 1.0, "d": 1.0, "tx": 0.0, "ty": 0.0}  # x' = x + y
        self.assertEqual(gb.placed((0.0, 0.0, 10.0, 5.0), shear), (0.0, 0.0, 15.0, 5.0))

    def test_the_renderer_draws_fill_geometry_first_and_the_paths_without_it(self):
        node = {"kind": {"Vector": {"paths": [poly(0, 0, 4, 4)]}}, "fill_geometry": [poly(0, 0, 9, 9)]}
        self.assertEqual(gb.geometry_bounds(node), (0.0, 0.0, 9.0, 9.0))
        del node["fill_geometry"]
        self.assertEqual(gb.geometry_bounds(node), (0.0, 0.0, 4.0, 4.0))
        node["fill_geometry"] = []  # an empty list draws nothing, so the paths are drawn (render/geom.rs filters an empty shape out)
        self.assertEqual(gb.geometry_bounds(node), (0.0, 0.0, 4.0, 4.0))
        self.assertIsNone(gb.geometry_bounds({"kind": "Rectangle"}))

    def test_scaling_bounds_to_a_glyph_box(self):
        self.assertEqual(gb.scaled((2.0, 4.0, 10.0, 8.0), 1.25, 0.5), (2.5, 2.0, 12.5, 4.0))


if __name__ == "__main__":
    unittest.main()
