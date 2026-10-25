# v0.6 AI design: task specs, part 2 (E15-18 … E15-48)

Pack goal: **agents that design well through the user's own agent CLI, and a harness that proves it.**
- Styles are data: archetypes with typed parameters compile straight into variables, modes and styles.
- Surfaces are data: one file per surface feeds both the agent guide and the linter, plus verified templates.
- `designos-eval` measures quality offline, deterministic metrics first, and gates CI on them.
- Critique is structured and never done by the generator; repairs are scoped and guarded.
- Variants run in parallel, locked to their frames or branches, and are ranked and merged.
- Assets (images, stock, SVG) come from the user's agent CLI. No model is bundled.
- Tokens round-trip between code and design; generated code is verified by rendering it.
- Skills: the MIT `jakubkrehel/skills` set (attributed) plus our own pack built from the ease-design knowledge.

Milestone: v0.6 AI design. Epic: #585 (E15 AI design). Part 1 (E15-1 … E15-17 and the pulled-forward tasks) is written separately; the two indexes are merged later.

These specs were written against `main` at `767acf9` on 2026-10-18. Before coding, re-anchor the spec against the current `main` and report any drift, especially against the part 1 tasks that land first.

## Index

| Task | Issue | Title | Spec | Size |
|---|---|---|---|---|
| E15-18 | #604 | Style archetypes with typed parameters and our own catalogue | [E15-18](E15-18-style-archetype-catalogue.md) | L |
| E15-19 | #606 | Compile an archetype into variables, modes and styles (`designos_apply_style`) | [E15-19](E15-19-apply-style-compiler.md) | L |
| E15-20 | #608 | Style preset gallery rendered by our own renderer | [E15-20](E15-20-style-preset-gallery.md) | M |
| E15-21 | #609 | Surface guides as data and lint profiles | [E15-21](E15-21-surface-guides-and-lint-profiles.md) | M |
| E15-22 | #610 | Surface templates and slide layout contracts | [E15-22](E15-22-surface-templates-and-slide-contracts.md) | L |
| E15-23 | #611 | Licence register for eval data, model weights and skills | [E15-23](E15-23-eval-and-skills-licence-register.md) | S |
| E15-24 | #612 | Large deterministic test documents from open kits | [E15-24](E15-24-deterministic-large-document-generator.md) | M |
| E15-25 | #613 | `designos-eval` harness | [E15-25](E15-25-designos-eval-harness.md) | L |
| E15-26 | #614 | Deterministic eval metrics | [E15-26](E15-26-deterministic-eval-metrics.md) | M |
| E15-27 | #616 | Suite S1 recreate, fidelity metrics | [E15-27](E15-27-suite-s1-recreate-fidelity.md) | L |
| E15-28 | #617 | Suite S2 brief-to-screen on two open design systems | [E15-28](E15-28-suite-s2-brief-to-screen.md) | M |
| E15-29 | #618 | Suite S3 edits on large generated files, drift metric | [E15-29](E15-29-suite-s3-large-file-edits-drift.md) | M |
| E15-30 | #619 | Suite S4 multi-screen flows, consistency metric | [E15-30](E15-30-suite-s4-flow-consistency.md) | M |
| E15-31 | #620 | Checklist and pairwise judge, local aesthetic score | [E15-31](E15-31-checklist-pairwise-judge.md) | L |
| E15-32 | #621 | Suite S5 judge calibration | [E15-32](E15-32-suite-s5-judge-calibration.md) | M |
| E15-33 | #622 | CI gate on deterministic eval metrics | [E15-33](E15-33-ci-eval-regression-gate.md) | M |
| E15-34 | #623 | Rubric critique with boxes, never by the generator | [E15-34](E15-34-rubric-critique.md) | L |
| E15-35 | #624 | Region-scoped repair with a regression guard | [E15-35](E15-35-scoped-repair-regression-guard.md) | L |
| E15-36 | #625 | Variant explorer in frames or branches | [E15-36](E15-36-variant-explorer.md) | L |
| E15-37 | #626 | Rank variants and merge the winner | [E15-37](E15-37-rank-and-merge-variants.md) | M |
| E15-38 | #627 | Variant explorer UI | [E15-38](E15-38-variant-explorer-ui.md) | M |
| E15-39 | #628 | Generate images through the agent CLI | [E15-39](E15-39-generate-image-assets.md) | L |
| E15-40 | #629 | Stock images with attribution | [E15-40](E15-40-stock-images-attribution.md) | M |
| E15-41 | #631 | Stream generated SVG into frames | [E15-41](E15-41-stream-generated-svg.md) | L |
| E15-42 | #632 | Import globals.css and Tailwind themes as variables | [E15-42](E15-42-import-css-tailwind-tokens.md) | L |
| E15-43 | #633 | CSS custom properties to tokens on web capture | [E15-43](E15-43-web-capture-css-tokens.md) | M |
| E15-44 | #634 | Repeated subtrees to components on web import | [E15-44](E15-44-web-import-component-detection.md) | L |
| E15-45 | #635 | Verify generated code by rendering and pixel diff | [E15-45](E15-45-verify-generated-code.md) | L |
| E15-46 | #636 | Adopt MIT `jakubkrehel/skills` as canvas-checkable rules | [E15-46](E15-46-adopt-mit-ui-skills.md) | M |
| E15-47 | #637 | Our own design:os skills pack | [E15-47](E15-47-designos-skills-pack.md) | L |
| E15-48 | #638 | Image fills and effects in `apply_spec` | [E15-48](E15-48-apply-spec-image-fills-effects.md) | M |

