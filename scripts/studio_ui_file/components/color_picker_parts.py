"""Builders for the parts of Popover/ColorPicker (color_picker.py): every part is placed with `abs` at the rect `picker::show` computes, so each helper returns one node with its `abs`.
Not a component module (no `component_sets`): `discover` skips it."""
from rest_ir import E, F, I, T, X, lit

GLYPH_STROKE = "widgets/picker.rs:956,1202: `Stroke::new(1.2, ..)` and `Stroke::new(2.0, ..)`: literals of the Rust, no stroke-weight constant in theme.rs"
THUMB_STROKE = "widgets/picker.rs:674-680 `thumb_ring(p, c, outer, ring, fill)`: a ring of `ring` px (2 on the area, 3 on the strips), a literal of the Rust with no stroke-weight constant"
SWATCH_RADIUS = "widgets/picker.rs:1196 `theme::cr((SWATCH * 0.2).round())` = round(16 * 0.2) = 3: computed from the swatch's size, no radius role holds it"
BADGE_RADIUS = "widgets/picker.rs:969 `theme::cr(4)`: a literal of the Rust, 4 is no radius role (RADIUS_MARK 2, RADIUS_SEGMENT 6)"
CHEVRON_RIGHT = "widgets/picker.rs:1299: the 12 px chevron is centred 12 from the right edge, so it ends 6 from it: a difference with no variable"
SWATCH_EDGE = "widgets/picker.rs:1202 `Stroke::new(2.0, from_white_alpha(0x1A))`: a literal of the Rust, no stroke-weight constant in theme.rs"
FRAME_STROKE = "widgets/picker.rs:749 `Stroke::new(1.0, theme::ELEVATION_400_RING)`: a literal of the Rust (BORDER_WIDTH is also 1, but `show` does not name it)"
DROPDOWN_STROKE = "widgets/picker.rs:1293 `Stroke::new(1.0, ..)`: a literal of the Rust (BORDER_WIDTH is also 1, but `picker_dropdown` does not name it)"
DROPDOWN_PAD = "widgets/picker.rs:1298 `rect.left() + 8.0`: a literal of the Rust (INSET_CONTROL is also 8, but `picker_dropdown` does not name it)"
TAB_PAD = ("widgets/picker.rs:758,777: the tab is `text_width + 16.0` wide and its label starts at `r.left() + 8.0`: "
           "8 a side, literals of the Rust (INSET_CONTROL is also 8, but `show` does not name it)")
HEX_PAD = "widgets/picker.rs:1093: `grouped_hex_text(.., 8.0)`: a literal of the Rust (INSET_CONTROL is also 8, but the call does not name it)"
OPACITY_PAD = "widgets/picker.rs:1436 `edit_num_text(.., 8.0, ..)`: a literal of the Rust (INSET_CONTROL is also 8, but `opacity_cell` does not name it)"
PERCENT_PAD = "widgets/picker.rs:1438 `r.right() - 8.0 - text_width(\"%\")`: a literal of the Rust (INSET_CONTROL is also 8, but `opacity_cell` does not name it)"
HUE_RADIUS = "widgets/picker.rs:1043 `round_corners(p, hue_r, 6.0, ..)`: a literal of the Rust (RADIUS_SEGMENT is also 6, but the call does not name it)"
ALPHA_RADIUS = "widgets/picker.rs:1054 `round_corners(p, al_r, 6.0, ..)`: a literal of the Rust (RADIUS_SEGMENT is also 6, but the call does not name it)"
HALF_DISC_RADIUS = "widgets/picker.rs:957 `rect_filled(.., 2.0, fg_col)`: a literal of the Rust (RADIUS_MARK is also 2, but `show` does not name it)"


def button(name, set_name, variant, rect, icon=None):
    """An icon_button (widgets/mod.rs:144-150) at `rect`: the set's 32 px main resized to 24, its glyph swapped to `icon`."""
    return X(name, set_name, variant, props={"Icon": icon} if icon else None, abs=rect)


