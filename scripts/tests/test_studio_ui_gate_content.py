"""G4 (content): the whole saved file, every string value and every key, is matched against a static deny list.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_content
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import make_doc, with_node  # noqa: E402


# Strings that only the Untitled kit uses (its page, frame and style names), not generic words: `Desktop` is Studio's frame preset and Tree/Row's default name, `Base` is a ramp name.
KIT_TERMS = ("_primitives", "color modes", "display 2xl", "dashboards", "swatch base", "logomark", "press logo", "company logo", "screen mockups", "\u2756")
OWN_NAMES = ("Desktop", "Base", "Neutral", "Brand", "Getting started", "Text styles", "Slate", "Frame 1", "Heading 1", "Design file", "Page 1")


class ContentTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def content(self, deny=gate.DENY):
        return gate.check_content(self.doc, list(deny))[0]

    def test_clean_document_passes(self):
        self.assertEqual(self.content(), [])

    def test_deny_listed_node_name_fails_case_insensitively(self):
        with_node(self.doc, name="UnTiTlEd Button")
        self.assertEqual(len(self.content()), 1)

    def test_deny_listed_text_and_description_fail(self):
        with_node(self.doc, kind={"Text": {"content": "see vsf", "style": {}}})
        self.doc["variables"]["s.bg"]["description"] = "from PCP"
        self.assertEqual(len(self.content()), 2)

    def test_every_string_in_the_file_is_scanned(self):
        """Seeded leaks under keys a names-and-text list would miss."""
        leaks = {
            "component key": lambda d: with_node(d, fig_metadata={"componentKey": "untitled-ui-button"}),
            "source id": lambda d: with_node(d, source_id="pcp:12"),
            "other platform syntax": lambda d: d["variables"]["s.bg"]["code_syntax"].update({"ANDROID": "R.color.olivia"}),
            "variant default": lambda d: with_node(d, component={"props": [{"name": "T", "default": {"Variant": "Untitled"}}]}),
            "variant options": lambda d: with_node(d, component={"props": [{"name": "T", "kind": {"Variant": {"options": ["VSF"]}}}]}),
            "comment text": lambda d: d.update(comments=[{"messages": [{"text": "ask Olivia"}]}]),
            "a key": lambda d: with_node(d, vsf_extra=1),
        }
        for label, seed in leaks.items():
            with self.subTest(label):
                doc = make_doc()
                seed(doc)
                self.assertEqual(len(gate.check_content(doc, list(gate.DENY))[0]), 1, label)

    def test_extra_deny_terms_extend_the_static_list(self):
        with_node(self.doc, name="Quokka card")
        self.assertEqual(self.content(), [])
        self.assertEqual(len(self.content(list(gate.DENY) + ["quokka"])), 1)

    def test_the_static_list_is_the_four_names_and_the_kit_only_terms(self):
        self.assertEqual(tuple(gate.DENY), ("untitled", "vsf", "pcp", "olivia") + KIT_TERMS)

    def test_each_kit_only_term_is_one_hit_in_any_case_and_in_a_key_too(self):
        for term in KIT_TERMS:
            with self.subTest(term):
                doc = make_doc()
                with_node(doc, name=f"Frame {term.upper()} 2")
                self.assertEqual(len(gate.check_content(doc, list(gate.DENY))[0]), 1)
                doc = make_doc()
                with_node(doc, **{f"x {term}": 1})
                self.assertEqual(len(gate.check_content(doc, list(gate.DENY))[0]), 1)

    def test_studios_own_generic_names_are_not_hits(self):
        for name in OWN_NAMES:
            with self.subTest(name):
                with_node(self.doc, name=name)
                self.assertEqual(self.content(), [])

    def test_the_gate_does_not_read_the_machine_library(self):
        self.assertFalse(hasattr(gate, "library_names"))


if __name__ == "__main__":
    unittest.main()
