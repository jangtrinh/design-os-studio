"""Spec C specimen: every Studio widget kind x state, Untitled UI dark at R5 density, as one NodeSpec board.

Usage:
  python3 C-widget-kit-specimen-spec.py board.json        # NodeSpec for `designos <file> --apply-spec board.json`
  python3 C-widget-kit-specimen-spec.py --md              # the state tables of spec C, from the same data
  python3 C-widget-kit-specimen-spec.py --inventory       # table 3.3
  python3 C-widget-kit-specimen-spec.py --json            # every state look as data, for the kit tests

Colours are named by Untitled token (Dark mode) and resolved through TOK, so the board and the spec tables
cannot drift. Brand roles carry the R4 orange remap, aligned with specs/token-contract.md (spec A wins on conflict).
"""
import json, sys

TOK = {  # Untitled UI PRO v8.0, 1_ Color modes / Dark mode, resolved (evidence/resolve-untitled-tokens.py)
    "bg-primary": "#0A0A0A", "bg-primary_hover": "#171717", "bg-secondary": "#171717",
    "bg-secondary_hover": "#262626", "bg-tertiary": "#262626", "bg-quaternary": "#404040",
    "border-primary": "#404040", "border-secondary": "#262626", "border-secondary_alt": "#262626",
    "text-primary": "#FAFAFA", "text-secondary": "#D4D4D4", "text-secondary_hover": "#E5E5E5",
    "text-tertiary": "#A3A3A3", "text-tertiary_hover": "#D4D4D4", "text-quaternary": "#A3A3A3",
    "text-white": "#FFFFFF",
    "text-placeholder": "#737373",  # R8.1: the kit value, the one named exception to the AA text law
    "fg-primary": "#FFFFFF", "fg-secondary": "#D4D4D4", "fg-secondary_hover": "#E5E5E5",
    "fg-tertiary": "#A3A3A3", "fg-quaternary": "#737373", "fg-quaternary_hover": "#A3A3A3",
    "bg-error-solid": "#DC2626", "bg-error-solid_hover": "#B91C1C",  # R8.7: Red-700 (kit dark value Red-500 fails AA)
    "Red-800": "#991B1B",  # primitive: derived destructive Pressed (white 8.31:1) "bg-error-primary": "#450A0A",
    "border-error": "#F87171", "text-error-primary": "#F87171", "fg-error-primary": "#EF4444",
    "fg-success-primary": "#22C55E", "fg-warning-primary": "#EAB308", "bg-overlay": "#262626", "bg-primary-solid": "#262626",
    "bg-primary_alt": "#171717", "tooltip-supporting-text": "#D4D4D4", "fg-white": "#FFFFFF",
    "utility-neutral-50": "#171717", "utility-neutral-200": "#404040", "utility-neutral-700": "#D4D4D4",
    "utility-orange-50": "#431407", "utility-orange-200": "#9A3412", "utility-orange-700": "#FDBA74",
    # Brand roles: kit Brand ramp -> Orange ramp; R4 pins solid and focus. Aligned with specs/token-contract.md (A wins).
    "bg-brand-solid": "#E36323", "bg-brand-solid_hover": "#F97316",
    "Orange-600": "#EA580C",  # primitive: derived Pressed fill (the kit has no Pressed state)
    "border-brand": "#FB923C", "fg-brand-primary": "#F97316", "focus-ring": "#F97316",
    "Neutral-950": "#0A0A0A",  # _Primitives Neutral 950: the R4 dark label on orange
    # Studio canvas exceptions (plan.md controller defaults), never Untitled roles.
    "canvas-component": "#06B6D4",
    "alpha-black-10": "#FFFFFF1A",  # Dark mode resolves the kit alpha-black-10 role to white at 10 %
}
NONE = None
STATES = ["Default", "Hover", "Pressed", "Focused", "Disabled", "Selected"]


def hx(t):
    return TOK[t] if t else "transparent"


def text(s, size, color, weight=400, name=None):
    return {"kind": "text", "name": name or s[:24], "text": s, "font_size": size, "font_weight": weight,
            "text_color": hx(color) if color in TOK else color}


def frame(name, children=(), direction="row", gap=0, pad=(0, 0, 0, 0), fill=None, stroke=None, sw=1, radius=None,
          w=None, h=None, align="center", main="start", opacity=None):
    n = {"kind": "frame", "name": name, "children": list(children), "fill": hx(fill) if fill in TOK else (fill or "transparent"),
         "layout": {"direction": direction, "gap": gap, "padding": list(pad), "primary_align": main, "counter_align": align}}
    if stroke:
        n["stroke"], n["stroke_weight"] = hx(stroke), sw
    if radius is not None:
        n["corner_radius"] = radius
    n["width_sizing"], n["height_sizing"] = ("fixed" if w else "hug"), ("fixed" if h else "hug")
    if w:
        n["width"] = w
    if h:
        n["height"] = h
    if opacity is not None:
        n["opacity"] = opacity
    return n


def glyph(ch, color, size=12):
    return text(ch, size, color, 500, name="icon " + ch)


def ring(child, radius, on=True):
    """Focus ring: 2px focus-ring stroke, 2px gap outside the control (kit Focus rings, R4 colour)."""
    if not on:
        return frame("no ring", [child], pad=(4, 4, 4, 4))
    return frame("focus ring", [child], pad=(2, 2, 2, 2), stroke="focus-ring", sw=2, radius=radius + 2)


# ---------------------------------------------------------------- widget painters (one cell = one widget in one state)
def look(st):
    """State tuple -> (fill, border, border_w, text, icon); a 5th element overrides the border width."""
    fill, border, txt, icon = st[:4]
    return fill, border, (st[4] if len(st) > 4 else 1), txt, icon


def w_button(st, label="Share", h=32, padx=10, radius=8, size=14, weight=600, icon=None):
    fill, border, bw, txt, ic = look(st)
    kids = ([glyph(icon, ic, 14)] if icon else []) + ([text(label, size, txt, weight)] if label else [])
    return frame("button", kids, gap=4, pad=(0, padx, 0, padx), fill=fill, stroke=border, sw=bw, radius=radius, h=h,
                 w=h if not label else None, main="center")


