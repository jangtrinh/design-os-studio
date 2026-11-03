"""Dialog/Modal Kind=Libraries, Plugins, InstallPlugin and Compare (components/dialogs*.py), read from the IR the builder returns against the real Semantic plan and the real registry.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b4_dialogs
"""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
import components  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

SPEC = fx.SPECS["Dialog/Modal"]
REGISTRY = json.loads((fx.REPO / "design/component-registry.json").read_text(encoding="utf-8"))
BOUND = ("fill", "stroke", "gap", "radius", "pad", "sw")


def modal(kind):
    return SPEC["build"]({"Kind": kind, "State": "Default"}, fx.ENV)


def unbound(node):
    """Every property the Lowerer binds that holds a raw number instead of a Semantic name or a `lit(value, reason)`."""
    out = []
    for k in walk(node):
        for key in BOUND:
            v = k.get(key)
            for e in (v if isinstance(v, list) else [v]):
                if e is not None and not isinstance(e, (str, tuple)):
                    out.append((k["name"], key, e))
    return out


class AxesTest(unittest.TestCase):
    def test_the_axes_are_the_registrys_with_libraries_and_plugins_first(self):
        components.check_specs([SPEC], REGISTRY)
        self.assertEqual(SPEC["axes"]["Kind"], ["Libraries", "Plugins", "InstallPlugin", "Compare"])

    def test_a_set_that_drops_compare_is_refused_by_the_registry_check(self):
        mutant = dict(SPEC, axes={"Kind": ["Libraries", "Plugins", "InstallPlugin"], "State": ["Default"]})
        with self.assertRaises(ValueError):
            components.check_specs([mutant], REGISTRY)


class BodyTest(unittest.TestCase):
    def test_plugins_default_body_is_the_painters_tools_glyph_and_three_left_aligned_paragraphs(self):
        """`plugins/ui.rs:111-116`: a 28 px `Icon::Tools` centred in a 60 px row, then three `font_body` paragraphs 12 apart, flush with the modal's `PAD`."""
        m = modal("Plugins")
        self.assertEqual([k["set"] for k in walk(m) if k["t"] == "X"], ["Button/Secondary", "Button/Primary"], "no EmptyState: the painter does not draw the Home empty state")
        (glyph,) = [k for k in walk(m) if k["t"] == "I"]
        self.assertEqual((glyph["icon"], glyph["size"], glyph["color"]), ("Tools", 28, fx.ENV.c("ICON_SECONDARY")))
        body = fx.part(m, "Body")
        self.assertEqual(body["kids"][0]["abs"], (266.0, 16.0, 28.0, 28.0))
        paragraphs = [(k["text"], k["color"], k["abs"][:2]) for k in body["kids"][1:]]
        self.assertEqual(paragraphs, [("Your plugin library starts here", fx.ENV.c("TEXT"), (24.0, 71.0)),
                                      ("Install a plugin from jangtrinh/design-os-plugins to keep its package on this device.", fx.ENV.c("TEXT_TERTIARY"), (24.0, 98.0)),
                                      ("Browser plugins can insert SVG icons into the open document. Each insertion supports Undo.", fx.ENV.c("TEXT_TERTIARY"), (24.0, 125.0))])

    def test_libraries_default_body_is_the_publish_list(self):
        m = modal("Libraries")
        names = [k["name"] for k in walk(m)]
        for want in ("This file", "Status", "File name", "Summary", "Change 1", "Change 4", "More", "Description", "Publish", "Libraries", "Search", "Empty title"):
            self.assertIn(want, names)
        self.assertEqual([(k["set"], k["variant"]) for k in walk(m) if k["name"] in ("Description", "Publish", "Search")],
                         [("Input/Field", "Size=Sm, State=Default"), ("Button/Primary", "Size=Sm, State=Default"), ("Input/Search", "Size=Sm, State=Default")])
        chips = [k for k in walk(m) if k["t"] == "X" and k["set"] == "Badge/StatusCount"]
        self.assertEqual({(c["variant"], c["props"]["Label"]) for c in chips}, {("Size=Md, Tone=Success, State=Default", "New")})


