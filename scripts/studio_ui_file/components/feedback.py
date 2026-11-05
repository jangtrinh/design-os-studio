"""Registry `component` records that sit beside the buttons: the file-kind tile, the selection chip, the inline loading chip, the
progress row and the tooltip. Each follows the painter 0.7.5 calls (D5: production is the truth); where it differs from the kit
painter or spec C the docstring names the Rust line and the report lists the gap. kit-states.json describes the kit painters, so
the chip, the loading chip and the progress row take their colours straight from the live constants (`env.c`).
"""
from rest_ir import F, I, T, lit
TILES = {"Design": ("TILE_DESIGN_BG", "TILE_DESIGN_GLYPH", "Pen"), "Board": ("TILE_BOARD_BG", "TILE_BOARD_GLYPH", "Sticky"),
         "Deck": ("TILE_DECK_BG", "TILE_DECK_GLYPH", "Play"), "Spec": ("TILE_SPEC_BG", "TILE_SPEC_GLYPH", "Specs"),
         "Neutral": (None, "ICON_QUATERNARY", "File")}
PILL = "theme::cr_pill(h) = h / 2, computed from the element's height: no radius role holds it"


def icon_tile(values, env):
    """A file-kind tile. Sm (20 px, glyph 12) is `draw_file_type_badge` (home/mod.rs:2752-2766: Pen, Sticky, Play, Specs by kind). Md is
    32 px (CONTROL_XS): coloured kinds are the Home hub cards' tile with a 17 px glyph (home/mod.rs:2151-2154), Neutral is the Home
    empty-state tile with a 16 px File glyph and a BORDER_STRONG outline (home/mod.rs:2402-2405). No painter draws a Neutral Sm."""
    bg, glyph, icon = TILES[values["Kind"]]
    sm = values["Size"] == "Sm"
    side = 20 if sm else int(env.num("CONTROL_XS"))
    size = 12 if sm else (int(env.num("LAYER_ICON_SIZE")) if bg is None else 17)
    border = {"stroke": env.c("BORDER_STRONG"), "sw": env.c("BORDER_WIDTH")} if bg is None else {}
    return F("Tile", [I("Glyph", icon, size, env.c(glyph), swap="Icon")], dir="H", fill=env.c(bg) if bg else None,
             radius=env.c("RADIUS_CONTROL"), w=side, h=side, sx="fixed", sy="fixed", **border)


def tag_chip(values, env):
    """The live selection chip, `paint_selection_chip` (agent_selection_chip.rs:161-187): a CONTROL_DENSE (24) pill, `cr_pill(24)`, filled
    BG_SELECTED (BG_SELECTED_HOVER on hover), no border and no close glyph, the label caption regular TEXT at INSET_CONTROL. It paints no
    focus ring, so State=Focus has the Default look. `kit::tag` (badge.rs:82-96: bordered, close glyph, ring) has no caller."""
    fill = env.c("BG_SELECTED_HOVER" if values["State"] == "Hover" else "BG_SELECTED")
    return F("Chip", [T("Label", "Tag", "Caption / Regular", env.c("TEXT"), prop="Label")], dir="H", fill=fill, radius=lit(env.num("CONTROL_DENSE") / 2, PILL),
             pad=[None, env.c("INSET_CONTROL"), None, env.c("INSET_CONTROL")], h=env.num("CONTROL_DENSE"), sx="hug", sy="fixed")


GAP12 = "image_jobs.rs:1254-1268: chip_w = status_w + 8 + cancel_w + 12, the label starts at 6 and the Cancel pill ends INSET_SEGMENT from the edge, which leaves 12 between them; a sum of literals"
SIX = "image_jobs.rs:1251-1263: the label starts at INSET_CONTROL - INSET_SEGMENT = 6 and the Cancel text sits 6 inside its pill (cancel_w = text + 12); a sum, no variable"


def loading_inline(values, env):
    """`layer_row_progress_chip` (image_jobs.rs:1248-1283, the pill on a layer row with an AI image job): a 20 px chip, `cr_pill(20)`,
    BG_SECONDARY with a 1 px BORDER, the caption label TEXT_SECONDARY at 6 from the left, and a `Cancel` text action (a 16 px frame, 6 px
    inside, ending INSET_SEGMENT from the right). No ring. Hovering Cancel paints a BG_HOVER pill and TEXT; the registry lists only
    State=Default for this set, so that state is not drawn (it needs a registry variant). `kit::loading_ring` has no caller."""
    cancel = F("Cancel", [T("Action", "Cancel", "Caption / Regular", env.c("TEXT_SECONDARY"))], dir="H", pad=[None, lit(6, SIX), None, lit(6, SIX)],
               h=16, sx="hug", sy="fixed")
    return F("Chip", [T("Label", "Working…", "Caption / Regular", env.c("TEXT_SECONDARY"), prop="Label"), cancel], dir="H",
             gap=lit(12, GAP12), pad=[None, env.c("INSET_SEGMENT"), None, lit(6, SIX)], fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"),
             sw=env.c("BORDER_WIDTH"), radius=lit(10, PILL), h=20, sx="hug", sy="fixed")


