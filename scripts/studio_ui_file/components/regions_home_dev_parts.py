"""Parts of the Dev inspect body (`regions_home_dev.py`), each placed at the capture's own rect. Lines are the v0.7.5 tag. Positions are the region's own: the cursor of `inspect::body`
(`c.y`, :338-640) starts at the region's top, `PAD` 16 in from its left, and every `c.rect(PAD, dy, w, h)` below is its box.

Text is placed by its line centre (`p.text(.., LEFT_CENTER, ..)` / `rect.center()`): `text()` takes the centre and gives the node the style's line box (Body and Caption 18, Large 20).
"""
from rest_ir import F, E, I, T, lit
from components.regions_home_parts import icon_button, rule

PILL = "theme::cr_pill(h) = radius_full(h) = h / 2: computed from the pill's height, no radius role holds it"
TINT = "dev/sections.rs:36 `status_color(s).gamma_multiply(0.22)`: the DEV_READY constant at a raw alpha of 0.22, a paint opacity no variable holds"
CODE_PILL = "inspect.rs:506 `theme::cr(4)`: a raw 4 in the Rust, no constant named"
LINE = {"Large": 20}
W = 241  # `c.w` = region.width + BORDER_WIDTH


def text(env, name, content, style, color, x, cy, w=80):
    h = LINE.get(style.split(" / ")[0], 18)
    return T(name, content, style, env.c(color), abs=(x, cy - h / 2, w, h))


def button(env, name, label, x, y, w, h, fill="BG_SECONDARY", radius=None, style="Body / Regular", color="TEXT"):
    """`small_button` (inspect.rs:217-223: `BG_SECONDARY`, `RADIUS_CONTROL`, a centred `font_body` label) or a pill-shaped `primary_button`."""
    return F(name, [T("Label", label, style, env.c(color))], dir="H", fill=env.c(fill), radius=radius or env.c("RADIUS_CONTROL"), abs=(x, y, w, h), w=w, h=h, sx="fixed", sy="fixed",
             align=("center", "center"))


def glyph(env, name, icon, size, cx, cy, color="ICON_SECONDARY"):
    return I(name, icon, size, env.c(color), abs=(cx - size / 2, cy - size / 2, size, size))


def dot(env, name, cx, cy, color):
    return E(name, 7, 7, fill=env.c(color), abs=(cx - 3.5, cy - 3.5, 7, 7))


def header(env):
    """:341-401: the node name (`font_large_strong`) and three 20 px action boxes at the row's right, then the kind row."""
    boxes = [F("Link action", [I("Glyph", "Connector", 13, env.c("ICON_SECONDARY"))], dir="H", abs=(167, 14, 20, 20), w=20, h=20, sx="fixed", sy="fixed"),
             F("Code action", [I("Glyph", "Dev", 13, env.c("ICON_SECONDARY"))], dir="H", abs=(189, 14, 20, 20), w=20, h=20, sx="fixed", sy="fixed"),
             F("More action", [T("Dots", "•••", "Caption / Semibold", env.c("ICON_SECONDARY"))], dir="H", abs=(209, 14, 20, 20), w=20, h=20, sx="fixed", sy="fixed")]
    return [text(env, "Name", "Frame 1", "Large / Semibold", "TEXT", 16, 24, 70), *boxes, glyph(env, "Kind icon", "Frame", 12, 22, 45),
            text(env, "Kind", "Frame", "Body / Regular", "TEXT_SECONDARY", 34, 45, 34)]


def status(env):
    """:51-114 and `status_pill` (sections.rs:31-40): a 20 px pill (dot at left + 10, label at left + 19), the date 8 after it, then the two action buttons 8 apart (`split_actions`, :124-135)."""
    label = text(env, "Label", "Ready for dev", "Caption / Medium", "TEXT", 19, 10, 80)
    dot = E("Dot", 7, 7, fill=env.c("DEV_READY"), abs=(6.5, 6.5, 7, 7))
    pill = F("Status pill", [dot, label], dir="H", fill=env.c("DEV_READY"), alpha=lit(0.22, TINT), radius=lit(10, PILL), abs=(16, 62, 106, 20), w=106, h=20,
             sx="fixed", sy="fixed")
    return [pill, text(env, "Date", "2026-10-29", "Caption / Regular", "TEXT_TERTIARY", 130, 73, 62),
            button(env, "Mark completed", "Mark completed", 16, 90, 101, 28, fill="BG_BRAND", radius=lit(14, PILL), style="Body / Semibold"),
            button(env, "Remove status", "Remove status", 125, 90, 100, 28)]


def titled(env, title, y, action=None, action_y=None):
    """`section_title` (:225-231): the hairline at y, the title centred in the `SECTION_TITLE_HEIGHT` row below it; an `Add` small button 24 high at its right."""
    out = [rule("Rule " + title, 0, y, W), text(env, title, title, "Body / Semibold", "TEXT", 16, y + 21, 90)]
    if action:
        out.append(button(env, action, "Add", W - 16 - 56, action_y, 56, 24))
    return out


def annotations(env):
    """`annotations_section` (sections.rs:242-286): a dot at left + 4 and the text at left + 14 in a `ROW_H` 24 row, then the property rows (20 high, label at +14, value 100 on) in caption."""
    rows = [("Width", "1440px", 195), ("Fill", "#FFFFFF", 215), ("Padding", "0 0 0 0", 235)]
    kids = [dot(env, "Annotation dot", 20, 171, "ANNOTATION_LINE"), text(env, "Annotation", "Keep 20px padding on every si…", "Body / Regular", "TEXT", 30, 171, 190)]
    for label, value, cy in rows:
        kids += [text(env, f"{label} property", label, "Caption / Regular", "TEXT_SECONDARY", 30, cy, 40), text(env, f"{label} value", value, "Caption / Regular", "TEXT", 130, cy, 50)]
    return titled(env, "Annotations", 126, "Add annotation", 135) + kids


