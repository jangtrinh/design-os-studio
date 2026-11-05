"""Registry `component` Menu/Item (row Menus & Overlays): one row of `popup_tree_with_row_h` (widgets/menu.rs:321-573, rows 455-516), coloured by `kit::menu_row_look`
(rows.rs:68-89; 3 callers: widgets/menu.rs:487, panels/menus.rs:761, panels/context.rs:213).

The fill is the pill, the row inset `MENU_PILL_INSET` (8) on both sides (menu.rs:467-470), so the root is that pill (`MENU_WIDTH` 200 is the menu's minimum,
so 184 here; a real menu is as wide as its widest row) and offsets from the row's edge lose 8. Xs is `MENU_ITEM_HEIGHT` 32; Sm is the 36 the three Home menus
pass (`popup_menu_with_row_h(.., CONTROL_SM)`, home/mod.rs:493, 512, 532). Parts, left to right (row coordinates):

* check (Selected only, `checked`): `Icon::Check` 16 px at `MENU_CHECK_X` 12, in `paint.mark` = the selected icon colour (menu.rs:490-497);
* icon (`Has Icon`): a 24 px box at `MENU_ICON_X_PLAIN` 12, or `MENU_ICON_X_CHECKED` 32 after a check, the glyph 16 px centred in it (menu.rs:499-502);
* label: `font_menu` (Body Medium) at 16, 40 with an icon, 32 after a check, 60 after both (`label_x`, menu.rs:252-259);
* shortcut hint (`Has Shortcut`): right-aligned `MENU_SHORTCUT_PAD` (16) from the row's right edge in `kit::shortcut_look` (Default, or Disabled when the row is):
  inside the pill that is 16 - 8 = `MENU_PILL_INSET` of right padding (a literal there would sit on a hidden node, which the MCP `has-hardcoded` check does not report);
* submenu chevron (`Has Submenu`): `Icon::ChevronRight` 12 px centred in a 24 px box 8 from the right edge, which replaces the hint (menu.rs:504-513).

The icon slot is its box plus `MENU_ICON_LABEL_GAP` of right padding, so a hidden box leaves only the 4 px and the label lands at 16 without a spacer. A checked
row (Selected) has no padding after its 16 px check; its icon box is 28 wide with 4 px of left padding instead, so the gap after the check hides with the box:
the label is at 32 (`MENU_LABEL_X_CHECK`) without an icon and 60 with one, and the glyph, centred in the 24 px box, stays at 36.
Looks: Hover, Active and Focus are one look (the `lit` highlight, `menu_item_look`, rows.rs:47-54, no ring: the zoom menu's focus mark, panels/menus.rs:761, is a
second painter); Selected is the check plus the brand icon colour; Disabled is the Default look at half alpha per paint (`kit/mod.rs:134-162`).

The hint is an instance of Menu/Shortcut (`kit::shortcut_look`), Disabled for a Disabled row and Default otherwise, set to `Ctrl+K` through its own `Keys` text: the importer binds a
text property per node, so no Menu/Item property can drive the nested text and the set has none (a use sets `Keys` on the instance). The slot frame keeps the right padding.
In a menu where any row is checkable every row's label sits at 32 or 60 (`Layout::has_check`, menu.rs:177-199): `Show check column` (default off) is the hidden 16 px box that does it for a
row that is not Selected, so the label lands at 32, or at 60 after an icon (the 4 px gap after the check hides with the icon box, as on a Selected row).
Gaps: the zoom menu (check at 18, 12 px, panels/menus.rs:765-766) and the layer picker (icon at 12, label at 26,
panels/context.rs:213-222) are second painters; `command_row_look` and `kit::menu_item_height` rows (cmdk) are not this set.
"""
from rest_ir import F, I, T, X, lit

PILL_TO_ROW = "widgets/menu.rs:490-502: the check box (MENU_CHECK_X) and the icon box (MENU_ICON_X_PLAIN) both start 12 from the row, which is MENU_PILL_INSET (8) outside the pill: 12 - 8"
HEIGHT = {"Xs": "MENU_ITEM_HEIGHT", "Sm": "CONTROL_SM"}