Issue numbers for E15-23 … E15-48 were filed after the specs were first written and are filled in the index above.

## Shared contracts used by these specs

Each is owned by one task. Others reference it and never redefine it.

- **SC-1 Error envelope** (owner: E15-5, #590). Every named error in these specs is returned as an MCP tool result with `isError: true` and `structuredContent` of the shape `{"name", "message", "safe_to_retry", "corrected_example", "details"}`. Error names are `snake_case`. If E15-5 lands different field spellings, E15-5 wins and these specs follow by rename only.
- **SC-2 Variable references** (owner: E15-3, #588; the exact resolution order is E15-3 §3.1). Three forms. Qualified: `"var:<collection id or name>/<variable name>"`, for example `"var:designos-style:color/color/primary"` (collection id `designos-style:color`, variable `color/primary`); a hit ends the search and a same-named variable in another collection never matches. Unqualified: `"var:<variable name>"`, for example `"var:color/primary"`, valid only while exactly one collection holds that name (two collections: `AMBIGUOUS_VARIABLE`). By id: `"var:id:<variable id>"`. Order: qualified, then unqualified, then library. Everything E15-19 compiles is referenced in the qualified form.
- **SC-3 Lint findings** (owner: E15-9, #594; per-call block E15-12, #597). Rule ids come from the unified rule registry. These specs add the `surface.*` family (E15-21) and consume rule ids by registry lookup, never by string literal outside a mapping table that a test checks against the registry.
- **SC-4 Agent identity and node agent metadata** (owner: E15-34 for the struct; RD-3, #572, for run identity). `Node.agent: AgentMeta` and `Document.agent_runs`. Whichever of E15-21, E15-34, E15-39 lands first creates the struct with only its own fields; later tasks add theirs. If E15-7 (#592) already created an equivalent struct for the in-progress flag, extend that one instead.
- **SC-5 Agent jobs** (owner: E15-39 for the job contract, code in `designos-eval/src/agent_job.rs`; E5-10, #177, for the Studio runtime bridge). One `AgentJob` model (prompt file in, artefact out, timeout, cancel) used by assets (E15-39/40/41), the judge (E15-31), critique helpers and variants (E15-36).
- **SC-6 Colour space.** Owner rule: colour is Display P3 per document. Today `Color { r, g, b, a }` carries no profile (`crates/designos-doc/src/paint.rs:6-11`) and `designos-doc` has no colour-space field. Every data file in these specs declares `"space": "srgb" | "display-p3"` for its colours. Values are converted into the document's profile on write. Contrast always uses the E15-10 (#595) math. The document colour profile is owned by E1-7 (#136, pack v0.8). Until it lands, every document is sRGB, so values convert into sRGB on write; no task in this pack adds its own colour-space field to `Document`.
- **SC-7 Determinism helpers** (owner: E15-25). `designos-eval` owns a seeded xoshiro256** PRNG (SplitMix64 seeding, no `rand` dependency; none is in `Cargo.lock` today) and SHA-1 content hashes via `sha1_smol` (BSD-3-Clause, no dependencies). SHA-1 is also Figma's image hash, so E15-39/E15-48 use the same helper for image keys.

## `designos-eval` crate map (new; E15-25 creates it)

```
crates/designos-eval/
  Cargo.toml              features: default = [], "aesthetic" (CLIP/UIClip inference; never enabled by designos-mcp)
  src/main.rs             bin `designos-eval` (CLI, E15-25)
  src/lib.rs              task, runner, stats, jsonl, split, rng, hash, agent_job (SC-5, shared with designos-mcp and designos-app)
  src/gen/                large-document generator and kit definitions (E15-24)
  src/metrics/            deterministic.rs (E15-26), fidelity.rs (E15-27), drift.rs (E15-29), consistency.rs (E15-30)
  src/judge/              checklist.rs, pairwise.rs, bradley_terry.rs, aesthetic.rs (E15-31; shared with E15-37)
  suites/s1 … s5/         task files, split.lock, checklists (E15-27 … E15-32)
  baselines/              release baseline JSONL (E15-33)
```

`designos-mcp` depends on `designos-eval` with `default-features = false` (judge and metrics only). `cargo tree -p designos-app` and `cargo tree -p designos-mcp` must never list an ML inference crate.

## Build order

1. **Foundations, no part 1 dependency:** E15-23 → E15-25. E15-46 can start right after E15-23.
2. **Styles:** E15-18 (needs E15-10, #248) → E15-19 (needs #131, #138, E15-3, E15-4) → E15-20 (needs E15-3 for the sample screens).
3. **Spec grounding:** E15-48 right after E15-1 (templates and assets use image fills).
4. **Surfaces:** E15-21 (needs E15-9, E15-11, E15-48, #248) → E15-22 (needs E15-1/2/3, E15-19).
5. **Eval layers:** E15-26 (needs E15-9/10/11, E15-21) → E15-24 (needs E15-1/3, E15-26) → E15-27, E15-28, E15-31 in parallel → E15-29 (needs E15-17) → E15-30 → E15-32 → E15-33.
6. **Quality loop:** E15-34 (needs #572, #238, E15-7, E15-9, E15-21, E15-31) → E15-35 → E15-36 (needs #343, #345, E15-16, E15-19) → E15-37 → E15-38.
7. **Assets:** E15-39 (needs #177, E15-7, E15-18) → E15-40 and E15-41 in parallel.
8. **Code and design:** E15-42 (needs #138) → E15-43 (needs #276, #277, E15-11); E15-44 (needs #277, E15-1); E15-45 (needs #250, #276).
9. **Skills:** E15-47 last (needs #573, E15-15, E15-34).

Dependencies that are other v0.6 tasks and whose code does not exist on `main` yet: #248 (design-system rules prompt), #276 (web capture script), #277 (web capture import), #250 (HTML codegen). A task that needs one of them starts only after it merges; it never stubs the missing module.

Shared files, merge one at a time in this order: `crates/designos-doc/src/spec.rs` (E15-48 → E15-21 `surface` key), `crates/designos-mcp/src/engine.rs` tool list (E15-19 → E15-21 → E15-34 → E15-35 → E15-36 → E15-37 → E15-39 → E15-42 → E15-45), `crates/designos-doc/src/node.rs` `AgentMeta` (SC-4 rule above).

## Owner rulings that apply to every spec here

- Full scope, never an MVP. Packs only order the work.
- All AI goes through the user's agent CLI (R4.13). No model is bundled in the app.
- Auto-fix makes safe fixes with a diff report and is configurable (E15-13/E15-14).
- Skills: adopt the MIT `jakubkrehel/skills` with attribution, plus the design:os CLI skill and the ease-design knowledge set (`/Users/jang/Products/ease-design`: `knowledge/`, `prompts/`, its scoring scripts); write our own where needed; skip anything without a licence.
- The eval never uses client files (no VSF-PCP). Open kits and self-generated large files only.
- Parallel agents are hard-locked to their frames (RD-3, #572).
- Colour is Display P3 per document.
- Variables JSON is two separate formats (owner ruling 2026-10-19, #138): `figma-native` exactly as Figma writes it, and strict W3C DTCG 2025.10 with our metadata only under `org.designos`. Never a hybrid.

## Owner answers (2026-10-19)

| Question | Answer | Recorded in |
|---|---|---|
| Archetype catalogue | All 26 ease-design personas ship now as `candidate`; the owner's blind review later removes or reworks misses | E15-18 |
| Open kits for S2 | shadcn/ui (MIT) + IBM Carbon (Apache-2.0) | E15-23, E15-28 |
| Judge trust and voting | Thresholds as proposed (owner ≥ 0.70, UICrit ≥ 0.65); the whole evaluation experience lives in Studio, so voting and the trust table are a Studio Eval view, not an offline HTML file | E15-32 |
| Surface rule values | Accepted as written, stored as data so they can be tuned later | E15-21 |
| CI eval gate | Required for merge after three consecutive green runs | E15-33 |
| Licences for Design2Code, UICrit/RICO, UIClip | Approved as `eval-only-cache`: downloaded at run time, never committed or shipped | E15-23 |
| Unknown NodeSpec keys | Rejected from v0.6 at every level, no warning release | E15-48, E15-21 |
| Pending fill | Canvas overlay drawn by Studio, never a paint in the file | E15-39 |

The thresholds were not objected to; the owner's answer to that question was about where voting happens. If the owner wants different thresholds, only `judge-trust.toml` changes.
