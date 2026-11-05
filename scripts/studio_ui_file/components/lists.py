"""Registry `component` List/Row and Tree/Row (row Lists & Trees), each drawn from the painter 0.7.5 calls (D5); the kit painters have 0 callers
(`kit::list_row`, widgets/kit/rows.rs:96-111; `kit::tree_row_bg`, rows.rs:91-94), so their looks are gaps.

* List/Row is one set drawn from two live painters by size (rule f), both coloured by `kit::row_look` (rows.rs:10-23; 6 callers: home/dialog.rs:346,
  home/mod.rs:1230 and 2829, panels/left.rs:335, libraries/review.rs:136, variables/view.rs:430).
  Sm = the Home sidebar row (home/mod.rs:1218-1246): 36 high, a 15 px glyph centred at 26 and the label at 42, `Text sm` Regular, Semibold when selected.
  The glyph sits in a 28 px slot (centre 14 in from `ix = left + 12`) and the label follows `INSET_SEGMENT` (2) later, so no half-pixel literal is needed.
  It has no ring and never asks for the Focused look, so Focus is the Default look. Its drop-hover `BORDER_SELECTED` stroke (home/mod.rs:1233) is no
  registry state: a gap.
  Xs = the pages-list row (panels/left.rs:311-352): `PAGE_ROW_HEIGHT` 32, no glyph, the label at `PAGE_ROW_TEXT_PAD`, `font_body_strong` when
  selected. It asks `row_look` for Selected, Hover or Default only (never Focused) and adds `kit::focus_ring` on `has_focus()` (left.rs:341), so Focus is
  the Default look plus a ring on the row.
* Tree/Row is the layers row (panels/layers.rs:485-700), look `kit::tree_row_look` (rows.rs:34-45). The fill is the row inset by `INSET_FRAME` on both
  sides, so the root is that pill and every offset below is from its left edge. A 24 px chevron box (the glyph is `Has Children`, swapped to
  ChevronDown by the caller when open), the 16 px kind glyph, 6 px, the name (`font_body`), the type label (caption, right-aligned: `text_right = row right - (2 * LAYER_TOGGLE_SIZE + 8)`,
  label at `text_right - 2`) and, at the right edge, the two 24 px toggle boxes (layers.rs:672-699). The boxes are always there and reserve their 48 px; Hover
  draws the 14 px eye and lock glyphs in them (`row_hot || ...`), the other states leave the boxes empty (no hidden glyph is kept: each one would be
    copied into every row of every screen). The label's `INSET_SEGMENT` right padding is the 2 px. A depth-d row adds `d * LAYER_INDENT` to its left
    padding:
  the variant `Depth=d` carries it: the root's `paddingLeft` is 0 at depth 0, the LAYER_INDENT variable at depth 1 and d * LAYER_INDENT as a recorded literal from depth 2 (the gate,
  studio_ui_gate_bound, holds every Depth=d variant to d * LAYER_INDENT and fails an instance that sets the padding itself); the fill is not indented, the contents are.
  Gaps: Selected under a pointer (`BG_SELECTED_HOVER`), a child of a selected row (`BG_SELECTED_SECONDARY`), the canvas-hover stroke (layers.rs:534-536),
  component rows (`TEXT_COMPONENT` glyph and name) and the hidden or locked glyphs.
"""
from rest_ir import F, I, T, X, lit

HOME_ROW_X = "home/mod.rs:1235: `ix = r.left() + 12.0` (+ indent): a literal in the Rust, no constant"
DEPTH_INDENT = "panels/layers.rs:538 `indent = row.depth as f32 * theme::LAYER_INDENT`: a product of a constant and the row's depth, no variable holds it"
DEPTHS = ("0", "1", "2", "3", "4", "5")
ICON_TO_NAME = "panels/layers.rs:584: text_left = glyph centre + LAYER_ICON_SIZE / 2 + 6; the literal 6 is the only gap between the glyph and the name"


def ring_on(root, has_ring):
    """The Focus ring instance of a host; the layout pass grows it to the host's size + 8."""
    if has_ring:
        root["kids"].append(X("Focus ring", "Focus/Ring", "Tone=Brand", abs=(-4, -4, root["w"] + 8, root["h"] + 8)))
    return root


