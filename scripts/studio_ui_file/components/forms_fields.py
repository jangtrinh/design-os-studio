"""Registry `component` Input/Field, Input/Search, Input/PropertyField and Select/Trigger (row Forms & Inputs, second module): the text-field family,
all framed by `kit::paint_field` (widgets/kit/field.rs:43-50) or the same `paint_frame` call with a `body` look (field.rs:7-40).

The frame is `BG` filled with a 1 px `BORDER_STRONG` inside stroke and `RADIUS_CONTROL` at every size; focus swaps the stroke to `BORDER_SELECTED` and widens it to 2 px
(`W * 2.0`, field.rs:10: theme.rs has no stroke-weight constant for it, so the weight is a pinned literal). There is no hover stroke and no ring ("there is no hover stroke
and no focus ring", field.rs:42), so Hover is the Default look; a disabled field is the Default at half alpha on each paint (`kit/mod.rs:134-162`). The text itself is
drawn by the caller's TextEdit, so each set carries the caller's text geometry:

* Input/Field: Xs is `FIELD_HEIGHT` 32, the value at `FIELD_PLAIN_X` 8 in `font_body` (`text_input_ph`, widgets/field.rs:711-716); Sm is `CONTROL_SM` 36, the value 10 in
  (`Margin::symmetric(10, 0)`, home/dialog.rs:297-304, history/modal.rs:50-57) in `Text sm` Regular. 7 callers of `paint_field` (comments/ui.rs:64 is the multiline `text_box`,
  a 10 x 10 margin and top-aligned, so it is a caller but not an Xs source; comments/panel.rs:63, home/dialog.rs:297, history/modal.rs:50, widgets/field.rs:703,
  variables/widgets.rs:193, decks/ui.rs:192 at Md): the Md caller has no registry size. A placeholder is a hidden part. Xs paints it in `TEXT_TERTIARY`: `text_input_ph` ->
  `edit_text_hint` draws the hint itself (widgets/field.rs:98-101, 715), not through egui's weak colour. Sm paints it in `TEXT_PLACEHOLDER`: the dialog field sets
  `weak_text_color` to it (home/dialog.rs:308-311), and egui draws a TextEdit hint in that colour.
* Input/Search: `search_look` (field.rs:34) in two painters: the layers filter (find/filter.rs:75-118, Xs: 14 px glyph centred at 14, text at 26, a clear control) and the Home search
  (home/mod.rs:940-960, Sm: glyph centred at 14, text at 28, `Text sm` Regular, no clear control). The text is the placeholder colour until focus (`body(.., TEXT_PLACEHOLDER, TEXT)`). A 2 px
  inset and a 24 px glyph slot put the glyph's centre at 14. Md has no `search_look` caller: rule (b), Sm's geometry at `CONTROL_MD` (UNVERIFIED-VISUAL, a gap).
  Recorded D5 deviation (issue #1389: set the placeholder colour once at theme level): neither live search painter sets `weak_text_color` (home/dialog.rs:311, comments/ui.rs:77 and
  cmdk.rs:398 do), and egui replaces a TextEdit hint's colour with `weak_text_color`, not the RichText colour the painters pass. The app's weak colour is TEXT at alpha 0.6
  (`gamma_multiply(0.6)`, 154 over BG, about #9A9A9A, strongest pixel 154 on `h-home`), where the set draws `TEXT_PLACEHOLDER` (115, the colour the painters name and #1389 will make
  real). Drawing production would need TEXT at 0.6 opacity, which the G2 opacity rule blocks (opacity belongs to a State=Disabled paint only); it is not a missing variable. So the
  set keeps TEXT_PLACEHOLDER, the hint is not compared (crops-w1c.json), and `00 Start Here` records it. The same holds for any field whose painter does not set `weak_text_color`.
* Input/PropertyField: `num_field` (widgets/field.rs:563-644, 77 call lines): the prefix letter at `FIELD_PREFIX_X` 8 in `TEXT_SECONDARY`, the value at `FIELD_PREFIX_W` 24 in `TEXT`, an optional
  suffix in `TEXT_SECONDARY`; `property_field_look` (field.rs:28-31, a `TEXT_TERTIARY` prefix) has 0 callers and is not used. The icon prefix (rotation, `Prefix::Icon`, 12 px) is a gap.
* Select/Trigger: `dropdown_items` (widgets/menu.rs:598-627, 72 call lines): the label in `font_menu` 8 in, a 12 px chevron centred 12 from the right; Open and Focus are one look
  (`focused = open || has_focus`, menu.rs:603), Hover and Disabled follow the Default (no hover arm, no disabled path). No Sm caller was found (by grep: `dropdown*` is defined only in widgets/menu.rs and no `CONTROL_SM` sits within four lines before a call; the rects come from
  `pane.col`/`pane.rect` and literals, so the census is not exhaustive; the Dev panel's code dropdown, dev/sections.rs:432, is a 24 px third size): Sm is rule (b), the painter's
  geometry at `CONTROL_SM`.
"""
from rest_ir import F, I, T, lit

TWO = "kit/field.rs:10 (and 22 for an invalid field) `W * 2.0` (the focused stroke is twice BORDER_WIDTH): a product with no stroke-weight constant in theme.rs; the Semantic variables that equal 2 are other roles"
TEN = "home/dialog.rs:303 and history/modal.rs:55: `Margin::symmetric(10, 0)`, a literal in the Rust"
SIX = "widgets/menu.rs:609: the chevron is 12 wide and centred 12 from the right edge, so it ends 6 from it: a difference, no variable"
KIT = {"Open": "Focus"}  # Select/Trigger Open is the Focused look (menu.rs:603)


