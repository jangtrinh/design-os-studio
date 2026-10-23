# #897 Summarize how a file uses variables, radii, spacing and fonts (designos_get_app_state)

| Field | Value |
|---|---|
| Issue | #897 (study of the Figma plugin, 2026-10-22; "do now" list; Size S/M, label enhancement) |
| Size | M (one to two days: a pure walk with ten counting rules, a renderer, the tool surface and generated docs; the issue says S/M) |
| Depends on | none |
| Unblocks | nothing hard. #899 (library audit) can reuse the walk's per-component instance tally (INFERENCE: its spec is not written yet) |
| Touches | new `crates/designos-doc/src/usage.rs` (+ `pub mod usage;` in `lib.rs`), new `crates/designos-export/src/agent_context_usage.rs`, `crates/designos-export/src/lib.rs` (one `mod` line), `crates/designos-export/src/agent_context.rs` (one cap constant, the opt-in gate in `bootstrap`, a shared `section_targets` helper, `node_has_scoped_shell_marker` and `truncate_section_by_lines` made `pub(crate)`), `crates/designos-mcp/src/tools/bootstrap.rs` (section list, schema enum, description), generated: `crates/designos-mcp/tests/snapshots/tools.json`, `README.md`, `CLAUDE.md`, `skills/design-os-studio/SKILL.md`, `docs/mcp-parity.md`. Hot files: `bootstrap.rs` is shared with #900 (INFERENCE: its issue cites `bootstrap.rs:61-68`); the generated tables are shared with #896, #898, #899, #900, #901. |

Code baseline: `origin/main@42a0d027` (0.6.0). Evidence: Figma plugin `jangtrinh/design-os-figma-plugin@7c1ef8bb`, `cli/src/commands/scan-conventions.ts` (read in full); Studio files read in the worktree this session. Re-anchor every cited line against HEAD and report drift before writing code. No plugin code is copied; the walk is rewritten in Rust on Studio's model.

## 1. Summary and user story

As an agent about to build into an existing file, I can call `designos_get_app_state {"sections":["usage"]}` and get one compact block (about 230 tokens, at most 600) that says how the file is really built: how much of it uses variables and styles, auto layout against absolute frames, the components it reuses most, the radius, gap and padding values it uses, and its fonts. A walk that did not cover everything says so on its first line and in `truncated`. I no longer dump the outline to learn the conventions.

## 2. Current state (FACT)

