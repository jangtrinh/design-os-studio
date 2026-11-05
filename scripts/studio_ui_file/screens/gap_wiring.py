"""Places the per-screen data of the `gap_*_data` modules on the screens of page `04 Screens`. Plain functions, no `screens()`.

    regions(spec)      what a shell's regions draw in this capture: the right inspector's state and values (`gap_right_panel_data`), the left panel's layer trees and the Dev list
                       (`gap_left_dev_data`), the active rail and toolbar tool (`gap_tool_states_data`), the Variables and Home tab bar parts (`gap_variables_present_data`),
                       the dirty tab's dot (`gap_tab_bar_data`) and the agents panel's state (`gap_agents_data`), and the overlays' aliases. Applied by the four spec
                       constructors (`w4a_common.screen`, `w4b_shell.screen`, `w4c_shell.spec`, `w4d_common.screen`).
    overlays(state)    the instances of the overlay the capture shows (popover, menu, vector bar, dialog, colour picker, command palette, banner): a screen module appends them to its shell.
    aliases(state)     the named widgets of the capture that those overlays and dialogs draw, and the dirty tab's dot; `regions` merges them into the spec's `aliases`.

A `patch` entry `{component: {key: value}}` edits the instance the shell already places: a key set to None is removed, a `variant` is merged into the placed one. An `add` entry is placed
after Panel/LeftLayers when it is the Dev list, else last. Every entry is deep-copied: two screens may share one `place` list (`w4c_present`) and one entry feeds several states.
"""
import copy

from screens import gap_agents_data as agents
from screens import gap_dialogs_data as dialogs
from screens import gap_left_dev_data as left_dev
from screens import gap_overlays_data as popovers
from screens import gap_overlays_data_menus as menus
from screens import gap_right_panel_data as right_panel
from screens import gap_tab_bar_data as tab_bar
from screens import gap_tool_states_data as tools
from screens import gap_variables_present_data as variables_present

RIGHT, LEFT, TERMINAL, DEV_INSPECTOR = "Panel/RightInspector", "Panel/LeftLayers", "Panel/AgentTerminal", "Panel/DevInspect"
PATCHES = (left_dev.DEV, variables_present.VARIABLES_PRESENT, tools.TOOLS, tab_bar.TAB_BAR)
DIALOG_RECORDS = ((dialogs.DIALOG, "Dialog/Modal"), (dialogs.PICKER, "Popover/ColorPicker"), (dialogs.PALETTE, "Dialog/CommandPalette"), (dialogs.BANNER, "Banner/Inline"))


def _patch(placed, patch):
    for key, value in copy.deepcopy(patch).items():
        if value is None:
            placed.pop(key, None)
        elif key == "variant" and placed.get("variant"):
            placed[key] = {**placed["variant"], **value}
        else:
            placed[key] = value


def _add(place, entry):
    entry = copy.deepcopy(entry)
    is_list = entry["component"] == DEV_INSPECTOR and entry["variant"].get("State") == "List"
    after = [i for i, p in enumerate(place) if p["component"] == LEFT]
    place.insert(after[-1] + 1 if is_list and after else len(place), entry)


def regions(spec):
    """`spec` with its shell's regions drawing the state the capture shows; `spec["place"]` is replaced by a copy."""
    state = spec["state"]
    spec["place"] = place = copy.deepcopy(spec["place"])
    if aliases(state):
        spec["aliases"] = {**(spec.get("aliases") or {}), **aliases(state)}
    for p in place:
        if p["component"] == RIGHT and state in right_panel.RIGHT_PANEL:
            entry = right_panel.RIGHT_PANEL[state]
            p["variant"], p["overrides"] = dict(entry["variant"]), copy.deepcopy(entry["overrides"])
        if p["component"] == LEFT and state in left_dev.LEFT:
            entry = left_dev.LEFT[state]
            p["slots"] = copy.deepcopy(entry["slots"])
            p["overrides"] = list(p.get("overrides") or []) + copy.deepcopy(entry.get("overrides", []))
        if p["component"] == TERMINAL and state in agents.AGENTS:
            _patch(p, agents.AGENTS[state])
    for data in PATCHES:
        for component, patch in data.get(state, {}).get("patch", {}).items():
            for p in place:
                if p["component"] == component:
                    _patch(p, patch)
        for entry in data.get(state, {}).get("add", []):
            _add(place, entry)
    return spec


def _record(name, entry):
    return {"name": name, "component": name, "variant": copy.deepcopy(entry["variant"]), "x": entry["xy"][0], "y": entry["xy"][1], **copy.deepcopy(entry["extra"])}


def overlays(state):
    """The overlay instances of `state`, in paint order: popover, menus, vector bar, then the dialog, colour picker, palette and banner entries."""
    out = [copy.deepcopy(popovers.POPOVER[state])] if state in popovers.POPOVER else []
    out += copy.deepcopy(menus.MENU.get(state, [])) + ([copy.deepcopy(popovers.VECTOR[state])] if state in popovers.VECTOR else [])
    return out + [_record(name, table[state]) for table, name in DIALOG_RECORDS if state in table]


def aliases(state):
    return {**popovers.ALIASES.get(state, {}), **dialogs.ALIASES.get(state, {}), **tab_bar.ALIASES.get(state, {})}
