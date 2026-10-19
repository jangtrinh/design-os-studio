#!/usr/bin/env python3
"""Build e15-tasks.json: the E15 AI design epic, its tasks, amendments and pack moves.

Owner rulings 2026-10-18: E15 is P0 and sits in the new pack "v0.6 AI design", right after
v0.5. Existing tasks that E15 depends on (plus their transitive deps) move into that pack;
the old "v0.6 Effects & Export" contents move to "v0.7 Agent design context".
The script validates the competitor-name ban and the pack ordering, and fails on either.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
PACK = "v0.6 AI design"
OLD_V06 = "v0.6 Effects & Export"
V07 = "v0.7 Agent design context"
_BANNED_FILE = pathlib.Path.home() / "Products/plans/reports/banned-terms.txt"  # kept outside the repo
BANNED = re.compile(_BANNED_FILE.read_text().strip() if _BANNED_FILE.exists() else r"(?!)", re.I)
EVIDENCE = "Visual evidence (screenshot or terminal capture) is required in the PR."
UI_LINE = "Load es-designer before UI code; desktop capture at 1440 goes in the PR (responsive: SKIPPED, owner default)."

master = json.loads((HERE / "master-tasks.json").read_text())
state = json.loads((HERE / "filed-state.json").read_text())
FT = state["tasks"]
RD = state["rd"]

TASKS = []


def task(tid, title, size, touches, deps, context, scope, ac, seam, rulings=None, ui=False):
    deps_txt = ", ".join(deps) if deps else "none"
    body = [f"**Epic:** E15 | **Pack:** {PACK} | **Size/Prio:** {size} / P0", "", "### Context", context, "", "### Scope"]
    body += [f"- {s}" for s in scope]
    body += ["", "### Acceptance criteria"] + [f"{i}. {a}" for i, a in enumerate(ac, 1)]
    body += ["", "### Test seam", seam]
    if ui:
        body.append(UI_LINE)
    body += ["", "### Touches", touches, "", "### Deps", deps_txt]
    if rulings:
        body += ["", "### Owner rulings"] + [f"- {r}" for r in rulings]
    body += ["", "### Source", "plans/261017-epics-full-scope/e15-tasks.json (owner rulings 2026-10-18)", "", EVIDENCE]
    TASKS.append({"id": tid, "epic": "E15", "title": title, "body": "\n".join(body), "prio": "P0", "size": size,
                  "touches": touches, "pack": PACK, "deps": deps})


R_AI = "R4.13: every AI call goes through the user's own agent CLI; no model is bundled in the app."
R_FULL = "Full scope, never MVP (2026-10-17)."
R_FIX = "Auto-fix (2026-10-18): safe mechanical fixes apply automatically with a diff report; configurable to off or report-only."
R_LOCK = "Parallel agents are hard-locked to their own frames (RD-3, 2026-10-18)."
R_EVAL = "Eval never uses client files; only open kits and self-generated large files (2026-10-18)."
R_SKILL = "Skills (2026-10-18): adopt MIT jakubkrehel/skills with attribution, write our own where needed, skip anything without a licence."
R_EXACT = "R2.7 (D-E3-3): token suggestions use exact value matches only; ambiguous matches are reported without a fix."

# ---------------------------------------------------------------- A. Grounding and write protocol
task("E15-1", "Place component instances and variants from apply_spec (NodeSpec kind instance)", "L",
     "designos-doc/src/spec.rs, designos-doc spec instance resolver (new module), designos-library lookup",
     [],
     "NodeSpec supports frame, rectangle, ellipse, text, section and component only, so an agent cannot place the file's own components and redraws them as loose layers.",
     ["`kind:\"instance\"` with `component` (local id, component key or library path) and `variant` (property map, e.g. `{Size:\"lg\", State:\"hover\"}`)",
      "`overrides`: text, fills, visibility, instance swap and nested properties, addressed by descendant name path (`Header/Title`) or id",
      "Unknown component, variant value or override path returns a named error listing the valid options",
      "Instances keep their main-component link (no detach) and appear in the response's name-to-id map"],
     ["A spec placing a library Button with `variant:{Size:\"lg\"}` yields an instance linked to the right variant, not a copy",
      "A name-path override changes only the addressed descendant; the rest stays inherited",
      "An invalid variant value fails with a named error that lists the allowed values, and the document is unchanged",
      "Export and re-import through .designos and .fig keep the instance link"],
     "designos-doc unit tests on a fixture library + MCP apply_spec test", [R_FULL])

task("E15-2", "Fill component slots from apply_spec, validated against each slot's recommended components", "M",
     "designos-doc/src/spec.rs, designos-doc spec instance resolver",
     ["#150", "E15-1"],
     "Slots get recommended component ids (#150 extension). The spec path must fill them without breaking the slot contract.",
     ["`slots:{\"Content\":[<NodeSpec>...]}` on an instance replaces that slot's children as a descendant override",
      "Fills outside the slot's recommended list are accepted with a lint warning, or refused when the slot is strict",
      "An unused slot can be disabled from the spec"],
     ["A Card instance with a filled Content slot round-trips through .designos and the JSON scene format",
      "A non-recommended fill returns a warning naming the recommended components",
      "Disabling a slot hides it in the instance without detaching"],
     "designos-doc unit test with the synthetic slot fixture (E3-15)")

task("E15-3", "Bind fills, strokes, numbers and styles to document variables from apply_spec (var: references)", "L",
     "designos-doc/src/spec.rs (resolve_easeui_color, apply_easeui_preset), designos-doc variables binding",
     [],
     "Spec colours are hex or hard-coded `$easeui.*` tokens resolved to fixed RGB (spec.rs:145). Nothing binds to the document's own variables.",
     ["`var:Collection/Group/Name` on fill, stroke, gap, padding, radius, stroke width, font size, line height and opacity creates a real variable binding",
      "`style:\"Text/Heading L\"` (text, colour, effect styles) applies the document style by name",
      "Mode-aware: a frame-level `mode:{Collection:\"Dark\"}` sets the explicit variable mode",
      "Back-compat: `$easeui.*` and `easeui-*` presets keep working as deprecated aliases that resolve through an EaseUI variable collection inserted on first use; each use adds a deprecation warning to the response"],
     ["A spec with `fill:\"var:Color/Primary\"` produces a bound fill; changing the variable recolours the node",
      "An unknown variable returns a named error listing the 5 closest names",
      "An existing spec using `$easeui.bg.panel` renders bit-identically to today and reports a deprecation warning",
      "Hard-coded hex where an exact-match variable exists is flagged by the per-call issues block (E15-12)"],
     "designos-doc unit tests + CPU render snapshot for the alias path", [R_FULL, R_EXACT])

task("E15-4", "Apply batched agent writes as one transaction: dry-run, one undo group, created ids and a name-to-id map", "L",
     "designos-mcp/src/tools/batch.rs (new), designos-mcp/src/engine.rs dispatch only, designos-doc history",
     [],
     "apply_spec already builds on a trial document and commits one Insert (engine.rs:244-262), but other writes are single calls with no batch, preview or id map.",
     ["`designos_apply_batch(ops[], dry_run)` with ops apply_spec, insert, update, move, delete, set_variables, set_style",
      "All ops run on a trial document; any failure rolls back the whole batch; success commits one undo group labelled with the agent run",
      "Reply: created ids, a name-to-id map (including descendants), changed-node counts and bounds of every touched root",
      "`dry_run:true` returns the same reply plus the issues block without committing",
      "New MCP tools land in `designos-mcp/src/tools/`; engine.rs keeps dispatch only (it is over 1,000 lines)"],
     ["A 3-op batch whose last op fails leaves the document byte-identical (hash compare)",
      "A successful batch undoes and redoes as exactly one history step",
      "Dry-run returns ids and bounds identical to the real commit that follows",
      "The name-to-id map resolves every named node the batch created"],
     "cargo test -p designos-mcp batch fixtures")

task("E15-5", "Return named MCP errors with a corrected example, a safe_to_retry flag and structured output", "M",
     "designos-mcp/src/server.rs, designos-mcp/src/tools/errors.rs (new), tool schemas",
     ["E15-4"],
     "Tool errors are free-text strings today, so agents guess at the fix and resend whole payloads.",
     ["Error catalogue: stable code, message, offending path, a corrected example payload, `safe_to_retry` (true only when no state changed)",
      "Errors returned as MCP `isError:true` results with `structuredContent`; every tool declares an `outputSchema`",
      "1-5 worked examples in the description of every complex tool (apply_spec, batch, apply_style, query)"],
     ["Each catalogue code has a fixture that triggers it and checks the corrected example passes",
      "`safe_to_retry` is false whenever any partial state was written (none should be, verified by hash)",
      "tools/list output validates against the MCP 2025-11-25 tool schema"],
     "MCP protocol tests over stdio")

task("E15-6", "Deduplicate replayed agent writes with idempotency keys", "S",
     "designos-mcp/src/tools/batch.rs, designos-mcp session ledger",
     ["E15-4"],
     "Agent CLIs retry on timeouts; a replayed write must not duplicate layers.",
     ["Optional `idempotency_key` on every mutating tool; the session ledger stores key, payload hash and the original reply",
      "Same key + same payload returns the stored reply without re-applying; same key + different payload returns a named error",
      "Ledger lives for the document session and survives an MCP reconnect; cleared when the document closes"],
     ["Sending the same batch twice with one key creates the layers once and returns identical replies",
      "Key reuse with a changed payload fails with the named error and changes nothing"],
     "cargo test -p designos-mcp")

task("E15-7", "Flag frames as in progress while an agent works and clear the flag when it finishes", "M",
     "designos-doc node model (in_progress flag), designos-fig JSON scene format placeholder mapping, designos-app canvas badge, designos-mcp tools",
     ["E15-4", "#570"],
     "Reviewers, critics and other agents need to know which frames are unfinished.",
     ["Node flag `in_progress` settable from spec/batch and by the agent runtime on root frames being worked on",
      "Canvas shows a badge on flagged frames; critique (E15-23) and variant ranking skip them",
      "Cleared by the agent at the end of the frame, automatically when the agent run ends or exits, and after a configurable stale timeout",
      "Round-trips through .designos and maps to the JSON scene format `placeholder` field (#570)"],
     ["A frame flagged by a batch shows the badge and loses it when the run ends",
      "A killed agent leaves no flag after the stale timeout",
      "JSON scene export/import keeps the flag"],
     "designos-doc unit test + shell test + screenshot", ui=True)

task("E15-8", "Return before/after diff images for the region a write touched", "M",
     "designos-mcp/src/tools/batch.rs, designos-diff, designos-render CPU path",
     ["#238", "E15-4"],
     "Agents verify a write by taking a fresh screenshot; a diff of just the touched region is cheaper and clearer.",
     ["Batch option `diff_image:true` renders the union bounds of touched roots before and after on the bit-exact CPU path",
      "Returns before, after and a highlighted diff as MCP image content, under the screenshot long-edge cap",
      "Reports changed-pixel count and bounding box"],
     ["A recolour batch returns three images whose diff box equals the recoloured node's bounds within 1 px",
      "A no-op batch reports 0 changed pixels and returns no diff image"],
     "cargo test -p designos-mcp with a pixel fixture")

# ---------------------------------------------------------------- B. Geometric linter and auto-fix
task("E15-9", "Unify design lint into one solver-backed geometric linter", "L",
     "designos-doc/src/lint/ (new module absorbing lint_document and lint_fix_document), designos-mcp designos_lint",
     ["#157", "#158"],
     "MCP `designos_lint` checks 2 rules (zero bounds, default names). designos-doc has a separate 17-rule `lint_document` that MCP never calls. Neither uses text layout results.",
     ["One rule registry with ids, categories, severities and presets; MCP, Check designs and eval all call it",
      "Uses computed layout bounds and text layout (line count, wrap, overflow) from the engine, not heuristics",
      "Rules: overflow of parent, sibling overlap, partial/full clipping, text truncation and unexpected wrap, collapsed size (fit with no children, circular fit/fill), absolutely positioned child inside auto layout, placeholder text (lorem ipsum, default labels), detached instances, plus the existing 17 rules",
      "Findings: rule, severity, node id, name, xpath, bounds, message, suggested fix, fixable flag; dismiss/ignore shared with #158",
      "Incremental: lint a subtree; shares the off-thread scanner from #157"],
     ["Every rule has a fail-before fixture and a clean fixture",
      "MCP designos_lint returns the full rule set and the old 2-rule output is gone",
      "Linting the 723k-node file runs off the UI thread and finishes within a recorded budget",
      "An ignored finding never reappears (shared with #158)"],
     "designos-doc unit tests per rule + perf test on the large fixture")

task("E15-10", "Check WCAG contrast of text against its resolved backdrop", "M",
     "designos-doc/src/lint/contrast.rs (new), designos-render CPU sampling",
     ["E15-9", "#158"],
     "Contrast today compares against a guessed parent fill. Text over gradients, images or stacked translucent layers is not measured.",
     ["Composite every paint under the text bounds (fills, gradients, images, opacity, blend) on the CPU path and take the worst-case ratio",
      "WCAG 2.x AA and AAA, large-text thresholds per #158",
      "Finding reports measured ratio, required ratio, and the variable pair that would pass when one exists"],
     ["Grey text on a gradient fails where the old parent-fill check passed",
      "Text over a translucent overlay on an image reports the worst-case ratio within 0.05 of a reference calculation",
      "Large text uses the 3:1 threshold"],
     "designos-doc unit tests with rendered fixtures")

task("E15-11", "Lint spacing scale, type scale and hard-coded values that match tokens", "M",
     "designos-doc/src/lint/scale.rs (new), Check designs matcher (shared library function)",
     ["E15-9", "#154", "#155"],
     "Spacing and type drift is the most common generated-design defect. Check designs already matches hard-coded values to variables (#154, #155).",
     ["Spacing scale and type scale taken from the document's number variables and text styles, or from the active style archetype",
      "Off-scale gap, padding, radius and font size flagged with the nearest scale step",
      "Token-match reuses the Check designs exact matcher as a shared library function; no second matcher"],
     ["A 15 px gap on a 4-step scale is flagged with 16 as the suggestion",
      "A hard-coded colour equal to Color/Primary is flagged with a Bind fix; an ambiguous match is reported without a fix",
      "Check designs and the linter return identical token findings on the same fixture"],
     "designos-doc unit tests", [R_EXACT])

task("E15-12", "Return a lint issues block from every mutating MCP call", "M",
     "designos-mcp/src/tools/issues.rs (new), every mutating tool reply",
     ["E15-9", "E15-4"],
     "Agents fix problems best in the very next call, while the context is fresh.",
     ["create, update, delete, apply_spec, apply_batch and style tools lint the touched subtrees and append `issues[]` (node names, not only ids)",
      "Budgeted: at most N findings per reply, sorted by severity, with a total count",
      "Configurable per session: rule preset, off, errors only"],
     ["A write creating clipped text returns a clipping issue naming the node",
      "A clean write returns `issues: []`",
      "Turning the block off removes it with no other reply change"],
     "MCP fixture tests")

task("E15-13", "Auto-fix safe mechanical lint findings in the engine and return a diff report", "L",
     "designos-doc/src/lint/fix.rs (extends lint_fix_document), designos-mcp/src/tools/issues.rs",
     ["E15-12", "E15-10", "E15-11"],
     "Many findings have exactly one correct fix. Applying them in the engine saves an agent round trip.",
     ["Safe tier (auto): fit/fill conflicts, missing text fill, absolute child inside auto layout when order is unambiguous, exact token binds, off-scale values with one exact scale step",
      "Suggest tier (never auto): contrast changes, ambiguous token matches, anything that changes a visible colour",
      "Fixes apply after the agent's batch as a separate undo step labelled Auto-fix",
      "Diff report: node, property, before, after, rule",
      "Mode: on (default), report-only, off; per user, file and session, with per-rule overrides"],
     ["A fill-container child with no layout parent is fixed and listed in the report",
      "Report-only lists the same fixes and changes nothing",
      "Undoing once removes only the auto-fixes, keeping the agent's batch",
      "Contrast findings are never auto-applied"],
     "designos-doc unit tests + MCP fixture", [R_FIX])

task("E15-14", "Configure auto-fix and review its diff report in Studio", "M",
     "designos-app Check designs panel, settings, designos-app agent panel",
     ["E15-13"],
     "Users must see and control what the engine changed on their behalf.",
     ["Settings: auto-fix on / report-only / off, per file and per user, with per-rule toggles",
      "Check designs panel shows the latest auto-fix report; clicking a row selects the node",
      "Undo-one-fix action per row"],
     ["Switching to report-only stops changes and still lists fixes",
      "Clicking a report row selects and zooms to the node",
      "Settings persist across restarts"],
     "shell test + screenshot", [R_FIX], ui=True)

# ---------------------------------------------------------------- C. Agent context
task("E15-15", "Generate the design-agent rules pack from the tool schemas, with a build-verify-fix workflow", "L",
     "designos-mcp prompts/resources, designos-mcp/src/tools/guidelines.rs, build script for generated rules, skills/design-os-studio",
     ["#248", "#243", "E15-5", "E15-12"],
     "Agents need one authoritative rules text that matches the tools actually exposed.",
     ["Rules generated from the live tool list and the NodeSpec schema; a CI check fails when they drift",
      "Workflow: plan, build one section per call, verify (issues block first, then one small screenshot of the finished section), fix in place, never delete and rebuild",
      "Anti-generic rules written for design:os from the ease-design knowledge (design-tells, taste-rubric): no card-wrapping everything, restrained gradients/shadows/radius, no hand-drawn logos or illustrations, avoid repetitive grids",
      "Sections for disabled tools drop out; a per-tool enable list is added to the MCP config if missing",
      "Served as an MCP prompt, a resource, and `designos_get_guidelines(topic:\"rules\")`"],
     ["Disabling the asset tool removes its section from the generated rules",
      "The drift check fails when a tool schema changes without regenerating",
      "The rules text stays under a recorded token budget"],
     "snapshot test + CI drift check")

task("E15-16", "Prefetch design context on an agent's first turn", "M",
     "designos-app agent runtime bridge (terminal.rs), designos-mcp/src/tools/bootstrap.rs (new)",
     ["#239", "#241", "#247", "#272", "E15-15"],
     "Agents waste their first calls discovering the document.",
     ["On run start the runtime attaches: open document, pages, selection, depth-3 outline of target frames, variables summary, components and libraries, active style, rules pack",
      "Marked fresh with a timestamp so the agent does not re-read it",
      "Also available as `designos_get_app_state` for agents outside Studio",
      "Token budget with truncation markers, never silent cuts"],
     ["A recorded first turn contains every listed section",
      "On the 723k-node file the bootstrap stays under the budget and says what was truncated",
      "A stub agent run makes zero discovery calls before its first write"],
     "recorded-fixture test with a stub agent CLI")

task("E15-17", "Query large documents with a streaming visitor: paging, node budget and compact rows", "M",
     "designos-mcp designos_query_xpath, designos-doc query module, designos-mcp/src/tools/query.rs",
     ["E15-9"],
     "Dumping a 723k-node document is impossible; agents need targeted queries.",
     ["XPath and predicate queries stream results with a cursor; `limit` and a hard node budget per call",
      "Compact rows: id, name, type, bounds, lint problems, in-progress flag",
      "Predicates on lint problems (e.g. all clipped text) and on hard-coded values",
      "Runs off the UI thread; reports scanned and remaining counts"],
     ["Paging through all text nodes of the large fixture returns each node exactly once",
      "A query for clipped text returns the same set as the linter",
      "No page exceeds the node budget"],
     "MCP test on the large fixture + perf record")

# ---------------------------------------------------------------- D. Style archetypes and surfaces
task("E15-18", "Model style archetypes with typed parameters and ship our own catalogue", "L",
     "designos-doc style archetype data (new), assets/styles/*.json (new), designos-mcp/src/tools/guidelines.rs",
     ["#248", "E15-10"],
     "Colour and type choices are where models are weakest. Briefs plus concrete, validated values remove those choices.",
     ["Archetype = prose brief (core, identity, layout, token map, decoration, hierarchy, typography) + typed parameter sources",
      "Parameters: font slots (heading, body, caption, data), role-keyed palettes, radius scales, elevation sets, imagery recipes",
      "Calling without params returns the valid options; single-option params fill automatically; invalid params fail with a named error",
      "Catalogue written for design:os from ease-design knowledge (design-soul, personas, color-science, token-taxonomy); fonts limited to licences that allow embedding"],
     ["Every archetype validates against the schema",
      "Every palette passes AA for its declared text/background role pairs (E15-10 math)",
      "No-param call returns the option lists; a bad font slot value fails with the allowed list"],
     "unit tests over the catalogue + schema check in CI")

task("E15-19", "Compile a style archetype straight into variables, modes and styles (designos_apply_style)", "L",
     "designos-doc style compiler (new), designos-mcp/src/tools/style.rs (new)",
     ["E15-18", "#131", "#138", "E15-4"],
     "The agent should not transcribe values by hand; the engine writes them.",
     ["Writes colour, number (radius, spacing) and string (font) variable collections with Light/Dark modes, text styles, effect styles bound to those variables",
      "Names are CSS-custom-property compatible so code export keeps them",
      "Deterministic and idempotent: re-applying updates in place; switching archetype re-points the same names",
      "One undo step; dry-run supported through the batch protocol"],
     ["Applying an archetype twice yields identical documents",
      "A frame built with `var:` references re-themes when another archetype is applied",
      "DTCG export of the result validates (#138)"],
     "designos-doc unit tests + render snapshot")

task("E15-20", "Show a style preset gallery rendered by our own renderer", "M",
     "designos-app agent panel style picker, build script rendering thumbnails with designos-render",
     ["E15-19"],
     "Users choose styles by looking, not by reading parameter names.",
     ["Presets = archetype + palette + fonts + radius + elevation, with web and mobile sample thumbnails",
      "Thumbnails rendered by the CPU path from a sample screen built with E15-19; cached and regenerated on catalogue change",
      "Picking a preset applies it and tells the agent which style is active"],
     ["Every preset has both thumbnails and they match a fresh render bit-exactly",
      "Picking a preset applies the variables in one undo step",
      "The gallery opens with no network"],
     "shell test + screenshot", ui=True)

task("E15-21", "Encode surface guides as data and lint profiles: web app, landing page, mobile, slides, table, design system", "M",
     "assets/surfaces/*.json (new), designos-doc/src/lint/surface.rs (new), designos-mcp/src/tools/guidelines.rs",
     ["#248", "E15-9", "E15-11"],
     "Prose guides are advice; numeric rules can be checked.",
     ["Each surface = prose (served by #248) + machine rules: minimum body size, edge margins, section and item gaps, tab bar item count, slide title/body minimums, required data states (loading, empty, error, success, permission)",
      "A frame tagged with a surface is linted against its profile",
      "Content written for design:os, using ease-design page-structures and ux-psychology as source"],
     ["A slide with 18 px body text fails the slides profile",
      "A mobile frame with 6 tab items fails the mobile profile",
      "get_guidelines prose and lint profile come from one data file"],
     "unit tests per surface")

task("E15-22", "Ship surface templates, including slide layout contracts with an intent-to-layout map", "L",
     "assets/templates/surfaces/ (new bundled library), designos-mcp guidelines/templates listing",
     ["E15-21", "E15-1", "E15-2", "E15-3"],
     "Agents start better from a verified skeleton than from an empty frame.",
     ["Templates for web app shell, landing sections, mobile shell (status bar, content wrapper, tab bar), table, design-system page",
      "Slides: 20 layout contracts (cover through closing), each with grid, size ranges and rules, plus an intent-to-layout selection map",
      "Built from our own components and bound to style variables, so they re-theme with E15-19",
      "Insertable via apply_spec instance and listed by get_guidelines"],
     ["Every template passes its surface lint profile with zero errors",
      "Applying another archetype re-themes every template",
      "The selection map returns a valid contract for each listed intent"],
     "lint run over all templates in CI + render snapshots")

# ---------------------------------------------------------------- E. Eval harness
task("E15-23", "Review dataset, model-weight and skill licences for the eval and skills pack", "S",
     "docs/eval-licences.md (new register), THIRD_PARTY_LICENSES",
     [],
     "Eval data and adopted skills must be legally usable; client files are banned.",
     ["Register per source: Design2Code, WebSight, UICrit, UIClip weights, CLIP weights, open UI kits (shadcn and others), jakubkrehel/skills, transitions.dev",
      "Records licence, allowed use (eval only / redistribute), attribution text, pinned version",
      "Anything without a licence is excluded; no client file (VSF-PCP) is ever listed"],
     ["Every source used by E15 eval tasks appears in the register with a licence",
      "Unlicensed sources are marked excluded and no task fetches them",
      "A CI grep fails if a client fixture path appears under eval/"],
     "CI register check", [R_EVAL, R_SKILL])

task("E15-24", "Generate large deterministic test documents from open kits", "M",
     "crates/designos-eval/src/gen/ (new)",
     ["E15-1", "E15-3"],
     "Edit tasks on big files need fixtures we own.",
     ["Seeded generator: 10k, 100k and 1M-node documents with pages, components, instances, variables with modes, auto layout and text",
      "Built only from open kits in the licence register",
      "Same seed gives a byte-identical file"],
     ["Two runs with one seed produce identical bytes",
      "The 1M-node file opens and renders headless within a recorded budget",
      "Instance ratio and variable coverage match the requested profile"],
     "cargo test -p designos-eval", [R_EVAL])

task("E15-25", "Build the designos-eval harness: runner, tasks, 3 runs, confidence intervals, held-out set, JSONL", "L",
     "crates/designos-eval (new crate)",
     ["E15-23"],
     "No public benchmark exists for canvas agents; quality cannot improve without measurement.",
     ["Task format with suite, brief/inputs, checklist, design system, split (dev / held-out 30%, hash-locked)",
      "Runner drives the user's agent CLI (claude -p, codex exec, any configured command) against a sandboxed document per run",
      "3 runs per task; mean with bootstrap confidence intervals",
      "Cost metrics: tool calls, tokens when reported, wall time, error and retry rate, undo groups",
      "JSONL results per release; renders use the bit-exact CPU path"],
     ["A stub agent CLI run produces valid JSONL for every task",
      "Held-out tasks are refused in tuning mode",
      "Re-running a recorded transcript reproduces identical deterministic metrics"],
     "cargo test -p designos-eval with a stub agent", [R_AI, R_EVAL])

task("E15-26", "Compute deterministic eval metrics from the linter and renderer", "M",
     "crates/designos-eval/src/metrics/deterministic.rs",
     ["E15-25", "E15-9", "E15-10", "E15-11"],
     "The cheapest, most trusted signal comes first.",
     ["Render success, token coverage %, instance ratio, lint violations per 100 nodes by rule, contrast failures, clipped/overflowing text, off-grid %, auto-layout coverage"],
     ["Each metric has a fixture with a known value",
      "Metrics match the linter's own counts on the same document"],
     "unit tests")

task("E15-27", "Add suite S1 recreate with fidelity metrics", "L",
     "crates/designos-eval/suites/s1/, crates/designos-eval/src/metrics/fidelity.rs",
     ["E15-25", "E15-23"],
     "Recreating a reference screen measures fidelity.",
     ["40 reference pages (fixed seed) from a licence-cleared source",
      "Block-Match, Text, Position, Color and CLIP similarity; CLIP weights downloaded locally to the user cache, never bundled in the app"],
     ["Ground truth scored against itself reaches the expected ceiling",
      "Metric code reproduces the published reference implementation on 3 samples"],
     "unit tests + one live run", [R_AI, R_EVAL])

task("E15-28", "Add suite S2 brief-to-screen on two open design systems", "M",
     "crates/designos-eval/suites/s2/",
     ["E15-25", "E15-23"],
     "Measures grounding and taste from a written brief.",
     ["50 briefs x 2 open design systems loaded as libraries",
      "Per-task checklist for the judge"],
     ["Every brief has a checklist and a surface tag",
      "A stub run scores all 100 tasks with deterministic metrics"],
     "stub run", [R_EVAL])

task("E15-29", "Add suite S3 edits on large self-generated files", "M",
     "crates/designos-eval/suites/s3/",
     ["E15-24", "E15-25", "E15-17"],
     "Editing complex existing files is weaker than greenfield generation.",
     ["30 edit tasks on generated large files",
      "Drift metrics: untouched regions must stay pixel- and structure-identical"],
     ["An edit that touches an unrelated frame is caught by the drift metric",
      "Every task runs on the 100k and 1M fixtures"],
     "stub run + drift fixture", [R_EVAL])

task("E15-30", "Add suite S4 multi-screen flows with a consistency metric", "M",
     "crates/designos-eval/suites/s4/, crates/designos-eval/src/metrics/consistency.rs",
     ["E15-25", "E15-26"],
     "Consistency across screens is a known weakness.",
     ["10 flows of 4-6 screens",
      "Consistency metric: same components for same roles, identical title sizes, token use, spacing rhythm across screens"],
     ["A flow with one off-size title scores lower than its corrected twin",
      "Metric is deterministic across re-runs"],
     "unit tests")

task("E15-31", "Judge designs with checklists through the agent CLI, with position swap, plus a local aesthetic score", "L",
     "crates/designos-eval/src/judge/ (shared library used by variant ranking)",
     ["E15-25", "E15-23"],
     "A judge without a checklist cannot be trusted on aesthetics.",
     ["Per-task checklist judge through the agent CLI; rubric reuses the ease-design taste rubric",
      "Pairwise mode swaps A/B positions and takes 3 samples; disagreement reported",
      "Judge model must differ from the generator model; recorded per result",
      "UIClip-style aesthetic score from locally downloaded weights as one signal, never a gate"],
     ["Swapping positions on identical inputs yields a tie within tolerance",
      "A run whose judge equals its generator is refused",
      "Aesthetic scoring works with no network once weights are cached"],
     "unit tests with a stub judge + one live run", [R_AI])

task("E15-32", "Add suite S5 judge calibration against UICrit and owner votes", "M",
     "crates/designos-eval/suites/s5/",
     ["E15-31", "E15-23"],
     "The judge must be calibrated before its scores steer anything.",
     ["UICrit subset (licence permitting) + 100 owner pairwise votes collected through a small voting view",
      "Agreement metrics; a judge configuration is trusted only above a recorded threshold",
      "The owner vote leg is flagged owner-manual"],
     ["Agreement is computed and stored in JSONL",
      "An untrusted judge configuration is marked and excluded from gates"],
     "unit tests; owner-manual vote session flagged", [R_EVAL])

task("E15-33", "Gate CI on deterministic eval metrics", "M",
     "crates/designos-eval CI job, .github/workflows",
     ["E15-26", "E15-29"],
     "CI has no agent CLI, so it replays recorded transcripts against the current engine.",
     ["Replay recorded tool-call transcripts per suite; compute deterministic metrics",
      "Fail on a significant regression versus the release baseline JSONL (bootstrap test)",
      "Live-agent runs happen locally and update the baseline only through a reviewed commit"],
     ["A seeded regression (clipping introduced) fails the gate",
      "An unchanged engine passes on 3 consecutive CI runs"],
     "CI run")

# ---------------------------------------------------------------- F. Critique and variants
task("E15-34", "Critique designs against a rubric checklist with bounding boxes, never by the generator", "L",
     "designos-mcp/src/tools/critique.rs (new), designos-doc agent authorship records",
     ["#572", "#238", "E15-7", "E15-9", "E15-21", "E15-31"],
     "Unstructured screenshot loops do not improve quality; rubric-scoped critique does.",
     ["`designos_critique(frame)` returns the rubric (taste axes + surface checklist) and the lint findings as a JSON scaffold",
      "The critic submits findings with node ids or region boxes, severity and a repair target; schema-validated",
      "Critic is never the generator: the engine records which agent run wrote each node (RD-3 identity) and refuses a critique from the same run, with a named error",
      "Returns an annotated screenshot with numbered boxes; findings can be posted as canvas comments",
      "Skips frames flagged in progress"],
     ["A critique submitted by the authoring agent run is refused",
      "Findings outside the frame bounds are rejected by the schema check",
      "The annotated image boxes match finding bounds within 1 px"],
     "MCP tests with two stub agent runs + screenshot", [R_AI, R_LOCK])

task("E15-35", "Repair critique findings inside their region only, with a regression guard", "L",
     "designos-mcp/src/tools/critique.rs, RD-3 ownership enforcement, designos-diff",
     ["E15-34", "E15-4", "#572"],
     "Fixing one region can break another unless each fix is scoped.",
     ["A repair run gets a temporary write lease on the finding's nodes or region; writes outside are refused (RD-3 enforcement)",
      "After the repair: lint diff and pixel diff outside the region must be unchanged, otherwise the batch rolls back with a named error",
      "Each repair is its own undo step and is linked to its finding"],
     ["A repair touching a node outside its lease is refused and the document is unchanged",
      "A repair that changes pixels outside its region rolls back",
      "A valid repair resolves its finding on re-critique"],
     "MCP tests + pixel fixtures")

task("E15-36", "Explore design variants in parallel frames or branches with mixed models", "L",
     "designos-mcp/src/tools/variants.rs (new), agent runtime orchestration",
     ["#572", "#343", "#345", "E15-19", "E15-16", "E15-7"],
     "Variety comes from parallel attempts, different models and style changes.",
     ["`designos_explore_variants(brief, n, mode, placement, models[])`: placement = N numbered frames or N E10 branches",
      "First run builds one anchor screen; later runs iterate in `same_style` (keep the archetype) or `new_style` (new archetype params) mode",
      "Quick actions: layout, style, colour, typography, surprise",
      "Each variant agent is locked to its own frame or branch; models can differ per agent; stop cancels all"],
     ["N=3 in frames mode creates 3 frames written by 3 agents with no cross-frame writes",
      "Branch mode creates 3 branches that each merge cleanly into a fresh main",
      "new_style variants use different archetype parameters, verified from the variables written"],
     "MCP tests with stub agents", [R_AI, R_LOCK])

task("E15-37", "Rank variants by lint, checklist and pairwise scores and merge the winner", "M",
     "designos-mcp/src/tools/variants.rs, crates/designos-eval judge library",
     ["E15-36", "E15-26", "E15-31", "E15-34"],
     "Selecting the best of several attempts beats one attempt.",
     ["Score = deterministic metrics + checklist judge + pairwise judge with position swap",
      "Ranked list with per-component scores",
      "Merge the winner (branch merge #345) or keep its frame; losers are archived, never deleted"],
     ["Ranking is reproducible from stored scores",
      "Merging the winner leaves the losers archived and restorable"],
     "unit tests + MCP test")

task("E15-38", "Add the variant explorer UI: count, mode, model per agent, progress and ranking", "M",
     "designos-app agent panel variant view",
     ["E15-36", "E15-37"],
     "Users drive exploration from Studio, not only from an agent CLI.",
     ["N x multiplier, same/new style toggle, model picker per agent, quick actions",
      "Per-frame progress and owner agent; ranking view with scores; merge/keep actions"],
     ["Starting 3 variants shows 3 progress rows and 3 owned frames",
      "The ranking view lists scores and merges the chosen winner"],
     "shell test + screenshot", ui=True)

# ---------------------------------------------------------------- G. Assets through the agent CLI
task("E15-39", "Generate images into frames through the agent CLI with a pending placeholder", "L",
     "designos-mcp/src/tools/assets.rs (new), designos-app agent runtime bridge, image assets",
     ["#177", "E15-7", "E15-18"],
     "Generated imagery should never stall the design or bypass the user's own agent.",
     ["`designos_generate_asset(kind:\"image\", prompt, target)` sets a pending fill at once and returns",
      "The user's agent CLI produces the image; the result is stored in the document and replaces the pending fill in one undo step",
      "Prompt follows the active style's imagery recipe",
      "Agent stop or failure leaves a visible failed state, never an empty frame without warning"],
     ["The pending fill appears immediately and is replaced on completion",
      "With no agent configured the tool is disabled with an explanation",
      "cargo tree shows no bundled model crate"],
     "shell test with a stub agent command + screenshot", [R_AI])

task("E15-40", "Insert stock images with attribution metadata", "M",
     "designos-mcp/src/tools/assets.rs, node metadata, Inspect panel",
     ["E15-39"],
     "Stock photos need their credit kept with the layer.",
     ["`kind:\"stock\"`: the agent CLI finds and fetches the photo; author, source URL and licence are stored as node metadata",
      "Attribution shows in Inspect and is included in exports that support credits",
      "A miss returns a warning, not a failure"],
     ["A stock fill carries author, URL and licence metadata after save and reload",
      "A miss returns the named warning and leaves the frame unchanged"],
     "shell test with a stub agent + screenshot", [R_AI])

task("E15-41", "Stream generated SVG into frames as it arrives", "L",
     "designos-mcp/src/tools/assets.rs, designos-doc vector_fit, MCP progress notifications",
     ["E15-39", "E15-7"],
     "Logos and illustrations should be generated vectors, not hand-drawn shapes.",
     ["`kind:\"svg\"`: the agent CLI streams SVG drafts; each complete path is inserted as it arrives, fitted to the frame and cleaned with the existing vector fitting",
      "Target frame is flagged in progress until the stream ends",
      "Stop aborts the stream, keeps complete paths and marks the frame partial"],
     ["A streamed 20-path SVG appears path by path and matches the final SVG render within threshold",
      "Stopping mid-stream keeps only complete paths and clears the in-progress flag"],
     "shell test with a stub streaming agent + screenshot", [R_AI])

# ---------------------------------------------------------------- H. Code and design tokens
task("E15-42", "Import globals.css and Tailwind themes as variable collections with modes", "L",
     "designos-export or designos-interop tokens import (new module), CLI `designos tokens import`, MCP tool",
     ["#138"],
     "Projects already define tokens in code; design should start from them.",
     ["Parse `:root`, `.dark`, `[data-theme]`, `@media (prefers-color-scheme)` and Tailwind v4 `@theme` into collections with modes",
      "Parse Tailwind v3 theme objects from static JSON/TS config (no code execution)",
      "Names stay CSS-custom-property compatible so export round-trips; `var()` aliases become variable aliases"],
     ["A shadcn-style globals.css imports with Light and Dark modes and aliases intact",
      "Export after import reproduces the same custom property names and values",
      "Unsupported syntax is listed in import warnings, nothing dropped silently"],
     "fixture round-trip tests")

task("E15-43", "Turn CSS custom properties into tokens when importing a web capture", "M",
     "designos-interop/webcapture token mapping",
     ["#276", "#277", "E15-42", "E15-11"],
     "Captured pages lose their token structure and come in as hard-coded values.",
     ["Custom properties recorded by the capture become variables; nodes whose computed value came from a property get bound",
      "Remaining hard-coded values that exactly match variables are bound; others are reported through the token-match lint"],
     ["A captured page using `--primary` yields a Color/primary variable bound on its buttons",
      "Unmatched values appear as lint findings with node names"],
     "capture fixture test + screenshot")

task("E15-44", "Detect repeated subtrees in web imports and turn them into components", "L",
     "designos-interop/webcapture component detection",
     ["#277", "E15-1"],
     "Imported pages repeat cards, rows and buttons as loose copies.",
     ["Structural hash of tag, class and layout shape with size tolerance finds repeats",
      "Creates a main component and instances; differing text and images become overrides",
      "Captured framework component names are used as hints and names"],
     ["A page with 6 identical cards imports as 1 component and 6 instances",
      "Override text matches the source page; screenshot diff stays within threshold"],
     "capture fixture test + screenshot")

task("E15-45", "Verify generated code by rendering it and pixel-diffing against our render", "L",
     "designos-export verify module (new), designos-diff, MCP `designos_verify_code`",
     ["#250", "#276"],
     "Codegen correctness (fill/fit to flex mapping, tokens) is unproven without rendering.",
     ["Render generated HTML/CSS, Tailwind and React output headless through the capture runtime",
      "Pixel-diff against the bit-exact CPU render with designos-diff; report per-node mismatches",
      "Fixture set covered in CI; available to agents as an MCP tool"],
     ["The fixture set passes within threshold",
      "A deliberately broken flex mapping fails with the mismatching node named"],
     "browser pixel diff in CI")

# ---------------------------------------------------------------- I. Skills
task("E15-46", "Adopt the MIT jakubkrehel/skills with attribution, rewritten as canvas-checkable rules", "M",
     "skills/vendor/ (new), THIRD_PARTY_LICENSES, skill rule map",
     ["#573", "E15-23"],
     "Proven UI-quality skills exist under MIT; they are web-code oriented and need canvas rules.",
     ["Vendor at a pinned commit with full MIT text and attribution",
      "Map each rule to a lint rule id or rubric checklist item; rules with no canvas meaning are marked web-only",
      "Skip any source without a licence (transitions.dev today)"],
     ["The licence text and pinned commit are present and match upstream",
      "Every adopted rule has a mapping entry",
      "No unlicensed skill is bundled"],
     "CI licence and mapping check", [R_SKILL])

task("E15-47", "Build our own design:os skills pack from the ease-design knowledge", "L",
     "skills/design-os-studio/ and new skills (designos-use, generate-design, generate-library, critique), MCP prompts",
     ["#573", "E15-15", "E15-34"],
     "Agents need tool-cadence and craft skills specific to design:os.",
     ["Skills: designos-use (tool cadence, batch protocol, verify order), generate-design, generate-library, critique",
      "Content built from ease-design knowledge (taste-rubric, design-tells, token-taxonomy, color-science, accessibility, page-structures, ux-psychology) and the existing design-os-studio skill",
      "Follows the skill-forge tool-product profile: router SKILL.md under 120 lines, generated contract, validate script, CI shape tests",
      "Attached through #573 and served as MCP prompts"],
     ["Every skill passes the profile's shape and contract tests",
      "A chosen skill's text reaches the agent context in a recorded fixture",
      "No skill references a tool that is disabled"],
     "skill shape tests + recorded fixture")

# ---------------------------------------------------------------- epic, amendments, moves
EPIC_BODY = """AI design: give agents eyes, grounding, a safe write protocol, an exact linter, curated styles, structured critique, variants and an offline eval, all running through the user's own agent CLI.

