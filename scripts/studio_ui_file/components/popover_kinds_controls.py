"""The controls of the settings popovers (popover_kinds_shell.py), each returning the nodes that sit in a row's 112 px control column (`CONTROL_X` 112, `CONTROL_W` 112, `ROW_H` 24,
widgets/popover.rs:10-11,55-58). Positions are relative to the row. Not a component module (no `component_sets`): `discover` skips it. Lines are the v0.7.5 tag of crates/designos-app/src.

* text_field: `field::text_input` (widgets/field.rs:707-716), the Input/Field Xs copy sized to the column; a numeric field with no prefix (`Prefix::None`, value at `FIELD_PLAIN_X` 8) is
  the same copy.
* select: `dropdown` -> `dropdown_items` (widgets/menu.rs:598-627), the Select/Trigger Xs copy sized to the column.
* num: `field::num_field` (widgets/field.rs:563-644) with `Prefix::Icon`: the glyph (`FIELD_PREFIX_GLYPH` 12) at `FIELD_PREFIX_X` 8, centred, and the value at `FIELD_PREFIX_W` 24, which is where
  Input/PropertyField puts its value after its 16 px prefix slot. The set draws a letter prefix only, so the copy's prefix text is empty and the glyph sits over its slot (a recorded gap of the set).
  The suffix (`%`, a degree sign) is part of the value text: the set's Value text fills the field, so its own suffix text would sit at the far edge where production draws it after the value.
* color_hex: `effects::color_row` (panels/effects.rs:298-317): the field chrome, a `SWATCH_SIZE` swatch at x + 4 and the hex at x + 24, the same slot as the numeric prefix. The swatch
  is drawn local: the
  colour is the style's own content, which a variant cannot set on a nested Swatch/Color copy, so it draws the nearest theme tone by value (see TONE).
* pill: `variables::widgets::pill` (variables/widgets.rs:117-149), the bound-variable pill that replaces a field: BG_SECONDARY at RADIUS_CONTROL, a hexagon at x + 12, the optional swatch, the name.
* check: `popover::check` -> `kit::checkbox` (widgets/popover.rs:67-71, widgets/kit/choice.rs:99-113): the 16 px box at the left of the control column, centred on the row, inside a frame of the
  column's size.
* dial: `popover::dial` (widgets/popover.rs:73-101): a 24 px disc (BG_SECONDARY, a BORDER ring), a 1.5 px needle and a 2.5 px tip.
"""
import math

from rest_ir import E, F, I, T, V, X, lit

CONTROL_X, CONTROL_W, ROW_H = 112.0, 112.0, 24.0
FIELD = "Size=Xs, State=Default"
PROPERTY = "Size=Xs, Axis=Scrub, State=Default"
GLYPH, GLYPH_X = 12, 8.0  # theme.rs FIELD_PREFIX_GLYPH, FIELD_PREFIX_X
HEX_R = 5.0  # variables/widgets.rs:20 `hexagon(p, c, r, ..)` called with 5.0 (variables/widgets.rs:125)
HEX_STROKE = "variables/widgets.rs:30 `Stroke::new(1.2, col)`: a literal of the Rust, no stroke-weight constant in theme.rs"
NEEDLE = "widgets/popover.rs:84 `Stroke::new(1.5, theme::TEXT)`: a literal of the Rust, no stroke-weight constant in theme.rs"
RING = "widgets/popover.rs:81 `circle_stroke(c, rad - 0.5, Stroke::new(1.0, ..))`: a ring of one BORDER_WIDTH drawn half a pixel inside the disc"
DISC = "widgets/popover.rs:79 `circle_filled(c, rad, ..)`: a disc, its radius half its 24 px box"
SWATCH_X, SWATCH = 4.0, 16.0  # panels/effects.rs:299 `r.left() + 4.0`, theme.rs SWATCH_SIZE
# The nearest theme tone by value of a style's own colour, which no constant holds (the largest per-channel difference in brackets): red (255, 0, 0) is BG_ERROR_SOLID (38),
# white is TEXT_WHITE (0), black (0, 0, 0) is BG (10).
TONE = {"red": "BG_ERROR_SOLID", "white": "TEXT_WHITE", "black": "BG"}


def _col(w=CONTROL_W, x=CONTROL_X):
    return (x, 0.0, w, ROW_H)


def text_field(value, name="Control"):
    return X(name, "Input/Field", FIELD, props={"Value": value} if value else {"Has Value": False}, abs=_col())


def select(label, name="Control"):
    return X(name, "Select/Trigger", FIELD, props={"Label": label}, abs=_col())


def num(env, icon, value, x=CONTROL_X, w=CONTROL_W):
    """A numeric field with an icon prefix: the PropertyField copy with an empty prefix text and the glyph over its slot."""
    glyph = I("Prefix icon", icon, GLYPH, env.c("TEXT_SECONDARY"), abs=(x + GLYPH_X, (ROW_H - GLYPH) / 2, GLYPH, GLYPH))
    return [X("Control", "Input/PropertyField", PROPERTY, props={"Prefix": "", "Value": value}, abs=_col(w, x)), glyph]


