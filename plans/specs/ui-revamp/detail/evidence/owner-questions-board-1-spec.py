"""Owner-questions board: every open owner question of specs A, B and C with its options side by side,
Untitled UI dark at R5 editor density, Studio orange brand. One NodeSpec tree for `designos --apply-spec`.

Usage:
  python3 owner-questions-board-spec.py board.json
  designos <scratch>.designos --apply-spec board.json        # then designos_render_png node 2, scale 2

Colours are Untitled dark values (specs/token-contract.md); contrast ratios are computed here (WCAG 2.x), not typed.
"""
import json, sys

T = {  # Untitled UI PRO v8.0 dark, resolved (token-contract.md section 0)
    "bg-primary": "#0A0A0A", "bg-secondary": "#171717", "bg-tertiary": "#262626", "bg-quaternary": "#404040",
    "border-primary": "#404040", "border-secondary": "#262626",
    "text-primary": "#FAFAFA", "text-secondary": "#D4D4D4", "text-tertiary": "#A3A3A3", "text-white": "#FFFFFF",
    "fg-quaternary": "#737373", "brand": "#E36323", "orange-500": "#F97316", "dark-label": "#0A0A0A",
    "red-500": "#EF4444", "red-600": "#DC2626", "red-700": "#B91C1C",
}
CARD_W, GAP, PAD = 752, 16, 20


def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def cr(a, b):
    hi, lo = sorted((lum(a), lum(b)), reverse=True)
    return f"{(hi + 0.05) / (lo + 0.05):.2f}:1"


def text(s, size=12, color="text-secondary", weight=400, name=None):
    return {"kind": "text", "name": name or s[:28], "text": s, "font_size": size, "font_weight": weight,
            "text_color": T.get(color, color)}


def frame(name, children=(), direction="row", gap=0, pad=(0, 0, 0, 0), fill=None, stroke=None, sw=1, radius=None,
          w=None, h=None, align="center", main="start"):
    n = {"kind": "frame", "name": name, "children": list(children), "fill": T.get(fill, fill) if fill else "transparent",
         "layout": {"direction": direction, "gap": gap, "padding": list(pad), "primary_align": main, "counter_align": align}}
    if stroke:
        n["stroke"], n["stroke_weight"] = T.get(stroke, stroke), sw
    if radius is not None:
        n["corner_radius"] = radius
    n["width_sizing"], n["height_sizing"] = ("fixed" if w else "hug"), ("fixed" if h else "hug")
    if w:
        n["width"] = w
    if h:
        n["height"] = h
    return n


def option(letter, value, rec, content, note, w):
    label = f"{letter} · {value}" + ("  (rec)" if rec else "")
    return frame(f"option {letter}", [content, text(label, 12, "orange-500" if rec else "text-primary", 600, name=f"label {letter}"),
                                      text(note, 12, "text-tertiary", 400, name=f"note {letter}")],
                 direction="column", gap=6, align="start", w=w)


def card(qid, title, ask, options):
    n = len(options)
    cell = (CARD_W - 2 * PAD - (n - 1) * GAP) // n
    opts = [option(*o[:5], w=cell) for o in options]
    head = frame("head", [text(f"{qid}  {title}", 14, "text-primary", 600), text(ask, 12, "text-tertiary")],
                 direction="column", gap=2, align="start")
    return frame(qid, [head, frame("options", opts, gap=GAP, align="start")], direction="column", gap=14,
                 pad=(PAD, PAD, PAD, PAD), fill="bg-primary", stroke="border-secondary", radius=12, w=CARD_W, align="start")


# ------------------------------------------------------------------ Q1 placeholder
def search_field(ph, surface, w=300):
    stroke = "border-primary" if surface == "bg-primary" else None
    return frame("search field", [text("⌕", 13, "fg-quaternary", 500), text("Search layers", 12, ph)], gap=8, pad=(0, 10, 0, 10),
                 fill=surface, stroke=stroke, radius=8, w=w, h=32)


def q1_cell(ph):
    rows = []
    for surface in ("bg-secondary", "bg-primary"):
        rows.append(frame(f"on {surface}", [search_field(ph, surface),
                                             text(f"on {T[surface]}  {cr(T.get(ph, ph), T[surface])}", 12, "text-tertiary")],
                          direction="column", gap=4, align="start"))
    return frame("stage", rows, direction="column", gap=10, pad=(12, 12, 12, 12), fill="bg-primary", stroke="border-secondary", radius=8)