def w_field(st, value="1440", placeholder=False, h=32, w=150, prefix=None, suffix=None, chevron=False, search=False):
    fill, border, bw, txt, ic = look(st)
    kids = []
    if search:
        kids.append(glyph("⌕", ic, 13))
    if prefix:
        kids.append(text(prefix, 12, ic, 500, name="prefix"))
    kids.append(text(value, 12, txt, 400, name="value"))
    if suffix:
        kids.append(text(suffix, 12, ic, 400, name="suffix"))
    body = frame("content", kids, gap=8)
    row = [body] + ([glyph("⌄", ic, 13)] if chevron else [])
    return frame("field", row, pad=(0, 10, 0, 10), fill=fill, stroke=border, sw=bw, radius=8, h=h, w=w,
                 main="space-between" if chevron else "start")


def w_menu_item(st, label="Duplicate", shortcut="⌘D", w=180, pitch=32):
    fill, border, bw, txt, ic = look(st)
    mark = "√" if ic == "fg-brand-primary" else shortcut  # Selected shows the brand check, not a coloured shortcut
    pill = frame("pill", [text(label, 12, txt, 600), text(mark, 12, ic, 500, name="shortcut")], pad=(0, 8, 0, 8),
                 fill=fill, radius=8, h=pitch, w=w - 16, main="space-between")
    return frame("menu item", [pill], pad=(0, 8, 0, 8), w=w, h=pitch)  # R8.5: item radius 8 at inset 8


def w_menu(st):
    fill, border, bw, txt, ic = look(st)
    sep = frame("separator", [frame("line", fill="border-secondary", h=1, w=176)], pad=(4, 0, 4, 0))
    items = [w_menu_item(("bg-primary_hover", None, "text-secondary_hover", "text-quaternary"), "Copy", "⌘C", 176),
             sep, w_menu_item((None, None, "text-secondary", "text-quaternary"), "Delete", "⌫", 176)]
    return frame("menu panel", items, direction="column", pad=(8, 0, 8, 0), fill=fill, stroke=border, sw=bw, radius=16, w=178)


def w_separator(st):
    fill, border, bw, txt, ic = look(st)
    return frame("separator", [frame("line", fill=fill, h=1, w=150)], pad=(4, 0, 4, 0))


def w_toggle(st, on=True):
    fill, border, bw, txt, ic = look(st)
    knob = frame("knob", fill=ic, radius=8, w=16, h=16)
    return frame("toggle", [knob], pad=(2, 2, 2, 2), fill=fill, stroke=border, sw=bw, radius=10, w=36, h=20,
                 main="end" if on else "start")


def w_checkbox(st, on=True):
    fill, border, bw, txt, ic = look(st)
    kids = [glyph("√", ic, 11)] if on else []
    return frame("checkbox", kids, fill=fill, stroke=border, sw=bw, radius=4, w=16, h=16, main="center")


def w_radio_card(st, title="Ask", body="Approve each edit"):
    fill, border, bw, txt, ic = look(st)
    on = ic == "bg-brand-solid"  # Icon column = the radio circle: brand fill with a Neutral-950 dot, or an fg-quaternary ring
    dot = frame("radio", [frame("dot", fill="Neutral-950", radius=3, w=6, h=6)] if on else [], fill=ic if on else None,
                stroke=None if on else ic, radius=8, w=16, h=16, main="center")
    words = frame("words", [text(title, 12, txt, 600), text(body, 12, "text-tertiary", 400)], direction="column", align="start")
    return frame("radio card", [dot, words], gap=8, pad=(12, 12, 12, 12), fill=fill, stroke=border, sw=bw, radius=12, w=170, align="start")


def w_tab(st, labels=("Design", "Prototype"), sel=0, segmented=True):
    fill, border, bw, txt, ic = look(st)
    kids = []
    for i, l in enumerate(labels):
        on = i == sel
        kids.append(frame("tab " + l, [text(l, 12, txt if on else "text-quaternary", 600)], pad=(0, 10, 0, 10),
                          fill=fill if on else None, stroke=border if on else None, sw=bw, radius=6 if segmented else 8,
                          h=20 if segmented else 26, main="center"))  # R10.1: today's editor heights (24 strip, 26 file tab)
    if segmented:  # tiles 6 inside a padded 8 container (RADIUS_CONTROL 8 - INSET_SEGMENT 2)
        return frame("tabs", kids, gap=2, pad=(2, 2, 2, 2), fill="bg-secondary", stroke="border-secondary", radius=8)
    return frame("tabs", kids, gap=4)


def w_badge(st, label="12"):
    fill, border, bw, txt, ic = look(st)
    return frame("badge", [text(label, 12, txt, 500)], pad=(2, 8, 2, 8), fill=fill, stroke=border, sw=bw, radius=11, h=22, main="center")


def w_tag(st, label="Frame 12"):
    fill, border, bw, txt, ic = look(st)
    return frame("tag", [text(label, 12, txt, 500), glyph("×", ic, 12)], gap=4, pad=(2, 6, 2, 8), fill=fill, stroke=border, sw=bw,
                 radius=8, h=24)


def w_tooltip(st, label="Frame  F"):
    fill, border, bw, txt, ic = look(st)
    return frame("tooltip", [text(label, 12, txt, 600)], pad=(8, 12, 8, 12), fill=fill, stroke=border, sw=bw, radius=8)


def w_surface(st, title="Rename file", w=180, title_size=16):
    fill, border, bw, txt, ic = look(st)
    head = frame("header", [text(title, title_size, txt, 600), glyph("×", ic, 13)], w=w - 2, pad=(10, 12, 10, 16), main="space-between")
    body = frame("body", [text("Name", 12, "text-secondary", 500)], pad=(0, 16, 12, 16))
    return frame("surface", [head, body], direction="column", fill=fill, stroke=border, sw=bw, radius=16, w=w, align="start")


def w_toast(st, label="Saved to Drafts"):
    fill, border, bw, txt, ic = look(st)
    return frame("toast", [frame("dot", fill=ic, radius=4, w=8, h=8), text(label, 14, txt, 500)], gap=8, pad=(0, 16, 0, 16),
                 fill=fill, stroke=border, sw=bw, radius=12, h=40)


def w_row(st, label="Header", depth=1, chevron=True, w=180, h=32, size=12, icon="#"):
    fill, border, bw, txt, ic = look(st)
    kids = [frame("indent", w=depth * 12, h=4)] if depth else []
    if chevron:
        kids.append(glyph("›", ic, 13))
    kids += [glyph(icon, ic, 12), text(label, size, txt, 500)]
    return frame("row", kids, gap=6, pad=(0, 8, 0, 8), fill=fill, stroke=border, sw=bw, radius=8, w=w, h=h)


