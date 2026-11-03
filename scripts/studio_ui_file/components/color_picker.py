"""Popover/ColorPicker of page `03 Studio` (row `Menus & Popovers`): the colour picker, a set of three `Kind`s (the registry's `Kind=Fill, Page, Gradient`). Its parts are
in `color_picker_parts.py`.

Popover/ColorPicker
* Painter: `show` (widgets/picker.rs:737-1217). Callers: 1 in production, `panels/properties.rs:2033` (plus a test, `widgets/kit/painter_tip_tests.rs:113`; `git grep "picker::show("` at
v0.7.5); the capture is `f-picker`
  (`picker` (960, 346, 240, 538), `picker-tab:Custom` (968, 354, 61, 24), `picker-tab:Libraries` (1033, 354, 66.75, 24), `picker-add-style` (1140, 354, 24, 24), `picker-blend` (1140, 394, 24, 24),
  `picker-contrast-bar` (976, 432, 208, 20)). The popover is 240 wide (`W`); 538 high is `panel_height` for a solid paint with the Libraries tab and two rows of swatches.
* Kinds drawn: `Fill` (the solid paint, `f-picker`, today's look and the first variant), `Page` (`picker-page`: 240 x 498, the `!env.paint_types` path, :763-778 and :808-843: a
  plain `Custom` title at `PAD`, no Libraries tab, no add-style button and no paint-type row, so everything under the header is 40 higher; the colour is the file's page colour
  #F5F5F5, contrast 15.29 : 1) and `Gradient` (`f-picker-gradient`: 240 x 552, the Linear button active and the stops row of `color_picker_stops.py` in the place of the contrast
  row: `STOPS_H` 40 against `CONTRAST_BAR_H` 26, so everything under it is 14 lower, no AA dots and no hairline under the row). The image mode, the Libraries tab (`fn libraries`,
  :1219) and the Hex-less formats (Css, Rgb, Hsl, Hsb, the `Fmt::Css` and `Fmt::Rgb | Fmt::Hsl | Fmt::Hsb` arms, :1103-1157) are gaps: no registry variant names them.
* Nested sets (live, from the census of every call in :737-1217):
  - Button/Icon `Size=Xs, State=Default`: `picker-add-style`, `picker-none` (Remove fill) and `picker-eyedrop` are `icon_button` (widgets/mod.rs:144-150, the Icon look at Xs), the 32 px main resized
    to the caller's 24 with the glyph swapped (Plus, NoPaint, Eyedropper). Button/Close: `picker-close`, the same painter with `Icon::Close` (the Button/Close set follows `icon_button`).
    A resized swapped icon takes the Icon main's colour until #1427 merges (`fg_pending`).
  - Divider/Hairline: the four `hline(p, rect.left(), rect.right(), y, BORDER)` calls (header :799, paint row :843, contrast bar :972, swatch separator :1175), 240 wide at y 39, 79, 105, 441.
  - Not nested: Tabs/Pill (the picker's tabs are their own painter, W1c's tab census: `font_body_strong` always, hover `BG_TRANSPARENT_HOVER`); Swatch/Color (`widgets::swatch` is never called in
    `show`: the colour list paints `rect_filled` itself, the loop at :1189-1206; the only `swatch(` in the file is `paint_swatch`, the inspector swatch, :684-686); Slider/Handle (the
    strips' thumbs are
    `thumb_ring`, :674-680; `kit::slider_handle` has 0 callers: ruling (o)); Input/PropertyField and Select/Trigger (the fields are `grouped_hex_text` / `opacity_cell`, BG_SECONDARY cells with the
    outer corners rounded; the dropdowns are `picker_dropdown`, BG with a BORDER stroke and a Semibold label); Focus variants are never nested, the picker draws no focus ring of its own.
* Local parts: the frame (`BG`, a 1 px `ELEVATION_400_RING` inside stroke, `RADIUS_FRAME`; `elevation_400` also paints a shadow whose colour is alpha 0), the tabs, the six paint-type
buttons (24 px at
  8 + 28 i, `BG_SECONDARY` when active), the blend button (`button_bg` + `Icon::Blend` in ICON_SECONDARY, not Button/Icon: another glyph colour), the contrast row, the two thumbs and the two strips'
  rects, the format dropdown and source dropdown, the grouped hex and opacity field (two `BG_SECONDARY` cells with no edge at rest, each rounding only its outer corners: a per-corner
  radius) and the 11
  swatches (9 per row at pitch 24, radius 3, a 2 px `BORDER_TRANSLUCENT` edge on the dark ones). All are placed with `abs` at the rects of the Rust.
* Gradients (`color_picker_strips.py`): the SV square (`sv_area`, :637-656), the hue strip (`hue_bar`, :658-671) and the alpha strip (`checker` + `quad_mesh`, :612-635) are real gradient
paints at the
  capture's state (hue h = 0, saturation 0, value 1, alpha 1), each stop a literal with its reason (no variable holds a computed hue). The SV square is two layers (white to red, then transparent to
  black), which is exactly `v * lerp(white, hue colour, s)`; the alpha strip is a CHECKER_A base, 64 CHECKER_B cells and a white ramp, clipped at the strip's radius. Crops compare them
  pixel by pixel
  (`gradient` rows of crops.json), in windows that leave out the thumbs, the SV square's 24 AA contrast dots and, on the SV square, nothing else.
* Inline `Color32` literals, a deviation from the painter: `show` draws these colours from literals, so no theme constant and no variable holds them (G1 maps theme constants only). The
component draws the
  nearest theme tone by value, the smallest per-channel difference against every opaque colour constant of theme.rs, even when that tone is a hover role (controller rule; a tie takes the tone whose
  role fits the part). Every mapping, with the Rust (widgets/picker.rs at v0.7.5) and the distance:
  | contrast ink `0xE4E4E7` (:955): the ring, the half disc and the ratio | TEXT_SECONDARY_HOVER | 2 | (tied at 2: ICON_SECONDARY_HOVER, MENU_SHORTCUT_HIGHLIGHT, PROVIDER_GROK, READER_LIGHT_BORDER)
  | badge text `0x4CC58B` (:960) | TEXT_SUCCESS | 25 | (tied at 25: SYNTAX_STRING, TILE_DECK_GLYPH)
  | badge surface `0x17302A` (:960) | BG_SUCCESS_PRIMARY | 20 | (kept as the nearest success surface: a neutral one such as BG_TERTIARY is 15 away but loses the green)
  | hue thumb fill `0xFF0000`, the hue at the capture's position h = 0 (:1056) | BG_ERROR_SOLID | 38 |
  | alpha thumb fill `0xFFFFFF`, the colour at alpha 1 (:1058) | ICON | 0 | (tied at 0 with CANVAS_HANDLE_FILL, CHECKER_A, GUIDE_TEXT and others)
  | thumb ring `Color32::WHITE` (:679) | ICON | 0 |
  | swatch edge `Color32::from_white_alpha(0x1A)` (:1202) | BORDER_TRANSLUCENT | 0 | (an alpha colour: rgba(0xFFFFFF, 0x1A) is the same value)
  The thumbs' dark halo (`from_black_alpha(0x4D)`) is omitted. These tones are stand-ins, the exact literals are a gap (P-gate: the painter's own inline literals are the production defect).
* Gaps: the other modes and formats, the Libraries tab, the stops row of a gradient (`STOPS_H`), the dotted 4.5:1 curve on the SV square (24 dots), the swatch hover ring (`BORDER`-less 1 px
  `TEXT_TERTIARY` outside edge), the AA Large / Low badges, an open dropdown, the hex and opacity field's hover (`BORDER`) and focus (`BORDER_SELECTED`) strokes, the plain header when
  `paint_types` is off (page background, shadows).
"""
from components.color_picker_parts import FRAME_STROKE, button, contrast_bar, dropdown, hairline, hex_field, mode_button, swatch, tab, thumb
from components.color_picker_stops import stops_row
from components.color_picker_strips import alpha_strip, hue_strip, sv_square
from components.regions_sidepanels_parts import label
from rest_ir import F, I, lit

