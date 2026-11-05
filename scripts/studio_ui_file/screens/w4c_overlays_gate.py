"""The gate data of the dialog and presentation states of batch W4C (`screens/w4c_dialogs.py`, `w4c_present.py`): `j-cmdk`, `j-palette`, `j-plugins`, `plugin-install`,
`k-libraries`, `x-present`, `present-variables`. The panel and variables states are in `w4c_gate.py`; the conventions are the ones its docstring gives.
"""
import json
import pathlib

# the rects the wired data modules imply (gap_wired_texts.py writes the file): a state's kit strings the screen replaces with neutral ones, added to this module's own
_WIRED = {state: [(tuple(r[:4]), r[4], r[5]) for r in rects] for state, rects in json.loads(pathlib.Path(__file__).with_name("gap_wired_texts.json").read_text(encoding="utf-8")).items()}
_TOOLBAR = (464, 840, 512, 48)
_CONTENT_AREA = ((310, 52, 889, 848), _TOOLBAR)
# the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
_TAB, _FILE = ((116, 8, 185, 16), "project tab title", "Design file"), ((57, 40, 184, 24), "file name", "Design file")
_PAGES = [((61, 137 + 32 * i, 212, 32), "page name", n) for i, n in enumerate(["Page 1", "Page 2", "——————", "SECTION", "↳ Page 3"])]
_LAYERS = [((100, 345, 82, 555), "layer names of the whole Layers list (the name column; kind icon, chevron and type label count)",
            "Component 1, Component 2, Frame 1 ... Frame 16")]
_FOLDERS = [((1235, 281 + 160 * k, 110, 32), "text style folder", f"Heading {k + 1}") for k in range(4)]
_SHELL = [_FILE, _TAB] + _PAGES + _LAYERS
_EDITOR = {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, 241, 868), "right panel": (1199, 32, 241, 868), "ruler top strip": (310, 32, 889, 20),
           "ruler left strip": (290, 52, 20, 848), "toolbar": _TOOLBAR}
_PRESENT_TEXTS = [((16, 12, 200, 24), "flow name in the flow pill", "Frame 1"), ((620, 12, 200, 24), "frame title, centred", "Frame 1")]
_PAGE = (129, 80, 1182, 788)  # the presented page: the document's own frames, a kit's in the capture, the page colour only in the screen
_NO_TOOLBAR = (_PAGE[0], _PAGE[1], 0, 0)  # CONTENT is a (content, toolbar) pair: a presentation has no toolbar, so the pair's second rect is empty
_DIALOGS = ("j-plugins", "plugin-install", "k-libraries")

TEXTS = {"j-cmdk": _SHELL, "j-palette": _SHELL + _FOLDERS, "j-plugins": _SHELL + _FOLDERS, "plugin-install": _SHELL + _FOLDERS, "k-libraries": _SHELL,
         "x-present": _PRESENT_TEXTS, "present-variables": _PRESENT_TEXTS}
TEXTS = {s: texts + _WIRED.get(s, []) for s, texts in TEXTS.items()}
CONTENT = {s: _CONTENT_AREA for s in ("j-cmdk", "j-palette", *_DIALOGS)} | {"x-present": (_PAGE, _NO_TOOLBAR), "present-variables": (_PAGE, _NO_TOOLBAR)}
KEEP = {"j-cmdk": [(500, 512, 440, 320)], "j-palette": [(460, 438, 520, 394)], "j-plugins": [(440, 230, 560, 440)], "plugin-install": [(440, 230, 560, 440)],
        "k-libraries": [(440, 130, 560, 640)]}
REGIONS = {
    "j-cmdk": {**_EDITOR, "command palette": (500, 512, 440, 320)},
    "j-palette": {**_EDITOR, "command palette": (460, 438, 520, 394)},
    "j-plugins": {**_EDITOR, "dialog": (440, 230, 560, 440)},
    "plugin-install": {**_EDITOR, "dialog": (440, 230, 560, 440)},
    "k-libraries": {**_EDITOR, "dialog": (440, 130, 560, 640)},
    "x-present": {"present bar": (0, 0, 1440, 48), "page colour": (0, 48, 1440, 852)},
    "present-variables": {"present bar": (0, 0, 1440, 48), "page colour": (0, 48, 1164, 852), "variables panel": (1164, 56, 264, 836)},
}
SCRIMS = {s: {"variable": "color/bg-overlay", "reason": "BG_OVERLAY (#262626 at 0xB3) over the whole window, painted before the dialog"} for s in _DIALOGS}

