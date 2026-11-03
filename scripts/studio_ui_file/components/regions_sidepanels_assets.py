"""Registry `region` Panel/Assets (page `03 Studio`), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (panels/assets.rs).

Painter: `show` (panels/assets.rs:432-698), one caller, the rail's Assets tab: the left panel (240 wide). The capture is `k-assets`, the region is (49, 32, 240, 868) and every rect is relative to
its top-left. State drawn: the list view, the `All` category, the file's own local components shown under their frames (`Line::Container`, `Line::Page`, `Line::Item`), scrolled to the top.
* Nested sets (live): Button/Icon Xs (`kit::button_icon`, :438 the Libraries book at right - 8 - 32, :475 the category filter, :424 the list and grid toggle, 32 px each, 2 apart; the list one is
  Selected), Divider/Hairline (:443, under the 40 px header), Input/Search Xs (:454-467: `field_chrome` Xs, a 14 px glyph centred at left + 14, the text from left + 26: the Home search
  painter's geometry),
  Card/Library (`library_card`, :397-415, 208 x 86) and Button/Secondary Xs (`kit::button`, :569, 208 x `CONTROL_XS`, full width, `w=`).
* Local: the title (`font_large_strong`, TEXT, at `INSET_PANEL`, :436), the category pills (:490-511: `text_width + 14` wide, 20 high, a pill radius of half that, 4 apart, the selected one
  `BG_SECONDARY`
  with Caption Semibold `TEXT`, the others Caption `TEXT_SECONDARY`), the `All libraries` heading (Body Semibold, :515), the list rows and the scrollbar thumb. The list is the SLOT
  `Components`: a clipped column
  from y 266 of 32 px group rows (`GROUP_H`, :66) and 48 px component rows (`ROW_H`, :65). A group row is Body `TEXT_TERTIARY` at `PANEL_PADDING_X` (+ 8 for a container, :641-642); a
  component row is a
  40 px box at `INSET_FRAME` + `INSET_CONTROL` (`BG_SECONDARY`, a 1 px `BORDER_TRANSLUCENT` inside stroke at `RADIUS_CONTROL`, :302-312) with the name in Body `TEXT` `SPACE_GROUP` after it (:340).
  Production renders each component into the box (a lazily rendered thumbnail, :302-311); the drawing keeps the box's own fallback, the 16 px `CANVAS_COMPONENT` component glyph (:310),
  because a thumbnail
  is document content.
* Kit content: the capture shows the library's own page, frame and component names; they are neutral here (`Frame n`, `Button`, `Card` ...) and the count is `12 components` (the capture's
  is the library's own number).
* Deviation: the search hint. `assets.rs:462` passes `TEXT_PLACEHOLDER` as the hint's RichText colour without setting `weak_text_color`, so egui draws it at the weak colour (TEXT at 0.6,
  strongest pixel
  154) while the Input/Search main draws TEXT_PLACEHOLDER (115): the same recorded D5 deviation as the Home search (issue #1389). The hint is `Search all li…` as the capture clips it.
* Gaps: the grid view (:358-394), the row hover fill and drag (:331-333), the `No components yet` / `No results` states (:605-616), the grid cell's variant badge, a library's own section, the
  scrollbar's hover colour.
"""
from components.regions_sidepanels_parts import hairline, icon_button, label, scrollbar_thumb
from rest_ir import F, I, T, X, lit

ROW = "Regions"
W, H, LIST_Y, LIST_H = 240.0, 868.0, 266.0, 602.0
PILL_PAD = "panels/assets.rs:495 `text_width + 14.0`: a raw 14.0 around the label, 7 a side"
PILL_R = "panels/assets.rs:500 `theme::cr_pill(pr.height())`: half the pill's 20 px height, computed from it"
PREVIEW_STROKE = "panels/assets.rs:312 `Stroke::new(1.0, BORDER_TRANSLUCENT)`: a literal of the Rust (BORDER_WIDTH is also 1, but the call does not name it)"
# the capture's list from the top: (kind, name); C a container (a frame line, indented 8), I a component. A page line (`Line::Page`, :641) is the same row without the indent: a gap
LINES = [("C", "Frame 1"), ("I", "Button"), ("I", "Button group"), ("C", "Frame 2"), ("I", "Input field"), ("C", "Frame 3"), ("I", "Card"), ("I", "Tag"), ("I", "Toggle"), ("I", "Tab"),
         ("C", "Frame 4"),
         ("C", "Frame 5"), ("I", "Menu"), ("I", "Table")]


