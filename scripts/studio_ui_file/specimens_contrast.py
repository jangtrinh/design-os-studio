"""Colour pairs with contrast (S6): each text and icon colour on the surfaces design/principles.md P2 names, with the WCAG 2.x
ratio computed from the theme.rs values and the AA verdict (text 4.5:1, icons and control boundaries 3:1).

The two named exceptions are marked as exceptions, never counted as passes: placeholder text (owner ruling R8.1) and the
`BORDER_STRONG` resting boundary (R10.7, controller ruling C-F). Tertiary and quaternary text is not listed on the pressed
fill, which P2 reserves for `TEXT` and `TEXT_SECONDARY`.

    content = contrast_content(plan, kit)
"""
TEXT_FG = ["text", "text-secondary", "text-tertiary", "text-brand", "text-component", "text-error", "text-warning", "text-success"]
ICON_FG = ["icon", "icon-secondary", "icon-tertiary", "icon-quaternary"]
SURFACES = ["bg", "bg-secondary", "bg-tertiary"]
# (foreground, background, kind, exception note or None)
SPECIAL = [("text-on-brand", "bg-brand", "text", None), ("text-white", "bg-error-solid", "text", None),
           ("text", "bg-primary-solid", "text", None),
           ("text-placeholder", "bg", "text", "Exception R8.1"), ("text-placeholder", "bg-secondary", "text", "Exception R8.1"),
           ("border-strong", "bg", "edge", "Exception R10.7"), ("border-strong", "bg-secondary", "edge", "Exception R10.7")]
MIN = {"text": 4.5, "icon": 3.0, "edge": 3.0}
W = {"sample": 96, "fg": 232, "bg": 232, "ratio": 80, "min": 64}


def luminance(c):
    def lin(v):
        v /= 255
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    return 0.2126 * lin(c["r"]) + 0.7152 * lin(c["g"]) + 0.0722 * lin(c["b"])


def ratio(fg, bg):
    hi, lo = sorted((luminance(fg), luminance(bg)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


def pairs():
    out = [(f, b, "text", None) for f in TEXT_FG for b in SURFACES] + [(f, "bg-pressed", "text", None) for f in ("text", "text-secondary")]
    out += [(f, b, "icon", None) for f in ICON_FG for b in SURFACES]
    return out + SPECIAL


def sample(kit, kind, fg, bg):
    mark = {"text": lambda: kit.text("Sample", "Aa 0123", style="Body / Medium", color=f"color/{fg}"),
            "icon": lambda: kit.box("Sample", kind="rectangle", fill=f"color/{fg}", radius="radius/mark", width=16, height=16),
            "edge": lambda: kit.box("Sample", kind="rectangle", radius="radius/control", stroke=f"color/{fg}", width=32, height=16)}[kind]()
    return kit.box("Surface", direction="row", fill=f"color/{bg}", radius="radius/control", pad="inset/control", width=W["sample"],
                   height_sizing="hug", children=[mark])


def contrast_content(plan, kit):
    by = {s["name"]: s["value"] for s in plan["semantic"] if s["group"] == "color"}

    def row(fg, bg, kind, exception):
        for n in (fg, bg):
            if f"color/{n}" not in by:
                raise SystemExit(f"contrast pair uses an unknown colour variable color/{n}")
        r = ratio(by[f"color/{fg}"], by[f"color/{bg}"])
        passed = r >= MIN[kind]
        verdict, colour = ((exception, "color/text-warning") if exception else ("Pass AA", "color/text-success") if passed else
                           ("Fail AA", "color/text-error"))
        cells = [sample(kit, kind, fg, bg), kit.text("Foreground", f"color/{fg}", width=W["fg"]), kit.text("Background", f"color/{bg}", width=W["bg"]),
                 kit.text("Ratio", f"{r:.2f}:1", width=W["ratio"]), kit.text("Required", f">= {MIN[kind]:g}", color="color/text-tertiary", width=W["min"]),
                 kit.text("Verdict", verdict, style="Body / Semibold", color=colour)]
        return kit.row(f"Pair {fg} on {bg}", cells, gap="space/inline", pad=[None, "inset/control", None, "inset/control"],
                       align="center"), (passed, exception)
    rows, tally = [], {"pass": 0, "fail": 0, "exception": 0}
    for fg, bg, kind, exception in pairs():
        spec, (passed, exc) = row(fg, bg, kind, exception)
        rows.append(spec)
        tally["exception" if exc else "pass" if passed else "fail"] += 1
    head = kit.row("Contrast header", [kit.text(f"Head {t}", t, style="Body / Semibold", color="color/text-tertiary", width=w)
                                       for t, w in (("Sample", W["sample"]), ("Foreground", W["fg"]), ("Background", W["bg"]),
                                                    ("Ratio", W["ratio"]), ("Required", W["min"]), ("AA", 120))],
                   gap="space/inline", pad=[None, "inset/control", None, "inset/control"])
    note = kit.text("Note", "Text needs 4.5:1, icons and control boundaries 3:1. Exceptions are named, not counted as passes.", style="Body / Regular",
                    color="color/text-tertiary")
    content = kit.column("Contrast specimen", [kit.text("Section title", "Contrast pairs", style="Title / Semibold"), note, head] + rows, gap="space/row")
    return content, tally
