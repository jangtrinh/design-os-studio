"""Registry `region` Panel/VersionHistory (page `03 Studio`), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (history/panel.rs).

Painter: `show` (history/panel.rs:21-195), one caller; the capture is `j-version-history`, the region is the right panel (1199, 32, 241, 868) and every rect is relative to its top-left.
State drawn: the one of the capture, a saved version is being viewed (`history.viewing`), so the Save button is disabled (`enabled = !viewing`, :56), the Current row is not selected and the
viewed version is the selected pill (:115, :145). The banner `Viewing ...` above the canvas (history/banner.rs) is the canvas's, not the panel's.
* Nested sets (live): Button/Close (`icon_button` Close, :33-34, 24 px at right - 8 - 24), Button/Secondary Xs (`kit::button` Secondary Xs, :55-59: 224 wide, `CONTROL_XS` high), Divider/Hairline
  (`hline(rect.left() + 1, rect.right(), y, BORDER)` under the header :39 and under the Save button :61).
* Local: the left edge (`vline`, :24), the title (`font_large_strong`, TEXT, 16 in: a raw 16.0, :27) and the version list. The list is the SLOT `Versions`: a clipped column from y 98 (4 px
  above the first row,
  :96, rows inset `SIDE` 8, :12) holding the rows the capture shows. A row is `HISTORY_ROW_H` 44 high (`HISTORY_DAY_H` 28 for a day label), the dot a 4 px radius circle at x 16 (:122, :147) in
  `ICON_TERTIARY`, `TEXT_BRAND` for a named version; the Current row's title is `Body` Semibold at x 32, a version's two lines are `Body` Semibold `TEXT` centred 14 from its top and
  `Caption` `TEXT_SECONDARY` 30
  from it (:161-162), a day label is `Caption` Semibold `TEXT_TERTIARY` at x 8, 2 below the row's centre (:106). The selected pill is `BG_SELECTED` at `RADIUS_CONTROL` (`paint_row_bg`, :197-203).
  Version names and notes are document data: the capture's are the starter file's own, kept as they are (none is a kit or library name).
* Gaps: the undo-step rows (`Row::UndoStep`, :108-141, `TEXT_BRAND` dot), the row hover fill (`BG_HOVER`), the empty hint (:44-49, :175-184), the right-click menu (:205-228), and the
  scrollbar (the 14
  rows fit the 770 px list, so `scrollbar` draws nothing).
"""
from components.regions_sidepanels_parts import close_button, hairline, label
from rest_ir import E, F, T, X, lit

ROW = "Regions"
W, H, LIST_Y, LIST_H = 241.0, 868.0, 98.0, 770.0
ROW_W = 224.0
TITLE_X = "history/panel.rs:27 `rect.left() + 16.0`: a raw 16.0 (PANEL_PADDING_X and INSET_PANEL hold 16 for other roles)"
SIDE = "history/panel.rs:12 `const SIDE: f32 = 8.0`: a local constant, the list's inset on each side (INSET_FRAME also holds 8)"
LIST_TOP = "history/panel.rs:96 `list.top() + 4.0 - scroll`: a raw 4.0 above the first row"
# (day label, [(first line, second line, named)]) of the capture; the last version is the one being viewed
DAYS = [("Today", [("6 minutes ago", "Auto-saved", False), ("20 minutes ago", "Auto-saved", False), ("Before the colour pass", "55 minutes ago", True),
                   ("1 hour ago", "Auto-saved", False), ("3 hours ago", "Auto-saved", False)]),
        ("Yesterday", [("Ready for review", "Card and dashboard added", True), ("17:31 UTC", "Auto-saved", False)]),
        ("2 days ago", [("14:31 UTC", "Auto-saved", False)]), ("5 days ago", [("Initial layout", "First pass of the welcome scr…", True)])]


def dot(env, colour):
    return E("Dot", 8.0, 8.0, fill=env.c(colour), sx="fixed", sy="fixed", abs=(12.0, 18.0, 8.0, 8.0))


def pill(env, selected):
    return {"fill": env.c("BG_SELECTED"), "radius": env.c("RADIUS_CONTROL")} if selected else {}


def current_row(env):
    title = T("Label", "Current version", "Body / Semibold", env.c("TEXT"), abs=(32.0, 13.0, 184.0, 18.0))
    return F("Current version", [dot(env, "ICON_TERTIARY"), title], w=ROW_W, h=env.num("HISTORY_ROW_H"), sx="fixed", sy="fixed", **pill(env, False))


def version_row(env, n, first, second, named, selected):
    kids = [dot(env, "TEXT_BRAND" if named else "ICON_TERTIARY"), T("First line", first, "Body / Semibold", env.c("TEXT"), abs=(32.0, 5.0, 184.0, 18.0)),
            T("Second line", second, "Caption / Regular", env.c("TEXT_SECONDARY"), abs=(32.0, 21.0, 184.0, 18.0))]
    return F(f"Version {n}", kids, w=ROW_W, h=env.num("HISTORY_ROW_H"), sx="fixed", sy="fixed", **pill(env, selected))


def day_row(env, n, text):
    return F(f"Day {n}", [T("Text", text, "Caption / Semibold", env.c("TEXT_TERTIARY"), abs=(8.0, 7.0, 208.0, 18.0))], w=ROW_W, h=env.num("HISTORY_DAY_H"), sx="fixed", sy="fixed")


def rows(env):
    out, n = [current_row(env)], 0
    for d, (day, versions) in enumerate(DAYS, 1):
        out.append(day_row(env, d, day))
        for first, second, named in versions:
            n += 1
            out.append(version_row(env, n, first, second, named, n == sum(len(v) for _, v in DAYS)))
    return out


def history_panel(values, env):
    side = lit(8, SIDE)
    save = X("Save", "Button/Secondary", "Size=Xs, State=Disabled", props={"Label": "Save to version history…"}, abs=(9.0, 57.0, ROW_W, 32.0), w=ROW_W)
    versions = F("Versions", rows(env), dir="V", slot="Versions", pad=[lit(4, LIST_TOP), side, None, side], w=240.0, h=LIST_H, sx="fixed", sy="fixed", clip=True, align=("start", "start"),
                 abs=(1.0, LIST_Y, 240.0, LIST_H))
    kids = [F("Left edge", fill=env.c("BORDER"), w=1.0, h=H, sx="fixed", sy="fixed", abs=(0.0, 0.0, 1.0, H)),
            label("Title", (0.0, 0.0, W, 48.0), "Version history", "Large / Semibold", "TEXT", env, pad=[None, None, None, lit(16, TITLE_X)]),
            close_button((209.0, 12.0, 24.0, 24.0)), hairline(48.0, 240.0, 1.0), save, hairline(97.0, 240.0, 1.0), versions]
    return F("Version history", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def component_sets():
    return [{"name": "Panel/VersionHistory", "row": ROW, "axes": {}, "uses": ["Button/Close", "Button/Secondary", "Divider/Hairline"],
             "props": [{"name": "Versions", "type": "SLOT"}], "build": history_panel}]
