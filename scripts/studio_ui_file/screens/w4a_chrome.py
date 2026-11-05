"""The screens of Flow Home & editor shell that draw the shell's regions as they are, or with the closest instance where a region has one state only.

* `Screen/Home/h-home` (1440 x 900, `h-home`): the Home window, the Recents view with one file in grid view (shell.rs `home_frame`): the tab bar (0, 0), the sidebar (0, 32, 264, 868) and the main
  area (264, 32, 1176, 868) (`regions.home`). The capture's tab bar holds the Home tab and the `+` only: the instance turns `Show project tab` off and `Show Home selected` on, so the `+` follows
  the divider at x 94 (`gap_variables_present_data`). No `Canvas background`: Home has no canvas.
* `Screen/EditorDesign/d-window-chrome` and `Screen/EditorDesign/e-no-selection`: both captures are pixel for pixel the first screen's (`d-editor-default`), so the same six regions at the same rects.
* `Screen/EditorDesign/d-editor-selected`, `e-layers-selected`, `kit-editor`: one or more layers selected (or the Page header component), the file panel open. Navigation/LeftRail, TopTabBar, Rulers
  and BottomCanvasToolbar are the first screen's; Panel/LeftLayers draws the capture's layer tree (`gap_left_dev_data`; `e-layers-selected` keeps the 18 default rows, its list is
  scrolled), the name column masked as the open file's text, and Panel/RightInspector draws the state of the capture (`Frame`, `Layers` or `Instance`, `gap_right_panel_data`).
"""
from screens.w4a_common import screen, shell

SIDE, MAIN = "Panel/HomeSidebar > ", "Panel/HomeMain > "
SIDEBAR = {"recents": "Recents", "templates": "Templates", "drafts": "Drafts", "all-projects": "All folders", "resources": "Resources", "starred": "Starred", "trash": "Trash", "search": "Search",
           "notifications": "Notifications", "new-project": "New project", "project-Getting started": "Project"}
MAIN_AREA = {"crumb-0": "Top > Title", "filter": "Top > Filter", "sort": "Top > Sort", "import": "Top > Actions > Import", "new-board": "Top > Actions > New board",
             "new-file": "Top > Actions > New file", "view-grid": "Top > Grid view", "view-list": "Top > List view", "card-DESIGN-OS Studio Starter": "Content > File card",
             "kind-design": "Content > File card"}
# The capture's named widgets, found by the semantic name the regions give their nodes (never by matching the rect): the sidebar rows, the header actions, the filter and sort pills, the view
# toggles and the card.
HOME_ALIASES = {**{f"home-{k}": SIDE + v for k, v in SIDEBAR.items()}, **{f"home-{k}": MAIN + v for k, v in MAIN_AREA.items()}}


def home():
    place = [{"name": "Navigation/TopTabBar", "component": "Navigation/TopTabBar", "variant": {"Bar": "32px"}, "x": 0, "y": 0},
             {"name": "Panel/HomeSidebar", "component": "Panel/HomeSidebar", "variant": None, "x": 0, "y": 32},
             {"name": "Panel/HomeMain", "component": "Panel/HomeMain", "variant": None, "x": 264, "y": 32}]
    return screen("h-home", place, flow="Home", canvas=False, aliases=HOME_ALIASES)


def screens():
    return [home()] + [screen(case, shell()) for case in ("d-window-chrome", "e-no-selection", "d-editor-selected", "e-layers-selected", "kit-editor")]
