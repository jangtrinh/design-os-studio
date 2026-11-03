"""Registry `composite` Bar/Contextual and Toast/Default (page `03 Studio`, row Bars & Feedback). Lines are the v0.7.5 tag.

Bar/Contextual (no variants: a plain COMPONENT, rule (j)): `panels/toolbar.rs:149-211` `contextual_bar`, 1 caller (toolbar.rs:220). `paint_frame` at `RADIUS_XL` with `BG_TOOLBAR` and an
`INNER_HIGHLIGHT` edge (:169-177), `CONTROL_MD` high. From the left: `SPACE_FACT` of padding, two `button_look(Icon, ..)` buttons of `CONTROL_XS` at a `CONTROL_XS` + `INSET_SEGMENT` step (:164,
183-195: Rectangle and Pen = two Button/Icon Xs, swapped), the divider `vline` (:196-197, `BORDER`, 1 px wide, 6 px inside the top and bottom: a 28 px line) 5 px after the second button's edge
and 4 px before "More" (the step's 2 px gap, `cx += 3`, `cx += 5`, a 1 px line), and "More" (:199-205), then `SPACE_FACT`. Nested (live): 2 x Button/Icon Xs. Local: the frame; the buttons' group
(its 2 px gap is `INSET_SEGMENT`, and its 1 px of right padding, a literal, makes the 5 and 4 of the line's two sides come out of one 4 px frame gap, a `lit` too: the Rust's raw `3.0` and `5.0`); the divider (Divider/Hairline's main is
horizontal, so a local `BORDER` line of the bar's height minus 12, centred by the row: no padding literal); "More", which is the `button_look(Tertiary, ..)` look
(Button/Tertiary's, via `env.look`) with the label at `INSET_CONTROL` and a trailing ChevronDown at `INSET_CONTROL` from the right, an anatomy Button/Tertiary does not have, so it is a local frame
(deviation recorded). The width is the content's (`ctx_w`, :162-165). Gaps: `mark_focus` rings (:142-146, :190, :206): Focus is not a registry state for this record. No capture shows the bar (no
`contextual-floating-bar` in any layout.json): UNVERIFIED-VISUAL.

Toast/Default (State=Default): `shell.rs:1193-1289` `toast`, geometry in `shell/toast_layout.rs`, 1 caller. Kit `toast_surface` (`widgets/kit/surface.rs:76-79`, `toast_look` :46-49):
`BG_PRIMARY_ALT`, a `BORDER_MENU` edge, `RADIUS_XL`, `TOAST_HEIGHT` 40 high (a one-line message: `layout` takes the larger of the text plus 2 x `PAD_VERTICAL` and 40). The dot is the
`circle_filled(.., 4.0, ICON_BRAND_PRIMARY | DANGER)` of shell.rs:1259-1260 = Status/Dot Info (8 px), centred on the first text row, `DOT_INSET` 16 from the left (absolute at (12, 16)). The message
starts at `PAD_LEFT` 28 in Text sm Medium in TEXT; the action (`toast_action_look`, button.rs:126-137) is a `CONTROL_XS` high box at `RADIUS_CONTROL` of its label + 2 x `ACTION_PAD` 8, the label
Text sm Semibold in TEXT_SECONDARY, no fill at rest, `ACTION_GAP` 12 after the message and `PAD_RIGHT` 16 from the right edge. Nested (live): Status/Dot. Local: the frame, the message and the action:
census of the action against Button/Tertiary: same height (32), radius and label style, but Tertiary pads 12 (`INSET_BUTTON`) not 8 and colours its label TEXT_TERTIARY, not TEXT_SECONDARY, so a
Button/Tertiary copy would draw the wrong box and colour and an instance cannot override either: a local frame (deviation recorded). The painter paints no ring unless keyboard-focused (:1266-1268),
so Focus is not drawn. Gaps: `PAD_LEFT`, `PAD_RIGHT`, `ACTION_GAP` and `ACTION_PAD` are local constants of toast_layout.rs with no theme.rs name (each a `lit` with its Rust line; SPACE_GROUP and INSET_CONTROL only hold the same 12 and 8); a message of several lines wraps at the 720 max and the
toast grows (the frame hugs); the error tone (DANGER dot), the second action ("Details" beside "Reveal") and the timing (`TOAST_SECONDS`, reveal) are not drawn; no toast rect is in any layout.json:
UNVERIFIED-VISUAL.
"""
from rest_ir import F, I, T, X, lit

