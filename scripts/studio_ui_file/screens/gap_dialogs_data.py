"""Per-screen data of the dialogs, colour pickers, command palettes and version banner: plain dicts keyed by the capture's state, no `screens()` and no gate table.

A screen module reads one entry and places the component with it (`at(name, component, entry["xy"], entry["variant"], **entry["extra"])`).
Every entry names its `Kind` / `Tone`: today's look is the first value of each axis (Libraries and Plugins, Fill, Neutral), so a placement that names no variant draws it too.

    DIALOG    Dialog/Modal at the capture's overlay rect (`k-libraries`, `j-plugins`, `plugin-install`, `g-dev-compare`; the scrim stays the screen's, `SCRIMS`)
    PICKER    Popover/ColorPicker at the capture's picker rect (`f-picker`, `picker-page`, `f-picker-gradient`; `layout.json` `picker` / the `color picker` overlay)
    PALETTE   Dialog/CommandPalette with the Rust placeholder string of its painter (`cmdk.rs` hint, `panels/menus.rs` hint)
    BANNER    Banner/Inline at the history bar's rect (`j-version-history`: `history-banner` (290, 32, 909, 40))
"""
ALIASES = {  # the capture's named widgets that the dialog kinds draw: layout key -> path of names below the screen, joined by ` > `
    "plugin-install": {"plugin-url": "Dialog/Modal > Body > URL field", "plugin-install-cancel": "Dialog/Modal > Footer > Cancel", "plugin-install-submit": "Dialog/Modal > Footer > Primary action"},
    "g-dev-compare": {'btn-"compare-done"': "Dialog/Modal > Close"},
}
DEFAULT = "Default"
CMDK_HINT = "Search actions or type layout prompt (↵ Local Spec · ⌃` Live Agent CLI)"  # cmdk.rs `hint_text(...)` of the search field
ACTIONS_HINT = "Search..."  # panels/menus.rs `hint_text(...)` of the palette's search field


def _entry(xy, variant, **extra):
    return {"xy": xy, "variant": variant, "extra": extra}


DIALOG = {
    "k-libraries": _entry((440, 130), {"Kind": "Libraries", "State": DEFAULT}),
    "j-plugins": _entry((440, 230), {"Kind": "Plugins", "State": DEFAULT}),
    "plugin-install": _entry((440, 230), {"Kind": "InstallPlugin", "State": DEFAULT}),
    "g-dev-compare": _entry((310, 130), {"Kind": "Compare", "State": DEFAULT}),
}
PICKER = {
    "f-picker": _entry((960, 346), {"Kind": "Fill"}),
    "picker-page": _entry((960, 123), {"Kind": "Page"}),
    "f-picker-gradient": _entry((960, 332), {"Kind": "Gradient"}),
}
PALETTE = {
    "j-cmdk": _entry((500, 512), {"Kind": "CmdK", "State": DEFAULT}, props={"Placeholder": CMDK_HINT}),
    "j-palette": _entry((460, 438), {"Kind": "Actions", "State": DEFAULT}, props={"Placeholder": ACTIONS_HINT}),
}
BANNER = {
    "j-version-history": _entry((290, 32), {"Tone": "Neutral"}, overrides=[{"path": "Title", "text": "Viewing Initial layout"}]),
}
