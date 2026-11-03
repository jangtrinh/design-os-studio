"""Registry `region` Panel/SpecReader (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag, `specs.rs` `show_reader` (:2720-2900).

Panel/SpecReader has no registry variants, so it is one plain component, 909 x 868: the reader replaces the canvas (`show_reader(.., canvas_rect)`, shell.rs:1816; the default editor's canvas is
909 x 868, d-editor-default.layout.json `regions.canvas`). It is switched off in 0.7.5 (`let specs_reader_open = false;`, shell.rs:1813: the registry's `today=hidden`), so no capture shows it and
there is no crop row: UNVERIFIED-VISUAL, drawn from the Rust (the tie-breaker). Default state: the dark `Slate` palette (`light_mode: false`, :634), the `PRD & Overview` tab. The reader paints its
own palette (`ReaderPalette::for_mode`, :62-92), raw colour literals that are not theme constants, so each maps to the nearest theme tone (W2c ruling; each a deviation): `bg_page` 0x141518 -> BG
(d 11), `surface` 0x191A1E and `doc_sheet` 0x17181C -> BG_SECONDARY (d 7), `border` 0x282A32 -> BORDER (d 12), `text` 0xEDEEF0 -> TEXT, `text_muted` 0xB8BCC4 -> TEXT_QUATERNARY (d 21),
`text_subtle` 0x7E838E -> TEXT_PLACEHOLDER (d 15); the accent 0x0D99FF is INFO (d 0) and `Color32::WHITE` is ICON. The `Paper` palette (`light_mode`) is a gap. Drawn: the 42 px bar
(`BG_SECONDARY`, a `BORDER` hairline on its last row, :2731-2734), the breadcrumb at x 16 (`font_body_medium`, :2832-2837), the four 24 px buttons from the right (`Canvas` 56, `Slate` 50, `Copy
.md` 62, `Share` 48, 6 apart, 1 px border, radius a raw 5, `font_caption_medium`, :2737-2789), the five tabs with the active one's 2 px `INFO` underline inset 6 (:2791-2830), the document Slot
(below the bar, 52 px above the bottom: `content_rect`, :2840-2841) and the floating agent pill (460 x 32 at (20, bottom - 42), a pill, the `INFO` sparkle, the hint, the `INFO` send disc with a
white arrow, :2851-2867). The tab widths are estimates (the Rust measures `text_width(label, caption medium) + 16`, which the engine measures differently and no capture shows): 106, 93, 80, 80 and
48, 2 apart, right-aligned at `share.left() - 10`. The arrow is drawn as filled strokes (the IR has no stroked path).
Nested: none. The registry's `composes=Section/Header` is spec intent: the section titles of the document tabs are the document's own text (`render_po_view_tab` and four more), which the Slot
holds. Gaps: the five tab bodies (the Slot's default is neutral content), hover fills, the `Paper` palette, the toast and the left and right spec panels (`show_left`, `show_right`, other regions).
"""
from rest_ir import E, F, I, T, V, lit, segments_path
from components.regions_home_parts import REGION_ROW, region, rule

W, H, BAR = 909, 868, 42
RAW = "specs.rs:2740 `CornerRadius::same(5)`: a raw 5 in the Rust, no constant named"
EDGE = "specs.rs:2741 `Stroke::new(1.0, pal.border)`: a raw 1.0 in the Rust, no constant named"
PILL = "specs.rs:2854 `CornerRadius::same(99)`: a raw 99 that clamps to half the pill's height"
SEND_ARROW = "specs.rs:2864-2866 `Stroke::new(1.6, Color32::WHITE)`: a raw width, no stroke-weight constant in theme.rs"
UNDERLINE = "specs.rs:2823 `CornerRadius::same(1)`: a raw 1 in the Rust, no constant named"
TABS = [("PRD & Overview", 106), ("User Stories", 93), ("QA & Trace", 80), ("Blueprints", 80), ("Gates", 48)]  # widths: estimates, see the module docstring
BUTTONS = [("Canvas", 843, 56, "TEXT"), ("Slate", 787, 50, "TEXT_QUATERNARY"), ("Copy .md", 719, 62, "TEXT_QUATERNARY"), ("Share", 665, 48, "TEXT_QUATERNARY")]


