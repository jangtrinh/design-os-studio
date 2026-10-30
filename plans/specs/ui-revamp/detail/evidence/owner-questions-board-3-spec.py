"""Owner-questions board 3: the six questions only the owner can decide (plan.md "Still the owner's"), options side by
side, Untitled UI dark at R5 editor density, Studio orange brand. One NodeSpec tree for `designos --apply-spec`.

Usage:
  python3 owner-questions-board-3-spec.py board.json
  designos <scratch>.designos --apply-spec board.json        # then designos_render_png node 2, scale 2

Colours are token-contract.md names and values; the Home utility tiles (Q2, option A) are kit values from the resolved
Dark-mode export (matched 273/273 by evidence/figma-live-variable-check.md) and are not yet contract roles. Contrast
ratios are computed here (WCAG 2.x); none is typed. NodeSpec has no text alignment, so labels in fixed boxes may sit
off-centre (#1154); the board says so in its caption.
"""
import json, sys

T = {  # token-contract.md (a) values
    "BG": "#0A0A0A", "BG_SECONDARY": "#171717", "BG_TERTIARY": "#262626", "BG_SELECTED": "#262626", "BG_PRESSED": "#404040",
    "BG_SECONDARY_HOVER": "#262626", "HOME_THUMB_BG": "#262626", "BORDER": "#262626", "BORDER_STRONG": "#404040",
    "BORDER_SELECTED": "#FB923C", "TEXT": "#FAFAFA", "TEXT_SECONDARY": "#D4D4D4", "TEXT_TERTIARY": "#A3A3A3",
    "TEXT_ON_BRAND": "#0A0A0A", "ICON_SECONDARY": "#D4D4D4", "ICON_TERTIARY": "#A3A3A3", "BG_BRAND": "#E36323",
    "TEXT_BRAND": "#F97316", "ICON_QUATERNARY": "#737373", "DEV_READY": "#14AE5C", "SUCCESS": "#22C55E", "INFO": "#0D99FF", "PROVIDER_CLAUDE": "#D97757",
    "PROVIDER_CODEX": "#FAFAFA",
    # kit Dark-mode utility steps for H OQ2 option A (tile -50, glyph -600); not contract roles yet
    "utility-blue-50": "#172554", "utility-blue-600": "#60A5FA", "utility-amber-50": "#451A03", "utility-amber-600": "#FBBF24",
    "utility-green-50": "#052E16", "utility-green-600": "#4ADE80", "utility-orange-50": "#431407", "utility-orange-600": "#FB923C",
}
COL_W, WIDE_W, GAP, PAD = 752, 1528, 16, 20


def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def cr(a, b):
    a, b = T.get(a, a), T.get(b, b)
    hi, lo = sorted((lum(a), lum(b)), reverse=True)
    return f"{(hi + 0.05) / (lo + 0.05):.2f}:1"


def text(s, size=12, color="TEXT_SECONDARY", weight=400, name=None):
    return {"kind": "text", "name": name or s[:28], "text": s, "font_size": size, "font_weight": weight, "text_color": T.get(color, color)}


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


def box(label, w, h, fill=None, color="TEXT_SECONDARY", size=12, weight=500, stroke=None, sw=1, radius=8):
    """Fixed-size box with a centred label (the label may sit off-centre: #1154)."""
    return frame("box " + label, [text(label, size, color, weight)], fill=fill, stroke=stroke, sw=sw, radius=radius, w=w, h=h, main="center")


def stage(children, direction="column", gap=8, fill="BG", align="start"):
    return frame("stage", children, direction=direction, gap=gap, pad=(12, 12, 12, 12), fill=fill, stroke="BORDER", radius=8, align=align)


def option(letter, value, rec, content, note, w):
    label = f"{letter} · {value}" + ("  (rec)" if rec else "")
    return frame(f"option {letter}", [content, text(label, 12, "TEXT_BRAND" if rec else "TEXT", 600, name=f"label {letter}"),
                                      text(note, 12, "TEXT_TERTIARY", 400, name=f"note {letter}")], direction="column", gap=6, align="start", w=w)