# ------------------------------------------------------------------ Q2 selected row
def layer_row(icon, name, fill=None, txt="text-secondary", tag=None):
    kids = [text(icon, 12, "text-tertiary", 500), text(name, 12, txt, 500 if txt == "text-primary" else 400)]
    row = frame("row " + name, [frame("left", kids, gap=8)] + ([text(tag, 12, "text-tertiary")] if tag else []),
                pad=(0, 8, 0, 8), fill=fill, radius=8, w=300, h=32, main="space-between")
    return row


def q2_cell(sel):
    rows = [layer_row("#", "Header"), layer_row("T", "Title", fill=sel, txt="text-primary", tag="selected"),
            layer_row("▭", "Background", fill="bg-secondary", tag="hover #171717")]
    return frame("Layers list", rows, direction="column", gap=0, pad=(8, 8, 8, 8), fill="bg-primary", stroke="border-secondary", radius=8)


# ------------------------------------------------------------------ Q3 accent text
def q3_cell(accent):
    hist = frame("history row", [text("Autosave 12:04", 12, "text-secondary"), text("Restore", 12, accent, 600)],
                 pad=(0, 8, 0, 8), w=300, h=32, main="space-between")
    var = frame("rail row", [text("Variables", 12, "text-secondary"), text("New", 12, accent, 600)], gap=8, pad=(0, 8, 0, 8), h=32)
    return frame("stage", [hist, var, text(f"accent on #0A0A0A  {cr(T.get(accent, accent), T['bg-primary'])}", 12, "text-tertiary")],
                 direction="column", gap=4, pad=(12, 12, 12, 12), fill="bg-primary", stroke="border-secondary", radius=8, align="start")


# ------------------------------------------------------------------ Q4 field height
def field(prefix, value, h):
    return frame("field " + prefix, [text(prefix, 12, "text-tertiary", 500), text(value, 12, "text-primary")], gap=8,
                 pad=(0, 8, 0, 8), fill="bg-secondary", radius=8, w=88, h=h)


def q4_cell(h):
    grid = [frame("row 1", [field("X", "120", h), field("Y", "64", h)], gap=8),
            frame("row 2", [field("W", "1440", h), field("H", "900", h)], gap=8)]
    return frame("Position", [text("Position", 14, "text-primary", 600)] + grid, direction="column", gap=8,
                 pad=(12, 12, 12, 12), fill="bg-primary", stroke="border-secondary", radius=8, align="start")


# ------------------------------------------------------------------ Q5 floating chrome radius
def menu_item(label, key, hover, radius, h=32):
    return frame("item " + label, [text(label, 12, "text-primary" if hover else "text-secondary", 500), text(key, 12, "text-tertiary")],
                 pad=(0, 8, 0, 8), fill="bg-secondary" if hover else None, radius=radius, w=184, h=h, main="space-between")


def q5_cell(outer, inset, inner):
    items = [menu_item("Duplicate", "⌘D", True, inner), menu_item("Rename", "⌘R", False, inner), menu_item("Delete", "⌫", False, inner)]
    menu = frame("menu", items, direction="column", gap=0, pad=(inset, inset, inset, inset), fill="bg-primary",
                 stroke="border-secondary", radius=outer)
    return frame("stage", [menu], pad=(16, 16, 16, 16), fill="bg-secondary", radius=8)


# ------------------------------------------------------------------ Q6 button xs label
def q6_cell(size):
    share = frame("Share", [text("Share", size, "dark-label", 600)], pad=(0, 10, 0, 10), fill="brand", radius=8, h=32, main="center")
    export = frame("Export", [text("Export", size, "text-secondary", 600)], pad=(0, 10, 0, 10), fill="bg-primary",
                   stroke="border-primary", radius=8, h=32, main="center")
    row = frame("toolbar", [export, share], gap=8)
    rows = frame("next to rows", [text("Layer name  ·  12 px row text", 12, "text-tertiary")], h=32)
    return frame("stage", [row, rows], direction="column", gap=4, pad=(12, 12, 12, 12), fill="bg-primary", stroke="border-secondary",
                 radius=8, align="start")