MODES = ["PaintSolid", "PaintLinear", "PaintRadial", "PaintAngular", "PaintDiamond", "PaintImage"]  # Mode::ALL (picker.rs:65)
COLOURS = ["TEXT", "TEXT_SECONDARY", "BG_SECONDARY", "BG_TERTIARY", "BG", "BG_BRAND", "DANGER", "INFO", "SUCCESS", "ICON_QUATERNARY", "TEXT_WARNING"]  # the sample list: content
W = 240
KINDS = {"Fill": {"h": 538, "dy": 0}, "Page": {"h": 498, "dy": -40}, "Gradient": {"h": 552, "dy": 14}}  # panel_height (:565-585): PAINT_ROW_H 40 less for a page, STOPS_H 40 for CONTRAST_BAR_H 26
PAGE = {"ratio": "15.29 : 1", "hex": "F5F5F5", "sv": (16, 90)}  # the page colour #F5F5F5 (picker-page): its contrast, its hex and the SV thumb at saturation 0, value 245 / 255
SOLID = {"ratio": "16.67 : 1", "hex": "FFFFFF", "sv": (16, 122)}


def header(kind, env):
    close = button("Close", "Button/Close", "Size=Xs, State=Default", (208, 8, 24, 24))
    if kind == "Page":
        return [label("Title", (16, 8, 100, 24), "Custom", "Body / Semibold", "TEXT", env), close, hairline(39)]
    return [tab("Custom tab", "Custom", (8, 8, 61, 24), True, env), tab("Libraries tab", "Libraries", (73, 8, 66.75, 24), False, env),
            button("Add style", "Button/Icon", "Size=Xs, State=Default", (180, 8, 24, 24), "Plus"), close, hairline(39)]


