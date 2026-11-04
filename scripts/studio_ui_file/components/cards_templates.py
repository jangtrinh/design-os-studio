"""Card/StarterTemplate (row `Cards`) and EmptyState/Default (row `Bars & Feedback`) of page `03 Studio`. Card/Library and Card/FileThumbnail are in `cards.py`. Card/StarterTemplate
has no registry variants, so it is a plain COMPONENT (rule (j)); EmptyState/Default is a set of one variant (State=Default).

Card/StarterTemplate
* Painter: the template-card loop of `templates_view` (home/mod.rs:1634-1815; the card rect is `tcw` 240 by `tch` = 188 + SPACE_GROUP + SPACE_FACT = 204, :1688). Callers: 1
  (`templates_view`, the cards of its `cards` list, :1642-); there is no capture of the Templates view (`home-templates` is the sidebar row), so every variant is UNVERIFIED-VISUAL.
* The card drawn is the first one, `Blank design`: `TileKind::Design`, `Icon::Pen`, badge `Design File`, text `Blank vector & auto-layout canvas`, button `New file`. The other four
  cards differ in data only (tile kind and glyph, strings, action): gaps, because the registry gives the record no variants.
* Nested sets: Button/Secondary `Size=Sm, State=Default`: `paint_secondary` (home/mod.rs:1591-1603) is `kit::button_look(Secondary, Default)` + `paint_frame` at `RADIUS_CONTROL` and a centred `Text sm` Semibold
  label, the Button/Secondary Sm painter at `CONTROL_SM` (36) high but `card.width - 2 * SPACE_GROUP` wide (:1722-1725). The copy fills the card's content width (`X(.., sx="fill")`: `layout_child` Fill and its
  own sizing Fixed, as the editor writes a Fill child; the main hugs, and a Fill copy that kept the main's own Hug was re-hugged by the engine to 60.75 px, so the button was drawn locally until the
  copy wrote its own sizing). The button is the card's one in-flow child: the card bottom-aligns it with `SPACE_GROUP` padding on three sides. Deviations: the main carries an off-by-default
  `Has Leading Icon` / `Icon` pair the painter has no counterpart for (the kit paints no icon in a text button); the label is the nested button's own `Label` property, so the card has no `Action`
  TEXT property of its own (a set's property cannot drive the text of an instance nested in it). The hover state is a gap (`hot` -> State::Hover, no registry state).
* Local parts: the banner (72 high, inset by the 1 px stroke, filled `tile.fill`; only its top corners are rounded in the Rust, at `concentric(RADIUS_XL, 1)` = 11: a per-corner radius,
  `lit(11)` on the two top corners; the card still clips it at its own radius 12), the 28 px icon box (`tile.fill`, a `BORDER` stroke, `RADIUS_CONTROL`, a 15 px glyph; no Icon/Tile size is 28), the 20 px badge pill
  (`cr_pill(20)` = 10, text + 14 wide, `Text xs` Semibold in `tile.icon`, right edge 12 from the card), the two 4 px bars (`RADIUS_MARK`, 64 and 40 wide, 14 above the banner's bottom) and the two
  text lines. They are placed with `abs`: the title is centred 16 below the banner (a 20 px line) and the description starts 28 below it, which no stack gives. The badge's x is the right edge less
  its text width: the default label fixes it, and a longer label grows it rightward.
* Styles: the description is `Text xs / Regular` and the badge `Text xs / Semibold` (text_styles_plan.ROLES).
* Gaps: hover (`BORDER_STRONG` at 1.5 px), the Board, Deck, Starter and Spec cards, a description that wraps, the elided title, the optional fifth card (`SPEC_BUNDLE_ENTRY_ENABLED`).

EmptyState/Default
* Painter: the Home `empty_state` (home/mod.rs:2450-2474). Callers: 2, `files_view` (:2481) and :2916. Kit `empty_state` (widgets/kit/feedback.rs:77) has 0 callers and is a gap.
* Nested sets (live): Icon/Tile `Kind=Neutral, Size=Md` (`tile_look(Neutral)` + `paint_frame` at `RADIUS_CONTROL`, a 32 px = `CONTROL_XS` square with the 16 px `Icon::File`, the tile Icon/Tile Md Neutral
  is drawn from, home/mod.rs:2466-2468). The registry's `composes=Button/Secondary` is spec intent: the painter draws no button.
* Local parts: the column. The tile centre is 44 above the area's centre, the title line (`LINE_TEXT_MD`, 24) is centred on it and the body starts `LINE_TEXT_MD / 2 + SPACE_FACT` below it, so
  the tile ends 44 - 16 - 12 = 16 above the title's line (a difference of a literal and two constants: `lit`) and the body sits `SPACE_FACT` under it. A heading of tile and title carries the 16.
* Texts: title `Text md` Semibold `TEXT`, body `Text sm` Regular `TEXT_TERTIARY`, both centred on the area. The strings are the caller's by section; the default is the Recents one (:2394-2396).
  The body is wrapped at 380 (`min(380, width - 48)`, :2470-2471):
  the painter places the galley at `c.x - galley.size().x / 2`, so the BLOCK is centred and its LINES are left-aligned (egui's `layout`,
  no `halign`); the capture of the empty Home shows it (line 1 starts at x 667, line 2 at 668 in a 1440 px window), so the body is a left-aligned 380 px text box that the column centres, and
  `align_h="center"` is not used. The box is the cap, not the galley's own width (the longest line, 367 px here), so the block sits 6 px left of production's.
* Gaps: a wrap width that shrinks to the longest line (the galley is `min(380, ..)` wide at most and as wide as its longest line), the section strings (No files yet, Nothing starred, This project is empty, Trash is empty,
  No files match "...": data of the caller), kit `empty_state`, and the second painter `panels/check_designs.rs:548-556` (a 20 px `SUCCESS` check, `No issues found` in `Body` Semibold and a `Body`
  `TEXT_SECONDARY` note, laid out at `EMPTY_ICON_Y` / `EMPTY_TITLE_Y` / `EMPTY_NOTE_Y`): a different look, recorded and not drawn.
"""
from rest_ir import F, I, T, X, lit