- The tool lists seven valid sections plus `freshness` (`tools/bootstrap.rs:13`); the schema enum omits `freshness` (`:48-53`); an unknown section is `VALUE_OUT_OF_RANGE` (`:117-123`). The tool only parses arguments and calls `designos_export::agent_context::bootstrap` (`:136-151`), then copies each section's `id`, `title`, `tokens`, `truncated`, optional `truncation_note` and `content` (`:153-168`).
- The block is built in `designos-export`, not in the MCP crate (`agent_context.rs:800-895`; the comment at `:797-799` says the crate cannot depend on `designos-mcp`). `want_section` is `true` for every id when `sections` is `None` (`:805-810`). Caps: 60, 300, 2,500, 1,200, 1,200, 200, 6,000, selection 300 (`:31-39`); the caps scale down when `max_tokens` < 12,000 and floor at 120 (`:813-819`); `enforce_total_budget` trims only rules, outline, variables, components (`:1555-1587`, list at `:1562`). Truncation convention: a `[truncated: <n> more ...; call designos_get_app_state {...}]` marker, never a silent cut (`:1069-1081`, `:1495-1535`; E15-16 §3.1 "Never a silent cut").
- Two callers, both with every field of `BootstrapOptions` written out: the tool (`bootstrap.rs:136-149`, `sections: parsed.sections`) and Studio's first-turn job (`terminal.rs:2125-2138`, `sections: None`), which runs on a background thread over `doc.clone_without_images()` (`:2152-2163`, `:2327`, `:2413`). Tests pin seven default sections (`agent_context.rs:1691-1697`) and tool equals Studio block (`bootstrap.rs:221-253`).
- The editor does not show the block: `bootstrap_markdown` is read only inside `terminal.rs` (`:1113`, `:2198`) and reaches the agent through hook reply files (`:1322`); no other file in `designos-app/src` reads it (grep, this session). A new default-off section therefore changes nothing in Studio.
- Storage the walk reads: `Node.fills`, `Node.stroke.paints`, text colour in `TextStyle.fills` (not in `Node.fills`) (`node.rs:90-91`, `paint.rs:266-277`, `:464-465`); variable bindings in `Node.variables.bindings` keyed `fills/<i>`, `strokes/<i>`, `itemSpacing`, `paddingTop`..., `topLeftRadius`... (`variables.rs:1-6`, `:162-168`); style links in `Node.style_refs` (`node.rs:108-110`); auto layout in `FrameProps.auto_layout` (`node.rs:709-717`, `:913-943`); text runs in `TextProps.runs` (`:644-649`, `paint.rs:543-547`).
- The linter already defines "bound": a visible solid paint is bound when `<fills|strokes>/<i>` is in `bindings` or `style_refs` has `fill`/`stroke` (`lint/rules/scale.rs:689-710`; `Slot::paints` returns text colour for text nodes, `token_match.rs:440-480`); spacing, radius and font-size applicability and exclusions live in `number_value_at` (`token_match.rs:530-619`) and `PADDING_KEYS`/`RADIUS_KEYS` (`:20-21`); the lint treats an instance layer's inherited value as the main component's (`number_inherited`, `:623-626`). `lint::token_match` is a `pub mod` (`lint/mod.rs:11`).
- `NodeKind::Instance` children are "the resolved (overridden) copy" (`node.rs:486`), so descending into an instance re-walks the component's layers once per instance. `Document.nodes` is a `BTreeMap` (`lib.rs:179`, `get` `:991`); the only existing walk is recursive and unbudgeted (`Document::walk`, `:1237`).
- Component naming already exists for the outline: an instance's family is the parent `ComponentSet`'s name, else the component's (`agent_context.rs:261-281`).
- **Scoped copy (root cause of the issue's blind spot).** A session attached to Studio gets a scoped copy once the file has more than 25,000 nodes (`terminal.rs:2372`, `:2386`): the current page, at most 128 top-level frames as empty shells carrying `designos:scoped_shell`, selected frames in full up to 5,000 layers (`live_canvas_scope.rs:13-15`, `:27`, `:39-46`); frames past the first 128 are not listed at all. The helper that recognises a shell is private (`agent_context.rs:309-313`). The MCP reloads that copy as `engine.doc` (`server.rs:342-344`, `:416`).
- A headless MCP that calls `designos_open_file` holds the whole file, up to 1,200,000 layers and 256 nesting levels (`server.rs:1239-1253`), unscoped (`engine.rs:144`, `server.rs:869-875`): this is the 112k-node stress path.
- Plugin `scan-conventions.ts` (FACT, read in full): one summary per section, node budget 14,000 per section (`:30`), a stack walk that stops at the budget (`:77`, `:80`) and reports `truncated: visited >= budget` (`:92`), tokenized-fill % from SOLID paints only (`:86`), auto-layout against raw frames (`:82`), components keyed by the instance layer name (`:84`), radius and gap histograms, fonts by family only (`:88`). It walks instance children (Figma exposes them) and does not count strokes, padding, font size or weight; the issue asks for those.
- Studio's eval has a coarse coverage count (`designos-eval/src/runner.rs:171-191`) and a spec for a finer one, E15-26 rule 2 (bindable property bound by a variable or style; instances count through their own overrides only) (`docs/specs/v0.6/E15-26-deterministic-eval-metrics.md:23-29`).
- INFERENCE: one visit costs about 0.3 to 0.6 microseconds (a `BTreeMap` lookup among 112k entries plus small-map probes), so 50,000 nodes take about 15 to 30 ms. Not measured; no build was run.
- UNKNOWN: the real time on the benchmark file. Closing check: section 10, test 23 and the manual capture.
- UNKNOWN: whether selecting a frame in Studio and calling again returns that frame loaded within a second. Closing check: manual run on a file over 25,000 nodes (the mechanism is `selected_top_level`, `live_canvas_scope.rs:66-80`).

## 3. Behaviour rules

1. **Opt-in.** `usage` is built only when `options.sections` lists it by name (`sections.as_ref().is_some_and(..)`), never through `want_section` (which is true for `None`). `sections: None` still returns the seven sections and no `usage`; Studio's first turn and `compact_bootstrap` are unchanged. It is appended after `rules`, before `selection`; id `usage`, title `Usage` (unnumbered, like `selection`).
2. **Scope.** Roots are the explicit `target_node_ids`, else the current page's children, via one helper `section_targets` extracted from `build_outline_section` (`agent_context.rs:976-982`) and used by both. The canvas selection never sets the scope (#698). A `Page` root expands to its children; a root whose ancestor is also a root is dropped (no double count).
3. **Walk.** Explicit-stack pre-order in child order (no recursion), over the nodes of every root. Every visited node counts in `nodes_walked`, hidden ones included (the outline includes hidden nodes too). A `Page` node is not counted.
4. **Instances.** An instance root counts once in the component tally and nothing else: its own paints, radii and spacing, and all its children, are component-owned and are not walked (E15-26 counts inherited values once, on the main component; this rule differs from E15-26 only in not counting overrides, open question 3). Main components and component sets are walked like frames. An instance whose `component` is `None` counts as `unlinked`; one whose `component` is `Some(id)` is tallied under its component id even when the node is absent from the document, rolled up to its component set when the node resolves (a scoped copy carries no other page, so a main component on a Components page or in a library is usually absent there).
5. **Budget.** `USAGE_MAX_NODES = 50,000` and `USAGE_MAX_MILLIS = 100`, whichever comes first (the clock is read every 256 nodes). The result is the walked prefix, never a sample. Stop reason is `complete`, `node budget` or `time budget`. `first_unreached` is the first root not fully walked, for the follow-up call. Budgets are constants, not tool arguments.
6. **Paints.** For `Slot::Fill` and `Slot::Stroke` (via `Slot::paints`), count visible `Solid` paints only. A paint is `via_variable` when `<prefix>/<i>` is bound, else `via_style` when `style_refs[fill|stroke]` exists, else `raw`. `bound = via_variable + via_style`; percent = `bound / (bound + raw)`, rounded, omitted when the denominator is 0. Gradient and image paints go to `non_solid` and are not in the percent.
7. **Frames.** `Frame`, `Component`, `ComponentSet`: `auto layout` when `auto_layout.is_some()` (any direction, grid included), else `absolute`. Groups, sections and instances are not frames here.
8. **Radius, gap, padding.** Radius and padding applicability come from `number_value_at` (zero and excluded kinds are skipped). Radius: a node with a non-zero corner is one observation; all four equal gives the value, otherwise `mixed`; bound when every non-zero corner key is bound. Gap reads the layout field directly, not `number_value_at` (which drops negatives): `itemSpacing` (skipped for space-distributed layouts) plus `counterAxisSpacing` when `wrap`, every finite non-zero value, negative overlaps included; one observation per value; bound by its key. Padding: one observation per frame with any non-zero side, written as CSS shorthand (`16`, `12/24`, `8/16/24/8` as top/right/bottom/left); bound when every non-zero side is bound. Values print through `fmt_num`.
9. **Text and fonts.** Per text node, the effective styles are the base style (skipped when the runs cover the whole content) plus each run's style, de-duplicated by (family, size, weight), so a node counts once per distinct triple. Families, sizes and weights are three separate tallies. "On a style" is `style_refs["text"]` or a bound `fontSize` (the lint's skip rule, `scale.rs:479-480`).
10. **Components.** Identity is the family id, never the layer name: the parent `ComponentSet`'s id when the component node resolves and sits in a set, else the component's own id (also when the node is absent). A row is named by its family node (the set's name, else the component's, as `agent_context.rs:261-281` does), or, when the node is absent, by the layer name of the first instance seen; the Components line then ends with `Layer names stand in for <n> components not in this copy.` Variants of an absent set can split into several rows. Ties on count sort by name, then id. Names are cut to 40 characters (`...`) and control characters become spaces.
11. **Scoped copy.** Every walked node with `is_unloaded` true (the shell marker) adds to `unloaded_frames`. When it is above 0, `truncated` is true, line 1 ends with `<u> frames not loaded (scoped copy).`, the note adds that the file may list more frames, and no "of N" total is printed (frames past the first 128 are invisible to the MCP).
12. **Rendering.** Fixed lines in the order of the sample below; counts are plain integers (no thousands separators: they would collide with the list commas); lists are capped at 10 components, 8 values per histogram, 5 families, 8 sizes, 5 weights, ordered by count descending then value ascending (`mixed` last). A category with no observations prints `none`. The status on line 1 reads `complete`, `stopped at the node budget` or `stopped at the time budget`. Line 1 always carries scope, `nodes_walked`, the budget, the stop reason and the unloaded count, because the token cap keeps leading lines.
13. **Token cap.** `CAP_USAGE_TOKENS = 600`, not scaled by `max_tokens` (lists are bounded by construction: sample 232 tokens, worst case about 510 with both truncation notes and 40-character names, by the `chars / 4` rule; both measured on text written in the final format). If a body still exceeds it, `truncate_section_by_lines` cuts it with a hint naming `{"sections":["usage"]}`. `usage` stays out of the `enforce_total_budget` trim list.
14. **Truncated flag.** `truncated` is true when the stop reason is not `complete` or `unloaded_frames > 0`. The section ends with `[truncated: <note>]`, and the note is also `truncation_note`: `walk stopped at the <node|time> budget; <k> frames not fully walked; call designos_get_app_state {"sections":["usage"],"target_node_ids":[<first_unreached>]}`, and/or `<u> frames are empty shells in this scoped copy of a large file; select one in Studio, then call designos_get_app_state {"sections":["usage"],"target_node_ids":[<id>]}`.
15. **Where it runs.** In the process that holds the document: the MCP. Studio's UI thread never calls it (rule 1), and nothing in `designos-app` changes.
16. **Errors.** No new code: an unknown section is `VALUE_OUT_OF_RANGE`, a missing target is `NODE_NOT_FOUND`, as today.