def w_header(st, title="Layers", h=40, size=14, w=180, action="+"):
    fill, border, bw, txt, ic = look(st)
    kids = [text(title, size, txt, 600)] + ([frame("action", [glyph(action, ic, 14)], w=32, h=32, radius=8, main="center")] if action else [])
    return frame("header", kids, pad=(0, 4, 0, 12), fill=fill, stroke=border, sw=bw, w=w, h=h, main="space-between")


def w_swatch(st, color="#0D99FF"):
    fill, border, bw, txt, ic = look(st)
    return frame("chip", fill=color, stroke=border, sw=bw, radius=4, w=20, h=20)


def w_progress(st, frac=0.6, w=140):
    fill, border, bw, txt, ic = look(st)
    bar = frame("track", [frame("fill", fill=ic, radius=4, w=int(w * frac), h=8)], fill=fill, radius=4, w=w, h=8)
    return frame("progress", [bar, text(f"{int(frac * 100)}%", 12, txt, 500)], gap=8)


def w_loading(st):
    fill, border, bw, txt, ic = look(st)
    track = {"kind": "ellipse", "name": "track", "width": 16, "height": 16, "fill": "transparent", "stroke": hx("bg-tertiary"),
             "stroke_weight": 2, "width_sizing": "fixed", "height_sizing": "fixed"}
    arc = frame("spinner", [track], w=16, h=16)
    dot = frame("head", fill=ic, radius=2, w=4, h=4)
    return frame("loading", [arc, dot, text("Working…", 12, txt, 500)], gap=6, pad=(0, 8, 0, 8), fill=fill, stroke=border, sw=bw,
                 radius=8, h=24)


def w_empty(st):
    fill, border, bw, txt, ic = look(st)
    icon = frame("icon", [glyph("#", ic, 14)], stroke=border, sw=bw, radius=8, w=32, h=32, main="center")
    return frame("empty", [icon, text("No files yet", 14, txt, 600), text("Create a design file", 12, "text-tertiary", 400)],
                 direction="column", gap=6)


def w_slider(st):
    fill, border, bw, txt, ic = look(st)
    handle = frame("handle", fill=ic, stroke=border, sw=2, radius=8, w=16, h=16)
    left = frame("on", fill="fg-brand-primary", radius=4, w=60, h=8)
    right = frame("off", fill=fill, radius=4, w=60, h=8)
    return frame("slider", [left, handle, right])


def w_shortcut(st, keys="⌘K"):
    fill, border, bw, txt, ic = look(st)
    return frame("shortcut", [text(keys, 12, txt, 500)], pad=(1, 4, 1, 4), fill=fill, stroke=border, sw=bw, radius=4)


def w_divider(st):
    fill, border, bw, txt, ic = look(st)
    return frame("divider", [frame("line", fill=fill, h=1, w=150)], pad=(8, 0, 8, 0))


def w_palette(st):
    fill, border, bw, txt, ic = look(st)
    search = frame("search", [glyph("⌕", "fg-quaternary", 13), text("Search commands", 14, "text-placeholder", 400)], gap=8,
                   pad=(0, 12, 0, 12), h=40, w=176)
    line = frame("divider", fill="border-secondary", h=1, w=176)
    items = frame("items", [w_menu_item((P, None, "text-secondary_hover", "text-quaternary"), "Export…", "⌘E", 176),
                            w_menu_item((None, None, "text-secondary", "text-quaternary"), "Rename", "⌘R", 176)],
                  direction="column", pad=(8, 0, 8, 0))
    return frame("palette", [search, line, items], direction="column", fill=fill, stroke=border, sw=bw, radius=16, w=178)


# ---------------------------------------------------------------- the kit: one entry per Studio widget kind
# A state is a look tuple (fill, border, text, icon[, border_w]), ("ring", tuple) for a focused look with the
# focus ring, "half" (Default at 50 % opacity, the kit's disabled rule), or "n/a" (the widget has no such state).
N = None
P = "bg-primary_hover"  # kit hover fill on a bg-primary surface (rule S5 shifts it on bg-secondary)


def S(default, hover, pressed, focused, disabled="half", selected="n/a"):
    return dict(zip(STATES, [default, hover, pressed, focused, disabled, selected]))


