"""Registry `composite` Banner/Inline (page `03 Studio`, row Bars & Feedback): a set over the registry's `Tone=Neutral, Success, Warning, Error`. Lines are named with their tag.

The set is drawn once, from two painters (the v0.7.6 tag exists, so the failure painter is its own):
* Neutral: the history bar, `history/banner.rs:9-40` at v0.7.5 (unchanged in v0.7.6). `HISTORY_BANNER_H` 40 high :12, `BG_SECONDARY` with a 1 px `BORDER` inside stroke and no
  radius (:16-17); the title `Viewing <name>` in `Text sm` Semibold TEXT at `pad` 12 (:20, :27-29); on the right, 12 from the edge, `Restore this version` (kit Primary Xs) and
  `Back to current` (kit Secondary Xs), `CONTROL_XS` high :21, 8 apart (:23-26, :33-34). 909 wide in `j-version-history` (the canvas width: `bar` is `canvas.width()` x
  `HISTORY_BANNER_H`, :12).
* Warning and Error: `paint_banner` at v0.7.6 (`agent_session_failure.rs:871-984`, the mid-session failure banner): `inset * 2 + CONTROL_XS` = 48 high (:886), `BG_WARNING_PRIMARY`
  or `BG_ERROR_PRIMARY` with a 1 px `BADGE_WARNING_BORDER` or `BADGE_ERROR_BORDER` inside stroke, square (:888-896); a dot 8 px (`SPACE_FACT` radius, :899-904: `Status/Dot` Error,
  a `WARNING` disc for Warning) centred `inset` + 4 from the left; the sentence in `Body` TEXT starting at `inset + 2 x SPACE_ROW + SPACE_INLINE` = 28 (:951); at the right, `inset`
  from the edge, `Copy details` (kit Tertiary Xs :931-945) and the fix action (kit Secondary Xs, :909-929), `SPACE_INLINE` apart. Width 401 (the agents panel's viewport).
* Success: no painter draws it (rule (c)): it takes the Neutral look, recorded as a gap.
Content: the title and the sentence are Rust strings built from the session (`failure.banner_sentence()` :974-979); the defaults are an example of each shape. `Status/Dot` has no
Warning tone, so the Warning dot is a local 8 px disc in `WARNING`.
Gaps: the mid-session banner's unknown-class tail (up to three mono lines, :954-962), the rate-limit time in `Mono` (:963-972), the disabled action, a banner with no action button.
"""
from rest_ir import E, F, T, X, lit

NEUTRAL_W, FAILURE_W = 909.0, 401.0
PAD = "history/banner.rs:20 `let pad = 12.0`: a raw literal (SPACE_GROUP holds 12 for other roles)"
TONES = {"Neutral": "Neutral", "Success": "Neutral", "Warning": "Warning", "Error": "Error"}  # Success has no painter: rule (c), the Neutral look
FILL = {"Warning": ("BG_WARNING_PRIMARY", "BADGE_WARNING_BORDER"), "Error": ("BG_ERROR_PRIMARY", "BADGE_ERROR_BORDER")}


def history_bar(env):
    pad = lit(12, PAD)
    title = T("Title", "Viewing Version 1", "Text sm / Semibold", env.c("TEXT"), prop="Label", sx="fill")
    restore = X("Primary action", "Button/Primary", "Size=Xs, State=Default", props={"Label": "Restore this version"})
    back = X("Secondary action", "Button/Secondary", "Size=Xs, State=Default", props={"Label": "Back to current"})
    return F("Banner", [title, restore, back], dir="H", gap=env.c("SPACE_INLINE"), fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), pad=[None, pad, None, pad],
             w=NEUTRAL_W, h=env.num("HISTORY_BANNER_H"), sx="fixed", sy="fixed", align=("start", "center"))


def failure_banner(tone, env):
    fill, edge = FILL[tone]
    dot = X("Dot", "Status/Dot", "Tone=Error", abs=(0.0, 0.0, 8.0, 8.0)) if tone == "Error" else E("Dot", 8.0, 8.0, fill=env.c("WARNING"), sx="fixed", sy="fixed", abs=(0.0, 0.0, 8.0, 8.0))
    slot = F("Dot slot", [dot], w=12.0, h=8.0, sx="fixed", sy="fixed")
    sentence = T("Sentence", "Rate limited until 14:30." if tone == "Warning" else "Session ended early.", "Body / Regular", env.c("TEXT"), sx="fill")
    copy = X("Tertiary action", "Button/Tertiary", "Size=Xs, State=Default", props={"Label": "Copy details"})
    fix = X("Secondary action", "Button/Secondary", "Size=Xs, State=Default", props={"Label": "Open a shell"})
    side = env.c("INSET_FRAME")
    return F("Banner", [slot, sentence, copy, fix], dir="H", gap=env.c("SPACE_INLINE"), fill=env.c(fill), stroke=env.c(edge), sw=env.c("BORDER_WIDTH"), pad=[None, side, None, side],
             w=FAILURE_W, h=2 * env.num("INSET_FRAME") + env.num("CONTROL_XS"), sx="fixed", sy="fixed", align=("start", "center"))


def banner(values, env):
    tone = TONES[values["Tone"]]
    return history_bar(env) if tone == "Neutral" else failure_banner(tone, env)


def component_sets():
    return [{"name": "Banner/Inline", "row": "Bars & Feedback", "axes": {"Tone": ["Neutral", "Success", "Warning", "Error"]}, "build": banner,
             "uses": ["Button/Primary", "Button/Secondary", "Button/Tertiary", "Status/Dot"], "props": [{"name": "Label", "type": "TEXT", "default": "Viewing Version 1"}],
             "description": "Neutral is the history bar (history/banner.rs, v0.7.5); Warning and Error are the mid-session failure banner (agent_session_failure.rs, v0.7.6); "
             "Success has no painter and takes the Neutral look (rule (c))."}]