def swatch(env, tone, x, size=SWATCH):
    return F("Swatch", fill=env.c(TONE.get(tone, tone)), stroke=env.c("BORDER_TRANSLUCENT"), sw=env.c("BORDER_WIDTH"), radius=env.c("SWATCH_RADIUS"), w=size, h=size, sx="fixed", sy="fixed",
             abs=(x, (ROW_H - size) / 2, size, size))


def checker(env, x, size=12.0):
    """`widgets::swatch` over a translucent colour (widgets/mod.rs:159-169): the CHECKER_A base with two CHECKER_B quarters, the colour itself dropped (it is the style's content)."""
    half, y = size / 2, (ROW_H - size) / 2
    quarter = lambda n, dx, dy: F(n, fill=env.c("CHECKER_B"), w=half, h=half, sx="fixed", sy="fixed", abs=(dx, dy, half, half))
    return F("Swatch", [quarter("Checker 1", 0.0, 0.0), quarter("Checker 2", half, half)], fill=env.c("CHECKER_A"), radius=env.c("SWATCH_RADIUS"), w=size, h=size, sx="fixed", sy="fixed",
             abs=(x, y, size, size))


def color_hex(env, hex_text, tone):
    props = {"Prefix": "", "Value": hex_text}
    return [X("Control", "Input/PropertyField", PROPERTY, props=props, abs=_col()), swatch(env, tone, CONTROL_X + SWATCH_X)]


def hexagon(env, cx, cy):
    pts = [(cx + HEX_R * math.cos(math.pi / 3 * i - math.pi / 2), cy + HEX_R * math.sin(math.pi / 3 * i - math.pi / 2)) for i in range(6)]
    path = "M" + " L".join(f"{x:.3f} {y:.3f}" for x, y in pts) + " Z"
    return V("Hexagon", path, 2 * cx, 2 * cy, None, stroke=env.c("ICON_SECONDARY"), sw=lit(1.2, HEX_STROKE), join="miter", abs=(0.0, 0.0, 2 * cx, 2 * cy))


def pill(env, text, swatch_tone=None):
    """The bound variable pill: the hexagon at x + 12, the 12 px swatch at x + 24 (`swatch_tone` "checker" draws the alpha checker), the name after it."""
    kids, x = [hexagon(env, 12.0, ROW_H / 2)], 24.0
    if swatch_tone:
        kids.append(checker(env, x) if swatch_tone == "checker" else swatch(env, swatch_tone, x, 12.0))
        x += 18.0
    kids.append(T("Name", text, "Body / Regular", env.c("TEXT"), abs=(x, (ROW_H - 18) / 2, CONTROL_W - x - 8, 18)))
    return [F("Control", kids, fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=CONTROL_W, h=ROW_H, sx="fixed", sy="fixed", abs=_col())]


def check(on=True, w=CONTROL_W):
    """`popover::check` (widgets/popover.rs:67): the control rect is the column (its `w` wide, `ROW_H` high, the rect the capture names), the 16 px box sits at its left, centred on the row."""
    box = X("Box", "Checkbox/Box", f"State={'Selected' if on else 'Default'}", abs=(0.0, (ROW_H - 16) / 2, 16.0, 16.0))
    return F("Control", [box], w=w, h=ROW_H, sx="fixed", sy="fixed", abs=_col(w))


def dial(env):
    """The light-angle dial: a 24 px disc, its ring and the needle to the tip (315 degrees: up and to the right of centre)."""
    c, rad = 12.0, 11.5
    ring_d = 2 * (rad - 0.5)
    ring = E("Ring", ring_d, ring_d, stroke=env.c("BORDER"), sw=lit(1.0, RING), sa="center", sx="fixed", sy="fixed", abs=(c - ring_d / 2, c - ring_d / 2, ring_d, ring_d))
    a = math.radians(315)
    tip = (c + math.cos(a) * (rad - 4), c + math.sin(a) * (rad - 4))
    needle = V("Needle", f"M{c:g} {c:g} L{tip[0]:.3f} {tip[1]:.3f}", 24, 24, None, stroke=env.c("TEXT"), sw=lit(1.5, NEEDLE), cap="none", abs=(0.0, 0.0, 24, 24))
    dot = E("Tip", 5, 5, fill=env.c("TEXT"), sx="fixed", sy="fixed", abs=(tip[0] - 2.5, tip[1] - 2.5, 5, 5))
    return F("Dial", [ring, needle, dot], fill=env.c("BG_SECONDARY"), radius=lit(12, DISC), w=24.0, h=24.0, sx="fixed", sy="fixed", abs=(CONTROL_X, 0.0, 24.0, 24.0))