def progress_bar(values, env):
    """`progress_row` (panels/check_designs.rs:558-593): a 296 px row (PANEL_W - 2 * PAD), 32 high (PROGRESS_H). The bar sits at y 4
    (PROGRESS_BAR_Y) and is 8 high (PROGRESS_BAR_H): a BG_PRESSED pill track and an ICON_BRAND_PRIMARY pill fill (`kit::progress_bar`,
    feedback.rs:46-55). The label sits BELOW it at y 15 (PROGRESS_LABEL_Y): caption regular TEXT_SECONDARY, the two counts monospace at
    caption size in TEXT (`FontId::new(FONT_SIZE_CAPTION, Monospace)`, panels/check_designs.rs:569, which is not `font_mono` (FONT_SIZE_BODY): the
    style is `Mono / Caption`, bound to the caption size and line variables). A BORDER_SUBTLE hairline closes the row at y 31."""
    fill = F("Fill", fill=env.c("ICON_BRAND_PRIMARY"), radius=lit(4, PILL), w=118.4, h=8, sx="fixed", sy="fixed")
    track = F("Track", [fill], fill=env.c("BG_PRESSED"), radius=lit(4, PILL), w=296, h=8, sx="fixed", sy="fixed", abs=(0, 4, 296, 8))
    cap = lambda name, text: T(name, text, "Caption / Regular", env.c("TEXT_SECONDARY"), **({"prop": "Label"} if name == "Prefix" else {}))
    mono = lambda name, text, prop: T(name, text, "Mono / Caption", env.c("TEXT"), prop=prop)
    label = F("Label", [cap("Prefix", "Checking… "), mono("Scanned", "40", "Scanned"), cap("Of", " of "), mono("Total", "100", "Total"), cap("Unit", " layers")],
              dir="H", sx="hug", sy="hug", abs=(0, 15, 200, 18))
    line = F("Hairline", fill=env.c("BORDER_SUBTLE"), w=296, h=env.num("BORDER_WIDTH"), sx="fixed", sy="fixed", abs=(0, 31, 296, 1))
    return F("Progress", [track, label, line], w=296, h=32, sx="fixed", sy="fixed")


def tooltip(values, env):
    """kit surface.rs tooltip_frame: BG_PRIMARY_SOLID, RADIUS_CONTROL, margin INSET_FRAME / INSET_BUTTON, no stroke, TEXT in the stock
    egui tooltip font (Body regular). Spec C says text-xs semibold: the Rust wins."""
    return F("Tooltip", [T("Text", "Tooltip", "Body / Regular", env.c("TEXT"), prop="Text")], dir="H", fill=env.c("BG_PRIMARY_SOLID"),
             radius=env.c("RADIUS_CONTROL"), pad=[env.c("INSET_FRAME"), env.c("INSET_BUTTON"), env.c("INSET_FRAME"), env.c("INSET_BUTTON")], sx="hug", sy="hug")


def component_sets():
    label = lambda default: [{"name": "Label", "type": "TEXT", "default": default}]
    return [
        {"name": "Icon/Tile", "row": "Badges & Status", "axes": {"Kind": list(TILES), "Size": ["Sm", "Md"]},
         "props": [{"name": "Icon", "type": "INSTANCE_SWAP", "default": "Pen"}], "build": icon_tile},
        {"name": "Tag/Chip", "row": "Badges & Status", "axes": {"Size": ["Sm"], "State": ["Default", "Hover", "Focus"]},
         "props": label("Tag"), "build": tag_chip},
        {"name": "Loading/Inline", "row": "Badges & Status", "axes": {"State": ["Default"]}, "props": label("Working…"), "build": loading_inline},
        {"name": "Progress/Bar", "row": "Badges & Status", "axes": {"State": ["Default"]},
         "props": [{"name": "Label", "type": "TEXT", "default": "Checking… "}, {"name": "Scanned", "type": "TEXT", "default": "40"},
                   {"name": "Total", "type": "TEXT", "default": "100"}], "build": progress_bar},
        {"name": "Tooltip/Default", "row": "Menus & Overlays", "axes": {"State": ["Default"]},
         "props": [{"name": "Text", "type": "TEXT", "default": "Tooltip"}], "build": tooltip},
    ]
