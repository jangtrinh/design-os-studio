"""Window/Variables `Layout=Floating` and `Layout=Present` (lines are the v0.7.5 tag).

Floating (`floating_rect`, variables/view.rs:88-94: `VAR_FLOAT_W` x `VAR_FLOAT_H` = 642 x 380, a popover surface: BG, a BORDER_MENU line,
  RADIUS_FRAME): the same painters as the full window from a smaller `Geom`
(`sidebar` :444-641, `main` :644-749, `bottom_bar` :751-808). The sidebar is 240 wide and starts with the title `Variables` and the toggle (40 high, 9
  px less than the full window's file-name row, so
every row under it is 9 px higher); the main area has no title: the 174 x 24 search, the filter, the expand arrows and the close cross (the capture's `vars-search`, `vars-filter`, `vars-expand`,
`vars-close`), a hairline, the table (200 / 161 / 39 columns: `mode_w` clamps to `VAR_MODE_MIN`) clipped above the horizontal scrollbar, and the bottom bar. The table is drawn here, not nested
(Table/Data is 521 wide with a 280 mode column). Local window coordinates: the screen's rects (0, 8) away.
Present (`paint_vars_panel`, present.rs:804-869): the 264 x 836 popover surface a screen places at (1164, 56): `Variables` in `font_body_strong` at 16 and `live, not saved` in caption TEXT_TERTIARY
right-aligned, a BORDER_MENU hairline at 43, then per collection a 26 px heading (`font_caption_medium` TEXT_SECONDARY, the mode right-aligned in
  caption TEXT_TERTIARY) and `VARS_ROW_H` (22) rows: the
name in body TEXT, the value right-aligned in TEXT_SECONDARY. Names are neutral; the values are the capture's. The panel clips at its bottom edge.
Gaps: the filter and expand glyph hover fills, the vertical scrollbar thumb, the errors block, `changed` values in BG_BRAND.
"""
from rest_ir import F, I, T, V, X, lit, segments_path
from components.data_table import BODY, body_rows, table_header
from components.regions_home_parts import rule
from components.regions_home_variables_parts import PILL, THUMB, glyph, ring_glyph, search_field, sidebar, text

FLOAT_W, FLOAT_H, PRESENT_W, PRESENT_H, ROW = 642, 380, 264, 836, 22
ARROWS = "variables/widgets.rs:82 `Stroke::new(1.2, col)`: a raw width, no stroke-weight constant in theme.rs"
SIDE_HEAD = ("Rule under header", "File name", "Sidebar toggle")
FLOAT_SCOPES = (("All", 392, 30.5), ("Fills", 426.75, 38.5), ("Strokes", 469.25, 59.5), ("Text", 532.75, 40.75), ("Radius", 577.25, 54.75))
FLOAT_ROWS = BODY[:9]
PRESENT = ((("Collection 1", "Style"), (("A3A3A3",), ("525252",), ("FFFFFF",))), (("Collection 2", "Light mode"), (("525252",), ("000000",))),
           (("Collection 1", "Style"), tuple((h,) for h in ("FFFFFF", "FAFAFA") + ("FFFFFF",) * 11 + ("F5F5F5", "E5E5E5", "D4D4D4", "A3A3A3", "737373", "525252", "404040", "262626", "171717",
                                                                                                    "0A0A0A", "000000", "FFFFFF", "F5F5F5", "F5F5F5"))))


def popover(kids, env, w, h):
    return F("Window", kids, fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=w, h=h, sx="fixed", sy="fixed", clip=True)


def shifted(nodes, dy):
    return [dict(n, abs=(n["abs"][0], n["abs"][1] + dy, n["abs"][2], n["abs"][3])) for n in nodes]


def arrows(env, name, x, y):
    """`arrows_glyph(.., inward = false)` (widgets.rs:81-93): two diagonal arrows leaving the centre, 1.2 px strokes in `ICON`, in a 16 px box."""
    segs = []
    for s in (-1, 1):
        tip, tail = (8 + 5.5 * s, 8 - 5.5 * s), (8 + 1.5 * s, 8 - 1.5 * s)
        segs += [(tail, tip), (tip, (tip[0] - 3.5 * s, tip[1])), (tip, (tip[0], tip[1] + 3.5 * s))]
    return V(name, segments_path(segs), 16, 16, None, stroke=env.c("ICON"), sw=lit(1.2, ARROWS), abs=(x, y, 16, 16))


