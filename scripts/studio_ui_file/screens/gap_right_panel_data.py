"""Per-screen data of Panel/RightInspector (page `04 Screens`): which State a capture's right panel is, and the values it shows. Plain dicts, no `screens()`:
a screen places `Panel/RightInspector` with `variant` and `overrides` taken from `RIGHT_PANEL[state]`.

    UNDRAWN = {"micro-vector-edit": "the capture's inspector opens with the Vertex section (3 vertices, corner radius, stroke cap, handles, Join / Split / Simplify), which no variant holds"}
RIGHT_PANEL = {capture state: {"variant": {"State": ...}, "overrides": [{"path": ..., "text": ... | "visible": False | "swap": "Icon/..."}]}}

It covers every state of `RIGHT_46` (`gap_table_citations.py`) and the Dev states (g-dev-*, micro-dev-boxmodel). A path is the node names below the instance, joined by `/`; each is checked against
the IR of its variant (`scripts/tests/test_studio_ui_gap_b1_data.py`). Values are the numbers the capture's panel shows (zoom, position, size, gap, padding, opacity, radius, hex, count);
the layer names
are neutral ("Frame 1", "3 layers": the capture's names are the file's own strings) and every one of them sits in the capture at NAME_RECT, which the screen gate's TEXTS masks cover.
What data cannot say: a strip's lit cell, a swatch colour, a title colour, the pill look of a variable-bound field (only its glyph and text) and a section the variant does not hold; each such
state keeps the variant's default there, and the report lists the pixels.
"""
NAME_RECT = (1207, 125, 136, 32)  # capture px: the name field of a selected layer (`RIGHT_NAME_FIELD_X` - 1, the header row 12 below the content top)
ZOOM = "Zoom/Zoom value"
NAME = "Content/Name row/Name"
POSITION = "Content/Position section/Position body/"
AL = "Content/Auto layout section/Auto layout body/"
LAYOUT = "Content/Layout section/Layout body/"
APPEARANCE = "Content/Appearance section/Appearance body/Appearance group/Appearance row/"
TYPOGRAPHY = "Content/Typography section/Typography body/Typography rows/"
FILL = "Content/Fill section/Fill body/Fill rows/Fill 1 row/Fill 1 group/"
COLOURS = "Content/Selection colors section/Selection colors body/Colour rows/"
PAINT_SECTIONS = ("Fill", "Stroke", "Effects", "Selection colors")
VARIABLE = "Icon/Variables"  # the glyph of a field bound to a variable (`var_bind::pill_field`)


def text(path, value):
    return {"path": path, "text": value}


def value(path, number):
    return text(path + "/Value", number)


def common(name, zoom):
    return [text(ZOOM, zoom), text(NAME, name)]


def position(x, y):
    return [value(POSITION + "Position group/Position row/X field", x), value(POSITION + "Position group/Position row/Y field", y)]


def size(base, w, h):
    return [value(base + "Dimensions group/Dimensions row/W field", w), value(base + "Dimensions group/Dimensions row/H field", h)]


def frame(name, zoom, xy, wh, gap, sizing, padding, bound=()):
    """An auto-layout frame: gap, W / H resizing and padding (`bound`: the fields that show a variable, "gap" | "padding-h" | "padding-v", glyph swapped and text kept)."""
    base, out = AL, common(name, zoom) + position(*xy) + size(AL, *wh)
    out += [value(base + "Flow group/Flow row/Gap field", gap[0]), text(base + "Resizing group/Resizing row/W sizing/Label",
            sizing[0]), text(base + "Resizing group/Resizing row/H sizing/Label", sizing[1])]
    out += [value(base + "Padding group/Padding row/Horizontal padding", padding[0]), value(base + "Padding group/Padding row/Vertical padding", padding[1])]
    glyphs = {"gap": "Flow group/Flow row/Gap field/Glyph", "padding-h": "Padding group/Padding row/Horizontal padding/Glyph",
              "padding-v": "Padding group/Padding row/Vertical padding/Glyph"}  # the glyph is a child of the field, beside its value
    return out + [{"path": base + glyphs[b], "swap": VARIABLE} for b in bound]


def shown(paths, flag):
    return [{"path": p, "visible": flag} for p in paths]


