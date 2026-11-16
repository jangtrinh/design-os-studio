# #1443 Colour picker: binding context is rebuilt every frame across the whole selection

| Field | Value |
|---|---|
| Issue | #1443 (perf + cosmetic; milestone v0.8). From the #1425 round 3 review, MINOR 2 and 3 |
| Still real? | Perf half: yes. Mixed-mode pill: **fixed on main** by `96c640011` (#1612, ancestor of main), at `var_bind.rs:369-371`. Contributor **draft PR #1674** (NamAIEngineer, head `a89819799`, 1 file, +134/−5) targets the perf half |
| Size | S: about 2 h to review and carry |
| Tier | Review: Grok 4.7 read-only plus a controller check. Fix-up: Grok build-fast (one file, local test). A Grok-built fix-up is reviewed by Claude or Codex, never by Grok. Carry with credit |
| Depends on | None. **#1446 and #1630 touch the same file** (`panels/var_bind.rs`), so merge in this order: #1630 → #1674 → #1446 |
| Ready after owner answers? | Yes |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

With the colour picker open on a solid fill and 5,000 layers selected, frames cost nothing extra. With a gradient open, the binding context is rebuilt only when the selection, the paint or the bindings change.

## 2. Re-anchor (origin/main)

- FACT: `sync_picker_context` (`crates/designos-app/src/panels/var_bind.rs:323-389`) runs every frame. For a Fill or Stroke target it walks every selected node's bindings, building a `Vec<NodeId>`, a `String` prefix and a `BTreeSet` (`:336-351`). It does this whatever the paint kind: only the target is checked (`:327-332`), not whether the paint is a gradient. It ends with `ctx.data_mut(... insert_temp ...)` every frame (`:388`).
- FACT: it also calls `sync_picker_paint_from_doc(app)` first, every frame (`:324`). PR #1674's body does not mention it. Review point §3.3.
- FACT: the mixed-mode swatch shows "uniform name, no swatch" when the same variable resolves differently per node (`:369-371`). That is the issue's cosmetic half, fixed by #1612.

## 3. Review checklist for #1674

1. Gate: no stop-context work unless the picker paint is a gradient on a Fill or Stroke target. Switching to a non-gradient paint clears the stale stop state. The PR's test `solid_picker_clears_gradient_stop_context` covers this.
2. Cache key: picker target, selected ids, paint, document identity/epoch, library metadata and the active binding key. A cache hit allocates nothing. The PR also avoids `pending_all()` on the hit path.
3. **Added criterion:** `sync_picker_paint_from_doc` must not allocate per frame on an unchanged document. If it does, fix it in the same PR behind the same epoch key, or file a follow-up with a measurement.
4. CLAUDE.md §4.2: no `ctx.load_texture` inside `ctx.data_mut`. Unchanged, but verify.
5. Invalidation: undo/redo, a variable mode switch and a library update must refresh the context. A stale pill after a mode switch is a regression.

## 4. Acceptance criteria

- **AC-1** Solid fill picker, 5,000 selected layers with 2 stop bindings each, debug build: the per-frame cost of `sync_picker_context` is within noise of a 1-layer selection. Measure with the PR's harness or an `Instant` probe in a test, and report numbers with their citation.
- **AC-2** Gradient picker, unchanged inputs: the second frame reuses the cache (counter or pointer-equality test).
- **AC-3** Changing a stop binding, the selection, the mode or undo invalidates the cache, and the pill updates on the next frame.
- **AC-4** The mixed-mode pill test from #1612 still passes.
- **AC-5** Mutation check: remove the gradient gate, and AC-1's test fails.

## 5. Test steps

1. `gh pr checkout 1674`.
2. `cargo test -p designos-app --bin designos panels::var_bind`, then a revert check.
3. `cargo clippy -p designos-app`.
4. Controller GUI check when the Mac is idle: open the picker on a large kit selection, and Activity Monitor CPU stays flat (screenshot).

## 6. Edge cases

- A picker open while an agent edits the document (live sync): the epoch changes and the cache rebuilds.
- A selection containing deleted node ids: filtered as today (`:336`).
- Library variable updates pending: the key includes library metadata.

## 7. Non-goals

- Style-bound stops (#1446).
- Picker history resync (#1618 / PR #1630).

## 8. Builder rules

Carry with credit. Use `Closes #1443`. The commit body notes that the mixed-mode half shipped in #1612.
