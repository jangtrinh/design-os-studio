"""Registry `region` Bar/Presentation (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag, `present.rs` `show` (:405-500, the top bar; `PRESENT_BAR_HEIGHT` 48).

Bar/Presentation has no registry variants, so it is one plain component: the 1440 x 48 bar of the Present view at the top-left corner of the window (the layout.json named rects of `x-present` and
`present-variables` are its own coordinates). It has no fill: the page behind it is `page_bg` (:439, :661-678), the document's page colour or `PRESENT_BG` when that is the editor default, so the
captures' light page (245) belongs to the screen and the document, not to the bar. The bar paints, in order: the flow name (:452-460), `Restart` (:461-469), the `Variables` toggle (:471-480), the
`Fullscreen` button and the close button (:481-496), the current frame's name centred (:497-498) and a `BORDER` hairline on the last row (:499).
Nested (live): Divider/Hairline (the `hline`, resized to the window width). The registry's `composes=Button/*,Tooltip/Default` is spec intent: none of the five controls is a Button set
(`button_bg` = `kit::paint_icon_bg`, the Icon look: no fill at rest, a 24 px square or wider with a text label, where Button/Icon is a 32 / 36 px square with a glyph and the text buttons start at
32 px), and the tooltips (`on_hover_text`) are hover states. Local: the flow pill (`BG_SECONDARY` at `RADIUS_CONTROL`, the name 8 in, a raw `dd.left() + 8.0`: the file has no flows so the plain
branch is drawn, :454-456; with flows `dropdown` draws a Select look instead), the three ghost buttons (labels in `font_body`, `TEXT`, and `TEXT_SECONDARY` for the toggle while off), the close
button (an own painter, `icon_close`: two 1.5 px strokes of 10 px in `ICON`, :346-351; the Phosphor `Close` glyph stands in, deviation recorded) and the title.
Light-on-light: `TEXT` and `ICON` are near-white, drawn on the document's light page, so `Restart`, `Fullscreen` and the cross are barely visible in the captures; that is production (Non-goals:
the gap is reported, not fixed). Frame names are neutral (`Frame 1`); the compare sets the capture's text on a scratch copy.
The Variables toggle's on state (`button_bg(.., st.show_vars)`: the `BG_SELECTED` pill and `TEXT`, :475-476, capture `present-variables`) is the `Show Variables active` part, drawn over the
off look. Gaps: hover fills, the flow dropdown with flows (:457),
fullscreen (the bar is not drawn then, :443), the deck footer (`Previous` / `Next` / count, :503-520), and the live variables panel (`paint_vars_panel`, :804-869, 264 wide): a popover the registry
does not list in this record's `composes`, a screen part (W4).
"""
from rest_ir import F, I, T, lit
from components.regions_home_parts import REGION_ROW, region, rule

W, H = 1440, 48
FLOW_LEFT = "present.rs:456 `Pos2::new(dd.left() + 8.0, dd.center().y)`: a raw 8 in the Rust, no constant named"
FRAME = "Frame 1"  # the current frame's name: the document's data, neutral here


def ghost(name, text, color, x, w, env):
    """A `button_bg` ghost: no fill at rest, `RADIUS_CONTROL`, the label centred in `font_body`."""
    return F(name, [T("Label", text, "Body / Regular", env.c(color))], dir="H", radius=env.c("RADIUS_CONTROL"), abs=(x, 12, w, 24), w=w, h=24, sx="fixed", sy="fixed", align=("center", "center"))


def bar(values, env):
    name = T("Label", FRAME, "Body / Regular", env.c("TEXT"), prop="Flow name")
    flow = F("Flow", [name], dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), pad=[None, None, None, lit(8, FLOW_LEFT)],
             abs=(16, 12, 200, 24), w=200, h=24, sx="fixed", sy="fixed", align=("start", "center"))
    close = F("Close", [I("Cross", "Close", 16, env.c("ICON"))], dir="H", radius=env.c("RADIUS_CONTROL"), abs=(W - 40, 8, 32, 32), w=32, h=32, sx="fixed", sy="fixed", align=("center", "center"))
    title = F("Title", [T("Label", FRAME, "Body / Regular", env.c("TEXT_SECONDARY"), prop="Title")], dir="H", abs=(0, 0, W, H), w=W, h=H, sx="fixed", sy="fixed", align=("center", "center"))
    active = F("Variables active", [T("Label", "Variables", "Body / Regular", env.c("TEXT"))], dir="H", fill=env.c("BG_SELECTED"), radius=env.c("RADIUS_CONTROL"), abs=(308, 12, 76, 24), w=76, h=24,
               sx="fixed", sy="fixed", align=("center", "center"), visible_prop="Show Variables active", hidden=True)
    kids = [flow, ghost("Restart", "Restart", "TEXT", 224, 76, env), ghost("Variables", "Variables", "TEXT_SECONDARY", 308, 76, env), active, title,
            ghost("Fullscreen", "Fullscreen", "TEXT", W - 144, 96, env), close, rule("Rule", 0, H - 1, W)]
    return region("Bar", kids, env, W, env.num("PRESENT_BAR_HEIGHT"), fill=None)


def component_sets():
    return [{"name": "Bar/Presentation", "row": REGION_ROW, "axes": {}, "build": bar, "uses": ["Divider/Hairline"],
             "props": [{"name": "Flow name", "type": "TEXT", "default": FRAME}, {"name": "Title", "type": "TEXT", "default": FRAME},
                                                                                            {"name": "Show Variables active", "type": "BOOLEAN", "default": False}]}]
