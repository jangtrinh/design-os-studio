"""Parts of Window/Variables (`regions_home_variables.py`), each placed at the capture's own rect minus the area's origin (49, 32). Lines are the v0.7.5 tag, `variables/view.rs`.

Names are neutral (the capture shows a design kit's collection and group names): the collections are Primitives, Colors, Containers, Widths, Spacing, Radius and Typography with the capture's
counts, the groups colour ramp names.
"""
from rest_ir import F, E, I, T, V, X, lit, segments_path
from components.regions_home_parts import region, rule

ROW_X, ROW_W, ROW_H = 8, 224, 24  # `left + INSET_FRAME`, 224, `VAR_PILL_H` (:495, :501, :580)
COUNT_LOCK = "variables/view.rs:533 `paint_lock(.., count.left() - 10.0, 12.0)`: the 12 px lock's centre is a raw 10 left of the count, so its edge is 4 from it"
GROUP_X = "variables/view.rs:605 `tx = row.left() + INSET_CONTROL + depth * 16.0`: a sum with a raw 16 per level"
PILL = "theme::cr_pill(h) = radius_full(h) = h / 2: computed from the chip's height, no radius role holds it"
ARROWS = "variables/widgets.rs:82 `Stroke::new(1.2, col)`: a raw width, no stroke-weight constant in theme.rs"
THUMB = "variables/view.rs:955-982 `hscrollbar`: a 7 px bar, fully rounded"
OWN = [("Primitives", 343), ("Colors", 273), ("Containers", 3), ("Widths", 12), ("Spacing", 17), ("Radius", 11), ("Typography", 32)]
LIBRARY = [("Primitives", 3), ("Colors", 2), ("Typography", 1)]
GROUPS = [("All", 343, 0), ("Colors", 311, 1)] + [(n, c, 2) for n, c in (("Base", 3), ("Neutral", 11), ("Orange", 11), ("Red", 11), ("Green", 11), ("Yellow", 11), ("Cyan", 11), ("Blue", 11),
                                                                         ("Violet", 11), ("Pink", 11))]
SCOPES = [("All", 393, 30.5), ("Fills", 427.75, 38.5), ("Strokes", 470.25, 59.5), ("Text", 533.75, 40.75), ("Radius", 578.25, 54.75), ("Gap & Size", 637, 77.75)]


def text(env, name, content, style, color, x, cy, w=80):
    h = {"Large": 20}.get(style.split(" / ")[0], 18)
    return T(name, content, style, env.c(color), abs=(x, cy - h / 2, w, h))


def glyph(env, name, icon, cx, cy, size=16, color="ICON"):
    return I(name, icon, size, env.c(color), abs=(cx - size / 2, cy - size / 2, size, size))


def collapse_arrows(env):
    """`arrows_glyph(.., inward = true)` (widgets.rs:81-93): two diagonal arrows meeting at the centre, each three `line_segment`s of a 1.2 px stroke in `ICON`, drawn in a 16 px box."""
    segs = []
    for s in (-1, 1):
        tip, tail = (8 + 1.5 * s, 8 - 1.5 * s), (8 + 5.5 * s, 8 - 5.5 * s)
        segs += [(tail, tip), (tip, (tip[0] + 3.5 * s, tip[1])), (tip, (tip[0], tip[1] - 3.5 * s))]
    return V("Collapse", segments_path(segs), 16, 16, None, stroke=env.c("ICON"), sw=lit(1.2, ARROWS), abs=(1293, 16, 16, 16))


def side_row(env, name, label, count, y, fill=None, lock=False, depth=0, chevron=False):
    """One 224 x 24 row of the sidebar (:495-535, :580-619): the name in `font_body_strong` at `INSET_CONTROL` + depth * 16, the count right-aligned `INSET_CONTROL` from the end in
    `TEXT_SECONDARY`."""
    left = env.c("INSET_CONTROL") if depth == 0 else lit(8 + 16 * depth, GROUP_X)
    kids = [T("Label", label, "Body / Semibold", env.c("TEXT"), sx="fill")] + ([I("Lock", "Lock", 12, env.c("ICON_TERTIARY"))] if lock else [])
    kids.append(T("Count", str(count), "Body / Regular", env.c("TEXT_SECONDARY")))
    if chevron:
        kids.append(I("Fold", "ChevronDown", 12, env.c("ICON_TERTIARY"), abs=(left[1] - 14 if depth else 0, 6, 12, 12)))
    paint = {"fill": env.c(fill)} if fill else {}
    return F(name, kids, dir="H", radius=env.c("RADIUS_CONTROL"), pad=[None, env.c("INSET_CONTROL"), None, left], gap=lit(4, COUNT_LOCK) if lock else None, abs=(ROW_X, y, ROW_W, ROW_H), w=ROW_W,
             h=ROW_H, sx="fixed", sy="fixed", align=("start", "center"), **paint)


