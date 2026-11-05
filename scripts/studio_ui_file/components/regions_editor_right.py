"""Registry `region` Panel/RightInspector (page `03 Studio`, row Regions): the right panel with nothing selected (panels/right.rs `show`, panels/properties.rs `page_section`, `styles_section`,
`styles_list`). Lines are the 0.7.5 tag of crates/designos-app/src. A 241 x 868 region (layout.json `regions.right_panel`: the 1 px left border + 240), BG, its children placed where
  that code places
them (all y are from the region's top) and the named widgets carrying the layout.json key as their name.

* Top row (y 8, `RIGHT_BTN_Y`, 24 high): the avatar pill (`right-top-avatar`, (8, 8, 38, 24)) with a 20 px disc and its initial in `font_caption_strong`, then a 10 px ChevronDown; the
  Present button (`present-button`,
  right - 102, 24 square, a 14 px Play) and its chevron cell (`present-chevron`, 16 wide); the Share button (`share-button`, 48 wide, right - 56) in the info colour at radius 6 with
    `Share` in `font_body_strong`.
* Tabs (y 48, `RIGHT_TABS_Y`): Tabs/Pill Selected for Design (`right-tab-Design`) at x 9 and Default for Prototype (`right-tab-Prototype`), 4 px apart; the zoom dropdown at right - 63
  (55 x 24, no fill at
  rest): `2%` in `font_body` at +8 and a 10 px ChevronDown 10 from its right edge. A BORDER hairline at y 80 (`RIGHT_CONTENT_TOP` - 1) from x 1.
* The content (from y 81, clipped at the footer, y 832): a Section/Header `Page` (40 high; its action is the Opacity glyph, `properties.rs:405`), the page field at (17, 125, 184,
  32): BG_SECONDARY at
  `RADIUS_CONTROL`, a 12 px swatch of the page colour at x 6, the hex in `font_body` at x 24, a 1 px BG divider at x 129, the opacity at x 138 and a `%` 7 px from the right; the eye button
    (Button/Icon Xs resized to 24, an Eye glyph) at (209, 125);
  a hairline, a Section/Header `Styles` without its action, the `Text styles` label (`font_body`, TEXT_SECONDARY, y 222, 18 high), then 32 px rows from y 249: a folder row (a 12 px
    chevron at +14 in ICON_TERTIARY,
  the name at +28 in TEXT_SECONDARY) followed by four style rows (`Ag` in `font_body_strong` at +22, the name at +48, `size/line` right-aligned 10 px from the edge in TEXT_TERTIARY): the
    folders and styles are
  neutral names with the weights of a type ramp, as production lists a kit's text styles in this shape. A 4 px thumb at the top of an 8 px track (`scrollbar-track`).
* Footer (y 832, 36 high): BG with a BORDER hairline on top and the 28 px help button (`help-button`, centred 20 from the right edge): a 14 px disc, a stroke and a 14 px Help glyph.
Inline `Color32` literals in this painter have no variable (right.rs:37-139): the region draws the nearest theme tone by value (smallest per-channel difference), as the colour picker does:
| literal | tone | distance |
| avatar 0xF59E0B (right.rs:36) | WARNING (234, 179, 8) | 21 |
| share 0x0D99FF (right.rs:72) | INFO (13, 153, 255) | 0 |
| help disc 0x2C2C2C (right.rs:132) | BG_TERTIARY (38, 38, 38) | 6 |
| help stroke 0x383838 (right.rs:134) | BG_PRESSED (64, 64, 64) | 8 |
| help glyph 0xB3B3B3 (right.rs:137) | ICON_TERTIARY (163, 163, 163) | 16 |
The page colour is content (the page's own background, #F5F5F5 in the capture): the swatch binds READER_LIGHT_SURFACE_2, the theme constant that holds that value.
The Page state is the variant above (State=Page, first, unchanged). The other six states keep this shell (top row, tabs, hairline, footer, border) and differ in the tabs, the Share colour and the
content below the hairline (`panels/right.rs:11-181` `show` picks it: `properties::show` for the Design tab, `prototype::show`, the Dev tabs, :108-110, :147-153):
* Frame, Layers, Text, Instance: `properties::show` with a selection (`selection_ui`, properties.rs:823), the sections of `regions_editor_right_sections` / `_paints` / `_states` composed per state.
* Prototype: the Prototype tab is the selected pill (`regions_editor_right_proto`).
* Dev: the tabs read Inspect | Plugins, Share is green (right.rs:66 `0x1BC47D`: SUCCESS, distance 31) and the content area is left empty for Panel/DevInspect, which covers it.
Gaps: the dev-mode tab contents, the export section below the styles and the Colour, Effect and Grid style groups are other states or scroll; a selection state draws the sections whose title
  is visible in its capture (the rest scrolls below the footer).
"""
from components import regions_editor_right_states as states
from rest_ir import F, I, T, X

