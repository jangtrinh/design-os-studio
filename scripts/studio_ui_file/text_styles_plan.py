"""S3 text styles: one per type role of theme.rs (font_caption*, font_body*, font_large*, font_text,
font_title, font_canvas*, font_canvas_label, font_mono, font_text(size, weight)), at the weights theme.rs uses. A style binds the variable
of the constant the Rust names: a kit button label is `Text sm / Semibold` (FONT_SIZE_TEXT_SM), a badge, tag or status label is
`Text xs / Medium` (FONT_SIZE_TEXT_XS), and neither is `Large` or `Caption` even where the pixel size is equal. A `Text sm` or
`Text xs` Regular style is added when a set first uses that look. `Mono / Value` is `font_mono` (FONT_SIZE_BODY); `Mono / Caption` is
the monospace face at FONT_SIZE_CAPTION that the Progress counts use (panels/check_designs.rs:569), which `font_mono` is not.

Sizes, line heights, weights and families are read from the Semantic plan, never retyped. Each
style also lists the Semantic variables its font fields bind to (applied in the native post-pass,
because set_style has no binding field).

    styles, bindings = build(plan)   # styles: set_style upserts; bindings: {style: {prop: var id}}
"""

REGULAR, MEDIUM, SEMIBOLD = "text/weight-regular", "text/weight-medium", "text/weight-strong"
UI, MONO = "text/font-family-ui", "text/font-family-mono"
# (role, weight label, size var, line var or None, weight var, family var, theme.rs helper)
ROLES = [
    ("Caption", "Regular", "text/caption", "text/line-caption", REGULAR, UI, "font_caption"),
    ("Caption", "Medium", "text/caption", "text/line-caption", MEDIUM, UI, "font_caption_medium"),
    ("Caption", "Semibold", "text/caption", "text/line-caption", SEMIBOLD, UI, "font_caption_strong"),
    ("Body", "Regular", "text/body", "text/line-body", REGULAR, UI, "font_body"),
    ("Body", "Medium", "text/body", "text/line-body", MEDIUM, UI, "font_body_medium / font_menu"),
    ("Body", "Semibold", "text/body", "text/line-body", SEMIBOLD, UI, "font_body_strong"),
    ("Large", "Regular", "text/large", "text/line-text-sm", REGULAR, UI, "font_large"),
    ("Large", "Semibold", "text/large", "text/line-text-sm", SEMIBOLD, UI, "font_large_strong"),
    ("Text sm", "Regular", "text/text-sm", "text/line-text-sm", REGULAR, UI, "font_text(FONT_SIZE_TEXT_SM, Regular)"),
    ("Text sm", "Semibold", "text/text-sm", "text/line-text-sm", SEMIBOLD, UI, "font_text(FONT_SIZE_TEXT_SM, Semibold)"),
    ("Text sm", "Medium", "text/text-sm", "text/line-text-sm", MEDIUM, UI, "font_text(FONT_SIZE_TEXT_SM, Medium)"),
    ("Text xs", "Medium", "text/text-xs", "text/line-text-xs", MEDIUM, UI, "font_text(FONT_SIZE_TEXT_XS, Medium)"),
    ("Text xs", "Regular", "text/text-xs", "text/line-text-xs", REGULAR, UI, "font_text(FONT_SIZE_TEXT_XS, Regular)"),
    ("Text xs", "Semibold", "text/text-xs", "text/line-text-xs", SEMIBOLD, UI, "font_text(FONT_SIZE_TEXT_XS, Semibold)"),
    ("Text md", "Semibold", "text/text-md", "text/line-text-md", SEMIBOLD, UI, "font_text(FONT_SIZE_TEXT_MD, Semibold)"),
    ("Text xl", "Semibold", "text/text-xl", "text/line-text-xl", SEMIBOLD, UI, "font_text(FONT_SIZE_TEXT_XL, Semibold)"),
    ("Title", "Semibold", "text/title", "text/line-text-xl", SEMIBOLD, UI, "font_title"),
    ("Canvas", "Regular", "text/canvas", None, REGULAR, UI, "font_canvas"),
    ("Canvas", "Medium", "text/canvas", None, MEDIUM, UI, "font_canvas_medium"),
    ("Canvas", "Semibold", "text/canvas", None, SEMIBOLD, UI, "font_canvas_strong"),
    ("Canvas label", "Medium", "text/label-canvas", None, "text/weight-canvas-label", UI, "font_canvas_label"),
    ("Ruler", "Regular", "text/ruler", None, REGULAR, UI, "font_ruler"),
    ("Mono", "Value", "text/body", "text/line-body", "text/weight-mono", MONO, "font_mono"),
    ("Mono", "Caption", "text/caption", "text/line-caption", "text/weight-mono", MONO, "FontId::new(FONT_SIZE_CAPTION, Monospace)"),
]
INK = {"r": 250 / 255, "g": 250 / 255, "b": 250 / 255, "a": 1.0}  # theme::TEXT; a style's own fill cannot bind a variable


def build(plan):
    by_name = {s["name"]: s for s in plan["semantic"]}
    styles, bindings = [], {}
    for role, label, size, line, weight, family, helper in ROLES:
        name = f"{role} / {label}"
        ts = {"font_family": by_name[family]["value"], "font_weight": int(by_name[weight]["value"]), "italic": False,
              "font_size": float(by_name[size]["value"]), "line_height": {"Pixels": float(by_name[line]["value"])} if line else "Auto",
              "letter_spacing": 0.0, "paragraph_spacing": 0.0, "align_h": "Left", "align_v": "Top", "case": "Original",
              "decoration": "None", "fills": [{"kind": {"Solid": INK}, "opacity": 1.0, "visible": True, "blend": "Normal"}]}
        styles.append({"key": name, "name": name, "property": "text", "description": f"theme.rs {helper}", "text_style": ts})
        b = {"fontSize": by_name[size]["id"], "fontWeight": by_name[weight]["id"], "fontFamily": by_name[family]["id"]}
        if line:
            b["lineHeight"] = by_name[line]["id"]
        bindings[name] = b
    return styles, bindings
