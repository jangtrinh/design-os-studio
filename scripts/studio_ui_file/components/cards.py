"""Registry `composite` records of the Cards row on page `03 Studio` (row `Cards`): Card/Library and Card/FileThumbnail. Card/StarterTemplate and EmptyState/Default are in
`cards_templates.py`. A record with no registry variants is a plain COMPONENT (rule (j)); the painter's other states (hover, selected, missing) are gaps, not variants.

Card/Library
* Painter: `library_card` (panels/assets.rs:397-415). Callers: 1, `panels/assets.rs:550-554` (the card rect is `region.width - 2 * INSET_PANEL` = 208 wide, 86 high).
* Nested sets: none. `Icon::Component` is an `Icon/Component` instance (an icon, not a set `uses`). The registry's `composes=Tag/Chip,Toggle/Switch` is spec intent: the
  painter draws neither (rule (k)).
* Local parts: the whole card. It is not auto layout: the title and the count are centred 11 and 26 px below the preview (`preview_r.bottom() + 11 / + 26`), 15 apart, while a
  line of either font is 18 high, so their line boxes overlap by 3 px and no stack can express it. The three parts are placed with `abs`; the x offset 2 is a literal of the
  Rust (`card_r.left() + 2.0`), not `INSET_SEGMENT`, so no padding is bound to it.
* Gaps: hover fill `BG_SECONDARY_HOVER` (assets.rs:402; no registry state); the singular "1 component" (a data string).

Card/FileThumbnail
* Painter: `card` (home/mod.rs:2775-2817) with `thumb_area` (:2702-2735) and `draw_file_type_badge` (:2755-2766). Callers: 2, `files_view` (:2643) in grid view and
  `render_file_items` (:2290-); in list view the same file draws `list_row` (:2824), a second painter.
* Nested sets (live): Icon/Tile `Kind=Design, Size=Sm` (`draw_file_type_badge` is the painter Icon/Tile Sm follows: a 20 px `RADIUS_CONTROL` tile with a 12 px glyph, centred at
  (18, thumb.bottom + SPACE_GROUP + LINE_TEXT_SM / 2)) and Divider/Hairline (`widgets::hline(.., thumb.left() + 1, thumb.right() - 1, thumb.bottom(), BORDER)` is the painter Divider/Hairline
  follows, 222 wide at y 140). The registry's `composes=Icon/Tile,Badge/StatusCount` lists a badge: `card` paints none for a design file (`board_badge`, :2743, is Board only).
* Local parts: the card (`RADIUS_XL` fill `BG_SECONDARY`, a 1 px `BORDER` stroke inside), the thumbnail (140 = `HOME_THUMB_H`, `HOME_THUMB_BG`, all four corners at `RADIUS_XL`, the 24 px
  `Icon::File` the painter draws when it has no texture; the thumbnail image is content, so the default draws the glyph) and the two text lines, placed with `abs` (the title centre is
  162 in a 20 px line, the meta line 10 + 9 below it in an 18 px line; the meta text does not follow from the title's box in a stack).
* Meta style: `font_text(FONT_SIZE_TEXT_XS, Regular)` is the `Text xs / Regular` style (text_styles_plan.ROLES).
* Gaps: hover (`BORDER_STRONG` stroke) and selected (2 px `BORDER_SELECTED`) strokes, the missing file (meta `TEXT_ERROR` "File not found" and the Remove button), the hover star on a
  `BG_OVERLAY` disc (:2380-2385), the Board file (`BOARD_BG` thumbnail, dot grid, Sticky glyph, board badge, Board tile), the picture inset (6 px pad, 4 px inner radius, a border) and the
  elided title and meta (`widgets::elide`).
"""
from rest_ir import F, I, T, X, lit