Sample content (fixture-shaped, 232 tokens by `chars / 4`):

```
Scope: page `Dashboard` (id 12), 4 frames. Walked 4812 of at most 50000 nodes: complete.
- Fills: 1204 solid, 902 bound (75%: 871 variable, 31 style), 302 raw.
- Strokes: 188 solid, 40 bound (21%: 40 variable, 0 style), 148 raw.
- Not counted: 12 gradient or image paints.
- Frames: 214 auto layout, 37 absolute (85% auto layout).
- Text: 388 nodes, 301 on a text style or bound size (78%).
- Components: 460 instances of 23 components (instance contents are not walked), 4 unlinked. Top 10: Button 120, Icon/Chevron 88, Input 61, Avatar 40, Badge 33, Card 28, Tab 22, Checkbox 18, Toggle 15, Tooltip 12.
- Radius (540 nodes, 212 bound): 8 x214, 12 x90, 16 x40, 4 x22, 999 x15, mixed x9.
- Gap (214 values, 120 bound): 8 x120, 16 x66, 4 x30, 24 x12.
- Padding (62 frames, 18 bound): 16 x180, 12/24 x60, 8 x44, 24 x20.
- Fonts: Inter 388 nodes. Sizes: 14 x201, 12 x90, 16 x60, 20 x24, 32 x13. Weights: 400 x300, 600 x80, 700 x8.
```

