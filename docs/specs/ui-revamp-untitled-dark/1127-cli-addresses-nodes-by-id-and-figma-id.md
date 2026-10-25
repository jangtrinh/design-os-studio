# O The CLI addresses a node by id or Figma id, and never silently selects nothing

| Field | Value |
|---|---|
| Issue | #1127 (epic #1118, spec key O) |
| Size | M. One to two days: one shared resolver module, the GUI and agent-CLI call sites, an exit path for screenshots, `--zoom fit`, the tests. |
| Depends on | none |
| Unblocks | The R3 dogfood loop: specs A, B and C read kit frames by Figma id. Spec L's capture cases and spec M's README commands can name nodes exactly. |
| Touches | New `crates/designos-app/src/select_target.rs`<br>`crates/designos-app/src/cli.rs`:<br>- `select` doc `:12-13`<br>- `--select` parse `:224`<br>- `--zoom` parse `:255-258`<br>- `USAGE` `:326-338`<br>- tests `:340+`<br>`crates/designos-app/src/shell.rs`:<br>- GUI `--select` `:175-186`<br>- `startup_zoom` `:206`<br>`crates/designos-app/src/canvas/mod.rs:361-373` (first fit)<br>`crates/designos-app/src/main.rs`:<br>- `resolve_select_node` `:173-205`<br>- agent CLI uses `:261`, `:266`, `:276-277`, `:330-333`, `:423-426`, `:441-466`<br>- screenshot exit `:150-156`<br>- tests `:470+`<br>`crates/designos-app/src/app.rs` (`startup_zoom`, `:185`) |

Code baseline: `origin/main@ec47da26`. Re-anchor every cited line against HEAD and report drift before writing code. Scratch evidence for this spec, outside the repo: `$SCRATCH/agents/w5/o/`. `$SCRATCH` is the controller's scratchpad, `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As an agent working on a `.fig` through Studio's CLI, I pass `--select fig:5256:372339`, the Figma id I read from the file, or `--select id:4`, the Studio id `--inspect-json` printed. I get exactly that node, or a non-zero exit with a message that says what was searched. A command never acts on a different node than the one I named. With `--zoom fit`, a screenshot frames the node I selected.

## 2. Current state (FACT unless labelled)

**Two resolvers that disagree:**
- **GUI and `--screenshot`** (`shell.rs:175-186`): an exact layer name only. It looks first among page children (screens), then any node, in `doc.nodes` order, which is a `BTreeMap` by `NodeId` (`designos-doc/src/lib.rs:179`). A numeric id or a Figma id matches nothing. When nothing matches, the selection stays empty, nothing is printed, and the exit code is 0.
- **Headless agent CLI** (`main.rs:173-205`, `resolve_select_node`), in this order:
  1. a number that is an existing `NodeId` (`:178`)
  2. a page named exactly (`:181-184`)
  3. a page child named exactly (`:185-189`)
  4. a page-walk search for an exact or case-insensitive name (`:192-204`)

  It returns `None` when nothing matches. It has no Figma-id path.
- `--select` is documented as a name only: `cli.rs:1`, `:12-13` "Select the first layer with this name", and `USAGE` `[--select LAYER|none]`.

**Silent fallbacks when the agent CLI resolves `None`:**
- `--apply-spec`: inserts into the **first page and saves the file** (`main.rs:266`). This mutates the document.
- `--add-comment`: anchors to the first page, saves (`main.rs:276-277`).
- `--export-spec`: exports the first screen (`main.rs:330-333`).
- `--codegen`: generates code for the first screen (`main.rs:423-426`).
- `--inspect-json` ignores `--select` completely (`main.rs:441-466`), and its screens list carries no Figma id.
- Errors returned from `run_agent_cli` already exit 2 (`main.rs:122-127`).

**Other name lookups.** `--edit-text` prints `designos: no text layer named '…'` and carries on, exit 0 (`shell.rs:220-237`). `--layout-grid`, `--dev-demo` and the inspector popups of `--panel` act on the selection (`shell.rs:207-217`; `cli.rs:126`), so a missed `--select` spreads to them.

