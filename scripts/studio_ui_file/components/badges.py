"""Registry `component` Badge/StatusCount (row Badges & Status), one set drawn from two live painters by size (rule f), both coloured by
`kit::count_badge_look` (widgets/kit/badge.rs:31-38): the tone's tinted triple, a 1 px inside stroke, the label `Text xs / Medium` centred.

* Sm = `kit::count_badge` (badge.rs:75-80), a `radius_full(height)` pill, `BADGE_SM` (22). Callers (re-grepped; the registry's `today=home/mod.rs:1111-1113`
  is stale): Home version badge (home/mod.rs:928), Home trash count (home/mod.rs:1163), Check designs rule count (panels/check_designs.rs:410), command palette
  row badge (cmdk.rs:502). All four pass `Tone::Neutral`, so Sm Neutral is the only live Sm look; Sm Success, Warning and Error are rule (b), the painter's own
  values, UNVERIFIED-VISUAL, a gap.
  Padding is the caller's: Home pads the label by `2 * SPACE_FACT` with a `BADGE_SM` minimum (2 of 4 callers, drawn here); Check designs pads `2 * SPACE_INLINE`
  (check_designs.rs:407) and the palette `SPACE_GROUP` with no minimum (cmdk.rs:497). A hug frame holds one padding and an instance cannot override it, so those
  two paddings are gaps; so is the minimum width (a one-digit count is `BADGE_SM` wide in Home, narrower here), which rest_ir cannot express.
* Md = `kit::status_tag` (badge.rs:66-72), the Libraries dialog's New / Changed / Removed chips (`paint_change_chip`, libraries/modal.rs:134-139; callers
  libraries/modal.rs:220 and libraries/review.rs:161): `BADGE_MD` (24) high, `RADIUS_CONTROL` square corners (not a pill), text + `2 * SPACE_INLINE` wide, tone
  Success, Warning or Error (`change_tone`, modal.rs:117-123). Md Neutral has no caller (rule b, the same painter's value). The registry's `rust=` names
  `count_badge`, so Sm is the primary record and `status_tag` is the second live painter, drawn here because it is the only live Md.
"""
from rest_ir import F, T, lit

TONES = {"Neutral": ("BADGE_NEUTRAL_BG", "BADGE_NEUTRAL_BORDER", "BADGE_NEUTRAL_TEXT"),
         "Success": ("BG_SUCCESS_PRIMARY", "BADGE_SUCCESS_BORDER", "BADGE_SUCCESS_TEXT"),
         "Warning": ("BG_WARNING_PRIMARY", "BADGE_WARNING_BORDER", "BADGE_WARNING_TEXT"),
         "Error": ("BG_ERROR_PRIMARY", "BADGE_ERROR_BORDER", "BADGE_ERROR_TEXT")}
HEIGHT = {"Sm": "BADGE_SM", "Md": "BADGE_MD"}
PILL = "kit/badge.rs:77 `radius_full(rect.height())` = height / 2, computed from the badge's height: no radius role holds it"


def badge(values, env):
    fill, border, text = (env.c(c) for c in TONES[values["Tone"]])
    md = values["Size"] == "Md"
    h = env.num(HEIGHT[values["Size"]])
    radius = env.c("RADIUS_CONTROL") if md else lit(round(h / 2), PILL)
    pad = env.c("SPACE_INLINE" if md else "SPACE_FACT")
    return F("Badge", [T("Label", "v0.7.5", "Text xs / Medium", text, prop="Label")], dir="H", fill=fill, stroke=border, sw=env.c("BORDER_WIDTH"),
             radius=radius, pad=[None, pad, None, pad], h=h, sx="hug", sy="fixed")


def component_sets():
    return [{"name": "Badge/StatusCount", "row": "Badges & Status",
             "axes": {"Size": ["Sm", "Md"], "Tone": list(TONES), "State": ["Default"]},
             "props": [{"name": "Label", "type": "TEXT", "default": "v0.7.5"}], "build": badge}]