BODY = "Create a design file, FigJam board, or import a .fig, .svg or .designos file to get started."  # the Recents / Drafts / All projects / Templates / Resources default (home/mod.rs:2454-2456)
BODY_W = 380  # `380.0_f32.min(area.width() - 48.0)` (home/mod.rs:2470-2471): the wrap width on any area of 428 px or more
CARD_W, CARD_H = 240, 204  # `tcw`, and `188.0 + SPACE_GROUP + SPACE_FACT` (home/mod.rs:1688)
BADGE_LABEL_W = 64  # the default label `Design File` at 12 px Semibold as designos-mcp 0.7.5 measures it: the pill's x is `card.right - (text + 14) - 12` (:1696-1697)
CARD_STROKE = "home/mod.rs:1730 `Stroke::new(if hot { 1.5 } else { 1.0 }, ..)` (origin/main numbering): literals of the Rust, 1.0 at rest (BORDER_WIDTH is also 1, but the card does not name it)"
BANNER_TOP = "home/mod.rs:1733-1734 (origin/main numbering) `theme::concentric(theme::RADIUS_XL as f32, 1.0) as u8`: the card radius less the 1 px stroke inset = 11, computed from two values, no radius role holds it; only the top corners are rounded"
PILL = "home/mod.rs:1741-1742 theme::cr_pill(20) = radius_full(20).round() = 10: computed from the pill's height, no radius role holds it"
PILL_PAD = "home/mod.rs:1740-1741: the pill is text_width + 14 wide, so 7 px of padding each side: a literal in the Rust"
TILE_TO_TITLE = "home/mod.rs:2466,2469: the tile centre is 44 above the area's centre and the title line is centred on it: 44 - CONTROL_XS / 2 - LINE_TEXT_MD / 2 = 16, a difference with no variable"


