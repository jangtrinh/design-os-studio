# E15-9 Unify design lint into one solver-backed geometric linter

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | #157 (E3-12, off-thread incremental scanner), #158 (E3-13, rule set, thresholds, dismiss/ignore) |
| Unblocks | E15-10, E15-11, E15-12, E15-13, E15-17, T7.2 extension, part-2 eval metrics |
| Touches | new `crates/designos-doc/src/lint/` (absorbs `lint_document` and `lint_fix_document`), new `crates/designos-render/src/lint_env.rs`, `crates/designos-mcp` (`designos_lint`), `crates/designos-app/src/check_designs/` (becomes a consumer) |

## 1. Summary and user story

As an agent, a designer in Check designs, or the eval harness, I get one set of findings from one
rule registry, computed from the solved layout and real text shaping, so "clipped text" means the
glyphs really do not fit, and the same document gives the same findings everywhere.

## 2. Current state (FACT)

Three separate linters exist today:

1. MCP `designos_lint`: two rules, `zero-bounds` and `default-layer-name`
   (`crates/designos-mcp/src/engine.rs:350-375`). It is dispatched but missing from the declared tool
   list (`engine.rs:705-829` has no `lint` entry).
2. designos-doc `lint_document`: 17 rules over `layout`, `tokens`, `Naming`, `accessibility`,
   `structure`, with presets (`rule_enabled`) — in the legacy compatibility module of
   `crates/designos-doc/src/` (`lint_document` at line 763, `rule_enabled` at line 750, scales at
   lines 671-674). Only its own unit test calls it. `lint_fix_document` (line 992) mutates the document
   directly without `History`, removes empty containers (lines 1026-1034, 1198-1200) and enlarges
   touch targets (lines 1135-1145).
3. Studio Check designs: three rules (hard-coded values, contrast, detached instances) over at most
   `MAX_SCANNED_LAYERS = 20_000` layers on the UI thread
   (`crates/designos-app/src/check_designs/mod.rs:30-52`, `audit` `mod.rs:113-138`); fixes are one
   undo step through `Tx` (`check_designs/fix.rs:26-50`).

None uses text layout results: the 17-rule overlap and overflow checks use raw `transform`/`size`
(line 854-895 of the legacy module); spec text sizes come from a heuristic
(`estimate_intrinsic_text_size`, `crates/designos-doc/src/spec.rs:773`).

Crate constraint: designos-render depends on designos-doc (`crates/designos-render/Cargo.toml:10`),
so designos-doc cannot call the text shaper. Text geometry is `Rc` and cached in a thread-local
(`crates/designos-render/src/text.rs:704-710`), so it must be computed on the thread that lints.

## 3. Behaviour rules

### 3.1 Architecture

- `designos_doc::lint` owns the registry, configuration, findings, traversal and every rule.
- Rules that need shaping or pixels ask a `LintEnv` trait:
  ```rust
  pub trait LintEnv {
      fn text_metrics(&self, node: &Node, props: &TextProps, size: Vec2) -> Option<TextMetrics>;
      fn backdrop(&self, doc: &Document, node: NodeId, rect: Rect) -> Option<Backdrop>; // E15-10
  }
  pub struct TextMetrics { pub line_count: u32, pub visible_lines: u32, pub content_height: f32,
                           pub widest_line: f32, pub breaks_inside_word: Vec<usize> }
  ```
- `designos_render::lint_env::RenderLintEnv` implements it with `text_geometry` and the CPU renderer;
  it is created on the linting thread. Without an env, rules that need it are listed in
  `skipped_rules` with reason `no_text_engine`; there is no heuristic fallback.
- Geometry comes from the solved document: callers settle layout first (`Engine::settle_layout`,
  `engine.rs:400-404`). The traversal carries the parent world affine down the tree, so page-space
  bounds are O(1) per node.

### 3.2 Rule registry

Each rule: `id`, `category` (`layout`, `text`, `tokens`, `components`, `content`, `naming`,
`accessibility`, `structure`), default `severity` (`error`, `warning`, `info`), `fixable`, `needs`
(`geometry`, `text`, `pixels`), and the presets that include it. Old ids stay as aliases so stored
ignore entries keep working.

New rules (thresholds in page units, 0.5 px tolerance unless stated):

