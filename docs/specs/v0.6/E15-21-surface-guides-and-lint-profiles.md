# E15-21 Encode surface guides as data and lint profiles: web app, landing page, mobile, slides, table, design system

- Issue: #609 (task E15-21, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `assets/surfaces/{web-app,landing-page,mobile,slides,table,design-system}.json` (new), `crates/designos-doc/src/lint/surface.rs` (new; the `lint` module comes from E15-9), `crates/designos-doc/src/agent_meta.rs` (new or extended, SC-4), `crates/designos-doc/src/spec.rs` (`surface` key), the `get_guidelines` module from #248, `engine.rs` tool list (`designos_set_surface`).

## 1. Summary and user story

As an agent, I want the guide for the surface I am building (prose) and the numeric rules it implies (lint), from one source, so advice and checks never disagree.

As a user, I want a slide with 18 px body text, or a mobile screen with six tab items, flagged the moment it is written.

## 2. Current state

- FACT: `designos_lint` has two rules, `zero-bounds` and `default-layer-name` (`crates/designos-mcp/src/engine.rs:350-374`). E15-9 (#594) replaces it with one solver-backed linter; E15-11 (#596) adds spacing and type-scale rules.
- FACT: nodes carry no surface or purpose tag. Available per-node maps are `plugin_data` and `fig_metadata` (`crates/designos-doc/src/node.rs:146,155`), both Figma-shaped; slides have `slide: Option<SlideProps>` with notes only (`node.rs:137,159-162`).
- FACT: `NodeSpec` has no `surface` key and does not reject unknown keys (no `deny_unknown_fields` on `NodeSpec`, `crates/designos-doc/src/spec.rs:44-46`). E15-48 makes `NodeSpec` strict in v0.6 (owner ruling 2026-10-19), so the `surface` key and its parser must land in one PR, after E15-48 in the `spec.rs` queue.
- Owner ruling 2026-10-19: the per-surface values in the table below are ours and accepted as written; they live in the surface data files so they can be tuned later without code changes.
- FACT: source knowledge (MIT, owner's own): `ease-design/knowledge/page-structures.md` (21 macrostructures, honest copy, chrome tells) and `knowledge/ux-psychology.md`.
- FACT: #248 asks `get_guidelines` to serve guides per surface: code, design system, landing page, mobile app, slides, table, tailwind, web app.

## 3. Behaviour rules

1. **One file per surface** holds `prose` (served verbatim by `get_guidelines`) and `rules` (compiled into the linter). AC3: both come from the same file.
2. **Surface tag.** A frame is tagged by `Node.agent.surface` (SC-4). Tag sources: `NodeSpec.surface` in `apply_spec`, `designos_set_surface`, or the slides document kind (every slide frame in a deck is implicitly `slides`). Only frames and sections can be tagged.
3. **Profile application.** When the linter runs on a page or node, each tagged frame is checked against its profile's rules in addition to the global rules. Untagged frames get no surface rules. Rule ids are `surface.<surface>.<rule>` (SC-3).
4. **Text roles.** Rules that speak of title, body and caption text resolve a text node's role in this order:
   1. the text style it uses: names matching `Display|H1|H2|H3|H4|Title|Heading` → `title`; `Body*` → `body`; `Caption|Label|Overline` → `caption`; `Data` → `data`;
   2. otherwise, in a `slides` frame: the text with the largest font size whose top is in the upper 35% of the frame is `title`; other text is `body` when it has ≥ 2 lines or ≥ 40 characters, else `caption`;
   3. otherwise: font size ≥ 1.5 × the frame's median text size → `title`; ≤ 0.85 × median → `caption`; else `body`.
5. **Rule kinds** (each with `severity: error | warning`):

| Kind | Parameters | Check |
|---|---|---|
| `min_font_size` | `role`, `px` | every text of that role has font size ≥ px (all runs) |
| `max_font_families` | `count` | distinct font families used in the frame ≤ count |
| `edge_margin` | `px` | every visible descendant's world bounds stay ≥ px from the frame edge; full-bleed backgrounds (covering ≥ 95% of the frame) and nodes named `Background` are exempt |
| `gap_range` | `scope: section | item`, `min`, `max` | auto-layout `item_spacing` of the content wrapper's direct children (section) or of nested lists (item) within range |
| `tab_bar_items` | `min`, `max` | the tab bar (§3.6) has min ≤ children ≤ max |
| `max_line_chars` | `role`, `chars` | measured characters per laid-out line ≤ chars (uses the text layout's line breaks) |
| `required_states` | `states[]` | for a frame holding a data region (§3.7), sibling frames or variants exist for each state |
| `title_size_consistent` | none | across tagged frames on the same page, title font sizes are equal |
| `frame_size` | `width`, `height` (either may be `null`) | frame size equals the value (slides 1920×1080) |
| `min_row_height` | `px` | direct children of the data region (§3.7), the rows, are at least px tall |

6. **Tab bar detection** (mobile): a horizontal auto-layout child whose bottom edge is within 40 px of the frame bottom, at least 75% of the frame width, with ≥ 2 children of equal width (±1 px); or any node whose name or main component name matches `/tab ?bar/i`. When several candidates match, the bottom-most one wins (largest world bottom edge; ties go to the lowest node id), and the finding names it.
7. **Data regions** (web app, table): a table component instance, a node named `/table|list|grid/i` with ≥ 3 repeated children (same main component or same structure), or a chart. State frames are found as sibling frames named `<Name> / <State>` or as variants of a component property named `State` with values `Loading`, `Empty`, `Error`, `Success`, `Permission` (case-insensitive).
8. **Profiles** (our own values, written for design:os; `px` at 1×, that is at the profile's design size; only `slides` declares a design size and scales, rule 11):

| Surface | Rules |
|---|---|
| `slides` | design size 1920×1080, scaled by width (rule 11); frame 16:9 (error); body ≥ 24 (error); title ≥ 40 (error); caption ≥ 18 (warning); edge margin ≥ 96 (error); max 2 font families (error); max line chars 60 body (warning) |
| `mobile` | frame width 390 or 393 or 430 (warning); body ≥ 15 (error); caption ≥ 11 (error); edge margin ≥ 16 (error); section gap 24–32 (warning); item gap 8–16 (warning); tab bar 3–5 items (error); title sizes consistent (warning) |
| `web-app` | body ≥ 13 (error); caption ≥ 11 (error); edge margin ≥ 16 (warning); section gap 24–48 (warning); required states loading/empty/error/success/permission (warning); title sizes consistent (warning) |
| `landing-page` | body ≥ 16 (error); caption ≥ 12 (error); max line chars 75 body (warning); edge margin ≥ 24 (error); section gap 64–160 (warning); max 3 font families (warning) |
| `table` | body ≥ 13 (error); caption ≥ 11 (error); required states empty/loading/error (warning); row height ≥ 32 (warning) |
| `design-system` | caption ≥ 11 (error); edge margin ≥ 40 (warning); max 3 font families (warning) |

9. **Prose.** Each `prose` is Markdown, 300–1,200 words, our own text built from the ease-design sources, with these sections: Purpose, Structure, Hierarchy, Spacing, Content, States, Checks (the Checks section lists every rule in plain words, generated from `rules` by a test so it cannot drift).
10. **Findings** use the E15-9 finding row with the rule id, node id, measured value and threshold, for example `surface.slides.body-min-size: Body text 18 px is below 24 px`. When a scale applies (rule 11) the threshold shown is the scaled one and the finding adds `scale`: `surface.slides.body-min-size: Body text 14 px is below 16 px (24 px × 0.667)`.
11. **Scaling by width (owner ruling 2026-10-19).** A profile may declare `"scale": { "by": "width", "design_width": 1920 }`; `slides` does, the other five do not (k = 1). For a tagged frame, `k = frame world width / design_width`, and every pixel parameter of its rules is multiplied by `k` before the check: `min_font_size.px`, `edge_margin.px`, `gap_range.min` and `max`, `min_row_height.px`. Counts and characters (`max_font_families`, `tab_bar_items`, `max_line_chars`) and `required_states` are not scaled. `frame_size` under a scale does not compare absolute size: it requires the frame's aspect ratio to equal `width : height` of the stated size, that is `|height − width × 1080/1920| ≤ 1 px`, so a 1280×720 deck passes and a 1280×800 deck fails. Comparisons allow 0.01 px of slack so float rounding in `k` cannot flip a result. A deck at exactly 1920×1080 has `k = 1` and behaves as before. Slide contract geometry and type ranges (E15-22) scale by the same `k`.

## 4. Data model / schema

```json
// assets/surfaces/slides.json
{ "schema": 1, "id": "slides", "name": "Slides",
  "scale": { "by": "width", "design_width": 1920 },
  "prose": "## Purpose\n…\n## Checks\n…",
  "rules": [
    { "id": "frame-size", "kind": "frame_size", "width": 1920, "height": 1080, "severity": "error" },
    { "id": "body-min-size", "kind": "min_font_size", "role": "body", "px": 24, "severity": "error" },
    { "id": "title-min-size", "kind": "min_font_size", "role": "title", "px": 40, "severity": "error" },
    { "id": "edge-margin", "kind": "edge_margin", "px": 96, "severity": "error" },
    { "id": "font-families", "kind": "max_font_families", "count": 2, "severity": "error" } ] }
```

```rust
// designos-doc/src/agent_meta.rs (SC-4; this task adds `surface`)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentMeta { #[serde(default, skip_serializing_if = "Option::is_none")] pub surface: Option<SurfaceKind> }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "kebab-case")]
pub enum SurfaceKind { WebApp, LandingPage, Mobile, Slides, Table, DesignSystem }
// node.rs: #[serde(default, skip_serializing_if = "AgentMeta::is_empty")] pub agent: AgentMeta,

// designos-doc/src/lint/surface.rs
pub struct SurfaceProfile { pub id: SurfaceKind, pub prose: String, pub rules: Vec<SurfaceRule>, pub scale: Option<SurfaceScale> }
pub struct SurfaceScale { pub by: ScaleBy /* Width */, pub design_width: f32 }
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SurfaceRuleKind { MinFontSize { role: TextRole, px: f32 }, MaxFontFamilies { count: u32 }, EdgeMargin { px: f32 },
    GapRange { scope: GapScope, min: f32, max: f32 }, TabBarItems { min: u32, max: u32 }, MaxLineChars { role: TextRole, chars: u32 },
    RequiredStates { states: Vec<DataState> }, TitleSizeConsistent, FrameSize { width: Option<f32>, height: Option<f32> },
    MinRowHeight { px: f32 } }
pub fn profiles() -> &'static [SurfaceProfile];      // include_str! + OnceLock
pub fn check_surface(doc: &Document, frame: NodeId, profile: &SurfaceProfile, out: &mut Vec<Finding>);
```

`NodeSpec` gains `#[serde(default, skip_serializing_if = "Option::is_none")] pub surface: Option<String>`; `export_spec` writes it back. Old documents load with `agent` empty.

## 5. Contracts

- `apply_spec`: `{ "kind": "frame", "name": "Deck / Cover", "surface": "slides", … }`. Invalid value → `surface_unknown`, details `{ "allowed": ["web-app","landing-page","mobile","slides","table","design-system"] }`, corrected example `{"surface":"slides"}`, `safe_to_retry: true`.
- `designos_set_surface` input `{ "node_id": 42, "surface": "mobile" | null }`; output `{ "node_id": 42, "surface": "mobile", "changed": true, "issues": [ … E15-12 block … ] }`. Errors: `surface_unknown`; `surface_target_invalid` when the node is not a frame or section (corrected example uses the nearest frame ancestor id from `details`).
- `designos_get_guidelines { "category": "guide", "topic": "slides" }` returns `{ "topic": "slides", "prose": "<file prose>", "rules": [ …the rules array… ] }`.
- `designos_lint` (E15-9) accepts `surface` as an override for an untagged frame: `{ "node_id": 42, "surface": "slides" }`.

## 6. File-format mapping

- `.designos`: `agent.surface` native.
- `.fig` export: not exported (design:os-only metadata). Import of a `.fig` leaves it empty.

## 7. UI

Studio shows the tag in the Inspect panel for a selected frame: a 28 px row `Surface` with a dropdown (`None`, the six surfaces). Changing it is an undoable edit and re-runs the lint for that frame. Findings appear wherever E15-9/E15-14 list lint findings.

## 8. Edge cases

1. Nested tagged frames (a `table` frame inside a `web-app` frame): each frame is checked against its own profile only; the inner frame's descendants are excluded from the outer frame's `edge_margin` and `min_font_size` checks.
2. Text with mixed run sizes: `min_font_size` uses the smallest run size; the finding names the run range.
3. Hidden nodes and nodes with opacity 0 are skipped.
4. Rotated nodes: `edge_margin` uses the rotated world bounds.
5. Instances: rules check the rendered (override-applied) values.
6. A slide in a deck with an explicit `surface: mobile` tag: the explicit tag wins over the implicit slides tag.
7. A frame with no text: text rules produce no findings, not errors.
8. Two tab bar candidates: the bottom-most wins (rule 6); the finding names it.
9. A deck mixing frame sizes: `k` is computed per frame; each slide is checked against its own scaled thresholds. `title_size_consistent` compares title size divided by `k`, so a 1280×720 and a 1920×1080 slide with proportional titles are consistent.

## 9. Non-goals

- Templates for each surface (E15-22).
- `code` and `tailwind` guides from #248: prose only, owned by #248, stored in `assets/guides/` by that task.
- Auto-fix of surface findings (E15-13 decides which kinds are safe; none of these is in its safe list by default).

## 10. Test plan

Seam: `cargo test -p designos-doc lint::surface` (unit tests per surface) and `cargo test -p designos-mcp surface` (contract).

Fail-before: `slide_with_18px_body_fails` fails on `main`.

| Test | Input | Expected |
|---|---|---|
| `slide_with_18px_body_fails` (AC1) | 1920×1080 frame tagged slides, title 48 px, body text style `Body` at 18 px | one error `surface.slides.body-min-size`, measured 18, threshold 24 |
| `slide_1280x720_proportional_passes` | 1280×720 frame tagged slides (k = 0.667), title 28 px, body 17 px, caption 13 px, content 70 px from every edge, one font family | no `surface.slides.*` finding (absolute 24 px body would have failed; scaled it is 16 px) |
| `slide_1280x720_below_scaled_threshold_fails` | same frame, body text 14 px | one error `surface.slides.body-min-size`, measured 14, threshold 16 with `scale` 0.667; the title, caption and margin checks stay silent |
| `slide_1280x800_fails_aspect` | 1280×800 frame tagged slides, otherwise proportionally correct | one error `surface.slides.frame-size` (height 800, expected 720 ± 1) |
| `slide_2560x1440_scales_up` | 2560×1440 frame (k = 1.333), body 24 px | error `surface.slides.body-min-size`, threshold 32; body 33 px passes |
| `non_slide_surface_is_not_scaled` | 390×844 frame tagged mobile, body 14 px | error `surface.mobile.body-min-size`, threshold 15 (k = 1) |
| `mobile_six_tabs_fails` (AC2) | 390×844 frame tagged mobile, bottom horizontal auto layout 390 wide with 6 equal children | error `surface.mobile.tab-bar-items`, measured 6, max 5 |
| `mobile_four_tabs_passes` | same with 4 children | no tab-bar finding |
| `guide_and_profile_share_one_file` (AC3) | `get_guidelines` slides prose and `profiles()` rules | prose bytes equal the file's `prose`; rules equal the file's `rules` |
| `checks_section_is_generated` | each file | the prose Checks section equals the text generated from `rules` |
| `edge_margin_ignores_full_bleed_background` | slide with a 1920×1080 background rect and text at 120 px | no edge finding |
| `role_from_text_style_beats_heuristic` | 18 px text using `H3` style in a slide | role title, finding from title-min-size not body |
| `untagged_frame_gets_no_surface_rules` | same slide without a tag | no `surface.*` findings |
| `unknown_surface_named_error` | `apply_spec` with `surface: "poster"` | `surface_unknown` with allowed list |

Visual evidence: terminal capture of the slide and mobile tests plus an Inspect panel capture at 1440 showing the Surface row.

## 11. Performance budgets

- Surface checks add ≤ 10% to E15-9's lint time for a tagged frame and nothing for untagged frames.
- `max_line_chars` reuses cached text layout; it never re-shapes text on the UI thread.
- CLAUDE.md §4: no `to_json` on the UI thread; tagging a frame is a document change (it bumps the revision once), never a viewport change.

## 12. Dependencies and build order

- Needs #248, E15-9 (#594), E15-11 (#596) and E15-48 (#638). E15-48 makes `NodeSpec` strict, so the `surface` key and its parser must merge after it, in the same PR as the key's entry in the strict key list (README-part2 `spec.rs` queue).
- Unblocks E15-22 (#610), E15-26 (`lint_per_100` includes `surface.*` findings), E15-34 (critique uses the surface checklist), E15-28 (briefs carry a surface tag).

## 13. Owner rulings that apply

- Full scope: all six surfaces with machine rules.
- Content written for design:os, using ease-design `page-structures` and `ux-psychology` as sources.
- Auto-fix stays configurable; surface findings are report-only unless E15-13's settings opt them in.
