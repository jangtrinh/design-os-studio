"""Type ramp, spacing ladder, inset and control samples, radius nesting. All bound by construction.

Each function returns the content column of one Foundations section: (kit, plan[, styles]) -> spec.
"""
SAMPLE = "The quick brown fox jumps over the lazy dog 0123456789"
# Rows pay no vertical inset: the column gap between rows pays each boundary once (soul: pay each boundary once).
ROW_PAD = [None, "inset/control", None, "inset/control"]


def _title(kit, text):
    return kit.text("Section title", text, style="Title / Semibold")


def _sem(plan, group):
    return [s for s in plan["semantic"] if s["group"] == group]


def _px(v):
    return f"{v:g}px"


def type_content(kit, plan, styles):
    rows = []
    for st in styles:
        ts, name = st["text_style"], st["name"]
        line = f"{ts['line_height']['Pixels']:g}" if isinstance(ts["line_height"], dict) else "auto"
        spec = f"{ts['font_family']} {ts['font_size']:g}/{line} weight {ts['font_weight']}"
        rows.append(kit.row(f"Type {name}", [kit.text("Style", name, color="color/text-tertiary", width=200),
                                             kit.text("Sample", SAMPLE, style=name, width=640),
                                             kit.text("Spec", spec, color="color/text-tertiary", width=260)],
                            gap="space/inline", pad=ROW_PAD))
    return kit.column("Type specimen", [_title(kit, "Type")] + rows, gap="space/row")


def spacing_content(kit, plan):
    rows = [kit.row(f"Space {s['name']}", [
        kit.text("Name", s["name"], width=160), kit.text("Value", _px(s["value"]), color="color/text-tertiary", width=64),
        kit.box("Bar", kind="rectangle", fill="color/bg-brand", radius="radius/mark", width=kit.V(s["name"]), height=8)],
        gap="space/inline", pad=ROW_PAD) for s in _sem(plan, "space")]
    return kit.column("Spacing specimen", [_title(kit, "Spacing ladder")] + rows, gap="space/row")


# P4: the radius of the element that owns each inset. A segmented control, a control and a button are RADIUS_CONTROL 8; a frame,
# panel, popover and modal are RADIUS_FRAME 16. The tile inside is concentric: owner radius - inset, so 0 where the inset reaches it.
INSET_OUTER = {"segment": "radius/control", "control": "radius/control", "frame": "radius/frame", "button": "radius/control",
               "panel": "radius/frame", "popover": "radius/frame", "modal": "radius/frame"}
# P4: colour badges take 6, the large badge 8; every other control takes 8.
CONTROL_RADIUS = {"badge-sm": "radius/segment", "badge-md": "radius/segment"}
# Gate allow-list: the one sample that pads with a ladder rung because theme.rs has no 4 px inset role.
CARD_PATH = "01 Foundations/Radius nesting/Radius specimen/Concentric samples/Card holds control/Frame"
ALLOW = [{"path": CARD_PATH, "property": p, "value": 4.0, "reason": "P4 holds a card's controls at a 4 px inset (RADIUS_XL 12 - 4 = RADIUS_CONTROL 8) but theme.rs has "
          "no 4 px inset role and P3 forbids a ladder rung as an inset, so the 4 stays a literal until theme.rs names it"}
         for p in ("paddingTop", "paddingRight", "paddingBottom", "paddingLeft")]


def concentric_radius(plan, outer, inset_px):
    """Radius variable of a tile `inset_px` inside a frame of radius `outer` (outer - inset), or None when that is 0."""
    value = {s["name"]: s["value"] for s in _sem(plan, "radius")}
    inner = max(0.0, value[outer] - inset_px)
    if inner == 0:
        return None
    match = [n for n in ("radius/mark", "radius/segment", "radius/control", "radius/xl", "radius/frame") if value[n] == inner]
    if not match:
        raise SystemExit(f"no radius variable equals {outer} - {inset_px} = {inner}")
    return match[0]


def inset_control_content(kit, plan):
    def inset_tile(s):
        outer = INSET_OUTER[s["name"].split("/")[1]]
        inner = concentric_radius(plan, outer, s["value"])
        return kit.column(f"Inset {s['name']}", [
            kit.text("Name", s["name"]), kit.text("Value", f"{_px(s['value'])}  {outer}  {inner or '0'}", color="color/text-tertiary"),
            kit.box("Frame", direction="row", pad=s["name"], fill="color/bg-secondary", radius=outer, stroke="color/border", hug=True,
                    children=[kit.box("Content", kind="rectangle", fill="color/bg-tertiary", radius=inner, width=48, height=16)])],
            gap="space/fact")
    insets = [inset_tile(s) for s in _sem(plan, "inset")]
    controls = [kit.column(f"Control {s['name']}", [
        kit.text("Name", s["name"]), kit.text("Value", _px(s["value"]), color="color/text-tertiary"),
        kit.box("Control", kind="rectangle", fill="color/bg-tertiary", radius=CONTROL_RADIUS.get(s["name"].split("/")[1], "radius/control"),
                stroke="color/border-strong", width=96, height=kit.V(s["name"]))], gap="space/fact") for s in _sem(plan, "control")]
    return kit.column("Inset and control specimen", [
        _title(kit, "Inset and control"), kit.text("Insets", "Inset family: padding inside a frame", style="Large / Semibold"),
        kit.row("Inset samples", insets, gap="space/section"),
        kit.text("Controls", "Control family: button, input and badge heights, pointer floor", style="Large / Semibold"),
        kit.row("Control samples", controls, gap="space/section")], gap="space/section")


def radius_content(kit, plan):
    def nest(name, outer, inner, innermost, outer_pad, inner_pad):
        leaf = kit.box("Segment", kind="rectangle", fill="color/bg-pressed", radius=innermost, width=96, height=48)
        mid = kit.box("Control", direction="row", pad=inner_pad, fill="color/bg-tertiary", radius=inner, hug=True, children=[leaf])
        return kit.column(name, [kit.text("Name", f"{outer} > {inner} > {innermost}"), kit.box(
            "Frame", direction="row", pad=outer_pad, fill="color/bg-secondary", radius=outer, stroke="color/border", hug=True, children=[mid])],
            gap="space/fact")
    roles = [kit.column(f"Role {s['name']}", [kit.text("Name", s["name"]), kit.text("Value", _px(s["value"]), color="color/text-tertiary"),
             kit.box("Square", kind="rectangle", fill="color/bg-tertiary", radius=s["name"], stroke="color/border-strong", width=64, height=64)],
             gap="space/fact") for s in _sem(plan, "radius") if s["name"].split("/")[1] in ("mark", "segment", "control", "xl", "frame")]
    return kit.column("Radius specimen", [
        _title(kit, "Radius nesting"),
        kit.row("Radius roles", roles, gap="space/section"),
        kit.row("Concentric samples", [
            nest("Frame holds control", "radius/frame", "radius/control", "radius/segment", "inset/frame", "inset/segment"),
            nest("Card holds control", "radius/xl", "radius/control", "radius/segment", 4, "inset/segment")], gap="space/region")],
        gap="space/section")
