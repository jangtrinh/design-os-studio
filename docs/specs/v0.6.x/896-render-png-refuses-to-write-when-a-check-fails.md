# #896 Refuse to write a PNG when a structure check fails (designos_render_png)

| Field | Value |
|---|---|
| Issue | #896 (enhancement; from the Figma plugin study, owner pack ruling 2026-10-22: next-patch work) |
| Size | S (issue: S). About one day: a checks module, one query-engine flag, one error code, regenerated docs. |
| Depends on | none |
| Unblocks | the guidance rule "structure is the gate, the PNG is the arbiter" in `skills/design-os-studio` (later; the rules pack has no room for it, see rule 10) |
| Touches | new `crates/designos-mcp/src/tools/render_checks.rs`; `crates/designos-mcp/src/server.rs` (`RenderArgs` `:169-174`, arm `:976-1014`, schema `:1397-1409`, fixture test `:2009-2582`); `crates/designos-mcp/src/tools/errors.rs` (new code); `crates/designos-mcp/src/rules.rs` (error row `:547-549`); `crates/designos-doc/src/query/mod.rs` (`include_scope_root`); generated: `crates/designos-mcp/tests/snapshots/tools.json`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`, the marked tool blocks of `README.md`, `CLAUDE.md`, `skills/design-os-studio/SKILL.md`, `docs/mcp-parity.md` |

Code baseline: `origin/main@42a0d027`. Re-anchor every cited line against HEAD and report drift before writing code. Evidence: Figma plugin `jangtrinh/design-os-figma-plugin` @ `7c1ef8bb`, `cli/src/commands/export-png.ts` and `cli/src/command-catalog.ts:219-230`. No plugin code is copied. The plugin's assertion is a JavaScript file; Studio has no script runtime, so the assertion language here is Studio's own: XPath through the query engine, and the lint engine.

## 1. Summary and user story

As an agent that renders a frame to prove it is right, I pass `checks` to `designos_render_png` and get either the PNG plus a record of what was verified, or a typed error that names the offending layers and no file. A PNG that this call created therefore means the checks held.

## 2. Current state (FACT unless labelled)

- Plugin: `export-png --assert` runs the script first, read-only, and exports only when it passes (`export-png.ts:4-8`, `:74`). A reply passes when `result` is `{ok:true}` or truthy (`:20-27`). A failure throws `E_ASSERT_FAILED` quoting the result with "PNG not written" (`:52-54`); a throw inside the script "keeps its own code" (`command-catalog.ts:226-227`). The file is written only after the check (`:85`), and the reply carries `assert:{script,result}` (`:88`).
- Studio `designos_render_png` (`server.rs:976-1014`): `RenderArgs{path,node_id,scale}` with `deny_unknown_fields` (`:168-174`); scale range (`:978`); `png_output_path` (`:983`, `:637-658`); `settle_layout()` (`:984`); node exists (`:985-990`); `render_target` (`:991`); empty-image check (`:998-1002`); `OpenOptions::create_new` (`:1003`) and, on a write error, `remove_file` (`:1007-1011`); reply `{path,node_id,scale,bytes,rendered}` (`:1013`). Nothing runs between "image is non-empty" and the write, so the issue's complaint holds. The input schema forbids extra keys (`:1434`), the output schema requires the five reply keys (`:1401-1408`).
- A file is created only at `:1003`, after rendering. A failed write removes its own partial file. The write is `create_new` plus `write_all` with no temp-and-rename, so a process killed mid-write could leave a partial PNG (INFERENCE; no test kills the process). That limit exists today and checks do not cause it.
- Query engine: `execute_query_xpath` (`tools/query.rs:108-193`) runs `designos_doc::query::run_with_env`; limits `DEFAULT_LIMIT 100`, `MAX_LIMIT 500`, `DEFAULT_MAX_SCANNED 50_000`, `MAX_MAX_SCANNED 200_000` (`query/mod.rs:14-18`); the engine arm settles layout first (`engine.rs:906-909`). With `within`, the scope root is never a candidate: depth 0 is skipped unless it is a page (`query/mod.rs:985-988`; `count_scope_nodes` `:798`). So `//TEXT[has-problem('text-truncated')]` within a TEXT node matches nothing even when that node is truncated.
- A query that reaches `limit` or runs out of `max_scanned` returns `done:!has_more`, `complete:!has_more`, `truncated:has_more` and a cursor (`query/mod.rs:1002-1075`); zero rows with `done:false` is therefore not proof of absence. Query errors map to the catalogue in `query_error` (`tools/query.rs:87-104`); `InvalidSelector` carries path `/selector` and `corrected_example {selector}` (`:90`).
- Lint arm (`engine.rs:911-1060`): default preset `Agent` (`:948`); budget `lint_call_max_nodes` 100,000 nodes and 2 s (`:93`, `:134`, `:1026`); `lint_scoped_on_worker` (`:1029`, `lint_env.rs:333`). A non-page root is linted itself (`lint/traverse.rs:109-121`). The arm folds document-wide `broken_style_binding_findings_for` into the first chunk (`:1045`), counted in `by_severity.warning` (`engine.rs:65-67`), whatever the roots. `Severity` order is `Error < Warning < Info` (`lint/registry.rs:62-66`); `LintReport.complete` (`lint/finding.rs:171`).
- Catalogue: `ErrorCode` enum `errors.rs:12-103`, `ALL` `:145-217`, `as_str` `:219-293`, `default_next_step` `:295-322`, `from_code_str` `:324-399`. The test `every_code_has_a_fixture` (`server.rs:2009-2582`) fails unless every code in `ALL` has a recorded fixture (`:2579-2582`) and validates each envelope against `error_envelope_schema` (`errors.rs:1090-1114`; `received` is unconstrained).
- Rules pack budget: `MAX_RULES_TOKENS 6000`, so 24,000 chars (`rules.rs:21-22`). `generated/rules.md` is 23,968 chars (measured this session: Python `len` of the file), headroom 32. A tool's Args line lists its top-level keys only (`rules.rs:443-470`), its Errors line is `tool_error_codes` (`:547-549`), its purpose is the first sentence cut at 106 chars (`:475-492`). The guard is `rules_under_token_budget` (`:843`); the drift guard is `drift_check_fails_on_schema_change` (`:809`).
- `designos_get_screenshot` is `execute_get_screenshot(doc, raw_args)` (`tools/screenshot.rs:592`): it takes only a `Document` and returns an inline image. It writes no file.
- UNKNOWN: whether other code assumes the query scope root is never a candidate. Closing check: `cargo test -p designos-doc query` after the flag, plus the new test.
- UNKNOWN: cost of `has-problem` over a large frame. Closing check: measure on the "VSF - PCP" benchmark (section 11).
- UNKNOWN: whether the `QueryOptions { .. }` literals at `openpencil.rs:625`, `query/mod.rs:1159` and `:1228` use `..Default::default()`. Closing check: compile.

