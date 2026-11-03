"""The seven settings popovers drawn from `popover::show` + `popover::row` rows (popover_kinds_shell.py), each a function of `env` returning (title, rows). Not a component module.
Lines are the v0.7.5 tag of crates/designos-app/src. A neutral content stands where the capture shows a kit's own name (the style `Name` and `Description`, an elided variable name).

* EditStyleColor, EditStyleEffect, EditStyleGrid, EditStyleText: `panels/style_edit.rs:111` `popover_ui` (240 wide, `popover::height(rows)`, title "Edit style") over `body_ui` (:142):
  Name and Description,
  an item picker when the style has several items (not drawn: none of the captures has one), then `fill_rows` (:194), `effects::effect_body` (panels/effects.rs:360, the shadow rows),
  `grid_rows::grid_rows` (panels/style_edit_grid_rows.rs:14) or `text_rows` (panels/style_edit.rs:229). A bound property is the variable pill (`style_pill_field`, panels/var_bind.rs:86), as
  the captures show for the
  colour, font family, size and line height.
* Stroke: `panels/stroke.rs:112` `popover_ui`, five rows. Noise, Texture, Glass: `effects::effect_body` (panels/effects.rs:360) for `Effect::Noise`, `Effect::Texture` and
  `Effect::Glass`, behind `popover_ui`
  (panels/effects.rs:319).
"""
from components.popover_kinds_controls import check, color_hex, dial, num, pill, select, text_field
from components.popover_kinds_shell import CONTROL_X


def _name_rows():
    return [("Name", [text_field("Style name")]), ("Description", [text_field(None)])]


def edit_style_color(env):
    return "Edit style", _name_rows() + [("Color", pill(env, "Color...", "white")), ("Opacity", num(env, "Opacity", "100%"))]


def edit_style_effect(env):
    return "Edit style", _name_rows() + [("X", num(env, "DirHorizontal", "0")), ("Y", num(env, "DirVertical", "1")), ("Blur", num(env, "LayerBlur", "2")),
                                         ("Spread", num(env, "Scale", "0")), ("Color", pill(env, "Color...", "checker")), ("Blend", [select("Normal")]), ("Show behind", [check()])]


def edit_style_grid(env):
    return "Edit style", _name_rows() + [("Guide", [select("1 · Columns")]), ("Type", [select("Columns")]), ("Count", num(env, "GridView", "12")),
                                         ("Gutter", num(env, "Spacing", "32")), ("Margin", num(env, "PaddingH", "112")), ("Color", color_hex(env, "FF0000", "red")),
                                         ("Opacity", num(env, "Opacity", "4%"))]


def edit_style_text(env):
    return "Edit style", _name_rows() + [("Font", pill(env, "Font fam...")), ("Weight", [select("Regular")]), ("Size", pill(env, "Font size...")),
                                         ("Line height", pill(env, "Line heig...")), ("Letter spacing", num(env, "PaddingH", "-1.44"))]


def stroke(env):
    return "Stroke settings", [("Cap", [select("None")]), ("Join", [select("Miter")]), ("Miter angle", [text_field("29°")]), ("Dash", [text_field("0")]), ("Gap", [text_field("0")])]


def noise(env):
    return "Noise", [("Mode", [select("Monotone")]), ("Color", color_hex(env, "000000", "black")), ("Size", num(env, "Scale", "1")), ("Density", num(env, "Noise", "50%")),
                     ("Opacity", num(env, "Opacity", "25%"))]


def glass(env):
    angle = num(env, "Rotation", "315°", x=CONTROL_X + 32, w=80.0)
    return "Glass", [("Light angle", [dial(env), *angle]), ("Intensity", num(env, "Sparkle", "50%")), ("Refraction", num(env, "Glass", "50%")), ("Depth", num(env, "DropShadow", "10")),
                     ("Dispersion", num(env, "PaintLinear", "25%")), ("Frost", num(env, "BackgroundBlur", "4"))]


def texture(env):
    return "Texture", [("Size", num(env, "Scale", "2")), ("Radius", num(env, "Radius", "2")), ("Clip to shape", [check()])]


TABLES = {"EditStyleColor": edit_style_color, "EditStyleEffect": edit_style_effect, "EditStyleGrid": edit_style_grid, "EditStyleText": edit_style_text, "Stroke": stroke, "Noise": noise,
          "Glass": glass, "Texture": texture}