def floating_bar(env):
    filt = F("Filter", ring_glyph(env), fill=env.c("BG_SECONDARY"), radius=[None, env.c("RADIUS_CONTROL"), env.c("RADIUS_CONTROL"), None], abs=(549, 8, 24, 24), w=24, h=24, sx="fixed", sy="fixed")
    search = X("Search", "Input/Search", "Size=Xs, State=Default", abs=(374, 8, 174, 24), w=174, h=24)
    return [search, filt, arrows(env, "Expand", 586, 12), glyph(env, "Close", "Close", 622, 20), rule("Rule under bar", 241, 39, 401)]


def floating_bottom(env):
    chips = [F(f"Scope {n}", [T("Label", n, "Caption / Medium", env.c("TEXT" if n == "All" else "TEXT_SECONDARY"))], dir="H", radius=lit(11, PILL), abs=(x, 349, w, 22), w=w, h=22,
               sx="fixed", sy="fixed",
               align=("center", "center"), **({"fill": env.c("BG_TERTIARY")} if n == "All" else {})) for n, x, w in FLOAT_SCOPES]
    create = F("Create variable", [glyph(env, "Plus", "Plus", 14, 16), text(env, "Label", "Create variable", "Body / Regular", "TEXT", 34, 16, 90)], abs=(248, 344, 128, 32), w=128, h=32,
               sx="fixed", sy="fixed")
    thumb = F("Scroll thumb", fill=env.c("VAR_SCROLL_THUMB"), radius=lit(3.5, THUMB), abs=(250, 331, 382, 7), w=382, h=7, sx="fixed", sy="fixed")
    return [rule("Rule above bottom bar", 241, 339, 401), create, *chips, thumb]


def floating(env):
    side = [k for k in sidebar(env) if k["name"] not in SIDE_HEAD and k["name"] != "Sidebar edge"]
    head = [text(env, "Title", "Variables", "Large / Semibold", "TEXT", 16, 20, 100), glyph(env, "Sidebar toggle", "Sidebar", 219, 20), rule("Rule under header", 0, 40, 239),
            F("Sidebar edge", fill=env.c("BORDER"), abs=(240, 0, 1, FLOAT_H), w=1, h=FLOAT_H, sx="fixed", sy="fixed")]
    rows = F("Rows", body_rows(env, FLOAT_ROWS, 161), dir="V", align=("start", "start"), sx="fill", sy="hug")
    grid = F("Table", [table_header(env, 161, None), rows], dir="V", align=("start", "start"), w=402, sx="fixed", sy="hug", abs=(0, 0, 402, 300))
    area = F("Table area", [grid], w=401, h=299, sx="fixed", sy="fixed", clip=True, abs=(241, 40, 401, 299))
    return popover([*head, *shifted(side, -9.0), *floating_bar(env), area, *floating_bottom(env)], env, FLOAT_W, FLOAT_H)


def present(env):
    kids = [text(env, "Title", "Variables", "Body / Semibold", "TEXT", 16, 22, 120),
            T("State", "live, not saved", "Caption / Regular", env.c("TEXT_TERTIARY"), align_h="right", wrap=True, w=120, abs=(128, 13, 120, 18)), rule("Rule under header", 0, 43, PRESENT_W)]
    y, n = 48.0, 0
    for (coll, mode), rows in PRESENT:
        kids += [T(f"Heading {y:g}", coll, "Caption / Medium", env.c("TEXT_SECONDARY"), abs=(16, y + 6, 140, 18)),
                 T(f"Mode {y:g}", mode, "Caption / Regular", env.c("TEXT_TERTIARY"), align_h="right", wrap=True, w=100, abs=(148, y + 6, 100, 18))]
        y += 26
        for (value,) in rows:
            n += 1
            kids += [T(f"Name {n}", f"Color {n}", "Body / Regular", env.c("TEXT"), abs=(16, y + 2, 140, 18)),
                     T(f"Value {n}", f"#{value}", "Body / Regular", env.c("TEXT_SECONDARY"), align_h="right", wrap=True, w=100, abs=(148, y + 2, 100, 18))]
            y += ROW
    return popover(kids, env, PRESENT_W, PRESENT_H)
