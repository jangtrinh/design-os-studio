# E15-47 Build our own design:os skills pack from the ease-design knowledge

- Issue: #637 (task E15-47, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `skills/designos-use/`, `skills/generate-design/`, `skills/generate-library/`, `skills/critique/` (new, each in the tool-product profile layout), `skills/design-os-studio/` (updated), `skills/ATTRIBUTION.md` (new), `scripts/skills/{build_contract.py,validate.py,look.py,sync_hosts.py}` (new), `tests/skills/` shape, validate and contract tests (new), `crates/designos-mcp/src/prompts.rs` (new: MCP prompts), `crates/designos-mcp/src/server.rs` (enable prompts capability), `.github/workflows/ci.yml` (skill tests step), `.claude-plugin/plugin.json` (description tool count).

## 1. Summary and user story

As an agent working in design:os, I want short, exact skills for tool cadence and craft: how to call the tools (batch protocol, verify order), how to generate a screen and a library, and how to critique, built from our own knowledge set, attached to runs through #573 and served as MCP prompts.

## 2. Current state

- FACT: one skill exists, `skills/design-os-studio/SKILL.md` (52 lines), a router that lists binaries and workflows and says "Local stdio Model Context Protocol Server (22 tools)".
- FACT: `designos-mcp` advertises tools only: `ServerCapabilities::builder().enable_tools()` (`crates/designos-mcp/src/server.rs:635`); there are no MCP prompts.
- FACT: the owner's tool-product skill profile (es:skill-forge v2, `/Users/jang/Products/.claude/skills/skill-forge/references/tool-product-profile.md`) requires: router `SKILL.md` ≤ 120 lines with frontmatter `name` = folder and a "Use when …" description ≤ 200 chars (M1); every referenced sibling exists (M2); rules with "Rejected when" (M3); a generated contract with a sha256 header and staleness test (M4); a deterministic offline `validate` with `<check>: <what to fix>` lines and exit 0/1/2 (M5); a `look` script producing one sheet PNG (M6); examples that pass build + validate + look (M7); repo tests and CI (M8–M9); a hand-over contract (M10); single source synced to host bundles (M11); no secrets or network (M12); reviewer checks A1–A9 including ATTRIBUTION.md (A9).
- FACT: knowledge sources (MIT, owner's own): `ease-design/knowledge/taste-rubric.md`, `design-tells.md`, `token-taxonomy.md`, `color-science.md`, `accessibility.md`, `page-structures.md`, `ux-psychology.md`; E15-46 adds `canvas-rules.md` (MIT, attributed).
- FACT: #573 attaches bundled and user `SKILL.md` files to agent runs ("A chosen skill's text reaches the agent context, verified with a recorded fixture"). E15-15 (#600) generates the rules pack from tool schemas with a build-verify-fix workflow.

## 3. Behaviour rules

1. **Skills and their jobs.**
   - `designos-use`: tool cadence. Read before write (outline, query, screenshot), one batch per coherent change (E15-4), dry-run for risky batches, read the issues block after every write (E15-12), fix lint before moving on, verify with a screenshot at the end, named errors and `safe_to_retry` handling, idempotency keys on retries, in-progress flags, never write outside the owned frame (RD-3).
   - `generate-design`: from brief to screen. Pick the surface (E15-21) and a style (E15-18 catalogue or the active style), apply it (E15-19), start from a template when one fits (E15-22), build with instances and `var:` references only, assets through the asset tools (E15-39/40/41), finish with lint zero errors and a critique request by another run.
   - `generate-library`: build or extend a component library: tokens first (E15-19 or E15-42 import), components with variants and slots, naming, states matrix, documentation page (`design-system/page` template), publish.
   - `critique`: act as critic (E15-34): scaffold, look at the image and outline, submit findings with targets, severities and repair targets, never critique your own run's work; then repair flow (E15-35) for another run.
2. **Layout per skill** (profile canonical layout): `SKILL.md` (router ≤ 120 lines), `rules.md` (numbered rules, each with "Rejected when"), `concepts.md`, `look.md` (numbered visual checks), `contract.md` (generated), `examples/` (each a NodeSpec batch plus expected lint result), `scripts` shared in `scripts/skills/`.
3. **Content source.** Rules are written for design:os from the ease-design knowledge and `canvas-rules.md`; each rule cites its source file in a trailing `(source: knowledge/page-structures.md §2)` note. No third-party prompt text is copied. `skills/ATTRIBUTION.md` lists ease-design (MIT) and jakubkrehel/skills (MIT, via E15-46) (A9).
4. **Generated contract (M4).** `build_contract.py` reads the live tool list (`designos-mcp` tools JSON, produced by a `--list-tools` flag added here alongside the existing `--workspace`/`--help` arguments, `crates/designos-mcp/src/main.rs:6-13`), the E15-21 surface data, E15-18 catalogue ids and the E15-9 rule registry, and writes `contract.md` per skill with a `sha256:` header over its inputs. A test fails when the contract is stale.
5. **Validate (M5).** `validate.py <skill> [--example name]` checks offline: frontmatter, line limits, every referenced file exists, every tool name matching `designos_[a-z_]+` in the skill exists in the contract (AC3), every example batch parses as the E15-4 batch schema, and every example's expected lint counts match a headless run of `designos --apply-spec` + lint (product runtime only). Output lines `<check>: <what to fix>`; exit 0/1/2.
6. **Disabled tools (AC3).** Sections that depend on optional tools carry a marker `<!-- requires: designos_generate_asset -->` … `<!-- /requires -->`. When a session has the tool disabled (for example no agent profile, so asset tools are off, E15-39 rule 7), the served text drops the marked section. Validate fails if an unmarked section names a tool that can be disabled (the contract lists which tools are optional).
7. **Look (M6).** `look.py <skill>` renders each example with the CPU renderer into one contact sheet PNG and prints the `look.md` items it measures (for example `L3: all text ≥ 14 px: pass`); exit 1 on any machine-checkable failure. Browser dependencies are not needed (our renderer).
8. **MCP prompts.** `designos-mcp` enables the prompts capability and serves `prompts/list` with one prompt per skill (`designos-use`, `generate-design`, `generate-library`, `critique`, `design-os-studio`) and an optional argument `surface` for `generate-design`. `prompts/get` returns `SKILL.md` followed by `rules.md` and `contract.md` (disabled-tool sections removed), as one user-role text message. Sizes: each prompt ≤ 24k characters; a test enforces it.
9. **Attachment through #573.** The four skills and the updated `design-os-studio` skill are bundled skills in the #573 picker. AC2: a recorded fixture shows the chosen skill's text in the agent context of a stub run.
10. **Host bundles (M11).** The repo root is a Claude Code plugin (`.claude-plugin/plugin.json`, `marketplace.json`), which picks up `skills/` directly. `sync_hosts.py` copies the skills into any other host bundle folder the repo adds later, with a drift test; no symlinks. The plugin description's tool count ("22-tool MCP server") is updated from the contract.
11. **`design-os-studio` update.** Its router gains a section pointing to the four skills and the correct tool count (taken from the contract, not hard-coded).

## 4. Data model / schema

No document change. MCP prompt objects: `{ "name": "generate-design", "description": "Use when building a screen from a brief in design:os.", "arguments": [ { "name": "surface", "required": false } ] }`.

## 5. Contracts

- `prompts/list`, `prompts/get` as above. Unknown prompt → MCP error `prompt_unknown` with the list of names.
- `designos-mcp --list-tools` prints the tools JSON array and exits 0 (no workspace needed).
- Scripts: `python3 scripts/skills/validate.py skills/generate-design`, `python3 scripts/skills/look.py skills/generate-design --out sheet.png`, `python3 scripts/skills/build_contract.py --check`.

## 6. File-format mapping

None.

## 7. UI

No new UI; the skills appear in the #573 picker with their descriptions.

## 8. Edge cases

1. A tool renamed in a later PR: the contract goes stale, the test fails, and validate names every skill line using the old name.
2. A user-added skill with the same name as a bundled one: #573's rules decide precedence; bundled names are reserved with a `designos-` or exact match check in the picker.
3. Prompt too large after adding rules: test failure naming the prompt and its size; split content into `concepts.md` (not served by default).
4. Examples that depend on fonts not cached: `look.py` fetches through `designos-fonts` or fails with a clear message (no silent fallback face).
5. Offline machine: validate and look run with no network (M12); `--list-tools` needs no network.

## 9. Non-goals

- Adopting third-party skills (E15-46).
- Rules pack generation (E15-15); this pack references it.

## 10. Test plan

Seams: skill shape tests and contract staleness test (`tests/skills/`, run in CI), validate on broken examples, a recorded #573 fixture, MCP prompt tests (`cargo test -p designos-mcp prompts`).

Fail-before: the skills and prompts do not exist.

| Test | Input | Expected |
|---|---|---|
| `every_skill_passes_profile` (AC1) | four skills + design-os-studio | A1–A9 checks pass: ≤ 120-line router, references exist, contract fresh, examples build + validate + look |
| `broken_example_names_one_check` | one deliberately broken example per check | exit 1 with exactly that line |
| `skill_text_reaches_agent` (AC2) | #573 stub run choosing `generate-design` | recorded agent context contains the SKILL.md text |
| `no_disabled_tool_referenced` (AC3) | session without an agent profile; `prompts/get generate-design` | returned text contains no `designos_generate_asset` |
| `unmarked_optional_tool_fails_validate` | remove a `requires` marker | validate exit 1 naming the line |
| `prompt_size_limit` | all prompts | ≤ 24k characters |
| `rules_have_rejected_when` | rules.md files | every numbered rule has a "Rejected when" line |

Visual evidence: the `look.py` contact sheet for each skill, attached to the PR.

## 11. Performance budgets

- `prompts/get` ≤ 5 ms (text assembled at startup from embedded files).
- Validate + look for all skills ≤ 60 s in CI.

## 12. Dependencies and build order

- Needs #573, E15-15 (#600), E15-34; uses E15-46's `canvas-rules.md`.
- Last task of the pack.

## 13. Owner rulings that apply

- Skills: write our own from the ease-design knowledge and the existing design:os CLI skill; adopt MIT skills only with attribution.
- The tool-product skill profile is the owner's standard for skills that drive a design:os tool (2026-10-12).
- R4.13: skills instruct the user's agent; nothing bundles a model.