CAPS = {  # the editor-default cap where the region meets it, else the measured mean + 8 %, rounded up (every other cap has its cause below)
    "j-cmdk": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 9.8, "right panel": 6.9, "ruler top strip": 14.3, "ruler left strip": 14.9, "toolbar": 3.8,
               "command palette": 2.69},
    "j-palette": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8,
                  "command palette": 1.6},
    "j-plugins": {"screen": 4.6, "tab bar": 1.3, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8, "dialog": 1.27},
    "plugin-install": {"screen": 4.6, "tab bar": 1.3, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8,
                       "dialog": 2.7},
    "k-libraries": {"screen": 4.6, "tab bar": 1.3, "rail": 2.6, "left panel": 3.9, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8, "dialog": 4.64},
    "x-present": {"screen": 0.1, "present bar": 0.1, "page colour": 0.1},
    "present-variables": {"screen": 4.6, "present bar": 0.42, "page colour": 0.1, "variables panel": 2.86},
}
_RULERS = "the capture's rulers show another pan and the selected layer's band (-40000 to 0); Navigation/Rulers draws the 2 % view from 0"
_SCROLLED = ("the capture's Layers list is scrolled: its first row's glyph centre sits at y 335.5 of the panel where a row at rest is centred at 328.5 (measured on e-layers-selected, f-picker, "
             "j-comments and j-cmdk); a slot cannot express a scroll offset, so the screen keeps the 18 default rows")
_DIM = "the window is under the scrim: the engine blends BG_OVERLAY over the surface to level 30 where the capture has 29, one level over the whole region"
_PAGE_ONLY = "the presentation screen counts only the bar and the page colour (measured 0.0 and 0.013): the editor-default 4.6 would let a broken bar pass"
CAUSES = {
    "j-cmdk": {
        "left panel": _SCROLLED, "ruler top strip": _RULERS, "ruler left strip": _RULERS,
        "command palette": ("Dialog/CommandPalette Kind=CmdK draws the prompt placeholder: the heat map shows the text edges (egui against the renderer) over the 1 px label offset of its "
                            "crop rows"),
        "worst tile": "the worst tile (280, 400) lies on the left ruler strip, a pinned region of this state"},
    "j-palette": {
        "command palette": ("text edges of Dialog/CommandPalette Kind=Actions (egui against the renderer) with the 1 px label offset of its crop rows: measured 1.48, "
                            "the region has no editor-default cap")},
    "j-plugins": {
        "tab bar": _DIM,
        "dialog": ("Dialog/Modal `Kind=Plugins` draws the Tools glyph in a 60 px row and the three left-aligned paragraphs: their foreground box is 81 rows against the capture's 78 (the "
                   "wrapped `Undo.` line sits 3 to 4 px lower: the Body line is 18 px where the painter's row is 15)")},
    "plugin-install": {
        "tab bar": _DIM,
        "dialog": ("Dialog/Modal `Kind=InstallPlugin` draws the form (URL field with its ring, notes, Cancel, disabled Install): the heat map shows text and glyph edges and the field's outline, "
                   "and the hint, which Input/Field Sm draws in Text sm 14 where the painter draws font_body 12, so the string is cut earlier")},
    "k-libraries": {
        "tab bar": _DIM,
        "left panel": "the capture's left panel is not a layer tree here (the Libraries dialog and its scrim cover it), so no tree data applies and Panel/LeftLayers keeps its default rows",
        "dialog": ("Dialog/Modal `Kind=Libraries` draws the This file row, the publish rows, the description field, Publish and the libraries list: its text rows have no crop row of "
                   "their own, so what is left is measured by this mean only"),
        "worst tile": "the worst tile (520, 320) lies in the dialog's body (the publish rows), a pinned region of this state"},
    "x-present": {"screen": _PAGE_ONLY, "present bar": _PAGE_ONLY, "page colour": _PAGE_ONLY},
    "present-variables": {
        "page colour": _PAGE_ONLY,
        "present bar": ("Bar/Presentation `Show Variables active` draws the toggle on: the heat map shows the text and glyph edges of the bar (measured 0.42)"),
        "variables panel": ("Window/Variables `Layout=Present` draws the panel (264 x 836 at (1164, 56)) with its collection headings, names and values masked: what the heat map shows is the "
                            "panel's corner arcs (2 px) and its rows' swatches and text edges")},
}
WORST_TILE = {"j-cmdk": 39.17, "k-libraries": 41.27}
