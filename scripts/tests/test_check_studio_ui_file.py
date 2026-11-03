"""Gate `check_studio_ui_file.py`: G1 parity, S1/S2 structure, token half, G4 content and the CLI exit codes.

Run: python3 -B -m unittest scripts.tests.test_check_studio_ui_file
Synthetic documents, a synthetic theme and a stub token map only. Nothing here reads the MCP server, /Applications, plans/,
the committed .designos, the real theme.rs or the committed G01 / non-colour token maps (`CliTest` makes the real map
raise), so editing any of them cannot turn this suite red. Their drift is the advisory steps of design-gates.yml.
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
from studio_ui_gate_fixtures import GREY, MODE, REGISTRY, ROWS, THEME_SRC, make_doc, var, with_node  # noqa: E402


class Fixture(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def parity(self):
        return gate.check_parity(self.doc, ROWS)[0]


class ParityTest(Fixture):
    def test_clean_document_passes(self):
        self.assertEqual(self.parity(), [])

    def test_changed_value_fails(self):
        self.doc["variables"]["p.grey"]["values"][MODE] = {"Color": dict(GREY, r=24 / 255)}
        self.assertTrue(any("!= theme::BG_SECONDARY" in f for f in self.parity()), self.parity())

    def test_missing_variable_fails(self):
        del self.doc["variables"]["s.fact"]
        self.assertIn("theme::SPACE_FACT has no Semantic variable", self.parity())

    def test_literal_semantic_value_fails(self):
        self.doc["variables"]["s.bg"]["values"][MODE] = {"Color": GREY}
        self.assertTrue(any("literal" in f for f in self.parity()))

    def test_alias_of_alias_fails(self):
        self.doc["variables"]["p.grey"]["values"][MODE] = {"Alias": "p.4"}
        self.assertTrue(any("more than one hop" in f for f in self.parity()))

    def test_alias_to_semantic_fails(self):
        self.doc["variables"]["s.bg"]["values"][MODE] = {"Alias": "s.fact"}
        self.assertTrue(any("not a Primitive" in f for f in self.parity()))

    def test_two_variables_for_one_constant_fail(self):
        self.doc["variables"]["s.dup"] = var("s.dup", "color/dup", "cs", {"Alias": "p.grey"}, "theme::BG_SECONDARY")
        self.assertTrue(any("exactly one" in f for f in self.parity()))

    def test_unknown_constant_fails(self):
        self.doc["variables"]["s.bg"]["code_syntax"] = {"WEB": "theme::NOPE"}
        self.assertTrue(any("not a drawable theme.rs constant" in f for f in self.parity()))

    def test_no_semantic_collection_is_an_error(self):
        self.doc["variable_collections"]["cs"]["name"] = "Other"
        with self.assertRaises(gate.GateInputError):
            self.parity()


class StructureTest(Fixture):
    """S1/S2: Primitives hidden with scopes [], one mode named Dark, group/kebab Semantic names."""

    def test_clean_document_passes(self):
        self.assertEqual(gate.check_structure(self.doc), [])

    def test_a_second_semantic_mode_fails(self):
        self.doc["variable_collections"]["cs"]["modes"].append({"id": "light", "name": "Light"})
        self.assertTrue(any("exactly one mode" in f for f in gate.check_structure(self.doc)))

    def test_a_mode_not_named_dark_fails(self):
        self.doc["variable_collections"]["cp"]["modes"][0]["name"] = "Mode 1"
        self.assertTrue(any("named Dark" in f for f in gate.check_structure(self.doc)))

    def test_an_unhidden_primitive_fails(self):
        self.doc["variables"]["p.grey"]["hidden_from_publishing"] = False
        self.assertTrue(any("hidden" in f for f in gate.check_structure(self.doc)))

    def test_a_primitive_with_scopes_fails(self):
        self.doc["variables"]["p.4"]["scopes"] = ["ALL_SCOPES"]
        self.assertTrue(any("scopes" in f for f in gate.check_structure(self.doc)))

    def test_a_third_collection_fails(self):
        self.doc["variable_collections"]["cx"] = {"id": "cx", "name": "Scratch", "modes": [{"id": "dark", "name": "Dark"}],
                                                  "default_mode": "dark", "variables": []}
        self.assertTrue(any("Scratch" in f and "only Primitives and Semantic" in f for f in gate.check_structure(self.doc)))

    def test_a_semantic_name_not_group_kebab_fails(self):
        self.doc["variables"]["s.bg"]["name"] = "Color/BG Secondary"
        self.assertTrue(any("group/kebab" in f for f in gate.check_structure(self.doc)))


STUB_TOKEN_MAP = {"BG_SECONDARY": "colors.surface", "SPACE_FACT": "spacing.space-1", "SPACE_INLINE": "spacing.space-2"}


class CliTest(unittest.TestCase):
    def setUp(self):
        # No test may reach the committed G01 / non-colour maps: remapping a constant there must not turn this suite red.
        guard = mock.patch.object(gate.policy, "token_keys", side_effect=AssertionError("a unit test reached the real token map"))
        guard.start()
        self.addCleanup(guard.stop)

    def run_main(self, text, extra=(), token_map=STUB_TOKEN_MAP, allow=None):
        """`allow`: the allow-list document (default: written for this file, with the sha256 of its bytes)."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "f.designos").write_text(text)
            (tmp / "theme.rs").write_text(THEME_SRC)
            (tmp / "allow.json").write_text(json.dumps(pair.allowlist_document(tmp / "f.designos", []) if allow is None else allow))
            (tmp / "tokens.json").write_text("{}")
            (tmp / "registry.json").write_text(json.dumps(REGISTRY))
            args = [str(tmp / "f.designos"), "--theme", str(tmp / "theme.rs"), "--allowlist", str(tmp / "allow.json"),
                    "--tokens", str(tmp / "tokens.json"), "--registry", str(tmp / "registry.json"), *extra]
            if token_map is not None:
                (tmp / "map.json").write_text(json.dumps(token_map))
                args += ["--token-map", str(tmp / "map.json")]
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                return gate.main(args)

    def test_empty_file_is_an_error(self):
        self.assertEqual(self.run_main(""), 2)

    def test_malformed_and_foreign_json_are_errors(self):
        self.assertEqual(self.run_main("{not json"), 2)
        self.assertEqual(self.run_main("[]"), 2)

    def test_clean_document_exits_zero(self):
        self.assertEqual(self.run_main(json.dumps(make_doc())), 0)

    def test_failing_document_exits_one(self):
        doc = make_doc()
        doc["variables"]["p.grey"]["values"][MODE] = {"Color": dict(GREY, r=0.0)}
        self.assertEqual(self.run_main(json.dumps(doc)), 1)

    def test_a_broken_node_graph_exits_two_not_a_traceback(self):
        doc = make_doc()
        doc["nodes"]["2"]["children"] = [4, 999999]
        self.assertEqual(self.run_main(json.dumps(doc)), 2)

    def test_an_unreadable_token_map_module_exits_two(self):
        with mock.patch.object(gate.policy, "token_keys", side_effect=ImportError("no module")):
            self.assertEqual(self.run_main(json.dumps(make_doc()), token_map=None), 2)

    def test_the_token_map_is_injected_not_read_from_the_repo(self):
        wrong = dict(STUB_TOKEN_MAP, BG_SECONDARY="colors.card")  # the fixture description names colors.surface
        self.assertEqual(self.run_main(json.dumps(make_doc()), token_map=wrong), 1)

    def test_an_unreadable_or_malformed_token_map_file_exits_two(self):
        self.assertEqual(self.run_main(json.dumps(make_doc()), ["--token-map", "/nonexistent/map.json"], token_map=None), 2)
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp) / "bad.json"
            bad.write_text("{not json")
            self.assertEqual(self.run_main(json.dumps(make_doc()), ["--token-map", str(bad)], token_map=None), 2)

    def test_a_bug_in_the_gate_is_not_reported_as_a_malformed_file(self):
        with mock.patch.object(gate, "check_bound", side_effect=KeyError("a bug in the gate")):
            with self.assertRaises(KeyError):
                self.run_main(json.dumps(make_doc()))

    def test_a_wrong_primitive_name_fails_the_gate(self):
        doc = make_doc()
        doc["variables"]["p.grey"]["name"] = "neutral/901"  # #171717 is neutral/900: the CLI must call check_primitive_names
        self.assertEqual(self.run_main(json.dumps(doc)), 1)

    def test_a_component_set_outside_the_registry_exits_one(self):
        doc = make_doc()
        doc["nodes"]["40"]["name"] = "Divider/Line"
        self.assertEqual(self.run_main(json.dumps(doc)), 1)

    def test_a_dimmed_paint_in_a_default_variant_exits_one(self):
        doc = make_doc()
        doc["nodes"]["41"]["fills"] = [{"kind": {"Solid": GREY}, "opacity": 0.5, "visible": True, "blend": "Normal"}]
        doc["nodes"]["41"]["variables"] = {"modes": {}, "bindings": {"fills/0": "s.bg"}}  # bound, so only the opacity rule can fail
        self.assertEqual(self.run_main(json.dumps(doc)), 1)
        doc["nodes"]["41"]["fills"][0]["opacity"] = 1.0
        self.assertEqual(self.run_main(json.dumps(doc)), 0)

    def test_an_unreadable_registry_exits_two(self):
        self.assertEqual(self.run_main(json.dumps(make_doc()), ["--registry", "/nonexistent/registry.json"]), 2)

    def test_deny_option_extends_the_list(self):
        doc = make_doc()
        with_node(doc, name="Quokka card")
        self.assertEqual(self.run_main(json.dumps(doc)), 0)
        self.assertEqual(self.run_main(json.dumps(doc), ["--deny", "quokka"]), 1)


if __name__ == "__main__":
    unittest.main()
