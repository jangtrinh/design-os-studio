"""The kit strings the wired screens replace with neutral ones, as the rects the screen gate leaves out of the comparison (`gap_wired_texts.json`, read by the `*_gate.py` modules).

A gate module is plain data that imports nothing of the package, and a state's TEXTS are declared by the one module that owns the state, so the rects the data modules imply are written once
to `gap_wired_texts.json` (`{state: [[x, y, w, h, what, neutral text], ...]}`, capture px) and each gate module adds `json[state]` to its own list. `build()` derives them from the data
(`python3 screens/gap_wired_texts.py` rewrites the file; a test fails when the two differ):

* the layer names of the left tree, at the indent of their row (`gap_left_dev_data.LEFT_TEXTS`);
* the selected layer's name field of every right inspector state that shows one, the text style row of the Text states, the seven property definitions of the Instance state and the
  interaction row of the Prototype state (`gap_right_panel_data`; the rects are the `named` rects of the captures' layout.json files);
* the style name, thread message, skill and layer picker names the overlays draw (`gap_overlays_data.TEXTS`);
* the page name and the layer names of the Dev list (`gap_left_dev_data.DEV`), and the collection headings, variable names and values of the Present Variables panel;
* the Compare dialog's title, the install form's hint and the Floating Variables table's names (rects read from the built variants' node rects).
  Not added: the Compare change-list layer names (they raise `g-dev-compare`'s screen mean 44.212 -> 44.556 and its dialog mean 84.102 -> 85.749:
  the dialog draws them as the capture does, the mask only removes matching pixels).
"""
import json
import pathlib
import sys

for _path in (pathlib.Path(__file__).resolve().parents[1], pathlib.Path(__file__).resolve().parents[2]):  # studio_ui_file/ and scripts/, as `python3 screens/gap_wired_texts.py` runs
    sys.path.insert(0, str(_path))
from screens import gap_left_dev_data as left_dev  # noqa: E402
from screens import gap_overlays_data as popovers  # noqa: E402
from screens import gap_right_panel_data as right  # noqa: E402

PATH = pathlib.Path(__file__).with_name("gap_wired_texts.json")
WINDOW = (1440, 900)
NAMED_STATES = ("Frame", "Layers", "Text", "Instance")  # the inspector states that show the selected layer's name field
TEXT_STYLE_ROW = (1216, 610, 160, 32)  # `text-style-row` of edit-style-text and f-type-details
PROPERTY_DEFINITIONS = [(1216, 238 + 76 * n, 156, 32) for n in range(7)]  # `propdef:*` of kit-editor
INTERACTION_ROW = (1207, 187, 224, 44)  # `proto-row-0` of f-prototype-tab
# the layer rows each Dev capture lists; g-dev-compare is not here: its scrim dims the list in both renders, so a mask there only removes matching pixels and raises the mean
DEV_LIST_ROWS = {"g-dev-react": 1, "g-dev-annotate": 1, "micro-dev-boxmodel": 15}
DEV_LIST_ITEM_TOP, DEV_LIST_PAGE_TOP = 126, 94  # from the top of the list region: the first item row, the page row (`Panel/DevInspect` `State=List`)
PRESENT_AT, PRESENT_GROUPS, PRESENT_PITCH = (1164, 56), (3, 2, 27), 22  # the panel's origin; the variables of each collection; heading and row pitch
INSTALL_HINT = (474, 368, 472, 20)  # the URL field's placeholder (`Placeholder` in the field): the screen's string is cut earlier than the painter's
FLOATING_NAMES = [(277, 100, 36), (293, 141, 47), (285, 182, 39), (285, 223, 42), (285, 264, 42), (293, 305, 48), (285, 346, 42)]  # collection and variable names of the Floating table: x, y, w
COMPARE_TITLE = (330, 148, 300, 20)  # the title at (20, 18) of the Compare dialog's card at (310, 130)


def _text(entry, path):
    return next(o["text"] for o in entry["overrides"] if o["path"] == path)


def build():
    out = {}

    def add(state, rect, what, neutral):
        x, y, w, h = rect
        if y < WINDOW[1]:  # a tree row cut by the bottom edge of the window keeps the part inside it
            out.setdefault(state, []).append([x, y, w, min(h, WINDOW[1] - y), what, neutral])

    for state, rects in left_dev.LEFT_TEXTS.items():
        for rect, what, neutral in rects:
            add(state, rect, what, neutral)
    for state, entry in right.RIGHT_PANEL.items():
        kind = entry["variant"]["State"]
        if kind in NAMED_STATES:
            add(state, right.NAME_RECT, "selected layer name", _text(entry, right.NAME))
        if kind == "Text":
            add(state, TEXT_STYLE_ROW, "text style name", _text(entry, right.TYPOGRAPHY + "Style row/Style field/Style name"))
        if kind == "Instance":
            for n, rect in enumerate(PROPERTY_DEFINITIONS, 1):
                add(state, rect, "property definition name", f"Property {n}")
        if kind == "Prototype":
            add(state, INTERACTION_ROW, "interaction row: trigger and target", "On click, Scroll to Frame 2")
    for state, rects in popovers.TEXTS.items():
        for rect, what, neutral in rects:
            add(state, rect, what, neutral)
    lx, ly = left_dev.LIST_AT
    for state, rows in DEV_LIST_ROWS.items():
        add(state, (lx + 16, ly + DEV_LIST_PAGE_TOP, 140, 32), "page name in the Dev list", "Page 1")
        for n in range(rows):
            add(state, (lx + 24, ly + DEV_LIST_ITEM_TOP + 32 * n, 90, 32), "layer name in the Dev list", f"Frame {n + 1}")
    px, py, y, n = PRESENT_AT[0] + 16, PRESENT_AT[1] + 52, 0, 0  # the first heading sits 54 px under the panel's top; a 2 px margin makes the rect
    for group, count in enumerate(PRESENT_GROUPS, 1):
        y = y or py
        add("present-variables", (px, y, 140, PRESENT_PITCH), "collection name", f"Collection {group}")
        for row in range(count):
            n += 1
            add("present-variables", (px, y + PRESENT_PITCH * (row + 1), 140, PRESENT_PITCH), "variable name", f"Name {n}")
            add("present-variables", (px + 124, y + PRESENT_PITCH * (row + 1), 108, PRESENT_PITCH), "variable value", f"Value {n}")  # to the right edge of the panel's text
        y += PRESENT_PITCH * (count + 1) + 4
    add("plugin-install", INSTALL_HINT, "install form hint", "Install hint")
    for n, (x, y, w) in enumerate(FLOATING_NAMES, 1):  # not on micro-components-variables: there the mask raises its left ruler strip mean 32.825 -> 33.788
        add("k-variables-floating", (x - 2, y - 1, max(w, 100), 20), "collection or variable name in the Floating table", f"Name {n}")
    add("g-dev-compare", COMPARE_TITLE, "compare dialog title: the frame name", "Compare changes • Frame 1")
    return out


def render(texts):
    """One rect a line, states in name order, so a change to one state is a small diff."""
    blocks = [f"{json.dumps(state)}: [\n" + ",\n".join(" " + json.dumps(rect, ensure_ascii=False) for rect in rects) + "\n]" for state, rects in sorted(texts.items())]
    return "{\n" + ",\n".join(blocks) + "\n}\n"


if __name__ == "__main__":
    PATH.write_text(render(build()), encoding="utf-8")
