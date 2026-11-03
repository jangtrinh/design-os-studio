"""The gate data of the editor-shell states of batch W4C that show a panel, a popover or the variables window (`screens/w4c_panels.py`, `w4c_variables.py`): `j-check-designs`,
`j-comments`, `j-find`, `j-version-history`, `k-assets`, `k-variables`, `k-variables-floating`. The dialog and presentation states are in `w4c_overlays_gate.py`.

Plain data (studio_ui_gate_screen_data): `screen_caps` and `screen_masks` read the tables, and so does the file gate, which runs without the screens package. Rects are capture
px (x, y, w, h). Caps: the editor-default cap of a region where the screen meets it, else the measured mean + 8 %, rounded up, with its cause; no state calibrates against itself.
A kit's text that the screen replaces with neutral text is in TEXTS (rect, what the capture shows, what the screen draws), left out of the comparison. The shell's replaced
strings are the ones of `Screen/EditorDesign`.
"""
import json
import pathlib

# the rects the wired data modules imply (gap_wired_texts.py writes the file): a state's kit strings the screen replaces with neutral ones, added to this module's own
_WIRED = {state: [(tuple(r[:4]), r[4], r[5]) for r in rects] for state, rects in json.loads(pathlib.Path(__file__).with_name("gap_wired_texts.json").read_text(encoding="utf-8")).items()}
_TOOLBAR = (464, 840, 512, 48)
_CONTENT_AREA = ((310, 52, 889, 848), _TOOLBAR)  # the canvas between the ruler strips and the toolbar: the document's own frames in the capture, nothing in the screen
# the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
_TAB, _FILE = ((116, 8, 185, 16), "project tab title", "Design file"), ((57, 40, 184, 24), "file name", "Design file")
_PAGES = [((61, 137 + 32 * i, 212, 32), "page name", n) for i, n in enumerate(["Page 1", "Page 2", "——————", "SECTION", "↳ Page 3"])]
_LAYERS = [((100, 345, 82, 555), "layer names of the whole Layers list (the name column; kind icon, chevron and type label count)",
            "Component 1, Component 2, Frame 1 ... Frame 16")]
_FOLDERS = [((1235, 281 + 160 * k, 110, 32), "text style folder", f"Heading {k + 1}") for k in range(4)]
_SHELL = [_FILE, _TAB] + _PAGES + _LAYERS
_EDITOR = {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, 241, 868), "right panel": (1199, 32, 241, 868), "ruler top strip": (310, 32, 889, 20),
           "ruler left strip": (290, 52, 20, 848), "toolbar": _TOOLBAR}
_FINDING_ROWS = [((899, ry + dy, 210, 20), what, neutral) for ry in (164, 256, 348)
                 for dy, what, neutral in ((6, "layer name in a finding", "Frame 1 / Text block"), (40, "variable names in a finding", "space/16, space/12, space/2"))]
_VARIABLE_ROWS = [((65, 137 + 32 * i, 140, 24), "collection name", n) for i, n in enumerate(["Primitives", "Colors", "Containers", "Widths", "Spacing", "Radius", "Typography"])]
_LIBRARY_ROWS = [((65, 381 + 32 * i, 140, 24), "library collection name", n) for i, n in enumerate(["Primitives", "Colors", "Typography"])]
_GROUP_ROWS = [((65, 558 + 32 * j, 150, 24), "group name", n) for j, n in enumerate(["Base", "Neutral", "Orange", "Red", "Green", "Yellow", "Cyan", "Blue", "Violet", "Pink"])]

