"""The content of the selection states of Panel/RightInspector, composed from the shared section builders (`regions_editor_right_sections`, `_paints`, `_text_instance`, `_proto`). Lines are
the 0.7.5 tag.

Each state draws what its representative capture shows below the hairline at y 80, down to the first section whose title is cut by the footer (the rest of `selection_ui` scrolls under it):

| State | Capture | Painter state | Sections (top to bottom) |
|---|---|---|---|
| Frame | d-editor-selected | one frame with auto layout (properties.rs:823-1076) | name, Copy to Slide Deck, Position, Auto layout, Appearance |
| Layers | micro-smart-selection | several layers (a shape selection is the same sections) | name, Position, Layout, Appearance, Fill, Stroke, Effects, Selection colors |
| Text | edit-style-text | one text layer | name, Position, Layout (Dimensions, Resizing), Typography, Appearance |
| Instance | kit-editor | one component instance | name, Copy to Slide Deck, Properties, Position |
| Prototype | f-prototype-tab | the Prototype tab (`prototype::show`) | Interactions, Scroll behavior |
| Dev | g-dev-react | Dev mode tabs | none: Panel/DevInspect covers the content area |

The sections are auto-layout children of the `Content` frame (241 x 751 at (0, 81), clipped at the footer, 12 under the top: `RIGHT_SELECTION_HEADER_Y` - `RIGHT_CONTENT_TOP`,
properties.rs:833), a hairline
between two (`section_sep`). A screen hides a section by overriding its `visible` and sets values by overriding the texts it names. Nothing is hidden by default: every part a variant holds is
drawn in its capture, because a part a screen never shows would still be stored in every instance (size budget).

Not drawn, for that reason: the Copy row, the collapsed Auto layout, the Clip content row and the variable-mode rows of a frame without auto layout (micro-scale-tool, micro-inspector-math: the
Layers state is drawn with their values and everything below the name row sits 32 px high, 73 px below the Position section); micro-css-grid has no Copy row and no Auto layout in its capture and
lacks only the Clip row and the mode rows. Also not drawn: the Vertex section (micro-vector-edit) and the Styles list of a shape (micro-shape-controls).
"""
from components.regions_editor_right_paints import effects_section, fill_section, selection_colors_section, stroke_section
from components.regions_editor_right_parts import rule
from components.regions_editor_right_proto import prototype_content
from components.regions_editor_right_text_instance import properties_section, typography_section
from components.regions_editor_right_sections import appearance_section, auto_layout_section, copy_row, layout_section, name_row, position_section
from rest_ir import F

# From the panel's outer left edge, the border column included: every x of `properties.rs` is relative to it (`Pane::x0` = region.left - BORDER_WIDTH, :383); y 81 down to the footer (832).
CONTENT = (0.0, 81.0, 241.0, 751.0)


def stack(env, head, sections):
    """The name rows, then the sections with a hairline above each but the first."""
    out = list(head)
    for i, s in enumerate(sections):
        out += ([rule(env, f"Rule above {s['name']}")] if i or head else []) + [s]
    return out


def frame(env):
    return stack(env, [name_row(env, "Frame 1"), copy_row(env)], [position_section(env), auto_layout_section(env), appearance_section(env, clip=True)])


def layers(env):
    return stack(env, [name_row(env, "3 layers")], [position_section(env), layout_section(env), appearance_section(env), fill_section(env), stroke_section(env), effects_section(env),
                                                    selection_colors_section(env)])


def text(env):
    return stack(env, [name_row(env, "Text 1")], [position_section(env), layout_section(env, resizing=True), typography_section(env), appearance_section(env)])


def instance(env):
    return stack(env, [name_row(env, "Component 1", component=True), copy_row(env)], [properties_section(env), position_section(env)])


def dev(env):
    return []


BUILDERS = {"Frame": frame, "Layers": layers, "Text": text, "Instance": instance, "Prototype": prototype_content, "Dev": dev}


def content(env, state):
    x, y, w, h = CONTENT
    return F("Content", BUILDERS[state](env), dir="V", pad=[env.c("SPACE_GROUP") if state != "Prototype" else None,
            None, None, None], w=w, h=h, sx="fixed", sy="fixed", clip=True, align=("start", "start"), abs=(x, y, w, h))
