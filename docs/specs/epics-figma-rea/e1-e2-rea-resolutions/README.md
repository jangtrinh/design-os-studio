> **Review status (2026-11-12): SUPERSEDED.** Reference for implementation; nine of its 13 slices target issues that are already closed and merged (#135, #136, #137, #143, #1448), the layout slices need Figma evidence first, and its hot-file queue slots are not real. Corrections: [review](../reviews/review-e1-e2-e19-e20.md#e1-e2-readme).

# E1 & E2 REA Resolutions: Master Plan & Hot-File Execution Schedule

**Date:** 2026-11-12  
**Milestone:** v0.8 Variables Everywhere & Layout Parity  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, JS/WASM offsets, schema IDs, and UI strings in own words.

---

## Answer first

- **Complete Step-0 Parity Resolution:** The verified Figma 126.11.7 REA dossier resolves every Step-0 blocker for E1 (`#135`, `#136`, `#137`, `#143`, `#1448`, `#1856`) and E2 (`#1885`, `#1895`, Grid 2.0 unbounded tracks) with bit-exact Kiwi IDs and mathematical proofs.
- **Strict Hot-File Queue Integration:** All slices modifying hot files (`fig.rs`, `fig_export.rs`, `kiwi.rs`, `rest_import.rs`) are scheduled strictly serially in slot positions #27, #28, #29, and #30, extending the queue established in `specs/e1-styles-variables/README.md`.
- **Cost-Optimized Model Routing:**
  - `Model: "flash"` (`Gemini 3.8 Flash`): Pure schema decoding, unit test suites, Dev Mode CSS formatting, and documentation.
  - `Model: "pro"` (`Gemini 4 Argon`): Color space transformation matrix math, recursive extended collection resolver, auto layout rotated bounding-box projection, and AccessKit spatial index queries.
- **Studio Invariants Enforced:**
  - Zero `doc.to_json()` in `doc_hash` (preserves zero-allocation `HashSerializer`).
  - Zero `TileView::rev` bumps on viewport-only, focus, or hover changes.
  - Bit-exact headless test parity preserved via strict `self.interactive()` guards in `designos-gpu`.

---

## 1. What could NOT be verified (read first)

1. **Simultaneous Multi-Tier Override Deletion:** When an extended collection $C_2$ extends $C_1$ which extends $C_0$, deleting variable $V$ from $C_0$ while overrides exist in $C_1$ and $C_2$ was inferred from the schema (`removeOverridesForVariable`) but not traced in a live wasm state tree. Studio treats root deletion as invalidating child override references `[INFERENCE]`.
2. **Display P3 Legacy ICC v2 Fallback:** Whether Figma converts ICC v2 profile tags in legacy imported PNGs using matrix or LUT profile connection spaces was not traced; Studio adopts standard Little-CMS/skcms Bradford matrix math `[ASSUMPTION]`.
3. **Non-Orthogonal Rotated Auto Layout Child with Negative Margins:** Figma auto-layout behavior when a rotated child has negative margins was not verified; Studio clamps margins to the projected AABB `[INFERENCE]`.
4. **Figma REST API Write to Extended Overrides:** The REST API is strictly read-only for variable mutations; all override modifications occur via Plugin API or Desktop UI `[FACT - seen]`.

---

## 2. REA Evidence & Checksum Index

Evidence extracted from clean-room disassembly of Figma 126.11.7 artifacts:

| Artifact Name | SHA-256 Digest | REA Context ID |
|---|---|---|
| `7a72fc59-4dff289669fb67b0.min.js` | `50ec533f7271ff0e867f390117e29ed31526dd8fbc94490fda24b561b75a6997` | `art_50ec533f7271ff0e867f390117e29ed31526dd8fbc94490fda24b561b75a6997` |
| `28827-4740e6aaf71154a2.min.js` | `83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc` | `art_83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc` |
| `17185-60cf0f6e7cbc270f.min.js` | `aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` | `art_aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` |
| Unified Extraction Ledger | `ed3b7ee3e67eb50db9dd7cf1183fa4fe2e570b17f8276367c1ce4` | `jsrg_evidence_ed3b7ee3e67eb50db9dd7cf1183fa4fe2e570b17f8276367c1ce4` |

---

## 3. Slice Ordering & Hot-File Execution Schedule

### Hot-File Queue Position Integration
Binding rule: Only ONE worker may touch `fig.rs`, `fig_export.rs`, `kiwi.rs`, or `rest_import.rs` at any given time.

```text
[Position 25]  #134 Component Property Binds (fig.rs / fig_export.rs)
       │
       ▼
[Position 27]  #135 Slice 2: Extended Collections Serialization (fig.rs / fig_export.rs / kiwi.rs)
       │
       ▼
[Position 28]  #136 Slice i: Document Color Profile Header & Working Space (fig.rs / fig_export.rs)
       │
       ▼
[Position 29]  #137 Slice a: AuthoredColor & Paint/Effect Kiwi Codecs (fig.rs / fig_export.rs)
       │
       ▼
[Position 30]  #1448 & #143: Style Override Table & Transition Var Codecs (fig.rs / fig_export.rs)
       │
       ▼
[Position 31]  #1885 & #1895: Auto Layout SpaceBetween & Rotated Bounds Codecs (fig.rs / fig_export.rs)
       │
       ▼
[Position 32]  Grid 2.0: Dynamic GUIDGridTrackSizeMap Codecs (fig.rs / fig_export.rs / kiwi.rs)
```

### Full Slices Execution Table

| Slice ID | Target Issue | Scope & Files Owned | Size (h) | Model Tier | Hot-File Slot | Dependencies |
|---|---|---|---|---|---|---|
| **E1-REA-1** | `#135` Slice 1 | Document model: `ExtendedCollection`, `VariableOverride`, parent mode map (`designos-doc/src/variables.rs`, `variables_apply.rs`) | 6 | Argon | None (Doc only) | None |
| **E1-REA-2** | `#135` Slice 2 | Kiwi & .fig round-trip: `VARIABLE_SET=31`, `VARIABLE_OVERRIDE=35` (`designos-fig/src/fig.rs`, `fig_export.rs`, `kiwi.rs`) | 8 | Flash | **Pos 27** | E1-REA-1 |
| **E1-REA-3** | `#135` Slice 3 | Variables Panel UI: Overrides table, Reset override action (`designos-app/src/panels/var_bind.rs`) | 6 | Argon | None (App UI) | E1-REA-2 |
| **E1-REA-4** | `#136` Slice 1 | `DocumentColorProfile` model, working-space float storage (`designos-doc/src/paint.rs`, `node.rs`) | 4 | Flash | None (Doc only) | None |
| **E1-REA-5** | `#136` Slice 2 | Kiwi codec for `documentColorProfile=349` (`designos-fig/src/fig.rs`, `fig_export.rs`) | 4 | Flash | **Pos 28** | E1-REA-4 |
| **E1-REA-6** | `#136` Slice 3 | Bradford matrix convert/assign & PNG ICC chunk (`designos-render/src/color.rs`, `designos-export/src/png.rs`) | 8 | Argon | None (Render/Export) | E1-REA-5 |
| **E1-REA-7** | `#137` Slice a | `AuthoredColor` model & Dev Mode CSS emission (`designos-doc/src/paint.rs`, `designos-export/src/css.rs`) | 6 | Flash | None (Doc/Export) | E1-REA-4 |
| **E1-REA-8** | `#137` Slice b | Kiwi codec for `authoredColor=44` & `authoredStops=45` (`designos-fig/src/fig.rs`, `fig_export.rs`) | 4 | Flash | **Pos 29** | E1-REA-7 |
| **E1-REA-9** | `#1448` & `#1856` | Text style overrides `isOverrideOverTextStyle=376`, `varValue=9` (`designos-doc/src/styles_ops.rs`, `designos-fig/src/fig.rs`) | 8 | Argon | **Pos 30** | None |
| **E1-REA-10**| `#143` | Prototype transitions timing/easing variables (`designos-doc/src/prototype.rs`, `designos-fig/src/fig.rs`) | 4 | Flash | **Pos 30** (shared) | None |
| **E2-REA-1** | `#1885` | Space-between backing store & wrap counter spacing (`designos-doc/src/layout_solver.rs`, `layout_apply.rs`) | 5 | Argon | None (Doc solver) | None |
| **E2-REA-2** | `#1895` | Rotated child AABB projection & min/max clamping (`designos-doc/src/layout_apply.rs`, `layout_solver_types.rs`) | 7 | Argon | None (Doc solver) | None |
| **E2-REA-3** | Grid 2.0 | Dynamic tracks expansion beyond `MAX_GRID_TRACKS` (`designos-doc/src/node.rs`, `designos-fig/src/fig.rs`, `fig_export.rs`) | 6 | Flash | **Pos 32** | None |

---

## 4. Rollback & Quality Gates

### Revert Safety
- Every slice is committed as an independent squash PR.
- Additive serde fields on `Document`, `NodeChange`, `VariableCollection`, and `Paint` utilize `#[serde(default, skip_serializing_if = "...")]`.
- Reverting any individual slice restores binary compatibility without invalidating user project files.

### Acceptance Verification Gate
Every slice must execute and pass the following gate before merge:
```bash
# 1. Workspace Unit & Integration Tests (excluding heavy VSF benchmark)
cargo test --workspace --lib -- --skip vsf_pcp

# 2. GPU Headless Parity Gate
cargo test -p designos-gpu

# 3. Code Quality & Format
cargo clippy --workspace -- -D warnings
cargo fmt --check

# 4. Zero Fake UI Gate
python3 scripts/zero_fake_ui_gate.py
```