W, H = 241.0, 868.0  # layout.json regions.right_panel
STATES = ("Page", "Frame", "Layers", "Text", "Instance", "Prototype", "Dev")
TABS = {"Design": (("Design", True, 56.25), ("Prototype", False, 71.5)), "Prototype": (("Design", False, 55.25), ("Prototype", True, 73.25)),
        "Dev": (("Inspect", True, 59.0), ("Plugins", False, 57.25))}  # layout.json right-tab-<label>: the selected pill is wider (font_body_strong)
FOOTER_TOP, CONTENT_TOP = 832.0, 81.0
FOLDERS = ("Heading 1", "Heading 2", "Heading 3", "Heading 4")
WEIGHTS = ("Regular", "Medium", "Semibold", "Bold")
SIZES = ("72/90", "60/72", "48/60", "36/44")  # the type ramp's font size / line, one per folder, as the capture lists them (numbers, not names)


def box(env, name, x, y, w, h, kids=(), **kw):
    return F(name, list(kids), w=w, h=h, sx="fixed", sy="fixed", abs=(x, y, w, h), **kw)


def chevron(env, x, y):
    return I("Chevron", "ChevronDown", int(env.num("CHEVRON_SMALL")), env.c("ICON_SECONDARY"), abs=(x, y, 10, 10))


def top_row(env, dev=False):
    y, h = env.num("RIGHT_BTN_Y"), 24.0
    disc = F("Disc", [T("Initial", "J", "Caption / Semibold", env.c("TEXT_WHITE"))], dir="H", fill=env.c("WARNING"), radius=env.c("RADIUS_FRAME"), w=20.0, h=20.0, sx="fixed", sy="fixed",
             abs=(3.0, 2.0, 20.0, 20.0))
    avatar = box(env, "right-top-avatar", 8, y, 38, h, [disc, chevron(env, 25.0, 7.0)])
    present_x = W - env.num("RIGHT_PRESENT_FROM_RIGHT") - 46
    present = box(env, "present-button", present_x, y, h, h, [I("Play", "Play", 14, env.c("ICON"), abs=(5.0, 5.0, 14, 14))])
    cell = box(env, "present-chevron", present_x + h, y, env.num("RIGHT_PRESENT_CHEVRON_W"), h, [chevron(env, 3.0, 7.0)])
    label = T("Label", "Share", "Body / Semibold", env.c("TEXT_WHITE"))
    share = box(env, "share-button", W - env.num("RIGHT_PRESENT_FROM_RIGHT"), y, 48.0, h, [label], dir="H", fill=env.c("SUCCESS" if dev else "INFO"), radius=env.c("RADIUS_SEGMENT"))
    return [avatar, present, cell, share]