BTN = "Buttons/Button · Hierarchy={h} · Icon only={i} · Size=xs|sm|md"
W = [
    dict(name="Button/Primary", draw=w_button, kit=BTN.format(h="Primary", i="False"), node=24945,
         studio="home/dialog.rs:349-368 · libraries/modal.rs:81-101 · comments/ui.rs:92-108 · home/mod.rs:1407-1420 · agent_approval_card.rs:137-160",
         geom="xs 32 (pad 6/10, icon 16) · sm 36 (pad 8/12, icon 20) · md 40 (pad 10/14, icon 20) · gap spacing-xs 4 · radius-md 8 · label text-sm semibold",
         st=S(("bg-brand-solid", N, "Neutral-950", "Neutral-950"), ("bg-brand-solid_hover", N, "Neutral-950", "Neutral-950"),
              ("Orange-600", N, "Neutral-950", "Neutral-950"), ("ring", ("bg-brand-solid", N, "Neutral-950", "Neutral-950")))),
    dict(name="Button/Secondary", draw=w_button, kit=BTN.format(h="Secondary", i="False"), node=24945,
         studio="comments/ui.rs:92-108 (primary=false) · libraries/modal.rs:81-101 · home/mod.rs:1421-1426 · agent_approval_card.rs:150-158",
         geom="as Button/Primary; border 1 px",
         st=S(("bg-primary", "border-primary", "text-secondary", "fg-quaternary"), (P, "border-primary", "text-secondary_hover", "fg-quaternary_hover"),
              ("bg-quaternary", "border-primary", "text-secondary_hover", "fg-quaternary_hover"),
              ("ring", ("bg-primary", "border-primary", "text-secondary", "fg-quaternary")))),
    dict(name="Button/Tertiary", draw=w_button, kit=BTN.format(h="Tertiary", i="False"), node=24945,
         studio="home/dialog.rs:383-392 (Cancel) · comments/ui.rs:92-108 (plain tile)",
         geom="as Button/Primary; no fill, no border at rest",
         st=S((N, N, "text-tertiary", "fg-quaternary"), (P, N, "text-tertiary_hover", "fg-quaternary_hover"),
              ("bg-quaternary", N, "text-tertiary_hover", "fg-quaternary_hover"), ("ring", ("bg-primary", N, "text-tertiary", "fg-quaternary")))),
    dict(name="Button/Destructive", draw=w_button, kit="Buttons/Button destructive · Hierarchy=Primary · Size=xs|sm|md", node=26468,
         studio="home/mod.rs:1427-1432 (Kind::Danger) · home/dialog.rs:353-354 (d.danger)",
         geom="as Button/Primary",
         parts="kit Hover is Red-500 #EF4444 (white 3.76:1); R8.7 sets Red-700. Pressed Red-800 is derived (no kit Pressed).",
         st=S(("bg-error-solid", N, "text-white", "text-white"), ("bg-error-solid_hover", N, "text-white", "text-white"),
              ("Red-800", N, "text-white", "text-white"), ("ring", ("bg-error-solid", N, "text-white", "text-white")))),
    dict(name="Button/Icon", draw=w_button, args=dict(label="", icon="+"), kit=BTN.format(h="Tertiary", i="True"), node=24945,
         studio="widgets/mod.rs:169-186 icon_button · widgets/mod.rs:101-115 button_bg · comments/ui.rs:117-131 dots_button",
         geom="xs 32×32 (pad 8, icon 16) · sm 36×36 (pad 8, icon 20) · radius-md 8",
         st=S((N, N, N, "fg-quaternary"), (P, N, N, "fg-quaternary_hover"), ("bg-quaternary", N, N, "fg-quaternary_hover"),
              ("ring", ("bg-primary", N, N, "fg-quaternary")), "half", ("bg-secondary_hover", N, N, "fg-secondary_hover"))),
    dict(name="Button/Tool", draw=w_button, args=dict(label="", icon="#"), kit="derived: Buttons/Button · Hierarchy=Tertiary · Icon only=True · Size=xs, Selected from _Nav item base · Current=True", node=24945,
         studio="panels/toolbar.rs:209-227 (tool) · :232-270 (chevron, actions)",
         geom="32×32, icon 20 (R6 icon size kept), radius-md 8",
         st=S((N, N, N, "fg-quaternary_hover"), (P, N, N, "fg-secondary_hover"), ("bg-quaternary", N, N, "fg-secondary_hover"),
              ("ring", ("bg-primary", N, N, "fg-quaternary_hover")), "half", ("bg-secondary_hover", N, N, "fg-primary"))),
    dict(name="Button/Close", draw=w_button, args=dict(label="", icon="×"), kit="Buttons/Button close X · Size=sm · Dark background=False", node=26431,
         studio="widgets/popover.rs:40-43 · libraries/modal.rs:65-79 · tabs.rs:1075-1079 (file tab ×)",
         geom="sm 36×36 (pad 8, icon 20) · xs 32×32 (derived, icon 16) · radius-md 8",
         st=S((N, N, N, "fg-quaternary"), (P, N, N, "fg-quaternary_hover"), ("bg-quaternary", N, N, "fg-quaternary_hover"),
              ("ring", ("bg-primary", N, N, "fg-quaternary")), "n/a")),
]

FIELD = S(("bg-primary", "border-primary", "text-primary", "fg-quaternary"), ("bg-primary", "border-primary", "text-primary", "fg-quaternary"), "n/a",
          ("bg-primary", "border-brand", "text-primary", "fg-quaternary", 2))
