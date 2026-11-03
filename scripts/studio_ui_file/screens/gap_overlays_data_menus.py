"""Per-screen data of the open menus: the nine Menu/Panel overlays with their rows. Plain dicts, no `screens()`, no `_gate.py` suffix; `gap_wiring.overlays` places them.

    MENU    {state: [Menu/Panel place entry, ...]}: the open menus of that capture, each a Menu/Panel whose Body slot holds Menu/Item and Menu/Separator instances.

What the data adds over the rows the screens draw today:
* `Show check column` (Menu/Item): in a menu where any row is checkable every label sits at `MENU_LABEL_X_CHECK` 32, or 60 with an icon (`Layout::has_check`, widgets/menu.rs:166-199, :177 is the
  `any(.. checkable)`). The captures of the File submenu, the shape menu, the zoom menu (panels/menus.rs:704-801 `show_zoom_menu`) and the grid and effect style menus show their labels at 32 or 60,
  so every row of those carries it; a Selected row has its check anyway. The main menu and the effects menu have none.
* The zoom menu draws a separator under its input (panels/menus.rs:752-754: a `BORDER_MENU` line at `input_box.bottom() + 4 + MENU_SEP_MARGIN`, the rows from `input_box.bottom() + 4 +
  sep_h`), so its entry is `[ZOOM_FIELD, SEP, rows...]`. The rows then sit 2 px below production's (the input row is 32 high where production's is 26) instead of 15 px above.
* Group captions are Menu/Item `State=Disabled`; the effects menu's glyphs and its `Soon` hint, and the layer picker's kind icons, are data (a swap and a text).
* The AI image menu keeps `Size=Xs`: `popup_menu` is called with the default row height (image_jobs.rs:1294-1332 has no `popup_menu_with_row_h`), and the capture's rows are 32 px, so
  `Size=Sm` (the 36 px Home menus) does not match and no data changes.
* The layer picker (`show_layer_picker`, panels/context.rs:169-241) stays a second painter: its caption is Menu/Item Disabled (Body Medium at 16) where production draws Caption Regular
  at 12, and the panel's top padding is 8, so every row sits 8 px lower than production's (which starts at the caption's 32 px); its icons sit 4 px right and its labels 6 px right. The
  separator under the caption is left out: Menu/Separator is 17 px where production draws a flush 1 px line (panels/context.rs:198), and with it every row sits 25 px low and the last
  one ends 19 px past the 294 px panel. Region means of the screen gate on this data: no separator 11.780 unmasked and 8.345 with the eight TEXTS masks; Menu/Separator 12.646 unmasked
  and 6.966 with the masks. The masks hide more of a displaced render's labels, so the masked figure prefers the separator; the unmasked one does not.
"""


def row(label, state="Default", icon=None, hint=None, sub=False, check=False, size="Xs"):
    """One Menu/Item entry. `icon` is a registry icon name (a swap of the icon box), `hint` the shortcut text, `check` the check column."""
    props = {"Label": label, "Has Icon": icon is not None, "Has Shortcut": hint is not None, "Has Submenu": sub}
    if check:
        props["Show check column"] = True
    entry = {"name": label, "component": "Menu/Item", "variant": {"Size": size, "State": state}, "props": props}
    overrides = ([{"path": "Shortcut slot/Shortcut/Keys", "text": hint}] if hint is not None else []) + ([{"path": "Icon slot/Icon box/Icon", "swap": f"Icon/{icon}"}] if icon else [])
    if overrides:
        entry["overrides"] = overrides
    return entry


SEP = "-"


def panel(name, rect, rows, sized=False):
    """A Menu/Panel at rect (x, y, w[, h]): its items are `w - 16` wide (the 8 px pill inset each side), its separators span the panel. `sized` sets the panel's height from the rect."""
    x, y, w = rect[:3]
    body, n = [], 0
    for r in rows:
        if r == SEP:
            n += 1
            body.append({"name": f"Separator {n}", "component": "Menu/Separator", "variant": {"State": "Default"}, "w": w})
        else:
            body.append(dict(r, w=w - 16))
    place = {"name": name, "component": "Menu/Panel", "variant": {"State": "Default"}, "x": x, "y": y, "w": w, "slots": {"Body": body}}
    if sized:
        place["h"] = rect[3]
    return place


def checked(rows):
    """The rows of a menu with a check column: every item but a separator shows it."""
    out = []
    for r in rows:
        if r != SEP and r["variant"]["State"] != "Selected":
            r = dict(r, props=dict(r["props"], **{"Show check column": True}))
        out.append(r)
    return out


MAIN = [row("Back to files"), SEP, row("Actions...", icon="Search", hint="Ctrl+K"), SEP] + [row(n, sub=True) for n in ("File", "Edit", "View", "Object", "Text", "Arrange", "Vector")] + [
    SEP, row("Plugins"), row("Widgets", state="Disabled", sub=True), row("Preferences", sub=True), row("Libraries"), SEP, row("AI balance"), SEP, row("Help and account", sub=True)]