# ------------------------------------------------------------------ Q7 destructive hover
def q7_cell(hover):
    def btn(fill, state):
        return frame(f"Delete {state}", [frame("btn", [text("Delete", 14, "text-white", 600)], pad=(0, 12, 0, 12), fill=fill,
                                               radius=8, h=36, main="center"),
                                         text(f"{state} {T[fill]}  {cr(T['text-white'], T[fill])}", 12, "text-tertiary")],
                     direction="column", gap=4, align="start")
    return frame("stage", [btn("red-600", "default"), btn(hover, "hover")], direction="column", gap=10, pad=(12, 12, 12, 12), fill="bg-primary",
                 stroke="border-secondary", radius=8, align="start")


def board():
    q = [
        card("Q1", "Placeholder colour (spec A, OQ-1)", "Search field placeholder on both field fills.", [
            ("A", "#A3A3A3", True, q1_cell("text-tertiary"), "Neutral-400. AA on every surface."),
            ("B", "#737373 kit text-placeholder", False, q1_cell("fg-quaternary"), "Kit value. Below AA 4.5:1 on both fills."),
        ]),
        card("Q2", "Selected row fill (spec A, OQ-2)", "Layers list, row 2 selected, row 3 hovered.", [
            ("A", "#262626 bg-secondary_hover", True, q2_cell("bg-tertiary"), "Selected reads one step above hover."),
            ("B", "#171717 bg-secondary (kit literal)", False, q2_cell("bg-secondary"), "Selected equals hover in dark mode."),
        ]),
        card("Q3", "Accent text (spec A, OQ-3)", "A text link and an active mark at editor density.", [
            ("A", "#F97316 orange", True, q3_cell("orange-500"), "Studio orange marks actions and new items."),
            ("B", "#D4D4D4 kit text-brand-secondary", False, q3_cell("text-secondary"), "Kit dark brand text is neutral; the mark disappears."),
        ]),
        card("Q4", "Inspector field height (spec B, OQ-4)", "Position block X/Y/W/H, fields 88 wide.", [
            ("A", "32 CONTROL_XS", True, q4_cell(32), "R5 row 32; section +12 per group."),
            ("B", "36 kit Input sm", False, q4_cell(36), "Smallest kit input; +16 per group."),
            ("C", "24 today", False, q4_cell(24), "Densest; no kit size."),
        ]),
        card("Q5", "Floating-chrome radius (spec B, OQ-5)", "Context menu over the editor.", [
            ("A", "16 radius-2xl, items 8 at inset 8", True, q5_cell(16, 8, 8), "Concentric: 16 − 8 = 8."),
            ("B", "8 kit radius-md, items 6 at inset 4", False, q5_cell(8, 4, 6), "Kit menu; 8 − 4 ≠ 6, not concentric."),
        ]),
        card("Q6", "Button xs label size (spec C)", "Export and Share, 32 high, Semibold 600.", [
            ("A", "14 text-sm (kit)", True, q6_cell(14), "Kit Buttons/Button xs uses Text sm Semibold."),
            ("B", "12 text-xs (R5 rows)", False, q6_cell(12), "Same size as row text; quieter actions."),
        ]),
        card("Q7", "Destructive button hover (spec C)", "Delete button, default and hover, white label 14.", [
            ("A", "#EF4444 kit dark hover", False, q7_cell("red-500"), "Kit dark value; hover fails AA."),
            ("B", "#DC2626 keep", False, q7_cell("red-600"), "AA, but hover shows no change."),
            ("C", "#B91C1C Red-700", True, q7_cell("red-700"), "Kit light-mode hover direction; AA."),
        ]),
    ]
    rows = [frame(f"row {i}", q[i:i + 2], gap=24, align="start") for i in range(0, len(q), 2)]
    title = frame("title", [text("Open owner questions — Untitled UI dark revamp", 20, "text-primary", 600),
                            text("R5 editor density · Untitled UI v8 dark · Studio orange #E36323 · (rec) = designer's recommendation · "
                                 "contrast computed (WCAG 2.x). Built with designos --apply-spec.", 12, "text-tertiary")],
                  direction="column", gap=4, align="start")
    return frame("Owner questions board", [title] + rows, direction="column", gap=24, pad=(32, 32, 32, 32), fill="#000000", align="start")


if __name__ == "__main__":
    json.dump(board(), open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