def paint_sections(*names):
    """Each section and the rule above it."""
    return [p for n in names for p in (f"Content/Rule above {n} section", f"Content/{n} section")]


def layers(name, zoom, xy, wh, appearance, fill=None, colours=(), hide=(), show=()):
    """A selection of layers (or one shape or frame): position, dimensions, opacity and radius, the fill hex and the selection-colour rows (hex, count text); `hide` / `show` name the paths whose
    `visible` the capture flips."""
    out = common(name, zoom) + position(*xy) + size(LAYOUT, *wh)
    out += [value(APPEARANCE + "Opacity field", appearance[0]), value(APPEARANCE + "Radius field", appearance[1])]
    out += [text(FILL + "Hex", fill)] if fill else []
    for n, (hex_text, count) in enumerate(colours, 1):
        out += [value(COLOURS + f"Colour {n} row/Hex field", hex_text), text(COLOURS + f"Colour {n} row/Opacity badge/Count", count)]
    return out + shown(hide, False) + shown(show, True)


def frame_without_auto_layout(name, zoom, xy, wh, appearance):
    """A frame with auto layout off (or a selection of frames), drawn as the Layers state: the paint sections are not in the capture."""
    return layers(name, zoom, xy, wh, appearance, hide=paint_sections(*PAINT_SECTIONS))


def text_layer(zoom, wh, style=("Heading 1/Regular", "Regular", "72", "90", "-1.44")):
    base = "Content/Layout section/Layout body/"
    t = TYPOGRAPHY
    out = common("Text 1", zoom) + size(base, *wh)
    return out + [text(t + "Style row/Style field/Style name", style[0]), text(t + "Weight row/Weight/Label", style[1]), value(t + "Weight row/Size field", style[2]),
                  value(t + "Spacing row/Line height field", style[3]), value(t + "Spacing row/Letter spacing field", style[4])]


def entry(state, overrides):
    return {"variant": {"State": state}, "overrides": overrides}


def dev(zoom):
    return entry("Dev", [text(ZOOM, zoom)])


CTA = dict(name="Frame 1", xy=("0", "0"), wh=("343", "78"), gap=("12",), sizing=("W: Fill", "H: Hug"), padding=("16", "16"))
DESKTOP = dict(name="Frame 2", xy=("37502", "620"), wh=("1440", "960"), gap=("0",), sizing=("W: Fixed", "H: Fixed"), padding=("0", "0"))
CONTAINER = dict(name="Frame 3", xy=("0", "0"), wh=("375", "361"), gap=("24",), sizing=("W: Fill", "H: Hug"), padding=("cont...", "0"), bound=("padding-h",))
AVATARS = dict(name="Frame 4", xy=("5680", "730"), wh=("4944", "13148"), gap=("spac...",), sizing=("W: Fixed", "H: Hug"), padding=("spac...", "spac..."), bound=("gap", "padding-h", "padding-v"))
AUTO = dict(name="Frame 5", xy=("2400", "200"), wh=("444", "180"), gap=("20",), sizing=("W: Hug", "H: Hug"), padding=("28", "28"))
RED, BLUE = "E36323", "0D99FF"


def frame_state(zoom="2%", **spec):
    return entry("Frame", frame(zoom=zoom, **spec))


def layer_state(zoom, **spec):
    return entry("Layers", layers(zoom=zoom, **spec))