W += [
    dict(name="Input/Field", draw=w_field, args=dict(value="Frame 12"), kit="Input field · Type=Default · Size=sm (xs derived)", node=27883,
         studio="widgets/field.rs:695-705 field_bg · :707-717 text_input/_ph · :25-126 edit_text_hint",
         geom="xs 32 (derived: pad 0/10, value text-xs) · sm 36 (pad 8/12, gap 8, icon 16, text-sm) · radius-md 8 · placeholder text-placeholder",
         st=FIELD),
    dict(name="Input/PropertyField", draw=w_field, args=dict(value="1440", prefix="W"), kit="derived: Input field · Type=Leading text · Size=sm (add-on prefix without its divider)", node=27883,
         studio="widgets/field.rs:563-645 num_field · :648-693 scrub_prefix · :128-182 edit_num_text",
         geom="xs 32; prefix slot 24 (letter text-xs medium or icon 12) in the add-on colour; value text-xs (numbers mono, spec B); radius-md 8",
         st=S(("bg-primary", "border-primary", "text-primary", "text-tertiary"), ("bg-primary", "border-primary", "text-primary", "text-tertiary"), "n/a",
              ("bg-primary", "border-brand", "text-primary", "text-tertiary", 2))),
    dict(name="Input/Search", draw=w_field, args=dict(value="Search layers", search=True), kit="Input field · Type=Default · Size=sm with a leading search icon", node=27883,
         studio="find/filter.rs:64-80 · libraries/modal.rs:129-144 · find/panel.rs:158-177 · cmdk.rs:357-385 (palette input, borderless)",
         geom="as Input/Field; leading icon 16 at pad-left, gap 8",
         st=S(("bg-primary", "border-primary", "text-placeholder", "fg-quaternary"), ("bg-primary", "border-primary", "text-placeholder", "fg-quaternary"), "n/a",
              ("bg-primary", "border-brand", "text-primary", "fg-quaternary", 2))),
    dict(name="Select/Trigger", draw=w_field, args=dict(value="Fill", chevron=True), kit="Select · Type=Default · Size=sm (State=Open uses the focused look)", node=34416,
         studio="widgets/menu.rs:571-601 dropdown/dropdown_checked · widgets/picker.rs:1255-1292 picker_dropdown",
         geom="as Input/Field; chevron-down 16 at pad-right 10",
         st=FIELD),
    dict(name="Menu/Panel", draw=w_menu, kit="Dropdown menu · Type=Icon advanced · Open=True → Menu (Context menu uses the same frame, node 27823)", node=27475,
         studio="widgets/mod.rs:90-99 menu_panel · widgets/menu.rs:429 · panels/context.rs:145 (context menu via popup_tree)",
         geom="radius-2xl 16 (R8.5; kit radius-md 8) · pad 8 all round, items inset 8 (R8.5, concentric 16 - 8 = 8) · width unchanged (layout) · no shadow (dark shadow-lg is transparent)",
         st=S(("bg-primary", "border-secondary_alt", "text-secondary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Menu/Context", draw=w_menu, kit="Context menu · Type=Simple · Open=True → Menu (same frame as Dropdown menu)", node=27823,
         studio="panels/context.rs:145 (show via widgets/menu.rs:299 popup_tree) · shell.rs:990 (ctx_menu position)",
         geom="as Menu/Panel: bg-primary, border-secondary_alt 1 px, radius-2xl 16, pad 8, rows Menu/Item xs (R8.5)",
         st=S(("bg-primary", "border-secondary_alt", "text-secondary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Menu/Item", draw=w_menu_item, kit="_Dropdown menu list item · Inset icon=True (Selected from _Select menu item · Selected=True, node 36212)", node=27767,
         studio="widgets/menu.rs:430-505 (pill :466, label :468, chevron :475-488, shortcut :492-501)",
         geom="xs pill 32, sm pill 36, stacked with no gap · pill radius-md 8 at inset 8 (R8.5; kit radius-sm 6 at inset 6) · pad 8 / 8 · gap 4 · label text-xs semibold (sm: text-sm)",
         st=S((N, N, "text-secondary", "fg-quaternary"), (P, N, "text-secondary_hover", "fg-quaternary_hover"), (P, N, "text-secondary_hover", "fg-quaternary_hover"),
              (P, N, "text-secondary_hover", "fg-quaternary_hover"), "half", (N, N, "text-secondary", "fg-brand-primary"))),
    dict(name="Menu/Shortcut", draw=w_shortcut, kit="_Dropdown menu list item → Shortcut (text) · _Command shortcut for key caps", node=27767,
         studio="widgets/menu.rs:492-501 (MENU_SHORTCUT_HIGHLIGHT on the lit row)",
         geom="text-xs medium, right-aligned, pad-right spacing-xs 4 inside the pill",
         st=S((N, N, "text-quaternary", N), (N, N, "text-quaternary", N), (N, N, "text-quaternary", N), "n/a")),
    dict(name="Menu/Separator", draw=w_separator, kit="_Dropdown menu list item · Divider=True", node=27767,
         studio="widgets/menu.rs:441, :504 (BORDER_MENU, MENU_SEP_MARGIN 8)",
         geom="height 9: spacing-xs 4 + 1 px line + 4 (kit literal; 9 is not a spacing step, allowed in kit/ only), full width inside the 8 inset",
         st=S(("border-secondary", N, N, N), "n/a", "n/a", "n/a", "n/a")),
]

W += [
    dict(name="Tabs/Segmented", draw=w_tab, kit="_Tab button base · Type=Button minimal · Size=sm (container: Horizontal tabs · Type=Button minimal)", node=86171,
         studio="comments/ui.rs:133-152 segmented · panels/properties.rs:299-336 · panels/toolbar.rs:279-315 (mode switch)",
         geom="height is the surface's current height (R10.1): Inspector segmented strip 24 with 20 px tiles (24 - 2 x INSET_SEGMENT); Home and dialogs keep sm (32 tiles in a 36 strip) · container pad 2, radius-md 8, tiles radius 6 (8 - 2 inset), pad 0/10 · label text-xs 12/18 semibold (R5, R8.6) · icon segments 16",
         st=S((N, N, "text-quaternary", "fg-quaternary"), (N, N, "text-secondary", "fg-quaternary_hover"), "n/a",
              ("ring", ("bg-primary", N, "text-quaternary", "fg-quaternary")), "half", ("bg-secondary_hover", "border-primary", "text-secondary", "fg-secondary"))),
    dict(name="Tabs/Pill", draw=w_tab, args=dict(segmented=False), kit="_Tab button base · Type=Button gray · Size=sm (xs 32 derived)", node=86171,
         studio="panels/right.rs:103-127 (Design / Prototype, Inspect / Plugins) · tabs.rs:1048-1079 (file tabs: hover and active fill)",
         geom="height is the surface's current height (R10.1): file tab 26 inside the 32 bar (spec D), agent session tab 24 (spec I); Home and dialogs keep sm 36 (pad 8/10) · pad 0/10 · radius-md 8 (R5; kit radius-sm 6) · gap 4 · label text-xs 12/18 semibold (R5, R8.6)",
         st=S((N, N, "text-quaternary", "fg-quaternary"), (P, N, "text-secondary", "fg-quaternary_hover"), "n/a",
              ("ring", ("bg-primary", N, "text-quaternary", "fg-quaternary")), "half", ("bg-secondary_hover", N, "text-secondary", "fg-quaternary_hover"))),
    dict(name="Toggle/Switch", parts="Fill/Border = the track, Icon = the knob. The knob stays fg-white on the brand track (kit; non-text 3.46:1), the one exception to T5.",  draw=w_toggle, args=dict(on=False), radius=10, kit="Toggle · Type=Default · Size=sm → _Toggle base (Selected = Pressed=True)", node=37581,
         studio="panels/components.rs:69-88 · libraries/modal.rs:108-127 · dev/playground.rs:315-334",
         geom="track 36×20, radius-full, pad spacing-xxs 2 · knob 16 · hit box >= 24 (TARGET_MIN)",
         st=S(("bg-tertiary", "border-secondary", N, "fg-white", 0.5), ("bg-tertiary", "border-secondary", N, "fg-white", 0.5), "n/a",
              ("ring", ("bg-tertiary", "border-secondary", N, "fg-white", 0.5)), "half", ("bg-brand-solid", N, N, "fg-white"))),
    dict(name="Checkbox/Box", draw=w_checkbox, args=dict(on=False), radius=4, kit="Checkbox → _Checkbox base · Type=Checkbox · Size=sm (Selected = Checked=True)", node=27396,
         studio="widgets/popover.rs:70-81 check · panels/properties.rs:338-352 · panels/check_designs.rs:330-337",
         geom="16×16, radius-xs 4, check icon 12 · hit box >= 24 with its label",
         parts="Icon = the check mark. The unchecked border is the kit's 1 px border-primary #404040 (contract BORDER_STRONG): 1.91:1 on bg-primary, 1.73:1 on bg-secondary, a named exception to the 3:1 boundary rule decided by the owner (R10.7).",
         st=S((N, "border-primary", N, N), (N, "border-primary", N, N), "n/a", ("ring", ("bg-primary", "border-primary", N, N)),
              ("half", ("bg-tertiary", "border-primary", N, N)), ("bg-brand-solid", N, N, "Neutral-950"))),
    dict(name="Radio/Card", draw=w_radio_card, kit="Radio group item · Type=Radio button · Size=sm (dot: _Checkbox base · Type=Radio)", node=32801,
         studio="agent_approval_card.rs:219-249 (permission mode options)",
         geom="card pad 16, radius-xl 12, gap 8 · radio 16, radius-full, dot 6 · title text-sm medium, caption text-sm (dialog, sm)",
         parts="Fill/Border = the card. Icon = the radio circle: a 1 px border-primary ring when off (contract BORDER_STRONG, the R10.7 exception as Checkbox), a bg-brand-solid disc when Selected, with a 6 px Neutral-950 dot (5.73:1 on the disc).",
         st=S(("bg-primary", "border-secondary", "text-secondary", "border-primary"), (P, "border-secondary", "text-secondary", "border-primary"), "n/a",
              ("ring", ("bg-primary", "border-secondary", "text-secondary", "border-primary")), "half", ("bg-primary", "border-brand", "text-primary", "bg-brand-solid", 2))),
    dict(name="Badge/StatusCount", draw=w_badge, kit="Badge · Type=Pill color · Color=Gray · Icon=False · Size=sm (status: Color=Error|Success|Warning|Orange)", node=22406,
         studio="home/mod.rs:1128-1130 (Trash count) · cmdk.rs:468-471 · canvas/agent_changes_chip.rs:452-457",
         geom="height 22, pad 2/8, radius-full, text-xs medium (count digits mono, spec B)",
         st=S(("utility-neutral-50", "utility-neutral-200", "utility-neutral-700", N), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Tag/Chip", draw=w_tag, kit="Tag · Icon=False · Action=X close · Checkbox=False · Size=sm", node=36937,
         studio="agent_selection_chip.rs:160-200 · libraries/modal.rs:146-152 change_tag · image_jobs.rs:1241-1262 (status chip)",
         geom="height 24, pad 2 / 6 right / 8 left and gap 4 (kit 3 / 4 / 8 and gap 3, snapped to the 2 and 4 steps), radius-md 8 (R5; kit radius-sm 6), text-xs medium, close × 14",
         st=S(("bg-primary", "border-primary", "text-secondary", "fg-quaternary"), ("bg-primary", "border-primary", "text-secondary", "fg-quaternary_hover"), "n/a",
              ("ring", ("bg-primary", "border-primary", "text-secondary", "fg-quaternary")), "n/a")),
    dict(name="Tooltip/Default", draw=w_tooltip, kit="Tooltip · Supporting text=False · Arrow=None", node=37898,
         studio="theme.rs:618-655 (egui stock tooltips from on_hover_text, 84 calls in 33 files)",
         geom="pad 8/12, radius-md 8 (live kit: Content frame radius 8, fill bg-primary-solid), title text-xs semibold text-primary; supporting line text-xs medium text-secondary (token-contract); no arrow",
         st=S(("bg-primary-solid", N, "text-primary", N), "n/a", "n/a", "n/a", "n/a")),
]


W += [
    dict(name="Tree/Row", na={"Focused": "Layers has no arrow-key navigation at 205c72ef (curator review); a later spec that adds it draws the S4 ring"},  draw=w_row, kit="_Tree view item · Size=sm (disclosure and indent derived)", node=86883,
         studio="panels/layers.rs:516-596 (pill :518-530, chevron :532-544, icon :571-579, label :587-664)",
         geom="row 32, pad 6/8, gap 8, radius-md 8 (R5; kit radius-sm 6), chevron 16 (kit), layer icon 16, indent 12 per level (derived), label text-xs semibold (R5)",
         st=S((N, N, "text-tertiary", "fg-quaternary"), (P, N, "text-tertiary", "fg-quaternary"), "n/a", "n/a", "n/a",
              ("bg-secondary_hover", N, "text-tertiary", "fg-quaternary"))),
    dict(name="List/Row", draw=w_row, args=dict(depth=0, chevron=False, label="Drafts"), kit="_Nav item base (xs 32 derived for Pages)", node=72558,
         studio="home/mod.rs:1188-1205 (sidebar) · panels/left.rs:272-276 (pages) · home/dialog.rs:321-323 (picker list)",
         geom="sm pitch 38 (row 36, pad 8, icon 20, gap 8, text-sm semibold) · xs pitch 32 (icon 16, text-xs) · radius-md 8 (R5; kit radius-sm 6)",
         st=S((N, N, "text-secondary", "fg-quaternary"), (P, N, "text-secondary_hover", "fg-quaternary_hover"), "n/a",
              ("ring", ("bg-primary", N, "text-secondary", "fg-quaternary")), "n/a", ("bg-secondary_hover", N, "text-secondary_hover", "fg-quaternary_hover"))),
    dict(name="Panel/Header", draw=w_header, kit="derived: _Modal header (title role) + Button/Icon action", node=None,
         studio="panels/left.rs:192-194 section_title · widgets/popover.rs:31-36 · libraries/modal.rs:65-79",
         geom="height 40 (unchanged), pad-left 16, title text-sm semibold (R5 titles), action Button/Icon xs at the right",
         st=S((N, N, "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Section/Header", draw=w_header, args=dict(title="Fill", h=40), kit="derived: _Modal header title role; separator from _Modal header Divider", node=None,
         studio="panels/properties.rs:199-226 (section_sep, title) · :285-293 sublabel · specs.rs:3777",
         geom="height 40, title text-sm semibold, sublabels text-xs medium text-tertiary, separator 1 px border-secondary",
         st=S((N, "border-secondary", "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", ("half", (N, "border-secondary", "text-primary", "fg-quaternary")))),
    dict(name="Dialog/Modal", draw=w_surface, kit="Modal → _Modal header · Type=Left aligned + _Modal actions · Horizontal fill container", node="81903 + 81763",
         studio="home/dialog.rs:228-392 · libraries/modal.rs:43-79 · agent_approval_card.rs:168-200 · shell.rs:1036+ (web-capture report)",
         geom="frame radius-2xl 16 and no border (live kit: Modal set 4057:415205, frame Modal bound to radius-2xl, no stroke) over the bg-overlay scrim · header pad 24, gap 16, title text-md 16 semibold (kit), supporting text-sm text-tertiary, close Button/Close sm · footer divider + md 40 buttons, gap 12, pad 24",
         st=S(("bg-primary", N, "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Popover/Panel", draw=w_surface, args=dict(title="Drop shadow", title_size=14), kit="derived: Dropdown menu frame + _Modal header (editor size)", node=27475,
         studio="widgets/popover.rs:17-68 (show, row, height)",
         geom="radius-2xl 16 (R8.5), border-secondary_alt 1 px (menu frame), header 40 with title text-sm semibold and Button/Close xs, rows 32 (R5)",
         st=S(("bg-primary", "border-secondary_alt", "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
]

W += [
    dict(name="Dialog/CommandPalette", draw=w_palette, kit="Command bar frame (= Modal surface) · items _Command dropdown menu item · Type=Default · State=Default|Hover|Focus", node=75734,
         studio="cmdk.rs:268-272 (private palette) · :350-356 frame · :357-385 input · :395-426 tabs · :437-474 rows and badges",
         geom="frame radius-2xl 16 (modal surface) · input row 40 (md, borderless, icon 16, text-sm; aligns with spec J) · items Menu/Item at inset 8 (kit _Command dropdown menu item: pitch 44, pill radius-md 8, gap 8), text-sm medium text-primary + text-tertiary supporting text · shortcut Menu/Shortcut",
         st=S(("bg-primary", "border-secondary_alt", "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Toast/Default", parts="Icon = the status dot: fg-brand-primary for info, fg-error-primary #EF4444 for errors, fg-success-primary #22C55E for success.",  draw=w_toast, kit="Notification · Type=No icon (single-line form derived; status dot replaces the featured icon)", node=71816,
         studio="shell.rs:993-1035 (shadow :1013, status dot :1015) · shell/toast_layout.rs (layout) · theme.rs:458 TOAST_HEIGHT 40",
         geom="height 40 (unchanged), pad 0/16, gap 8, radius-xl 12 (kit Notification; contract RADIUS_XL), text-sm medium text-primary, dot 8: fg-brand-primary (info) / fg-error-primary (error) / fg-success-primary",
         st=S(("bg-primary_alt", "border-secondary_alt", "text-primary", "fg-brand-primary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Swatch/Color", parts="Border = the 1 px inner outline; Selected adds the 2 px focus-ring outline (derived).",  draw=w_swatch, radius=4, kit="_Color swatch · Type=Default · State=Default|Hover|Focused", node=75651,
         studio="widgets/mod.rs:188-200 swatch · :201-233 swatch_circle · widgets/picker.rs:675 paint_swatch, :1383 opacity_cell",
         geom="shape and size unchanged (layout); 1 px inner stroke alpha-black-10 (white 10 % in Dark) · focused: 2 px ring in the swatch's own colour at 4 px out (kit 28 ring on a 20 chip) · selected: Studio ring focus-ring (derived)",
         st=S((N, "alpha-black-10", N, N), (N, "alpha-black-10", N, N), "n/a", ("ring", (N, "alpha-black-10", N, N)), "n/a",
              (N, "focus-ring", N, N, 2))),
    dict(name="Slider/Handle", parts="Fill = the empty track, Border = the handle border (kit slider-handle-border = bg-primary, contract BG), Icon = the handle fill (kit slider-handle-bg = fg-brand-primary, contract ICON_BRAND_PRIMARY); the filled part of the track is fg-brand-primary.",  draw=w_slider, kit="Slider · Label=False (track 8, radius-full) + _Control handle", node=36722,
         studio="widgets/picker.rs:649-674 hue_bar and thumb_ring · :721 slider_pos · :1383 opacity_cell (gradient tracks are content, unchanged)",
         geom="track 8 bg-quaternary radius-full, fill fg-brand-primary · handle 16 in editor rows (derived; kit handle size not exported) with a 2 px bg-primary border (kit slider-handle-border)",
         st=S(("bg-quaternary", "bg-primary", N, "fg-brand-primary"), ("bg-quaternary", "bg-primary", N, "fg-brand-primary"), ("bg-quaternary", "bg-primary", N, "fg-brand-primary"),
              ("ring", ("bg-quaternary", "bg-primary", N, "fg-brand-primary")))),
    dict(name="Progress/Bar", parts="Fill = the track, Icon = the progress fill, Text = the percentage label.",  draw=w_progress, kit="Progress bar · Label=Right", node=32400,
         studio="panels/check_designs.rs:518-530 progress_row (bar :525, 4 px today; owned by spec K)",
         geom="track 8, radius-full, gap 12 to the label, label text-xs medium (R5; kit text-sm) text-secondary",
         st=S(("bg-quaternary", N, "text-secondary", "fg-brand-primary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Loading/Inline", parts="Icon = the spinner arc, drawn over a bg-tertiary ring; Fill/Border = the chip.",  draw=w_loading, kit="derived from Loading indicator · Style=Line simple (track bg-tertiary, arc fg-brand-primary), sized for a 24 chip", node=78330,
         studio="image_jobs.rs:1241-1262 (Queued… / Working… chip)",
         geom="ring 16, stroke 2 (kit sm: 32, stroke 4) · chip height 24, radius-md 8 (R5), text-xs medium text-secondary",
         st=S(("bg-secondary", "border-secondary", "text-secondary", "fg-brand-primary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="EmptyState/Default", draw=w_empty, kit="Empty state · Icon=Featured icon · Size=sm (background pattern dropped)", node=76795,
         studio="home/mod.rs:2201-2228 · panels/check_designs.rs:508 (spec K) · agent_sample_prompts.rs:250-262",
         geom="icon tile 32 radius-md 8 with 1 px border-primary, gap 6 · title text-sm semibold text-primary · supporting text-xs text-tertiary (INFERENCE: kit text roles not extracted)",
         st=S((N, "border-primary", "text-primary", "fg-quaternary"), "n/a", "n/a", "n/a", "n/a")),
    dict(name="Divider/Hairline", draw=w_divider, kit="derived: the 1 px border-secondary line of _Modal header · Divider=True and _Dropdown menu list item · Divider=True", node=81903,
         studio="widgets/mod.rs:56-62 hline / vline (119 calls in 39 files)",
         geom="1 px, border-secondary; panel edges keep their own role from spec A",
         st=S(("border-secondary", N, N, N), "n/a", "n/a", "n/a", "n/a")),
]

# ---------------------------------------------------------------- board and tables
def resolve(w, state):
    v = w["st"][state]
    if v == "half":
        return ("half", w["st"]["Default"] if isinstance(w["st"]["Default"], tuple) else w["st"]["Default"][1])
    return v


def cell(w, state):
    v = resolve(w, state)
    args = w.get("args", {})
    if v == "n/a":
        return frame("n/a", [text("n/a", 12, "text-placeholder", 400)], w=190, h=56, main="center")
    if isinstance(v, tuple) and v[0] == "ring":
        inner = ring(w["draw"](v[1], **args), w.get("radius", 8))
    elif isinstance(v, tuple) and v[0] == "half":
        inner = frame("disabled", [w["draw"](v[1], **args)], pad=(4, 4, 4, 4), opacity=0.5)
    else:
        inner = frame("no ring", [w["draw"](v, **args)], pad=(4, 4, 4, 4))
    return frame(state, [inner], w=190, pad=(6, 0, 6, 0), main="center")


def board():
    head = frame("columns", [frame("label col", [text("Widget (spec C anatomy)", 12, "text-tertiary", 600)], w=260)] +
                 [frame(s, [text(s, 12, "text-tertiary", 600)], w=190, main="center") for s in STATES])
    rows = [head, frame("rule", fill="border-secondary", h=1, w=260 + 190 * 6)]
    for w in W:
        label = frame("label", [text(w["name"], 14, "text-primary", 600),
                                text(w["kit"][:44] + ("…" if len(w["kit"]) > 44 else ""), 11, "text-tertiary", 400, name="kit ref")],
                      direction="column", gap=2, w=260, align="start")
        rows.append(frame(w["name"], [label] + [cell(w, s) for s in STATES], align="center"))
        rows.append(frame("rule", fill="border-secondary", h=1, w=260 + 190 * 6))
    title = frame("title", [text("Spec C widget kit — Untitled UI PRO v8.0, Dark mode, R5 density", 20, "text-primary", 600),
                            text("Surface bg-primary #0A0A0A (kit native). Brand = Studio orange #E36323 with #0A0A0A label (R4). "
                                 "Focus ring #F97316, 2 px with a 2 px gap. Disabled = 50 % opacity. Built with designos --apply-spec. Labels off-centre: #1154.", 12,
                                 "text-tertiary", 400, name="legend")], direction="column", gap=4, align="start")
    return frame("Spec C widget kit specimen", [title] + rows, direction="column", gap=0, pad=(32, 32, 32, 32), fill="bg-primary",
                 align="start")


def tok(t):
    return "none" if t is None else f"`{t}` {TOK[t]}"


def md_cell(v, part):
    if v == "n/a":
        return "n/a"
    if isinstance(v, tuple) and v[0] in ("ring", "half"):
        fill, border, bw, txt, ic = look(v[1])
    else:
        fill, border, bw, txt, ic = look(v)
    val = [fill, border, txt, ic][part]
    s = tok(val)
    if part == 1 and val and bw != 1:
        s += f" ({bw:g} px)"
    if isinstance(v, tuple) and v[0] == "half" and val:
        s += " @50 %"
    if isinstance(v, tuple) and v[0] == "ring" and part == 1:
        s += " + ring `focus-ring` #F97316 2 px, gap 2"
    return s


def markdown():
    out = []
    for w in W:
        out.append(f"**{w['name']}** — {w['kit']}\n")
        out.append("| State | Fill | Border | Text | Icon |\n|---|---|---|---|---|")
        for s in STATES:
            v = resolve(w, s)
            if v == "n/a":
                out.append(f"| {s} | n/a | n/a | n/a | n/a |")
            else:
                out.append(f"| {s} | " + " | ".join(md_cell(v, i) for i in range(4)) + " |")
        out.append("")
        if w.get("parts"):
            out.append(f"Parts: {w['parts']}  ")
        na = [s for s in STATES if resolve(w, s) == "n/a"]
        if na:
            out.append("n/a: " + "; ".join(f"{s}: {w.get('na', {}).get(s, NA_REASON[s])}" for s in na) + ".")
        out.append("")
    return "\n".join(out)


NA_REASON = {"Hover": "not a pointer target", "Pressed": "no separate pressed look: a press selects, opens or focuses it",
             "Focused": "not focusable", "Disabled": "never disabled in Studio", "Selected": "has no selected state"}


def inventory():
    out = ["| Anatomy (registry name) | Studio today (file:line at 205c72ef) | Untitled component · variant (node id in the kit copy) | Geometry at R5 |",
           "|---|---|---|---|"]
    for w in W:
        node = f" (node {w['node']})" if w.get("node") else ""
        new = " (**new name**)" if w["name"] not in REGISTERED else ""
        out.append(f"| `{w['name']}`{new} | {w['studio']} | {w['kit']}{node} | {w['geom']} |")
    return "\n".join(out)


REGISTERED = {"Badge/StatusCount", "Button/Primary", "Button/Secondary", "Dialog/CommandPalette", "Input/PropertyField"}

if __name__ == "__main__":
    if sys.argv[1:] == ["--md"]:
        print(markdown())
    elif sys.argv[1:] == ["--inventory"]:
        print(inventory())
    elif sys.argv[1:] == ["--json"]:  # test data for every_table_row_matches_its_look (spec C section 10)
        rows = {}
        for w in W:
            rows[w["name"]] = {}
            for st in STATES:
                v = resolve(w, st)
                if v == "n/a":
                    rows[w["name"]][st] = None
                    continue
                kind = v[0] if isinstance(v, tuple) and v[0] in ("ring", "half") else "plain"
                fill, border, bw, txt, ic = look(v[1] if kind != "plain" else v)
                rows[w["name"]][st] = {"fill": fill, "border": border, "border_w": bw, "text": txt, "icon": ic,
                                       "ring": kind == "ring", "alpha": 0.5 if kind == "half" else 1.0}
        print(json.dumps({"tokens": TOK, "rows": rows}, indent=1, ensure_ascii=False))
    else:
        json.dump(board(), open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