**Figma ids.**
- `Node.source_id: Option<String>` holds the imported Figma id, for example `"12:34"` (`designos-doc/src/node.rs:67-69`; `lib.rs:121-122`). It is set by the REST/`.fig` importer (`designos-fig/src/rest_import.rs:818`, `:847`, `:927`, `:952`).
- Instance children get `I<instance id>;<child id>` (`:2280`). In files with at least 10,000 nodes and no REST overrides, the fast clone path **removes** `source_id` from instance children (`:2223-2224`, `:2276-2279`).
- So in the kit, sublayers inside instances have no Figma id. Only nodes that exist as their own records in the file keep one.
- The `.fig` exporter writes `source_id`, or `9000:<NodeId>` when there is none (`designos-fig/src/export.rs:264-266`).
- No code looks up a node by `source_id` except one-off scans (`comments/import.rs:58`, `lint/rules/detached.rs:22-55`).

**Lazy pages.** Large `.fig` imports defer instance expansion per page (`Document::ensure_page_hydrated`, `designos-doc/src/lib.rs:633-645`; `hydrate_all_pages` `:647`). A node inside an unexpanded instance is not in `doc.nodes` until its page is hydrated. The GUI resolver hydrates the page of a node it finds (`shell.rs:180-182`), but it cannot find a node that does not exist yet.

**Framing.**
- At first fit, `--zoom <pct>` calls `zoom_on_content`, which centres on the selection (`canvas/mod.rs:361-364`, `ops.rs:1864-1869`).
- Without `--zoom`, the canvas runs `fit_all`, the page bounds (`:372`).
- `fit_selection` exists (`ops.rs:1850-1857`, `View::fit` with `FIT_PADDING`, `app.rs:43-51`), but nothing on the CLI reaches it.
- So `--select X --screenshot` without `--zoom` shows the whole page.

**Reproduction (scratch, 2026-10-25; the document is two frames in a board).** Built with `--apply-spec two-cards.spec.json`. Ids: 1 page, 2 `Board`, 3 `Card A`, 4 `Title`, 5 `Card B`, 6 `Title`.

```text
$ designos two-cards.designos --inspect-json
{ "kind": "Design", "name": "two-cards", "node_count": 6,
  "pages": [ { "child_count": 1, "id": 1, "name": "Page 1",
      "screens": [ { "dev_status": null, "height": 500.0, "id": 2, "kind": "Frame", "name": "Board", "width": 900.0 } ] } ],
  "unresolved_comments": 0 }
$ designos two-cards.designos --select "Card Z" --export-spec spec-missing.json
{ "exported_node_id": 2, "path": "spec-missing.json" }          # exit=0: exported "Board", not "Card Z"
$ designos two-cards.designos --select 4 --export-spec spec-id4.json
{ "exported_node_id": 4, "path": "spec-id4.json" }              # agent CLI accepts a bare id
$ designos two-cards.designos --select "Card Z" --inspect-json   # identical output; --select ignored, exit=0
$ designos two-cards.designos --select "Card Z" --codegen css
width: 900px;
height: 500px;
background: #F5F5F5;                                          # exit=0: CSS of "Board"
```

The mutating fallback, the screenshot path and a Figma id. Same session, with the `designos` binary installed from HEAD into scratch:

```text
$ designos two-cards-copy.designos --select "Card Z" --apply-spec probe.spec.json
{ "root_node_id": 7, "saved": true }                           # exit=0
$ designos two-cards-copy.designos --inspect-json | grep -E '"name"|"id"'
  "name": "two-cards", "id": 1, "name": "Page 1", "id": 2, "name": "Board", "id": 7, "name": "Probe"
                                                               # Probe became a page-level screen; file saved
                                                               # shasum two-cards.designos 1f38c0c1… vs copy 494490ed…
$ designos two-cards.designos --select "Card Z" --size 1000x700 --no-toast --screenshot shot-select-missing-name.png   # exit=0
$ designos two-cards.designos --select 5 --size 1000x700 --no-toast --screenshot shot-select-id-5.png               # exit=0
$ cmp shot-select-id-5.png shot-select-missing-name.png && echo IDENTICAL
IDENTICAL                                                      # an id selects exactly as much as a missing name: nothing
$ designos two-cards.designos --select "Card B" --size 1000x700 --no-toast --screenshot shot-select-name-card-b.png    # exit=0
        # viewed: Card B is selected (handles, inspector shows Card B) but the canvas is at page fit, 37%; Card B is a small rectangle
$ designos two-cards.designos --export-fig two-cards.fig
{ "bytes": 22969, "exported_fig": true, "omitted_extensions": [], "path": "two-cards.fig" }
$ designos two-cards.fig --select 9000:5 --export-spec spec-figid.json   # 9000:<id> is the id export.rs:265 writes
{ "exported_node_id": 2, "path": "spec-figid.json" }          # exit=0: a Figma id falls back to the first screen
```