## 4. Data model / schema changes

None to the document or any file format. New runtime types in `designos-doc::usage`: `UsageSummary` (roots, `nodes_walked`, `stop`, `first_unreached`, `unloaded_frames`, `fills`/`strokes` as `PaintUsage { via_variable, via_style, raw, non_solid }`, `frames_auto`, `frames_absolute`, `text_nodes`, `text_on_style`, `instances`, `unlinked`, `components: Vec<ComponentRow { family: NodeId, count: usize, layer_name: String }>`, `radius`/`gap`/`padding` as `Histogram { observations, bound, values: Vec<(String, usize)> }`, `families`, `sizes`, `weights` as `Vec<(String, usize)>`), `UsageStop`, `UsageParams`.

## 5. Contracts

```rust
// crates/designos-doc/src/usage.rs (pure; no I/O, no clock besides the deadline)
pub const USAGE_MAX_NODES: usize = 50_000;
pub const USAGE_MAX_MILLIS: u64 = 100;
pub struct UsageParams<'a> { pub max_nodes: usize, pub max_duration: Option<Duration>, pub is_unloaded: &'a dyn Fn(&Node) -> bool }
pub fn summarize_usage(doc: &Document, roots: &[NodeId], params: &UsageParams) -> UsageSummary;
// crates/designos-export/src/agent_context.rs
pub const CAP_USAGE_TOKENS: usize = 600;
// crates/designos-export/src/agent_context_usage.rs
pub(crate) fn build_usage_section(doc: &Document, targets: &[NodeId], page: Option<NodeId>, cap_tokens: usize) -> BootstrapSection;
```