TEXTS = {
    "j-check-designs": _SHELL + _FINDING_ROWS,
    "j-comments": _SHELL + [((1224, 239, 192, 18), "comment text in the card", "Align icon spacing to 8px grid")],
    "j-find": _SHELL + _FOLDERS + [((95, 417 + 64 * k, 120, 32), "group name", f"Frame {k + 1}") for k in range(8)]
    + [((103, 449 + 64 * k, 120, 32), "hit excerpt", "Card and button") for k in range(7)],
    "j-version-history": _SHELL,
    "k-assets": [_TAB, ((65, 238, 208, 16), "library component count", "12 components"),
                 ((49, 298, 240, 602), "container and component rows with their previews", "Frame 1 ... Frame 5, Button ... Table")],
    "k-variables": [_TAB, ((65, 45, 170, 24), "document name", "Design file"), ((306, 44, 200, 24), "collection title", "Primitives")] + _VARIABLE_ROWS + _LIBRARY_ROWS + _GROUP_ROWS,
    "k-variables-floating": [_TAB] + _LAYERS,
}
TEXTS = {s: texts + _WIRED.get(s, []) for s, texts in TEXTS.items()}
CONTENT = {"j-check-designs": _CONTENT_AREA, "j-comments": _CONTENT_AREA, "j-find": _CONTENT_AREA, "j-version-history": _CONTENT_AREA, "k-assets": _CONTENT_AREA,
           "k-variables": None, "k-variables-floating": _CONTENT_AREA}
KEEP = {"j-check-designs": [(867, 44, 320, 364)], "j-comments": [(801, 381, 301, 212), (1107, 379, 36, 36)], "j-version-history": [(290, 32, 909, 40)],
        "k-variables-floating": [(0, 8, 642, 380)]}
REGIONS = {
    "j-check-designs": {**_EDITOR, "ruler top strip": (310, 32, 557, 20), "check designs popover": (867, 44, 320, 364)},
    "j-comments": {**_EDITOR, "thread popover": (801, 381, 301, 212), "comment pin": (1107, 379, 36, 36)},
    "j-find": {**_EDITOR, "find panel": (49, 305, 240, 595)},
    "j-version-history": {k: v for k, v in _EDITOR.items() if k != "ruler top strip"} | {"ruler left strip": (290, 72, 20, 828), "version bar": (290, 32, 909, 40)},
    "k-assets": dict(_EDITOR),
    "k-variables": {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "variables sidebar": (49, 32, 241, 868), "variables main": (290, 32, 1150, 868)},
    "k-variables-floating": {"tab bar": (642, 0, 798, 32), "rail": (0, 388, 49, 512), "left panel": (49, 388, 241, 512), "right panel": (1199, 32, 241, 868),
                             "ruler top strip": (642, 32, 557, 20), "ruler left strip": (290, 388, 20, 512), "toolbar": _TOOLBAR, "floating variables": (0, 8, 642, 380)},
}

CAPS = {  # the editor-default cap where the region meets it, else the measured mean + 8 %, rounded up (every other cap has its cause below)
    "j-check-designs": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 15.0, "ruler left strip": 14.4, "toolbar": 3.8,
                        "check designs popover": 4.6},
    "j-comments": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 9.8, "right panel": 6.9, "ruler top strip": 14.3, "ruler left strip": 14.9, "toolbar": 3.8,
                   "thread popover": 1.86, "comment pin": 112.8},
    "j-find": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 3.4, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8, "find panel": 3.4},
    "j-version-history": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler left strip": 15.8, "toolbar": 3.8, "version bar": 4.2},
    "k-assets": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 4.3, "right panel": 6.9, "ruler top strip": 14.3, "ruler left strip": 14.9, "toolbar": 3.8},
    "k-variables": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "variables sidebar": 3.3, "variables main": 3.11},
    "k-variables-floating": {"screen": 5.22, "tab bar": 0.4, "rail": 2.6, "left panel": 12.8, "right panel": 6.9, "ruler top strip": 14.8, "ruler left strip": 15.8, "toolbar": 3.8,
                             "floating variables": 5.45},
}
_SUM = "the whole screen: the pinned regions of this state, counted together with the regions that meet their caps"
_RULERS = "the capture's rulers show another pan and the selected layer's band (-40000 to 0); Navigation/Rulers draws the 2 % view from 0"
_SCROLLED = ("the capture's Layers list is scrolled: its first row's glyph centre sits at y 335.5 of the panel where a row at rest is centred at 328.5 (measured on e-layers-selected, f-picker, "
             "j-comments and j-cmdk); a slot cannot express a scroll offset, so the screen keeps the 18 default rows")