def card(qid, title, ask, options, w=COL_W):
    n = len(options)
    cell = (w - 2 * PAD - (n - 1) * GAP) // n
    head = frame("head", [text(f"{qid}  {title}", 14, "TEXT", 600), text(ask, 12, "TEXT_TERTIARY")], direction="column", gap=2, align="start")
    return frame(qid, [head, frame("options", [option(*o, w=cell) for o in options], gap=GAP, align="start")], direction="column", gap=14,
                 pad=(PAD, PAD, PAD, PAD), fill="BG", stroke="BORDER", radius=12, w=w, align="start")


# ------------------------------------------------------------------ Q1 editor tab heights (D file tabs, I session tabs, F strip)
def file_tabs(tab_h, bar_h):
    tabs = [box("⌂", 24, 24, color="ICON_TERTIARY", size=13), box("Untitled UI kit", 132, tab_h, fill="BG_SECONDARY", color="TEXT", stroke="BORDER_STRONG"),
            box("VSF - PCP", 96, tab_h, color="TEXT_TERTIARY")]
    return frame("D file tabs", tabs, gap=4, pad=(0, 8, 0, 8), fill="BG", stroke="BORDER", radius=0, w=320, h=bar_h)


def session_tabs(tab_h):
    def tab(name, dot, active):
        return frame("tab " + name, [frame("dot", [], fill=dot, radius=3, w=6, h=6), text(name, 12, "TEXT" if active else "TEXT_TERTIARY", 500)],
                     gap=6, pad=(0, 10, 0, 10), fill="BG_SELECTED" if active else None, radius=8, h=tab_h)
    return frame("I session tabs", [tab("Claude", "PROVIDER_CLAUDE", True), tab("Codex", "PROVIDER_CODEX", False), tab("Shell", "ICON_TERTIARY", False)],
                 gap=4, pad=(0, 8, 0, 8), fill="BG", stroke="BORDER", w=320, h=36)


def segmented(track_h):
    tile_h = track_h - 4
    return frame("F segmented strip", [box("Design", 88, tile_h, fill="BG_SELECTED", color="TEXT", radius=6),
                                       box("Prototype", 88, tile_h, color="TEXT_TERTIARY", radius=6)],
                 gap=0, pad=(2, 2, 2, 2), fill="BG_SECONDARY", radius=8, h=track_h)


def q1_cell(tall):
    rows = [
        ("D file tabs, bar " + ("40" if tall else "32"), file_tabs(32 if tall else 26, 40 if tall else 32)),
        ("I session tabs in the 36 bar", session_tabs(32 if tall else 24)),
        ("F Inspector strip", segmented(32 if tall else 24)),
    ]
    kids = []
    for cap, node in rows:
        kids += [text(cap, 12, "TEXT_TERTIARY"), node]
    return stage(kids, gap=6)


# ------------------------------------------------------------------ Q2 Home category colours
KINDS = [("Design file", "D", "utility-blue"), ("Board", "B", "utility-amber"), ("Deck", "K", "utility-green"), ("Spec bundle", "S", "utility-orange")]


def home_card(name, glyph, tile, ink):
    meta = frame("meta", [box(glyph, 20, 20, fill=tile, color=ink, size=11, weight=600, radius=6), text(name, 12, "TEXT", 500)], gap=8, pad=(8, 8, 8, 8))
    return frame("card " + name, [frame("thumb", [], fill="HOME_THUMB_BG", w=150, h=56, radius=0), meta], direction="column", gap=0,
                 fill="BG_SECONDARY", stroke="BORDER", radius=12, w=150, align="start")


def q2_cell(tinted):
    cards, notes = [], []
    for name, g, u in KINDS:
        tile, ink = (f"{u}-50", f"{u}-600") if tinted else ("BG_TERTIARY", "ICON_SECONDARY")
        cards.append(home_card(name, g, tile, ink))
        notes.append(f"{g} {cr(ink, tile)}")
    disc_tile, disc_ink = ("utility-blue-50", "utility-blue-600") if tinted else ("BG_TERTIARY", "ICON_SECONDARY")
    side = frame("sidebar heading", [box("P", 20, 20, fill=disc_tile, color=disc_ink, size=11, weight=600, radius=10),
                                     text("FILES & PROJECTS", 12, "TEXT_TERTIARY", 500)], gap=8)
    grid = [frame("row 1", cards[:2], gap=8), frame("row 2", cards[2:], gap=8)]
    return stage([side] + grid + [text("glyph on tile: " + " · ".join(notes), 12, "TEXT_TERTIARY")], gap=8)


