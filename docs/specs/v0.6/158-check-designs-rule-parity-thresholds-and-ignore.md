# #158 Check designs: match Figma's rule set, thresholds and dismiss/ignore (E3-13)

| Field | Value |
|---|---|
| Issue | #158 (task E3-13; parent #16; label in-review at spec time; needs the reverse-engineering read "E3-d") |
| Size | M (L if the read finds many rules not listed below) |
| Depends on | #35 (panel on `main`); #157 (the scan job); the E3-d read (step 0) |
| Unblocks | part 1's E15-9 and E15-11 (they consume the rules this task fixes) |
| Touches | `crates/designos-app/src/check_designs/*` (`mod.rs` `Rule` `:34-52`, `contrast.rs`, `hardcoded.rs`, `detached.rs`, new `ignore.rs`, `state.rs`), `crates/designos-doc/src/lib.rs` (a persisted ignore list), `crates/designos-doc/src/history.rs` (one `Command` variant), `crates/designos-app/src/panels/check_designs.rs`, the `.designos` loader, tests per rule |

Code baseline: `origin/main@9f3f7a5`. Issue source: `plans/261017-epics-full-scope/full-scope-audit-e1-e2-e3-e5.md` (row E3-13 line 184; section "REA reads needed", line 207).

## 1. Summary and user story

As a designer, I want Check designs to report what Figma's Check designs reports, with the same thresholds, and I want to ignore a finding once and have it stay ignored after every rescan and every reopen of the file.

## 2. Current state (FACT)

- Three rules exist: `HardCodedValue`, `Contrast`, `DetachedInstance` (`check_designs/mod.rs:34-52`). Hard-coded values cover fills, strokes, text styles, effect lists and number properties against variables and styles of the file and of enabled libraries (`hardcoded.rs`, module doc `:1`; number tolerance 0.0005 at `:19`; library sources: `hardcoded::Index::with_libraries`; tests in `check_designs/tests/numbers.rs` and `libraries.rs`).
- Contrast uses WCAG 2.x **AA** only: 3:1 for large text (24 px, or 18.66 px and bold from weight 700), else 4.5:1 (`contrast.rs:56`). There is no AAA level.
- There is no dismiss or ignore: `Finding`, `Fix`, `Report` hold no ignore state (`mod.rs:55-118`), and the document has no field for it (a search of `designos-doc/src/lib.rs`, `node.rs` and `check_designs/` for `ignore` and `dismiss` finds nothing relevant).
- Fixes run through `Tx`; each fix and "Fix all" is one undo step (module doc, `mod.rs:12-13`).
- The document's history is a `Command` enum (`designos-doc/src/history.rs:8-78`) that other v0.6 tasks extend (part 1's E15-14 adds `SetAutoFix`).
- The panel lists findings in groups by rule, with a count pill per group (`panels/check_designs.rs:186-196`).
- UNKNOWN: Figma's Check designs rule list, its thresholds, whether it offers an AAA level, how its ignore list is keyed and where Figma stores it. The issue says these come from the reverse-engineering read "E3-d" (rule list, number tokens, AAA and large text, ignore list, library sources). No result of that read is on `main`. Everything below that depends on it is marked and is a gate (step 0), not a guess.
- UNKNOWN: whether a `.fig` file can carry an ignore list. Check: the E3-d read, on a file that has ignored findings.

## 3. Behaviour rules

