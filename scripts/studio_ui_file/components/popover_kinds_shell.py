"""The shell of the Popover/Panel kinds (`Kind` other than Default): one production popover drawn from its painter, a header and label + control rows.
Not a component module (no `component_sets`): `discover` skips it. Lines are the v0.7.5 tag of crates/designos-app/src.

The shell is `widgets/popover.rs:17-50` `show` (header: title in Body Semibold at `LABEL_X`, a `BORDER` hairline at `HEADER_H - 1`, a 24 px close at (right - 32, top + 8)) and a row is
`popover::row` (widgets/popover.rs:53-60): the label in Body Regular TEXT_SECONDARY at x + 16, the control in the 112 px column at x + 112, 24 high, one row per `ROW_H` 32 from 8 below the header.
A popover with `n` rows is `HEADER_H + 8 + n * ROW_H + 8` high (`popover::height`, widgets/popover.rs:63-65). Every part is placed with `abs` at the rect that arithmetic
gives, the way the colour picker is,
and a kind is drawn at its capture's own size: an instance's absolutely placed children do not follow a resize.
"""
from components.popover_kinds_controls import CONTROL_X, ROW_H
from rest_ir import F, T, X

HEADER_H, ROW_PITCH, LABEL_X, BODY_TOP = 40.0, 32.0, 16.0, 8.0
LABEL_X_REASON = "widgets/popover.rs:9 `pub const LABEL_X: f32 = 16.0`: a local constant, no theme.rs name (INSET_PANEL and INSET_POPOVER hold 16 for other roles)"
LABEL_W = CONTROL_X - LABEL_X  # the label column: 96 px between the label and the control


def height(rows):
    return HEADER_H + BODY_TOP + rows * ROW_PITCH + BODY_TOP  # widgets/popover.rs:63-65


def hairline(env, name, y, w):
    """`hline(p, left, right, y, BORDER)` (widgets/popover.rs:31): the Divider/Hairline instance, the one BORDER_WIDTH line, sized to `w` and placed at `y`."""
    return X(name, "Divider/Hairline", "State=Default", abs=(0.0, y, w, env.num("BORDER_WIDTH")))


def header(env, title, w):
    """The popover header (widgets/popover.rs:29-34), parts placed straight in the popover: the title, the close button and the hairline at `HEADER_H - 1`."""
    return [T("Title", title, "Body / Semibold", env.c("TEXT"), abs=(LABEL_X, 11.0, w - 56.0, 18.0)), X("Close", "Button/Close", "Size=Xs, State=Default", abs=(w - 32.0, 8.0, 24.0, 24.0)),
            hairline(env, "Hairline", HEADER_H - 1, w)]


def label_text(env, text, x=LABEL_X, w=LABEL_W, dim=False, top=0.0):
    """A row label: Body Regular, TEXT_SECONDARY (TEXT_TERTIARY when the font lacks the feature, panels/typography.rs:537), centred in a `ROW_H` row whose top is `top`."""
    return T(f"{text} label", text, "Body / Regular", env.c("TEXT_TERTIARY" if dim else "TEXT_SECONDARY"), abs=(x, top + (ROW_H - 18) / 2, w, 18))


def place(label, controls, top):
    """The control nodes of a row, named after it and moved down to the row's `top` in the popover: a row is no frame of its own, which saves a node per row in every instance of the kind."""
    out = []
    for c in controls:
        names = {"Control": label, "Prefix icon": f"{label} icon", "Dial": f"{label} dial"}
        x, y, w, h = c["abs"]
        out.append(dict(c, name=names.get(c["name"], c["name"]), abs=(x, y + top, w, h)))
    return out


def rows_nodes(env, rows, w):
    """`popover::row` for every (label, controls) of `rows`: the label at x 16 and the controls from x 112, rows `ROW_PITCH` apart from 8 below the header (widgets/popover.rs:53-60)."""
    out = []
    for i, (label, controls) in enumerate(rows):
        top = HEADER_H + BODY_TOP + i * ROW_PITCH
        out += [label_text(env, label, top=top), *place(label, controls, top)]
    return out


def surface(env, kids, w, h):
    """`kit::popover_surface` (widgets/kit/surface.rs:60-62): BG, a BORDER_MENU edge, RADIUS_FRAME."""
    return F("Popover", kids, fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=w, h=h, sx="fixed", sy="fixed")


def settings_popover(env, title, rows, w=240.0):
    return surface(env, [*header(env, title, w), *rows_nodes(env, rows, w)], w, height(len(rows)))