| Id | Definition | Sev |
|---|---|---|
| `overflow-parent` | Parent does not clip; child's axis-aligned box in parent space exceeds `[0,w]x[0,h]` by > 0.5 on any side. Skips groups, boolean ops, pages, sections' own children. Alias `overflowing-children`. | warning |
| `clipped-partial` | Parent clips; child box intersects the parent box but is not contained (> 0.5 outside). | warning |
| `clipped-full` | Parent clips; child box and parent box do not intersect (child invisible). | error |
| `sibling-overlap` | Container without auto layout; two visible siblings (opacity > 0, not masks) whose boxes intersect with area ≥ 1 and neither contains the other. Sweep-and-prune on x. Alias `overlapping-siblings`. | warning |
| `text-truncated` | `TextMetrics.line_count > visible_lines`, or (fixed-size text) `content_height > node height + 0.5`. | error |
| `text-unexpected-wrap` | Single-line candidate (no `\n`, ≤ 40 chars, auto-resize not width-and-height) laid out on ≥ 2 lines; or any line break inside a word (`breaks_inside_word` non-empty). | warning |
| `collapsed-size` | Visible node with width or height < 0.5 (lines and vectors exempt on their thin axis); or auto layout hugging an axis with no visible children. Alias `zero-bounds`. | error |
| `sizing-conflict` | Child `Fill`/`Hug` sizing on an axis whose parent has no auto layout (sizing ignored); or child `Fill` on the primary axis of a parent that hugs that axis (circular). | warning |
| `text-missing-fill` | Visible text whose node fills and every run fill are empty, invisible or zero opacity (it paints nothing). | error |
| `absolute-in-auto-layout` | `layout_child.absolute == true` inside an auto-layout parent. | info |
| `placeholder-text` | Text (outside main components and sets) matching `lorem ipsum`, `dolor sit amet`, or exactly one of `Text, Label, Button, Title, Heading, Placeholder, Sample text, Your text here, Type something, TODO, TBD, XXX` (case-insensitive, trimmed). | warning |
| `detached-instance` | Moved from Check designs (`check_designs/detached.rs`): frame with `detached_symbol_id` whose component is present; fix offered only while it still equals the component. | info |

Existing 17 rules move unchanged in logic except that geometry uses solved world bounds and text
rules use `TextMetrics`: `default-layer-name`, `deeply-nested`, `empty-container`, `non-4px-grid`,
`inconsistent-spacing`, `unbalanced-padding`, `missing-auto-layout`, `misaligned-siblings`,
`inconsistent-radius`, `small-touch-target`, `non-scale-font-size`, `tiny-text`,
`detached-text-style`, `hardcoded-color`; `overlapping-siblings` and `overflowing-children` become
aliases (above); `low-contrast-text` becomes an alias of E15-10 `contrast`. The token and scale rules
are re-specified in E15-11 (`hardcoded-value`, `off-scale`); `hardcoded-color` stays only in the
`strict` preset.

### 3.3 Presets

| Preset | Rules |
|---|---|
| `agent` (MCP default) | overflow-parent, clipped-partial, clipped-full, text-truncated, text-unexpected-wrap, text-missing-fill, collapsed-size, sizing-conflict, absolute-in-auto-layout, placeholder-text (every rule this spec defines). Later specs append their rule to this preset in the PR that registers it: E15-10 `contrast`, E15-11 `hardcoded-value` and `off-scale`, E15-2 `slot-non-recommended` |
| `check-designs` (Studio panel default, Figma parity) | the #158 rule set this spec registers: detached-instance and the rules #158 adds. E15-10 appends `contrast` and E15-11 appends `hardcoded-value` in the PR that registers each |
| `recommended` | `agent` + detached-instance + default-layer-name + empty-container |
| `strict` | every rule |
| `layout`, `text`, `tokens`, `accessibility`, `naming`, `structure`, `components`, `content` | by category |
| comma list | explicit rule ids (current behaviour of `rule_enabled`) |

### 3.4 Findings