def tabs_row(env, tab="Design"):
    y = env.num("RIGHT_TABS_Y")
    zoom = box(env, "Zoom", W - env.num("RIGHT_ZOOM_FROM_RIGHT"), y, env.num("RIGHT_ZOOM_W"), env.num("RIGHT_TABS_H"), [
        T("Zoom value", "2%", "Body / Regular", env.c("TEXT"), abs=(8.0, 3.0, 20.0, 18.0)),
        I("Chevron", "ChevronDown", int(env.num("CHEVRON_SMALL")), env.c("ICON_SECONDARY"), abs=(40.0, 7.0, 10, 10))])
    tabs, x = [], env.num("RIGHT_TABS_X")
    for label, selected, w in TABS[tab]:
        tabs.append(X(f"right-tab-{label}", "Tabs/Pill", f"State={'Selected' if selected else 'Default'}", props={"Label": label}, abs=(x, y, w, env.num("RIGHT_TABS_H"))))
        x += w + 4
    return tabs + [zoom]


def page_field(env, y):
    g = env.num("SECTION_CONTENT_X")
    swatch = F("Swatch", fill=env.c("READER_LIGHT_SURFACE_2"), radius=env.c("RADIUS_MARK"), w=env.num("PAGE_SWATCH"), h=env.num("PAGE_SWATCH"), sx="fixed", sy="fixed",
               abs=(env.num("PAGE_SWATCH_CX") - env.num("PAGE_SWATCH") / 2, 10.0, env.num("PAGE_SWATCH"), env.num("PAGE_SWATCH")))
    divider = F("Divider", fill=env.c("BG"), w=1.0, h=env.num("FIELD_HEIGHT"), sx="fixed", sy="fixed", abs=(env.num("PAGE_DIVIDER_X"), 0.0, 1.0, env.num("FIELD_HEIGHT")))
    row = lambda name, text, x, w: T(name, text, "Body / Regular", env.c("TEXT"), abs=(x, 7.0, w, 18.0))
    texts = [row("Hex", "F5F5F5", env.num("PAGE_HEX_X"), 50.0), row("Opacity", "100", env.num("PAGE_DIVIDER_X") + 1 + env.num("PAGE_OPACITY_PAD"), 24.0),
             row("Percent", "%", env.num("SECTION_CONTENT_W") - env.num("PAGE_PERCENT_RIGHT") - 9.0, 9.0)]
    fy, fw = y + env.num("PAGE_FIELD_DY"), env.num("SECTION_CONTENT_W")
    field = box(env, "Page field", g, fy, fw, env.num("FIELD_HEIGHT"), [swatch, divider] + texts, fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"))
    side = env.num("PANEL_ACTION_BUTTON")  # widgets::icon_button is Button/Icon Xs: resized to 24, as Section/Header places it
    eye = X("Eye button", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "Eye"}, abs=(env.num("SECTION_TRAILING_X"), fy, side, side))
    return [field, eye]


def style_row(env, y, kind, label, size=None):
    """A folder row (chevron, name) or a style row (`Ag`, name, size) of `styles_list`: 224 x 32 at x 8, children placed by the painter's offsets."""
    h, row_w, indent = env.num("STYLE_ROW_H"), 224.0, env.num("STYLE_INDENT")
    text = lambda name, content, style, colour, x, w: T(name, content, style, env.c(colour), abs=(x, 7.0, w, 18.0))
    if kind == "folder":
        kids = [I("Chevron", "ChevronDown", 12, env.c("ICON_TERTIARY"), abs=(8.0, 10.0, 12, 12)), text("Name", label, "Body / Regular", "TEXT_SECONDARY", 28.0, 80.0)]
    else:
        kids = [text("Ag", "Ag", "Body / Semibold", "TEXT", 10.0 + indent, 14.0), text("Name", label, "Body / Regular", "TEXT", 36.0 + indent, 60.0),
                text("Size", size, "Body / Regular", "TEXT_TERTIARY", row_w - 10.0 - 36.0, 36.0)]
    return box(env, f"{kind}-{y:g}", 8.0, y, row_w, h, kids)