# ------------------------------------------------------------------ Q3 selected file card border
def file_card(name, border, bw):
    meta = frame("meta", [text(name, 12, "TEXT", 500), text("Edited 2h ago", 12, "TEXT_TERTIARY")], direction="column", gap=2, pad=(8, 8, 8, 8), align="start")
    return frame("card " + name, [frame("thumb", [], fill="HOME_THUMB_BG", w=140, h=64), meta], direction="column", gap=0, fill="BG_SECONDARY",
                 stroke=border, sw=bw, radius=12, w=140, align="start")


def q3_cell(sel):
    return stage([frame("cards", [file_card("Checkout flow", sel, 2), file_card("Onboarding", "BORDER", 1)], gap=12, align="start"),
                  text(f"selected border on page {cr(sel, 'BG')} · on thumb {cr(sel, 'HOME_THUMB_BG')}", 12, "TEXT_TERTIARY")], gap=8)


# ------------------------------------------------------------------ Q4 Dev accent colour
def q4_cell(acc):
    toggle = frame("Dev toggle", [box("Design", 64, 28, color="TEXT_TERTIARY", radius=6), box("Dev", 52, 28, fill=acc, color="TEXT_ON_BRAND", weight=600, radius=6)],
                   gap=0, pad=(2, 2, 2, 2), fill="BG_SECONDARY", radius=8)
    tab = frame("tab with Dev badge", [text("Untitled UI kit", 12, "TEXT", 500), box("Dev", 32, 18, fill=acc, color="TEXT_ON_BRAND", size=11, weight=600, radius=6)],
                gap=8, pad=(0, 10, 0, 10), fill="BG_SECONDARY", stroke="BORDER_STRONG", radius=8, h=26)
    row = frame("Plugins row", [frame("left", [frame("dot", [], fill=acc, radius=4, w=8, h=8), text("Inspect", 12, "TEXT_SECONDARY")], gap=8),
                                text("Ready", 12, acc, 600)], pad=(0, 8, 0, 8), fill="BG", w=220, h=32, main="space-between")
    return stage([toggle, tab, row, text(f"dark label on fill {cr('TEXT_ON_BRAND', acc)} · accent text on bg {cr(acc, 'BG')}", 12, "TEXT_TERTIARY")], gap=10)


# ------------------------------------------------------------------ Q5 rail help dot (48 px rail after #1106)
def rail(dot):
    def tile(g, sel=False):
        return box(g, 32, 32, fill="BG_SELECTED" if sel else None, color="TEXT" if sel else "ICON_TERTIARY", size=14)
    help_kids = [text("?", 14, "ICON_TERTIARY", 500)] + ([frame("dot", [], fill=dot, radius=3, w=6, h=6)] if dot else [])
    helpb = frame("help", help_kids, gap=1, w=32, h=32, main="center", align="start", pad=(6, 0, 0, 0))
    col = frame("rail 48", [tile("≡"), tile("▤"), tile("◇", True), tile("▦"), frame("spacer", [], w=32, h=40), helpb], direction="column", gap=4,
                pad=(8, 8, 8, 8), fill="BG", stroke="BORDER", w=48)
    note = text(f"dot on rail {cr(dot, 'BG')}" if dot else "no dot", 12, "TEXT_TERTIARY")
    return stage([col, note], gap=8)


# ------------------------------------------------------------------ Q6 website colours (docs/index.html :83-85)
OLD = {"--bg": "#121215", "--panel": "#18181B", "--surface": "#1E1E22", "--hover": "#232329", "--border": "#27272A", "--line": "#52525B",
       "--text": "#F4F4F5", "--muted": "#A1A1AA", "--ink": "#121215"}
