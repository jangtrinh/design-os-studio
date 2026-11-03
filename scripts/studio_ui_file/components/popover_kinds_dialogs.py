"""The Popover/Panel kinds that are not `popover::row` rows: the two AI image popovers, the comment thread and the Agent permissions modal. Not a component module. Lines are the v0.7.5 tag of
crates/designos-app/src. Each is drawn at its capture's size from its own painter, parts placed with `abs`.

* Erase (248 x 144) and Expand (236 x 156): `image_jobs.rs:1440` `show_erase_popover` and `:1334` `show_expand_popover`: `menu_panel` at `RADIUS_FRAME`, the title in Body Semibold at
  `INSET_POPOVER`, then
  fields and two buttons `col_w` wide, 24 high. The buttons are drawn local (`rect_filled(.., RADIUS_CONTROL, BG_SECONDARY | BG_BRAND)`, not kit buttons). The Erase caption runs past the panel: the
  panel clips it, as the capture shows.
* CommentThread (300 x 212): `comments/thread.rs:122` `open_thread`: a 36 px header (Resolve and Close, `COMMENT_HEADER_H`), a message row (24 px avatar, author, age, the "..." button, the body), the
  reply box (`ui::text_box`, comments/ui.rs:63) and the `Ctrl+Enter to post` hint with the disabled Primary `Reply` button. Document data is neutral (`Agent`, `Align icon spacing to 8px grid`).
* Permissions (360 x 248): `agent_approval_card.rs:205` `paint_permissions_modal`: `BG_SECONDARY` at `RADIUS_FRAME` with a `BORDER` edge, the title, the two Radio/Card rows and the caption.
"""
from components.popover_kinds_controls import PROPERTY
from components.popover_kinds_shell import LABEL_X, hairline, surface
from rest_ir import E, F, T, X, lit

AVATAR_R = "comments/ui.rs:15 `circle_filled(center, diameter / 2.0, ..)`: radius half of the 24 px avatar, computed from its size"


def text(env, name, content, rect, style="Body / Regular", colour="TEXT", **kw):
    return T(name, content, style, env.c(colour), abs=rect, **kw)


