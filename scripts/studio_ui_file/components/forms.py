"""Registry `component` records of the Forms & Inputs row that are choices: Checkbox/Box, Toggle/Switch, Swatch/Color, Slider/Handle (and
Radio/Card, below). Each follows the painter 0.7.5 calls (D5: production is the truth). `kit-states.json` describes the kit looks
(widgets/kit/choice.rs), so a colour is read through `env.look`; sizes, radii and weights the Rust computes are written as the Rust computes them.

* Checkbox/Box: `kit::checkbox` (kit/choice.rs:99-113), look `:46-53`. The variant is the 16 px box (`LAYER_ICON_SIZE`, `:103-104`), the node the focus
  ring wraps (`:111`): the control rect (box and label hit area) is the caller's, not drawn. Radius `SPACE_FACT` (`:105`); the mark is `Icon::Check` at
  `FONT_SIZE_TEXT_XS` centred in the box (`:109`). Pressed shows the value the box already has (`held_value`, `:91-97`) and Hover is the Default look.
* Toggle/Switch: `kit::toggle` (kit/choice.rs:115-128), look `:37-44`. A 36 x 20 track (`SWITCH_W` = `CONTROL_SM`, `SWITCH_H` = `LAYER_ICON_SIZE` + 2 *
  `INSET_SEGMENT`, variables/widgets.rs:188-189) at `radius_full(h).round()`, a `W * 0.5` border, and a knob of radius `radius_full(LAYER_ICON_SIZE)` = 8
  inset `INSET_SEGMENT` from the end it sits at (`:123-125`). The ring wraps the track (`:126`).
* Swatch/Color: `widgets::swatch` -> `kit::swatch_ring` (widgets/mod.rs:153-170, kit/choice.rs:166-171): a `CHECKER_A` base, the colour on it, a
  `BORDER_TRANSLUCENT` edge on top, all at `SWATCH_RADIUS`. The colour is content, not a constant: the layer named `Color` is the one thing an instance
  may override (its fill).
* Radio/Card: the live permission rows of the Agent permissions modal (agent_approval_card.rs:256-289, identical on origin/main), D5 over the kit:
  `kit::radio_card` (kit/choice.rs:131-153) has no caller (production bug #1436: it rings a `RADIUS_XL` card at `RADIUS_CONTROL`) and is recorded as
  a gap. A 312 x 68 row (modal inner width, `CONTROL_MD` + `CONTROL_STANDARD`) at `RADIUS_CONTROL`, `BG_SELECTED` / `BG_HOVER` / `BG_TERTIARY` with a
  1 px `BORDER_SELECTED` / `BORDER` edge, a medium body title and a wrapping caption at `INSET_FRAME`. No radio circle, no ring, no disabled path:
  Focus is the Default look and Disabled is Default at half alpha on every paint (the kit's rule, kit/mod.rs:134-162).
* Slider/Handle: `kit::slider_handle` (kit/choice.rs:155-164), look `:71-77`. 0 production callers: a gap (spec C T8: 16 px, derived). It paints a disc
  in `look.icon` and a `border_w` stroke in `look.border` CENTRED on the disc's edge (`circle_stroke`); `look.fill` (BG_PRESSED) is never painted.
  The ring's host is a pill frame (radius = height / 2), never an Ellipse.
"""
from rest_ir import E, F, I, T, X, lit

RING = "Focus/Ring"
RING_BOX = "kit/choice.rs:105,111: the box is painted at SPACE_FACT (4) and paint_ring adds SPACE_FACT again, rounded (kit/focus.rs:38): 4 + 4 = 8, a sum with no variable"
RING_TRACK = "kit/choice.rs:121,126: the track is a pill of radius_full(20).round() = 10 and its ring is round(10 + SPACE_FACT) = 14 (kit/focus.rs:38), a sum with no variable"
RING_SWATCH = "kit/choice.rs:169-170: SWATCH_RADIUS (2) + SPACE_FACT (4) = 6 (kit/focus.rs:38), a sum with no variable"
TRACK_RADIUS = "kit/choice.rs:121 theme::radius_full(rect.height()).round() = 20 / 2 = 10: computed from the track's height, no radius role holds it"
TRACK_BORDER = "kit/choice.rs:38 border_w = W * 0.5 (BORDER_WIDTH / 2 = 0.5): no stroke-weight variable holds it"
HANDLE_RADIUS = "kit/choice.rs:159 radius = rect.width().min(rect.height()) * 0.5 = 16 / 2 = 8: computed from the handle's size, no radius role holds it"
HANDLE_STROKE = "kit/choice.rs:72 border_w = W * 2.0 (a 2 px stroke centred on the disc's edge, :161): computed in the Rust, with no stroke-weight constant in theme.rs; the Semantic variables that equal 2 are other roles"
SELECTED_EDGE = "kit/choice.rs:81 border_w = W * 2.0 (a 2 px band): computed in the Rust, with no stroke-weight constant in theme.rs; the Semantic variables that equal 2 are other roles"


ROW_W, ROW_H = 312, 68  # PERMISSIONS_MODAL_WIDTH 360 - 2 * INSET_MODAL 24; CONTROL_MD 40 + CONTROL_STANDARD 28 (agent_approval_card.rs:211,248-249)
TITLE = "Ask before acting (default)"
CAPTION = "Launch CLIs without skip flags and surface tool approvals in the Agents panel."


def ring_of(host_w, host_h, radius, reason):
    """The focus ring of a host node: the host grown by SPACE_FACT on every side. The layout pass sizes it from the node it sits in."""
    return X("Focus ring", RING, "Tone=Brand", abs=(-4, -4, host_w + 8, host_h + 8), radius=lit(radius, reason))


