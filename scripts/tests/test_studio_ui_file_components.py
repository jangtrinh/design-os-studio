"""Component framework of scripts/studio_ui_file: IR -> REST lowering with bind intents, spec discovery and checks, the native
post-pass. Synthetic plans only (no theme.rs, registry, MCP or committed file).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_components
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG = HERE.parent / "studio_ui_file"
sys.path.insert(0, str(PKG))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import assemble_foundation  # noqa: E402
import native_post_pass as post  # noqa: E402
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402


def sem(name, value, const=None):
    kind = "Color" if isinstance(value, dict) else "Number"
    return {"name": name, "id": "s." + name.replace("/", "."), "value": value, "kind": kind, "const": const or name,
            "code_syntax": f"theme::{const or name}"}


PLAN = {"semantic": [sem("color/bg", dict(r=10, g=10, b=10, a=255)), sem("color/line", dict(r=64, g=64, b=64, a=255)), sem("space/fact", 4.0),
                     sem("size/border-width", 1.0), sem("radius/control", 8.0), sem("inset/button", 12.0)]}
STYLES = [{"name": "Body / Medium", "text_style": {"font_family": "Inter", "font_weight": 500, "font_size": 12.0, "line_height": {"Pixels": 18.0}}}]


class FakeIcons:
    def entry(self, name):
        return {"id": "icon:0", "key": "icon-" + name, "glyph": "icon:0:v"}

    def path(self, name, size):
        return f"M0 0 L{size} {size}"


def lower(node, **kw):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    low.prop_keys = {"Label": "Label#s:0", "Icon": "Icon#s:1", "Has Icon": "Has Icon#s:2"}
    return low, low.lower(node, 0, 0, **kw)


def intents(low, sid=None):
    return {(i["key"], i["var"]) for i in low.intents if sid is None or i["sid"] == sid}


class LoweringTest(unittest.TestCase):
    def test_a_frame_binds_fill_stroke_weight_radius_gap_and_padding(self):
        node = ir.F("Box", [ir.T("L", "Hi", "Body / Medium", "color/line")], dir="H", gap="space/fact", pad=[None, "inset/button", None, "inset/button"],
                    fill="color/bg", stroke="color/line", sw="size/border-width", radius="radius/control")
        low, rest = lower(node)
        want = {("fills/0", "color/bg"), ("strokes/0", "color/line"), ("strokeWeight", "size/border-width"), ("itemSpacing", "space/fact"),
                ("paddingLeft", "inset/button"), ("paddingRight", "inset/button"), ("topLeftRadius", "radius/control"),
                ("topRightRadius", "radius/control"), ("bottomRightRadius", "radius/control"), ("bottomLeftRadius", "radius/control")}
        self.assertEqual(intents(low, rest["id"]), want)
        self.assertEqual((rest["itemSpacing"], rest["paddingLeft"], rest["paddingTop"], rest["cornerRadius"], rest["strokeWeight"]), (4.0, 12.0, 0, 8.0, 1.0))

    def test_the_stored_colour_is_the_variable_value(self):
        _, rest = lower(ir.F("Box", fill="color/line"))
        c = rest["fills"][0]["color"]
        self.assertEqual([round(c[k] * 255) for k in "rgba"], [64, 64, 64, 255])

    def test_text_gets_its_style_its_colour_binding_and_its_property_reference(self):
        low, rest = lower(ir.T("Label", "Save", "Body / Medium", "color/line", prop="Label", alpha=0.5))
        self.assertEqual(intents(low), {("fills/0", "color/line"), ("text_style", "Body / Medium")})
        self.assertEqual(rest["componentPropertyReferences"], {"characters": "Label#s:0"})
        self.assertEqual((rest["style"]["fontSize"], rest["style"]["lineHeightPx"], rest["fills"][0]["opacity"]), (12.0, 18.0, 0.5))

    def test_an_icon_instance_binds_the_glyph_child_not_the_instance(self):
        low, rest = lower(ir.I("Icon", "Search", 20, "color/line", swap="Icon", visible_prop="Has Icon"))
        glyph = rest["children"][0]
        self.assertEqual(glyph["id"], f"I{rest['id']};icon:0:v")
        self.assertEqual([i["sid"] for i in low.intents], [glyph["id"]])
        self.assertEqual(rest["overrides"], [{"id": glyph["id"], "overriddenFields": ["fills"]}])
        self.assertEqual(rest["componentPropertyReferences"], {"mainComponent": "Icon#s:1", "visible": "Has Icon#s:2"})

    def test_a_literal_is_an_allow_list_entry_with_its_reason_not_an_intent(self):
        low, rest = lower(ir.F("Ring", stroke="color/line", sw=ir.lit(2, "a 2 px band"), radius=ir.lit(12, "control + 4")))
        self.assertEqual(rest["strokeWeight"], 2.0)
        self.assertNotIn("strokeWeight", {i["key"] for i in low.intents})
        self.assertEqual({a["property"] for a in low.allow}, {"strokeWeight", "topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"})
        self.assertTrue(all(a["reason"] for a in low.allow))
        self.assertEqual({a["property"]: a["value"] for a in low.allow}, {"strokeWeight": 2.0, **{k: 12.0 for k in ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius")}})

    def test_an_unknown_variable_or_property_fails_loudly(self):
        with self.assertRaises(KeyError):
            lower(ir.F("Box", fill="color/nope"))
        low = Lowerer(PLAN, STYLES, FakeIcons())
        with self.assertRaises(KeyError):
            low.lower(ir.T("L", "x", "Body / Medium", "color/bg", prop="Label"), 0, 0)

    def test_a_nested_instance_mirrors_the_mains_bindings(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        low.component("Ring", "Tone=Brand", ir.F("Ring", stroke="color/line", sw="size/border-width", w=40, h=40), 0, 0)
        before = len(low.intents)
        rest = low.lower(ir.F("Host", [ir.X("Focus ring", "Ring", "Tone=Brand", abs=(-4, -4, 48, 48))], dir="H"), 0, 0)
        ring = rest["children"][0]
        self.assertEqual((ring["type"], ring["layoutPositioning"], ring["absoluteBoundingBox"]["width"]), ("INSTANCE", "ABSOLUTE", 48))
        self.assertIn(("strokes/0", "color/line"), intents(low, ring["id"]))
        self.assertGreater(len(low.intents), before)

    def test_a_bad_sizing_mode_is_refused(self):
        with self.assertRaises(ValueError):
            ir.F("Box", sx="stretch")


class SpecTest(unittest.TestCase):
    REGISTRY = {"components": [{"name": "A/B", "variants": ["Size=Xs", "Size=Sm", "State=Default"]}]}

    def spec(self, **kw):
        return {"name": "A/B", "row": "Actions", "axes": {"Size": ["Xs", "Sm"], "State": ["Default"]}, "build": lambda v, e: None, **kw}

    def test_a_matching_spec_passes(self):
        self.assertEqual(len(components.check_specs([self.spec()], self.REGISTRY)), 1)

    def test_an_unknown_name_wrong_values_unknown_row_and_the_cap_fail(self):
        for bad in (self.spec(name="A/C"), self.spec(axes={"Size": ["Xs"], "State": ["Default"]}), self.spec(row="Misc")):
            with self.assertRaises(ValueError):
                components.check_specs([bad], self.REGISTRY)
        big = {"components": [{"name": "A/B", "variants": [f"Axis=V{i}" for i in range(31)]}]}
        with self.assertRaisesRegex(ValueError, "split proposal"):
            components.check_specs([self.spec(axes={"Axis": [f"V{i}" for i in range(31)]})], big)

    def test_the_matrix_is_full_and_ordered(self):
        m = components.matrix({"Size": ["Xs", "Sm"], "State": ["Default", "Hover"]})
        self.assertEqual([components.variant_name(v) for v in m], ["Size=Xs, State=Default", "Size=Xs, State=Hover", "Size=Sm, State=Default", "Size=Sm, State=Hover"])

    def test_every_module_is_discovered_with_unique_names_rows_and_dependencies_first(self):
        specs = components.discover()
        names = [s["name"] for s in specs]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(all(s["row"] in components.ROWS + components.STUDIO_ROWS for s in specs))
        for i, s in enumerate(specs):
            self.assertTrue(all(u in names[:i] for u in s.get("uses", [])), f"{s['name']} comes before a set it uses")


class OutputPathTest(unittest.TestCase):
    def test_a_foundation_only_file_never_defaults_to_the_committed_path(self):
        with self.assertRaises(SystemExit):
            assemble_foundation.parse_args("t", [], default_out=False)
        self.assertTrue(assemble_foundation.parse_args("t", ["--out", "/tmp/x.designos"], default_out=False).out.endswith("x.designos"))

    def test_the_library_command_keeps_the_committed_default(self):
        self.assertTrue(assemble_foundation.parse_args("t", []).out.endswith("design/studio-ui.designos"))


class CropsTest(unittest.TestCase):
    """evidence/crops.json (which production widget each variant is compared with) must name real sets, variants and labels."""

    def test_every_entry_names_a_real_variant_and_carries_what_the_compare_needs(self):
        specs = {s["name"]: s for s in components.discover()}
        entries = json.loads((PKG / "evidence" / "crops.json").read_text())["entries"]
        self.assertGreater(len(entries), 0)
        for e in entries:
            self.assertIn(e["set"], specs, e)
            self.assertIn(e["variant"], [components.variant_name(v) for v in components.matrix(specs[e["set"]]["axes"])], e)
            self.assertTrue(("named" in e) != ("rect" in e) or ("named" in e and "rect" in e), e)
            self.assertTrue(e.get("case"), e)
            node = e.get("label_node", "Label")
            wants_text = any(p["name"] == node and p["type"] == "TEXT" for p in specs[e["set"]].get("props", [])) \
                and not e["set"].startswith(("Button/Icon", "Button/Close", "Button/Tool")) and not e.get("render_rect")
            self.assertEqual(bool(e.get("label")), wants_text, f"{e['set']}: a text widget needs the production text so the hug width is comparable")
            if "rect" in e:
                self.assertEqual(len(e["rect"]), 4, e)
                self.assertTrue(e.get("note"), f"{e['set']}: a hand rect records how it was derived")
            self.assertLessEqual(set(e) - {"set", "variant", "case", "named", "rect", "label", "label_node", "icon", "mask_glyph", "render_rect", "align", "bg",
                                           "colour_only", "fg_pending", "fg_inset", "finding", "finding_pin", "render_node", "render_trim", "captures", "gradient", "kit", "note", "texts", "render_pad", "fills", "labels"}, set(), e)


class PostPassTest(unittest.TestCase):
    def doc(self):
        return {"nodes": {"7": {"id": 7, "source_id": "c1", "name": "Box"}}, "variables": {"s.color.bg": {}, "s.space.fact": {}},
                "style_meta": {"Body / Medium": {}}}

    def test_intents_bind_by_source_id_and_set_the_text_style(self):
        d = self.doc()
        out = post.apply_intents(d, PLAN, [{"sid": "c1", "key": "fills/0", "var": "color/bg"}, {"sid": "c1", "key": "text_style", "var": "Body / Medium"}])
        self.assertEqual(d["nodes"]["7"]["variables"]["bindings"], {"fills/0": "s.color.bg"})
        self.assertEqual((d["nodes"]["7"]["style_refs"], out), ({"text": "Body / Medium"}, {"bindings": 1, "text_styles": 1}))

    def test_a_missing_node_variable_or_style_and_a_conflict_are_fatal(self):
        for it in ({"sid": "zz", "key": "fills/0", "var": "color/bg"}, {"sid": "c1", "key": "fills/0", "var": "color/nope"},
                   {"sid": "c1", "key": "text_style", "var": "No / Style"}):
            with self.assertRaises(SystemExit):
                post.apply_intents(self.doc(), PLAN, [it])
        with self.assertRaises(SystemExit):
            post.apply_intents(self.doc(), PLAN, [{"sid": "c1", "key": "fills/0", "var": "color/bg"}, {"sid": "c1", "key": "fills/0", "var": "space/fact"}])


if __name__ == "__main__":
    unittest.main()