def list_row(values, env):
    size, state = values["Size"], values["State"]
    look = env.look("List/Row", "Default" if state == "Focus" else state)
    selected = state == "Selected"
    if size == "Sm":
        style = "Text sm / Semibold" if selected else "Text sm / Regular"
        slot = F("Glyph slot", [I("Icon", "Clock", 15, look["icon"], swap="Icon")], dir="H", w=28, h=env.num("CONTROL_SM"), sx="fixed", sy="fixed")
        return F("Row", [slot, T("Label", "Recents", style, look["text"], prop="Label")], dir="H", gap=env.c("INSET_SEGMENT"), pad=[None, None, None, lit(12, HOME_ROW_X)],
                 fill=look["fill"], radius=env.c("RADIUS_CONTROL"), w=247, h=env.num("CONTROL_SM"), sx="fixed", sy="fixed", align=("start", "center"))
    style = "Body / Semibold" if selected else "Body / Regular"
    root = F("Row", [T("Label", "Getting started", style, look["text"], prop="Label")], dir="H", pad=[None, None, None, env.c("PAGE_ROW_TEXT_PAD")], fill=look["fill"],
             radius=env.c("RADIUS_CONTROL"), w=224, h=env.num("PAGE_ROW_HEIGHT"), sx="fixed", sy="fixed", align=("start", "center"))
    return ring_on(root, state == "Focus")


def depth_indent(depth, env):
    """The root's left padding of a row `depth` levels down: none at 0, the LAYER_INDENT variable at 1, a recorded `depth * LAYER_INDENT` literal beyond."""
    if depth == 0:
        return None
    return env.c("LAYER_INDENT") if depth == 1 else lit(depth * env.num("LAYER_INDENT"), DEPTH_INDENT)


def type_label(depth):
    """The default type label as production draws it: `split_row_text` (panels/layers.rs:235-241) keeps it only when the text column holds the label
      (64 px) plus its 8 px padding and a 16 px minimum name,
    that is while 130 - 16 * depth >= 88, so depth 3 and deeper draw none; a row of the Depth=5 variant then fits its 224 px box."""
    return "Auto layout" if depth <= 2 else ""


def tree_row(values, env):
    state, depth = values["State"], int(values["Depth"])
    look = env.look("Tree/Row", state)
    box = env.num("LAYER_TOGGLE_SIZE")
    chevron = F("Chevron box", [I("Chevron", "ChevronRight", 12, look["icon"], swap="Chevron", visible_prop="Has Children")], dir="H", w=env.num("LAYER_CHEVRON_BOX"),
                h=env.num("LAYER_CHEVRON_BOX"), sx="fixed", sy="fixed")
    kind = F("Icon slot", [I("Icon", "AutoLayoutH", 16, look["icon"], swap="Icon")], dir="H", pad=[None, lit(6, ICON_TO_NAME), None, None])
    kind_label = F("Type slot", [T("Type label", type_label(depth), "Caption / Regular", env.c("TEXT_TERTIARY"), prop="Type label")], dir="H",
                   pad=[None, env.c("INSET_SEGMENT"), None, None])
    shown = state == "Hover"
    toggle = lambda name, icon: F(f"{name} box", [I(name, icon, 14, look["icon"])] if shown else [], dir="H", w=box, h=box, sx="fixed", sy="fixed")
    toggles = F("Toggles", [toggle("Lock toggle", "Unlock"), toggle("Eye toggle", "Eye")], dir="H", w=2 * box, h=box, sx="fixed", sy="fixed")
    kids = [chevron, kind, T("Name", "Desktop", "Body / Regular", look["text"], prop="Name", sx="fill"), kind_label, toggles]
    return F("Row", kids, dir="H", pad=[None, None, None, depth_indent(depth, env)], fill=look["fill"], radius=env.c("RADIUS_CONTROL"), w=224, h=env.num("LAYER_ROW_HEIGHT"), sx="fixed",
             sy="fixed", align=("start", "center"))


def component_sets():
    return [
        {"name": "List/Row", "row": "Lists & Trees", "axes": {"Size": ["Xs", "Sm"], "State": ["Default", "Selected", "Hover", "Focus"]}, "uses": ["Focus/Ring"],
         "props": [{"name": "Label", "type": "TEXT", "default": "Recents"}, {"name": "Icon", "type": "INSTANCE_SWAP", "default": "Clock"}], "build": list_row},
        {"name": "Tree/Row", "row": "Lists & Trees", "axes": {"Size": ["Xs"], "State": ["Default", "Selected", "Hover"], "Depth": list(DEPTHS)},
         "props": [{"name": "Name", "type": "TEXT", "default": "Desktop"}, {"name": "Type label", "type": "TEXT", "default": "Auto layout"},
                   {"name": "Has Children", "type": "BOOLEAN", "default": True}, {"name": "Icon", "type": "INSTANCE_SWAP", "default": "AutoLayoutH"},
                   {"name": "Chevron", "type": "INSTANCE_SWAP", "default": "ChevronRight"}], "build": tree_row},
    ]
