"""The Attach skills popover of the Agents panel (Popover/Panel `Kind=Skills`, 320 x 192): `skills/mod.rs:816` `show_picker_popover`. Not a component module. Lines are the v0.7.5 tag of
crates/designos-app/src.

Geometry: the header is `CONTROL_STANDARD` 28 high (title in Caption Semibold at x 12, then the count pills `SPACE_INLINE` 8 apart, the 12 px close glyph at (right - 16, top + 16)) with
a line across it 8 in from each side; the rows start `SPACE_ROW` 4 below it, `CONTROL_MD` 40 high at a 44 pitch, inset `INSET_FRAME` 8; a 12 px checkbox, the title at x 24, the source
and token count right-aligned, the description under the title; a footer line 24 above the bottom with the user-skills path and a `READY` pill (`pills::paint_pill`, skills/pills.rs:13:
`INSET_PANEL` 16 high, fully round).
Colours the painter takes from `terminal.rs` (`TERM_*`, `STATUS_*`, lines 33-57) or as inline literals have no variable: each is drawn as the nearest theme tone in the same role (the
largest per-channel difference in brackets), as the Agents panel does (regions_sidepanels_agents_parts.py): popover and row fill 1F2024 BG_SELECTED (7), popover edge 3E3E46
BORDER_STRONG (6), line 2A2B31 BORDER (11), a selected row 1B2640 TILE_DESIGN_BG (20), its edge 2F4577 BORDER_STRONG (55), its text 8FB0FF BOX_CONTENT (23), the OK text 4CC58B
TEXT_SUCCESS (25), the OK fill 17302A BG_SUCCESS_PRIMARY (20), its edge 2C5F4A BADGE_SUCCESS_BORDER (22), the checkbox fill 16171B BG_HOVER (4).
Seven of the ten are also the nearest tone by value over every opaque colour constant of theme.rs; three are not: 1F2024 (BOX_TEXT and ANNOTATION_TEXT are 6 away, but they are text
tones, not a surface), 2F4577 (TILE_DESIGN_BG is 35 away, but it is the row's own fill, so an edge in it would vanish) and 17302A (the BG_SELECTED family is 15 away, but that is a
neutral grey, not the success fill). The skill names and descriptions are the user's own content: neutral names stand in, the counts are the capture's.
"""
from components.regions_sidepanels_agents_parts import box, centred, line, text
from rest_ir import F, I, lit

PILL_R = "skills/pills.rs:15 `cr_pill(rect.height())` = 16 / 2: computed from the pill's height, no radius role holds it"
ROWS = (("first-skill @1.0", "BUILT-IN · 226 tokens", "Describes what the first skill adds to an agent run.", True),
        ("second-skill @1.0", "BUILT-IN · 221 tokens", "Describes what the second skill adds to an agent run.", True),
        ("third-skill @1.0", "BUILT-IN · 227 tokens", "Describes what the third skill adds to an agent run.", False))


def pill(env, name, content, x, w, fill="BG_TERTIARY", ink="TEXT"):
    label = centred(env, content, "Caption / Semibold", ink, w, 16.0)
    return box(name, (x, 8.0, w, 16.0), env, [label], fill=fill, radius=lit(8, PILL_R))


def skill_row(env, i, title, meta, desc, on):
    y, w = 32.0 + 44.0 * i, 304.0
    fill, edge = ("TILE_DESIGN_BG", "BORDER_STRONG") if on else ("BG_SELECTED", "BORDER")
    mark = [I("Check", "Check", 8, env.c("BG"), abs=(2.0, 2.0, 8, 8))] if on else []
    check = box("Checkbox", (6.0, 6.0, 12.0, 12.0), env, mark, fill="BOX_CONTENT" if on else "BG_HOVER", stroke="BOX_CONTENT" if on else "BORDER", radius="RADIUS_SEGMENT")
    kids = [check, text("Title", (24.0, 3.0, 150.0, 18), title, "Caption / Semibold", "TEXT_WHITE", env),
            text("Meta", (w - 8 - 118.0, 3.0, 118.0, 18), meta, "Caption / Regular", "BOX_CONTENT" if on else "TEXT_SECONDARY", env, align_h="right"),
            text("Description", (24.0, 19.0, 270.0, 18), desc, "Caption / Regular", "TEXT_SECONDARY", env)]
    return box(f"Skill {i + 1}", (8.0, y, w, 40.0), env, kids, fill=fill, stroke=edge)


def skills(env):
    w, h = 320.0, 192.0
    close = F("Close", [I("Glyph", "Close", 12, env.c("TEXT_SECONDARY"))], dir="H", w=24.0, h=24.0, sx="fixed", sy="fixed", abs=(w - 28.0, 4.0, 24.0, 24.0))
    header = [text("Title", (12.0, 7.0, 80.0, 18), "Attach skills", "Caption / Semibold", "TEXT_WHITE", env), pill(env, "Count", "2/3", 92.0, 28.0),
              pill(env, "Tokens", "447/8000 tokens", 128.0, 92.0), close, line(env, "Header rule", (8.0, 28.0, w - 16, 1.0))]
    ready = box("Ready", (264.0, 168.0, 48.0, 16.0), env, [centred(env, "READY", "Caption / Semibold", "TEXT_SUCCESS", 48.0, 16.0)], fill="BG_SUCCESS_PRIMARY",
                stroke="BADGE_SUCCESS_BORDER", radius="RADIUS_SEGMENT")
    path = text("Path", (12.0, 174.0, 220.0, 18), "User skills: /library/skills/<name>/SKILL.md", "Caption / Regular", "TEXT_SUCCESS", env)
    footer = [line(env, "Footer rule", (8.0, 168.0, w - 16, 1.0)), path, ready]
    node = box("Popover", (0.0, 0.0, w, h), env, [*header, *(skill_row(env, i, *row) for i, row in enumerate(ROWS)), *footer], fill="BG_SELECTED", stroke="BORDER_STRONG", radius="RADIUS_FRAME")
    node.pop("abs")
    node["clip"] = True
    return node


SKILLS = {"Skills": skills}
