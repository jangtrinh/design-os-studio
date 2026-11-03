"""Registry `region` Panel/FindReplace (page `03 Studio`), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (find/panel.rs).

Painter: `show` (find/panel.rs:182-317), one caller, which takes the layers tree's place in the left sidebar (240 wide, below the pages list); the capture is `j-find`, the region is
(49, 305, 240, 595) and every rect below is relative to its top-left. State drawn: replace row hidden (`show_replace` false), a query with 19 results, no current hit, scope `This page`.
* Nested sets (live): Button/Close (`widgets::icon_button` close, :191-192, 24 px 8 from the right and from the top), Button/Icon (the replace toggle, :203-206, `icon_button` with
  ChevronRight, SPACE_FACT
  left of the close), Input/Field Xs (`field::input` -> `field_bg` -> `kit::paint_field` Xs, :156-180, the frame is 208 - 70 = 138 wide because the three option toggles share the 208 px row),
  Divider/Hairline (:299, the status row's bottom line).
* Local: the title (Body Semibold, `PANEL_PADDING_X`), the three option toggles (`toggle`, :112-130: a 22 px box, `Aa` / `ab` / `.*` in Body Semibold, `ICON_SECONDARY` at rest, no fill; the
  on and hover
  fills are gaps), the status row (:267-298: `N results` in Caption `TEXT_SECONDARY`, the scope chip `text_width + 12` wide and 22 high, Caption `TEXT_SECONDARY`), the result rows and the
  scrollbar thumb.
  The previous and next buttons (`chevron`, :132-153) are stroked open paths (two `line_segment`s 9 x 5 px, `Stroke::new(1.5, ICON_SECONDARY)`), drawn at the same 24 px boxes, not compared in a crop.
* Results: the SLOT `Results` (a clipped list of 32 px rows from y 112, as the capture shows 7 hits and the next group). A group row (`RowKind::Group`, :366-384) is a 16 px frame glyph in
  `ICON_TERTIARY`
  at `INSET_PANEL` and the name `SPACE_INLINE` after it, Body `TEXT_SECONDARY`; a hit row (:385-425) is indented 16 more, the glyph then the excerpt 8 after it, Body `TEXT`, with the match
  marked behind the
  text by an 18 px high `FIND_HIGHLIGHT` rect at `RADIUS_MARK` (the current hit would take `BG_SELECTED` and `FIND_HIGHLIGHT_SELECTED`: a gap). The excerpt is document data: neutral, `Card
  and button`
  (`button` marked, the same prefix width as the capture's text, measured with Inter 12 px: 55.08 and 36.47 px).
* Gaps: the replace row (:242-265), a failed pattern's `DANGER` message, a page row of `SearchIn::AllPages` (:349-365), the empty and `Type to search` states, hover fills, the current hit.
"""
from components.regions_sidepanels_parts import close_button, hairline, icon_button, label, scrollbar_thumb
from rest_ir import F, I, T, V, X, lit, segments_path

ROW = "Regions"
W, H, LIST_TOP, LIST_H = 240.0, 595.0, 112.0, 483.0
TEXT_X, PREFIX_W, MATCH_W = 56.0, 55.08, 36.47
STEP_STROKE = "find/panel.rs:148 `Stroke::new(1.5, col)`: a raw width (FOCUS_RING_WIDTH is also 1.5, but the call does not name it)"
HIT_INDENT = "find/panel.rs:396-397: `indent` is a raw 16.0 for a hit in a frame, on top of INSET_PANEL: the glyph's left edge is 16 + 16"
HIT_GAP = "find/panel.rs:412 `icon_c.x + LAYER_ICON_SIZE / 2.0 + 8.0`: a raw 8.0 between the glyph and the excerpt"
CHIP_PAD = "find/panel.rs:281 `text_width + 12.0`: the chip is its label plus a raw 12, so 6 a side"


