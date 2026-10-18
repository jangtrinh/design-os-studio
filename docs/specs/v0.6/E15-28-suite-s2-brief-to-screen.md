# E15-28 Add suite S2 brief-to-screen on two open design systems

- Issue: #617 (task E15-28, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/suites/s2/{briefs.toml,tasks.toml,split.lock,README.md}` (new), `crates/designos-eval/suites/s2/ds/{shadcn,carbon}.ds.toml` (new library recipes), `crates/designos-eval/src/suites/s2.rs` (new).

## 1. Summary and user story

As the team, we want to measure grounding and taste: given a written brief and a design system loaded as a library, does the agent build a screen from that system's components and tokens, and does the result satisfy a task-specific checklist?

## 2. Current state

- FACT: published libraries exist in `designos-library` (CLAUDE.md §2 crate map: "published libraries").
- FACT: E15-24's kit definitions re-create shadcn/ui (MIT) and IBM Carbon (Apache-2.0) tokens and components as our own documents (E15-23 register).
- FACT: E15-21 defines the surface ids used as tags; E15-31 defines the checklist judge.
- FACT: commercial kits are not open; the owner's video kit is not an eval source (E15-23 ban list).

## 3. Behaviour rules

1. **Briefs.** 50 briefs in `briefs.toml`, written by us, each 40–120 words, describing a product screen with a user, a goal and required content. Distribution: web-app 18, landing-page 10, mobile 14, table 4, design-system 2, slides 2. Each brief has `surface`, `checklist` (5–10 items) and optional `required_components` (role names such as `primary button`, `data table`).
2. **Design systems.** Two libraries, built by `designos-eval gen-ds --kit shadcn|carbon` from the E15-24 kit definitions into the cache (`gen-ds-<kit>.designos`, published as a library in the sandbox). The recipe files record kit version and attribution.
3. **Tasks** = briefs × design systems = 100. Task id `s2-<nnnn>-<kit>`. Each task starts from a blank document with the kit library enabled (subscribed) and the kit's style variables available, and asks for one frame named after the brief.
4. **Checklist rules.** Each item is a yes/no question a judge can answer from the screenshot plus the node outline, for example `Does the screen show exactly one primary action?`, `Is every table column header present: Name, Status, Owner, Updated?`. Items are concrete, observable and never about taste words alone. Every brief also gets three fixed grounding items appended automatically:
   - `Are interactive controls instances of the library components?` (deterministic proxy: `instance_ratio` over controls),
   - `Are colours taken from the library variables?` (deterministic proxy: `token_coverage`),
   - `Does the frame pass its surface profile with no errors?` (deterministic: E15-21 findings).
   The three are scored deterministically, not by the judge.
5. **Scores per run.** Deterministic metrics (E15-26) always; `checklist_pass_rate` (E15-31) when a judge is configured; `grounding.library_instance_ratio` = instances of the kit's components / interactive-looking nodes (buttons, inputs, selects, tabs, rows, detected by name or main-component name) in the output frame.
6. **Stub runs (AC2).** With the stub agent and a scripted response per task (a minimal valid frame), all 100 tasks complete and produce deterministic metrics; judge fields are `null` with `notes: "no judge configured"`.
7. **Held-out.** 30% of briefs are held out by the E15-25 rule on the brief id (so both kit variants of one brief share the split).

## 4. Data model / schema

```toml
# briefs.toml
[[brief]]
id = "0007"
surface = "web-app"
title = "Team access requests"
brief = "An admin reviews pending access requests …"
required_components = ["primary button", "data table", "badge"]
checklist = [
  "Is there a table listing requests with columns Name, Team, Requested, Status?",
  "Does each row offer Approve and Reject actions?",
  "Is the page title 'Access requests' visible at the top left?",
  "Is there an empty state message for when no requests exist?",
  "Does the screen show exactly one primary action outside the table?" ]
```

`tasks.toml` is generated from `briefs.toml` × kits by `designos-eval lock --suite s2 --write` and committed; editing briefs without regenerating fails the split-lock check.

## 5. Contracts

- `designos-eval gen-ds --kit shadcn|carbon [--out path]` builds the library document.
- `designos-eval run --suite s2 --agent … [--judge …]` (E15-25, E15-31).
- JSONL `run.metrics` adds `grounding.library_instance_ratio`, `grounding.token_coverage` (alias of E15-26), `grounding.surface_errors`, and `checklist_pass_rate` (nullable).

## 6. File-format mapping

Library documents are native `.designos` published in the sandbox library store.

## 7. UI

None.

## 8. Edge cases

1. Agent detaches library components: counted as non-instances; the grounding proxy drops.
2. Agent creates local copies of library variables: `token_coverage` counts them as bound, but `grounding.library_token_share` (bindings to library variables / all bindings) exposes it; reported, not gated.
3. Brief whose surface is `slides` in a kit without slide components: allowed; the kit's typography and colours still apply.
4. A checklist item that the judge marks `n/a`: excluded from the pass rate denominator; three or more `n/a` items in one run flag the brief for review in the suite report.
5. Library publish fails in the sandbox: run recorded as `crashed` with the reason.

## 9. Non-goals

- Fidelity to a reference image (S1).
- Writing the judge (E15-31).

## 10. Test plan

Seam: stub run through `cargo test -p designos-eval suites::s2`.

Fail-before: the suite does not exist.

| Test | Input | Expected |
|---|---|---|
| `every_brief_has_checklist_and_surface` (AC1) | briefs.toml | 50 briefs; each has a valid E15-21 surface and 5–10 checklist items |
| `stub_scores_all_100_tasks` (AC2) | stub agent | 100 tasks × runs produce run lines with deterministic metrics; no crash |
| `tasks_match_briefs` | regenerate tasks.toml | equal to committed file |
| `both_kits_share_split` | split.lock | `s2-0007-shadcn` and `s2-0007-carbon` have the same split |
| `checklist_items_are_questions` | all items | end with `?`, ≤ 160 characters, no item repeated within a brief |
| `ds_library_builds` | gen-ds for both kits | libraries publish; component and variable counts printed |

Visual evidence: terminal capture of the stub run summary (100 tasks) and one rendered stub frame.

## 11. Performance budgets

- Library build ≤ 10 s per kit (release).
- Stub run of 100 tasks × 1 run ≤ 5 min on CI.

## 12. Dependencies and build order

- Needs E15-25, E15-23; uses E15-24 kit definitions and E15-21 surface ids; judge scores need E15-31.
- Parallel with E15-27 and E15-31.

## 13. Owner rulings that apply

- The eval never uses client files; only open kits (shadcn/ui MIT, Carbon Apache-2.0) re-created as our own documents.
- R4.13: live runs go through the user's agent CLI.
