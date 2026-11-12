# #141 Model animation styles (StyleType ANIMATION) with picker and .fig round trip (E1-10)

| Field | Value |
|---|---|
| Issue | #141 (E1-10, P2). Milestone v0.17 Motion toolkit (owner amendment 2026-10-17 on epic #14) |
| Still real? | Yes, and **blocked**: there is no Motion model on main, and #140 (E6-1, timelines/tracks/keyframes) is OPEN |
| Size | M: about 8 builder hours once #140 has merged |
| Tier | Argon, if #140 lands a stable timeline API with tests. Claude Sonnet if the style↔timeline link needs judgment, which the controller decides at dispatch |
| Depends on | #140 (open), then the hot-file queue (`fig.rs`, `fig_export.rs`) |
| Ready after owner answers? | No. Ready when #140 merges; no owner question |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

A designer saves a layer's animation (its timeline) as a shared animation style, picks it from a style picker to apply it to other layers, and the styles survive `.fig` save and reopen. Old files open unchanged.

## 2. Re-anchor (origin/main)

- FACT: `Style` is `Fill | Text | Effect | Grid` (`crates/designos-doc/src/lib.rs:1660-1665`). There is no animation variant.
- FACT: there is no timeline, track or keyframe type in `designos-doc`. The only motion types are prototype `EasingType` (`prototype.rs:67`), `Easing` (`:212`) and `Transition { kind, easing, duration_ms }` (`:409-413`).
- FACT (schema, `crates/designos-fig/src/figma_schema.txt`):
  - `StyleType.ANIMATION = 7` (`:1200`);
  - `NodeChange.styleIdsForAnimation = 582 : StyleIdForAnimation[]` (`:1616`);
  - `StyleIdForAnimation { id, timelineDefId, animationStyleId: StyleId, timelineOffset: Int64, generatedAnimationPresets, sortPosition, timelineOffsetVar: VariableData }` (about `:2846-2853`);
  - `KeyframeTrackAnyParameter.animationStyleBindingId = 3` (about `:2828`).
- INFERENCE: one layer can hold several animation styles, each placed on a timeline at an offset. The offset itself can be variable-bound (`timelineOffsetVar`), which ties into #143.

## 3. Scope (to be finalised against #140's merged API)

1. `Style::Animation(AnimationStyle)`, holding the timeline payload in #140's types. `StyleProperty::Animation` and the `style_refs` key `"animation"` follow the pattern of the effect and grid styles (`styles_ops.rs:54-61`). Unlike those, a layer can link several animation styles, so the model is a list `animation_styles: Vec<AnimationStyleRef { style, timeline, offset_us, sort }>`.
2. Create from a layer's timeline, apply to selected layers, detach, rename, describe and order with sort positions, reusing the existing style operations.
3. Style picker: an "Animation" section in the style picker and assets list, using the existing picker components.
4. `.fig`: import and export ANIMATION style nodes and `styleIdsForAnimation`. `timelineOffsetVar` is kept verbatim until #143 resolves it.
5. Old files: no field, no change. Byte-identical serialisation (AC-3).

## 4. Acceptance criteria (from the issue, made testable)

- **AC-1** A Figma fixture with an ANIMATION style applied to two layers round-trips `.fig` → Studio → `.fig`. StyleType 7, all `styleIdsForAnimation` entries and their offsets are unchanged.
- **AC-2** Applying the style links its timeline to the layer's timeline (a doc test on #140's evaluator: the layer's track values at t equal the style's at t − offset).
- **AC-3** Pre-#141 `.designos` and `.fig` fixtures load and save byte-identical.
- **AC-4** A desktop capture at 1440 of the style picker with the Animation section.

## 5. Edge cases

- Applying a style to a layer that already has a manual timeline: the REA must answer this (step 0 at dispatch). The expected behaviour is that both coexist, ordered by `sortPosition`.
- Library animation styles: publish and consume stay with E6-19 (epic note), not here.
- A deleted style while it is in use: same as other styles (#127 rule: detach and keep values).

## 6. Non-goals

- Library publish/consume (E6-19).
- Motion playback UI (E6).

## 7. Figma parity

Checked: schema only. Needs REA at dispatch on the editor bundle: how apply links tracks (`animationStyleBindingId`) and what detach does.

## 8. Builder rules

Re-anchor this spec against #140's merged code before dispatch. Expected drift: the type names in §3.1. `Closes #141`.