def menu_item(values, env):
    size, state = values["Size"], values["State"]
    look = env.look("Menu/Item", state)
    alpha = look["alpha"]
    gap = env.c("MENU_ICON_LABEL_GAP")
    checked = state == "Selected"
    check = F("Check slot", [I("Check", "Check", int(env.num("MENU_CHECK_BOX")), look["icon"], alpha=alpha)], dir="H")
    # After a check the icon box starts MENU_ICON_LABEL_GAP in (MENU_ICON_X_CHECKED 32 = check 12 + 16 + 4), and that gap hides with the box: with no icon the
    # label sits at MENU_LABEL_X_CHECK 32 (menu.rs:252-259), not 36.
    box = F("Icon box", [I("Icon", "Search", int(env.num("MENU_ICON_SIZE")), look["icon"], swap="Icon", alpha=alpha)], dir="H",
            w=env.num("MENU_ICON_BOX") + (env.num("MENU_ICON_LABEL_GAP") if checked else 0), pad=[None, None, None, gap if checked else None],
            h=env.num("MENU_ICON_BOX"), sx="fixed", sy="fixed", visible_prop="Has Icon", hidden=True)
    icon = F("Icon slot", [box], dir="H", pad=[None, gap, None, None])
    label = T("Label", "Edit", "Body / Medium", look["text"], prop="Label", sx="fill", alpha=alpha)
    hint = X("Shortcut", "Menu/Shortcut", f"State={'Disabled' if state == 'Disabled' else 'Default'}", props={"Keys": "Ctrl+K"})
    shortcut = F("Shortcut slot", [hint], dir="H", pad=[None, env.c("MENU_PILL_INSET"), None, None], visible_prop="Has Shortcut", hidden=True)
    chevron = F("Submenu box", [I("Submenu", "ChevronRight", int(env.num("MENU_CHEVRON_SIZE")), look["icon"], alpha=alpha)], dir="H", w=env.num("MENU_CHEVRON_BOX"),
                h=env.num("MENU_CHEVRON_BOX"), sx="fixed", sy="fixed", visible_prop="Has Submenu")
    # Show check column: a hidden 16 px box before the icon slot (MENU_CHECK_BOX, menu.rs:490-497), so a row's label lands at MENU_LABEL_X_CHECK 32 (`label_x`, menu.rs:252-259).
    # The 4 px gap after the check hides with the icon box in production, so it is a second hidden frame inside the column, shown by `Has Icon` (a child is visible only when
    # its parent is): with an icon the label is at 60.
    column = F("Check column", [F("Check box", w=env.num("MENU_CHECK_BOX"), h=env.num("MENU_CHECK_BOX"), sx="fixed", sy="fixed"),
                                F("Check gap", w=env.num("MENU_ICON_LABEL_GAP"), h=env.num("MENU_CHECK_BOX"), sx="fixed", sy="fixed", visible_prop="Has Icon", hidden=True)],
               dir="H", visible_prop="Show check column", hidden=True)
    kids = ([check] if checked else [column]) + [icon, label, shortcut, chevron]
    return F("Item", kids, dir="H", fill=look["fill"], radius=env.c("RADIUS_CONTROL"), pad=[None, None, None, lit(4, PILL_TO_ROW)],
             w=env.num("MENU_WIDTH") - 2 * env.num("MENU_PILL_INSET"), h=env.num(HEIGHT[size]), sx="fixed", sy="fixed", align=("start", "center"))


def component_sets():
    return [{"name": "Menu/Item", "row": "Menus & Overlays", "axes": {"Size": ["Xs", "Sm"], "State": ["Default", "Selected", "Hover", "Active", "Focus", "Disabled"]},
             "uses": ["Menu/Shortcut"], "props": [{"name": "Label", "type": "TEXT", "default": "Edit"},
                       {"name": "Has Icon", "type": "BOOLEAN", "default": False}, {"name": "Has Shortcut", "type": "BOOLEAN", "default": False},
                       {"name": "Has Submenu", "type": "BOOLEAN", "default": True}, {"name": "Show check column", "type": "BOOLEAN", "default": False},
                       {"name": "Icon", "type": "INSTANCE_SWAP", "default": "Search"}],
             "build": menu_item}]
