"""Panel/DevInspect `State=Annotate` and `State=Component`: the other two captured bodies of the right panel in Dev mode (lines are the v0.7.5 tag; origin (1199, 113), 241 x 751, clipped).

* Annotate (capture `g-dev-annotate`): the Inspect body with the annotation editor (`annotation_editor`, dev/sections.rs:189-240) inserted under the property rows at y 243: a 28 px text field
  (`text_input_ph`, BORDER_STRONG outline), the `Pin properties` caption (SPACE_GROUP above the chips: `c.y += 36`, label centre `+ 8`), the wrapped chips (`chips`, :150-186: 22 high pills of
  text width + 16, 4 apart and 26 per row; the active ones BG_SELECTED with a BORDER_SELECTED line, the others BG_SECONDARY), then the `Save` pill and
  the `Cancel` small button (`third` = (w - 16) / 3
  wide, 8 apart) and `c.y += 40`: every part after the annotations moves down 254.
* Component (capture `micro-dev-boxmodel`): an instance is selected, so `body` (panels/inspect.rs:338-640) draws the kind `Instance`, the `Ready for dev` call to action (`primary_button`, a pill)
  where a status is not set, `Component properties` (`component_section`, dev/sections.rs:137-148: `kv` rows), `Component playground` (dev/playground.rs:451 `playground_section`:
  a `Reset` small button, the read-only preview box
  and one select per property), Annotations and Dev links with their empty lines, then MCP and Code Connect 162 further down.
Gaps: the preview image is the document's own pixels (not drawn), the select's popup, the chip hover fill, and the Delete button of an existing annotation.
Chip widths are the capture's (text width + 16 in egui's Inter metrics). Annotation text and the property names are neutral.
"""
from rest_ir import F, T, lit
from components.regions_home_dev_parts import PILL, button, code_connect, glyph, header, mcp, text, titled
from components.regions_home_parts import rule

CHIPS = ((("Width", 49, True), ("Height", 53, False), ("Min width", 72, False)), (("Max width", 75, False), ("Min height", 75, False)),
         (("Max height", 79, False), ("Fill", 32, True), ("Stroke", 52, False)), (("Effects", 55, False), ("Stroke weight", 94, False)),
         (("Corner radius", 94, False), ("Gap", 39, False), ("Padding", 62, False)), (("Opacity", 60, False),))
SHIFT = 254.0


def chip(env, label, w, active, x, y):
    paint = {"stroke": env.c("BORDER_SELECTED"), "sw": env.c("BORDER_WIDTH")} if active else {}
    return F(f"Chip {label}", [T("Label", label, "Caption / Regular", env.c("TEXT"))], dir="H", fill=env.c("BG_SELECTED" if active else "BG_SECONDARY"), radius=lit(11, PILL),
             abs=(x, y, w, 22), w=w, h=22, sx="fixed", sy="fixed", align=("center", "center"), **paint)


def shifted(nodes, dy):
    """The same parts `dy` further down: each is placed absolutely, so its box moves."""
    out = []
    for n in nodes:
        x, y, w, h = n["abs"]
        out.append(dict(n, abs=(x, y + dy, w, h)))
    return out


def editor(env):
    field = F("Annotation field", [T("Text", "Note for the layer", "Body / Regular", env.c("TEXT"), abs=(8, 5, 150, 18))], fill=env.c("BG"), stroke=env.c("BORDER_STRONG"), sw=env.c("BORDER_WIDTH"),
              radius=env.c("RADIUS_CONTROL"), abs=(16, 243, 209, 28), w=209, h=28, sx="fixed", sy="fixed")
    kids = [field, text(env, "Pin properties", "Pin properties", "Caption / Regular", "TEXT_SECONDARY", 16, 287, 90)]
    for row, chips in enumerate(CHIPS):
        x = 16.0
        for label, w, active in chips:
            kids.append(chip(env, label, w, active, x, 299 + 26 * row))
            x += w + 4
    third = (209 - 16) / 3
    kids += [button(env, "Save", "Save", 16, 457, third, 28, fill="BG_BRAND", radius=lit(14, PILL), style="Body / Semibold", color="TEXT_ON_BRAND"),
             button(env, "Cancel", "Cancel", 16 + third + 8, 457, third, 28)]
    return kids


def kv(env, label, value, cy):
    return [text(env, f"{label} label", label, "Body / Regular", "TEXT_SECONDARY", 16, cy, 90), text(env, f"{label} value", value, "Body / Regular", "TEXT", 116, cy, 100)]


def component_header(env):
    """The node name row of `header` with the kind `Instance` (a diamond) and the `Ready for dev` call to action (inspect.rs:51-114 with no status)."""
    kids = [k for k in header(env) if k["name"] not in ("Kind icon", "Kind")]
    kids += [glyph(env, "Kind icon", "Instance", 12, 22, 45), text(env, "Kind", "Instance", "Body / Regular", "TEXT_SECONDARY", 34, 45, 60),
             button(env, "Ready for dev", "Ready for dev", 16, 62, 209, 28, fill="BG_BRAND", radius=lit(14, PILL), style="Body / Semibold", color="TEXT_ON_BRAND")]
    return kids


def playground(env):
    """`playground_section`: the title row with `Reset`, the read-only preview box, one select (a 117 x 24 outlined field with a chevron)."""
    box = F("Playground preview", [text(env, "Read only", "Read-only", "Caption / Regular", "TEXT_SECONDARY", 8, 12, 80)], fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"),
            abs=(16, 220, 209, 96), w=209, h=96, sx="fixed", sy="fixed")
    select = F("Playground select", [T("Value", "False", "Body / Regular", env.c("TEXT"), sx="fill"), glyph(env, "Chevron", "ChevronDown", 12, 100, 12, "ICON_TERTIARY")], dir="H",
               stroke=env.c("BORDER_STRONG"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), pad=[None, None, None, env.c("INSET_CONTROL")], abs=(108, 326, 117, 24), w=117, h=24,
               sx="fixed", sy="fixed", align=("start", "center"))
    return titled(env, "Component playground", 187, "Reset", 196) + [box, text(env, "Playground label", "Resources", "Body / Regular", "TEXT", 16, 339, 80), select]


def component_body(env):
    kids = [*component_header(env), rule("Rule Component properties", 0, 98, 241), text(env, "Component properties", "Component properties", "Body / Semibold", "TEXT", 16, 119, 150)]
    kids += kv(env, "Component", "Resources=False", 143) + kv(env, "Resources", "False", 167) + playground(env)
    kids += titled(env, "Annotations", 360, "Add", 370) + [text(env, "No annotations", "No annotations", "Body / Regular", "TEXT_TERTIARY", 16, 405, 120)]
    kids += titled(env, "Dev links", 425, "Add", 435) + [text(env, "No links", "No links", "Body / Regular", "TEXT_TERTIARY", 16, 470, 120)]
    return kids + shifted(mcp(env), 162.0) + shifted(code_connect(env), 162.0)