def group_row(env, n):
    icon = "AutoLayoutH" if n <= 4 else "AutoLayoutV"  # the capture's first four frames lay out in a row, the last four in a column
    return F(f"Group {n}", [I("Icon", icon, 16, env.c("ICON_TERTIARY")), T("Name", f"Frame {n}", "Body / Regular", env.c("TEXT_SECONDARY"))], dir="H", gap=env.c("SPACE_INLINE"),
             pad=[None, None, None, env.c("INSET_PANEL")], h=env.num("FIND_GROUP_ROW_H"), sx="fill", sy="fixed", align=("start", "center"))


def hit_row(env, n):
    mark = F("Match", fill=env.c("FIND_HIGHLIGHT"), radius=env.c("RADIUS_MARK"), w=MATCH_W, h=18.0, sx="fixed", sy="fixed", abs=(round(TEXT_X + PREFIX_W, 2), 7.0, MATCH_W, 18.0))
    return F(f"Hit {n}", [mark, I("Icon", "AutoLayoutH", 16, env.c("ICON_TERTIARY")), T("Excerpt", "Card and button", "Body / Regular", env.c("TEXT"))], dir="H", gap=lit(8, HIT_GAP),
             pad=[None, None, None, lit(32, HIT_INDENT)], h=env.num("LAYER_ROW_HEIGHT"), sx="fill", sy="fixed", align=("start", "center"))


def find_panel(values, env):
    rows = [row for n in range(1, 8) for row in (group_row(env, n), hit_row(env, n))] + [group_row(env, 8)]
    toggles = [label(name, (156.0 + 22.0 * i, 45.0, 22.0, 22.0), text, "Body / Semibold", "ICON_SECONDARY", env, align=("center", "center"))
               for i, (name, text) in enumerate((("Match case", "Aa"), ("Whole word", "ab"), ("Regex", ".*")))]
    count = label("Count", (16.0, 80.0, 92.0, 32.0), "19 results", "Caption / Regular", "TEXT_SECONDARY", env, clip=True)
    pad = lit(6, CHIP_PAD)
    scope = F("Scope", [T("Text", "This page", "Caption / Regular", env.c("TEXT_SECONDARY"))], dir="H", pad=[None, pad, None, pad], h=22.0, sx="hug", sy="fixed", abs=(112.0, 85.0, 68.0, 22.0))
    results = F("Results", rows, dir="V", slot="Results", w=W, h=LIST_H, sx="fixed", sy="fixed", clip=True, abs=(0.0, LIST_TOP, W, LIST_H))
    kids = [label("Title", (0.0, 0.0, W, 40.0), "Find and replace", "Body / Semibold", "TEXT", env, pad=[None, None, None, env.c("PANEL_PADDING_X")]),
            icon_button("Replace toggle", "ChevronRight", (180.0, 8.0, 24.0, 24.0)), close_button((208.0, 8.0, 24.0, 24.0)),
            X("Search field", "Input/Field", "Size=Xs, State=Default", props={"Value": "Button"}, abs=(16.0, 40.0, 138.0, 32.0)), *toggles, count, scope,
            step("Previous", True, (184.0, 84.0, 24.0, 24.0), env), step("Next", False, (208.0, 84.0, 24.0, 24.0), env),
            hairline(111.0, W), results, scrollbar_thumb("Scrollbar thumb", (0.0, LIST_TOP, W, LIST_H), 0.0, 196.0, env)]
    return F("Find and replace", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def step(name, up, rect, env):
    """`chevron` (find/panel.rs:132-153): two `line_segment`s, 4.5 wide and 2.5 high either side of the centre of the 24 px box, `Stroke::new(1.5, col)` in `ICON_SECONDARY` at rest."""
    a, b = (14.5, 9.5) if up else (9.5, 14.5)
    return V(name, segments_path([((7.5, a), (12.0, b)), ((12.0, b), (16.5, a))]), 24.0, 24.0, None, stroke=env.c("ICON_SECONDARY"), sw=lit(1.5, STEP_STROKE), abs=rect)


def component_sets():
    return [{"name": "Panel/FindReplace", "row": ROW, "axes": {}, "uses": ["Button/Close", "Button/Icon", "Divider/Hairline", "Input/Field"],
             "props": [{"name": "Results", "type": "SLOT"}], "build": find_panel}]
