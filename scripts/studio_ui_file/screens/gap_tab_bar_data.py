"""Per-screen data of the top tab bar's dirty-tab marker: plain dicts, no `screens()`: a screen module patches the instance it places with an entry.

`UNSAVED_DOT` (`gap_table_citations`) lists the screens whose capture shows the project tab with unsaved changes: the 0.7.5 layout names a `tab-unsaved-dot-1` rect with a size (a 6 px
disc at (301, 13)) where a clean tab records none. `Navigation/TopTabBar` draws it with `Show unsaved dot` (tabs.rs:1081-1085). The four Dev mode captures show it as well, and
`gap_left_dev_data.DEV` sets it with the dev tile, so they are not listed here. A test reads the set from the capture layouts.
`ALIASES` gives the layout key `tab-unsaved-dot-1` the dot's node on all 17 screens, so the geometry check compares its rect (301, 13, 6, 6) by name.

The part does not shorten the neutral tab title as production does on a dirty tab (tabs.rs:1075-1076 reserves 10 px more): an open gap-table row. The title is a replaced text, left out of the
comparison up to the dot.
"""
from screens.gap_table_citations import DEV, UNSAVED_DOT

DOT_PATH = "Navigation/TopTabBar > project-tab-0 > Unsaved dot"  # the node below the screen that the layout key names (screen_geometry `PATH_SEP`)
ALIASES = {state: {"tab-unsaved-dot-1": DOT_PATH} for state in (*DEV, *UNSAVED_DOT)}  # the screens that draw the dot: the layout names its rect there and nowhere else
TAB_BAR = {state: {"patch": {"Navigation/TopTabBar": {"props": {"Show unsaved dot": True}}}} for state in UNSAVED_DOT}
