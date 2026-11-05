"""Registry `region` Panel/HomeMain (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag, `home/mod.rs` `mod main_view` (`show` :1292-1430, `files_view` :2413-2680).

Panel/HomeMain has no registry variants, so it is one plain component, 1176 x 868: the window less the 264 px sidebar and the 32 px tab bar (h-home.layout.json `regions.home`), the `BG` page
colour (:1294). Every rect is the capture's named rect minus the area's origin (264, 32). Default state: the capture `h-home`, the Recents view with one file in grid view. The other captured
state, the empty Recents (`h-home-empty`), swaps the Slot's content for EmptyState/Default, which already has its own capture row; `Trash`, `Templates & Specs`, `All folders`, `Resources` and list
view are views the registry gives no variant, so they are gaps (Slot content of the caller).
Head (`HOME_HEADER_H` 64) and sub-header form the `Top` frame, 111 high: the title, the right-aligned creation buttons, the filter and sort frames, the two view toggles and the `BORDER` hairline
at y 110 (:1415, 64 + `CONTROL_SM` + 10); below it the `Content` Slot fills the rest, padded `HOME_PAD` with `HOME_CARD_GAP` between cards (:2452-2455, `file_card_size` :1288). Nested (live):
Button/Primary `Sm` (`+ Design file`) and Button/Secondary `Sm` (`+ FigJam board`, `Import`), `kit::button` at `CONTROL_SM` and `label + 2 * SPACE_GROUP` wide (:1532, :1559-1568); Button/Icon `Sm`
for the two view toggles (`kit::button_icon`, :1402-1412; Selected on the grid); Divider/Hairline; Card/FileThumbnail (the Slot's default card, :2772-2814). Local: the title (`breadcrumb`,
:1472-1530: `font_text(FONT_SIZE_TEXT_XL, Semibold)`, the `Text xl / Semibold` style), the filter and sort frames (`paint_secondary(.., chevron = true)` :1544-1556: the Secondary look with the
label at `SPACE_GROUP` and a trailing `CHEVRON_SMALL` glyph, which Button/Secondary has no part for; each is `text + 3 * SPACE_GROUP` wide, drawn at the capture's width), and the group of the
three buttons (right-aligned, `gap` 8 raw, :1310). Gaps: `SPEC_BUNDLE_ENTRY_ENABLED` is false (no `+ Spec bundle`); the contextual `+ New folder` and `Empty trash` buttons; hover states and the
open filter and sort menus (`h.filter_menu`); the folder tiles (`HOME_FOLDER_H`), the drafts strip and list view (`HOME_LIST_ROW_H`); wrapping the card row at `cols` (the IR has no auto-layout
wrap: the default holds one card); scrolling.
"""
from rest_ir import F, I, T, X, lit
from components.regions_home_parts import REGION_ROW, icon_button, label, region, rule

W, H, HEAD_H, TOP_H = 1176, 868, 111, 111
GAP = "home/mod.rs:1310 `let gap = 8.0`: a raw 8 between the header buttons, no constant named"
CHEVRON_END = "home/mod.rs:1552: the chevron is centred at right - SPACE_GROUP (12) and is CHEVRON_SMALL (10) wide, so it ends 7 from the right edge, and a 7 px gap separates it from the label's end"
CARD = {"Title": "DESIGN-OS Studio Starter", "Meta": "Edited just now · In Getting st…"}  # the file's data; the elided meta is what `widgets::elide` shows at the card width


def button(name, kind, text):
    return X(name, f"Button/{kind}", "Size=Sm, State=Default", props={"Label": text})


def dropdown(name, text, x, w, env):
    """`paint_secondary` with the trailing chevron: the Secondary Default look as a frame of the capture's width."""
    look = env.look("Button/Secondary", "Default")
    kids = [T("Label", text, "Text sm / Semibold", look["text"], sx="fill"), I("Chevron", "ChevronDown", int(env.num("CHEVRON_SMALL")), look["icon"])]
    return F(name, kids, dir="H", fill=look["fill"], stroke=look["border"], sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), pad=[None, lit(7, CHEVRON_END), None, env.c("SPACE_GROUP")],
             abs=(x, 68, w, 36), w=w, h=env.num("CONTROL_SM"), sx="fixed", sy="fixed", align=("start", "center"))


def top(env):
    crumb = label("Title", "Recents", "Text xl / Semibold", env.c("TEXT"), 32, 17, 78.25, env.num("LINE_TEXT_XL"))
    actions = F("Actions", [button("Import", "Secondary", "Import"), button("New board", "Secondary", "+ FigJam board"), button("New file", "Primary", "+ Design file")], dir="H",
                gap=lit(8, GAP), abs=(821.5, 14, 322.5, 36), w=322.5, h=env.num("CONTROL_SM"), sx="fixed", sy="fixed", align=("end", "center"))
    kids = [crumb, actions, dropdown("Filter", "Filter: All files", 32, 128.25, env), dropdown("Sort", "Sort: Last viewed", 168.25, 153, env),
            icon_button("Grid view", "GridView", 1068, 68, 36, "Sm", "Selected"), icon_button("List view", "ListView", 1108, 68, 36, "Sm"), rule("Rule under header", 0, 110, W)]
    return F("Top", kids, dir="H", w=W, h=TOP_H, sx="fixed", sy="fixed", align=("start", "start"))


def main_area(values, env):
    cards = F("Content", [X("File card", "Card/FileThumbnail", "", props=CARD)], dir="H", slot="Content", gap=env.c("HOME_CARD_GAP"), pad=env.c("HOME_PAD"), sx="fill", sy="fill",
              align=("start", "start"))
    return region("Main", [top(env), cards], env, W, H)


def component_sets():
    return [{"name": "Panel/HomeMain", "row": REGION_ROW, "axes": {}, "build": main_area, "props": [{"name": "Content", "type": "SLOT"}],
             "uses": ["Button/Icon", "Button/Primary", "Button/Secondary", "Card/FileThumbnail", "Divider/Hairline"]}]