def action_button(env, name, label, rect, primary):
    """`rect_filled(rect, RADIUS_CONTROL, BG_BRAND | BG_SECONDARY)` and a centred label (Body Semibold white on the brand fill, Body Regular TEXT on the grey)."""
    t = T("Label", label, "Body / Semibold" if primary else "Body / Regular", env.c("TEXT_WHITE" if primary else "TEXT"))
    return F(name, [t], dir="H", fill=env.c("BG_BRAND" if primary else "BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect)


def title_at(env, label, x=LABEL_X):
    return text(env, "Title", label, (x, 15.0, 200.0, 18.0), "Body / Semibold")


def clipped(env, kids, w, h):
    node = surface(env, kids, w, h)
    node["clip"] = True
    return node


def erase(env):
    w, col = 248.0, 104.0
    hint = text(env, "Hint", "e.g. the person on the left", (8.0, 3.0, 200.0, 18.0), colour="TEXT_TERTIARY")
    field = F("Instruction", [hint], fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=w - 32, h=24.0, sx="fixed", sy="fixed", abs=(16.0, 38.0, w - 32, 24.0))
    mask = text(env, "Mask", "Drag on image to mark erase region (optional)", (16.0, 63.0, 300.0, env.num("LINE_CAPTION")), "Caption / Regular", "TEXT_SECONDARY")
    kids = [title_at(env, "Erase from image"), field, mask, action_button(env, "Cancel", "Cancel", (16.0, 88.0, col, 24.0), False), action_button(env, "Run", "Erase", (128.0, 88.0, col, 24.0), True)]
    return clipped(env, kids, w, 144.0)


def expand(env):
    w, col = 236.0, 98.0
    preset = F("Preset", [T("Label", "+25%", "Mono / Value", env.c("TEXT"))], dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=52.0, h=24.0, sx="fixed", sy="fixed",
               abs=(w - 16 - 52.0, 14.0, 52.0, 24.0))
    side = lambda name, prefix, value, x, y: X(name, "Input/PropertyField", PROPERTY, props={"Prefix": prefix, "Value": value}, abs=(x, y, col, 24.0))
    kids = [title_at(env, "Expand image"), preset, side("Top", "T", "15px", 16.0, 40.0), side("Right", "R", "64px", 122.0, 40.0), side("Bottom", "B", "15px", 16.0, 72.0),
            side("Left", "L", "64px", 122.0, 72.0), action_button(env, "Cancel", "Cancel", (16.0, 108.0, col, 24.0), False), action_button(env, "Run", "Expand", (122.0, 108.0, col, 24.0), True)]
    return clipped(env, kids, w, 156.0)


def avatar(env, initial, rect):
    return F("Avatar", [T("Initial", initial, "Body / Semibold", env.c("TEXT"))], dir="H", fill=env.c("AVATAR_COLORS[0]"), radius=lit(rect[2] / 2, AVATAR_R), w=rect[2], h=rect[3], sx="fixed",
             sy="fixed", abs=rect)


def dots(env, x, y):
    """`dots_button` (comments/ui.rs:118-129, the dots :123-125): a 32 px hit box (`CONTROL_XS`) whose three 1.2 px dots sit 4 px apart on its centre."""
    ball = lambda n, dx: E(n, 2.4, 2.4, fill=env.c("ICON_TERTIARY"), sx="fixed", sy="fixed", abs=(16.0 + dx - 1.2, 14.8, 2.4, 2.4))
    return F("More", [ball("Dot 1", -4.0), ball("Dot 2", 0.0), ball("Dot 3", 4.0)], w=32.0, h=32.0, sx="fixed", sy="fixed", abs=(x, y, 32.0, 32.0))


def comment_thread(env):
    w, pad = 300.0, 12.0
    line = lambda name, y: hairline(env, name, y, w)
    resolve = X("Resolve", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "Check"}, abs=(w - 60.0, 6.0, 24.0, 24.0))
    close = X("Close", "Button/Close", "Size=Xs, State=Default", abs=(w - 32.0, 6.0, 24.0, 24.0))
    box = X("Reply box", "Input/Field", "Size=Xs, State=Default", props={"Has Value": False}, abs=(pad, 104.0, w - 2 * pad, 56.0))
    hint = text(env, "Reply hint", "Reply", (pad + 10, 114.0, 100.0, 18.0), colour="TEXT_PLACEHOLDER")
    post = X("Reply", "Button/Primary", "Size=Xs, State=Disabled", props={"Label": "Reply"}, w=56.0, h=32.0, abs=(w - pad - 56.0, 168.0, 56.0, 32.0))
    kids = [resolve, close, line("Header rule", 35.0), avatar(env, "A", (pad, 42.0, 24.0, 24.0)), text(env, "Author", "Agent", (44.0, 42.0, 40.0, 18.0), "Body / Semibold"),
            text(env, "Age", "just now", (88.0, 42.0, 60.0, env.num("LINE_CAPTION")), "Caption / Regular", "TEXT_TERTIARY"), dots(env, w - 44.0, 32.0),
            text(env, "Message", "Align icon spacing to 8px grid", (44.0, 64.0, 244.0, 18.0)), line("Reply rule", 91.0), box, hint,
            text(env, "Post hint", "Ctrl+Enter to post", (pad, 175.0, 130.0, env.num("LINE_CAPTION")), "Caption / Regular", "TEXT_TERTIARY"), post]
    return surface(env, kids, w, 212.0)


def permissions(env):
    w, inner = 360.0, 312.0
    card = lambda name, state, x, y, props: X(name, "Radio/Card", f"State={state}", props=props, abs=(x, y, inner, 68.0))
    skip = {"Title": "Skip permissions", "Caption": "Launch CLIs with their auto-approve flag. Fastest inside a trusted project."}
    kids = [text(env, "Title", "Agent permissions", (24.0, 24.0, 200.0, 18.0), "Body / Medium"), card("Skip permissions", "Default", 24.0, 54.0, skip),
            card("Ask before acting", "Selected", 24.0, 126.0, {}),
            text(env, "Caption", "Applies to new agent tabs.", (24.0, 206.0, 240.0, env.num("LINE_CAPTION")), "Caption / Regular", "TEXT_SECONDARY")]
    node = surface(env, kids, w, 248.0)
    node["fill"] = env.c("BG_SECONDARY")
    node["stroke"] = env.c("BORDER")
    return node


DIALOGS = {"Erase": erase, "Expand": expand, "CommentThread": comment_thread, "Permissions": permissions}
