"""Registry `region` Panel/HomeSidebar (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag, `home/mod.rs` `mod sidebar` (:890-1168; `home/sidebar.rs` is not compiled: no `mod sidebar;`).

Panel/HomeSidebar has no registry variants, so it is one plain component, 264 x 868: `HOME_SIDEBAR_W` by the window below the 32 px tab bar (h-home.layout.json `regions.home` 0, 32, 1440, 868),
the `BG` page colour (:905) and the `BORDER` vline on its last pixel column (:906). Its default state is the capture `h-home`: Recents selected, one project (`Getting started`), no trash. The
capture `h-home-empty` (no project: `No projects yet` at x 20 in `TEXT_TERTIARY`, :1035-1042) is the same region with an empty tree: a gap, because the registry gives the record no state to draw
it in. Every rect is the capture's named rect minus the area's origin (0, 32); the rows' y are the Rust's own running `y` (listed below).

Nested (live): List/Row `Size=Sm` (the sidebar `row`, :1218-1246; 8 callers here), Input/Search `Size=Sm` (`kit::search_look` + `paint_frame`, :939-967: the field the TextEdit sits in, hint
`Search`), Badge/StatusCount `Sm Neutral` (the version pill, `kit::count_badge` :928), Button/Icon `Xs` resized to 24 (`widgets::icon_button` at 24 px: the activity bell :934 and the `+` :996),
Divider/Hairline (3 `hline`s). Local: the workspace avatar (a 20 px `BG_TERTIARY` tile with a `BORDER_STRONG` stroke and a 12 px brand folder, :918-921: no Icon/Tile look paints those), the
workspace name, the `P` disc and the section label, the Starred row's chevron (a glyph the row painter does not draw: `Icon::ChevronDown` 11 px, :1108-1113), and the edge.
Placement (v0.7.5): workspace row (8, 10, `w - 44`, `CONTROL_SM`) :911; search y = `CONTROL_SM` + 16 :937; Recents y 98 and Templates 136 (`ROW_H` + 2 pitch, :969-978); hline at 180 (+6, :981);
header y 188 (+8, :983): the disc at (20, 200), the label at x 32 (:985-995), the `+` at (`w - 12 - 24`, 188); rows from 214 (`+ 26`, :1001) at a `ROW_H` + 1 pitch (:1014): Drafts 214, All folders
251, Resources 288; the tree list from 329 (`+ 4`, :1016) with one 36 px row; `bottom_reserve` = 52 + 48 + `ROW_H` + 16 = 152 (:1018-1021) so the list ends at 868 - 152 = 716, the divider at 720
(+ 4, :1102), Starred at 726 (+ 6, :1104), the Trash divider at 868 - (`ROW_H` + 2 * `TRASH_PAD`) = 814 and the Trash row at 868 - `TRASH_PAD` - `ROW_H` = 823 (:1149-1155).
Deviations and gaps: the hint colour (D5 deviation of Input/Search: the painter leaves `weak_text_color` unset); the update card (`updater::show_sidebar_card_in_rect`, :1148-1151) draws nothing in
the capture and is not drawn; the Starred expansion rows (`starred_files`, :1125-1146), the trash count badge (:1158-1164), the drop-target stroke and hover fills are states the registry gives no
variant.
"""
from rest_ir import F, I, T, X, lit
from components.regions_home_parts import REGION_ROW, icon_button, label, region, rule

W, H, ROW_W = 264, 868, 247
AVATAR_STROKE = "home/mod.rs:920 `Stroke::new(1.0, BORDER_STRONG)`: a raw 1.0 in the Rust, no constant named"
DISC = "home/mod.rs:987 `p.circle_filled(ws_badge_c, 7.0, design.fill)`: a raw radius 7"


def row(name, text, icon, x, y, selected=False):
    return X(name, "List/Row", f"Size=Sm, State={'Selected' if selected else 'Default'}", props={"Label": text, "Icon": icon}, abs=(x, y, ROW_W, 36))


def workspace_row(env):
    avatar = F("Workspace avatar", [I("Folder", "Folder", 12, env.c("ICON_BRAND_PRIMARY"))], dir="H", fill=env.c("BG_TERTIARY"), stroke=env.c("BORDER_STRONG"), sw=lit(1, AVATAR_STROKE),
               radius=env.c("RADIUS_CONTROL"), abs=(12, 18, 20, 20), w=20, h=20, sx="fixed", sy="fixed")
    name = label("Workspace name", "Personal Workspace", "Text sm / Medium", env.c("TEXT"), 38, 18, 138, env.num("LINE_TEXT_SM"))
    version = X("Version", "Badge/StatusCount", "Size=Sm, Tone=Neutral, State=Default", props={"Label": "v0.7.5"}, abs=(181, 17, 42, 22))
    return [avatar, name, version, icon_button("Notifications", "Eye", 229, 16)]


def projects_header(env):
    disc = F("Project badge", [T("P", "P", "Text xs / Semibold", env.c("TILE_DESIGN_GLYPH"))], dir="H", fill=env.c("TILE_DESIGN_BG"), radius=lit(7, DISC), abs=(13, 193, 14, 14), w=14, h=14,
             sx="fixed", sy="fixed")
    title = label("Projects label", "FILES & PROJECTS", "Text xs / Semibold", env.c("TEXT_TERTIARY"), 32, 191, 110, env.num("LINE_TEXT_XS"))
    return [disc, title, icon_button("New project", "Plus", 227, 188)]


def sidebar(values, env):
    edge = F("Edge", fill=env.c("BORDER"), abs=(W - 1, 0, 1, H), w=1, h=H, sx="fixed", sy="fixed")
    kids = [edge, *workspace_row(env), X("Search", "Input/Search", "Size=Sm, State=Default", abs=(12, 52, 239, 36)),
            row("Recents", "Recents", "Clock", 8, 98, selected=True), row("Templates", "Templates & Specs", "Specs", 8, 136), rule("Rule after navigation", 12, 180, 239),
            *projects_header(env), row("Drafts", "Drafts", "File", 8, 214), row("All folders", "All folders", "GridView", 8, 251), row("Resources", "Resources & Libraries", "Assets", 8, 288),
            row("Project", "Getting started", "Frame", 8, 329), rule("Rule above starred", 12, 720, 239), row("Starred", "Starred", "Star", 8, 726),
            I("Starred chevron", "ChevronDown", 11, env.c("ICON_QUATERNARY"), abs=(237.5, 738.5, 11, 11)), rule("Rule above trash", 0, 814, W - 1), row("Trash", "Trash", "Trash", 8, 823)]
    return region("Sidebar", kids, env, W, H)


def component_sets():
    return [{"name": "Panel/HomeSidebar", "row": REGION_ROW, "axes": {}, "build": sidebar,
             "uses": ["Badge/StatusCount", "Button/Icon", "Divider/Hairline", "Input/Search", "List/Row"]}]