```rust
pub struct Finding {
    pub rule: &'static str, pub category: Category, pub severity: Severity,
    pub node: NodeId, pub node_name: String, pub xpath: String,
    pub bounds: Rect,               // page space
    pub message: String,
    pub suggested_fix: Option<FixSuggestion>,   // kind, property key, from, to
    pub fixable: bool,              // the engine can apply it (E15-13 decides the tier)
}
```

Order: severity (error, warning, info), then document paint order.

### 3.5 Dismiss and ignore (shared with #158)

- Ignore is persistent in the document: `Document.lint_ignores: BTreeMap<String, BTreeSet<NodeId>>`
  (rule id → nodes). If #158 has merged first with its own field, this task uses that field instead;
  only one store exists.
- Dismiss is per session (Studio panel / MCP session), not saved.
- Ignored findings are filtered after evaluation; `include_ignored: true` returns them with
  `ignored: true`. Aliases resolve to the canonical id before lookup.

### 3.6 Incremental and off-thread

- `lint_subtree(doc, roots, &LintConfig, env, &mut Budget) -> LintPage` evaluates the given roots
  only; `Budget` caps nodes and wall time and returns a continuation cursor (node id stack) when it
  runs out.
- The #157 off-thread scanner drives full-page scans by repeated `lint_subtree` calls on a worker
  thread holding a document snapshot of `(page, revision)`; the worker creates its own
  `RenderLintEnv`.
- Instance contents are linted on the instance (mirrors of main components are skipped as today,
  `check_designs/mod.rs:124`), except geometric rules, which apply to the instance's own box.

## 4. Data model / schema changes

- New module tree `lint/{mod.rs, registry.rs, config.rs, finding.rs, traverse.rs, rules/*.rs,
  fix.rs}`; the legacy module keeps thin wrappers `lint_document`/`lint_fix_document` that call the new
  code for one release, then they are removed (no other caller exists).
- `Document.lint_ignores` (serde default, skip when empty) unless #158 provides it.
- `lint_fix_document` behaviour moves to `lint/fix.rs` (E15-13 rewrites it to return `Command`s; this
  task does not change its semantics yet but routes it through `History` in the MCP path).

## 5. Contracts

Tool `designos_lint` (now declared; `readOnlyHint:true`):

```json
{"type":"object","additionalProperties":false,"properties":{
  "page_id":{"type":"integer","minimum":1},
  "node_ids":{"type":"array","items":{"type":"integer","minimum":1},"maxItems":500},
  "preset":{"type":"string","default":"agent"},
  "rules":{"type":"array","items":{"type":"string"}},
  "include_ignored":{"type":"boolean","default":false},
  "limit":{"type":"integer","minimum":1,"maximum":500,"default":100},
  "cursor":{"type":"string"}}}
```

Output:

```json
{"findings":[{"rule":"text-truncated","category":"text","severity":"error","node_id":88,
  "node_name":"Plan title","xpath":"/PAGE[1]/FRAME[@name='Pricing']/TEXT[2]",
  "bounds":{"x":24,"y":96,"width":180,"height":20},
  "message":"Text needs 2 lines but shows 1; 'Enterprise annual plan' is cut.",
  "suggested_fix":{"kind":"set","property":"textAutoResize","from":"NONE","to":"HEIGHT"},
  "fixable":true}],
 "total":14,"by_severity":{"error":3,"warning":9,"info":2},"by_rule":{"text-truncated":1},
 "skipped_rules":[],"scanned":1840,"complete":true,"elapsed_ms":11,"cursor":null}
```

Errors: `UNKNOWN_RULE` / `UNKNOWN_PRESET` (`valid_options` = ids / presets), `NODE_NOT_FOUND`,
`EXACTLY_ONE_SCOPE` (both page_id and node_ids). `safe_to_retry: true` (read-only).

## 6. File-format mapping

`lint_ignores` is native-only; on `.fig` export it is written to document plugin data
`designos:lint_ignores` so a round trip keeps it (nothing vanishes silently).

## 7. UI

Check designs keeps its panel (`crates/designos-app/src/panels/check_designs.rs`) and its current
grouping; it now gets findings from the registry with preset `check-designs`. Until E15-10 and E15-11
land, the panel's hard-coded-value and contrast groups keep their existing code paths (E15-11 3.4
deletes the app-side matcher), so no group disappears from the panel. Rows for new rules use
the same row layout (`ROW_H = 60`, `panels/check_designs.rs:20`). A preset selector is added by #158
or E15-14, not here. Zero fake UI: no control without a working action.

