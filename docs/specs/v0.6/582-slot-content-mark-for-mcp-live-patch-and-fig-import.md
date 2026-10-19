# #582 Mark slot content from MCP, live patches and `.fig` import so a main-component sync keeps it

| Field | Value |
|---|---|
| Issue | #582 (follow-up of #503) |
| Size / priority | M / P1 (silent data loss: an agent's insert vanishes on the next sync) |
| Depends on | none |
| Unblocks | agent edits into instance slots (E15-1 / E15-2 slot fills rely on content surviving a sync) |
| Touches | `crates/designos-doc/src/components.rs` (the slot predicate moved here from `ops.rs`) and new `crates/designos-doc/src/writer_command.rs` (the writer-boundary function shared with #583), `crates/designos-doc/src/live_patch/replay.rs` (call sites `:202`, `:204-216`, `:303`), `crates/designos-mcp/src/engine.rs` (`Engine::exec`, `:415`), `crates/designos-fig/src/rest_import.rs` / `fig.rs` (post-expansion pass), `crates/designos-app/src/ops.rs` (`mark_slot_content_override`, `:426-433`, reuse only). New tests in `crates/designos-doc/src/slot_model_tests.rs`, `crates/designos-mcp`, `crates/designos-fig/tests/fig_slots.rs`. |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue says the MCP insert paths are "around lines 264
and 464"; they are `engine.rs:267` (`designos_apply_spec`) and `:483` (`create`).

## 1. Summary and user story

As a user whose agent adds a layer into an instance slot (or whose `.fig` file carries custom slot
content), I keep that content when the main component is edited later. Today only edits made through
the Studio UI mark the instance slot as holding user content; everything else is removed by the next
main-component sync.

## 2. Current state (FACT)

- A slot the user edited carries `OverrideField::SlotContent` in `component.overrides`; sync skips
  such a slot: `if inode.component.overrides.contains(&OverrideField::SlotContent) { return; }`
  (`crates/designos-doc/src/components.rs:1087-1089`). Without it the slot "mirrors the main
  component's children again, including dropping anything the user added" (comment `:1085-1087`).
- The mark is set by app code only: `App::mark_slot_content_override` (`crates/designos-app/src/ops.rs:426-433`,
  predicate `is_slot_container && (is_linked(parent) || instance_root_of(..).is_some())`), by
  `comp_ops.rs:380-386` (the `+` picker), `canvas/interact.rs` (drag, reorder, duplicate), `clipboard.rs`
  (`place_svg`, `:280-300`), `libraries/mod.rs` (library drop), and by the doc-crate command builders
  `insert_into_slot`, `remove_from_slot`, `reorder_in_slot` (`components.rs:1464-1520`), which only
  the app calls (`comp_ops.rs:408`).
- Writers outside those paths never mark:
  - MCP: `Engine::exec` (`designos-mcp/src/engine.rs:414-419`) runs `Command::Insert` for
    `designos_apply_spec` (`:267`) and `create` (`:483`); `designos_delete_node` runs `Command::Remove`.
  - Live-patch replay: `resolve_insert`, the `Reparent` and `Remove` arms
    (`designos-doc/src/live_patch/replay.rs:202`, `:204-216`, `:303`) return plain commands. Studio
    applies them through `Tx::cmd`, which calls `Command::apply` directly (`designos-app/src/edit.rs:88-93`).
  - Plugins: the only document mutation a plugin can request is an SVG insert
    (`plugins/runtime.rs:19-25`), which goes through `place_svg`, already marked. Nothing to change.
- `Command::apply` cannot be the marking place: the sync itself inserts and removes mirror children
  with the same commands (`components.rs`, `Ctx::push(Command::Insert ..)`), and a mark there would
  flag every slot after every sync.
- `.fig` import: the importer reads an override entry named `slotContent` when the file lists one
  (`designos-fig/src/rest_import.rs:1346`, `:3179`). `expand_instance` copies main children only into an
  instance that has none (`rest_import.rs:2007-2010`); an instance that arrives with its own children
  keeps them. No code sets `SlotContent` for those. Whether the result survives a sync is what the
  issue calls "unchecked"; the first red test below settles it.
- Custom (non-mirror) content is distinguishable: a mirror child carries `component.source`
  (`expand_instance`, `rest_import.rs:2007`, sets it on every cloned child at `:2080`), user content has
  `source == None`.

## 3. Behaviour rules

1. New pure function in `designos-doc`: `slot_content_marked(doc: &Document, cmd: Command) -> WriterCommand`, one step of the shared writer-boundary function `writer_command(doc, cmd) -> WriterCommand` (`writer_command.rs`, shared with #583's direction step; whichever task merges first creates the file). `WriterCommand { cmd: Command, parts: Vec<Command> }`: `cmd` is what is applied and put on the undo stack (a `Command::Batch` when a step added work, else the original command), and `parts` lists the same commands un-batched, in order, **each independent of the others** (see rule 2).
   For `Command::Insert { parent }`, `Command::Reparent { id, parent }` (destination `parent` and the
   source slot `doc.get(id).parent`) and `Command::Remove { id }` (the removed node's parent), when the
   affected node is an instance slot (the predicate of `ops.rs:427`, moved into `designos-doc` and
   called by the app as well) and does not carry the mark yet, it returns
   `cmd = Command::Batch([Command::Update { slot with SlotContent override }, original])` and
   `parts = [Update, original]`. Otherwise it returns the original as `cmd` with `parts = [original]`. The inverse of the batch restores the slot's previous overrides, so an undo of the
   insert also removes the mark (the plain `Insert` inverse is `Remove`, which would not).
2. Call sites: `Engine::exec` (MCP) wraps every command before `live_ops_for` and `history.exec`, so the
   recorded live ops contain the mark as an `Update` op. **Recording must not clone the document.** `live_ops_for_command` handles a `Batch` by cloning the whole `Document` as a
   scratch copy so later commands see earlier ones (`live_patch/record.rs:42-48`), which on a large file would copy the scene graph for one insert. A new `live_ops_for_independent(doc, parts)` in `record.rs` records
   each part against the unmodified `doc` and concatenates the ops; this is correct because the parts are independent by construction: the slot `Update` changes only the slot node's `component.overrides`
   (an `Update` keeps the node's `children`, `history.rs:118-124`), so the `Insert` / `Reparent` / `Remove` that follows reads the same sibling list either way. `Engine::exec` records with
   `live_ops_for_independent(&doc, &w.parts)` and executes `w.cmd`; the general `Batch` path with the scratch clone stays for batches that are not built by `writer_command`. The replay functions wrap the `Insert`, `Reparent` and
   `Remove` they produce, so a patch from an older MCP build is marked in Studio as well. Marking an
   already-marked slot is a no-op.
3. The existing app call sites keep working unchanged (idempotent). They are not removed in this task.
4. `.fig` import: after instances are expanded and ids are final, one pass marks every instance slot
   container (slot container inside an instance, `is_slot_container` and `instance_root_of`) that has
   at least one direct child with `component.source == None` (custom content). A slot whose children
   are all mirrors is left unmarked so a later main-component edit still reaches it.
5. A main-component sync, an `Update` that only changes the slot's own fields, and the `Reset` action
   (which clears the override on purpose, comment at `components.rs:1085`) never go through this wrapper.
6. Direct users of the raw `Document::insert` / `reparent` (tests, importers building a document) are
   not wrapped; they are not editor writers.

## 4. Data model / schema changes

None. `OverrideField::SlotContent` already exists. The `.designos` format is unchanged; documents
written before the fix keep their unmarked slots (see edge case 6).

## 5. Contracts

- `designos_doc::slot_content_marked` is a new public function (called by MCP, replay, tests).
- MCP tools keep their schemas. Behavioural contract added to the tool docs of
  `designos_apply_spec`, `designos_create_node`, `designos_delete_node`: "content added to, removed
  from or moved in an instance slot is kept when the main component changes".
- The live patch format (`LIVE_PATCH_VERSION`, `live_patch/mod.rs:19`) does not change; the mark
  travels as a normal `Update` op.

## 6. File-format mapping

| Format | Mapping |
|---|---|
| `.designos` | the mark is `component.overrides` containing `SlotContent` (existing field) |
| `.fig` export | `OverrideField::SlotContent` is written as the override field `SLOT_CONTENT_ID` (`designos-fig/src/fig_export.rs:1147`); no change |
| `.fig` import | new post-expansion marking pass (rule 4); the explicit `slotContent` override entry keeps working |

## 7. UI

None. The existing Components panel row "slot content" (`panels/components.rs:363`) shows the override
for MCP-written content as it does for UI-written content (check in the visual evidence).

## 8. Edge cases

1. MCP inserts into a slot of an instance whose slot is empty in the main: marked; survives sync.
2. MCP inserts into a slot that is a plain frame (not a slot container) of an instance: not marked
   (predicate false); unchanged behaviour.
3. MCP removes a default mirror from an instance slot: marked; the sync no longer re-adds it
   (matches `remove_from_slot` in the UI).
4. Undo of the MCP insert in Studio: slot returns to unmarked together with the content.
5. Two inserts in one `designos_apply_spec` call: the second sees the slot already marked and does not
   add a second `Update`.
6. A file already saved with unmarked custom content (written by an MCP build before this fix): not
   repaired automatically (no reliable signal that the content was intended to persist; the user's
   first sync already removed it in most cases). Listed as a known limit, not an owner question.
7. `.fig` slot with only mirror children plus a hidden mirror: not marked.
8. An instance nested inside another instance's slot: the predicate walks `instance_root_of` as the app
   does.

## 9. Non-goals

- Marking on `Document::insert` / `reparent` themselves.
- Repairing already saved files.
- Changing what sync does with an unmarked slot.
- Per-slot `min_children` / `max_children` validation for MCP (E15-2 owns slot-fill validation).

## 10. Test plan (red first)

Seams: `designos-doc` unit tests (fixture builders in `slot_model_tests.rs`), an MCP engine test, and a
`.fig` round trip in `designos-fig/tests/fig_slots.rs` (hand-written content-free fixture, as that file
already does).

| Case | Steps | Expected (fails before) |
|---|---|---|
| `mcp_insert_into_instance_slot_survives_main_edit` | engine: `designos_create_node` with `parent_id` = instance slot; edit a main-component child; `sync_instances` | inserted layer still in the slot; slot has `SlotContent` |
| `mcp_apply_spec_into_instance_slot_survives` | same through `designos_apply_spec` | same |
| `mcp_delete_default_mirror_is_kept_deleted` | delete a mirror in an instance slot, sync | stays deleted |
| `live_patch_insert_is_marked_in_studio` | replay a `LiveOp::Insert` (no mark op) into an instance slot | slot marked; survives sync |
| `undo_of_marked_insert_clears_the_mark` | insert via wrapper, undo | slot back to its old overrides |
| `idempotent_second_insert` | two inserts in one batch | one `Update` only |
| `recording_a_marked_insert_does_not_clone_the_document` | MCP insert into an instance slot of a 100,000-node synthetic document | ops equal those of the batch path (`live_ops_for_command` on the same batch) and no document clone is made |
| `fig_instance_slot_with_custom_children_survives_main_edit` | import a fixture whose instance slot carries custom children, edit the main, sync | custom children kept (first, run it as-is to learn if import already marks; the test stays either way) |
| `fig_mirror_only_slot_is_not_marked` | fixture with mirrors only | slot unmarked; main edit propagates |
| `sync_does_not_mark` | run `sync_instances` on a marked-free instance | no slot gains the mark |

Run: `cargo test -p designos-doc`, `cargo test -p designos-mcp`, `cargo test -p designos-fig --test fig_slots`.
Evidence: red/green terminal capture, plus a Studio screenshot of an MCP-inserted layer still in the
instance slot after a main-component edit (use the stub agent harness in `live_sync_tests.rs`).

## 11. Performance budgets

The wrapper does one `HashSet`/`BTreeSet` lookup and one node lookup per command, O(1), and recording does not clone the document (rule 2); a test asserts that no `Document` clone happens on an MCP insert into a 100,000-node document (clone counter in the test build, or a timing bound of under 5 ms). The `.fig`
pass visits instance slot containers once: O(instances) with an early exit per slot; no measurable
change on the large fixture import (record the import time before and after in the PR, budget +1%).

## 12. Dependencies and build order

Independent. Serialize with other edits to `components.rs` and `replay.rs` (hot files for E15-1 / E15-2:
rebase rather than hand-merge).

## 13. Owner rulings

- #503 ruling: edits to instance slots must not detach the instance and must survive main edits.
- Nothing vanishes silently (delivery law 1): content an agent added must not disappear without a record.
- Fix at the shared layer (delivery law 4), so every writer gets the behaviour.