CARD_W, CARD_H, PREVIEW_H = 208, 86, 52  # `region.width - 2 * INSET_PANEL` (240 - 32), the caller's 86 (assets.rs:550-553), `Vec2::new(card_r.width(), 52.0)` (:401)
TITLE_Y, COUNT_Y = PREVIEW_H + 11 - 9, PREVIEW_H + 26 - 9  # centre y (`preview_r.bottom() + 11 / + 26`, :406, :408) less half an 18 px line (LINE_BODY / LINE_CAPTION)
TEXT_X = 2  # `card_r.left() + 2.0` (:406, :408)
CARD_STROKE = "home/mod.rs:2789 `Stroke::new(1.0, theme::BORDER)` (origin/main numbering): a literal of the Rust (BORDER_WIDTH is also 1, but `card` does not name it)"


def library_card(values, env):
    preview = F("Preview", [I("Icon", "Component", 18, env.c("ICON_TERTIARY"))], dir="H", align=("center", "center"), fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"),
                radius=env.c("RADIUS_CONTROL"), w=CARD_W, h=PREVIEW_H, sx="fixed", sy="fixed", abs=(0, 0, CARD_W, PREVIEW_H))
    title = T("Title", "Created in this file", "Body / Semibold", env.c("TEXT"), prop="Title", abs=(TEXT_X, TITLE_Y, 112, 18))
    count = T("Count", "12 components", "Caption / Regular", env.c("TEXT_TERTIARY"), prop="Count", abs=(TEXT_X, COUNT_Y, 78, 18))
    return F("Card", [preview, title, count], w=CARD_W, h=CARD_H, sx="fixed", sy="fixed")


def file_thumbnail(values, env):
    w, thumb_h = env.num("HOME_CARD_W"), env.num("HOME_THUMB_H")
    meta_h = env.num("SPACE_GROUP") + env.num("LINE_TEXT_SM") + env.num("LINE_TEXT_XS") + env.num("SPACE_GROUP")  # CARD_META_H (home/mod.rs:1289)
    thumb = F("Thumbnail", [I("Icon", "File", 24, env.c("ICON_TERTIARY"))], dir="H", align=("center", "center"), fill=env.c("HOME_THUMB_BG"), radius=env.c("RADIUS_XL"),
              w=w, h=thumb_h, sx="fixed", sy="fixed", abs=(0, 0, w, thumb_h))
    line = X("Divider", "Divider/Hairline", "State=Default", abs=(1, thumb_h, w - 2, env.num("BORDER_WIDTH")))
    title_y = thumb_h + env.num("SPACE_GROUP")  # the title's 20 px line starts SPACE_GROUP below the thumbnail (y_title is its centre)
    tile = X("File type", "Icon/Tile", "Kind=Design, Size=Sm", abs=(8, title_y, 20, 20))  # badge_c.x = r.left() + 18, a 20 px tile
    title = T("Title", "Design file", "Text sm / Semibold", env.c("TEXT"), prop="Title", abs=(34, title_y, 80, env.num("LINE_TEXT_SM")))
    meta = T("Meta", "Edited just now \u00b7 In Drafts", "Text xs / Regular", env.c("TEXT_TERTIARY"), prop="Meta", abs=(34, title_y + env.num("LINE_TEXT_SM"), 150, env.num("LINE_TEXT_XS")))
    return F("Card", [thumb, line, tile, title, meta], fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=lit(1, CARD_STROKE), radius=env.c("RADIUS_XL"),
             w=w, h=thumb_h + meta_h, sx="fixed", sy="fixed")


def component_sets():
    return [
        {"name": "Card/Library", "row": "Cards", "axes": {}, "build": library_card,
         "props": [{"name": "Title", "type": "TEXT", "default": "Created in this file"}, {"name": "Count", "type": "TEXT", "default": "12 components"}]},
        {"name": "Card/FileThumbnail", "row": "Cards", "axes": {}, "build": file_thumbnail, "uses": ["Divider/Hairline", "Icon/Tile"],
         "props": [{"name": "Title", "type": "TEXT", "default": "Design file"}, {"name": "Meta", "type": "TEXT", "default": "Edited just now \u00b7 In Drafts"}]},
    ]
