# #663 Check designs inside instances: scan cap, contrast suppression and a bound-override fixture

| Field | Value |
|---|---|
| Issue | #663 (three follow-ups from the #577 review; none blocks a release) |
| Size / priority | M / P2 |
| Depends on | none |
| Unblocks | none |
| Touches | `crates/designos-app/src/check_designs/mod.rs` (audit loop, constants), `check_designs/contrast.rs` (`check`, `:180-195`), tests in `check_designs/tests.rs` and `check_designs/tests/instances.rs`, the truncation note in `crates/designos-app/src/panels/check_designs.rs` |

Code baseline: `origin/main@01d48f2` (0.5.0). Moved references: the issue's `mod.rs:129` is now
`mod.rs:139-171` (the loop), the cap test at `tests.rs:462-473`, and `contrast.rs:179` is `:180-195`.

## 1. Summary and user story

As a designer running Check designs on a page full of component instances, the audit reaches every
frame, reports a contrast failure that an ancestor's lowered opacity causes inside an instance, and
has a test that proves a raw value replacing a bound property is listed.

## 2. Current state (FACT)

### 2.1 Scan cap

- `audit_with_libraries` (`check_designs/mod.rs:139-171`) keeps two counters: `visited` and
  `report.scanned`. Every visible, non-page layer increments `visited` and runs the three rule
  checks (`hardcoded::check`, `contrast::check`, `detached::check`, `:158-164`); `report.scanned` grows
  only for layers that are not inside an instance, carry an override, or produced a finding
  (`:165-167`).
- The stop condition uses `visited`: `if visited >= MAX_SCANNED_LAYERS` (`:155`), `MAX_SCANNED_LAYERS = 20_000`
  (`:31`), sets `truncated` and breaks. The depth-first stack (`:150-169`) means a page whose first
  sibling holds 20,000 mirror layers inside instances never reaches the sibling frames after it.
- The panel prints `report.scanned` as "layers checked" (`panels/check_designs.rs:102-105`) and a
  note when `report.truncated` (`:151-153`). The audit runs synchronously on the UI thread
  (`state.rs:61-75`, `refresh_check_designs`).
- Test: `an_oversized_page_is_cut_off_and_says_so` (`tests.rs:462-473`) builds 20,005 plain rectangles and
  asserts `truncated` and `scanned == MAX_SCANNED_LAYERS`.

### 2.2 Contrast suppression

- For a text layer inside an instance, `contrast::check` skips the row when the instance's own
  resolved properties equal the main's and the main fails with the same backdrop colour
  (`contrast.rs:180-195`): `failure(doc, src_id, bounds).is_some_and(|sf| sf.bg == f.bg)`.
- `Failure` holds `bg` (the plain backdrop colour on screen, without the text), `alpha`, `ratio`
  and `required` (`contrast.rs:81-93`). An ancestor's opacity scales the text's composite
  (`backdrop.rs`: `acc.over(own).scaled(layer.opacity)`), so lowering an ancestor's opacity in the
  instance changes `ratio` while `bg` stays the page or fill colour. The suppression then hides a
  failure that differs from the main's.
- `same_own` compares the text layer's own opacity, blend, fills, runs and required ratio
  (`:181-192`), never an ancestor's.

### 2.3 Bound-override fixture

- Rule: a value inside an instance that equals the main's is not reported there; an overridden value
  is (`hardcoded::paints_rule`, `check_designs/hardcoded.rs:636-689`): it returns early only when
  both the paints and the variable bindings equal the main's (`:641-650`), skips a paint that is bound
  (`:655-657`), and otherwise looks for a variable with the same value (`index.find`, `:661`),
  offering `Fix::BindVariable`.
- #52 acceptance 2: "A hard-coded override on a bound property is listed with the Bind fix, applied as
  an override". The existing tests in `tests/instances.rs` override a fill the main did not bind
  (`overridden_instance_child_text_or_fill_is_reported_on_the_instance_child`, `:45`, main swatch
  `gray(0x20)`) or fix one the same way (`:103`). No test has a main that binds `fills/0` to a variable
  and an instance that replaces it with a raw value.
- A raw colour that matches no variable produces no finding today (`Match::None => {}`, `:663`),
  whether the layer is in an instance or not.

## 3. Behaviour rules

### 3.1 Scan cap: separate the work cap from the meaningful-layer cap