## 3. Behaviour rules

1. **Argument.** `checks`, optional object, `additionalProperties:false`, at least one key. `no_match`: 1..=8 XPath selectors, each 1..=2048 chars (the limit of `query.rs:111`). `lint_clean`: `"error" | "warning" | "info"`, the weakest severity that counts as a failure. One nested object, not two flat `require_*` keys, because of the rules-pack budget (rule 10). Without `checks` the call and its reply are unchanged (no `checks` key).
2. **Scope is the node and everything under it.** `no_match` runs `run_with_env` with `within = node_id`, `include_scope_root = true`, `limit = 20`, `max_scanned = MAX_MAX_SCANNED`, and the session's `engine.lint_env()`. `QueryOptions` gains `include_scope_root: bool`, default `false`, not exposed on `designos_query_xpath` (that tool is unchanged). `lint_clean` runs `lint_scoped_on_worker` with `roots = [node_id]`, preset `agent`, the budget `Budget::with_limits(engine.lint_call_max_nodes.max(1), 2 s)` that `designos_lint` uses (`engine.rs:1026`), and without the document-wide style-binding extras (`engine.rs:1045`): a subtree check must not fail on a finding outside the subtree. Findings count when `severity <= threshold`.
3. **Order.** Parse, scale, path, `settle_layout`, node exists, **checks**, render, empty-image, `create_new`. Checks never run before layout has settled (the render uses the same settled document), and nothing is created before they pass.
4. **Verdicts.** Each check ends `passed`, `failed` or `incomplete`. `no_match` passes only with zero rows and `done && complete`. `lint_clean` passes only with zero qualifying findings and `report.complete`. A scan that stops on its budget is `incomplete`, and `incomplete` fails: the check fails closed. The agent is told to render a smaller frame.
5. **All checks run.** There is no short circuit, so one reply lists every problem. An error from the engine (`INVALID_SELECTOR`, `UNKNOWN_RULE`, `RULE_UNAVAILABLE`) aborts the call with its own code, as in the plugin; its path is rewritten from `/selector` to `/checks/no_match/<i>` and its `corrected_example` to `{"checks":{"no_match":["<corrected>"]}}`, so a retry has the right shape.
6. **Failure.** Any `failed` or `incomplete` check returns `CHECK_FAILED` (new code), `safe_to_retry:false` (an identical retry fails again; the agent must fix the layers first), path = the first failing check (`/checks/no_match/0` or `/checks/lint_clean`), `expected` "no matches", message naming the counts, `received` = the report of rule 8. No file is created, `files_created` does not change, and a file already at `path` is neither overwritten nor removed.
7. **Pass.** The PNG is written exactly as today and the reply gains `checks: {passed:true, node_id, ran:[...]}`.
8. **Report shape.** One entry per check: `{check:"no_match", selector, status, matched, more, nodes:[{node_id,name,type}], scanned}` (at most 20 nodes, names cut at 80 chars; `more` is true when the limit was reached) and `{check:"lint_clean", severity, preset:"agent", status, findings, by_rule:{...}, nodes:[{node_id,rule,severity}], scanned}`. `scanned` is the coverage the verdict rests on.
9. **`designos_get_screenshot` gets no `checks`.** It writes no file, so "a file on disk means the check held" does not apply to it; its function takes only a `Document` and has no lint environment; and the rules pack has no room for a second tool's arguments (rule 10). Agents that want a gated image use `designos_render_png`.
10. **Rules pack budget.** Measured at baseline: Args `(optional: \`scale\`)` becomes `(optional: \`checks\`, \`scale\`)` = +10 chars; the Errors row gains `, \`CHECK_FAILED\`` = +16 chars; total +26 of 32, leaving 6. Two flat keys would cost +42 and fail `rules_under_token_budget`. Therefore: keep the first sentence of the description and the worked example (`rules.rs:597`) unchanged, add no other error code to the row (`INVALID_SELECTOR`, `UNKNOWN_RULE` and `RULE_UNAVAILABLE` stay out of it on purpose; they come back in the standard envelope and the tool description, after its first sentence, names them), and add no workflow text. The worker re-measures at HEAD; see Open question 1.

