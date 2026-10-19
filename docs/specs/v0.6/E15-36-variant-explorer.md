# E15-36 Explore design variants in parallel frames or branches with mixed models

- Issue: #625 (task E15-36, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/variants.rs` (new), the RD-3 agent orchestration module from #572 (`spawn_agents`), `crates/designos-doc/src/explorations.rs` (new: stored explorations), `crates/designos-doc/src/style_compile/variant_modes.rs` (new), `crates/designos-doc/src/agent_meta.rs` (SC-4: `variant` field), `engine.rs` tool list.

## 1. Summary and user story

As a designer, I want to ask for N variants of a brief and have N agents (possibly on different models) build them at the same time, each locked to its own numbered frame or its own branch, keeping the anchor's style or trying new styles, so I can pick from real alternatives.

Variety comes from parallel attempts, different models and style changes.

## 2. Current state

- FACT: RD-3 (#572) provides `spawn_agents` with `{prompt, container_frames[]}` entries, prefetched context per agent, modes "split work", "side by side" and "keep iterating", enforced frame ownership, one undo block per agent run, and a UI that shows ownership and progress.
- FACT: T10.1 (#343) stores branch metadata and copies inside the file; T10.3 (#345) merges with id remap and per-layer conflict resolution; merged branches are archived, read-only and restorable (owner R5.19).
- FACT: E15-19 compiles a style into fixed collections; `VariableCollection.modes` is a list and `NodeVariables.modes` sets an explicit mode per node subtree (`crates/designos-doc/src/variables.rs:20-31,135-141`).
- FACT: E15-16 (#601) prefetches design context on an agent's first turn; E15-7 (#592) flags frames in progress.

## 3. Behaviour rules

1. **Call.** `designos_explore_variants({ brief, n, mode, placement, models, quick_action?, anchor_frame_id?, surface?, size? })`:
   - `n` ∈ 2…8; `placement` ∈ `frames | branches`; `mode` ∈ `same_style | new_style`;
   - `models`: one model name for all, or exactly `n` names (one per variant); each must map to a configured agent profile;
   - `quick_action` ∈ `layout | style | colour | typography | surprise` (optional; it picks the effective `mode` from the table in rule 5, overriding a `mode` argument, and also the style-assignment rule in rule 6. It is stored on the exploration, so `mode` stays a two-value enum).
2. **Anchor first.** Without `anchor_frame_id`, the first exploration on a brief builds one anchor screen with one agent (model = `models[0]`) in a frame named `<Brief title> / Anchor`; the call returns after spawning and the variants run when the anchor finishes (status `anchoring` → `exploring`). With `anchor_frame_id`, variants start at once and each agent's prompt includes the anchor's outline and screenshot (#238).
3. **Frames placement.** The engine creates N frames `<Brief title> / Variant 1…N`, the anchor's size (or `size`), placed in one row to the right of the anchor with a 160 px gap, top-aligned, all on the anchor's page. Each frame is tagged with the surface and flagged in progress (E15-7). RD-3 `spawn_agents` runs one agent per frame with `container_frames = [that frame]`, so cross-frame writes are refused (AC1).
4. **Branches placement.** The engine creates N branches `variant-<k>` from the current state (T10.1). Each agent edits the anchor frame inside its own branch (ownership = that frame within that branch). Main is not modified during exploration. AC2: each branch merges cleanly into a fresh main (no conflicts, because main did not change).
5. **Variation prompts** (our own text, versioned in `variants/prompts.md`): the brief, the anchor context, a fixed instruction to design one complete alternative inside the owned frame only, and a per-mode or per-quick-action direction:
   | Quick action | Effective `mode` | Direction in the prompt | Style assignment (rule 6) |
   |---|---|---|---|
   | none, `mode: same_style` / `layout` | `same_style` | keep the active style (`Document.applied_style`), change structure and composition | none |
   | none, `mode: new_style` / `style` | `new_style` | use the style parameters assigned by the engine | `full` |
   | `colour` | `new_style` | same layout; same archetype and fonts, a different palette option | `palette` |
   | `typography` | `new_style` | same layout; same archetype and palette, different font options | `fonts` |
   | `surprise` | `new_style` | new archetype and a different layout macrostructure (from the E15-21 landing or app structures) | `full` |

   Variant k also gets `variation_index = k` so prompts differ even on one model.
6. **Style assignment for `new_style`.** The engine picks parameters deterministically. The assignment kind comes from the rule 5 table. `full`: candidates are all (archetype, palette, fonts, radius, elevation) combinations whose archetype lists the brief's surface. `palette`: candidates are the palette options of the anchor's archetype, other parameters kept. `fonts`: candidates are the font-option combinations (heading, body, caption, data) of the anchor's archetype, palette and other parameters kept. (The type scale belongs to the archetype and is not a parameter, so a `typography` variant never changes it.) Exclude the anchor's combination; order candidates by `sha1(exploration_id ‖ archetype ‖ params)` and take the first N, with distinct archetypes when possible for `full` (distinct palettes otherwise), distinct palettes for `palette` and distinct font combinations for `fonts`. Fewer than N candidates: `variant_count_invalid` with the maximum (edge case 1). The assignment is stored and passed to each agent. AC3: the variables written differ per variant.
7. **Per-variant styles in frames placement.** For `new_style` in frames placement, E15-19's compiler writes each variant's style as additional modes on the four style collections, named `v<k> Light` / `v<k> Dark` (and `v<k>` for single-mode collections), and sets the variant frame's explicit modes (`NodeVariables.modes`). Variables keep their names, so agents still use `var:designos-style:color/color/primary`. In branches placement each branch simply applies its archetype normally.
8. **Stop.** `designos_stop_variants({ exploration_id })` cancels every running agent of the exploration (RD-3 stop), clears in-progress flags and keeps what each agent finished (each run is its own undo block; a run stopped mid-way is rolled back to its last completed batch per RD-3).
9. **Status.** `designos_variants_status({ exploration_id })` returns per variant: frame or branch, owner run, model, state (`queued | running | done | failed | stopped`), tool calls, wall time, last error.
10. **Concurrency.** At most `min(n, 4)` agents run at once by default (setting `variants.max_parallel`, 1–8); the rest queue.
11. **Storage.** `Document.explorations` records each exploration (brief, mode, placement, models, style assignments, variant records, status, created_at) so E15-37 can rank and merge later, even after reload. Each variant frame (or the anchor frame inside each branch) has `agent.variant = { exploration, index }` (SC-4).

## 4. Data model / schema

```rust
pub struct Exploration { pub id: String, pub brief: String, pub surface: Option<SurfaceKind>, pub mode: VariantMode, pub quick_action: Option<QuickAction>,
    pub placement: Placement, pub anchor: Option<NodeId>, pub variants: Vec<VariantRecord>, pub status: ExplorationStatus,
    pub created_at: String, pub prompt_version: String }
pub struct VariantRecord { pub index: u32, pub model: String, pub frame: Option<NodeId>, pub branch: Option<String>,
    pub run: Option<u32>, pub style: Option<StyleParamsWithArchetype>, pub state: VariantState,
    pub scores: Option<VariantScores> /* E15-37 */, pub archived: bool }
pub enum VariantMode { SameStyle, NewStyle }   pub enum Placement { Frames, Branches }
pub enum QuickAction { Layout, Style, Colour, Typography, Surprise }
// agent_meta.rs: pub variant: Option<VariantTag { exploration: String, index: u32 }>
```

Back-compat: `explorations` defaults to empty. Extra style modes are ordinary modes.

## 5. Contracts

- Input example: `{ "brief": "Pricing page for a team plan", "n": 3, "mode": "new_style", "placement": "frames", "models": ["claude-sonnet-5", "gpt-5.6-terra", "gemini-3.8-flash"], "surface": "landing-page" }`.
- Output: `{ "exploration_id": "x-2", "status": "anchoring" | "exploring", "anchor_frame_id": 501, "variants": [ { "index": 1, "frame_id": 610, "model": "claude-sonnet-5", "style": { "archetype": "quiet-editorial", "palette": "ink-on-paper", … } } … ] }`.
- Errors: `variant_count_invalid` (corrected example `n: 3`), `models_mismatch` (`models` length not 1 or n; corrected example repeats the first model), `model_not_configured` (details list configured profiles), `agent_not_configured`, `branches_unavailable` (document type without branches), `exploration_running` (a second exploration on the same anchor), `exploration_unknown`.

## 6. File-format mapping

`.designos` native. `.fig` export: variant frames export as frames; `explorations` and `agent.variant` are design:os-only; extra modes export as modes.

## 7. UI

The Studio UI for this tool is E15-38. While running, RD-3's ownership and progress UI shows which agent owns which variant frame.

## 8. Edge cases

1. Not enough distinct archetypes for the surface: distinct palettes or radius scales are used; if fewer than N distinct combinations exist (for `colour` and `typography`, within the anchor's archetype), `variant_count_invalid` with the maximum.
2. A model profile fails to start: that variant is `failed`; others continue; the status says why.
3. The user edits a variant frame while its agent runs: RD-3 ownership is for agents; a user edit is allowed and recorded as human-authored; the agent's next write sees the change.
4. Anchor frame deleted mid-exploration: running agents continue in their frames; ranking later skips anchor comparisons.
5. Frames placement where the row would overlap existing content: the row moves down below the lowest node right of the anchor, 160 px gap.
6. Branches placement on a document already on a branch: variants branch from the current branch; merge targets that branch.
7. Stop during anchoring: the anchor run is rolled back per RD-3; no variants start.

## 9. Non-goals

- Ranking and merging (E15-37); the UI (E15-38).
- Seeds or temperature control of models (variety comes from models, prompts and style changes).

## 10. Test plan

Seam: `cargo test -p designos-mcp variants` with stub agents (scripted MCP clients from E15-25's stub-agent mode used as agent commands).

Fail-before: the tool does not exist.

| Test | Input | Expected |
|---|---|---|
| `three_frames_three_agents_no_cross_writes` (AC1) | n=3 frames; stub k writes in its frame then tries frame k+1 | 3 frames, each authored by a distinct run; every cross-frame write refused; no frame holds nodes authored by another variant run |
| `branches_merge_cleanly` (AC2) | n=3 branches; stubs edit the anchor in each | 3 branches; merging each into a fresh copy of main reports 0 conflicts |
| `new_style_variants_differ` (AC3) | n=3 new_style frames | resolved `color/primary` (and fonts) per variant frame mode differ; stored assignments differ |
| `stop_cancels_all` | stop while 2 running | all states `stopped` or `done`; no in-progress flag left |
| `models_length_checked` | n=3, models of length 2 | `models_mismatch` |
| `quick_actions_assign_by_kind` | n=3 anchor in `quiet-editorial`; `colour`, then `typography` | `colour`: archetype and fonts equal the anchor's, palettes distinct; `typography`: archetype and palette equal the anchor's, font combinations distinct; both stored with mode `new_style` and the quick action |
| `assignment_is_deterministic` | same exploration id twice | identical style assignments |
| `max_parallel_respected` | n=6, max 4 | never more than 4 running (stub timestamps) |

Visual evidence: CPU render of the anchor plus three variant frames (contact sheet) from the stub run, and a live run capture in the PR.

## 11. Performance budgets

- Orchestration overhead per variant ≤ 200 ms (frame creation, ownership, prompt write).
- The UI thread never waits on agents; status updates arrive as document or live-patch events (CLAUDE.md §4: no periodic repaint when idle).

## 12. Dependencies and build order

- Needs #572, #343, #345, E15-19, E15-16 (#601), E15-7 (#592).
- Unblocks E15-37, E15-38.

## 13. Owner rulings that apply

- R4.13: every variant agent is the user's agent CLI; models can differ per agent.
- Parallel agents are hard-locked to their own frames (RD-3).
- Full scope: frames and branches, both modes, all quick actions.
