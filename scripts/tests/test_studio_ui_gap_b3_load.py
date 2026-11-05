"""The load guard of components/popover_kinds.py: every Kind of `ORDER` has a builder and every builder a Kind, checked when the module loads. The module is run again from its own source
with one name changed, so a deleted guard lets the changed copy load.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b3_load
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
from components import popover_kinds  # noqa: E402

SOURCE = Path(popover_kinds.__file__).read_text(encoding="utf-8")
LAST = '"Permissions")'
SKILLS_IN_ORDER = ', "Skills",'


def load(source):
    """Run the module source in a fresh namespace; its imports resolve as the real module's do."""
    namespace = {"__name__": "popover_kinds_copy"}
    exec(compile(source, "popover_kinds_copy.py", "exec"), namespace)  # noqa: S102 (a test of the module's own load-time check)
    return namespace


class LoadGuardTest(unittest.TestCase):
    def test_the_unchanged_source_loads_with_every_kind_built(self):
        self.assertEqual(list(load(SOURCE)["KINDS"]), list(popover_kinds.ORDER))

    def test_a_kind_in_order_without_a_builder_refuses_to_load(self):
        self.assertIn(LAST, SOURCE)
        with self.assertRaises(ValueError) as error:
            load(SOURCE.replace(LAST, '"Permissions", "NoSuchKind")', 1))
        self.assertIn("NoSuchKind", str(error.exception))

    def test_a_builder_whose_kind_is_missing_from_order_refuses_to_load(self):
        self.assertIn(SKILLS_IN_ORDER, SOURCE)
        with self.assertRaises(ValueError) as error:
            load(SOURCE.replace(SKILLS_IN_ORDER, ",", 1))
        self.assertIn("Skills", str(error.exception))

    def test_building_a_kind_that_is_not_registered_is_refused(self):
        with self.assertRaises(KeyError):
            popover_kinds.build("NoSuchKind", None)


if __name__ == "__main__":
    unittest.main()
