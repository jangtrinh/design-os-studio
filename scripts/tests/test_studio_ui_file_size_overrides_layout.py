"""The editor's `Layout` flag on a top instance inside a main whose `layout_child` differs from its main's.

`record_override` flags `Layout` for `before.layout_child != after.layout_child` (designos-doc/src/components.rs:482-486, `LayoutChild` derives `PartialEq`, `stretched`
included), and a sync of an unflagged Layout copies the main's `layout_child` back (`copy_field`, :1032-1036): a nested Menu/Item that fills its row is reset to the main's
sizing by the next main edit. The REST import records none of it, so the generator does, as it does `Size` and `CornerRadii`. Synthetic native documents.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_size_overrides_layout
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import size_overrides as so  # noqa: E402
from test_studio_ui_file_size_overrides_editor_rule import BTN, BTN_MAIN, HOST, PAGE, RING, doc, flags  # noqa: E402

HUG = {"absolute": False, "horizontal": "Hug", "vertical": "Hug", "grow": 0.0, "min_w": None, "max_w": None, "min_h": None, "max_h": None}
FIXED_HEIGHT = dict(HUG, vertical="Fixed")
FILL = dict(HUG, horizontal="Fill", stretched={"w": [184.0, 312.0]})


def with_layout(instance, main, **kw):
    """The editor-rule document at the main's width (so only `layout_child` can differ), the button instance and its main carrying the given layout_child."""
    d = doc(ring=(16.0, 16.0), btn=(60.0, 32.0), text=(36.75, 20.0), **kw)
    d["nodes"][str(BTN)]["layout_child"] = copy.deepcopy(instance)
    d["nodes"][str(BTN_MAIN)]["layout_child"] = copy.deepcopy(main)
    return d


def ours(d):
    return [m for m in so.missing(d) if m["id"] == BTN]


class LayoutFlag(unittest.TestCase):
    def test_a_fill_nested_copy_of_a_main_without_layout_child_is_reported(self):
        out = ours(with_layout(FILL, None))
        self.assertEqual([m["what"] for m in out], ["instance"])
        self.assertIn("Layout", out[0]["problem"])
        self.assertIn("Action", out[0]["name"])

    def test_hug_hug_against_a_hug_fixed_main_is_reported(self):
        self.assertEqual(len(ours(with_layout(HUG, FIXED_HEIGHT))), 1)

    def test_equal_layout_child_needs_no_flag(self):
        self.assertEqual(ours(with_layout(FIXED_HEIGHT, FIXED_HEIGHT)), [])
        self.assertEqual(ours(with_layout(None, None)), [])

    def test_a_missing_key_and_null_are_the_same_none(self):
        d = with_layout(None, None)
        d["nodes"][str(BTN)].pop("layout_child")
        d["nodes"][str(BTN_MAIN)]["layout_child"] = None
        self.assertEqual(ours(d), [])

    def test_stretched_is_part_of_the_comparison_as_in_the_editor(self):
        self.assertEqual(len(ours(with_layout(dict(HUG, stretched={"w": [1.0, 2.0]}), HUG))), 1)

    def test_a_hug_layout_child_against_a_main_at_the_default_is_a_difference(self):
        """A missing `layout_child` key is the Rust default (Fixed/Fixed, `LayoutChild::default()`, skipped when serialised); Hug/Hug is not that."""
        self.assertEqual(len(ours(with_layout(HUG, None))), 1)

    def test_an_instance_at_the_default_layout_child_against_a_main_with_a_fixed_height_is_reported(self):
        """The instance's key is missing, which is the default (Fixed/Fixed): a main that hugs its width and fixes its height differs from it, so the instance
        needs `Layout`. This is the case a module reaches with `X(.., sx="fixed")` on a nested control whose main is Hug/Fixed: the copy is Fixed/Fixed, the
        serialiser writes no key for it, and without the flag the next main edit resets it to Hug."""
        out = ours(with_layout(None, FIXED_HEIGHT))
        self.assertEqual([m["what"] for m in out], ["instance"])
        self.assertIn("Layout", out[0]["problem"])

    def test_record_adds_layout_keeps_the_other_flags_and_counts_it(self):
        d = with_layout(FILL, None, btn_ov=("Size",))
        counts = so.record(d)
        self.assertEqual(flags(d, BTN), ["Size", "Layout"])
        self.assertEqual((counts["layout"], counts["instances"]), (1, 0))
        self.assertEqual([m for m in so.missing(d) if m["id"] == BTN], [])

    def test_record_twice_adds_it_once(self):
        d = with_layout(FILL, None)
        so.record(d)
        self.assertEqual(so.record(d)["layout"], 0)
        self.assertEqual(flags(d, BTN).count("Layout"), 1)

    def test_a_ring_that_already_carries_layout_is_not_flagged_twice(self):
        d = with_layout(HUG, HUG)
        d["nodes"][str(RING)]["layout_child"] = dict(HUG, absolute=True)
        so.record(d)
        self.assertEqual(flags(d, RING), ["Layout"])

    def test_an_unflagged_ring_with_an_absolute_layout_child_is_reported(self):
        d = with_layout(HUG, HUG, ring_ov=())
        d["nodes"][str(RING)]["layout_child"] = dict(HUG, absolute=True)
        self.assertIn("Layout", [m for m in so.missing(d) if m["id"] == RING][0]["problem"])

    def test_an_instance_in_a_frame_is_a_specimen_not_a_definition(self):
        d = with_layout(FILL, None, host="Frame")
        self.assertEqual(ours(d), [])

    def test_the_page_and_host_are_untouched(self):
        d = with_layout(FILL, None)
        so.record(d)
        self.assertEqual((flags(d, HOST), flags(d, PAGE)), ([], []))


if __name__ == "__main__":
    unittest.main()