def group_row(env, n, name, indent):
    return F(f"Group {n}", [T("Name", name, "Body / Regular", env.c("TEXT_TERTIARY"), abs=(16.0 + indent, 7.0, 200.0, 18.0))], w=W, h=env.num("CONTROL_XS"), sx="fixed", sy="fixed")


def component_row(env, n, name):
    preview = F("Preview", [I("Icon", "Component", 16, env.c("CANVAS_COMPONENT"))], dir="H", fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER_TRANSLUCENT"), sw=lit(1, PREVIEW_STROKE),
                radius=env.c("RADIUS_CONTROL"), w=40.0, h=40.0, sx="fixed", sy="fixed", abs=(16.0, 4.0, 40.0, 40.0))
    return F(f"Component {n}", [preview, T("Name", name, "Body / Regular", env.c("TEXT"), abs=(68.0, 15.0, 160.0, 18.0))], w=W, h=48.0, sx="fixed", sy="fixed")


def rows(env):
    return [group_row(env, i, name, 8.0 if kind == "C" else 0.0) if kind != "I" else component_row(env, i, name) for i, (kind, name) in enumerate(LINES, 1)]


def pill(env, name, text, rect, selected):
    pad = lit(7, PILL_PAD)
    style, colour = ("Caption / Semibold", "TEXT") if selected else ("Caption / Regular", "TEXT_SECONDARY")
    paint = {"fill": env.c("BG_SECONDARY"), "radius": lit(10, PILL_R)} if selected else {}
    return F(name, [T("Text", text, style, env.c(colour))], dir="H", pad=[None, pad, None, pad], h=20.0, sx="hug", sy="fixed", abs=rect, **paint)


def assets_panel(values, env):
    list_rect = (0.0, LIST_Y, W, LIST_H)
    components = F("Components", rows(env), dir="V", slot="Components", w=W, h=LIST_H, sx="fixed", sy="fixed", clip=True, align=("start", "start"), abs=list_rect)
    kids = [label("Title", (0.0, 0.0, W, 40.0), "Assets", "Large / Semibold", "TEXT", env, pad=[None, None, None, env.c("INSET_PANEL")]),
            icon_button("Libraries", "BookOpen", (200.0, 4.0, 32.0, 32.0)), hairline(40.0, W),
            X("Search", "Input/Search", "Size=Xs, State=Default", props={"Value": "Search all li…"}, abs=(16.0, 48.0, 102.0, 32.0)),
            icon_button("Filter", "StrokeSettings", (122.0, 48.0, 32.0, 32.0)), icon_button("List view", "ListView", (158.0, 48.0, 32.0, 32.0), "Selected"),
            icon_button("Grid view", "GridView", (192.0, 48.0, 32.0, 32.0)),
            pill(env, "All pill", "All", (16.0, 86.0, 29.0, 20.0), True), pill(env, "Local pill", "Local", (49.0, 86.0, 44.5, 20.0), False),
            pill(env, "Used in file pill", "Used in file", (97.5, 86.0, 77.5, 20.0), False),
            label("Heading", (16.0, 111.0, 208.0, 18.0), "All libraries", "Body / Semibold", "TEXT", env),
            X("Library card", "Card/Library", "", props={"Count": "12 components"}, abs=(16.0, 134.0, 208.0, 86.0)),
            X("Add libraries", "Button/Secondary", "Size=Xs, State=Default", props={"Label": "Add more libraries"}, abs=(16.0, 226.0, 208.0, 32.0), w=208.0),
            components, scrollbar_thumb("Scrollbar thumb", list_rect, 0.0, 24.0, env)]
    return F("Assets", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def component_sets():
    return [{"name": "Panel/Assets", "row": ROW, "axes": {}, "uses": ["Button/Icon", "Button/Secondary", "Card/Library", "Divider/Hairline", "Input/Search"],
             "props": [{"name": "Components", "type": "SLOT"}], "build": assets_panel}]
