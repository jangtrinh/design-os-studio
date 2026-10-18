# E15-15 Generate the design-agent rules pack from the tool schemas, with a build-verify-fix workflow

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | T7.40 (#248), T7.6 (#243), E15-5, E15-12 |
| Unblocks | E15-16 (bootstrap attaches the pack), part-2 E15-47 (our own skills pack) |
| Touches | `crates/designos-mcp/src/prompts.rs`, new `crates/designos-mcp/src/resources.rs`, `crates/designos-mcp/src/tools/guidelines.rs`, generator (`crates/designos-mcp/src/bin/gen-tool-docs.rs` from T7.6), new `crates/designos-mcp/rules/` templates, generated `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/`, `crates/designos-doc/src/spec.rs` (JSON Schema derive) |

## 1. Summary and user story

As an agent CLI working in design:os, I receive one authoritative rules text that matches exactly the
tools I have: how to plan, build one section per call, verify with the issues block and one small
screenshot, and fix in place; the NodeSpec reference; the error and retry rules; and our own
anti-generic design rules. When a tool is disabled, its section disappears. When a schema changes
without regenerating, CI fails.

## 2. Current state (FACT)

- Agents get guidance only from free text: the server instructions string (`crates/designos-mcp/src/server.rs:637`),
  the `--append-system-prompt` in the Claude launch command (`crates/designos-app/src/terminal.rs:140-151`),
  and a generated per-session `SKILL.md` that still lists the old tools and the live-spec file drop
  (`terminal.rs:936-962`).
- `skills/design-os-studio/SKILL.md` describes the product, not the agent workflow, and states a
  22-tool server.
- The apply_spec input schema describes `spec` as a bare object with prose (`crates/designos-mcp/src/engine.rs:794-801`);
  no machine schema of `NodeSpec` exists.
- No per-tool enable list exists in the MCP config written by Studio (`mcp-config.json`,
  `terminal.rs:840-860`).
- T7.40 adds prompt plumbing and `designos_get_guidelines`; T7.6 adds the tool snapshot and generator.
- Our design knowledge exists as MIT-licensed modules `knowledge/design-tells.md` (surfaces, colour and
  light, type, motion and decoration, rendered tells) and `knowledge/taste-rubric.md` (seven axes and a
  critique gate) in the ease-design repository.

## 3. Behaviour rules

### 3.1 Sources and generation

The generator assembles `generated/rules.md` from:
1. `rules/workflow.md` (hand-written template, below);
2. a tool section per enabled tool, generated from `tools()`: name, one-line purpose, required
   arguments, the error codes the tool can return (from the E15-5 catalogue mapping), and the first
   worked example from its description;
3. the NodeSpec reference generated from a JSON Schema of `NodeSpec`, `LayoutSpec`, `OverrideSpec`,
   `SlotFillSpec` and `NumberOrVar` (`schemars` derive in designos-doc, behind a `schema` feature used
   only by the generator and tests);
4. the issues-block and auto-fix description (E15-12, E15-13) and the error/retry rules (E15-5);
5. `rules/design-quality.md`: anti-generic rules written for design:os from the design-tells and
   taste-rubric knowledge (our own wording, attributed in a header comment to the MIT ease-design
   knowledge).

Placeholders `{{tool:<name>}}` in templates expand only when that tool is enabled; a template paragraph
wrapped in `{{#if tool:<name>}} … {{/if}}` drops out with it.

### 3.2 Workflow section (content requirements)

1. Read first: use the bootstrap context (E15-16) or `designos_get_metadata`; do not dump the document.
2. Plan the screen as sections; choose components and tokens via `designos_search_design_system`.
3. Build one section per call (`designos_apply_batch` / `designos_apply_spec`), with
   `in_progress: true` on the root frame (E15-7), components as instances (E15-1), tokens as `var:` /
   `style:` (E15-3), auto layout by default.
4. Verify: read the issues block first; then take one small screenshot of the finished section
   (`designos_get_screenshot`, default scale, long edge ≤ 1024) or use `diff_image`.
5. Fix in place with update ops; never delete and rebuild a section to fix it.
6. Clear `in_progress` when the section is done.
7. Errors: follow `next_step`; resend `corrected_example` when `safe_to_retry` is true; re-read when it
   is false; use `idempotency_key` on writes.

### 3.3 Design-quality section (content requirements)

Rules, each phrased as a checkable instruction: do not wrap every group in a card; keep gradients,
shadows and corner radii restrained and consistent with the file's tokens; never hand-draw logos,
illustrations or icons with vector paths (use library components or leave a labelled placeholder);
avoid repetitive uniform grids of identical cards; one primary action per view; use the type scale and
spacing scale; real content, never lorem ipsum (the `placeholder-text` rule flags it).

### 3.4 Enable list

- Studio's `mcp-config.json` gains `"designos": {"enabled_tools": [...]}` (absent = all enabled); the
  MCP reads it at start (path from `DESIGNOS_MCP_CONFIG`). Disabled tools are omitted from `tools/list`
  and from the rules text; calling one returns `TOOL_DISABLED`.

### 3.5 Serving

- MCP prompt `designos_rules` (no arguments) returns the rules text for the current enable list.
- MCP resource `designos://rules` (`text/markdown`), with the resources capability enabled.
- `designos_get_guidelines(topic: "rules")` (T7.40 registry).
- The per-session `SKILL.md` written by Studio (`terminal.rs:936-962`) is replaced by the generated
  rules text plus the session paths, so every agent CLI that loads session skills reads the same rules.
- `skills/design-os-studio/` gains a `rules.md` reference generated from the same source (the router
  `SKILL.md` links to it).

### 3.6 Budget and drift

- Token budget: the full rules text ≤ 6,000 tokens (chars / 4), recorded; the generator fails when over.
- Drift: a test regenerates the text from the live `tools()` and schema and compares it with the
  committed `generated/rules.md`; mismatch fails with the regenerate command (same mechanism as T7.6).

## 4. Data model / schema changes

- `schemars::JsonSchema` derives on the NodeSpec types (feature `schema`).
- MCP config key `designos.enabled_tools`.
- No document change.

## 5. Contracts

- Prompt `designos_rules`; resource `designos://rules`; topic `rules`.
- Error `TOOL_DISABLED` (catalogue addition to E15-5): `safe_to_retry: true`, `next_step` "Ask the user
  to enable <tool> in Studio's agent settings."
- `tools/list` honours the enable list.

## 6. File-format mapping

None.

## 7. UI

None in this task (the enable list is written by Studio from its existing MCP config generation; a
settings UI for it is not in scope).

## 8. Edge cases

1. All write tools disabled (read-only agent): workflow steps 3-6 drop out; the text says the session
   is read-only.
2. Asset tool (part 2) disabled: its section and any `{{#if}}` paragraph disappear (acceptance 1 uses
   whichever asset tool exists when this lands; before that, `designos_get_screenshot` is the test
   subject).
3. Unknown tool name in the enable list: ignored with a warning in `designos_get_app_state`.
4. Template references a tool that no longer exists: generator fails (cannot expand placeholder).
5. Schema description strings containing markdown: escaped.
6. Rules over budget after a schema grows: generator fails with the measured size, forcing a decision.

## 9. Non-goals

- Skills pack content (E15-46, E15-47).
- Surface guides and archetypes (E15-21, E15-18).

## 10. Test plan

Seams: snapshot test plus CI drift check.

| Case | Expected |
|---|---|
| `disabling_a_tool_removes_its_section` | enable list without `designos_get_screenshot` (and later the asset tool) → generated text has no section for it and no `{{#if}}` paragraph that depends on it |
| `drift_check_fails_on_schema_change` | change a NodeSpec field description in a test build → regenerated text differs from committed → failure message names the regenerate command |
| `rules_under_token_budget` | full enable list → ≤ 6,000 tokens; size recorded |
| `prompt_resource_tool_identical` | `prompts/get designos_rules`, `resources/read designos://rules`, `get_guidelines("rules")` return the same text |
| `session_skill_matches_rules` | Studio-generated session `SKILL.md` contains the generated rules body |

## 11. Performance budgets

Generation at build/test time only; serving is a static string per enable-list (cached). No runtime
cost per call.

## 12. Dependencies and build order

After T7.40 (prompt plumbing), T7.6 (snapshot/generator), E15-5 (catalogue) and E15-12 (issues block).
Before E15-16.

## 13. Owner rulings

- All AI through the user's agent CLI (R4.13): the rules are for the user's own CLI; no bundled model.
- Skills (2026-10-18): adopt MIT skills with attribution, write our own, skip anything without a
  licence. The design-quality section is our own text from MIT ease-design knowledge, attributed.
- Full scope: workflow, tool reference, schema reference, errors and design-quality rules ship together.