## 3. Behaviour rules

1. **One resolver.** `select_target::resolve(doc: &mut Document, raw: &str) -> Result<Target, SelectError>`. The GUI or screenshot path (`shell.rs:175`) and the agent CLI (`main.rs:261`) both call it. `resolve_select_node` and the inline GUI lookup are deleted.
2. **Grammar** (case-sensitive prefixes):

   | Input | Meaning |
   |---|---|
   | `none` | no selection, explicitly (unchanged) |
   | `id:<n>` | the node whose `NodeId` is `n` (decimal) |
   | `fig:<id>` | the node whose `source_id` equals `<id>` exactly, for example `fig:12:34` or `fig:I12:34;56:78` |
   | `name:<text>` | a layer name taken literally, for a name that starts with `id:`, `fig:` or `name:` |
   | anything else | a layer name |
3. **Name order**, which is today's agent-CLI order made the single rule:
   1. a page with that exact name
   2. a page child (screen) with that exact name, pages in document order, children in order
   3. any layer with that exact name, in page-walk order
   4. the same walk, case-insensitive

   The first hit wins. The GUI moves from `NodeId` order to page-walk order. That changes which of several same-named layers wins, to the one the user sees first in the Layers panel. No test asserts the old order (closing check: `cargo test -p designos-app select`).
4. **Bare numbers (compatibility).** A bare value that matches no layer name, but parses as an existing `NodeId`, selects that node. stderr then prints `designos: --select 4 matched no layer name; selected node id 4 (use --select id:4)`. Names are checked first, so a layer literally named `4` still wins. The agent CLI today checks the id first; the PR body notes the change.
5. **No match is an error, never a fallback.**
   - The message names the selector and what was searched: `--select "Card Z" matched no node (searched 6 layers on 1 page by name; use --inspect-json to list screens, or id:<n> / fig:<id>)`.
   - **Agent CLI.** `run_agent_cli` returns `Err` before any action runs, which exits 2 (`main.rs:122-127`). `--apply-spec`, `--add-comment`, `--export-spec`, `--codegen` and `--inspect-json` never fall back to the first page or screen when `--select` was given. Without `--select` they keep their current defaults.
   - **`--screenshot`.** `Shell::new` records the miss and requests `ViewportCommand::Close` on the first frame. `main.rs` prints the message and exits 2 before its `SHOT_OK` check. No PNG is written.
   - **Interactive launch** (no `--screenshot`, no agent command). The window opens on the file, and an error toast shows the message. It is also printed to stderr. A person is not thrown out of the app for a typo.
6. **Lazy pages.** When the first pass finds no match and the document has pages that are not hydrated, the resolver calls `hydrate_all_pages()` once and searches again. The miss message then says `(all pages expanded)`.
7. **Several matches.**
   - For names, the first hit wins (rule 3), and stderr notes `matched 3 layers; selected id 12 (first in page order; use id:<n> for another)`.
   - For `fig:`, several nodes with the same `source_id` is an error listing up to 10 ids, exit 2, because a Figma id is meant to be unique.
   - `id:` cannot match twice.
8. **Pages.** A selector that resolves to a page switches to that page (`app.page`) and selects nothing, in both GUI and screenshot mode. The agent CLI uses the page as the target, which is today's behaviour for `--apply-spec` and `--add-comment`. For `--export-spec` and `--codegen` it uses the page node.
9. **`--inspect-json` reports the target.** With `--select`, the output gains `"selection": {"id", "name", "kind", "page_id", "source_id", "x", "y", "width", "height"}`, in page coordinates. Every screen entry gains `"source_id"` (null when absent), so an agent can map a Figma id to a Studio id.
10. **`--zoom fit`.** `--zoom` accepts `fit` as well as a percentage. `startup_zoom` becomes `enum StartupZoom { Percent(f32), Fit }`. At first fit, `Fit` calls `fit_selection()`, or `fit_all()` when nothing is selected (`canvas/mod.rs:361-373`). The default stays page fit when `--zoom` is absent, so existing capture commands do not reframe.

## 4. Data model / schema changes

None to documents. New in-memory types: `select_target::{Target, SelectError}`, where `Target` is `{ node: Option<NodeId>, page: NodeId, matched_by: By, ambiguous: usize }`, plus `StartupZoom`.

## 5. Contracts

