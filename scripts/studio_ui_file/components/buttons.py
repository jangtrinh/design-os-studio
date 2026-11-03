"""The eight Button sets (registry `component`, row Actions), painted from the kit: look functions in widgets/kit/button.rs via
kit-states.json, geometry from `kit::button` / `kit::button_icon` and the callers' rects.

* Text buttons (Primary, Secondary, Tertiary, Destructive, SecondaryDestructive): the rect is the caller's. Heights are the
  control steps 32 / 36 / 40 (`CONTROL_XS/SM/MD`); the label is centred; the width a caller gives is the label plus
  2 * INSET_BUTTON (comments/ui.rs `kit_button_width`), so the horizontal padding is `inset/button` at every size (spec C says
  10 / 12 / 14: the Rust wins). Radius `RADIUS_CONTROL`. The kit paints no icon in a text button; spec C's optional leading
  icon is kept as a hidden `Has Leading Icon` part, off by default.
* Icon buttons (Icon, Tool, Close): a square rect, glyph centred; the glyph is 16 at Xs and 20 at Sm (`icon_px`) for Icon, 20 for a tool.
  Close: `widgets::icon_button` (widgets/mod.rs:144-150) is what paints every button-style close in 0.7.5 and it hard-codes
  `Hierarchy::Icon, Size::Xs`, so the glyph is LAYER_ICON_SIZE (16) at every rect size and the look is the Icon look
  (`button_look`, button.rs:42), which equals the Close look in Default, Hover, Active and Focus. `Hierarchy::Close` has no caller.
* Disabled is `half_color` on each paint: the fill, border, label and glyph are each at alpha 0.5 (a paint opacity, not a group).
* Focus adds the 2 px ring (`kit/focus.rs`): an instance of Focus/Ring, Error tone for the destructive pair.
"""
from rest_ir import F, I, T, X

TEXT_ROWS = ("Primary", "Secondary", "Tertiary", "Destructive", "SecondaryDestructive")
STATES = ["Default", "Hover", "Active", "Focus", "Disabled"]
HEIGHT = {"Xs": "CONTROL_XS", "Sm": "CONTROL_SM", "Md": "CONTROL_MD"}


def with_ring(root, look, env, error):
    """Append the focus ring to a variant whose look carries one; the layout pass sizes it to the host box + 8."""
    if look["ring"]:
        w, h = env.estimate(root)
        root["kids"].append(X("Focus ring", "Focus/Ring", "Tone=Error" if error else "Tone=Brand", abs=(-4, -4, w + 8, h + 8)))
    return root


def text_button(kind):
    row, error = f"Button/{kind}", kind in ("Destructive", "SecondaryDestructive")

    def build(values, env):
        size, look = values["Size"], env.look(row, values["State"])
        glyph = int(env.num("LAYER_ICON_SIZE") + (0 if size == "Xs" else env.num("SPACE_FACT")))
        stroke = {"stroke": look["border"], "sw": env.c("BORDER_WIDTH")} if look["border"] else {}
        kids = [I("Leading icon", "Plus", glyph, look["icon"], swap="Icon", visible_prop="Has Leading Icon", hidden=True, alpha=look["alpha"]),
                T("Label", "Label", "Text sm / Semibold", look["text"], prop="Label", alpha=look["alpha"])]
        root = F(kind, kids, dir="H", gap=env.c("SPACE_FACT"), pad=[None, env.c("INSET_BUTTON"), None, env.c("INSET_BUTTON")], fill=look["fill"],
                 radius=env.c("RADIUS_CONTROL"), h=env.num(HEIGHT[size]), sx="hug", sy="fixed", alpha=look["alpha"], **stroke)
        return with_ring(root, look, env, error)
    return build


def icon_button(row, icon, glyph_of, look_row=None):
    def build(values, env):
        size = values.get("Size", "Xs")
        look = env.look(look_row or row, values["State"])
        side = env.num("CONTROL_XS" if size == "Xs" else "CONTROL_SM") if row != "Button/Tool" else env.num("TOOL_BUTTON")
        glyph = glyph_of(size, env)
        root = F(row.split("/")[1], [I("Icon", icon, glyph, look["icon"], swap="Icon", alpha=look["alpha"])], dir="H", fill=look["fill"],
                 radius=env.c("RADIUS_CONTROL"), w=side, h=side, sx="fixed", sy="fixed", alpha=look["alpha"])
        return with_ring(root, look, env, False)
    return build


def small_glyph(size, env):
    return int(env.num("LAYER_ICON_SIZE") + (0 if size == "Xs" else env.num("SPACE_FACT")))


def close_glyph(size, env):
    return int(env.num("LAYER_ICON_SIZE"))


def tool_glyph(size, env):
    return int(env.num("TOOL_ICON_SIZE"))


def component_sets():
    out = [{"name": f"Button/{k}", "row": "Actions", "axes": {"Size": ["Xs", "Sm", "Md"], "State": STATES}, "uses": ["Focus/Ring"],
            "props": [{"name": "Label", "type": "TEXT", "default": "Label"}, {"name": "Has Leading Icon", "type": "BOOLEAN", "default": False},
                      {"name": "Icon", "type": "INSTANCE_SWAP", "default": "Plus"}], "build": text_button(k)} for k in TEXT_ROWS]
    swap = [{"name": "Icon", "type": "INSTANCE_SWAP", "default": "Search"}]
    out += [
        {"name": "Button/Icon", "row": "Actions", "axes": {"Size": ["Xs", "Sm"], "State": STATES + ["Selected"]}, "uses": ["Focus/Ring"],
         "props": swap, "build": icon_button("Button/Icon", "Search", small_glyph)},
        {"name": "Button/Tool", "row": "Actions", "axes": {"State": STATES + ["Selected"]}, "uses": ["Focus/Ring"],
         "props": swap, "build": icon_button("Button/Tool", "Pen", tool_glyph)},
        {"name": "Button/Close", "row": "Actions", "axes": {"Size": ["Xs", "Sm"], "State": ["Default", "Hover", "Active", "Focus"]},
         "uses": ["Focus/Ring"], "props": swap, "build": icon_button("Button/Close", "Close", close_glyph, look_row="Button/Icon")},
    ]
    return out