def sidebar(env):
    """:444-641: the file name and toggle (header row), `Collections` with the settings and add glyphs, the collection rows, the `From library` heading and rows, a rule, `Groups` and the group
    rows."""
    kids = [F("Sidebar edge", fill=env.c("BORDER"), abs=(240, 0, 1, 868), w=1, h=868, sx="fixed", sy="fixed"), rule("Rule under header", 0, 48, 239),
            text(env, "File name", "Design file", "Large / Semibold", "TEXT", 16, 25, 90), glyph(env, "Sidebar toggle", "Sidebar", 216, 24),
            text(env, "Collections", "Collections", "Body / Semibold", "TEXT", 16, 69, 80),
            glyph(env, "Settings", "Sliders", 193, 69), glyph(env, "Add collection", "Plus", 220, 69)]
    kids += [side_row(env, f"Collection {i + 1}", n, c, 93 + 32 * i, "BG_SECONDARY" if i == 0 else None) for i, (n, c) in enumerate(OWN)]
    kids.append(text(env, "Library heading", "From library", "Body / Semibold", "TEXT_TERTIARY", 16, 329, 90))
    kids += [side_row(env, f"Library {i + 1}", n, c, 349 + 32 * i, lock=True) for i, (n, c) in enumerate(LIBRARY)]
    kids += [rule("Rule above groups", 0, 449, 239), text(env, "Groups", "Groups", "Body / Semibold", "TEXT", 16, 470, 50)]
    kids += [side_row(env, f"Group {i + 1}", n, c, 494 + 32 * i, "BG_SELECTED" if i == 0 else None, depth=d, chevron=(n == "Colors")) for i, (n, c, d) in enumerate(GROUPS)]
    return kids


def ring_glyph(env):
    """`filter_glyph` (widgets.rs:72-78): a 1.1 px ring of radius 7.2 with three centred lines of 7.2, 4.8 and 2 px, in `ICON_SECONDARY`."""
    bar = lambda n, w, y: F(n, fill=env.c("ICON_SECONDARY"), abs=(12 - w / 2, y, w, 1), w=w, h=1, sx="fixed", sy="fixed")
    ring = E("Ring", 14, 14, stroke=env.c("ICON_SECONDARY"), sw=lit(1.1, "widgets.rs:73 `Stroke::new(1.1, col)`: a raw width"), abs=(5, 5, 14, 14))
    return [ring, bar("Line 1", 7.2, 9.1), bar("Line 2", 4.8, 11.5), bar("Line 3", 2, 13.9)]


def search_field(env):
    """`field_chrome(.., Size::Xs)` (:679-692): Input/Search Xs, resized from 224 x 32 to the field's 174 x 24 (`w=`, `h=`): the look is the component's. The component draws the layers filter's glyph
    (14 px, centred at 14) and text (from 26); this field's painter puts a 16 px glyph at 11 and the text at 24, which the compare reports as a pinned finding."""
    return X("Search", "Input/Search", "Size=Xs, State=Default", abs=(1078, 12, 174, 24), w=174, h=24)


def main_bar(env):
    """:644-706: the title (`font_large`), the 174 x 24 search field (`field_chrome`, Xs) with the filter button attached on its right (right corners only, `BG_SECONDARY`, `ring_glyph`) and the
    collapse glyph."""
    filt = F("Filter", ring_glyph(env), fill=env.c("BG_SECONDARY"), radius=[None, env.c("RADIUS_CONTROL"), env.c("RADIUS_CONTROL"), None], abs=(1253, 12, 24, 24), w=24, h=24, sx="fixed", sy="fixed")
    return [rule("Rule under title", 241, 48, 1150), text(env, "Title", "Primitives", "Large / Regular", "TEXT", 261, 24, 90),
            search_field(env), filt, collapse_arrows(env)]


def bottom_bar(env):
    """:751-808: a hairline at the bar's top (`VAR_BOTTOM_H` 41 from the bottom), `Create variable` (Plus 16 at left + 14, label at left + 34, 32 high) and the scope chips (22 high pills, caption
    medium in `TEXT_SECONDARY`; the active `All` is `BG_TERTIARY` with `TEXT`), then the horizontal scrollbar's thumb (`VAR_SCROLL_THUMB`) just above it."""
    chips = []
    for n, x, w in SCOPES:
        on = n == "All"
        label = T("Label", n, "Caption / Medium", env.c("TEXT" if on else "TEXT_SECONDARY"))
        chips.append(F(f"Scope {n}", [label], dir="H", radius=lit(11, PILL), abs=(x, 837, w, 22), w=w, h=22, sx="fixed", sy="fixed",
                       align=("center", "center"), **({"fill": env.c("BG_TERTIARY")} if on else {})))
    create = F("Create variable", [glyph(env, "Plus", "Plus", 14, 16), text(env, "Label", "Create variable", "Body / Regular", "TEXT", 34, 16, 90)],
               abs=(249, 832, 128, 32), w=128, h=32, sx="fixed", sy="fixed")
    thumb = F("Scroll thumb", fill=env.c("VAR_SCROLL_THUMB"), radius=lit(3.5, THUMB), abs=(251, 819, 1130, 7), w=1130, h=7, sx="fixed", sy="fixed")
    return [rule("Rule above bottom bar", 241, 827, 1150), create, *chips, thumb]


def table_body(env):
    """Table/Data nested at the table's top-left (241, 49), 521 x 778 (down to the bottom bar's rule at 827, the copy clips there): its header text is the `Mode` property of the copy and
    its default rows are the component's, the capture's 18. A screen fills the rows on the copy (an engine limit, see rest_ir.X `slots`)."""
    return X("Table", "Table/Data", "", props={"Mode": "Style"}, abs=(241, 49, 521, 778), h=778)


def window(env):
    kids = [*sidebar(env), *main_bar(env), table_body(env), *bottom_bar(env)]
    root = region("Variables", kids, env, 1391, 868)
    root["clip"] = True
    return root
