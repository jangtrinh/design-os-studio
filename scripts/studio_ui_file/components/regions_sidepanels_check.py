"""Registry `region` Panel/CheckDesigns (page `03 Studio`), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (panels/check_designs.rs).

Painter: `show` (panels/check_designs.rs:135-322), one caller: a floating popover over the canvas (`Area`, 320 wide, 12 below the canvas top and 12 in from its right). The capture is
`j-check-designs`
(`check-designs-fixed` is the same picture), the region is the popover (867, 44, 320, 364) and every rect is relative to its top-left. State drawn: a checked selection with three findings
(one per rule),
none of them fixable, so `Fix all` is disabled (`can_fix_all` needs a fixable finding, :180) and a row has no Fix button (:446); the Rules popover is closed.
* Nested sets (live): Button/Close (`icon_button` Close: the header's close :165-170 and each row's dismiss :465-467, 32 px = `CONTROL_XS`), Button/Icon Xs (the header's Rules button, a
  swapped Sliders, :170-174,
  and each row's Ignore, a swapped EyeOff, :472-474), Button/Primary Xs State=Disabled (`Fix all`, `kit::button` Primary Xs, :199-212, `label_width` = the label + 2 x `INSET_BUTTON`: hugging) and
  Badge/StatusCount Sm Neutral (`kit::count_badge`, :398-411: width `max(text + 2 x SPACE_INLINE, BADGE_SM)`, so 22 for one digit, which a hug main cannot express: the instance is resized with
  `w=22`).
* Local: the surface (`popover_surface`, widgets/kit/surface.rs: `BG`, a `BORDER_MENU` 1 px stroke, `RADIUS_FRAME`), the title (`font_large_strong`, TEXT, `PAD` 12 in), the summary text
  (`font_body_medium` `TEXT`
  14 below the header and `font_body` `TEXT_SECONDARY` 30 below it, :195-198), the group titles (`font_body_strong` `TEXT_SECONDARY`, :398-403) and the finding rows. The rules are
  `BORDER_SUBTLE` lines
  (:175, :218, :442), not Divider/Hairline (its main is `BORDER`). A finding row (`row`, :413-478) is 60 high: the rule's 14 px glyph in `WARNING` centred at (`PAD` + `SPACE_INLINE`, 16),
  the layer name in
  `font_body_medium` `TEXT` at x `PAD` + 24 (`ICON_COL`), a two-line message in `font_body` `TEXT_SECONDARY` from 26 down, a `BORDER_SUBTLE` rule on its last pixel inset `PAD`, and at the
  right the Ignore and
  Dismiss buttons 8 from the top, 2 apart. The list is the SLOT `Findings`: a clipped column from y 88 holding the capture's three groups and rows (3 x (32 + 60) = 276, the list's whole height).
* Kit content: the capture's layer names and the variable names inside each message are the file's own; they are neutral here (`Frame 1`, `Text block`, `"space/16"`, ...). The summary line,
  group titles and
  message templates are the Rust's.
* Gaps: the Fix button of a fixable row (:446-463), the Library badge (:434-439), the scanning row (`progress_row`), the empty state, the ignored footer (:480-), the Rules popover
  (:324-396), an enabled
  `Fix all`, the row hover fill (`BG_HOVER`), the list's scrollbar and the `Contrast` rule's `DANGER` glyph.
"""
from components.regions_sidepanels_parts import close_button, icon_button, label
from rest_ir import F, I, T, X, lit

ROW = "Regions"
W, H = 320.0, 364.0
PAD = "panels/check_designs.rs:14 `const PAD: f32 = 12.0`: a local constant (INSET_BUTTON, SPACE_GROUP and COMMENT_PAD also hold 12)"
# (rule title, its measured Body Semibold width, layer name, message line 1, message line 2); the capture's three findings, neutral
GROUPS = [("Padding", 47.47, "Frame 1", "Padding 16 matches 3 variables:", '"space/16", ...'),
          ("Horizontal spacing", 108.91, "Frame 1", "Gap 12 matches 2 variables:", '"space/12", "space/lg"'),
          ("Vertical spacing", 93.73, "Text block", "Gap 2 matches 2 variables:", '"space/2", "space/xxs"')]


def line(env, name, rect):
    """`widgets::hline(.., BORDER_SUBTLE)`: a 1 px line at `rect`."""
    return F(name, fill=env.c("BORDER_SUBTLE"), w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect)


def group_row(env, n, title, width):
    count = X("Count", "Badge/StatusCount", "Size=Sm, Tone=Neutral, State=Default", props={"Label": "1"}, abs=(round(12.0 + width + 8.0, 2), 5.0, 22.0, 22.0), w=22.0)
    heading = T("Group title", title, "Body / Semibold", env.c("TEXT_SECONDARY"), abs=(12.0, 7.0, width + 4.0, 18.0))
    return F(f"Group {n}", [heading, count], w=W, h=env.num("CONTROL_XS"), sx="fixed", sy="fixed")


def finding_row(env, n, name, first, second):
    kids = [I("Rule icon", "Variables", 14, env.c("WARNING"), abs=(13.0, 9.0, 14.0, 14.0)), T("Name", name, "Body / Medium", env.c("TEXT"), abs=(36.0, 7.0, 200.0, 18.0)),
            T("Message line 1", first, "Body / Regular", env.c("TEXT_SECONDARY"), abs=(36.0, 25.0, 200.0, 18.0)),
            T("Message line 2", second, "Body / Regular", env.c("TEXT_SECONDARY"), abs=(36.0, 40.0, 200.0, 18.0)),
            line(env, "Rule", (12.0, 59.0, 296.0, 1.0)), icon_button("Ignore", "EyeOff", (242.0, 8.0, 32.0, 32.0)), close_button((276.0, 8.0, 32.0, 32.0), "Dismiss")]
    return F(f"Finding {n}", kids, w=W, h=60.0, sx="fixed", sy="fixed")


def rows(env):
    return [row for n, (title, width, name, first, second) in enumerate(GROUPS, 1) for row in (group_row(env, n, title, width), finding_row(env, n, name, first, second))]


def check_panel(values, env):
    findings = F("Findings", rows(env), dir="V", slot="Findings", w=W, h=276.0, sx="fixed", sy="fixed", clip=True, align=("start", "start"), abs=(0.0, 88.0, W, 276.0))
    kids = [label("Title", (0.0, 0.0, W, 44.0), "Check designs", "Large / Semibold", "TEXT", env, pad=[None, None, None, lit(12, PAD)]), icon_button("Rules", "Sliders", (240.0, 6.0, 32.0, 32.0)),
            close_button((276.0, 6.0, 32.0, 32.0)), line(env, "Line 44", (0.0, 44.0, W, 1.0)),
            label("Scope", (12.0, 44.0, 220.0, 28.0), "Selection · 1 layer", "Body / Medium", "TEXT", env),
            label("Counts", (12.0, 60.0, 220.0, 28.0), "3 issues · 4 layers checked", "Body / Regular", "TEXT_SECONDARY", env),
            X("Fix all", "Button/Primary", "Size=Xs, State=Disabled", props={"Label": "Fix all"}, abs=(245.25, 50.0, 62.75, 32.0)), line(env, "Line 88", (0.0, 88.0, W, 1.0)), findings]
    return F("Check designs", kids, fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def component_sets():
    return [{"name": "Panel/CheckDesigns", "row": ROW, "axes": {}, "uses": ["Badge/StatusCount", "Button/Close", "Button/Icon", "Button/Primary"],
             "props": [{"name": "Findings", "type": "SLOT"}], "build": check_panel}]