NEW = {"--bg": "BG", "--panel": "BG_SECONDARY", "--surface": "BG_TERTIARY", "--hover": "BG_SECONDARY_HOVER", "--border": "BORDER",
       "--line": "BORDER_STRONG", "--text": "TEXT", "--muted": "TEXT_TERTIARY", "--ink": "TEXT_ON_BRAND"}


def site(pal):
    v = {k: T.get(c, c) for k, c in pal.items()}
    sw = [frame("sw " + k, [frame("chip", [], fill=v[k], stroke="#404040", radius=4, w=28, h=28), text(k, 11, "TEXT_SECONDARY", 500), text(v[k], 11, "TEXT_TERTIARY")],
                direction="column", gap=2, w=66, align="start") for k in pal]
    hero = frame("hero", [
        text("Design in Studio", 20, v["--text"], 600), text("A native canvas for agents and people.", 12, v["--muted"]),
        frame("buttons", [box("Download", 96, 32, fill=v["--text"], color=v["--ink"], weight=600), box("Read the docs", 112, 32, stroke=v["--line"], color=v["--text"], weight=600)], gap=8),
        frame("band", [box("designos --home", 128, 24, fill=v["--surface"], color=v["--text"], size=11, radius=6), text("band on --panel", 12, v["--muted"])],
              gap=8, pad=(8, 8, 8, 8), fill=v["--panel"], stroke=v["--border"], radius=8)], direction="column", gap=10, pad=(16, 16, 16, 16),
        fill=v["--bg"], stroke=v["--border"], radius=12, align="start", w=420)
    nums = text(f"--muted on --bg {cr(v['--muted'], v['--bg'])} · on --panel {cr(v['--muted'], v['--panel'])} · --ink on --text {cr(v['--ink'], v['--text'])}",
                12, "TEXT_TERTIARY")
    return stage([frame("swatches", sw, gap=6, align="start"), hero, nums], gap=10)


# ------------------------------------------------------------------ Q7 unchecked checkbox and radio border (spec C OQ1)
def q7_cell(border):
    def control(kind, checked, surface):
        r = 4 if kind == "box" else 8
        if checked:
            inner = [text("✓", 11, "TEXT_ON_BRAND", 700)] if kind == "box" else [frame("dot", [], fill="TEXT_ON_BRAND", radius=3, w=6, h=6)]
            return frame(f"{kind} on", inner, fill="BG_BRAND", radius=r, w=16, h=16, main="center")
        return frame(f"{kind} off", [], fill=surface, stroke=border, sw=1, radius=r, w=16, h=16)
    def row(surface):
        items = []
        for kind, name in (("box", "Checkbox"), ("radio", "Radio")):
            for checked in (False, True):
                items.append(frame(f"{name} {checked}", [control(kind, checked, surface), text(name, 12, "TEXT_SECONDARY")], gap=8))
        return frame("on " + surface, [frame("controls", items, gap=14, pad=(10, 10, 10, 10), fill=surface, stroke="BORDER", radius=8),
                                       text(f"unchecked border on {T[surface]} {cr(border, surface)}", 12, "TEXT_TERTIARY")],
                     direction="column", gap=4, align="start")
    return stage([row("BG"), row("BG_SECONDARY"),
                  text(f"checked: fill on surface {cr('BG_BRAND', 'BG')} · mark on fill {cr('TEXT_ON_BRAND', 'BG_BRAND')}", 12, "TEXT_TERTIARY")], gap=10)


