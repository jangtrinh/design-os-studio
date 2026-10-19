# E15-13 Auto-fix safe mechanical lint findings in the engine and return a diff report

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | E15-12, E15-10, E15-11, E15-6 (for the idempotency undo test only) |
| Unblocks | E15-14, part-2 eval (auto-fix rate metric) |
| Touches | `crates/designos-doc/src/lint/fix.rs` (replaces `lint_fix_document`), `crates/designos-mcp/src/tools/issues.rs`, `crates/designos-mcp/src/tools/session_options.rs` |

## 1. Summary and user story

As an agent, the engine applies the fixes that have exactly one correct answer and leave the rendered
result unchanged right after my write (a fill sizing with no layout parent, an exact token match) and
tells me precisely what it changed, so I do not spend a round trip on them. Fixes that change what the
user sees (a 15 px gap snapped to 16, a missing text fill) are listed as suggestions with a Fix action. As a user, I can switch this to
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
- E15-12 computes findings for the touched scope of every mutating call; E15-11 marks unique exact
  token matches `safe` (off-scale snaps change spacing, so they are never `safe`).

## 3. Behaviour rules

### 3.1 Tiers

Safe tier (applied automatically when the mode is `on`). Owner rule: a fix is automatic only when it
leaves the rendered result unchanged (see the owner ruling at the end of this file):

| Rule | Fix | Safety condition |
|---|---|---|
| `sizing-conflict` (fill/hug without an auto-layout parent) | set that axis's sizing to Fixed at the current size | parent has no auto layout; no size change results |
| `sizing-conflict` (fill on the primary axis of a hugging parent) | set the child's axis to Fixed at its current resolved size | resolved size unchanged after relayout (checked) |
| `hardcoded-value` | bind to the variable or link the style | `Match::One`; the variable is constant across modes (or the style is a whole-value match), so the resolved value is unchanged in every mode |

Suggest tier (never applied automatically, listed in the report as `suggested`, each with a Fix action
in the issues block and the panel): `text-missing-fill`, `absolute-in-auto-layout`, `off-scale` (every
snap, unique or not, changes a gap, padding, radius or font size), `contrast`, ambiguous token matches,
`text-truncated`, `small-touch-target`, `tiny-text`, `default-layer-name`, and any fix that changes a
visible colour or any visible size, position or order.
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
fixes to `suggested`. `rules` accepts only safe-tier rule ids (`sizing-conflict`, `hardcoded-value`);
a suggest-tier id is rejected with `UNKNOWN_FIELD` naming the accepted ids, so no setting can switch a
suggest-tier rule to automatic.

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
             {"node_id":93,"node_name":"Card","property":"fills/0","before":"#0D99FF","after":"var:Color/Primary","rule":"hardcoded-value"}],
  "would_apply":[],
  "suggested":[{"node_id":95,"node_name":"Caption","rule":"contrast","fix":{"kind":"bind","property":"fills/0","to":"var:Color/Text strong"}},
               {"node_id":97,"node_name":"List","rule":"off-scale","fix":{"kind":"bind","property":"itemSpacing","from":15,"to":"var:Spacing/4"}}],
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
   (sizing → binds); B re-validates and is skipped if stale, then picked up by
   pass 2 if still valid.
2. A safe fix would create a new error (binding to a variable whose value later differs per mode, or a
   sizing change that exposes a clipped child): pass 2's lint reports it; the engine never reverts a
   safe fix automatically; the agent sees both in the reply.
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
| `text_missing_fill_is_suggestion_only` | text with no visible fill, written by batch | `text-missing-fill` only in `suggested` with its fix; `applied` empty; document equal to the no-fix state |
| `absolute_in_auto_layout_is_suggestion_only` | child with `absolute: true` inside an auto-layout parent | `absolute-in-auto-layout` only in `suggested`; `absolute` and sibling order unchanged |
| `off_scale_is_suggestion_only` | auto layout gap 15 on the default 4 px scale | `off-scale` only in `suggested` (fix to 16 or the bound step); `itemSpacing` still 15 |
| `bind_unchanged_value_applied` | fill equal to exactly one mode-constant variable | `applied` has `hardcoded-value`; resolved fill unchanged in every mode |
| `binding_never_overwritten` | gap bound to a variable whose value is off-scale | no fix |
| `instance_mirror_skipped` | exact-match hard-coded fill inside an instance's mirrored child | no fix; finding still reported |
| `undo_of_auto_fix_keeps_write_key_valid` | write with idempotency key `k1` (E15-6) that triggers an applied fix; one `designos_undo`; resend `k1` | replay (`designos/idempotent_replay`); the agent's write is still present, only the fix is gone; E15-6 stores `undo_depth` before the fix entry |
| `two_pass_limit` | `plan_fixes` driven by a test rule that reports a new safe finding after every pass (the shipped safe rules cannot reliably chain, so the cap is tested with an injected rule) | `passes: 2`, no third pass |

Fail-before: `fill_child_without_layout_parent_fixed` fails today (no `auto_fix` key).

## 11. Performance budgets

Planning is O(findings); applying ≤ 200 fixes plus relayout and re-lint stays within an extra 30 ms on
top of E15-12's 50 ms per call (recorded). No whole-document passes.

## 12. Dependencies and build order

After E15-12 (findings and reply), E15-10 (contrast stays suggest-tier), E15-11 (`safe` flags) and
E15-6 (wave 3; the `undo_of_auto_fix_keeps_write_key_valid` test needs idempotency keys). Before E15-14.

## 13. Owner rulings

- Auto-fix (2026-10-18): safe mechanical fixes apply automatically with a diff report; configurable to
  off or report-only.
- R2.7: ambiguous token matches are reported without a fix.
- Parallel agents hard-locked to frames (RD-3): fixes never touch another agent's frames.

## Owner ruling (2026-10-18): tier change
"Missing text fill" and "absolute child inside auto layout" are moved from the automatic tier to the **suggest tier**. They appear in the issues block and the panel with a Fix action, and they are never applied automatically. The automatic tier is limited to fixes that leave the rendered result unchanged. The tests must assert that these two findings arrive as suggestions with no document change.

Consequence for off-scale (applied 2026-10-19 after the independent review): E15-11's snap of a gap, padding, radius
or font size changes spacing, so `off-scale` is suggest-only; its `safe` flag is always `false`. The
safe tier is therefore `sizing-conflict` and `hardcoded-value` only. The tests above assert the
suggestion and an unchanged document for each of the three.