def hairline(y, w=240):
    """`hline(p, rect.left(), rect.right(), y, BORDER)`: Divider/Hairline's painter."""
    return X(f"Rule {y}", "Divider/Hairline", "State=Default", abs=(0, y, w, 1))


def thumb(name, centre, outer, ring, fill, env):
    """`thumb_ring` (picker.rs:674-680): a white ring `ring` px wide centred on `outer - ring / 2`, and, when `fill`, a disc of `outer - ring` inside it. The 1 px dark halo
    (`from_black_alpha(0x4D)`) has no Semantic variable (a raw alpha of black): a gap."""
    size = 2 * outer
    ring_d = 2 * (outer - ring / 2)
    kids = ([E("Disc", 2 * (outer - ring), 2 * (outer - ring), fill=env.c(fill), sx="fixed", sy="fixed", abs=(ring, ring, 2 * (outer - ring), 2 * (outer - ring)))] if fill else [])
    kids.append(E("Ring", ring_d, ring_d, stroke=env.c("ICON"), sw=lit(ring, THUMB_STROKE), sa="center", sx="fixed", sy="fixed", abs=(outer - ring_d / 2, outer - ring_d / 2, ring_d, ring_d)))
    return F(name, kids, w=size, h=size, sx="fixed", sy="fixed", abs=(centre[0] - outer, centre[1] - outer, size, size))


def dropdown(name, label, rect, env):
    """`picker_dropdown` (picker.rs:1283-1300): BG with a 1 px BORDER inside stroke at RADIUS_CONTROL, the label `Body` Semibold in TEXT 8 in, a 12 px ICON_SECONDARY chevron 12 from the right."""
    return F(name, [T("Label", label, "Body / Semibold", env.c("TEXT")), I("Chevron", "ChevronDown", 12, env.c("ICON_SECONDARY"))], dir="H", align=("between", "center"), fill=env.c("BG"),
             stroke=env.c("BORDER"), sw=lit(1, DROPDOWN_STROKE), radius=env.c("RADIUS_CONTROL"), pad=[None, lit(6, CHEVRON_RIGHT), None, lit(8, DROPDOWN_PAD)], w=rect[2], h=rect[3],
             sx="fixed", sy="fixed", abs=rect)


def tab(name, label, rect, active, env):
    """A header tab (picker.rs:754-787): `text + 16` wide, 24 high, RADIUS_CONTROL, `BG_SECONDARY` when active, the label 8 in in `Body` Semibold."""
    pad = lit(8, TAB_PAD)
    return F(name, [T("Label", label, "Body / Semibold", env.c("TEXT" if active else "TEXT_SECONDARY"))], dir="H", fill=env.c("BG_SECONDARY") if active else None, radius=env.c("RADIUS_CONTROL"),
             pad=[None, pad, None, pad], h=rect[3], sx="hug", sy="fixed", abs=rect)


def mode_button(i, icon, active, env):
    """A paint-type button (picker.rs:809-817): 24 px at 8 + 28 i, `BG_SECONDARY` when active, the 16 px glyph in ICON (active) or ICON_SECONDARY."""
    glyph = I("Icon", icon, 16, env.c("ICON" if active else "ICON_SECONDARY"))
    return F(f"Mode {i}", [glyph], dir="H", fill=env.c("BG_SECONDARY") if active else None, radius=env.c("RADIUS_CONTROL"), w=24, h=24, sx="fixed", sy="fixed", abs=(8 + 28 * i, 48, 24, 24))


