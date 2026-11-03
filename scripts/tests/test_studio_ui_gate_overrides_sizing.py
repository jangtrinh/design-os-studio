"""G2 overrides: an instance's own sizing modes are placement, not style. `set_sizing` (the Properties panel, v0.7.5 panels/properties.rs:1107-1130) writes a Fill child's `layout_child` as Fill and
the node's own `primary_sizing` / `counter_sizing` as Fixed, so a nested copy of a hugging main that fills its row differs from the main in those two keys and in `layout_child`, which the gate already
leaves out. The exemption holds only for a copy that carries the `Layout` flag (`record_override` raises it for exactly these changes): a copy that sizes itself with no flag fails.
Everything else of the frame (gap, padding, direction, alignment, clipping) stays a Layout override and still fails. Synthetic documents (studio_ui_override_fixtures).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_overrides_sizing
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BOX, IBOX, INST, base  # noqa: E402


def own_layout(doc):
    return doc["nodes"][str(INST)]["kind"]["Instance"]["frame"]["auto_layout"]


def problems(doc):
    return check_overrides(doc)[0]


def flag_layout(doc, nid=INST):
    """The `Layout` override flag the editor records on a copy that sizes itself (`record_override`, components.rs)."""
    doc["nodes"][str(nid)]["component"]["overrides"] = ["Layout"]


class OwnSizingNeedsTheLayoutFlag(unittest.TestCase):
    def test_a_copy_that_sizes_itself_without_the_flag_fails(self):
        for key, value in (("primary_sizing", "Fixed"), ("counter_sizing", "Hug")):
            with self.subTest(key=key):
                d = base()
                own_layout(d)[key] = value
                out = problems(d)
                self.assertEqual(len(out), 1, out)
                self.assertIn("Layout override flag", out[0])

    def test_the_editors_fill_shape_without_the_flag_fails(self):
        d = base()
        own_layout(d).update(primary_sizing="Fixed", counter_sizing="Fixed")
        d["nodes"][str(INST)]["layout_child"] = {"absolute": False, "horizontal": "Fill", "vertical": "Fixed", "grow": 0.0}
        self.assertEqual(len(problems(d)), 1)

    def test_another_flag_is_no_layout_flag(self):
        d = base()
        own_layout(d)["primary_sizing"] = "Fixed"
        d["nodes"][str(INST)]["component"]["overrides"] = ["Size"]
        self.assertEqual(len(problems(d)), 1)

    def test_a_flag_with_no_sizing_change_is_no_failure(self):
        d = base()
        flag_layout(d)
        self.assertEqual(problems(d), [])


class OwnSizingIsPlacement(unittest.TestCase):
    def test_the_primary_sizing_of_a_copy_may_differ_from_its_main(self):
        d = base()
        flag_layout(d)
        d["nodes"]["50"]["kind"]["Component"]["auto_layout"]["primary_sizing"] = "Hug"
        own_layout(d)["primary_sizing"] = "Fixed"
        self.assertEqual(problems(d), [])

    def test_the_counter_sizing_of_a_copy_may_differ_from_its_main(self):
        d = base()
        flag_layout(d)
        own_layout(d)["counter_sizing"] = "Hug"
        self.assertEqual(problems(d), [])

    def test_both_together_with_the_editors_fill_shape_pass(self):
        d = base()
        flag_layout(d)
        own_layout(d).update(primary_sizing="Fixed", counter_sizing="Fixed")
        d["nodes"][str(INST)]["layout_child"] = {"absolute": False, "horizontal": "Fill", "vertical": "Fixed", "grow": 0.0}
        self.assertEqual(problems(d), [])

    def test_a_gap_next_to_a_sizing_change_still_fails(self):
        d = base()
        flag_layout(d)
        own_layout(d).update(primary_sizing="Fixed", item_spacing=99.0)
        out = problems(d)
        self.assertEqual(len(out), 1)
        self.assertIn("Layout differs from its main", out[0])

    def test_padding_direction_and_alignment_alone_still_fail(self):
        for key, value in (("padding", [1.0, 2.0, 3.0, 4.0]), ("direction", "Vertical"), ("primary_align", "End"), ("counter_align", "Start"), ("item_spacing", 7.0), ("wrap", True)):
            with self.subTest(key=key):
                d = base()
                own_layout(d)[key] = value
                self.assertTrue(any("Layout differs" in p for p in problems(d)), problems(d))

    def test_a_plain_frame_inside_a_copy_may_not_change_its_sizing(self):
        """Only the copy itself writes its own sizing (`set_sizing`): a frame inside it that mirrors a main frame keeps that frame's sizing, and nothing else checks it."""
        for key in ("primary_sizing", "counter_sizing"):
            with self.subTest(key=key):
                d = base()
                d["nodes"][str(BOX)]["kind"]["Frame"]["auto_layout"][key] = "Hug"
                d["nodes"][str(IBOX)]["kind"]["Frame"]["auto_layout"][key] = "Fixed"
                out = problems(d)
                self.assertEqual(len(out), 1, out)
                self.assertIn("Layout differs from its main", out[0])

    def test_the_same_change_on_the_copy_itself_is_free(self):
        d = base()
        flag_layout(d)
        d["nodes"]["50"]["kind"]["Component"]["auto_layout"]["primary_sizing"] = "Hug"
        d["nodes"][str(INST)]["kind"]["Instance"]["frame"]["auto_layout"]["primary_sizing"] = "Fixed"
        d["nodes"][str(BOX)]["kind"]["Frame"]["auto_layout"]["primary_sizing"] = "Hug"
        d["nodes"][str(IBOX)]["kind"]["Frame"]["auto_layout"]["primary_sizing"] = "Hug"
        self.assertEqual(problems(d), [])

    def test_a_sizing_key_only_the_main_holds_is_no_difference_either(self):
        d = base()
        d["nodes"]["50"]["kind"]["Component"]["auto_layout"]["primary_sizing"] = "Hug"
        self.assertEqual(problems(d), [])


if __name__ == "__main__":
    unittest.main()
