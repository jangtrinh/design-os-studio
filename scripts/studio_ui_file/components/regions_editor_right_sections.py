"""Sections of the selection inspector (`panels/properties.rs` `selection_ui`, :823), each one builder that a state composes (`regions_editor_right_states`). Lines are the 0.7.5 tag.

* `name_row` (:832-869): the name at `RIGHT_NAME_FIELD_X` + 8 in `font_large` (the field draws no border at rest; a multi-selection says `N layers`) and the three quick toggles
  (`RIGHT_NAME_ACTION_X`: Mask, Unlock, Eye) as 24 px icon buttons at the row's top; the row advances `FIELD_HEIGHT` + 8.
* `copy_row` (:870-886): `Copy to Slide Deck`, 184 x 26 at `SECTION_CONTENT_X`, the row advances 32. A local button (`button`): the painter's fill and stroke are the literals 0x2C2C2C and
  0x383838 (:875-877), no state of Button/Secondary holds them.
* `position` (:897-956): Alignment (two 3-cell strips, 8 px apart, and the distribute button at `DISTRIBUTE_BTN_X`), Position (X, Y) and Rotation (a field with the Rotation glyph and the
  `°` suffix, then the
  Rotate90 / FlipH / FlipV strip at the second column).
* `auto_layout_section` (:960-971, `auto_layout_ui` :1208-1347): the title with Minus, Flow (a 4 x 22 strip and the gap field), Dimensions, Resizing, Padding and Alignment (two strips with the
  auto-spacing button 4 px after the first, :1316-1317) and the `Update auto layout` button of a layout older than v1 (:1332-1346, 24 px, 4 px below the row, 2 px of room under it).
* `layout_section` (:972-996, `dimensions_ui` :1142-1153): the title and Dimensions, for a layer that is not an auto-layout frame.
* `appearance_section` (:1006-1039): the title, opacity (the Opacity glyph, `%`) and corner radius (the Radius glyph) side by side, and for a frame the `Clip content` checkbox 4 px below
  (:1026-1035). The variable-mode rows under it (`var_bind::mode_rows`, var_bind.rs:215-243, 7 rows of 36 px, a collection name and a mode dropdown each) are not drawn: see the size note
  in `regions_editor_right_states`.
"""
from components.regions_editor_right_parts import button, cols, field, group, icon_button, row, section, select, strip, title
from rest_ir import F, T, X

ALIGN_H, ALIGN_V = ("AlignLeft", "AlignHCenter", "AlignRight"), ("AlignTop", "AlignVCenter", "AlignBottom")
FLOW = ("DirHorizontal", "DirVertical", "Wrap", "Grid2x2")
SEC_W = 239.0


def name_row(env, text, component=False):
    kids = [T("Name", text, "Large / Regular", env.c("TEXT_COMPONENT" if component else "TEXT"), abs=(env.num("RIGHT_NAME_FIELD_X") - 1 + 8.0, 6.0, 120.0, 20.0))]
    kids += [icon_button(env, n, icon, env.num("RIGHT_NAME_ACTION_X[%d]" % i) - 1) for i, (n, icon) in enumerate((("Mask", "Mask"), ("Lock", "Unlock"), ("Eye", "Eye")))]
    return F("Name row", kids, w=SEC_W, h=env.num("FIELD_HEIGHT") + env.num("SPACE_INLINE"), sx="fixed", sy="fixed")


def copy_row(env):
    copy = button(env, "Copy button", "Copy to Slide Deck", env.num("SECTION_CONTENT_X"), 0.0, env.num("SECTION_CONTENT_W"), 26.0)
    return F("Copy row", [copy], w=SEC_W, h=32.0, sx="fixed", sy="fixed")


