"""The search hint stays TEXT_PLACEHOLDER while 0.7.5 draws egui's weak colour: a recorded D5 deviation (issue #1389), written in the module docstring and on page
`00 Start Here`. The Xs value cites the painter that draws it, not the multiline comment box.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_forms_hint
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))


class SearchHintDeviationTest(unittest.TestCase):
    """The search hint stays TEXT_PLACEHOLDER while 0.7.5 draws egui's weak colour: a recorded D5 deviation on page `00 Start Here`."""

    def test_start_here_records_the_deviation_with_its_cause_and_the_issue(self):
        import start_here
        words = " ".join(n["text"] for n in start_here.start_here({"semantic": [], "primitives": []}, 117, 18)["children"])
        self.assertIn("Search hint colour (D5 deviation, issue #1389)", words)
        for fact in ("#1389", "Input/Search", "TEXT_PLACEHOLDER", "weak_text_color", "TEXT at alpha 0.6", "G2 opacity rule", "Input/Field Xs", "TEXT_TERTIARY"):
            self.assertIn(fact, words, fact)

    def test_the_docstring_gives_the_cause_and_not_a_missing_variable(self):
        from components import forms_fields
        doc = " ".join((forms_fields.__doc__ or "").split())
        for fact in ("#1389", "weak_text_color", "TEXT at alpha 0.6", "G2 opacity rule"):
            self.assertIn(fact, doc, fact)
        self.assertNotIn("No Semantic variable holds the 154", doc)

    def test_the_xs_value_cites_the_xs_painter_and_not_the_multiline_box(self):
        """`comments/ui.rs:64` paints the multiline `text_box` (Margin 10 x 10, top-aligned), so it is no source for the Xs value at FIELD_PLAIN_X 8."""
        from components import forms_fields
        xs_source = " ".join((forms_fields.__doc__ or "").split()).split("; Sm is")[0]
        self.assertIn("widgets/field.rs:711-716", xs_source)
        self.assertNotIn("comments/ui.rs", xs_source)


if __name__ == "__main__":
    unittest.main()
