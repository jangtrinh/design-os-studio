"""The two sections only the Text and Instance states draw: Typography (`panels/typography.rs:160` `section`) and component Properties (`panels/components.rs:378` `component_props`). Lines
are the 0.7.5 tag.

* Typography: the title row with three actions, the styles button at `SECTION_TRAILING_X` - 54, the type-settings button at - 26 (typography.rs:163-175, :179-194) and the `...` more button at
  `SECTION_TRAILING_X` (drawn as the text `…`, :185); then the linked style row (160 x 32 with a 12 px styles glyph, the edit button at 181 and the detach minus at 209, the row
  advances 36), the font
  family (a 184 wide dropdown, :201-204), the weight dropdown and the size field (`S`), the line height (`↕`) and letter spacing (`↔`) fields, and the alignment strip with the two text
  cells `•` `1.`
  at the second column (:206-255); rows are 4 apart (`row_end`) and the section ends with 4 (properties.rs:1003).
* Properties: the title with Plus, then one 76 px block per definition (components.rs:398-461): the name field 156 wide with a minus at `SECTION_TRAILING_X` (a variant property has no minus,
  :404), 4 below it the kind label (`font_body`, TEXT_SECONDARY) at the first column and the default control at the second: a 28 x 16 switch (`toggle` :69-86: BG_BRAND, a 12 px white knob at
  its right end, no record: Toggle/Switch is the 36 x 20 kit track) or a dropdown for a variant; 8 under the second row.
"""
from components.regions_editor_right_parts import cols, field, icon_button, plain_field, row, section, select, strip, title
from rest_ir import E, F, I, T, X

SEC_W = 239.0


def typography_section(env):
    c, h = cols(env), env.num("FIELD_HEIGHT")
    edge = env.num("SECTION_TRAILING_X")
    more = T("More", "…", "Body / Semibold", env.c("ICON_SECONDARY"), abs=(edge + 2.0, 11.0, 18.0, 18.0))
    head = title(env, "Typography", "Typography", extra=[("Styles", edge - 54.0), ("StrokeSettings", edge - 26.0)])
    head["kids"].append(more)
    style_name = F("Style field", [I("Styles glyph", "Styles", 12, env.c("ICON_TERTIARY"), abs=(8.0, 10.0, 12, 12)),
            T("Style name", "Heading 1/Regular", "Body / Regular", env.c("TEXT"), abs=(28.0, 7.0, 120.0, 18.0))],
                   stroke=env.c("BORDER_STRONG"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), w=160.0, h=h, sx="fixed", sy="fixed", abs=(0.0, 0.0, 160.0, h))
    linked = row(env, "Style row", [style_name, icon_button(env, "Edit style", "StrokeSettings", 164.0, 4.0), icon_button(env,
            "Detach style", "Minus", edge - env.num("SECTION_CONTENT_X"), 4.0)], h=36.0)
    family = row(env, "Family row", [select(env, "Font family", 0.0, "Inter")])
    family["kids"][0].update(abs=(0.0, 0.0, env.num("SECTION_CONTENT_W"), h), w=env.num("SECTION_CONTENT_W"))
    weight = row(env, "Weight row", [select(env, "Weight", c[0], "Regular"), field(env, "Size field", c[1], "S", "72")])
    spacing = row(env, "Spacing row", [field(env, "Line height field", c[0], "↕", "90"), field(env, "Letter spacing field", c[1], "↔", "-1.44")])
    align = row(env, "Align row", [strip(env, "Text align", ("TextLeft", "TextCenter", "TextRight"), 0.0, (0,)), strip(env, "List", ("txt:•", "txt:1."), c[1])])
    rows = F("Typography rows", [linked, family, weight, spacing, align], dir="V", gap=env.c("SPACE_FACT"), pad=[None, None, None, env.c("SECTION_CONTENT_X")], w=SEC_W, sx="fixed", sy="hug",
             align=("start", "start"))
    return section(env, "Typography", head, [rows])


def switch(env, name, x):
    """The 28 x 16 `toggle` (components.rs:69-86), on: BG_BRAND at `RADIUS_CONTROL`, the 12 px knob 8 from the right edge, centred."""
    knob = E("Knob", 12.0, 12.0, fill=env.c("TEXT_WHITE"), sx="fixed", sy="fixed", abs=(14.0, 2.0, 12, 12))
    return F(name, [knob], fill=env.c("BG_BRAND"), radius=env.c("RADIUS_CONTROL"), w=28.0, h=16.0, sx="fixed", sy="fixed", abs=(x, env.num("SPACE_INLINE"), 28.0, 16.0))


def property_block(env, n, kind):
    c, h = cols(env), env.num("FIELD_HEIGHT")
    name = plain_field(env, "Name field", 0.0, f"Property {n}", 156.0)
    first = [name] + ([icon_button(env, "Delete", "Minus", env.num("SECTION_TRAILING_X") - env.num("SECTION_CONTENT_X"))] if kind == "Boolean" else [])
    label = T("Kind", kind, "Body / Regular", env.c("TEXT_SECONDARY"), abs=(c[0], 7.0, 80.0, 18.0))
    control = switch(env, "Default switch", c[1]) if kind == "Boolean" else select(env, "Default value", c[1], "")
    block = F(f"Property {n}", [row(env, "Name row", first), row(env, "Default row", [label, control])], dir="V", gap=env.c("SPACE_FACT"), pad=[None, None, env.c("SPACE_INLINE"), None],
              w=SEC_W - env.num("SECTION_CONTENT_X"), sx="fixed", sy="hug", align=("start", "start"))
    return block


KINDS = ("Boolean", "Boolean", "Variant", "Boolean", "Boolean", "Variant", "Boolean")  # kit-editor's seven definitions, by kind


def properties_section(env, kinds=KINDS):
    rows = F("Property rows", [property_block(env, n, k) for n, k in enumerate(kinds, 1)], dir="V", pad=[None, None, None, env.c("SECTION_CONTENT_X")], w=SEC_W, sx="fixed", sy="hug",
             align=("start", "start"))
    return section(env, "Properties", title(env, "Properties", "Properties", plus=True), [rows], bottom=None)
