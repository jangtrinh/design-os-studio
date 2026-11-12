# #1446 Colour picker: gradient stops bound through a linked fill style are not locked

| Field | Value |
|---|---|
| Issue | #1446 (milestone v0.8). From the #1425 round 4 review ("Not verified") |
| Still real? | Yes, and reachable through import, not through the UI. Evidence below. The issue asked to close it if unreachable; it is reachable |
| Size | S: about 3 h |
| Tier | Argon (one file plus tests, machine-checkable) |
| Depends on | #1443 / PR #1674 (same file `crates/designos-app/src/panels/var_bind.rs`; this spec relies on its cache invalidation). #1630, an unrelated contributor draft, is a file-order constraint only: rebase after whichever of #1630/#1674 merges first |
| Ready after owner answers? | Yes |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

When a layer's gradient fill comes from a fill style whose stop is bound to a variable, the colour picker shows that stop's binding pill and locks the stop, as it does for a stop bound on the layer itself. The picker can no longer write a colour the canvas would ignore.

## 2. Re-anchor (origin/main)

- FACT: the renderer resolves style bindings for a linked node. `resolve_node` walks `style_refs` slots, including `"fill"`, and applies `StyleMeta.bindings` unless the node has its own binding (`crates/designos-doc/src/variables_apply.rs:562-594`). `Document::effective_binding` gives the same answer per key (`crates/designos-doc/src/variables.rs:275-294`).
- FACT: the picker reads node bindings only.
  - Keys come from `node.variables.bindings` (`crates/designos-app/src/panels/var_bind.rs:343-350`).
  - The lock and pill go through `bound_var`, then `doc_bound_var`, which reads `doc.get(node)?.variables.bindings` (`:522-530`).
- FACT, reachability. The **UI cannot** create it: the style editor shows only a "Type" row for gradient fills (`crates/designos-app/src/panels/style_edit.rs:215-220`), and there is no stop bind there.
- FACT, reachability. **Import can** create it:
  - style metadata bindings are parsed from the style node with `parse_node_variables` (`crates/designos-fig/src/rest_import.rs:3833`);
  - the kiwi path writes `fillStops` from paint `stopsVar` (`crates/designos-fig/src/fig.rs:1997-2027`);
  - the parser maps `fillStops/i/j` (`crates/designos-fig/src/variables.rs:288-309`).
- FACT, reachability. MCP batch `set_style` keeps existing style bindings (`crates/designos-mcp/src/tools/batch.rs:1643`).
- FACT: the doc layer already tests style-level stop bindings (`crates/designos-doc/src/styles_ops.rs:3311-3328`).
- INFERENCE: whether the kiwi function at `fig.rs:1997` runs for *style* nodes as well as layers is not traced. AC-1 settles it with a fixture.

## 3. Scope and rules

1. The picker's stop key set comes from **effective** bindings:
   - the node's own `fills/N/stops/*` / `strokes/N/stops/*` keys;
   - plus, when the node links a fill style (or stroke style), that style's `fills/N/stops/*` keys, mapped to `strokes/...` for the stroke slot, the same way `effective_binding` maps them.
2. Lock and pill use `effective_binding`. Builder decision: add `doc_effective_var(doc, node, key)` next to `doc_bound_var` and use it at the lock, pill and write-back sites. Keep `doc_bound_var` for node-only callers. The one predicate "variable exists" for lock and write-back stays (the #1425 r3 ruling).
3. **Detach on a style-sourced pill.** Follow the rule the picker already applies when a style-linked paint is edited. The builder locates it (fill style overrides, #1464 / PR #1484) and cites it in the PR. Controller decision, recorded: the pill does **not** unbind the style's variable, which would change every layer using the style. It acts like any edit of a style-linked paint on this layer.
4. The tooltip on a style-sourced pill names the variable and adds "via <style name>". Copy follows `design/ux-writing.md` (about eight words).

## 4. Files likely touched

`crates/designos-app/src/panels/var_bind.rs` (keys `:343-350`, `doc_bound_var` `:522`); tests in the same module; a new fixture test in `crates/designos-fig/tests/` for AC-1 (test-only).

## 5. Acceptance criteria

- **AC-1** (reachability, fixture) A `.fig` fixture holds a gradient fill style whose stop 1 is bound to a colour variable, linked on a rectangle. After import, `style_meta[key].bindings` contains `fills/0/stops/1`. If this fails, the importer gap is a separate bug: file it, and build AC-2 from the doc-level setup instead.
- **AC-2** Open the picker on that rectangle's fill: stop 1 shows the pill with the variable name and is locked. Typing a hex value does not change the stop. Red on main.
- **AC-3** A node-level binding on the same stop wins over the style's, in the pill and in the canvas (`resolve_node` order).
- **AC-4** Stroke slot: the same setup through a stroke style gives the same lock.
- **AC-5** Mutation check: revert to `doc_bound_var`, and AC-2 fails.

## 6. Test steps

`cargo test -p designos-app --bin designos panels::var_bind`, `cargo test -p designos-fig --test <new fixture test>`, a revert check, then clippy and fmt. Evidence: a terminal capture, plus a 1440 capture of the locked stop with its pill.

## 7. Edge cases

- The style's variable is missing: same as a dangling node binding (the "variable exists" predicate). Not locked, and edits allowed.
- Multi-select where only some nodes link the style: the pill shows "Mixed" (existing logic at `var_bind.rs:381-382`).
- The style is detached while the picker is open: the cache from #1674 must invalidate. The epoch changes, so the lock drops on the next frame.

## 8. Non-goals

- A stop bind entry in the style editor (no issue asks for it).
- Changes to how the importer fills style bindings, beyond filing a bug if AC-1 fails.

## 9. Builder rules

Use `Closes #1446`.