NO_VARIANTS = "A plain component: the registry gives Bar/Contextual no variants, so it is one component, not a set. Drawn from `contextual_bar` (panels/toolbar.rs:149-211); no capture shows it"
DIVIDER_BAND = 12.0  # toolbar.rs:196 `ctx_bar.top() + 6.0 .. ctx_bar.bottom() - 6.0`: the line is the bar's height minus 6 on each side
TO_LINE = "toolbar.rs:196-197: the buttons end 2 px short of the step (cx), `cx += 3` puts the 1 px line 5 px after the second button, and `cx += 5` puts More 4 px after the line: 4 px of frame gap + 1 px here"
PAD_LEFT = "shell/toast_layout.rs:14 PAD_LEFT = 28: a local constant, no theme.rs name"
PAD_RIGHT = "shell/toast_layout.rs:15 PAD_RIGHT = 16: a local constant, no theme.rs name (INSET_PANEL and INSET_POPOVER hold 16 for other roles)"
ACTION_GAP = "shell/toast_layout.rs:18 ACTION_GAP = 12: a local constant, no theme.rs name (SPACE_GROUP holds 12 for other roles)"
ACTION_PAD = "shell/toast_layout.rs:20 ACTION_PAD = 8: a local constant, no theme.rs name (INSET_CONTROL and INSET_FRAME hold 8 for other roles)"
BAR_GAP = "toolbar.rs:196-198: `cx += 3.0` before the 1 px line and `cx += 5.0` after it are raw literals: the line sits 5 px after the second button and More 4 px after the line (5 less the line's own 1 px): the frame gap is that 4, and the 1 px left over before the line is Actions' right padding"
DOT = 8.0  # shell.rs:1260 `circle_filled(.., 4.0, ..)`: a radius of 4 = SPACE_FACT
DOT_INSET = 16.0  # shell/toast_layout.rs:27 DOT_INSET: the dot's centre


def contextual_bar(values, env):
    xs, band = env.num("CONTROL_XS"), env.num("CONTROL_MD")
    icon = lambda name, glyph: X(name, "Button/Icon", "Size=Xs, State=Default", props={"Icon": glyph})
    actions = F("Actions", [icon("Select area", "Rectangle"), icon("Edit with prompt", "Pen")], dir="H", gap=env.c("INSET_SEGMENT"), pad=[None, lit(1, TO_LINE), None, None])
    divider = F("Divider", fill=env.c("BORDER"), w=env.num("BORDER_WIDTH"), h=band - DIVIDER_BAND, sx="fixed", sy="fixed")
    look = env.look("Button/Tertiary", "Default")
    more = F("More", [T("Label", "More", "Text sm / Semibold", look["text"]), I("Chevron", "ChevronDown", int(env.num("LAYER_ICON_SIZE")), look["icon"])], dir="H",
             gap=env.c("SPACE_FACT"), pad=[None, env.c("INSET_CONTROL"), None, env.c("INSET_CONTROL")], radius=env.c("RADIUS_CONTROL"), h=xs, sx="hug", sy="fixed")
    return F("Bar", [actions, divider, more], dir="H", gap=lit(4, BAR_GAP), pad=[None, env.c("SPACE_FACT"), None, env.c("SPACE_FACT")], fill=env.c("BG_TOOLBAR"),
             stroke=env.c("INNER_HIGHLIGHT"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_XL"), h=band, sx="hug", sy="fixed", align=("start", "center"))


def toast(values, env):
    look = env.look("Toast/Default", "Default")
    h = env.num("TOAST_HEIGHT")
    dot = X("Dot", "Status/Dot", "Tone=Info", abs=(DOT_INSET - DOT / 2, (h - DOT) / 2, DOT, DOT))
    message = T("Message", "Message", "Text sm / Medium", look["text"], prop="Message")
    label = T("Label", "Action", "Text sm / Semibold", env.c("TEXT_SECONDARY"), prop="Action")
    action = F("Action", [label], dir="H", pad=[None, lit(8, ACTION_PAD), None, lit(8, ACTION_PAD)], radius=env.c("RADIUS_CONTROL"), h=env.num("CONTROL_XS"),
               sx="hug", sy="fixed", visible_prop="Has Action")
    return F("Toast", [dot, message, action], dir="H", gap=lit(12, ACTION_GAP), pad=[None, lit(16, PAD_RIGHT), None, lit(28, PAD_LEFT)], fill=look["fill"], stroke=look["border"],
             sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_XL"), h=h, sx="hug", sy="fixed", align=("start", "center"))


def component_sets():
    return [
        {"name": "Bar/Contextual", "description": NO_VARIANTS, "row": "Bars & Feedback", "axes": {}, "uses": ["Button/Icon"], "build": contextual_bar},
        {"name": "Toast/Default", "row": "Bars & Feedback", "axes": {"State": ["Default"]}, "uses": ["Status/Dot"],
         "props": [{"name": "Message", "type": "TEXT", "default": "Message"}, {"name": "Action", "type": "TEXT", "default": "Action"}, {"name": "Has Action", "type": "BOOLEAN", "default": True}],
         "build": toast},
    ]