def links(env):
    """`links_section` (sections.rs:325-368): a 36 px row, the label at top + 11 in `TEXT_BRAND`, the url at top + 26 in caption, a 24 px close icon button at the row's right."""
    row = F("Dev link", [text(env, "Link label", "Storybook", "Body / Regular", "TEXT_BRAND", 0, 11, 60),
                         text(env, "Link url", "https://storybook.example.co\u2026", "Caption / Regular", "TEXT_TERTIARY", 0, 26, 170)],
            abs=(16, 284, 181, 36), w=181, h=36, sx="fixed", sy="fixed")
    return titled(env, "Dev links", 251, "Add link", 260) + [row, icon_button("Remove link", "Close", 201, 290)]


def mcp(env):
    """:408-487: the `v MCP` header row (chevron at left + 5, label at left + 16, three 13 px glyphs at right - 44 / - 24 / - 4), `Selection details`, two key-value rows (`kv`, :248-262: label at the
    left, value 100 on, `font_mono` for a number), the prompt box (code_view.rs:174-189) and the copy button. Raw colours map to the nearest theme tone (W2c ruling): `Color32::WHITE` is ICON, `0xB3`
    TEXT_QUATERNARY (d 16)."""
    title = F("MCP header", [glyph(env, "MCP chevron", "ChevronDown", 10, 5, 20), text(env, "MCP label", "MCP", "Body / Semibold", "ICON", 16, 20, 28), glyph(env, "MCP help", "Help", 13, 165, 20),
                             glyph(env, "MCP settings", "StrokeSettings", 13, 185, 20), glyph(env, "MCP add", "Plus", 13, 205, 20)], abs=(16, 329, 209, 40), w=209, h=40, sx="fixed", sy="fixed")
    head = [rule("Rule MCP", 0, 328, W), title, text(env, "Selection details", "Selection details", "Body / Semibold", "TEXT_QUATERNARY", 16, 374, 100)]
    rows = [text(env, "Session label", "Session activity", "Body / Regular", "TEXT_SECONDARY", 16, 397, 90), text(env, "Session value", "Not sent", "Body / Regular", "TEXT", 116, 397, 50),
            text(env, "Tokens label", "Estimated tokens", "Body / Regular", "TEXT_SECONDARY", 16, 421, 90), text(env, "Tokens value", "13.4k", "Mono / Value", "TEXT", 116, 421, 34)]
    prompt = F("MCP prompt", [text(env, "Prompt lead", "Implement this design\nfrom design:os.", "Mono / Value", "TEXT", 8, 17, 160),
                              text(env, "Prompt link", "@designos://design/44757/\nFrame-1", "Mono / Value", "INFO", 8, 53, 180)],
               fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER_STRONG"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), abs=(16, 437, 209, 88), w=209, h=88, sx="fixed", sy="fixed")
    return head + rows + [prompt, button(env, "Copy MCP link", "Copy design:os MCP link", 16, 533, 209, 28)]


def code_connect(env):
    """:489-547: the header row (label at left, a 64 x 18 `Not started` pill at its right: raw `0x38` fill and `0xB3` text, nearest tones BG_PRESSED (d 8) and TEXT_QUATERNARY (d 16), radius a raw 4),
    the description in egui's own 15 px rows (a `layout` of `font_body` at the row width) and the 26 px `Send to agent` button."""
    label = T("Label", "Not started", "Caption / Regular", env.c("TEXT_QUATERNARY"))
    pill = F("Connect status", [label], dir="H", fill=env.c("BG_PRESSED"), radius=lit(4, CODE_PILL), abs=(145, 11, 64, 18), w=64, h=18,
             sx="fixed", sy="fixed", align=("center", "center"))
    lines = ["Publish components to a library,", "then you can connect them to your", "codebase with Code Connect.", "Learn more"]
    kids = [text(env, f"Connect text {i + 1}", s, "Body / Regular", "TEXT_QUATERNARY", 16, 609.5 + 15 * i, 190) for i, s in enumerate(lines)]
    title = F("Code Connect header", [text(env, "Code Connect label", "Code Connect", "Body / Semibold", "ICON", 0, 20, 90), pill], abs=(16, 568, 209, 40), w=209, h=40, sx="fixed", sy="fixed")
    return [rule("Rule Code Connect", 0, 567, W), title, *kids, button(env, "Send to agent", "Send to agent", 16, 670, 209, 26)]


def layer_properties(env):
    """:549-575: the `v Layer properties` header row (chevron at left + 5, label at left + 16) with an 88 x 20 `+ Add redlines` small button at its right, then `kv` rows (Width first, 24 high)."""
    return [rule("Rule Layer properties", 0, 702, W), glyph(env, "Layer chevron", "ChevronDown", 10, 21, 723), text(env, "Layer label", "Layer properties", "Body / Semibold", "ICON", 32, 723, 100),
            button(env, "Add redlines", "+ Add redlines", 137, 713, 88, 20), text(env, "Width label", "Width", "Body / Regular", "TEXT_SECONDARY", 16, 747, 40),
            text(env, "Width value", "1440px", "Mono / Value", "TEXT", 116, 747, 44)]