def board():
    q1 = card("Q1", "Editor tab heights (D file tabs, I session tabs, F Inspector strip)",
              "One answer for all three strips. Untitled dark styling either way; only the height differs.", [
                  ("A", "today: D tabs 26 in the 32 bar, I 24, F 24", True, q1_cell(False), "Layout unchanged (R1); D and F recommend it, I follows D."),
                  ("B", "32 everywhere (R5 button xs)", False, q1_cell(True), "D's bar grows 32 → 40, F's content moves 8 px down, I fits its 36 bar."),
              ], w=WIDE_W)
    q2 = card("Q2", "Home category colours (H OQ2)", "File-type tiles, the \"P\" disc; template, hub and quick-create accents follow.", [
        ("A", "utility tiles: blue, amber, green, orange (-50 / -600)", True, q2_cell(True), "Kinds recognisable at a glance; kit Featured icon."),
        ("B", "one neutral tile #262626 / #D4D4D4", False, q2_cell(False), "Maximum restraint; kinds told apart by glyph only."),
    ])
    q3 = card("Q3", "Selected file card border (H OQ3)", "Selected card next to an unselected one (1 px #262626).", [
        ("A", "2 px BORDER_SELECTED #FB923C", True, q3_cell("BORDER_SELECTED"), "Untitled selected-card pattern; reads on dark thumbs."),
        ("B", "2 px BORDER_STRONG #404040", False, q3_cell("BORDER_STRONG"), "Neutral selection (P2); weak on dark thumbnails."),
    ])
    q4 = card("Q4", "Dev mode accent colour (G OQ1)", "Dev toggle, file-tab badge and Plugins row, dark label #0A0A0A.", [
        ("A", "#14AE5C DEV_READY (Figma parity)", True, q4_cell("DEV_READY"), "Mode colour matches the canvas Ready colour."),
        ("B", "#22C55E SUCCESS (contract success role)", False, q4_cell("SUCCESS"), "Dev mode reads as \"success\" in Untitled terms."),
    ])
    q5 = card("Q5", "Rail help dot (D)", "48 px rail after #1106; the dot is always on, no state behind it.", [
        ("A", "remove the dot", True, rail(None), "No state behind it (P6, Zero Fake UI)."),
        ("B", "orange #F97316 TEXT_BRAND", False, rail("TEXT_BRAND"), "D's current mapping (R4)."),
        ("C", "blue #0D99FF (today)", False, rail("INFO"), "Today's literal; D drops chrome blue."),
    ])
    q6 = card("Q6", "Website colours (M OQ1, docs/index.html)", "Main palette roles plus a small hero block; --orange, --blue, --emerald stay in both.", [
        ("A", "Untitled dark neutrals (contract roles)", True, site(NEW), "Site matches the new app screenshots; one fill step per role kept."),
        ("B", "keep today's Zinc palette", False, site(OLD), "Marketing stays as is; screenshots and page disagree."),
    ], w=WIDE_W)
    q7 = card("Q7", "Unchecked checkbox and radio border (C OQ1)", "Checkbox/Box and Radio, unchecked and checked, on bg-primary and bg-secondary.", [
        ("A", "ICON_QUATERNARY fg-quaternary #737373", True, q7_cell("ICON_QUATERNARY"),
         f"Meets the 3:1 non-text rule (WCAG 1.4.11): {cr('ICON_QUATERNARY', 'BG')} / {cr('ICON_QUATERNARY', 'BG_SECONDARY')}."),
        ("B", "kit 1 px BORDER_STRONG border-primary #404040", False, q7_cell("BORDER_STRONG"),
         f"Kit value; below the 3:1 non-text rule: {cr('BORDER_STRONG', 'BG')} / {cr('BORDER_STRONG', 'BG_SECONDARY')}."),
    ], w=WIDE_W)
    rows = [q1, frame("row 2", [q2, q3], gap=24, align="start"), frame("row 3", [q4, q5], gap=24, align="start"), q6, q7]
    title = frame("title", [text("Owner questions, board 3: the seven only the owner can decide", 20, "TEXT", 600),
                            text("R5 editor density · Untitled UI v8 dark · Studio orange · (rec) = designer's recommendation · contrast computed "
                                 "(WCAG 2.x) · token-contract.md names. Labels may sit off-centre: #1154, a renderer bug, not a design choice.", 12, "TEXT_TERTIARY")],
                  direction="column", gap=4, align="start")
    return frame("Owner questions board 3", [title] + rows, direction="column", gap=24, pad=(32, 32, 32, 32), fill="#000000", align="start")


if __name__ == "__main__":
    json.dump(board(), open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