1. Two limits replace the single one:
   - `MAX_SCANNED_LAYERS = 20_000` stays the cap on meaningful layers, `report.scanned`: the audit
     stops when `report.scanned` would exceed it, sets `truncated`, and the "layers checked" number
     never exceeds it;
   - a new `MAX_VISITED_LAYERS` caps the work: layers examined, mirrors included. It exists so a page
     of mirrors cannot stall the UI thread. Hitting it also sets `truncated`.
2. **One predicate for the cheap skip.** A visible layer is a *quiet mirror* when it sits inside an instance, has `component.source` set, an empty `component.overrides`, and every ancestor up to the instance root is
   un-overridden (`overrides` empty) and fully opaque (`opacity >= 1.0`, normal blend). A quiet mirror counts only toward `visited` (a cheap lookup) and skips **all three** checks (`hardcoded::check`,
   `contrast::check`, `detached::check`): with no override of its own and no ancestor that can change its backdrop or its text alpha, none of them can produce a row that the main component does not already produce.
   Every other layer runs all three checks, contrast included, exactly as today; in particular a layer under an ancestor whose opacity, blend or fills were overridden in the instance is not a quiet mirror, so the
   ancestor-opacity contrast row of 3.2 is still found. (Today every visited layer runs all three checks; this predicate is what makes a large `MAX_VISITED_LAYERS` affordable.)
3. `MAX_VISITED_LAYERS` is set by measurement (section 11), initial value 100,000 pending that measurement, and never below `2 * MAX_SCANNED_LAYERS`. A quiet mirror costs one lookup, so 40,000 of them are far under the
   UI-thread budget; if the measurement shows otherwise, the cheap-skip design is insufficient and the task stops and reports instead of lowering the cap.
4. The truncation note names which limit was hit (checked layers or work) in one sentence; copy in
   section 7.

### 3.2 Contrast suppression compares what the user would see

1. The instance row is suppressed only when the main fails with the same backdrop colour and the same
   contrast ratio: `sf.bg == f.bg && (sf.ratio - f.ratio).abs() < 1e-6 && sf.required == f.required`.
   (The ratio is the number the row prints, so equal ratio means the user sees the same failure.)
2. An override that changes an ancestor's opacity, blend or fills inside the instance (anything that
   moves the ratio) therefore produces its own row on the instance's text layer, with the fix path the
   rule already has (override through the instance, never the main).
3. No change to the finding text, the fix, or the main component's own row.

### 3.3 Bound-override fixture

1. Add the missing test (no production change expected). If it already passes, the test is still
   added and must be shown red once by a deliberate violation (section 10), which is how every gate
   counts.
2. Behaviour the test pins: main binds `fills/0` to variable V (colour C); the instance child replaces it
   with a raw colour equal to the value of another variable W; the audit lists the instance child with
   `Fix::BindVariable` to W, applied as an override (`OverrideField::Fills`), main untouched, one undo step
   (same assertions as `:103-147`).
3. A raw colour that matches no variable stays unreported (owner ruling 2026-10-19): the same raw colour on a normal layer is not listed either, and the Bind fix needs a variable to bind.

## 4. Data model / schema changes

None. Two constants (`MAX_VISITED_LAYERS`), no field on `Report` besides an optional
`truncated_by: Option<Truncation>` (`Scanned` | `Work`) if the note names the limit (rule 3.1.4); `Report`
is `Clone + PartialEq`, so tests compare it as before.

## 5. Contracts

`audit` / `audit_with_libraries` keep their signatures. `Report.scanned` keeps its meaning (meaningful
layers). `Report.truncated` is true when either limit was hit.

## 6. File-format mapping

None.

## 7. UI

The existing truncation note (`truncation_note`, `panels/check_designs.rs`) gets one of two sentences:
"Stopped after {MAX_SCANNED_LAYERS} checked layers. Check a smaller selection to see the rest." or "Stopped
after {MAX_VISITED_LAYERS} layers examined. Check a smaller selection to see the rest." Same style,
colour and slot as today's note. No other visual change. Desktop capture at 1440 of the note in both
forms (use a synthetic page built by the test generator); responsive SKIPPED (owner default).

## 8. Edge cases

1. A page with 25,000 mirror layers in its first frame, then a frame with one failing text: the failing
   text is reported (item 1 acceptance).