## 8. Edge cases

1. Rotated child: overflow and clipping use the axis-aligned box of the rotated rect (conservative);
   message says "rotated".
2. Text with zero width and auto-resize width-and-height: shaping gives the natural width; not
   truncated.
3. Mask layers: excluded from overlap; clipping by a mask is not reported as `clipped-*` (masks are
   intentional).
4. Hidden ancestors: hidden subtrees are skipped entirely (as both current linters do).
5. Missing fonts: shaping uses the same fallback as rendering, so findings match what the user sees;
   the finding message notes the substituted family.
6. Very wide container with 10,000 siblings: overlap uses sweep-and-prune, O(n log n + k); k capped
   at 1,000 reported pairs per container with a `truncated` note.
7. Negative item spacing in auto layout: overlap is intentional there and not reported.

## 9. Non-goals

- Contrast algorithm details (E15-10), token/scale rules (E15-11), the per-call block (E15-12), the
  safe-fix tiers (E15-13), the settings UI (E15-14).
- New Check designs panel controls.

## 10. Test plan

Seam: designos-doc unit tests per rule (with `RenderLintEnv` from designos-render in an integration
test crate `crates/designos-render/tests/lint_rules.rs`, since designos-doc cannot depend on the
renderer), plus a perf test on the large fixture.

Per rule, two fixtures built in code: `fail_<rule>` and `clean_<rule>`. Examples:

| Rule | Fail fixture | Clean fixture |
|---|---|---|
| `text-truncated` | text "Enterprise annual plan", Inter 16, fixed 120x20 | same with auto-resize height |
| `clipped-full` | clipping frame 100x100, child at (150,0) 20x20 | child at (10,10) |
| `sibling-overlap` | two 50x50 rects at (0,0) and (25,25) in a plain frame | background 100x100 containing a 20x20 |
| `sizing-conflict` | child width Fill inside a frame without auto layout | same inside an auto-layout row |
| `text-unexpected-wrap` | "Subscribe now" in a 60 px wide fixed text | 120 px wide |
| `placeholder-text` | text "Lorem ipsum dolor" | "Plan your week" |

Also:
- `presets_name_only_registered_rules`: every rule id named by a built-in preset exists in the registry
  at that commit (a preset never names a rule a later spec adds; `UNKNOWN_RULE` cannot fire on a
  built-in preset). Each later spec's PR extends the preset and this test together.
- `mcp_lint_returns_full_rule_set`: `designos_lint` with `preset:"strict"` lists every registry id in
  `by_rule` keys for a fixture that trips all rules; the old two-rule shape (`passed` field) is gone.
- `ignored_never_reappears`: ignore `(text-truncated, 88)`, re-lint twice and after save/load: absent.
- `same_findings_three_callers`: MCP, Check designs (preset check-designs) and a direct library call
  return identical findings for the shared rules on one fixture.
- `large_fixture_off_thread_budget`: synthetic large fixture (README) linted by the scanner on a
  worker thread; the UI-thread simulation records no frame over 8 ms; total time recorded.

Fail-before: `text-truncated` fail fixture produces no finding today (no such rule).

## 11. Performance budgets

- Per-call subtree lint (≤ 2,000 nodes, warm shaping cache): ≤ 25 ms release.
- Full scan of the large fixture (112,474 scene nodes; CI synthetic equivalent): off the UI thread;
  target ≤ 6 s including first-time shaping, recorded in the PR; no UI frame over 8 ms while it runs
  (#157 budget).
- No `to_json`, no whole-document clone in the lint path; the worker holds an `Arc` snapshot provided
  by #157.

## 12. Dependencies and build order

After #157 and #158. First in the lint track; E15-10 and E15-11 add rules to the registry; E15-12 and
E15-17 call `lint_subtree`.

## 13. Owner rulings

- Full scope, never MVP: all new rules plus the 17 existing and the 3 Check designs rules in one
  registry.
- Auto-fix (2026-10-18) is E15-13; this task only marks `fixable`.
- The eval never uses client files: the perf test uses the synthetic generator in CI; the local
  benchmark file is never committed or used by the eval.