## 4. Data model / schema changes

None to the document or its files. `QueryOptions.include_scope_root` is an in-memory option. New private types in `render_checks.rs`: `RenderChecks`, `CheckVerdict`, `ChecksReport`.

## 5. Contracts

- Input (`server.rs` schema): `"checks": {"type":"object","additionalProperties":false,"minProperties":1,"properties":{"no_match":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"string","minLength":1,"maxLength":2048}},"lint_clean":{"type":"string","enum":["error","warning","info"]}}}`. Output schema: optional `"checks": {"type":"object"}`; the five required keys do not change.
- Rust: `#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(crate) struct RenderChecks { #[serde(default)] pub no_match: Vec<String>, pub lint_clean: Option<Severity> }`; `pub(crate) fn run_render_checks(engine: &mut Engine, node: NodeId, checks: &RenderChecks) -> Result<ChecksReport, ToolError>`; the arm calls it between `:990` and `:991` and turns a non-passing report into `CHECK_FAILED`. `render_checks.rs` stays under 200 lines.
- Error code: `ErrorCode::CheckFailed`, `"CHECK_FAILED"`, added in the enum (a new "Render codes (1)" group), `ALL`, `as_str`, `from_code_str`, the count in the `ALL` doc comment, and `default_next_step`: "Fix the layers listed in `received`, then render again. No file was written." Row: `designos_render_png` gains `` `CHECK_FAILED` `` (`rules.rs:547`).
- Pass reply: `{"path":"a.png","node_id":12,"scale":1.0,"bytes":4211,"rendered":true,"checks":{"passed":true,"node_id":12,"ran":[{"check":"no_match","selector":"//*[has-problem('text-truncated')]","status":"passed","matched":0,"more":false,"nodes":[],"scanned":38},{"check":"lint_clean","severity":"error","preset":"agent","status":"passed","findings":0,"scanned":38}]}}`.
- Failure: `{"error":{"code":"CHECK_FAILED","path":"/checks/no_match/0","safe_to_retry":false,"received":{"passed":false,"node_id":12,"ran":[{"check":"no_match","status":"failed","matched":1,"nodes":[{"node_id":31,"name":"Title","type":"TEXT"}]}]},"next_step":"Fix the layers listed in `received`, then render again. No file was written."}}`.
- Generated output: run `cargo run -p designos-mcp --bin gen-tool-docs`; commit the changes it makes.