`build_usage_section` passes `node_has_scoped_shell_marker` as `is_unloaded`, so `designos-doc` holds no marker strings. `BootstrapOptions` gets no new field.

MCP: `designos_get_app_state` input `sections` enum becomes `document, outline, variables, components, style, rules, usage` (and `VALID_SECTIONS` gains `usage`). Output shape is unchanged: `sections[i] = {"id":"usage","title":"Usage","tokens":n,"truncated":bool,"truncation_note"?:string,"content":string}`. Description: keep the first sentence ("Return the pre-fetched design context block.") and append: `Opt-in section "usage" (sections: ["usage"]) summarizes how the file uses variables, styles, components, radii, spacing and fonts.` Keeping the first sentence keeps `purpose_summary` (`rules.rs:478-488`) and `generated/rules.md` stable. Error codes: none new.

## 6. File-format mapping

None. The section reads the in-memory document only.

## 7. UI

None. No Studio string, panel or control changes, so `es-designer` and `design/ux-writing.md` do not apply; the block is agent-facing text.

## 8. Edge cases

- Empty page or no frames: `No frames in scope.`, `truncated` false.
- Everything is a shell (a large file, nothing selected): line 1 reports the unloaded frames, the list lines print `none`, `truncated` true.
- A target that is a descendant of another target, a duplicate target, a `Page` id: rule 2.
- Mixed corner radii, per-side padding, wrapped layouts, negative gaps, space-between layouts (no gap observation): rule 8.
- Text with runs that cover the whole content: the base style is not counted twice: rule 9.
- An instance whose component node is absent (scoped copy, or a library copy not loaded): tallied by id, named from its layer, never `unlinked`; only `component: None` is unlinked (rule 4).
- A paint with a binding but `visible == false`: not counted.
- A corrupt tree or a cycle: the node budget ends the walk; `Document::walk`'s recursion is not used.
- Names with newlines or backticks: sanitised (rule 10).
- Instances on inactive pages whose children a page hydrator has not expanded (`lib.rs:235`): no effect, instance children are never read.

## 9. Non-goals