- `pub enum By { Id, Figma, Name, NameCaseInsensitive, BareId }` and `pub fn resolve(doc: &mut Document, raw: &str) -> Result<Target, SelectError>`. `SelectError` implements `Display` with the message of rule 5. `select_target.rs` stays under 200 lines.
- **CLI**. `USAGE` and the help line become:

  ```text
  --select id:<n>|fig:<figma id>|name:<text>|LAYER|none
  --zoom PERCENT|fit
  ```

  The `cli.rs:1` doc comment and the `Options.select` doc are updated to match.
- **Exit codes**:
  - 2: no match, ambiguous `fig:`, a malformed `id:` value (for example `id:abc`), or a parse error
  - 1: screenshot not captured (unchanged)
  - 0: success
- **`--inspect-json` with `--select`** (example):

  ```json
  {"name":"two-cards","kind":"Design","node_count":6,"unresolved_comments":0,
   "pages":[{"id":1,"name":"Page 1","child_count":1,"screens":[{"id":2,"name":"Board","kind":"Frame","width":900.0,"height":500.0,"dev_status":null,"source_id":null}]}],
   "selection":{"id":5,"name":"Card B","kind":"Frame","page_id":1,"source_id":null,"x":480.0,"y":200.0,"width":360.0,"height":220.0}}
  ```

## 6. File-format mapping

None. `source_id` is read only.

## 7. UI

- Interactive launch with a missed `--select`: one error toast through the existing toast path. The text is rule 5's message, shortened to the UX-writing limits: `No layer matches "Card Z". Check the name, or use id: or fig:`.
- No new controls. `es-designer` is not needed. The toast uses the existing component.

## 8. Edge cases

- `--select ""` or whitespace only: a parse error, exit 2 (today it is silently ignored, `main.rs:175-176`).
- `id:` with a number that does not exist: no match, exit 2. `id:-1` or `id:1.5`: malformed, exit 2.
- `fig:` on a non-Figma document (`.designos` built in Studio): no `source_id` anywhere, so no match. The message adds `(this document has no Figma ids)` when no node in the document has a `source_id`.
- `fig:` naming an instance sublayer in a large kit: the fast clone path removed its id (section 2), so no match. The message adds `(instance sublayers in files over 10,000 nodes keep no Figma id; select the instance and use id:<n>)` when the document has more than 10,000 nodes.
- A name that matches only after hydration: found after rule 6, with the page hydrated.
- `--select` together with `--home`: the home screen has no document, so `--select` is a parse error ("--select needs a file").
- `--edit-text` keeps its own name lookup (text layers only). Its miss becomes an exit 2 in screenshot mode, for the same reason as rule 5. The PR body names it.
- `--zoom fit` with `--present`: present mode ignores `startup_zoom` today, so `fit` is ignored too.

## 9. Non-goals

- Selecting several nodes (`--select a,b`).
- XPath or query selectors on the CLI (use `designos_query_xpath` in the MCP).
- Keeping Figma ids on fast-cloned instance children (a `designos-fig` import change with memory cost; follow-up if needed).
- Changing the default framing of `--screenshot` without `--zoom`.
- The MCP tools, which address nodes by `node_id` already.

## 10. Test plan (red first)

| Case | Where | Expected | Fails before |
|---|---|---|---|
| `parses_select_prefixes_and_zoom_fit` | `cli.rs` tests | `id:5`, `fig:I1:2;3:4`, `name:id:3` are kept verbatim; `--zoom fit` parses; `--select ""` errors | yes (`fit` and empty) |
| `resolve_by_id_fig_and_name` | `select_target.rs` | each grammar row resolves on a fixture with `source_id`s set | yes (no module) |
| `name_order_prefers_page_then_screen_then_walk` | `select_target.rs` | same name on a page child and a deeper layer: the page child wins; a case-insensitive match is used only when there is no exact one | yes |
| `bare_number_is_a_name_first_then_an_id` | `select_target.rs` | a layer named `4` beats node 4; `4` with no such name selects node 4 with `BareId` | yes |
| `fig_id_duplicates_are_an_error` | `select_target.rs` | two nodes with the same `source_id`: `SelectError` listing both | yes |
| `miss_hydrates_lazy_pages_once` | `select_target.rs` | a fixture with a page hydrator: a node created by hydration is found after one `hydrate_all_pages` | yes |
| `export_spec_with_unknown_select_fails_and_writes_nothing` | `main.rs` tests | `Err`, no output file | yes (exports node 2) |
| `apply_spec_with_unknown_select_leaves_the_file_unchanged` | `main.rs` tests | `Err`; file bytes identical | yes (inserts into page 1 and saves) |
| `codegen_and_add_comment_with_unknown_select_fail` | `main.rs` tests | `Err` | yes |
| `inspect_json_reports_the_selection_and_screen_source_ids` | `main.rs` tests | `selection.id == 5` for `--select "Card B"`; screens carry `source_id` | yes |
| `select_by_figma_id_in_a_round_tripped_fig` | `main.rs` tests | export a document to `.fig` (`--export-fig`) and reopen it; `fig:<the source_id the reopened node has>` resolves | yes |
| `gui_select_by_id_selects_the_node` | `shell/tests` (`Sim::with(cli::parse([...]))`) | `app.selection == [5]` for `id:5` | yes |
| `gui_select_miss_is_recorded_for_exit` | `shell/tests` | the miss flag is set, with the message | yes |
| `zoom_fit_frames_the_selection` | `shell/tests` | with `--select "Card B" --zoom fit`, the selection's screen rect lies inside `canvas_rect`, and its larger side fills at least 80% of the canvas's matching side | yes |
| `screenshot_exit_code_for_a_select_miss` | `main.rs` (pure fn `exit_code(select_miss, shot_ok)`) | 2 before 1 | yes |