def position_section(env):
    c = cols(env)
    gap = env.num("SEGMENT_GROUP_GAP")
    align = row(env, "Alignment row", [strip(env, "Align horizontal", ALIGN_H, 0.0), strip(env, "Align vertical", ALIGN_V, 3 * env.num("SEGMENT_W") + gap),
                                      icon_button(env, "Distribute", "Distribute", env.num("DISTRIBUTE_BTN_X") - env.num("SECTION_CONTENT_X"))])
    xy = row(env, "Position row", [field(env, "X field", c[0], "X", "0"), field(env, "Y field", c[1], "Y", "0")])
    rot = row(env, "Rotation row", [field(env, "Rotation field", c[0], "", "0", "\u00b0", glyph="Rotation"), strip(env, "Transform", ("Rotate90", "FlipH", "FlipV"), c[1])])
    return section(env, "Position", title(env, "Position", "Position"), [group(env, "Alignment", "Alignment", align), group(env, "Position", "Position", xy), group(env, "Rotation", "Rotation", rot)])


def dimensions_group(env):
    c = cols(env)
    return group(env, "Dimensions", "Dimensions", row(env, "Dimensions row", [field(env, "W field", c[0], "W", "100"), field(env, "H field", c[1], "H", "100")]))


def auto_layout_section(env):
    c, seg = cols(env), env.num("SEGMENT_W")
    flow = row(env, "Flow row", [strip(env, "Flow direction", FLOW, 0.0, (0,), 22.0), field(env, "Gap field", c[1], "", "12", glyph="Spacing")])
    sizing = row(env, "Resizing row", [select(env, "W sizing", c[0], "W: Fill"), select(env, "H sizing", c[1], "H: Hug")])
    pad = row(env, "Padding row", [field(env, "Horizontal padding", c[0], "", "16", glyph="PaddingH"), field(env, "Vertical padding", c[1], "", "16", glyph="PaddingV")])
    space = icon_button(env, "Auto spacing", "Spacing", 3 * seg + 4.0)
    align = row(env, "Alignment row", [strip(env, "Main alignment", ALIGN_H, 0.0, (0,)), space, strip(env, "Cross alignment", ALIGN_V, c[1] + 16.0, (0,))])
    update = button(env, "Update button", "Update auto layout", 0.0, 4.0, env.num("SECTION_CONTENT_W"), 24.0, radius="RADIUS_CONTROL")
    update_row = F("Update row", [update], w=env.num("SECTION_CONTENT_W"), h=30.0, sx="fixed", sy="fixed")
    groups = [group(env, "Flow", "Flow", flow), dimensions_group(env), group(env, "Resizing", "Resizing", sizing), group(env, "Padding", "Padding", pad),
              group(env, "Alignment", "Alignment", align, update_row)]
    return section(env, "Auto layout", title(env, "Auto layout", "Auto layout", extra=[("Minus", env.num("SECTION_TRAILING_X"))]), groups, bottom="SPACE_FACT")


def layout_section(env, resizing=False):
    c = cols(env)
    sizing = [group(env, "Resizing", "Resizing", row(env, "Resizing row", [select(env, "W sizing", c[0], "W: Fill"), select(env, "H sizing", c[1], "H: Hug")]))] if resizing else []
    return section(env, "Layout", title(env, "Layout", "Layout"), [dimensions_group(env)] + sizing)


def appearance_section(env, clip=False):
    """`clip` adds the `Clip content` row of a frame."""
    c = cols(env)
    fields = row(env, "Appearance row", [field(env, "Opacity field", c[0], "", "100", "%", glyph="Opacity"), field(env, "Radius field", c[1], "", "0", glyph="Radius")])
    rows = [fields]
    if clip:
        box = X("Clip checkbox", "Checkbox/Box", "State=Default", abs=(0.0, 9.0, 14.0, 14.0))
        rows.append(row(env, "Clip row", [box, T("Clip label", "Clip content", "Body / Regular", env.c("TEXT"), abs=(22.0, 7.0, 120.0, 18.0))]))
    return section(env, "Appearance", title(env, "Appearance", "Appearance"), [group(env, "Appearance", None, *rows)])
