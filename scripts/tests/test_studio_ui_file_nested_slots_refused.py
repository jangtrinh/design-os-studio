"""`X(slots=...)` is refused with the reason, not accepted and silently dropped.

Probed on designos-mcp 0.7.5 (a saved library file, a nested Table/Data copy and a top-level one): `designos_apply_batch` `set_slot` answers ok and creates nothing, with a text or an instance as
content. In the engine (`designos-mcp/src/tools/batch.rs`, the `SetSlot` arm) the fill is built into `temp_doc`, a clone of the working document, and the returned command is dropped, so no write
reaches the document. REST gives an instance no partial content either: an instance that lists children is taken as written, whole, and one without is expanded from its main (rest_import.rs
`expand_instance`). The one route that works is `apply_spec` with `slots` on an instance it creates, which is how a screen fills a Slot; a module cannot use it for a copy nested in a component.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_nested_slots_refused
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402


class SlotsRefusedTest(unittest.TestCase):
    def test_slots_on_a_nested_copy_raise_and_say_why_and_what_works(self):
        with self.assertRaises(ValueError) as raised:
            ir.X("Table", "Table/Data", "", slots={"Rows": [ir.X("Row", "Divider/Hairline", "State=Default")]})
        text = str(raised.exception)
        for part in ("Table", "set_slot", "0.7.5", "apply_spec"):
            self.assertIn(part, text)

    def test_a_nested_copy_without_slots_is_unchanged(self):
        self.assertNotIn("slots", ir.X("Table", "Table/Data", ""))


if __name__ == "__main__":
    unittest.main()