## 6. File-format mapping

None. The PNG bytes are the same as today's.

## 7. UI

None. This is a headless MCP tool; no Studio panel, canvas or copy changes. `es-designer` is not needed (no rendered UI file changes).

## 8. Edge cases

- `node_id` is a leaf (a TEXT node): the scope root is a candidate (rule 2), so the node itself is checked.
- `node_id` is a page: both engines already treat a page root specially (`query/mod.rs:985-988`; `lint/traverse.rs:113-117`, children only) and the checks follow them. A page is rarely rendered.
- A file already exists at `path` and a check fails: `CHECK_FAILED`, the old file untouched. The existing file with passing checks: `FILE_EXISTS` after the render, as today (an early existence probe is a separate follow-up).
- `checks: {}` or `checks: {"no_match": []}`: `INVALID_ARGUMENTS` at `/checks`. An unknown key inside `checks`: `UNKNOWN_FIELD` at `/checks/<key>`.
- A text-shaping rule in a selector while `disable_text_lint_engine` is set: `RULE_UNAVAILABLE` (query rule), never a pass.
- Duplicate selectors run twice and report twice. No dedupe.
- A selector that matches the node but an `//*` descendant selector matches thousands: `matched` is capped at the limit, `more:true`, the report lists 20.
- A concurrent edit cannot land between the checks and the render: both run inside one `&mut self` call (INFERENCE; closing check: the arm holds `&mut self` throughout).

## 9. Non-goals

- Running checks without writing a file (use `designos_query_xpath` and `designos_lint`).
- Any script, expression or predicate language; arbitrary lint presets or per-rule selection (`lint_clean` uses the `agent` preset).
- Atomic write by temp file and rename; an early `FILE_EXISTS` probe before the checks.
- Adding checks to `designos_get_screenshot`, `designos_export_fig_file` or `designos_save_file`.
- Exposing `include_scope_root` on `designos_query_xpath`.
- The skill guidance text (blocked by the rules-pack budget).

## 10. Test plan (red first)

Fixture for the failing cases: a frame holding the known truncated TEXT (10 `W` at 16 px in a 100x24 fixed box, `tools/query.rs:210-217`) and an `iiii` text that fits. Tests live in `crates/designos-mcp/tests/session.rs` (workspace on disk), `server.rs` (`every_code_has_a_fixture`), `tools/render_checks.rs` and `query/mod.rs`.

| Case | Expected | Fails before |
|---|---|---|
| `render_png_without_checks_is_unchanged` | reply has the five keys and no `checks`; PNG exists | guard |
| `failing_no_match_returns_check_failed_and_no_file` | `CHECK_FAILED`, path `/checks/no_match/0`, offender id in `received`, `ls` shows no PNG | yes (`checks` is `UNKNOWN_FIELD`) |
| `existing_file_is_untouched_when_a_check_fails` | pre-created bytes identical after the call | yes |
| `passing_checks_write_the_png_and_report_what_ran` | `checks.passed:true`, `ran` lists both checks with `scanned` | yes |
| `no_match_covers_the_rendered_node_itself` | render the truncated TEXT node with the `text-truncated` selector: `CHECK_FAILED` | yes |
| `include_scope_root_matches_the_root_and_default_does_not` (designos-doc) | flag on: root row returned; off: unchanged output | yes |
| `lint_clean_error_refuses_a_frame_with_an_error_finding` | `CHECK_FAILED`, finding quoted (the worker picks an Error-severity rule from `lint/registry.rs`; verify) | yes |
| `lint_clean_ignores_findings_below_the_threshold_and_outside_the_subtree` | a warning does not fail `"error"`; a document-wide style-binding finding elsewhere does not fail | yes |
| `lint_clean_covers_the_node_itself` | a finding on the rendered node fails | yes |
| `incomplete_lint_scan_fails_closed` | `engine.lint_call_max_nodes = 8` on a bigger frame: `CHECK_FAILED`, `status:"incomplete"` | yes |
| `zero_rows_with_an_unfinished_scan_is_incomplete` | pure verdict function on a synthetic page (`rows` empty, `done` false) | yes |
| `invalid_selector_keeps_its_code_and_gets_a_checks_path` | `INVALID_SELECTOR` at `/checks/no_match/0`, example in `checks` shape, no file | yes |
| `all_checks_run_and_are_reported_together` | two failing checks, both in `received` | yes |
| `verdict_equals_the_standalone_tools` | `checks` verdict equals `designos_query_xpath` and `designos_lint` after the same auto-layout edit (layout settled) | yes |
| `empty_or_unknown_checks_are_invalid_arguments` | `/checks` and `/checks/<key>` | yes |
| `every_code_has_a_fixture` | fixture 70 produces `CHECK_FAILED`, `safe_to_retry:false` | yes (new code in `ALL`) |
| `rules_under_token_budget`, `drift_check_fails_on_schema_change`, `tool_list_matches_snapshot` | green after `gen-tool-docs`; pack <= 24,000 chars | yes until regenerated |