def frame(values, env, row, kids, **kw):
    """The shared field chrome: fill, border (colour and weight from the look), radius, half alpha when disabled."""
    state = values["State"]
    look = env.look(row, KIT.get(state, state))
    weight = lit(2, TWO) if KIT.get(state, state) == "Focus" else env.c("BORDER_WIDTH")
    return look, F("Field", kids, dir="H", fill=look["fill"], stroke=look["border"], sw=weight, radius=env.c("RADIUS_CONTROL"), alpha=look["alpha"], sx="fixed", sy="fixed",
                   align=("start", "center"), **kw)


def input_field(values, env):
    xs = values["Size"] == "Xs"
    look = env.look("Input/Field", values["State"])
    style, a = ("Body / Regular" if xs else "Text sm / Regular"), look["alpha"]
    kids = [T("Value", "Value", style, look["text"], prop="Value", visible_prop="Has Value", alpha=a),
            T("Placeholder", "Placeholder", style, env.c("TEXT_TERTIARY" if xs else "TEXT_PLACEHOLDER"), prop="Placeholder", visible_prop="Has Placeholder", hidden=True, alpha=a)]
    return frame(values, env, "Input/Field", kids, pad=[None, None, None, env.c("FIELD_PLAIN_X") if xs else lit(10, TEN)], w=216 if xs else 240,
                 h=env.num("CONTROL_XS" if xs else "CONTROL_SM"))[1]


def input_search(values, env):
    size = values["Size"]
    look = env.look("Input/Search", values["State"])
    a, style = look["alpha"], "Body / Regular" if size == "Xs" else "Text sm / Regular"
    inset, box = env.c("INSET_SEGMENT"), env.num("TARGET_MIN")
    glyph = F("Glyph slot", [I("Icon", "Search", 14, look["icon"], swap="Icon", alpha=a)], dir="H", w=box, h=box, sx="fixed", sy="fixed")
    clear = F("Clear box", [I("Clear", "Close", 12, look["icon"], alpha=a)], dir="H", w=box, h=box, sx="fixed", sy="fixed", visible_prop="Has Clear", hidden=True)
    kids = [glyph, T("Value", "Search", style, look["text"], prop="Value", sx="fill", alpha=a), clear]
    h = {"Xs": "CONTROL_XS", "Sm": "CONTROL_SM", "Md": "CONTROL_MD"}[size]
    return frame(values, env, "Input/Search", kids, pad=[None, None, None, inset], gap=inset if size != "Xs" else None, w=224 if size == "Xs" else 239, h=env.num(h))[1]


def property_field(values, env):
    look = env.look("Input/PropertyField", values["State"])
    a, prefix_w = look["alpha"], env.num("FIELD_PREFIX_W") - env.num("FIELD_PREFIX_X")
    prefix = F("Prefix slot", [T("Prefix", "X", "Body / Regular", env.c("TEXT_SECONDARY"), prop="Prefix", alpha=a)], dir="H", w=prefix_w, sx="fixed",
               align=("start", "center"))
    kids = [prefix, T("Value", "0", "Body / Regular", look["text"], prop="Value", sx="fill", alpha=a),
            T("Suffix", "°", "Body / Regular", env.c("TEXT_SECONDARY"), prop="Suffix", visible_prop="Has Suffix", hidden=True, alpha=a)]
    return frame(values, env, "Input/PropertyField", kids, pad=[None, None, None, env.c("FIELD_PREFIX_X")], w=88, h=env.num("FIELD_HEIGHT"))[1]


def select_trigger(values, env):
    look = env.look("Select/Trigger", KIT.get(values["State"], values["State"]))
    a = look["alpha"]
    kids = [T("Label", "W: Fill", "Body / Medium", look["text"], prop="Label", sx="fill", alpha=a), I("Chevron", "ChevronDown", 12, look["icon"], alpha=a)]
    return frame(values, env, "Select/Trigger", kids, pad=[None, lit(6, SIX), None, env.c("INSET_CONTROL")], w=88, h=env.num("CONTROL_XS" if values["Size"] == "Xs" else "CONTROL_SM"))[1]


def component_sets():
    states = ["Default", "Hover", "Focus", "Disabled"]
    return [
        {"name": "Input/Field", "row": "Forms & Inputs", "axes": {"Size": ["Xs", "Sm"], "State": states},
         "props": [{"name": "Value", "type": "TEXT", "default": "Value"}, {"name": "Placeholder", "type": "TEXT", "default": "Placeholder"},
                   {"name": "Has Value", "type": "BOOLEAN", "default": True}, {"name": "Has Placeholder", "type": "BOOLEAN", "default": False}], "build": input_field},
        {"name": "Input/Search", "row": "Forms & Inputs", "axes": {"Size": ["Xs", "Sm", "Md"], "State": states},
         "props": [{"name": "Value", "type": "TEXT", "default": "Search"}, {"name": "Has Clear", "type": "BOOLEAN", "default": False},
                   {"name": "Icon", "type": "INSTANCE_SWAP", "default": "Search"}], "build": input_search},
        {"name": "Input/PropertyField", "row": "Forms & Inputs", "axes": {"Size": ["Xs"], "Axis": ["Scrub"], "State": states},
         "props": [{"name": "Prefix", "type": "TEXT", "default": "X"}, {"name": "Value", "type": "TEXT", "default": "0"}, {"name": "Suffix", "type": "TEXT", "default": "°"},
                   {"name": "Has Suffix", "type": "BOOLEAN", "default": False}], "build": property_field},
        {"name": "Select/Trigger", "row": "Forms & Inputs", "axes": {"Size": ["Xs", "Sm"], "State": ["Default", "Open", "Hover", "Focus", "Disabled"]},
         "props": [{"name": "Label", "type": "TEXT", "default": "W: Fill"}], "build": select_trigger},
    ]