Run:
- `cargo test -p designos-app --bin designos select`
- `cargo test -p designos-app --bin designos cli`
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python3 scripts/zero_fake_ui_gate.py`

Evidence for the PR, a terminal capture on the PR build:
- the five agent-CLI commands of section 2 now exit 2 with the message, and `two-cards-copy.designos` is unchanged (`shasum` before and after)
- `--select fig:<id> --inspect-json` on a copy of the kit, showing `selection` with the Studio id
- `--select "Card B" --zoom fit --screenshot` viewed as an image, framed on Card B
- `--select "Card Z" --screenshot` printing the message, with no PNG (`ls`)

## 11. Performance budgets

- Resolving by `id:` is O(1). `fig:` and name lookups are one page walk, O(n), measured on the kit (207,601 loaded nodes) at under 100 ms in release (closing check: time `--select fig:<id> --inspect-json` on the kit in the PR; INFERENCE from a single linear scan).
- `hydrate_all_pages` runs only after a miss, at most once per invocation. Its cost on the kit is UNKNOWN. Closing check: time `--select "No such layer" --inspect-json` on the kit, recorded in the PR. If it exceeds 30 s, the miss message says so, and the worker reports the time rather than skipping the retry.
- The GUI path runs once at startup. Nothing runs per frame and there is no idle cost (CLAUDE.md section 4).

## 12. Dependencies and build order

Independent of the other specs. It should merge before spec M's captures if M uses `fig:` or `--zoom fit` (it does not need to). Order:
1. `select_target.rs` and its unit tests.
2. The agent-CLI call sites and their tests.
3. `--inspect-json` additions.
4. The GUI call site, the miss flag and the exit path.
5. `--zoom fit`.
6. `USAGE` and docs.
7. Evidence.
8. PR (squash).

## 13. Owner rulings that apply

- Soul and principles: no silent fallbacks.
  - `design/soul.md:17` ("Never silently fall back to `Recents`…", the Home case of the rule).
  - P6 in `design/principles.md` ("never silently falling back").
  - The designer agent's Never list ("silent fallbacks", `.project-agent.md`, the "Project Design Agents" list).
  - The plan calls a missed `--select` a break of this rule (plan.md, dogfood gaps).
- R3 (2026-10-25): Studio improves itself through its own CLI and MCP; no Figma.
- 2026-09-22: visual evidence in every PR. Standing: `main` is protected.

## Decisions on the open questions (writer, 2026-10-25)

1. **Prefix grammar or a separate flag (`--select-id`)?** Prefixes on `--select`. One flag keeps every command that takes `--select` working with ids at once, and `name:` keeps every literal name reachable.
2. **Hard error or warning on a miss?** A hard error (exit 2) for the agent CLI and screenshots, where a script cannot see a warning and acts on the wrong node. A toast plus stderr for an interactive launch, where a person can see and recover.
3. **Frame the selection by default?** No. `--zoom fit` is opt-in, because changing the default would reframe every existing capture command (`scripts/capture-visual-proof.sh`, the README commands, spec L's cases).
4. **Hydrate everything on a miss?** Yes, once. Otherwise a valid name inside an unexpanded instance would be reported missing. The cost is paid only on a miss.

## Open questions for the owner

None. Only the interactive toast is user-visible, and it follows the no-silent-fallback rule.