_BEHIND_WINDOW = ("the capture's Layers list sits behind the floating Variables window; drawn from the top its rows scored worse than the 18 default rows (11.894 against 11.839), so the screen "
                  "keeps the default rows (the scroll offset was not measured)")
_SUBTITLE = ("the capture's header subtitle is 80 px wide; Panel/LeftLayers draws its own neutral `Drafts · Team project` at 124.5 px, and no data replaces that text")
CAUSES = {
    "j-check-designs": {
        "ruler top strip": _RULERS, "ruler left strip": _RULERS,
        "check designs popover": ("Panel/CheckDesigns: its crop rows pass with accepted findings (interior 198 vs 200 wide, mean 7.9); over the whole popover the replaced layer and "
                                  "variable names leave the 1-2 px text offset")},
    "j-comments": {
        "left panel": _SCROLLED, "ruler top strip": _RULERS, "ruler left strip": _RULERS,
        "thread popover": ("Popover/Panel `Kind=CommentThread` draws the thread (300 x 212, header, message, reply box, Reply): its message is masked, the heat map shows its text and "
                           "glyph edges"),
        "comment pin": "no component draws the 36 px comment pin (an orange disc with the author's initial): nothing is placed there",
        "worst tile": "the worst tile (280, 400) lies on the left ruler strip, a pinned region of this state"},
    "j-find": {
        "left panel": ("the Pages search button is pressed in the capture (Panel/Header has no pressed state) and the Find panel's text sits 1-2 px off (Panel/FindReplace crop "
                       "rows carry accepted findings, whole panel mean 6.4)"),
        "find panel": "the Find panel's pinned text-offset findings (whole panel mean 6.4 in its crop rows) over the replaced group and excerpt names"},
    "j-version-history": {
        "ruler left strip": ("the capture's left ruler starts below the version bar (y 72) and counts the starter file's range 400 to 2000; Navigation/Rulers draws the 2 % view "
                             "from y 52"),
        "version bar": ("Banner/Inline `Tone=Neutral` draws the `Viewing ...` bar (title, Restore this version, Back to current): its crop rows pass, the heat map shows text and glyph edges")},
    "k-assets": {
        "ruler top strip": _RULERS, "ruler left strip": _RULERS,
        "left panel": ("Panel/Assets text and glyph anti-aliasing (its crop rows carry accepted findings: category pills mean 13.79) over the replaced rows and previews: the heat map shows "
                       "the text and glyph edges of the header, the search field, the tabs and the Add more libraries button")},
    "k-variables": {
        "variables sidebar": ("Window/Variables sidebar text offset of 1-2 px (its crop row carries an accepted finding: sidebar 209 x 798 against 209 x 800, mean 3.57)"),
        "variables main": ("Window/Variables draws Table/Data's 18 default rows 521 px wide; the capture's rows and hairlines run across the 1150 px with the add column (recorded "
                           "gap of the region)")},
    "k-variables-floating": {
        "screen": _SUM, "left panel": _BEHIND_WINDOW, "ruler top strip": _RULERS, "ruler left strip": _RULERS,
        "floating variables": ("Window/Variables `Layout=Floating` draws the window: its ink box is 613 x 352 against the capture's 622 x 364, and the crop finding is not isolated"),
        "worst tile": "the worst tile (280, 400) lies on the left ruler strip, a pinned region of this state"},
}
WORST_TILE = {"j-comments": 39.17, "k-variables-floating": 39.17}
GEOMETRY = {
    "j-version-history": [
        {"key": "left-header-subtitle", "layout": [63.0, 71.0, 80.0, 18.0], "node": [63.0, 71.0, 124.5, 18.0], "cause": _SUBTITLE}],
    "k-variables-floating": [
        {"key": "scrollbar-track", "layout": [634.0, 89.0, 8.0, 247.0], "node": [281.0, 137.0, 8.0, 160.0],
         "cause": ("the floating window's own scrollbar (634, 89) is not drawn by Window/Variables `Layout=Floating`; the nearest node of that name is the Pages list's")}],
}
