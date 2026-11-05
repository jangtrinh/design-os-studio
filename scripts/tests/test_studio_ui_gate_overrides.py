"""G2 overrides gate: an instance differs from its main only in what the editor lets an instance own.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_overrides
Synthetic documents only (studio_ui_override_fixtures). Every non-style field, every forbidden style field and the CLI wiring has its own case; the
exceptions are in test_studio_ui_gate_overrides_exceptions.
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import test_check_studio_ui_file as cli_tests  # noqa: E402  (a module, not its test class: unittest would collect the class twice)
from studio_ui_gate_doc import GateInputError  # noqa: E402
from studio_ui_gate_fixtures import GREY, make_doc, solid  # noqa: E402
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BLUE, HOST, IBOX, INST, ITEXT, PLACE, base, set_text  # noqa: E402


def problems(doc):
    return check_overrides(doc)[0]


class NonStyleFieldsAreFree(unittest.TestCase):
    def test_identical_copy_passes(self):
        self.assertEqual(problems(base()), [])

    def test_name_visible_locked_size_transform_and_text_pass(self):
        d = base()
        d["nodes"][str(INST)].update(name="Other", visible=False, locked=True, size={"x": 99.0, "y": 40.0},
                                     transform={"a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 5.0, "ty": 6.0})
        d["nodes"][str(ITEXT)]["kind"]["Text"]["content"] = "A much longer label"
        d["nodes"][str(ITEXT)]["size"] = {"x": 120.0, "y": 20.0}
        self.assertEqual(problems(d), [])

    def test_placement_layout_child_and_constraints_pass(self):
        d = base()
        d["nodes"][str(IBOX)]["layout_child"] = dict(PLACE, absolute=True, horizontal="Fill")
        d["nodes"][str(IBOX)]["constraints"] = {"horizontal": "Stretch", "vertical": "Stretch"}
        self.assertEqual(problems(d), [])

    def test_instance_swap_passes(self):
        d = base()
        d["nodes"][str(INST)]["kind"]["Instance"]["component"] = HOST
        self.assertEqual(problems(d), [])

    def test_counts_instances_and_nodes(self):
        self.assertEqual(check_overrides(base())[1], {"instances": 2, "nodes": 4})  # the fixture's own instance, then the three of base()

    def test_no_instance_is_an_input_error(self):
        d = base()
        for nid in (INST, ITEXT, IBOX, 43):
            d["nodes"][str(nid)]["component"] = {}
        with self.assertRaises(GateInputError):
            check_overrides(d)

    def test_missing_source_fails(self):
        d = base()
        d["nodes"][str(IBOX)]["component"] = {"source": 999}
        self.assertTrue(any("does not exist" in f for f in problems(d)))


class CliWiring(unittest.TestCase):
    def test_the_cli_runs_the_override_gate(self):
        doc = make_doc()
        self.assertEqual(cli_tests.CliTest.run_main(None, json.dumps(doc)), 0)
        doc["nodes"]["43"]["opacity"] = 0.5  # the fixture's own instance, dimmed against its main
        self.assertEqual(cli_tests.CliTest.run_main(None, json.dumps(doc)), 1)


class StyleFieldsFail(unittest.TestCase):
    def fails(self, doc, field):
        got = problems(doc)
        self.assertTrue(any(f"{field} differs" in f for f in got), got)

    def test_opacity(self):
        d = base()
        d["nodes"][str(INST)]["opacity"] = 0.5
        self.fails(d, "Opacity")

    def test_blend(self):
        d = base()
        d["nodes"][str(IBOX)]["blend"] = "Multiply"
        self.fails(d, "Blend")

    def test_fills_value(self):
        d = base()
        d["nodes"][str(IBOX)]["fills"] = [solid(BLUE)]
        self.fails(d, "Fills")

    def test_stroke(self):
        d = base()
        d["nodes"][str(IBOX)]["stroke"] = {"paints": [solid()], "weight": 2.0}
        self.fails(d, "Stroke")

    def test_effects(self):
        d = base()
        d["nodes"][str(IBOX)]["effects"] = [{"DropShadow": {"color": GREY, "visible": True}}]
        self.fails(d, "Effects")

    def test_corner_radii(self):
        d = base()
        d["nodes"][str(IBOX)]["corner_radii"] = [6.0] * 4
        self.fails(d, "CornerRadii")

    def test_text_size(self):
        d = base()
        set_text(d, ITEXT, font_size=20.0)
        self.fails(d, "TextStyle")

    def test_text_fill(self):
        d = base()
        set_text(d, ITEXT, fills=[solid(BLUE)])
        self.fails(d, "TextStyle")

    def test_text_style_ref(self):
        d = base()
        d["nodes"][str(ITEXT)]["style_refs"] = {"text": "Text lg / Bold"}
        self.fails(d, "TextStyle")

    def test_layout_gap(self):
        d = base()
        d["nodes"][str(IBOX)]["kind"]["Frame"]["auto_layout"]["item_spacing"] = 12.0
        self.fails(d, "Layout")

    def test_layout_padding(self):
        d = base()
        d["nodes"][str(INST)]["kind"]["Instance"]["frame"]["auto_layout"]["padding"] = [0.0, 8.0, 0.0, 8.0]
        self.fails(d, "Layout")

    def test_layout_direction(self):
        d = base()
        d["nodes"][str(INST)]["kind"]["Instance"]["frame"]["auto_layout"]["direction"] = "Vertical"
        self.fails(d, "Layout")

    def test_binding_to_another_variable(self):
        d = base()
        d["nodes"][str(IBOX)]["variables"]["bindings"]["fills/0"] = "s.accent"
        got = problems(d)
        self.assertTrue(any("fills/0 is bound to color/bg-brand" in f for f in got), got)

    def test_binding_the_main_does_not_carry(self):
        d = base()
        d["nodes"][str(IBOX)]["variables"]["bindings"]["itemSpacing"] = "s.inline"
        self.assertTrue(any("itemSpacing is bound to space/inline" in f for f in problems(d)))

    def test_binding_the_main_carries_but_the_instance_dropped(self):
        d = base()
        del d["nodes"][str(IBOX)]["variables"]["bindings"]["topLeftRadius"]
        self.assertTrue(any("topLeftRadius" in f for f in problems(d)))


if __name__ == "__main__":
    unittest.main()