class InstallPluginTest(unittest.TestCase):
    def setUp(self):
        self.m = modal("InstallPlugin")

    def test_the_header_is_the_plugins_one_with_the_install_title(self):
        header = fx.part(self.m, "Header")
        self.assertEqual((header["h"], fx.texts(header)), (72.0, ["Install plugin"]))

    def test_the_url_field_is_a_focused_sm_field_in_a_host_with_the_keyboard_ring(self):
        host = fx.part(self.m, "URL field")
        self.assertEqual((host["w"], host["h"], host["radius"]), (512.0, 36.0, "radius/control"))
        field, ring = fx.part(host, "Field"), fx.part(host, "Focus ring")
        self.assertEqual((field["set"], field["variant"], field["sx"], field["sy"]), ("Input/Field", "Size=Sm, State=Focus", "fill", "fill"))
        self.assertEqual((ring["set"], ring["variant"], ring["abs"]), ("Focus/Ring", "Tone=Brand", (-4.0, -4.0, 520.0, 44.0)))
        self.assertTrue(field["props"]["Placeholder"].startswith("https://github.com/jangtrinh/design-os-plugins/blob/main/plugins/ico"))

    def test_the_notes_and_label_are_the_rusts_words_at_the_captures_rows(self):
        body = fx.part(self.m, "Body")
        self.assertEqual(fx.texts(body)[:2], ["Paste a manifest URL from jangtrinh/design-os-plugins.", "Plugin URL"])
        self.assertEqual(fx.part(body, "Token note")["abs"][1], 190.0 - 72.0)
        self.assertIn("DESIGNOS_PLUGINS_TOKEN", fx.part(body, "Token note")["text"])
        self.assertEqual(fx.part(body, "Download note")["abs"][1], 232.0 - 72.0)

    def test_the_footer_is_cancel_and_a_disabled_install_a_row_apart(self):
        footer = fx.part(self.m, "Footer")
        cancel, install = fx.part(footer, "Cancel"), fx.part(footer, "Primary action")
        self.assertEqual((cancel["set"], cancel["variant"], cancel["w"], cancel["props"]), ("Button/Secondary", "Size=Md, State=Default", 72.0, {"Label": "Cancel"}))
        self.assertEqual((install["set"], install["variant"], install["w"], install["props"]), ("Button/Primary", "Size=Md, State=Disabled", 96.0, {"Label": "Install"}))
        self.assertEqual((footer["gap"], pads(footer)), ("space/inline", [None, "inset/modal", "inset/modal", None]))

    def test_the_form_holds_no_unbound_literal(self):
        self.assertEqual(unbound(self.m), [])


class CompareTest(unittest.TestCase):
    def setUp(self):
        self.m = modal("Compare")

    def test_the_card_is_the_menu_surface_818_by_638(self):
        m = self.m
        self.assertEqual((m["w"], m["h"], m["fill"], m["stroke"], m["radius"]), (818.0, 638.0, "color/bg", "color/border-menu", "radius/frame"))

    def test_header_panes_summary_list_and_footer_are_at_the_rusts_offsets(self):
        m = self.m
        self.assertEqual(fx.part(m, "Close button")["abs"], (776.0, 14.0, 28.0, 28.0))
        self.assertEqual([fx.part(m, f"{n} area")["abs"] for n in ("Before", "After")], [(20.0, 80.0, 382.0, 300.0), (418.0, 80.0, 382.0, 300.0)])
        self.assertEqual(fx.part(m, "Change list")["abs"], (20.0, 416.0, 780.0, 160.0))
        self.assertEqual((fx.part(m, "Close")["abs"], fx.part(m, "Mark as ready")["abs"]), ((522.0, 588.0, 80.0, 32.0), (610.0, 588.0, 190.0, 32.0)))
        self.assertEqual(fx.part(m, "Mark as ready")["fill"], "color/bg-brand")
        self.assertEqual(fx.part(m, "Title")["abs"][:2], (20.0, 18.0))

    def test_the_list_is_clipped_and_carries_a_scroll_thumb(self):
        self.assertTrue(fx.part(self.m, "Change list")["clip"])
        self.assertEqual(fx.part(self.m, "Scrollbar")["w"], 4.0)

    def test_no_kit_layer_name_is_drawn(self):
        text = " ".join(fx.texts(self.m)).lower()
        for word in fx.KIT_WORDS:
            self.assertNotIn(word, text)

    def test_every_text_uses_a_defined_style_and_nothing_is_unbound(self):
        for t in (k for k in walk(self.m) if k["t"] == "T"):
            self.assertIn(t["style"], fx.STYLES)
        self.assertEqual(unbound(self.m), [])


if __name__ == "__main__":
    unittest.main()