### Scope
- Grounding: component instances, variants, slots and variable/style binding in apply_spec (replaces hard-coded EaseUI tokens).
- Write protocol: batched transactions with dry-run, one undo group, name-to-id map, named errors with corrected examples, safe_to_retry, idempotency keys, in-progress flag, before/after diff images.
- Solver-backed geometric linter returned on every write, plus configurable engine auto-fix with a diff report.
- Agent context: generated rules pack, first-turn bootstrap, streaming large-document query.
- Styles: archetypes with typed parameters compiled into variables and styles, preset gallery, surface guides as data and templates.
- Quality loop: rubric critique by a separate critic, region-scoped repair with a regression guard, variant explorer with ranking.
- Assets through the agent CLI: images, stock with attribution, streaming SVG.
- Code and design: CSS/Tailwind themes to variables, web-import tokens and components, verified codegen.
- designos-eval harness (S1-S5, deterministic metrics first, judge calibration, CI gate) and a skills pack.

### Related tasks kept in their own epics (E15 depends on them)
T7.1 #238, T7.2 #239, T7.4 #241, T7.6 #243, T7.38 #247, T7.40 #248, T7.9 #272, T7.29 #250, E3-7 #150, E3-10..13 #154 #155 #157 #158, E1-4 #131, E1-9 #138, T10.1 #343, T10.3 #345, E5-10 #177, T8.25 #276, T8.27 #277, RD-1 #570, RD-3 #572, RD-4 #573. Linked by amendment only: T7.13 #258, E13-6 #410, E9-10 #329.

