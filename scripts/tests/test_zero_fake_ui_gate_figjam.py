"""Z18 of the zero-fake-UI gate: no new FigJam word in shipped UI strings or asset names."""

import importlib.util
import sys
import unittest
from pathlib import Path

GATE = Path(__file__).resolve().parents[1] / "zero_fake_ui_gate.py"
sys.path.insert(0, str(GATE.parent))
_spec = importlib.util.spec_from_file_location("zero_fake_ui_gate_figjam_under_test", GATE)
gate = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = gate
_spec.loader.exec_module(gate)

ANCHOR = ("panels/mod.rs", "pub mod toolbar;")


class FigjamGateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.base = gate.SourceSet.from_disk()

    def verdict(self, sources, assets=None):
        return gate.check_z18(sources, assets)[0]

    def test_the_tree_passes(self):
        self.assertTrue(self.verdict(self.base, []))

    def test_a_planted_figjam_label_fails(self):
        planted = self.base.with_replacement(*ANCHOR, 'pub mod toolbar;\npub const L: &str = "FigJam";')
        self.assertFalse(self.verdict(planted, []))

    def test_a_planted_lowercase_figjam_literal_fails(self):
        planted = self.base.with_replacement(*ANCHOR, 'pub mod toolbar;\npub const L: &str = "figjam";')
        self.assertFalse(self.verdict(planted, []))

    def test_a_comment_mention_is_not_a_ui_string(self):
        self.assertTrue(self.verdict(self.base.with_replacement(*ANCHOR, "pub mod toolbar; // FigJam"), []))

    def test_a_figjam_named_asset_fails(self):
        self.assertFalse(self.verdict(self.base, ["assets/figjam-sticky.svg"]))

    def test_a_cleaned_legacy_site_must_shrink_the_baseline(self):
        legacy = dict(gate.FIGJAM_LEGACY_ALLOWED)
        gate.FIGJAM_LEGACY_ALLOWED["tabs.rs"] = legacy["tabs.rs"] + 1
        try:
            self.assertFalse(self.verdict(self.base, []))
        finally:
            gate.FIGJAM_LEGACY_ALLOWED.clear()
            gate.FIGJAM_LEGACY_ALLOWED.update(legacy)


if __name__ == "__main__":
    unittest.main()
