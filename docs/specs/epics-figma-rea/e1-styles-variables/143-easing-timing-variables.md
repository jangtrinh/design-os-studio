# #143 Add easing and timing variables bound to transitions and Motion tracks (E1-11)

| Field | Value |
|---|---|
| Issue | #143 (E1-11, P2). Milestone v0.17 Motion toolkit. It absorbed the duplicate E6-20 draft |
| Still real? | Yes, and **blocked** for the Motion half: #140 and #142 are OPEN. #139 (easing in `designos-doc`) is CLOSED |
| Size | M: about 10 builder hours. Includes a 2 h REA spike |
| Tier | Claude Sonnet: a new variable kind touches every resolve, lint and picker path |
| Depends on | #140 and #142 (AC-2). REA step 0 (AC-1 on prototype transitions) |
| Ready after owner answers? | No. Ready when #142 merges. A transition-only slice could start earlier if step 0 confirms Figma binds them (see §3.0) |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

A designer makes an easing variable (for example "Ease/Standard") and a duration variable ("Duration/Fast" = 150 ms, with a slower value in a "Calm" mode). They bind them to prototype transitions and Motion tracks. Switching the mode changes the feel of every bound animation at once. Bindings survive `.fig` round trips.

## 2. Re-anchor (origin/main)

- FACT: `VariableKind` is `Color | Number | String | Boolean` (`crates/designos-doc/src/variables.rs:64-70`).
- FACT: DTCG import already tags easing and duration tokens in `Variable.extensions["org.designos.figmaTokenType"]` (`variables.rs:131`, doc comment). So they arrive as untyped variables today.
- FACT: prototype transitions hold `easing: Easing` and `duration_ms: u32` (`crates/designos-doc/src/prototype.rs:409-413`). There are no binding keys for them in `property_kind` (`variables_apply.rs:21-44`).
- FACT (schema, `crates/designos-fig/src/figma_schema.txt`):
  - `VariableResolvedDataType.EASING = 15`, `TIMING = 16` (`:2605-2606`);
  - `VariableDataType.EASING = 22` (`:2575`);
  - `VariableField.EASING_DATA = 57` (`:1734`) and `TIMELINE_DURATION = 82` (`:1759`);
  - `TransitionParameterValue { startValue, endValue, easing: EasingData, offsetUs, durationUs, timelineDefId }` (about `:2840-2846`);
  - `StyleIdForAnimation.timelineOffsetVar : VariableData`.
- FACT: `.fig` import already walks `parameterConsumptionMap` entries (`crates/designos-fig/src/fig.rs:1921-1941`), but only for the fields it maps.
- UNKNOWN: whether Figma binds easing/timing variables to *prototype* transitions or only to Motion timelines. Closing check: step 0.

## 3. Scope

0. **REA step 0 (about 2 h)**:
   - which node fields consume EASING/TIMING variables (prototype `transitionEasing`/`transitionDuration` versus keyframe tracks);
   - the `EasingData` payload in `VariableAnyValue` (schema field list);
   - whether TIMING is microseconds.

   If prototype transitions consume them, ship slice T (transitions) before #142; slice M (Motion tracks) follows.
1. **Kinds.** `VariableKind::Easing` and `VariableKind::Timing`, with values `VariableValue::Easing(Easing)` and `VariableValue::Timing(u64 /*µs*/)`. The existing `Easing` type comes from #139. Serde is additive, so old files are unchanged.
2. **Binding keys.** For transitions: `transition/easing` and `transition/duration` on the reaction (exact key per step 0). For tracks: `easingData` and `timelineDuration`, matching the VariableField names. Add them to `property_kind`, `scope_allows` and `set_bound_property`.
3. **Variables panel.** Create, edit and alias easing and timing variables. The easing cell reuses the prototype easing editor; the timing cell is a number in ms (stored µs).
4. **Bind entry points.** On the prototype transition panel's easing and duration rows, and on Motion track rows (slice M).
5. **`.fig`.** Import and export `parameterConsumptionMap` entries for these fields and EASING/TIMING variable values.
6. **DTCG.** Tokens tagged `easing` / `duration` import as the new kinds instead of untyped.

## 4. Acceptance criteria (from the issue, made testable)

- **AC-1** A transition with easing bound to a variable whose Default mode = ease-out and Calm mode = spring. Under Calm, the prototype player uses the spring, verified by a player doc test sampling the curve.
- **AC-2** (slice M) A track whose duration is bound to a timing variable: changing the variable from 200 ms to 400 ms doubles the track's duration in #142's evaluator.
- **AC-3** A `.fig` round trip keeps `parameterConsumptionMap` bindings and the EASING/TIMING variable values.
- **AC-4** Documents without the new kinds serialise byte-identical.
- **AC-5** 1440 capture of the bind entry on the transition panel.

## 5. Edge cases

- An alias from a timing variable to a number variable: invalid (kind mismatch), refused as for other kinds.
- A negative or zero timing: clamped to 0, matching the player's handling of `duration_ms = 0`.
- MCP: `designos_get_variables` returns the new kinds with readable values (`{"easing": "ease-out"}`, `{"timing_ms": 150}`).

## 6. Non-goals

- Spring-physics editor changes.
- Timeline UI (E6).

## 7. Figma parity

Checked: schema enums. Needs REA: step 0.

## 8. Builder rules

Re-anchor against #140/#142 at dispatch. Two PRs if step 0 allows slice T early. Slice T uses `Refs #143`, slice M uses `Closes #143`.
