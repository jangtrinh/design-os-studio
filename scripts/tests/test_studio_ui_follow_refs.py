"""`follows` in crops.json says which Rust a set's look comes from. The compare prints it next to every UNVERIFIED-VISUAL variant and adds the
kit-states.json row of the set only when the set follows a `widgets/kit/` painter: a set drawn by a legacy painter (Radio/Card, whose live rows are
`agent_approval_card.rs::paint_permissions_modal`) is not the kit row's look, so naming the row would point at what the set does not draw.

The module is pure text work (no Pillow), so the suite reaches it.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_follow_refs
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import follow_refs as fr  # noqa: E402

CROPS = json.loads((HERE.parent / "studio_ui_file" / "evidence" / "crops.json").read_text(encoding="utf-8"))
LEGACY, KIT = "agent_approval_card.rs::paint_permissions_modal", "widgets/kit/choice.rs::radio_card"
KIT_ROWS = {"Radio/Card": {}, "Checkbox/Box": {}}


class FollowRefsTest(unittest.TestCase):
    def test_a_set_that_follows_only_a_legacy_painter_gets_no_kit_states_row(self):
        refs = fr.follow_refs("Radio/Card", [LEGACY], KIT_ROWS)
        self.assertEqual(len(refs), 1)
        self.assertTrue(refs[0].startswith("agent_approval_card.rs:") and refs[0].endswith("paint_permissions_modal"), refs)

    def test_a_set_that_follows_a_kit_painter_gets_the_kit_states_row(self):
        refs = fr.follow_refs("Checkbox/Box", ["widgets/kit/choice.rs::checkbox"], KIT_ROWS)
        self.assertEqual(refs[-1], "widgets/kit/testdata/kit-states.json row Checkbox/Box")
        self.assertTrue(refs[0].startswith("widgets/kit/choice.rs:") and refs[0].endswith("checkbox"), refs)

    def test_a_set_that_follows_both_gets_the_row_once_after_its_refs(self):
        refs = fr.follow_refs("Radio/Card", [LEGACY, KIT], KIT_ROWS)
        self.assertEqual(len(refs), 3)
        self.assertEqual(refs[-1], "widgets/kit/testdata/kit-states.json row Radio/Card")

    def test_a_kit_painter_without_a_row_for_the_set_adds_nothing(self):
        self.assertEqual(len(fr.follow_refs("Tabs/Pill", ["widgets/kit/choice.rs::checkbox"], KIT_ROWS)), 1)

    def test_a_missing_function_is_fatal(self):
        with self.assertRaises(SystemExit):
            fr.follow_refs("Radio/Card", ["agent_approval_card.rs::no_such_painter_exists"], KIT_ROWS)


class PinnedRefTest(unittest.TestCase):
    """A painter the app removed is still what the 0.7.5 captures show. Its ref is pinned: `v0.7.5 <path>:<line> <fn> (removed by #<pr>)` prints as written, and only while
    the function is gone from the tree. A plain ref whose function is gone stays fatal."""
    PINNED = "v0.7.5 old_chip.rs:160 paint_old_chip (removed by #1438)"

    def tree(self, source):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        (Path(tmp.name) / "old_chip.rs").write_text(source, encoding="utf-8")
        patch = mock.patch.object(fr, "SRC", Path(tmp.name))
        patch.start()
        self.addCleanup(patch.stop)

    def test_a_pinned_ref_prints_its_pinned_text_without_a_lookup_while_the_function_is_gone(self):
        self.tree("pub fn paint_new_chip() {}\n")
        self.assertEqual(fr.rust_refs([self.PINNED]), ["old_chip.rs:160 paint_old_chip (v0.7.5, removed by #1438)"])

    def test_a_pinned_ref_to_a_function_that_is_still_in_the_tree_is_fatal(self):
        self.tree("pub fn paint_old_chip() {}\n")
        with self.assertRaises(SystemExit) as raised:
            fr.rust_refs([self.PINNED])
        self.assertIn("still in old_chip.rs", str(raised.exception))

    def test_a_plain_ref_to_a_function_that_is_gone_stays_fatal(self):
        self.tree("pub fn paint_new_chip() {}\n")
        with self.assertRaises(SystemExit):
            fr.rust_refs(["old_chip.rs::paint_old_chip"])

    def test_a_pinned_ref_without_the_line_or_the_removing_pr_is_fatal(self):
        self.tree("pub fn paint_new_chip() {}\n")
        for bad in ("v0.7.5 old_chip.rs paint_old_chip (removed by #1438)", "v0.7.5 old_chip.rs:160 paint_old_chip", "v0.7.5 old_chip.rs:160 paint_old_chip (removed)"):
            with self.assertRaises(SystemExit, msg=bad):
                fr.rust_refs([bad])

    def test_a_pinned_ref_and_a_plain_one_resolve_in_one_list(self):
        self.tree("pub fn paint_new_chip() {}\n")
        self.assertEqual(fr.rust_refs([self.PINNED, "old_chip.rs::paint_new_chip"]), ["old_chip.rs:160 paint_old_chip (v0.7.5, removed by #1438)", "old_chip.rs:1 paint_new_chip"])


class CropsFollowsTest(unittest.TestCase):
    def test_tag_chip_follows_the_selection_chip_painter_pinned_to_v0_7_5_because_the_agents_panel_pr_removed_it(self):
        """The set draws the 0.7.5 selection pill and is compared with 0.7.5 captures; `paint_selection_chip` was removed by #1438 (spec I). The redraw from the composer chips is scheduled apart."""
        self.assertEqual(CROPS["follows"]["Tag/Chip"], ["v0.7.5 agent_selection_chip.rs:160 paint_selection_chip (removed by #1438)"])
        self.assertEqual(fr.follow_refs("Tag/Chip", CROPS["follows"]["Tag/Chip"], {}), ["agent_selection_chip.rs:160 paint_selection_chip (v0.7.5, removed by #1438)"])

    def test_every_follows_ref_resolves_in_the_tree_the_compare_runs_on(self):
        unresolved = []
        for name, refs in CROPS["follows"].items():
            try:
                fr.rust_refs(refs)
            except SystemExit as e:
                unresolved.append(f"{name}: {e}")
        self.assertEqual(unresolved, [])

    def test_radio_card_follows_only_the_painter_production_calls(self):
        """`radio_card` is live only after the spec I PR merges (v0.7.6); until then the rows are `paint_permissions_modal`."""
        self.assertEqual(CROPS["follows"]["Radio/Card"], [LEGACY])


if __name__ == "__main__":
    unittest.main()