def checkbox(values, env):
    state = values["State"]
    look = env.look("Checkbox/Box", state)
    side = int(env.num("LAYER_ICON_SIZE"))
    stroke = {"stroke": look["border"], "sw": env.c("BORDER_WIDTH")} if look["border"] else {}
    kids = [I("Check", "Check", int(env.num("FONT_SIZE_TEXT_XS")), look["icon"])] if state == "Selected" else []
    if look["ring"]:
        kids.append(ring_of(side, side, 8, RING_BOX))
    return F("Box", kids, dir="H", fill=look["fill"], radius=env.c("SPACE_FACT"), w=side, h=side, sx="fixed", sy="fixed", alpha=look["alpha"], **stroke)


def toggle(values, env):
    state = values["State"]
    look = env.look("Toggle/Switch", state)
    w, h = env.num("CONTROL_SM"), env.num("LAYER_ICON_SIZE") + 2 * env.num("INSET_SEGMENT")
    d = int(env.num("LAYER_ICON_SIZE"))
    knob = E("Knob", d, d, fill=look["icon"], alpha=look["alpha"], sx="fixed", sy="fixed")
    kids = [knob] + ([ring_of(w, h, 14, RING_TRACK)] if look["ring"] else [])
    stroke = {"stroke": look["border"], "sw": lit(0.5, TRACK_BORDER)} if look["border"] else {}
    return F("Track", kids, dir="H", pad=env.c("INSET_SEGMENT"), align=("end" if state == "Selected" else "start", "center"), fill=look["fill"],
             radius=lit(h / 2, TRACK_RADIUS), w=w, h=h, sx="fixed", sy="fixed", alpha=look["alpha"], **stroke)


def swatch(values, env):
    state = values["State"]
    look = env.look("Swatch/Color", state)
    side = int(env.num("LAYER_ICON_SIZE"))
    weight = lit(2, SELECTED_EDGE) if state == "Selected" else env.c("BORDER_WIDTH")
    colour = F("Color", fill=env.c("TEXT"), stroke=look["border"], sw=weight, radius=env.c("SWATCH_RADIUS"), sx="fill", sy="fill")
    kids = [colour] + ([ring_of(side, side, 6, RING_SWATCH)] if look["ring"] else [])
    return F("Swatch", kids, dir="H", fill=env.c("CHECKER_A"), radius=env.c("SWATCH_RADIUS"), w=side, h=side, sx="fixed", sy="fixed")


def slider_handle(values, env):
    look = env.look("Slider/Handle", values["State"])
    side = int(env.num("LAYER_ICON_SIZE"))
    root = F("Handle", [], fill=look["icon"], stroke=look["border"], sw=lit(2, HANDLE_STROKE), sa="center", radius=lit(side / 2, HANDLE_RADIUS), w=side, h=side,
             sx="fixed", sy="fixed", alpha=look["alpha"])
    if look["ring"]:
        root["kids"].append(X("Focus ring", RING, "Tone=Brand", abs=(-4, -4, side + 8, side + 8)))
    return root


def permission_row(values, env):
    """The row of `paint_permissions_modal` (agent_approval_card.rs:256-289): fill by selected / hovered / neither (:260-266), a 1 px edge (:268-273), the
    title at (INSET_FRAME, INSET_FRAME) in `font_body_medium` TEXT (:275-278) and the caption `LINE_BODY` + `SPACE_FACT` below it in `font_caption`
    TEXT_SECONDARY, wrapped at the row width less 2 * INSET_FRAME (:279-286)."""
    state = values["State"]
    fill = {"Selected": "BG_SELECTED", "Hover": "BG_HOVER"}.get(state, "BG_TERTIARY")
    edge = "BORDER_SELECTED" if state == "Selected" else "BORDER"
    alpha = 0.5 if state == "Disabled" else 1.0
    inner = ROW_W - 2 * int(env.num("INSET_FRAME"))
    title = T("Title", TITLE, "Body / Medium", env.c("TEXT"), prop="Title", alpha=alpha)
    caption = T("Caption", CAPTION, "Caption / Regular", env.c("TEXT_SECONDARY"), prop="Caption", alpha=alpha, wrap=True, w=inner, sx="fill", line=18)
    side = env.c("INSET_FRAME")
    return F("Card", [title, caption], dir="V", gap=env.c("SPACE_FACT"), pad=[side, side, None, side], align=("start", "start"), fill=env.c(fill),
             stroke=env.c(edge), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), w=ROW_W, h=ROW_H, sx="fixed", sy="fixed", alpha=alpha)


def component_sets():
    states = ["Default", "Hover", "Focus", "Disabled"]
    return [
        {"name": "Checkbox/Box", "row": "Forms & Inputs", "axes": {"State": ["Selected"] + states}, "uses": [RING], "build": checkbox},
        {"name": "Toggle/Switch", "row": "Forms & Inputs", "axes": {"State": ["Selected"] + states}, "uses": [RING], "build": toggle},
        {"name": "Swatch/Color", "row": "Forms & Inputs", "axes": {"State": ["Selected", "Default", "Hover", "Focus"]}, "uses": [RING], "build": swatch},
        {"name": "Radio/Card", "row": "Forms & Inputs", "axes": {"State": ["Selected"] + states}, "build": permission_row,
         "props": [{"name": "Title", "type": "TEXT", "default": TITLE}, {"name": "Caption", "type": "TEXT", "default": CAPTION}]},
        {"name": "Slider/Handle", "row": "Forms & Inputs", "axes": {"State": ["Default", "Hover", "Active", "Focus", "Disabled"]}, "uses": [RING],
         "build": slider_handle},
    ]
