"""Dialog/Modal against the painters 0.7.5 calls (synthetic env: names are the theme constants; looks from the real kit-states.json).

Surface: `kit::modal_surface` (widgets/kit/surface.rs:64-69, `modal_look` :36-39: `BG`, no border, `RADIUS_FRAME`). Libraries: `libraries/modal.rs:46-61` `frame`, `:68-79` `header`
(`HEADER_H` 48, `PAD` = `INSET_MODAL`, title `font_large_strong`, a `BORDER` hairline on the last header pixel, a `CONTROL_SM` close), `show` :147-170 (560 x 640). Plugins:
`plugins/ui.rs:17-227` `show` (560 x 440, title `font_title` centred 36 down, a kit Secondary Sm "Close" 18 down, a kit Primary Md footer button `PAD` from the bottom and right).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_dialogs
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"CONTROL_MD": 40.0, "CONTROL_SM": 36.0, "BORDER_WIDTH": 1.0, "INSET_MODAL": 24.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out

    def estimate(self, node):
        return ir.estimate(node, mains={})


SETS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(**values):
    return SETS["Dialog/Modal"]["build"](dict({"State": "Default"}, **values), FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class ModalTest(unittest.TestCase):
    def test_the_set_is_the_four_registry_kinds_in_the_dialogs_row(self):
        s = SETS["Dialog/Modal"]
        self.assertEqual((s["row"], s["axes"]), ("Dialogs", {"Kind": ["Libraries", "Plugins", "InstallPlugin", "Compare"], "State": ["Default"]}))
        self.assertEqual(sorted(s["uses"]), ["Badge/StatusCount", "Button/Close", "Button/Primary", "Button/Secondary", "Divider/Hairline", "Focus/Ring", "Input/Field", "Input/Search"])
        self.assertEqual(s["props"], [{"name": "Body", "type": "SLOT"}])

    def test_the_surface_is_bg_with_no_edge_at_the_frame_radius_560_wide(self):
        for kind, h in (("Libraries", 640.0), ("Plugins", 440.0)):
            modal = build(Kind=kind)
            self.assertEqual((modal["fill"], modal["radius"], modal["w"], modal["h"], modal["sx"], modal["sy"]), ("BG", "RADIUS_FRAME", 560.0, h, "fixed", "fixed"), kind)
            self.assertNotIn("stroke", modal, kind)

    def test_libraries_header_is_48_high_with_the_large_title_a_36_px_close_and_the_hairline_on_row_47(self):
        header = named(build(Kind="Libraries"), "Header")[0]
        self.assertEqual((header["h"], header["sx"], ir.pads(header)), (48.0, "fill", [None, "INSET_MODAL", None, "INSET_MODAL"]))
        title = named(header, "Title")[0]
        self.assertEqual((title["style"], title["color"], title["sx"]), ("Large / Semibold", "TEXT", "fill"))
        close, line = named(header, "Close")[0], named(header, "Hairline")[0]
        self.assertEqual((close["set"], close["variant"]), ("Button/Close", "Size=Sm, State=Default"))
        self.assertEqual((line["set"], line["variant"], line["abs"]), ("Divider/Hairline", "State=Default", (0.0, 47.0, 560.0, 1.0)))
        self.assertEqual([k["name"] for k in header["kids"]], ["Title", "Close", "Hairline"])

    def test_plugins_header_is_72_high_with_the_title_font_and_a_secondary_close_button_and_no_hairline(self):
        header = named(build(Kind="Plugins"), "Header")[0]
        self.assertEqual((header["h"], ir.pads(header)), (72.0, [None, "INSET_MODAL", None, "INSET_MODAL"]))
        title = named(header, "Title")[0]
        self.assertEqual((title["style"], title["color"]), ("Title / Semibold", "TEXT"))
        close = named(header, "Close")[0]
        self.assertEqual((close["set"], close["variant"], close["props"]), ("Button/Secondary", "Size=Sm, State=Default", {"Label": "Close"}))
        self.assertEqual(named(header, "Hairline"), [])

    def test_plugins_footer_is_76_high_with_the_primary_md_button_modal_inset_from_the_corner(self):
        footer = named(build(Kind="Plugins"), "Footer")[0]
        self.assertEqual((footer["h"], ir.pads(footer), footer["align"]), (76.0, [None, "INSET_MODAL", "INSET_MODAL", None], ("end", "end")))
        button = named(footer, "Primary action")[0]
        self.assertEqual((button["set"], button["variant"], button["props"]), ("Button/Primary", "Size=Md, State=Default", {"Label": "Install plugin"}))
        self.assertEqual(named(build(Kind="Libraries"), "Footer"), [], "the Libraries modal has no footer; its buttons are the caller's body")

    def test_the_body_is_a_slot_that_fills_the_rest_and_holds_the_default_content_of_its_kind(self):
        for kind in ("Libraries", "Plugins", "InstallPlugin"):
            body = named(build(Kind=kind), "Body")[0]
            self.assertEqual((body["slot"], body["sx"], body["sy"], body["dir"]), ("Body", "fill", "fill", "V"), kind)
            self.assertEqual(ir.pads(body), [None, "INSET_MODAL", None, "INSET_MODAL"], f"{kind}: a caller's content in the slot sits inside the modal's side inset")
        self.assertEqual([k["name"] for k in named(build(Kind="Plugins"), "Body")[0]["kids"]], ["Icon", "Title", "Keep note", "Undo note"])
        self.assertEqual(len(named(build(Kind="Libraries"), "This file")), 1)

    def test_no_scrim_is_in_the_set(self):
        for kind in ("Libraries", "Plugins", "InstallPlugin", "Compare"):
            self.assertEqual([k for k in ir.walk(build(Kind=kind)) if k["name"] == "Scrim"], [], "the full-screen scrim is a screen layer (W4)")

    def test_every_text_uses_a_defined_text_style(self):
        for kind in ("Libraries", "Plugins", "InstallPlugin", "Compare"):
            for t in (k for k in ir.walk(build(Kind=kind)) if k["t"] == "T"):
                self.assertIn(t["style"], STYLES, kind)


if __name__ == "__main__":
    unittest.main()
