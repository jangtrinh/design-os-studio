# E15-13 Auto-fix safe mechanical lint findings in the engine and return a diff report

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | E15-12, E15-10, E15-11 |
| Unblocks | E15-14, part-2 eval (auto-fix rate metric) |
| Touches | `crates/designos-doc/src/lint/fix.rs` (replaces `lint_fix_document`), `crates/designos-mcp/src/tools/issues.rs`, `crates/designos-mcp/src/tools/session_options.rs` |

## 1. Summary and user story

As an agent, the engine applies the fixes that have exactly one correct answer right after my write
(a fill sizing with no layout parent, an exact token match, a 15 px gap on a 16 px scale) and tells me
precisely what it changed, so I do not spend a round trip on them. As a user, I can switch this to
report-only or off, and undo it separately from the agent's work.

## 2. Current state (FACT)

- `lint_fix_document(doc, page, rules)` (line 992-1209 of the legacy module in
  `crates/designos-doc/src/`) mutates the document in place, outside `History`, and marks it fully
  dirty (line 1201-1203). It deletes empty containers (lines 1026-1034, 1198-1200), renames layers
  (1073-1106), raises font sizes to 12 (1108-1119), snaps sizes to the 4 px grid (1147-1159) and
  enlarges touch targets to 44 px (1135-1145): several of these change what the user sees or destroy
  layers. Nothing calls it outside its unit test.
- Check designs fixes are user-initiated, re-validated against the current state, and applied as one
  `Tx` (`crates/designos-app/src/check_designs/fix.rs:1-50`, re-validation `fix.rs:52-82`).
- E15-12 computes findings for the touched scope of every mutating call; E15-11 marks off-scale and
  token findings `safe` when unique.

## 3. Behaviour rules

### 3.1 Tiers

Safe tier (applied automatically when the mode is `on`):

| Rule | Fix | Safety condition |
|---|---|---|
| `sizing-conflict` (fill/hug without an auto-layout parent) | set that axis's sizing to Fixed at the current size | parent has no auto layout; no size change results |
| `sizing-conflict` (fill on the primary axis of a hugging parent) | set the child's axis to Fixed at its current resolved size | resolved size unchanged after relayout (checked) |
| `text-missing-fill` | add one solid fill, bound to the same variable (or equal to the same colour) as every other text fill in the nearest frame | all other visible text fills in that frame share one value; else suggest-tier |
| `absolute-in-auto-layout` | clear `absolute`, insert at the flow index whose neighbours' primary-axis centres bracket the child's centre | the child's primary-axis interval overlaps no flow sibling, and the bracket is unique |
| `hardcoded-value` | bind to the variable or link the style | `Match::One`; value unchanged by the bind |
| `off-scale` | set to (or bind to) the nearest step | E15-11 `safe: true` (unique step, distance ≤ max(1, 10%)) |

Suggest tier (never applied automatically, listed in the report as `suggested`): `contrast`,
ambiguous token matches, off-scale beyond the distance limit, `text-truncated`, `small-touch-target`,
`tiny-text`, `default-layer-name`, and any fix that changes a visible colour.
Never fixed by the engine: `empty-container` deletion and every other destructive change.

### 3.2 Common safety conditions (re-checked right before applying each fix)

- The node and property still hold the value the finding saw (no drift since lint).
- The property is not bound to a variable unless the fix is the bind itself (a binding is never
  overwritten).
- The node is not a mirror inside an instance (`component.source` set); the fix belongs on the main
  component, as Check designs does.
- The node is not locked, not in another agent's frame lease (RD-3), not flagged in progress by
  another run (E15-7).

### 3.3 Execution

1. After the agent's call commits (single tool or batch), E15-12 lints the touched scope.
2. Plan safe fixes (`plan_fixes(doc, findings, &AutoFixSettings) -> Vec<PlannedFix>`), each producing
   `Command`s; never direct mutation.
3. Mode `on`: execute all planned fixes as one `Command::Batch` through `History::exec` (a separate
   undo entry), relayout, publish a separate `LivePatch` line labelled `Auto-fix`. Then re-lint the
   scope once; if new safe findings appear, run one more pass (max 2 passes) into the same undo entry.
   The final issues block reflects the post-fix state.
4. Mode `report_only`: plan only; the report lists the same fixes under `would_apply`; nothing
   changes.
5. Mode `off`: no planning, no `auto_fix` key in the reply.
6. Dry runs always behave as `report_only` against the trial.
7. If applying fails (command error), the auto-fix entry is rolled back as a whole; the agent's write
   stays committed; the reply carries `auto_fix.error`.

### 3.4 Configuration and precedence

