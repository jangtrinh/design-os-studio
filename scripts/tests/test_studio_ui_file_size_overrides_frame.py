"""The editor's `Layout` flag also covers the frame props: `record_override` compares `before.kind.frame_props() != after.kind.frame_props()` next to `layout_child` (designos-doc/src/components.rs:481-486),
and a main sync copies the main's frame back into an instance that does not carry the flag (`copy_field` Layout, :1032-1036). A nested copy that sets its own sizing (Fill is own Fixed, so a Fill copy of a hugging
main is not re-hugged) differs from its main in `frame_props` and may differ in nothing else; without the flag the next main edit would give it the main's Hug again. Synthetic native documents.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_size_overrides_frame
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import size_overrides as so  # noqa: E402
from test_studio_ui_file_size_overrides_editor_rule import BTN, BTN_MAIN, doc, flags  # noqa: E402

HUG_AL = {"direction": "Horizontal", "item_spacing": 4.0, "padding": [0.0, 12.0, 0.0, 12.0], "primary_sizing": "Hug", "counter_sizing": "Fixed"}
FIXED_AL = dict(HUG_AL, primary_sizing="Fixed")
PLACE = {"absolute": False, "horizontal": "Fill", "vertical": "Fixed", "grow": 0.0}


def with_frames(instance_al, main_al, instance_extra=None, layout_child=PLACE):
    """The editor-rule document at the main's size (so only the frame props and layout_child can differ), the button instance and its main holding the given auto layouts."""
    d = doc(ring=(16.0, 16.0), btn=(60.0, 32.0), text=(36.75, 20.0))
    inst, main = d["nodes"][str(BTN)], d["nodes"][str(BTN_MAIN)]
    inst["kind"] = {"Instance": {"component": BTN_MAIN, **({"frame": {"auto_layout": copy.deepcopy(instance_al)}} if instance_al is not None else {}), **(instance_extra or {})}}
    main["kind"] = {"Component": ({"auto_layout": copy.deepcopy(main_al)} if main_al is not None else {})}
    inst["layout_child"] = main["layout_child"] = copy.deepcopy(layout_child)
    return d


def ours(d):
    return [m for m in so.missing(d) if m["id"] == BTN]


class FramePropsTest(unittest.TestCase):
    def test_a_copy_whose_own_sizing_differs_is_reported_as_layout_though_layout_child_is_equal(self):
        out = ours(with_frames(FIXED_AL, HUG_AL))
        self.assertEqual([m["what"] for m in out], ["instance"])
        self.assertIn("Layout", out[0]["problem"])
        self.assertIn("frame", out[0]["problem"])

    def test_record_flags_it_once_and_keeps_the_other_flags(self):
        d = with_frames(FIXED_AL, HUG_AL)
        counts = so.record(d)
        self.assertEqual(flags(d, BTN), ["Layout"])
        self.assertEqual(counts["layout"], 1)
        self.assertEqual(ours(d), [])
        self.assertEqual(so.record(d)["layout"], 0)
        self.assertEqual(flags(d, BTN).count("Layout"), 1)

    def test_equal_frames_need_no_flag(self):
        self.assertEqual(ours(with_frames(HUG_AL, HUG_AL)), [])
        self.assertEqual(ours(with_frames(None, None)), [])

    def test_an_instance_with_no_frame_against_a_main_with_one_differs(self):
        self.assertEqual(len(ours(with_frames(None, HUG_AL))), 1)

    def test_the_component_link_inside_the_instances_frame_is_not_a_frame_prop(self):
        d = with_frames(HUG_AL, HUG_AL, instance_extra={"component_key": "k"})
        d["nodes"][str(BTN)]["kind"]["Instance"]["frame"]["component"] = BTN_MAIN
        self.assertEqual(ours(d), [])

    def test_every_other_frame_field_counts_too(self):
        for key, value in (("item_spacing", 8.0), ("padding", [0.0, 8.0, 0.0, 8.0]), ("direction", "Vertical"), ("counter_sizing", "Hug")):
            with self.subTest(key=key):
                self.assertEqual(len(ours(with_frames(dict(HUG_AL, **{key: value}), HUG_AL))), 1)

    def test_a_text_inside_the_copy_has_no_frame_and_is_never_a_layout_difference(self):
        d = with_frames(HUG_AL, HUG_AL)
        self.assertEqual([m for m in so.missing(d) if m["name"] == "Label"], [])

    def test_a_difference_in_both_layout_child_and_the_frame_is_one_flag(self):
        d = with_frames(FIXED_AL, HUG_AL, layout_child=PLACE)
        d["nodes"][str(BTN)]["layout_child"] = dict(PLACE, horizontal="Hug")
        so.record(d)
        self.assertEqual(flags(d, BTN).count("Layout"), 1)


if __name__ == "__main__":
    unittest.main()