def paint_row(kind, env):
    blend = F("Blend", [I("Icon", "Blend", 16, env.c("ICON_SECONDARY"))], dir="H", radius=env.c("RADIUS_CONTROL"), w=24, h=24, sx="fixed", sy="fixed", abs=(180, 48, 24, 24))
    active = 1 if kind == "Gradient" else 0
    row = [mode_button(i, icon, i == active, env) for i, icon in enumerate(MODES)]
    return [*row, blend, button("Remove paint", "Button/Icon", "Size=Xs, State=Default", (208, 48, 24, 24), "NoPaint"), hairline(79)]


def picker(values, env):
    kind = values["Kind"]
    cfg = KINDS[kind]
    dy, data = cfg["dy"], PAGE if kind == "Page" else SOLID
    kids = header(kind, env) + ([] if kind == "Page" else paint_row(kind, env))
    kids += stops_row(env) if kind == "Gradient" else [contrast_bar(env, 86 + dy, data["ratio"]), hairline(105 + dy)]
    kids += [sv_square(env, dy), thumb("SV thumb", (data["sv"][0], data["sv"][1] + (dy if kind == "Gradient" else 0)), 7, 2, None, env),
             button("Eyedropper", "Button/Icon", "Size=Xs, State=Default", (16, 351 + dy, 24, 24), "Eyedropper"),
             hue_strip(env, dy), thumb("Hue thumb", (58, 349 + dy), 8, 3, "BG_ERROR_SOLID", env),
             alpha_strip(env, dy), thumb("Alpha thumb", (218, 377 + dy), 8, 3, "ICON", env),
             dropdown("Format", "Hex", (16, 397 + dy, 55, 24), env), hex_field(env, 397 + dy, data["hex"]), hairline(441 + dy), dropdown("Source", "On this page", (16, 450 + dy, 208, 24), env),
             *[swatch(i, c, env, dy) for i, c in enumerate(COLOURS)]]
    return F("Color picker", kids, fill=env.c("BG"), stroke=env.c("ELEVATION_400_RING"), sw=lit(1, FRAME_STROKE), radius=env.c("RADIUS_FRAME"), w=W, h=cfg["h"], sx="fixed", sy="fixed")


def component_sets():
    return [{"name": "Popover/ColorPicker", "row": "Menus & Popovers", "axes": {"Kind": list(KINDS)}, "build": picker, "uses": ["Button/Icon", "Button/Close", "Divider/Hairline"]}]
