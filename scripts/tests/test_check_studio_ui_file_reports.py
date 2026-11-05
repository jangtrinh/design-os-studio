"""The lines the file gate prints and the exit it hands back for the checks added after the first CLI tests (`test_check_studio_ui_file`):
G2 stray, the allow-listed paint opacity, the G4 deny list, the G5 size report.
Synthetic documents, a synthetic theme and a stub token map only, as there.

Run: python3 -B -m unittest scripts.tests.test_check_studio_ui_file_reports
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
import studio_ui_gate_pair as pair  # noqa: E402
import studio_ui_gate_size as size  # noqa: E402
from studio_ui_gate_fixtures import GREY, REGISTRY, THEME_SRC, make_doc  # noqa: E402

STUB_TOKEN_MAP = {"BG_SECONDARY": "colors.surface", "SPACE_FACT": "spacing.space-1", "SPACE_INLINE": "spacing.space-2"}


def run_gate(doc, allow=()):
    """(exit code, stdout) of the gate on `doc` written to a temporary file with a matching allow-list holding the entries `allow`."""
    with tempfile.TemporaryDirectory() as tmp, mock.patch.object(gate.policy, "token_keys", side_effect=AssertionError("a unit test reached the real token map")):
        tmp = Path(tmp)
        (tmp / "f.designos").write_text(json.dumps(doc))
        for name, text in (("theme.rs", THEME_SRC), ("tokens.json", "{}"), ("registry.json", json.dumps(REGISTRY)), ("map.json", json.dumps(STUB_TOKEN_MAP))):
            (tmp / name).write_text(text)
        (tmp / "allow.json").write_text(json.dumps(pair.allowlist_document(tmp / "f.designos", list(allow))))
        args = [str(tmp / "f.designos"), "--theme", str(tmp / "theme.rs"), "--allowlist", str(tmp / "allow.json"), "--tokens", str(tmp / "tokens.json"),
                "--registry", str(tmp / "registry.json"), "--token-map", str(tmp / "map.json")]
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            return gate.main(args), out.getvalue()


class StrayLineTest(unittest.TestCase):
    def test_a_clean_document_prints_zero_stray_bindings_and_passes(self):
        code, text = run_gate(make_doc())
        self.assertEqual(code, 0)
        self.assertIn("G2 stray  : 5 bindings on pages 01-03, 0 on a property the node does not set", text)

    def test_a_stray_binding_fails_the_gate_and_is_named(self):
        doc = make_doc()
        doc["nodes"]["4"]["variables"]["bindings"]["strokeBottomWeight"] = "s.fact"
        code, text = run_gate(doc)
        self.assertEqual(code, 1)
        self.assertIn("FAIL 01 Foundations/Swatch: strokeBottomWeight is a binding on a property the node does not set", text)
        self.assertIn("G2 stray  : 6 bindings on pages 01-03, 1 on a property", text)


class OpacityEntryTest(unittest.TestCase):
    """The pill's paint opacity through the whole gate: the bound fill, the stored 0.22 and the allow-list entry that waives it."""
    PATH = "02 Core/Divider/Hairline/State=Default"

    def doc(self, opacity=0.22):
        doc = make_doc()
        doc["nodes"]["41"]["fills"] = [{"kind": {"Solid": GREY}, "opacity": opacity, "visible": True, "blend": "Normal"}]
        doc["nodes"]["41"]["variables"] = {"modes": {}, "bindings": {"fills/0": "s.bg"}}
        return doc

    def entry(self, value=0.22):
        return {"path": self.PATH, "property": "fills/0/opacity", "reason": "dev/sections.rs:36 gamma_multiply(0.22)", "value": value}

    def test_a_listed_opacity_passes_and_the_line_counts_it(self):
        code, text = run_gate(self.doc(), [self.entry()])
        self.assertEqual(code, 0, text)
        self.assertIn("(1 with an allow-listed opacity)", text)

    def test_an_unlisted_opacity_fails(self):
        self.assertEqual(run_gate(self.doc())[0], 1)

    def test_a_listed_opacity_that_is_not_the_stored_one_fails(self):
        self.assertEqual(run_gate(self.doc(), [self.entry(0.2)])[0], 1)

    def test_an_entry_with_no_paint_behind_it_fails(self):
        self.assertEqual(run_gate(self.doc(1.0), [self.entry()])[0], 1)
        stale = dict(self.entry(), path="02 Core/Divider/Hairline/State=Nowhere")
        self.assertEqual(run_gate(self.doc(), [self.entry(), stale])[0], 1)


class ContentLineTest(unittest.TestCase):
    def test_the_gate_names_the_fourteen_deny_terms_it_scans_for(self):
        code, text = run_gate(make_doc())
        self.assertEqual(code, 0)
        self.assertRegex(text, r"G4 content: \d+ strings and keys vs 14 deny terms, 0 hits")

    def test_a_kit_only_name_fails_the_gate(self):
        doc = make_doc()
        doc["nodes"]["4"]["name"] = "Dashboards 01"
        code, text = run_gate(doc)
        self.assertEqual(code, 1)
        self.assertIn("`dashboards` in", text)


class SizeReportTest(unittest.TestCase):
    """The page-04 arithmetic is `test_studio_ui_gate_size`'s; a synthetic page 04 would also fail G3 screens, so these drive the gate's own call."""

    def test_the_gate_hands_the_report_the_documents_byte_size_and_prints_its_lines(self):
        doc = make_doc()
        with mock.patch.object(gate, "check_size", wraps=size.check_size) as spy:
            code, text = run_gate(doc)
        self.assertEqual(code, 0)
        self.assertEqual(spy.call_args[0][1], len(json.dumps(doc).encode()))
        self.assertIn(f"G5 size   : the file is {len(json.dumps(doc).encode()):,} bytes", text)
        self.assertIn("no screen on page 04 yet", text)

    def test_the_gate_prints_a_projection_and_a_huge_one_fails_nothing(self):
        huge = {"file": 10**12, "screens": [("Screen/Huge", 9, 10**11)], "base": 9 * 10**11, "mean": 1e11, "projected": 10**13}
        with mock.patch.object(gate, "check_size", return_value=huge):
            code, text = run_gate(make_doc())
        self.assertEqual(code, 0)
        self.assertIn("projection for 74 screens: 900,000,000,000 bytes without screens + 74 x 100,000,000,000 = 10,000,000,000,000 bytes (10000000.0 MB)", text)
        self.assertNotIn("FAIL", text)


if __name__ == "__main__":
    unittest.main()