Run: `cargo test -p designos-doc query`, `cargo test -p designos-mcp render_png`, `cargo test -p designos-mcp every_code_has_a_fixture`, `cargo test -p designos-mcp --test session`, `cargo test -p designos-mcp rules::`, `cargo test -p designos-mcp --test tools_snapshot`, `cargo run -p designos-mcp --bin gen-tool-docs` (then `git diff --stat` shows only the generated files), `cargo fmt --all --check`, `cargo clippy -p designos-doc -p designos-mcp --all-targets -- -D warnings`. Evidence (PR): a terminal capture of a failing call (error JSON, then `ls` with no PNG) and a passing call, and the passing PNG viewed as an image.

## 11. Performance budgets

- Checks add at most 8 query scans (each capped at 200,000 nodes) and one lint scan (100,000 nodes, 2 s). The reply adds at most about 4 KB.
- Stress case: the "VSF - PCP" benchmark (112,474 scene nodes). Render the largest frame with `lint_clean` and one `has-problem` selector; record the wall time. Beyond the budgets the verdict is `incomplete`, never a hang; the headless server has no UI thread.
- No `doc.to_json()` and no document mutation (CLAUDE.md section 4). Idle cost: none, no background work.

## 12. Dependencies and build order

Independent of #897 and #898, except that all three next-patch items draw on the same 32-char rules-pack headroom (Open question 1). Order: `include_scope_root` and its test; `render_checks.rs` and its tests; the error code in all six places and its fixture; the arm, schema and `rules.rs` row; `gen-tool-docs`; re-measure the pack; evidence capture; PR (squash).

## 13. Owner rulings that apply

- 2026-10-22 (pack): "Small items (#896, #897, #898) are next-patch work."
- Standing: new MCP params go through the generated docs and the drift test (`gen-tool-docs`, `tools_snapshot.rs`, `rules.rs`).
- 2026-09-22: "Làm cái gì trên toàn bộ project cũng phải chụp hình chứng minh" (visual evidence in every PR).
- Standing: `main` is protected (PR, green macOS test, clippy and rustfmt, squash); releases are verified on macOS.

## Decisions on the open questions (controller, 2026-10-22)

The owner delegated these calls ("Orchestrate it yourself and answer it yourself", relayed by the release controller on 2026-10-22). Each question below was settled with its recommended answer, so they are binding for the implementer. The reasons are kept so the owner can reverse any of them.

1. **Who pays for rules-pack characters?** After this change 6 of 24,000 chars remain, and #897 needs some. Recommended: keep the cap and trim prose in the pack in the same PR if HEAD has under 26 chars of headroom, because the cap is the per-session token cost of every agent. Alternative: raise `MAX_RULES_TOKENS`.
2. **`safe_to_retry:false` for `CHECK_FAILED`?** Recommended: yes, so a client does not auto-retry an identical call that fails again; the `next_step` says what to fix.
3. **No `checks` on `designos_get_screenshot`?** Recommended: no (rule 9). Alternative: add it when a second tool's argument line fits the pack.
4. **`lint_clean` fixed to the `agent` preset and a severity only?** Recommended: yes, for a deterministic verdict; a `preset` key can follow.
5. **Add `include_scope_root` to `designos-doc` (touches the query engine) or document that a check covers descendants only?** Recommended: add the flag; otherwise checking one rendered TEXT node passes vacuously.