`AutoFixSettings { mode: Option<Mode>, rules: BTreeMap<String, bool> }`, at three levels; the most
specific set value wins per field: session (`designos_configure_session`, E15-12) > file
(`Document.auto_fix`) > user (`settings.json` key `auto_fix` in the library root, the existing app
settings file). Default: `mode = on`, every safe-tier rule on. A per-rule `false` moves that rule's
fixes to `suggested`.

## 4. Data model / schema changes

- `Document.auto_fix: Option<AutoFixSettings>` (serde default, skipped when `None`).
- `lint::fix::{PlannedFix { node, node_name, property, before, after, rule, commands }, plan_fixes}`.
- The legacy `lint_fix_document` wrapper is removed (no external caller); its safe parts are the
  fixes above, its unsafe parts are dropped.

## 5. Contracts

Reply key on mutating tools (next to the issues block):

```json
{"auto_fix":{"mode":"on","undo_label":"Auto-fix","passes":1,
  "applied":[{"node_id":91,"node_name":"Badge","property":"layoutSizingHorizontal",
              "before":"FILL","after":"FIXED","rule":"sizing-conflict"},
             {"node_id":93,"node_name":"Card","property":"itemSpacing","before":15,"after":"var:Spacing/4","rule":"off-scale"}],
  "would_apply":[],
  "suggested":[{"node_id":95,"node_name":"Caption","rule":"contrast","fix":{"kind":"bind","property":"fills/0","to":"var:Color/Text strong"}}],
  "error":null}}
```

`designos_configure_session` gains `auto_fix: {mode: "on"|"report_only"|"off", rules: {<rule>: bool}}`.
Undo: one `designos_undo` (or one Studio undo) after a call with fixes removes only the fixes; a
second undo removes the agent's write.

## 6. File-format mapping

`Document.auto_fix` is native-only; `.fig` export writes it to document plugin data
`designos:auto_fix` (nothing vanishes silently).

## 7. UI

Studio surfaces are E15-14. Here: the Studio History panel shows `Auto-fix` as its own entry.

## 8. Edge cases

1. Fix A makes fix B stale (both on the same node): fixes are applied in rule order
   (sizing → absolute → binds → snaps); B re-validates and is skipped if stale, then picked up by
   pass 2 if still valid.
2. A safe fix would create a new error (snapping a gap makes text truncate): pass 2's lint reports it;
   the engine never reverts a safe fix automatically; the agent sees both in the reply.
3. User preference `off` but session sets `on`: session wins for that agent session only.
4. Live editor rejects the auto-fix patch line (publish failure): the auto-fix entry is rolled back,
   the agent's write stays; `auto_fix.error` names the cause.
5. A rule disabled per file appears in `suggested` with `disabled_by: "file"`.
6. Batch with `dry_run:true` and mode on: `would_apply` only.

## 9. Non-goals

- Fixing whole pages proactively (only touched scopes after agent writes; Check designs keeps its own
  user-initiated Fix all).
- Contrast auto-repair.

## 10. Test plan

Seams: designos-doc unit tests for `plan_fixes`; MCP fixture tests for execution.

| Case | Steps | Expected |
|---|---|---|
| `fill_child_without_layout_parent_fixed` | create frame without auto layout; child width Fill via batch | `applied` has `sizing-conflict` FILL→FIXED; node size unchanged |
| `report_only_lists_and_changes_nothing` | same with mode report_only | `would_apply` equals the `applied` list of the previous case; document equal to the no-fix state |
| `undo_once_removes_only_fixes` | case 1 then `designos_undo` | sizing back to FILL, child still exists; second undo removes the batch |
| `contrast_never_auto_applied` | text failing contrast with a passing variable | `contrast` only in `suggested`; fill unchanged |
| `binding_never_overwritten` | gap bound to a variable whose value is off-scale | no fix |
| `instance_mirror_skipped` | off-scale gap inside an instance's mirrored child | no fix; finding still reported |
| `two_pass_limit` | fixture where pass 1 enables another safe fix | `passes: 2`, no third pass |

Fail-before: `fill_child_without_layout_parent_fixed` fails today (no `auto_fix` key).

## 11. Performance budgets

Planning is O(findings); applying ≤ 200 fixes plus relayout and re-lint stays within an extra 30 ms on
top of E15-12's 50 ms per call (recorded). No whole-document passes.

## 12. Dependencies and build order

After E15-12 (findings and reply), E15-10 (contrast stays suggest-tier) and E15-11 (`safe` flags).
Before E15-14.

## 13. Owner rulings

- Auto-fix (2026-10-18): safe mechanical fixes apply automatically with a diff report; configurable to
  off or report-only.
- R2.7: ambiguous token matches are reported without a fix.
- Parallel agents hard-locked to frames (RD-3): fixes never touch another agent's frames.
