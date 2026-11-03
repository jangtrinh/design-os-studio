"""Paint sections of the selection inspector: Fill, Stroke, Effects and Selection colors (`panels/properties.rs`, `stroke.rs`, `effects.rs`; lines are the 0.7.5 tag).

* Fill / Stroke / Effects titles (`paint_title` properties.rs:230-236, `effects::section` effects.rs:196): the header's Plus and the styles button 28 px left of it (a Button/Icon
  with the Styles glyph at `SECTION_TRAILING_X` - 28); a section with no paint, stroke or effect is the title row alone (the capture's Stroke and Effects), and a title that is dim
  (`dim`, TEXT_SECONDARY) is drawn in TEXT: the header holds one title colour.
* `paint_rows` (properties.rs:1573-1714): a paint row advances `FIELD_HEIGHT` + 8 and the first sits 4 under the title; its group is 159 wide (`PAINT_GROUP_W`, :1569) in `BG_SECONDARY` at
  `RADIUS_CONTROL`: a 16 px swatch 4 in, the hex 24 in (`FIELD_PREFIX_X` + `FIELD_PREFIX_ICON`), a 1 px `BG` divider at `PAINT_DIVIDER_X` 104 (:1571), the opacity after it and a `%`; then the
  eye at `RIGHT_NAME_ACTION_X[1]` + 3 and the minus at `SECTION_TRAILING_X`. Fill ends with 8 more (:1055-1056) and the section's 4, so 16 under the last row.
* `selection_colors_section` (properties.rs:769-821): after the title (no gap) a row per colour, 32 + 8 apart: a 16 px swatch 4 down at `SWATCH_ROW_X`, the hex field 96 wide 24 px right of
it (Input/Field), and
  the opacity badge 52 wide at +100 (`BG_SECONDARY`, `font_caption` in TEXT_SECONDARY, `100 %`, or `100% · N` for a colour used N times).
The swatch is the node's own paint and an instance override cannot recolour it: each swatch takes the theme tone nearest the representative capture's colour (27272A: BG_TERTIARY, E36323: BG_BRAND).
"""
from components.regions_editor_right_parts import icon_button, plain_field, row, section, title
from rest_ir import F, T

SEC_W = 239.0
HEX_X, PERCENT_RIGHT = 28.0, 7.0  # properties.rs:1662 `field_text(.., 0.0, 4.0, ..)` after the 24 px prefix; the `%` of page_field (right 7)


def styled_title(env, name, text):
    return title(env, name, text, plus=True, extra=[("Styles", env.num("SECTION_TRAILING_X") - 28.0)])


def swatch(env, x, y, tone="INFO"):
    side = env.num("SWATCH_SIZE")
    return F("Swatch", fill=env.c(tone), radius=env.c("RADIUS_MARK"), w=side, h=side, sx="fixed", sy="fixed", abs=(x, y, side, side))


def paint_row(env, name, hex_text, opacity="100", tone="INFO"):
    """One paint row: the 159 x 32 group and the eye and minus buttons."""
    w, h, div = 159.0, env.num("FIELD_HEIGHT"), 104.0
    text = lambda n, s, x, tw: T(n, s, "Body / Regular", env.c("TEXT"), abs=(x, 7.0, tw, 18.0))
    divider = F("Divider", fill=env.c("BG"), w=env.num("BORDER_WIDTH"), h=h, sx="fixed", sy="fixed", abs=(div, 0.0, 1.0, h))
    kids = [swatch(env, 4.0, (h - env.num("SWATCH_SIZE")) / 2, tone), text("Hex", hex_text, HEX_X, 60.0), divider,
            text("Opacity", opacity, div + 9.0, 24.0), text("Percent", "%", w - PERCENT_RIGHT - 9.0, 9.0)]
    cell = F(f"{name} group", kids, fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=w, h=h, sx="fixed", sy="fixed", abs=(0.0, 0.0, w, h))
    eye = icon_button(env, "Eye", "Eye", env.num("RIGHT_NAME_ACTION_X[1]") + 3.0 - env.num("SECTION_CONTENT_X"))
    minus = icon_button(env, "Minus", "Minus", env.num("SECTION_TRAILING_X") - env.num("SECTION_CONTENT_X"))
    return row(env, f"{name} row", [cell, eye, minus])


def fill_section(env, hex_text="27272A", tone="BG_TERTIARY"):
    rows = F("Fill rows", [paint_row(env, "Fill 1", hex_text, tone=tone)], dir="V", gap=env.c("SPACE_INLINE"), pad=[None,
            None, None, env.c("SECTION_CONTENT_X")], w=SEC_W, sx="fixed", sy="hug", align=("start", "start"))
    return section(env, "Fill", styled_title(env, "Fill", "Fill"), [rows], bottom="SPACE_BLOCK", top="SPACE_FACT")


def stroke_section(env):
    return section(env, "Stroke", styled_title(env, "Stroke", "Stroke"), [])


def effects_section(env):
    return section(env, "Effects", styled_title(env, "Effects", "Effects"), [])


def colour_row(env, n, hex_text, count, tone):
    badge_text = "100 %" if count < 2 else f"100% · {count}"
    side = env.num("SWATCH_SIZE")
    hexf = plain_field(env, "Hex field", side + 8.0, hex_text, 96.0)
    label = T("Count", badge_text, "Caption / Regular", env.c("TEXT_SECONDARY"))
    badge = F("Opacity badge", [label], dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), align=("center", "center"), w=52.0, h=env.num("FIELD_HEIGHT"), sx="fixed",
              sy="fixed", abs=(side + 8.0 + 100.0, 0.0, 52.0, env.num("FIELD_HEIGHT")))
    return row(env, f"Colour {n} row", [swatch(env, 0.0, 4.0, tone), hexf, badge])


def selection_colors_section(env, colours=(("27272A", "BG_TERTIARY", 1), ("E36323", "BG_BRAND", 1))):
    rows = F("Colour rows", [colour_row(env, n, h, c, t) for n, (h, t, c) in enumerate(colours, 1)], dir="V", gap=env.c("SPACE_INLINE"), pad=[None, None, None, env.c("SECTION_CONTENT_X")], w=SEC_W,
             sx="fixed", sy="hug", align=("start", "start"))
    return section(env, "Selection colors", title(env, "Selection colors", "Selection colors"), [rows], bottom="SPACE_GROUP")