- The first-turn block (Studio) stays as is: no `usage` there. No new `BootstrapOptions` field, tool argument or budget parameter.
- Whole-file or per-page selection beyond `target_node_ids` and the current page. No `page_id` argument.
- A Studio-published full-document snapshot for scoped sessions (open question 1).
- Sample screens (the plugin's `sampleScreens`), effects, grids, opacity, stroke weight, line height, colour histograms.
- Counting instance overrides; changing E15-26's `token_coverage` or the eval baseline; changing lint.
- Fixing the pre-existing drift noticed here: the schema enum omits `freshness` though the code accepts it (`bootstrap.rs:13` vs `:51`), and `tool_error_codes` gives this tool only the default codes although it returns `VALUE_OUT_OF_RANGE` and `NODE_NOT_FOUND` (`rules.rs:558`). Follow-up issues, not this task.

## 10. Test plan (red first)

Fixtures are hand-built in the test modules; no committed test reads a client file. "Fails before": every row is yes (nothing exists) unless marked guard.

| # | Test | Expected | Fails before |
|---|---|---|---|
| 1 | `paints_split_variable_style_and_raw` (doc) | rects: bound `fills/0`, style-linked fill, raw fill, invisible raw fill, gradient, text colour bound in `style.fills`, one bound and one raw stroke: exact `PaintUsage` numbers and percent | yes |
| 2 | `variable_binding_wins_over_style_link` (doc) | a paint with both counts once, as `via_variable` | yes |
| 3 | `usage_and_hardcoded_value_agree_on_raw` (doc) | the lint's `hardcoded-value` finds exactly the raw paint that matches a variable; usage counts it raw and the bound one in neither | yes |
| 4 | `frames_split_auto_layout_and_absolute` (doc) | frame/component/set counted; group, section, instance root not | yes |
| 5 | `instance_children_are_not_walked_and_components_tally_by_family` (doc) | raw fills inside an instance do not change counts; two variants of one set are one row; renamed instance layers do not split a row (component resolved) | yes |
| 6 | `unlinked_means_no_component_id` (doc) | `component: None` gives `unlinked == 1`, not in the top list; `Some(absent id)` is tallied, not unlinked | yes |
| 7 | `radius_gap_padding_histograms` (doc) | zero skipped; uniform vs `mixed`; wrap adds counter spacing; a negative gap is counted; space-between skipped; padding shorthand; bound counts | yes |
| 8 | `fonts_count_once_per_triple_and_runs_replace_a_covered_base` (doc) | families, sizes, weights; `text_on_style` via style link and via bound size | yes |
| 9 | `nested_and_page_roots_are_not_double_counted` (doc) | page root expands; nested target dropped | yes |
| 10 | `walk_stops_at_the_node_budget_with_a_deterministic_prefix` (doc) | budget 5 on 12 nodes: `nodes_walked == 5`, stop `NodeBudget`, `first_unreached` set, same result twice | yes |
| 11 | `walk_stops_at_the_time_budget` (doc) | zero duration returns `TimeBudget`, no panic | yes |
| 12 | `a_ten_thousand_level_chain_does_not_overflow_the_stack` (doc) | returns | yes |
| 13 | `shells_count_as_unloaded` (doc) | marker nodes counted through `is_unloaded` | yes |
| 14 | `usage_is_absent_unless_named` (export) | `sections: None` gives seven ids and no `usage` (extends `agent_context.rs:1691-1697`); `Some(["usage"])` gives exactly one | yes |
| 15 | `usage_section_golden_content` (export) | content equals the sample's shape on the fixture; tokens under the cap | yes |
| 16 | `usage_worst_case_fits_its_cap` (export) | 12 families, 60 components with 60-character names, 40 radii: `tokens <= 600`, `truncated` false | yes |
| 17 | `usage_on_a_scoped_copy_is_truncated_and_prints_no_total` (export) | shells present: `truncated`, note names the frames and the follow-up call, no "of N" | yes |
| 18 | `usage_scope_ignores_the_selection` (export) | `include_selection` with ids set does not change roots | yes |
| 19 | `usage_over_its_cap_is_cut_by_lines_and_keeps_line_one` (export) | `build_usage_section` with `cap_tokens = 80`: `truncated`, line 1 still carries budget and stop reason | yes |
| 20 | `usage_scoped_instance_with_an_absent_component_is_tallied_not_unlinked` (export) | scoped fixture, instance with `component: Some(id)` and no such node: `unlinked == 0`, a row under the layer name, and the Components line says layer names stand in for 1 component | yes |
| 21 | `app_state_usage_matches_the_export_builder` (mcp) | tool section equals the builder's; default call has no `usage`; unknown section still `VALUE_OUT_OF_RANGE` | yes |
| 22 | `tool_list_matches_snapshot`, docs drift tests, `rules` drift test | green after `gen-tool-docs`; `generated/rules.md` unchanged | guard |
| 23 | `usage_walk_budget_on_a_synthetic_page` (doc, `#[ignore]`) | 60,000 deterministic nodes: `nodes_walked == 50000`, stop `NodeBudget`; prints elapsed ms | yes |

Commands: `cargo test -p designos-doc usage`, `cargo test -p designos-export agent_context`, `cargo test -p designos-mcp tools::bootstrap`, `cargo run -p designos-mcp --bin gen-tool-docs` then `cargo test -p designos-mcp --test tools_snapshot` and `cargo test -p designos-mcp rules`, `cargo test -p designos-app bootstrap` (Studio's tests that bound the block and compare it with the tool, `terminal.rs:12906`, `:12989`), `cargo test -p designos-doc --release usage_walk_budget -- --ignored --nocapture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`. Evidence in the PR: terminal capture of the golden section, of the tool reply over the real stdio client pattern of `crates/designos-mcp/tests/stdio.rs`, and of test 23 with its milliseconds.

## 11. Performance budgets

- Per call: at most 50,000 nodes and 100 ms in the MCP process; output at most 600 tokens. INFERENCE: 15 to 30 ms at the node cap. Closing check: test 23, plus a manual run of `designos_get_app_state {"sections":["usage"]}` in a headless `designos-mcp` that opened "VSF - PCP.fig" (owner's machine, outside CI): record `nodes_walked`, `stop` and ms, for the current page and for one 50-frame target list; flag owner-manual if the file is absent.
- Attached session, file over 25,000 nodes (the 112,474-node benchmark): the walk sees the scoped copy, at most about 5,200 nodes, so a few ms; it is partial by design and says so (rule 11).
- Studio UI thread: untouched. `usage` is never built by Studio (rule 1), `designos-app` has no change, no `doc.to_json()`, no `app.changed()`, no `TileView::rev` bump (CLAUDE.md §4.1). The first-turn build budget of 300 ms (E15-16 §11) is unchanged because the section is not in it.
- Memory: tallies are bounded by distinct values; the component map by distinct components (at most the node count); the result is a few KB.

## 12. Dependencies and build order

None to wait for. Order: (1) `usage.rs` with tests 1-13 and 23; (2) `agent_context_usage.rs`, the `section_targets` extraction, the opt-in gate and the cap, tests 14-20; (3) tool surface (`VALID_SECTIONS`, enum, description), test 21; (4) `gen-tool-docs`, test 22, commit the generated files; (5) evidence. If #900 lands first on `bootstrap.rs`, rebase and regenerate (never hand-merge the generated tables).

## 13. Owner rulings that apply

- 2026-10-22 (study): "Small items (#896, #897, #898) are next-patch work" (spec folder `docs/specs/v0.6.x/`).
- E15-16 §13 owner ruling: "Full scope: every listed section, with explicit truncation, ships together." and §3.1: "Never a silent cut." Rules 11, 12 and 14 follow them.
- #698 owner ruling (`bootstrap.rs:4-5`; E15-16 §1): the canvas selection never sets what the agent is told; rule 2.
- CLAUDE.md §4: no UI-thread allocation or full-document serialisation; rule 15 and section 11.
- Standing, 2026-10-22: new tool parameters go through `gen-tool-docs`, `tools_snapshot.rs` and `rules.rs`; main is protected (PR, squash); visual evidence is required.

## Decisions on the open questions (controller, 2026-10-22)

The owner delegated these calls ("Orchestrate it yourself and answer it yourself", relayed by the release controller on 2026-10-22). Each question below was settled with its recommended answer, so they are binding for the implementer. The reasons are kept so the owner can reverse any of them.

1. **Scoped sessions (a file over 25,000 nodes).** Recommended: ship the honest partial result now. The MCP only holds shells plus the selected frames, so on the benchmark the numbers cover what the user selected and say so. Full coverage needs Studio to compute the summary from its full document in the background (reusing the first-turn job, `terminal.rs:2122-2176`), write it to the session folder and have the MCP serve it with a stale flag: a separate M task. Alternative: hold #897 until that exists (delays the headless and small-file value).
2. **Opt-in or default.** Recommended: opt-in. Default would add 232 tokens (typical) to about 510 (worst) to every first turn, would force Studio to build it too (the tool-equals-Studio test), and would give partial numbers on large files.
3. **Instance internals.** Recommended: not counted (rule 4), reported as component-owned. Alternative: walk instances and count only overridden properties, as E15-26 does: it needs `component.overrides` and a page-hydrator check (about +1 day), and walks every instance's materialised copy, so the node budget burns faster.
4. **Tell agents to use it.** Recommended: no sentence in the workflow rules pack for now (it changes every agent's first-turn instructions, which have a 6,000-token cap; the tool description is the discovery path). Alternative: add "before building into an existing file, call `usage`" once it has proven useful.