0. **Step 0 (gate): the E3-d read.** Use the project's reverse-engineering tool (the global reverse-engineering rule) on Figma's Check designs. Write `plans/reports/release-060-evidence/rea-e3d-check-designs-rules.md` with: every rule id and name, its trigger, its thresholds (contrast level and large-text definition, number-token properties and tolerance), what an ignore keys on, and where it is stored. Nothing in rules 1 to 6 is coded for a rule the read does not list; a rule the read lists and this spec lacks is added to the table in rule 1 first.
1. **Rule table.** `Rule` becomes data-backed: one row per rule with id, group title, trigger, thresholds. The three present rules stay. Rows still to confirm by step 0, none assumed: contrast at AAA; any further rule family Figma lists.
2. **Contrast levels.** Thresholds become a table: normal text 4.5:1 at AA and 7:1 at AAA; large text 3:1 at AA and 4.5:1 at AAA (the WCAG 2.x values), with the large-text size rule already in `contrast.rs:56`. Whether AAA is a second group, a level switch, or both is set by step 0 (open question 1). An AAA-only failure never appears as an AA failure.
3. **Ignore.** Each finding row has an "Ignore" action. It stores an entry in the document and the finding disappears from the list. Entry key: `(rule id, node id, finding key, value fingerprint)`, where the finding key is the property (`fills/0`, `itemSpacing`, the contrast pair, the detached component) and the fingerprint is a hash of the offending value (raw colour, number, text-and-backdrop colours). The same finding after a rescan has the same key and stays ignored. If the value changes, the fingerprint changes and the finding is a new one that can appear.
4. **Persistence.** The ignore list is saved in the `.designos` file (`check_ignored`, omitted when empty) and survives close and reopen. Ignoring is an undoable edit: one step per ignore, one step for "Restore". Dropping nodes removes their entries on the next save; the list is capped at 10,000 entries (oldest dropped with a visible count in the panel footer).
5. **Show ignored.** The panel footer has "Ignored (N)" that lists ignored findings with a "Restore" action each and "Restore all". N is a separate count pill.
6. **Ignored findings never reappear after rescan.** The rescan (#157's job) filters by the list as it reports; fixed findings that are also ignored are dropped from the list.
7. **`.fig` round trip.** Until step 0 says where Figma keeps the ignore list, `.fig` export writes none and `.fig` import reads none; the panel says nothing about it. After step 0, the mapping goes in section 6.
8. Fixture parity: each rule has a fixture file pair (a document that must report it, a document that must not), generated into `check_designs/tests/` with the threshold edges (just under, exactly at, just over).

## 4. Data model / schema changes

- `Document.check_ignored: Vec<CheckIgnore { rule: String, node: NodeId, key: String, fingerprint: u64 }>`, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- `Command::SetCheckIgnored { before, after }` (or the whole-list form the other `Set*` commands use), undoable and replayable by the live patch log's refusal list like the other document-level commands (it is listed with them in `live_ops_for_command`; an agent cannot change it).
- Old files load unchanged (the field is absent).

## 5. Contracts

- `designos-doc` `lib.rs`/`history.rs` gain the field and the command; `history.rs` is shared with E15-14: add the variant in its own commit and rebase.
- The one-shot `audit_with_libraries` keeps its signature; it takes the ignore list from the document it is given.
- Fingerprints use the zero-allocation hasher style (`HashSerializer`, CLAUDE.md section 4, item 1) on the offending value only, never `to_json`.

## 6. File-format mapping

- `.designos`: `check_ignored` as above.
- `.fig`: UNKNOWN until step 0. Check: the E3-d read finds whether Figma stores ignored findings in the file and under which node or plugin-data key. Until then no mapping and no claim of parity for persistence in `.fig`.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- Each finding row gets an "Ignore" text button of `CONTROL_DENSE` (24 px) height at the right of its fix button, `RADIUS_CONTROL`, neutral fill. It is secondary: the one primary action in the panel is the existing "Fix all". Tooltip: "Hide this finding. It stays hidden until the value changes."
- The footer shows "Ignored" with its count in a separate pill (P5, no count inside the label), as a 24 px row; clicking it expands the ignored list. Ignored rows are `TEXT_SECONDARY` with a "Restore" button.
- A contrast level control appears only if step 0 says Figma has one: a segmented control `AA | AAA`, 24 px high; the choice is stored with the other panel settings.
- No animation, no timer; counts in `font_mono`.
- Desktop capture at 1440: a finding list with Ignore, the ignored footer expanded, an AAA finding. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The user ignores a finding, then edits the offending value: the fingerprint changes; the finding returns as new (rule 3). The old entry is purged on save.
2. The user undoes an ignore: the finding returns on the next scan; the list matches.
3. A node is duplicated: the copy has a new id; its findings are new.
4. A layer inside an instance: the existing rule skips it unless its effective value diverges; ignoring keys on the instance child's id as scanned.
5. A file saved by a newer build with extra ignore fields: unknown fields are kept (the `check_ignored` entries use a struct with a flatten for unknown keys; UNKNOWN whether serde's `deny_unknown_fields` is on for `Document`; check at implementation).
6. 10,001 ignores: the oldest entry drops; the footer count says so.
7. Library findings (from an enabled library): ignorable like any; the key holds the library asset id in the finding key.
8. A rescan running while the user ignores: the running job filters by the list at report time, so the new entry applies at once.
9. `.fig` import of a file with Figma-side ignores: ignored until step 0 maps them; no data is invented.

## 9. Non-goals

- New auto-fixes. A project-wide or library-wide ignore. AI suggestions. Anything the E3-d read does not list.

## 10. Test plan (red first)

Seams: per-rule fixtures; `Document` round-trip tests; the `Sim`/panel tests for Ignore and Restore.

| Case | Expected | Fails before |
|---|---|---|
| `every_rule_in_the_e3d_list_has_a_fixture_pair` | one table row per listed rule; both fixtures behave | yes (list does not exist) |
| `contrast_aaa_normal_text_needs_seven_to_one` | a pair at 4.54:1 passes AA and fails AAA (the test computes the ratio itself); a pair at 7.0:1 passes both | yes |
| `contrast_aaa_large_text_needs_four_and_a_half` | edges just under, at, over | yes |
| `large_text_definition_is_unchanged` | 24 px, and 18.66 px bold from weight 700 | guard |
| `an_aa_failure_is_not_reported_twice_as_aaa` | one finding per level as designed in step 0 | yes |
| `ignore_hides_the_finding_and_survives_a_rescan` | not in the list after the scan | yes |
| `ignored_findings_survive_save_and_reopen` | `.designos` round trip keeps the list | yes |
| `ignore_is_one_undo_step_and_restore_is_one` | history depth +1 each | yes |
| `an_edited_value_brings_the_finding_back` | new fingerprint | yes |
| `fingerprint_uses_no_json_string` | no `to_json` call (probe) | yes |
| `old_files_load_without_the_field` | loads | guard |
| `agent_cannot_change_the_ignore_list_through_the_live_log` | the command is refused by `live_ops_for_command` like the other document-level commands | yes |
| `ignore_cap_drops_the_oldest` | 10,001 entries leave 10,000 | yes |

Run: `cargo test -p designos-app check_designs`, `cargo test -p designos-doc`. Evidence: red/green capture, the step 0 report, the 1440 captures.

## 11. Performance budgets

- Filtering by the ignore list: a hash set lookup per finding, O(1); the list is loaded into the set once per scan. Adds under 0.1 ms to a 10,000-finding report.
- Fingerprinting: one streamed hash of the offending value per finding.
- No new timers; the scan job and its budget are #157's (4 ms per frame).
- No UI-thread `to_json`.

## 12. Dependencies and build order

After #157 (shared job), after part 3's FU-663 (same module), and gated by step 0. `history.rs` is shared with part 1's E15-14. Order in README-part4: FU-663, #157, #158.

## 13. Owner rulings

- Every rule in the reverse-engineering list exists with Figma's thresholds (AAA, large text); dismiss and ignore persists in the document; ignored findings never reappear after a rescan; fixtures per rule. (Issue acceptance criteria.)
- Check designs ships to all users (full-scope notes, E3 section).

## Open questions for the owner (recommended first)

1. AAA presentation. Recommended: follow what step 0 finds in Figma; if Figma has no AAA level, add a second group "Contrast (AAA)" that is collapsed by default, so AA stays the headline.
2. Ignore scope. Recommended: one verb "Ignore" per finding (this value on this layer), plus "Restore" in the footer; a broader "ignore this rule on this layer" only if step 0 shows Figma has it.
3. If Figma stores no ignore list in the `.fig`. Recommended: keep ours in `.designos` only and say so in the release notes.