def text(env, name, content, style, color, x, cy, w):
    return T(name, content, style, env.c(color), abs=(x, cy - 9, w, 18))


def button(env, name, x, w, color):
    """:2738-2742: a 24 px frame on `doc_sheet` with a 1 px `border` stroke and a centred `font_caption_medium` label."""
    return F(name, [T("Label", name, "Caption / Medium", env.c(color))], dir="H", fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=lit(1, EDGE), radius=lit(5, RAW),
             abs=(x, 9, w, 24), w=w, h=24, sx="fixed", sy="fixed", align=("center", "center"))


def bar(env):
    tabs, x = [], 240
    for i, (label, w) in enumerate(TABS):
        active = i == 0
        style, color = ("Caption / Semibold", "TEXT") if active else ("Caption / Medium", "TEXT_PLACEHOLDER")
        tabs.append(F(f"Tab {i + 1}", [T("Label", label, style, env.c(color))], dir="H", abs=(x, 7, w, 28), w=w, h=28,
                      sx="fixed", sy="fixed", align=("center", "center")))
        x += w + 2
    underline = F("Tab underline", fill=env.c("INFO"), radius=lit(1, UNDERLINE), abs=(246, BAR - 2, TABS[0][1] - 12, 2), w=TABS[0][1] - 12, h=2, sx="fixed", sy="fixed")
    crumb = text(env, "Breadcrumb", "< design:os studio  /  PRD: Spec title", "Body / Medium", "TEXT_QUATERNARY", 16, BAR / 2, 200)
    kids = [crumb, *tabs, underline, *[button(env, n, x0, w, c) for n, x0, w, c in BUTTONS], rule("Rule", 0, BAR - 1, W)]
    return F("Bar", kids, dir="H", fill=env.c("BG_SECONDARY"), abs=(0, 0, W, BAR), w=W, h=BAR, sx="fixed", sy="fixed")


def agent_pill(env):
    """:2851-2867: the sparkle at left + 16 (13 px), the hint at left + 30, the 20 px send disc at right - 16 with a 1.6 px white arrow (a vertical line and two 3.5 px legs)."""
    arrow = V("Send arrow", segments_path([((10, 15), (10, 6)), ((6.5, 9), (10, 5.5)), ((13.5, 9), (10, 5.5))]), 20, 20, None, stroke=env.c("ICON"), sw=lit(1.6, SEND_ARROW), abs=(434, 6, 20, 20))
    kids = [I("Sparkle", "Sparkle", 13, env.c("INFO"), abs=(9.5, 9.5, 13, 13)), text(env, "Hint", "Ask @agent about this Spec", "Body / Regular", "TEXT_QUATERNARY", 30, 16, 160),
            E("Send disc", 20, 20, fill=env.c("INFO"), abs=(434, 6, 20, 20)), arrow]
    return F("Agent pill", kids, dir="H", fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=lit(1, EDGE), radius=lit(16, PILL), abs=(20, H - 42, 460, 32), w=460, h=32, sx="fixed", sy="fixed")


def reader(values, env):
    content = T("Neutral content", "Content", "Body / Medium", env.c("TEXT_QUATERNARY"))
    body = F("Document", [content], dir="V", slot="Document", abs=(0, BAR, W, H - 52 - BAR), w=W, h=H - 52 - BAR, sx="fixed", sy="fixed",
             pad=env.c("INSET_PANEL"), align=("start", "start"))
    root = region("Reader", [bar(env), body, agent_pill(env)], env, W, H)
    root["clip"] = True
    return root


def component_sets():
    return [{"name": "Panel/SpecReader", "row": REGION_ROW, "axes": {}, "build": reader, "uses": ["Divider/Hairline"], "props": [{"name": "Document", "type": "SLOT"}]}]