def starter_template(values, env):
    fill, glyph = env.c("TILE_DESIGN_BG"), env.c("TILE_DESIGN_GLYPH")
    border, edge = env.c("BORDER"), env.c("BORDER_WIDTH")
    banner = F("Banner", fill=fill, radius=[lit(11, BANNER_TOP), lit(11, BANNER_TOP), None, None], w=CARD_W - 2, h=72, sx="fixed", sy="fixed", abs=(1, 1, CARD_W - 2, 72))
    box = F("Icon box", [I("Icon", "Pen", 15, glyph, swap="Icon")], dir="H", align=("center", "center"), fill=fill, stroke=border, sw=edge, radius=env.c("RADIUS_CONTROL"),
            w=28, h=28, sx="fixed", sy="fixed", abs=(12, 14, 28, 28))
    pill_w = BADGE_LABEL_W + 14
    pill = F("Badge", [T("Label", "Design File", "Text xs / Semibold", glyph, prop="Badge")], dir="H", fill=fill, stroke=border, sw=edge, radius=lit(10, PILL),
             pad=[None, lit(7, PILL_PAD), None, lit(7, PILL_PAD)], h=20, sx="hug", sy="fixed", abs=(CARD_W - pill_w - 12, 16, pill_w, 20))
    bars = [F(name, fill=glyph, radius=env.c("RADIUS_MARK"), w=w, h=4, sx="fixed", sy="fixed", abs=(x, 59, w, 4)) for name, x, w in (("Bar short", 12, 64), ("Bar long", 82, 40))]
    title = T("Title", "Blank design", "Text sm / Semibold", env.c("TEXT"), prop="Title", abs=(12, 79, 84, env.num("LINE_TEXT_SM")))
    desc = T("Description", "Blank vector & auto-layout canvas", "Text xs / Regular", env.c("TEXT_TERTIARY"), prop="Description", wrap=True, w=CARD_W - 24, abs=(12, 101, CARD_W - 24, 18))
    button = X("Action", "Button/Secondary", "Size=Sm, State=Default", props={"Label": "New file"}, sx="fill")
    side = env.c("SPACE_GROUP")
    return F("Card", [banner, box, pill, *bars, title, desc, button], dir="V", align=("end", "start"), fill=env.c("BG_SECONDARY"), stroke=border, sw=lit(1, CARD_STROKE),
             radius=env.c("RADIUS_XL"), clip=True, pad=[None, side, side, side], w=CARD_W, h=CARD_H, sx="fixed", sy="fixed")


def empty_state(values, env):
    tile = X("Tile", "Icon/Tile", "Kind=Neutral, Size=Md")
    title = T("Title", "No files yet", "Text md / Semibold", env.c("TEXT"), prop="Title")
    heading = F("Heading", [tile, title], dir="V", gap=lit(16, TILE_TO_TITLE), align=("start", "center"))
    body = T("Body", BODY, "Text sm / Regular", env.c("TEXT_TERTIARY"), prop="Body", wrap=True, w=BODY_W, sx="fixed")
    return F("Empty state", [heading, body], dir="V", gap=env.c("SPACE_FACT"), align=("start", "center"))


def component_sets():
    return [
        {"name": "Card/StarterTemplate", "row": "Cards", "axes": {}, "build": starter_template, "uses": ["Button/Secondary"],
         "props": [{"name": "Title", "type": "TEXT", "default": "Blank design"}, {"name": "Badge", "type": "TEXT", "default": "Design File"},
                   {"name": "Description", "type": "TEXT", "default": "Blank vector & auto-layout canvas"}, {"name": "Icon", "type": "INSTANCE_SWAP", "default": "Pen"}]},
        {"name": "EmptyState/Default", "row": "Bars & Feedback", "axes": {"State": ["Default"]}, "build": empty_state, "uses": ["Icon/Tile"],
         "props": [{"name": "Title", "type": "TEXT", "default": "No files yet"}, {"name": "Body", "type": "TEXT", "default": BODY}]},
    ]