### Owner rulings
- 2026-10-18: E15 is P0 and is the pack right after v0.5 ("v0.6 AI design"); its existing dependencies move into that pack.
- Full scope, never MVP; packs only order the work.
- Auto-fix: safe mechanical fixes apply automatically with a diff report; configurable to off or report-only.
- Skills: adopt MIT jakubkrehel/skills with attribution; write our own; skip anything without a licence.
- Eval: never client files; open kits and self-generated large files only.
- R4.13: all AI goes through the user's agent CLI; no bundled model.
- Parallel agents are hard-locked to their own frames (RD-3)."""

AMEND = [
    (238, "E15 extension (2026-10-18, moved to v0.6 AI design): (1) `region` parameter crops to a page-space rectangle or a node set; (2) `max_long_edge` (default 1024) downsizes instead of failing, default scale 0.5, and the reply states the scale used. Acceptance: a region crop of a 4000 px frame returns an image whose long edge is 1024 and whose content matches the requested rectangle."),
    (239, "E15 extension (2026-10-18, moved to v0.6 AI design): each node row carries computed bounds, the in-progress flag (E15-7) and its lint issue count when the linter (E15-9) has run. Acceptance: the outline of a fixture with clipped text shows a non-zero issue count on that node."),
    (272, "E15 extension (2026-10-18, moved to v0.6 AI design): results are instance-ready: component key, variant property schema, slot definitions with recommended components, and `var:` paths for variables, usable verbatim in NodeSpec instance and var references (E15-1, E15-3). Acceptance: a result pasted into apply_spec places a correct instance without edits."),
    (248, "E15 link (2026-10-18): guide prose and archetype briefs are served from the E15 data files (E15-18 archetypes, E15-21 surfaces) so there is one source; archetype parameters are validated by the E15 engine and compiled by designos_apply_style (E15-19)."),
    (572, "E15 link (2026-10-18, moved to v0.6 AI design): frame ownership enforcement is reused for region-scoped repair leases (E15-35) and variant agents (E15-36); the engine records the authoring agent run per node so critique can refuse self-review (E15-34)."),
    (573, "E15 link (2026-10-18, moved to v0.6 AI design): the bundled skills are the E15 skills pack (E15-46 adopted MIT skills with attribution, E15-47 our own skills)."),
    (258, "E15 link (2026-10-18): the script tool also returns the per-call lint issues block (E15-12), uses the named error contract with safe_to_retry (E15-5) and honours the in-progress flag (E15-7)."),
    (276, "E15 extension (2026-10-18, moved to v0.6 AI design): the capture also records, per element, the CSS custom properties behind computed values and framework component-name hints, consumed by E15-43 and E15-44."),
    (410, "E15 link (2026-10-18): site pixel diffs reuse the verified-codegen harness from E15-45 instead of a second browser diff."),
    (329, "E15 link (2026-10-18): the template browser also lists the E15 surface templates and slide layout contracts (E15-22)."),
    (154, "E15 link (2026-10-18): expose the exact-match matcher as a shared library function; the E15 linter (E15-11) and auto-fix (E15-13) call it instead of re-implementing it."),
]


def gh_milestones(numbers):
    """Fetch milestone titles for issues outside the task lists (live read, no writes)."""
    out = {}
    for n in sorted(numbers):
        r = subprocess.run(["gh", "issue", "view", str(n), "--repo", "jangtrinh/design-os-studio", "--json", "milestone,state",
                            "-q", '(.milestone.title // "") + "|" + .state'], capture_output=True, text=True)
        title, _, st = r.stdout.strip().partition("|")
        out[n] = (title, st)
    return out


def main():
    iss2id = {v: k for k, v in FT.items()}
    byid = {t["id"]: t for t in master["tasks"]}
    packs = [p["title"] for p in master["packs"]]
    order = [PACK if p == OLD_V06 else p for p in packs]
    pidx = {p: i for i, p in enumerate(order)}

    # moves: E15 existing deps + transitive closure; RD issues; old v0.6 contents
    seeds = sorted({int(d[1:]) for t in TASKS for d in t["deps"] if d.startswith("#")})
    moves, seen, stack = [], set(), []
    for n in seeds:
        if n in iss2id:
            stack.append(iss2id[n])
        elif n in RD.values():
            rd = next(k for k, v in RD.items() if v == n)
            moves.append({"issue": n, "task_id": rd, "from": "v0.19 Desktop MCP" if rd in ("RD-3", "RD-4") else "v0.4 Layout 2.0", "to": PACK})
    while stack:
        tid = stack.pop()
        if tid in seen:
            continue
        seen.add(tid)
        t = byid[tid]
        for d in t["deps"]:
            if d.startswith("#") and int(d[1:]) in iss2id:
                stack.append(iss2id[int(d[1:])])
            elif not d.startswith("#"):
                stack.append(d)
    new_pack = {t["id"]: t["pack"] for t in master["tasks"]}
    for tid in sorted(seen, key=lambda x: (packs.index(byid[x]["pack"]), x)):
        if pidx[byid[tid]["pack"]] > pidx[PACK]:
            moves.append({"issue": FT[tid], "task_id": tid, "from": byid[tid]["pack"], "to": PACK})
            new_pack[tid] = PACK
    moves = [m for m in moves if not (m["task_id"] == "RD-1")]  # RD-1 is in v0.4, already earlier
    for t in master["tasks"]:
        if t["pack"] == OLD_V06:
            moves.append({"issue": FT[t["id"]], "task_id": t["id"], "from": OLD_V06, "to": V07})
            new_pack[t["id"]] = V07
    old_v06_issues = {528: "milestone issue", 521: "milestone issue", 520: "milestone issue", 39: "milestone issue", 38: "milestone issue", 37: "milestone issue"}
    for n in sorted(old_v06_issues):
        moves.append({"issue": n, "task_id": f"#{n} (pre-roadmap issue)", "from": OLD_V06, "to": V07})

    # pack-order check across master + E15 + RD after the moves
    issue_pack = {FT[k]: p for k, p in new_pack.items()}
    issue_pack.update({RD["RD-1"]: "v0.4 Layout 2.0", RD["RD-2"]: "v0.24 Custom shaders", RD["RD-3"]: PACK, RD["RD-4"]: PACK, RD["RD-5"]: "v0.47 Code layers"})
    for n in old_v06_issues:
        issue_pack[n] = V07
    e15_pack = {t["id"]: PACK for t in TASKS}
    all_tasks = [(tid, p, byid[tid]["deps"]) for tid, p in new_pack.items()] + [(t["id"], PACK, t["deps"]) for t in TASKS]
    external = {int(d[1:]) for _, _, deps in all_tasks for d in deps if d.startswith("#") and int(d[1:]) not in issue_pack}
    ext_ms = gh_milestones(external)
    unknown, violations = [], []

    def dep_pack(d):
        if d.startswith("#"):
            n = int(d[1:])
            if n in issue_pack:
                return issue_pack[n]
            title, st = ext_ms.get(n, ("", ""))
            if st == "CLOSED":
                return "closed"
            if title in pidx:
                return title
            unknown.append((n, title, st))
            return None
        return new_pack.get(d) or e15_pack.get(d)

    for tid, p, deps in all_tasks:
        if p not in pidx:
            continue
        for d in deps:
            dp = dep_pack(d)
            if dp in (None, "closed"):
                continue
            if dp not in pidx:
                continue
            if pidx[dp] > pidx[p]:
                violations.append(f"{tid} ({p}) -> {d} ({dp})")
    # E15 internal ordering must be acyclic
    ids = {t["id"] for t in TASKS}
    for t in TASKS:
        for d in t["deps"]:
            if not d.startswith("#") and d not in ids:
                violations.append(f"{t['id']} -> unknown {d}")
    visiting, done = set(), set()

    def dfs(x):
        if x in done:
            return
        if x in visiting:
            violations.append(f"cycle at {x}")
            return
        visiting.add(x)
        for d in next(t for t in TASKS if t["id"] == x)["deps"]:
            if d in ids:
                dfs(d)
        visiting.discard(x)
        done.add(x)

    for t in TASKS:
        dfs(t["id"])

    out = {
        "epic": {"code": "E15", "title": "[Epic] E15 AI design", "body": EPIC_BODY},
        "pack": {"title": PACK, "replaces_slot_of": OLD_V06,
                 "description": "Headline: AI design (E15, P0, owner ruling 2026-10-18), right after v0.5. Grounded agent writes, per-call geometric lint and auto-fix, styles compiled to variables, critique, variants, assets through the agent CLI, code-token round trip, designos-eval with a CI gate, skills pack. Pulled-forward deps: agent eyes and context (T7.1-T7.6, T7.9, T7.29, T7.38, T7.40), variables (E1-4, E1-9), branches core (T10.1-T10.3), web capture core (T8.25, T8.27), agent image bridge (E5-10), RD-3, RD-4."},
        "tasks": TASKS,
        "amend": [{"issue": n, "add_text": s} for n, s in AMEND],
        "moves": moves,
        "pack_check": {"violations": violations, "unknown_external": unknown, "external_checked": len(external)},
    }

    texts = [out["epic"]["title"], out["epic"]["body"]] + [t["title"] for t in TASKS] + [t["body"] for t in TASKS] + [a["add_text"] for a in out["amend"]] + [out["pack"]["description"]]
    hits = [x[:80] for x in texts if BANNED.search(x)]
    if hits:
        sys.exit(f"BANNED NAME HIT: {hits}")
    (HERE / "e15-tasks.json").write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
    print(f"tasks={len(TASKS)} amend={len(AMEND)} moves={len(moves)} banned_hits=0")
    print(f"pack check: {len(violations)} violations; {len(external)} external issue deps checked; unknown={unknown}")
    for v in violations:
        print("  VIOLATION", v)
    if violations:
        sys.exit(1)


if __name__ == "__main__":
    main()
