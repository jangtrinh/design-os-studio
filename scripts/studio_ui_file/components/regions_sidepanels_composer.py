"""Registry `composite` Input/Composer (page `03 Studio`, row Agents), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (agent_composer.rs).

Not built before this round: the W2 census deferred the agents-panel composites because spec I (#1438, v0.7.6) replaces the composer's controls; Panel/AgentTerminal (the 0.7.5
capture `i-agents`) needs it, so it is drawn from the v0.7.5 painter and spec I supersedes it. The two conflict: spec I's composer holds the agent picker, this one is the 0.7.5
box.

Painter: `show_composer` (agent_composer.rs:870-1180), the box at the panel's bottom: 376 x 80 in the capture (`agents-composer-box` (61, 808, 376, 80)), every rect relative to its
top-left. State drawn: empty draft, not focused, no chips, no skills chosen. The container is `RADIUS_FRAME`, `BG_SECONDARY` with a `BORDER` stroke (:954-958), the hint `Ask for
changes` in Body `TEXT_TERTIARY` at `INSET_BUTTON` from the left, `INSET_CONTROL` from the top (:960-1017); the control row is `CONTROL_MD` high at the bottom (:997) and centred on
its middle (:1048): the `+` attach button and the open-book button (24 px = `CONTROL_DENSE`, glyphs 18 px = `LINE_CAPTION` in `ICON_SECONDARY`, `INSET_CONTROL` from the left and
`SPACE_FACT` apart, :1050-1077), the `PRD` chip (`PRD_CHIP_WIDTH` = `CONTROL_MD` 40 wide, `BG_TERTIARY` with a `BORDER` stroke at `RADIUS_CONTROL`, `PRD` in Caption Semibold
`TEXT_SECONDARY`, :1083-1102), the `Skills` chip (`SkillsState::paint_chip`, same height, its width the label + 2 x `INSET_CONTROL`, `SPACE_ROW` after PRD, :1121-1124) and the Send
circle (28 px = `CONTROL_STANDARD`, `BG_TERTIARY` while nothing can be sent, an 18 px `ArrowUp` in `TEXT_SECONDARY`, `INSET_CONTROL` from the right, :1126-1160). The Skills chip's
colours are the inline literals of `regions_sidepanels_agents_parts.py` (nearest tones).
Gaps: the focus ring (`FOCUS_RING_WIDTH` 1.5, `BG_BRAND`, :956), the reference chips row (:962-995), the typed draft, a hovered button's `BG_HOVER` tile, the sendable and
over-limit Send colours, the shell tab's hint.
"""
from components.regions_sidepanels_agents_parts import box, glyph_box, skills_chip, text
from rest_ir import F, T, lit

ROW = "Agents"
SEND = "agent_composer.rs:1158 `circle_filled(.., 14.0, ..)`: the 28 px Send disc is a circle of radius 14, half its side: no radius role holds it"
W, H = 376.0, 80.0


def composer(values, env):
    prd_label = F("Label", [T("Text", "PRD", "Caption / Semibold", env.c("TEXT_SECONDARY"))], dir="H", w=40.0, h=24.0, sx="fixed", sy="fixed", abs=(0.0, 0.0, 40.0, 24.0))
    send = F("Send", fill=env.c("BG_TERTIARY"), radius=lit(14.0, SEND), w=28.0, h=28.0, sx="fixed", sy="fixed", abs=(340.0, 46.0, 28.0, 28.0))  # `circle_filled` (:1158): a frame as round as a circle
    kids = [text("Hint", (12.0, 8.0, 352.0, 18.0), "Ask for changes", "Body / Regular", "TEXT_TERTIARY", env), glyph_box(env, "Attach", (8.0, 48.0, 24.0, 24.0), "Plus", 18, "ICON_SECONDARY"),
            glyph_box(env, "Book", (36.0, 48.0, 24.0, 24.0), "BookOpen", 18, "ICON_SECONDARY"), box("PRD", (68.0, 48.0, 40.0, 24.0), env, [prd_label], fill="BG_TERTIARY", stroke="BORDER"),
            skills_chip(env, "Skills", (112.0, 48.0, 46.75, 24.0)), send,
            glyph_box(env, "Send arrow", (340.0, 46.0, 28.0, 28.0), "ArrowUp", 18, "TEXT_SECONDARY")]
    return F("Composer", kids, fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def component_sets():
    return [{"name": "Input/Composer", "row": ROW, "axes": {}, "build": composer,
             "description": "Drawn from the v0.7.5 composer box (agent_composer.rs show_composer). "
             "Spec I (v0.7.6) replaces its controls and holds the agent picker: redraw from the new painter once it ships."}]