FILE = [row("New design file"), row("New board"), row("New deck"), row("Open...", hint="Ctrl+O"), row("Import web capture..."), row("Save", hint="Ctrl+S"), row("Save as...", hint="Ctrl+Shift+S"),
        row("Save .fig copy"), row("Save to library...", state="Disabled"), SEP, row("Show version history"), row("Save to version history...", state="Disabled", hint="Ctrl+Alt+S"), SEP,
        row("Export selection as PNG...", state="Disabled")]
SHAPE = [row("Rectangle", state="Selected", icon="Rectangle", hint="R"), row("Line", icon="Line", hint="L"), row("Ellipse", icon="Ellipse", hint="O"), row("Polygon", icon="Polygon"),
         row("Star", icon="Star"), row("Image/video...", icon="Image", hint="Ctrl+Shift+K")]
ZOOM_FIELD = {"name": "Zoom field", "component": "Input/Field", "variant": {"Size": "Xs", "State": "Focus"}, "overrides": [{"path": "Value", "text": "2%"}]}
ZOOM = [row("Zoom in", hint="⌘+"), row("Zoom out", hint="⌘-"), row("Zoom to fit", hint="⇧1"), row("Zoom to 50%"), row("Zoom to 100%", hint="⌘0"), row("Zoom to 200%"), SEP,
        row("Pixel grid", state="Selected", hint="⌘'"), row("Snap to pixel grid", state="Selected", hint="⇧⌘'"), row("Layout guides", state="Selected", hint="^G"),
        row("Rulers", state="Selected", hint="⇧R"), SEP, row("Comments", hint="⇧C")]
EFFECTS = [row(n, icon=g) for n, g in (("Inner shadow", "InnerShadow"), ("Drop shadow", "DropShadow"), ("Layer blur", "LayerBlur"), ("Background blur", "BackgroundBlur"),
                                       ("Noise", "Noise"), ("Texture", "Texture"), ("Glass", "Glass"))] + [SEP, row("Shader", state="Disabled", icon="Shader", hint="Soon")]
GRID = [row(f"Grid style {i}") for i in (1, 2, 3)] + [row(f"{n} columns (auto)") for n in (12, 6, 5, 3, 2)] + [row("Icon grid"), SEP, row("Create grid style")] + [
    row(n, state="Disabled") for n in ("Rename style", "Detach style", "Delete style", "Edit style")]
STYLE_GROUPS = (("Group 1", 7), ("Group 2", 5), ("Group 3", 7), ("Group 4", 4))


def style_rows():
    """The effect style list: a Disabled caption per group, then its style rows, the first style selected (the capture's 27 rows between the captions)."""
    rows, n = [], 0
    for caption, count in STYLE_GROUPS:
        rows.append(row(caption, state="Disabled"))
        for _ in range(count):
            n += 1
            rows.append(row(f"Style {n}", state="Selected" if n == 1 else "Default"))
    return rows


STYLES = style_rows()
AI = [row("Remove background"), row("Isolate subject"), SEP, row("Expand..."), row("Erase...")]
LAYERS = [row("Select layer", state="Disabled")] + [row(n, state="Hover" if i == 2 else "Default", icon=g) for i, (n, g) in enumerate(
    (("Text 1", "Text"), ("Frame 2", "AutoLayoutV"), ("Frame 3", "AutoLayoutH"), ("Frame 4", "AutoLayoutV"), ("Frame 5", "AutoLayoutV"), ("Frame 6", "AutoLayoutV"), ("Frame 7", "AutoLayoutV"),
     ("Frame 8", "AutoLayoutV")))]

MENU = {
    "d-menu-main": [panel("Main menu", (12, 44, 210), MAIN)],
    "d-menu-main-file": [panel("Main menu", (12, 44, 210), [dict(r, variant={"Size": "Xs", "State": "Hover"}) if r != SEP and r["name"] == "File" else r for r in MAIN]),
                         panel("File submenu", (222, 142, 265), checked(FILE))],
    "d-menu-shape": [panel("Shape menu", (619, 636, 246), checked(SHAPE))],
    "d-menu-zoom": [panel("Zoom menu", (1216, 44, 212), [ZOOM_FIELD, SEP] + checked(ZOOM))],
    "f-effects-menu": [panel("Effects menu", (1264, 571, 168, 289), EFFECTS, sized=True)],
    "grid-styles": [panel("Grid styles menu", (1220, 417, 184, 481), checked(GRID), sized=True)],
    "f-effect-styles": [panel("Effect styles menu", (1134, 2, 304, 896), checked(STYLES), sized=True)],
    "ai-image-menu": [panel("AI image menu", (1216, 668, 180, 161), AI, sized=True)],
    "micro-layer-picker": [panel("Layer picker", (680, 410, 220, 294), LAYERS, sized=True)],
}