UNDRAWN = {"micro-vector-edit": "the capture's inspector opens with the Vertex section (3 vertices, corner radius, stroke cap, handles, Join / Split / Simplify), which no variant holds"}
RIGHT_PANEL = {
    "d-editor-selected": frame_state(**CTA), "fill-d9": frame_state(**CTA), "f-picker-gradient": frame_state(**CTA), "f-edit-style-color": frame_state(**CTA),
    "edit-style-effect": frame_state(**CTA), "edit-style-grid": frame_state(**CTA), "f-stroke-settings": frame_state(**CTA), "effect-noise": frame_state(**CTA),
    "effect-glass": frame_state(**CTA), "effect-texture": frame_state(**CTA), "ai-image-menu": frame_state(**CTA), "ai-image-erase": frame_state(**CTA),
    "ai-image-expand": frame_state(**CTA), "j-check-designs": frame_state(**CTA), "micro-layer-picker": frame_state("105%", **CTA),
    "k-assets": frame_state(**DESKTOP), "i-agents": frame_state(**DESKTOP), "i-agents-picker": frame_state(**DESKTOP), "i-terminal": frame_state(**DESKTOP),
    "i-agents-chat": frame_state(**DESKTOP), "i-agents-start-failure": frame_state(**DESKTOP), "i-agent-permissions": frame_state(**DESKTOP),
    "i-agents-approval-command": frame_state(**DESKTOP), "agents-approval-open-tab": frame_state(**DESKTOP), "agents-approval-queued": frame_state(**DESKTOP),
    "agents-skills": frame_state(**DESKTOP), "f-picker": frame_state(**DESKTOP), "j-cmdk": frame_state(**DESKTOP), "k-variables-floating": frame_state(**DESKTOP),
    "micro-components-variables": frame_state("115%", **DESKTOP), "e-layers-selected": frame_state(**CONTAINER), "kit-agents": frame_state("100%", **AVATARS),
    "micro-autolayout": frame_state("145%", **AUTO),
    "micro-smart-selection": layer_state("130%", name="3 layers", xy=("Mixed", "1100"), wh=("148", "190"), appearance=("100%", "14"), fill="27272A", colours=(("27272A", "100 %"), (RED, "100 %"))),
    "micro-selection-corner": layer_state("132%", name="3 layers", xy=("Mixed", "2840"), wh=("144", "180"), appearance=("100%", "Mixed"), fill=BLUE, colours=((BLUE, "100 %"), (RED, "100 %"))),
    "micro-typography-multiedit": layer_state("125%", name="3 layers", xy=("18", "20"), wh=("112", "42"), appearance=("100%", "10"), fill=BLUE, colours=((BLUE, "100% · 3"),),
                                               hide=("Content/Selection colors section/Selection colors body/Colour rows/Colour 2 row",)),
    "micro-shape-controls": layer_state("150%", name="Shape 1", xy=("2420", "1450"), wh=("220", "220"), appearance=("100%", "0"), fill=BLUE, hide=paint_sections("Selection colors")),
    "micro-scale-tool": entry("Layers", frame_without_auto_layout("Frame 6", "145%", ("2420", "2200"), ("320", "200"), ("100%", "17.5"))),
    "micro-inspector-math": entry("Layers", frame_without_auto_layout("Frame 7", "138%", ("2420", "2520"), ("420", "216"), ("100%", "24"))),
    "micro-css-grid": entry("Layers", frame_without_auto_layout("2 layers", "130%", ("Mixed", "Mixed"), ("Mixed", "Mixed"), ("100%", "Mixed"))),
    "edit-style-text": entry("Text", text_layer("2%", ("311", "90"))),
    "f-type-details": entry("Text", text_layer("2%", ("311", "44"), ("Heading 4/Semibold", "Semi Bold", "36", "44", "-0.72"))),
    "d-menu-zoom": entry("Page", []),
    "kit-editor": entry("Instance", common("Component 1", "100%")),
    "f-prototype-tab": entry("Prototype", [text(ZOOM, "19%")]),
    "g-dev-react": dev("2%"), "g-dev-annotate": dev("2%"), "g-dev-compare": dev("2%"), "micro-dev-boxmodel": dev("140%"),
}

# What to wire first when the size budget does not hold them all (every state costs its variant's nodes over the Page state's in each screen that places it, measured in the report):
# 1. the states that make the file smaller (the Page state's style rows weigh more than the Dev and Prototype content);
# 2. the states that free a duplicate frame (each one's capture draws its own state);
# 3. the states that only draw their capture more closely (their panel was the Page state, or a duplicate of another screen's selected frame).
SMALLER_FILE = ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel", "f-prototype-tab")
FREES_A_DUPLICATE = ("d-editor-selected", "kit-editor", "e-layers-selected", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-shape-controls", "micro-autolayout",
                     "micro-css-grid", "micro-inspector-math", "micro-typography-multiedit", "micro-components-variables", "kit-agents")
WIRING_TIERS = (
    ("smaller file", SMALLER_FILE),
    ("frees a duplicate", FREES_A_DUPLICATE),
    ("closer capture", tuple(s for s in RIGHT_PANEL if s not in SMALLER_FILE + FREES_A_DUPLICATE)),
)