def swatch(i, colour, env, dy=0):
    """A swatch of the colour list (picker.rs:1189-1206): 16 px at pitch 24, 9 per row, radius 3; the dark ones carry a 2 px white-alpha inside edge."""
    edge = {"stroke": env.c("BORDER_TRANSLUCENT"), "sw": lit(2, SWATCH_EDGE)} if colour in ("BG", "BG_SECONDARY", "BG_TERTIARY") else {}
    x, y = 16 + 24 * (i % 9), 482 + dy + 24 * (i // 9)
    return F(f"Swatch {i}", fill=env.c(colour), radius=lit(3, SWATCH_RADIUS), w=16, h=16, sx="fixed", sy="fixed", abs=(x, y, 16, 16), **edge)


def contrast_bar(env, y=86, ratio_text="16.67 : 1"):
    """The contrast row (picker.rs:949-973) at (16, 86, 208, 20): a 11 px ring and a half disc (the contrast glyph), `ratio : 1` in `Body` Semibold and the level badge at the right end."""
    ink = env.c("TEXT_SECONDARY_HOVER")  # the painter's inline Color32 (0xE4, 0xE4, 0xE7, :955) has no variable: the nearest theme tone by value (distance 2), a hover role
    ring = E("Ring", 11, 11, stroke=ink, sw=lit(1.2, GLYPH_STROKE), sa="center", sx="fixed", sy="fixed", abs=(1.5, 4.5, 11, 11))
    half = F("Half", fill=ink, radius=lit(2, HALF_DISC_RADIUS), w=5, h=9, sx="fixed", sy="fixed", abs=(2, 5.5, 5, 9))
    ratio = T("Ratio", ratio_text, "Body / Semibold", ink, abs=(18, 1, 60, 18))
    level = F("Badge", [T("Level", "AAA", "Caption / Semibold", env.c("TEXT_SUCCESS"))], dir="H", fill=env.c("BG_SUCCESS_PRIMARY"), radius=lit(4, BADGE_RADIUS), w=44, h=20,
              sx="fixed", sy="fixed", abs=(164, 0, 44, 20))
    return F("Contrast bar", [ring, half, ratio, level], w=208, h=20, sx="fixed", sy="fixed", abs=(16, y, 208, 20))


def hex_field(env, y=397, value="FFFFFF"):
    """`grouped_hex_text` (picker.rs:1398-1408) + `opacity_cell` (:1424-1440): two BG_SECONDARY cells with no edge at rest (the Rust strokes `BORDER_SELECTED` only `if
    focused` and `BORDER` only `else if` the pointer is over the cell: hover and focus have no registry state, so both strokes are gaps), 89 and 54 wide with a 1 px gap
    that shows the popover's BG, the hex `Body` TEXT 8 in and the opacity `Body` TEXT with a `%` in TEXT_SECONDARY 8 from the right. Each cell rounds only its outer
    corners, as `cr_each(RADIUS_CONTROL, 0, 0, RADIUS_CONTROL)` (:1092) and `cr_each(0, RADIUS_CONTROL, RADIUS_CONTROL, 0)` (:1427) do: a per-corner radius, [tl, tr, br,
    bl]."""
    control = env.c("RADIUS_CONTROL")
    hex_cell = F("Hex cell", [T("Hex", value, "Body / Regular", env.c("TEXT"))], dir="H", align=("start", "center"), fill=env.c("BG_SECONDARY"), radius=[control, None, None, control],
                 pad=[None, None, None, lit(8, HEX_PAD)], w=89, sx="fixed", sy="fill")
    gap = F("Gap", fill=env.c("BG"), w=1, sx="fixed", sy="fill")
    opacity = F("Opacity cell", [T("Opacity", "100", "Body / Regular", env.c("TEXT")), T("Percent", "%", "Body / Regular", env.c("TEXT_SECONDARY"))], dir="H", align=("between", "center"),
                fill=env.c("BG_SECONDARY"), radius=[None, control, control, None], pad=[None, lit(8, PERCENT_PAD), None, lit(8, OPACITY_PAD)], w=54, sx="fixed", sy="fill")
    return F("Hex field", [hex_cell, gap, opacity], dir="H", align=("start", "center"), w=144, h=24, sx="fixed", sy="fixed", abs=(80, y, 144, 24))
