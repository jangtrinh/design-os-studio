# E15-30 Add suite S4 multi-screen flows with a consistency metric

- Issue: #619 (task E15-30, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/suites/s4/{flows.toml,tasks.toml,split.lock,README.md}` (new), `crates/designos-eval/src/metrics/consistency.rs` (new), `crates/designos-eval/tests/fixtures/s4/` (new twin fixtures).

## 1. Summary and user story

As the team, we want to measure whether an agent keeps a flow of 4–6 screens consistent: the same components for the same roles, identical title sizes, shared tokens and one spacing rhythm, because cross-screen consistency is a known weakness.

## 2. Current state

- FACT: no consistency metric exists. E15-26 gives per-document deterministic metrics; E15-21 defines text-role resolution (§3.4) and the `title_size_consistent` rule for tagged frames on one page.
- FACT: instance links (`Node.component`, `crates/designos-doc/src/node.rs:122`) and variable bindings (`node.rs:128`) are available for every node.

## 3. Behaviour rules

1. **Flows.** 10 flows, each 4–6 screens, written by us: sign-up and onboarding (mobile, 5), checkout (web, 4), settings and billing (web, 5), project creation wizard (web, 6), inbox to detail to reply (mobile, 4), booking (mobile, 5), dashboard drill-down (web, 4), file upload and share (web, 4), support ticket (mobile, 4), team invite and roles (web, 5). Each flow has one brief per screen plus a flow brief, a surface, and an ordered list of screen frame names the agent must create on one page.
2. **Roles.** Each screen's nodes are assigned roles:
   - `title`: E15-21 text-role resolution, restricted to the single largest title-role text in the top 25% of the screen;
   - `primary-action`, `secondary-action`, `input`, `nav`, `tab-bar`, `list-row`, `card`: a node whose name or main-component name matches the role's pattern (`/primary|cta/i` with a button-like shape for primary action, `/button|btn/i` otherwise for secondary, `/input|field|text ?box/i`, `/nav|sidebar|top ?bar/i`, `/tab ?bar/i`, `/row|item|cell/i` with ≥ 3 siblings of the same structure, `/card/i`).
3. **Sub-scores** (each in [0, 1]):
   - **Component consistency** `C_comp`: for each role present on ≥ 2 screens, the share of its occurrences that are instances of the role's modal main component (the most frequent one; ad-hoc frames count as their own "component"). Average over roles; 1 when no role repeats.
   - **Title consistency** `C_title`: the share of screens whose title has the modal (font family, font size, font weight, fill binding or fill colour) tuple. A screen without a title counts as non-modal.
   - **Token consistency** `C_token` = `1 − (max_s tc_s − min_s tc_s)` where `tc_s` is E15-26 `token_coverage` per screen (screens with `null` excluded); additionally multiplied by the share of bound colour variables that come from the same collection across screens.
   - **Spacing rhythm** `C_space`: per screen, the set `G_s` of distinct gap and padding values used by auto-layout frames (rounded to 0.5 px). `C_space` = mean over screen pairs of the Jaccard index `|G_a ∩ G_b| / |G_a ∪ G_b|`; 1 when only one screen has values.
4. **Consistency** = mean of the four sub-scores. All sub-scores are also written.
5. **Determinism (AC2).** Ties for modal values break by the smallest serialised value (font size, then family name, then component id); screens iterate in flow order; pairs in index order.
6. **Twin check (AC1).** For a flow whose screens all use one title style, changing one screen's title to a different size must lower `C_title` by exactly `1/n` (n screens) and lower the overall consistency score.
7. **Other metrics** per run: E15-26 metrics on the union of the flow's frames, `screens_built` / `screens_requested`, and judge checklist (E15-31) with flow-level items such as `Do all screens use the same navigation pattern?`.

## 4. Data model / schema

```toml
[[flow]]
id = "f03"
name = "Settings and billing"
surface = "web-app"
brief = "A workspace admin updates billing details and changes the plan."
screens = ["Settings / General", "Settings / Members", "Billing / Overview", "Billing / Change plan", "Billing / Confirmation"]
screen_briefs = ["…", "…", "…", "…", "…"]
checklist = ["Do all screens share the same sidebar with the current section highlighted?", "…"]
```

```rust
pub struct Consistency { pub component: f64, pub title: f64, pub token: f64, pub spacing: f64, pub overall: f64,
    pub roles: BTreeMap<String, RoleStat> }
pub fn consistency(doc: &Document, screens: &[NodeId]) -> Consistency;
```

JSONL `run.metrics`: `consistency.component`, `consistency.title`, `consistency.token`, `consistency.spacing`, `consistency.overall`, `screens_built_ratio`.

## 5. Contracts

`designos-eval consistency --doc out.designos --frames "Settings / General,Settings / Members,…"` prints `Consistency` JSON.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Missing screens: consistency is computed on the screens that exist; `screens_built_ratio` reports the gap; a flow with fewer than 2 built screens gets `null` consistency.
2. Two titles with equal size in one screen: the upper-left one wins (smallest y, then x).
3. Screens of different sizes in a web flow: allowed; sub-scores do not depend on screen size.
4. Instances of different variants of one component set count as the same component for `C_comp` (variant changes are legitimate across screens).
5. Text styles: two titles using the same text style but an overridden size count as different (the tuple uses resolved values).

## 9. Non-goals

- Judging whether the flow makes sense (judge checklist covers that).
- Cross-page flows: the flow must live on one page.

## 10. Test plan

Seam: `cargo test -p designos-eval metrics::consistency` with twin fixtures built in test code.

Fail-before: the module does not exist.

| Test | Input | Expected |
|---|---|---|
| `off_size_title_scores_lower` (AC1) | 4-screen flow, titles 32/32/32/28 vs twin 32/32/32/32 | `C_title` 0.75 vs 1.0; overall lower for the off-size flow |
| `metric_is_deterministic` (AC2) | same document, 3 runs | bit-equal results |
| `adhoc_button_lowers_component_score` | 4 screens, primary action is an instance on 3 and a plain frame on 1 | `C_comp` for primary-action 0.75 |
| `spacing_jaccard` | screen A gaps {8,16,24}, B {8,16,32} | 2/4 = 0.5 |
| `token_spread` | coverage 0.9 and 0.6 | `1 − 0.3 = 0.7` before the collection factor |
| `flows_are_well_formed` | flows.toml | 10 flows, 4–6 screens each, screen names unique per flow |

Visual evidence: terminal capture of the twin test plus a contact sheet of the two 4-screen fixtures.

## 11. Performance budgets

≤ 50 ms per flow of 6 screens with ≤ 5k nodes each. CPU renders are not needed for this metric.

## 12. Dependencies and build order

- Needs E15-25, E15-26 (and E15-21 role resolution).
- Unblocks E15-33 (S4 transcripts can be replayed too).

## 13. Owner rulings that apply

- Full scope: four sub-scores, 10 flows.
- The eval never uses client files.