2. A page of exactly 20,000 meaningful layers: not truncated.
3. A page where meaningful layers exceed 20,000 first: truncated as today, `scanned == 20_000`.
4. Mirror text under an ancestor with opacity 0.5 set in the instance: contrast row on the instance text
   (item 2).
5. Same opacity set on the main component: the main reports once; the instances stay suppressed (their
   ratio equals the main's).
6. Instance child whose raw override colour matches two variables: report-only row (existing `Ambiguous`).
7. Hidden layers: skipped before any counter, as today.

## 9. Non-goals

- Moving the audit off the UI thread (E3-12 / #157 owns that).
- Changing which rules exist or their messages.
- Reporting raw values that match no variable (ruled out by the owner, 2026-10-19).

## 10. Test plan (red first)

Seams: `check_designs` unit tests, `fx()` builders in `tests.rs`, `instances.rs` fixtures. Synthetic
data only.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `mirror_heavy_page_still_reaches_the_later_frame` | page: frame A holds `MAX_SCANNED_LAYERS + 5_000` quiet mirrors (25,000 today), frame B holds a failing text; `audit` | B's contrast row present; `scanned` small; not truncated. The fixture size is tied to `MAX_SCANNED_LAYERS`, and rule 3 requires `MAX_VISITED_LAYERS >= 2 * MAX_SCANNED_LAYERS`, so it always fits under the work cap |
| `quiet_mirror_predicate_keeps_the_ancestor_opacity_row` | the 3.2 fixture inside a mirror-heavy page | the instance text row is still reported (the layer is not a quiet mirror) |
| `cheap_skip_changes_no_findings` | run the existing `tests/instances.rs` fixtures and the library and covering-sibling fixtures with the cheap skip on and off (a `#[cfg(test)]` switch) | identical `findings` and `scanned` in every fixture |
| `meaningful_layer_cap_still_truncates` | existing `an_oversized_page_is_cut_off_and_says_so` | unchanged |
| `work_cap_truncates_and_says_which` | page of `MAX_VISITED_LAYERS + 5` quiet mirrors (the assertion applies only to a fixture that exceeds the chosen cap) | `truncated`, `truncated_by == Work` |
| `ancestor_opacity_override_reports_contrast_on_the_instance_text` | main text passes; in the instance set the text's parent frame opacity 0.4 with an override; `audit` | one `Rule::Contrast` row on the instance text (suppressed before) |
| `same_opacity_on_the_main_is_reported_once_on_the_main` | set opacity on the main's frame | one row, on the main |
| `bound_main_property_replaced_by_raw_value_is_listed_with_bind_fix` | main swatch binds `fills/0` to `Color/Primary`; instance child gets raw colour = `Color/Accent` value, `OverrideField::Fills`; `audit`; `check_designs_fix_all` | one `HardCodedValue` row on the instance child; fix binds `Color/Accent`; main snapshot unchanged; undo restores |
| `raw_value_matching_no_variable_is_not_listed` | raw colour equal to no variable | no row (pins today's behaviour; flips only with the owner's answer) |
| `bound_override_test_fails_on_a_deliberate_violation` | (manual, in the PR text) make `same_bindings` ignore bindings, run the test | red; revert; green |

Run: `cargo test -p designos-app check_designs`. Evidence: red/green terminal capture for items 1 and 2,
the deliberate-violation run for item 3, and the 1440 capture of the Check designs panel with both
truncation notes.

## 11. Performance budgets

- Work cap value: measured, not guessed. The worker times `audit_with_libraries` on the synthetic
  mirror page at 10k, 50k, 100k and 200k visited layers (release build, otherwise idle machine) and sets
  `MAX_VISITED_LAYERS` to the largest count whose audit stays under 150 ms. The numbers go in the PR
  and in the comment above the constant.
- The cheap-skip (3.1.2) must not make a typical page slower: audit of the 112,474-node synthetic large
  fixture (`large_file_perf` generator pattern) stays within 5% of the pre-change time.

## 12. Dependencies and build order

Independent. `check_designs/` is shared with E15-9..E15-11 (linter work reuses these rules); rebase
rather than hand-merge. The three items can ship as three commits of one PR.

## 13. Owner rulings

- Check designs reports overridden values only and fixes them as overrides (#52).
- Scans must never stall the UI thread (comment at `mod.rs:29`).

## Open questions for the owner

None. The one question of the first draft was answered by the owner on 2026-10-19: a raw override that matches no variable is not listed (rule 3.3.3).