def page_content(env):
    """The Content frame of State=Page: the page field and the styles list, every child placed (`properties::show` with nothing selected, properties.rs:404-552)."""
    y0, h = CONTENT_TOP, env.num("SECTION_TITLE_HEIGHT")
    inner = [X("Page header", "Section/Header", "State=Default", props={"Title": "Page", "Has Action": False, "Show Opacity action": True}, abs=(0.0, 0.0, 239.0, h))]
    inner += page_field(env, h)
    y = h + env.num("PAGE_FIELD_DY") + env.num("FIELD_HEIGHT") + env.num("PAGE_BOTTOM_PAD")
    inner += [X("Page rule", "Divider/Hairline", "State=Default", abs=(0.0, y, W - 1, 1)),
              X("Styles header", "Section/Header", "State=Default", props={"Title": "Styles", "Has Action": False}, abs=(0.0, y + 1, 239.0, h))]
    y += 1 + h + env.num("STYLES_LABEL_GAP")
    inner.append(T("Text styles", "Text styles", "Body / Regular", env.c("TEXT_SECONDARY"), abs=(env.num("SECTION_CONTENT_X"), y, 80.0, 18.0)))
    y += 18.0 + env.num("STYLES_ROW_GAP")
    while y < FOOTER_TOP - y0:
        folder = len([k for k in inner if k["name"].startswith("folder-")])
        inner.append(style_row(env, y, "folder", FOLDERS[folder % 4]))
        y += env.num("STYLE_ROW_H")
        for name in WEIGHTS:
            if y >= FOOTER_TOP - y0:
                break
            inner.append(style_row(env, y, "style", name, SIZES[folder % 4]))
            y += env.num("STYLE_ROW_H")
    return box(env, "Content", 1.0, y0, W - 1, FOOTER_TOP - y0, inner, clip=True)


def right_inspector(values, env):
    state = values.get("State", "Page")
    tab = state if state in ("Prototype", "Dev") else "Design"
    kids = top_row(env, state == "Dev") + tabs_row(env, tab) + [X("Tabs rule", "Divider/Hairline", "State=Default", abs=(1.0, CONTENT_TOP - 1, W - 1, 1))]
    kids.append(page_content(env) if state == "Page" else states.content(env, state))
    if state != "Prototype":  # the Prototype tab's content never overflows the region, so no scrollbar thumb (f-prototype-tab names none)
        kids.append(scrollbar_track(env))
    glyph = I("Help", "Help", 14, env.c("ICON_TERTIARY"), abs=(7.0, 7.0, 14, 14))
    disc = F("Disc", [glyph], fill=env.c("BG_TERTIARY"), stroke=env.c("BG_PRESSED"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=28.0, h=28.0, sx="fixed", sy="fixed",
             abs=(0.0, 0.0, 28.0, 28.0))
    help_btn = box(env, "help-button", W - 20.0 - 14.0, FOOTER_TOP + 18.0 - 14.0, 28.0, 28.0, [disc])
    kids += [X("Footer rule", "Divider/Hairline", "State=Default", abs=(1.0, FOOTER_TOP, W - 1, 1)), help_btn, X("Left border", "Divider/Hairline", "State=Default", abs=(0.0, 0.0, 1, H))]
    return F("Panel", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed")


def scrollbar_track(env):
    thumb = F("Thumb", fill=env.c("TEXT_TERTIARY"), radius=env.c("RADIUS_MARK"), w=4.0, h=24.0, sx="fixed", sy="fixed", abs=(2.0, 0.0, 4.0, 24.0))
    return box(env, "scrollbar-track", W - 8.0, CONTENT_TOP, 8.0, FOOTER_TOP - CONTENT_TOP, [thumb])


def component_sets():
    uses = ["Button/Icon", "Checkbox/Box", "Divider/Hairline", "Section/Header", "Select/Trigger", "Tabs/Pill"]
    return [{"name": "Panel/RightInspector", "row": "Regions", "axes": {"State": list(STATES)}, "uses": uses, "build": right_inspector}]
