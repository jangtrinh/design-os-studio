"""Navigation/BottomCanvasToolbar `State=Dev` and `State=DevAnnotate`: the Dev Mode dock and the `Re-center` pill above it (`show_dev_mode_toolbar`,
  panels/toolbar.rs:369-487; lines are the v0.7.5 tag).

The variant is the 266 x 88 box the screen places at (587, 800): the pill (131 x 32 at the dock's centre, `SPACE_INLINE` above it, toolbar.rs:393-398) and the dock (266 x 48 =
`TOOLBELT_PADDING` * 2 + 5 * `TOOL_BUTTON` + 4 * `TOOL_GROUP_GAP` + `TOOLBAR_DIVIDER_GAP_BEFORE` + `BORDER_WIDTH` + `TOOLBAR_DIVIDER_GAP_AFTER` + 36, toolbar.rs:371-379). The dock is BG_TOOLBAR at
RADIUS_FRAME with the 1 px INNER_HIGHLIGHT stroke (:414-415), five Button/Tool at `TOOL_GROUP_GAP` (Select, Annotate, Measure, Comment, Actions: `dev_tools`, :421-427), the BORDER divider
(:473-475) and the 36 x 32 DEV_READY toggle with a 16 px Dev glyph in TEXT_ON_BRAND (:476-487). `Dev` has Select and Measure Selected (the capture
  g-dev-react: the Move tool, rulers on). `DevAnnotate` is the
same dock while an annotation is being edited: `dev_tools` marks Select active only when `app.dev.annot.is_none()` (:422) and Annotate when
  `app.dev.annot.is_some()` (:423), so Annotate and Measure are
Selected and Select is not (the capture g-dev-annotate). It is a registry value, not a part: a nested variant override loses the glyph and the fill.
The pill sits at x 68 of the box (`(bar.center().x - recenter_w * 0.5).round()`, :395) and is the Secondary button look (`button_look(Secondary, ..)`,
  :400) at RADIUS_CONTROL: `Re-center` in `Text sm / Semibold` at its left + 10 and the `Shift+2` hint in body at its right - 10.
Deviation: the Rust's `↩` and `⇧` arrows are Unicode characters that Inter does not carry, so they render in the fallback font here.
"""
from rest_ir import F, I, T, X

DEV_TOOLS = ("Move", "Pen", "Spacing", "Comment", "Sparkle")
SELECTED = {"Dev": (0, 2), "DevAnnotate": (1, 2)}  # Select (the Move tool) and Measure (rulers on); Annotate and Measure while an annotation is edited
W, H, BAR_Y, PILL_W = 266.0, 88.0, 40.0, 131.0


def tile(env, name, icon, selected, x):
    state = "Selected" if selected else "Default"
    return X(name, "Button/Tool", f"State={state}", props={"Icon": icon}, abs=(x, 8.0, 32.0, 32.0))


def recenter(env):
    look = env.look("Button/Secondary", "Default")
    label = T("Label", "↩ Re-center", "Text sm / Semibold", look["text"], abs=(10.0, 5.0, 84.0, 20.0))
    hint = T("Hint", "⇧2", "Body / Regular", env.c("TEXT_QUATERNARY"), abs=(PILL_W - 10.0 - 20.0, 7.0, 20.0, 18.0))
    return F("dev-recenter", [label, hint], fill=look["fill"], stroke=look["border"], sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), w=PILL_W, h=32.0, sx="fixed", sy="fixed",
             abs=(68.0, 0.0, PILL_W, 32.0))


def dev_toolbar(env, state="Dev"):
    pad, tool, gap = env.num("TOOLBELT_PADDING"), env.num("TOOL_BUTTON"), env.num("TOOL_GROUP_GAP")
    kids = [tile(env, f"tool-{i}", icon, i in SELECTED[state], pad + i * (tool + gap)) for i, icon in enumerate(DEV_TOOLS)]
    x = pad + 5 * tool + 4 * gap + env.num("TOOLBAR_DIVIDER_GAP_BEFORE")
    kids.append(X("Divider", "Divider/Hairline", "State=Default", abs=(x, 1.0, 1.0, 46.0)))
    x += env.num("BORDER_WIDTH") + env.num("TOOLBAR_DIVIDER_GAP_AFTER")
    toggle = F("dev-mode-toggle", [I("Glyph", "Dev", int(env.num("LAYER_ICON_SIZE")), env.c("TEXT_ON_BRAND"))], dir="H", fill=env.c("DEV_READY"), radius=env.c("RADIUS_CONTROL"), w=36.0, h=32.0,
                sx="fixed", sy="fixed", abs=(x, 8.0, 36.0, 32.0))
    highlight = F("Highlight", stroke=env.c("INNER_HIGHLIGHT"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=W - 1, h=47.0, sx="fixed", sy="fixed", abs=(1.0, 1.0, W - 1, 47.0))
    dock = F("Toolbar", [highlight, *kids, toggle], fill=env.c("BG_TOOLBAR"), radius=env.c("RADIUS_FRAME"), w=W, h=48.0, sx="fixed", sy="fixed", abs=(0.0, BAR_Y, W, 48.0))
    return F("Dev toolbar", [recenter(env), dock], w=W, h=H, sx="fixed", sy="fixed")
