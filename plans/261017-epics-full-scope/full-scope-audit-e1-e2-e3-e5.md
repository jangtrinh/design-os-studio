# Full-scope audit of existing epics E1, E2, E3, E5

Brainstormer audit, 2026-10-17, under the owner ruling: full scope, no MVP. Status: owner decisions pending.

The four epics are not full scope yet. My count is 41 MISSING or partial rows, and each has a task draft below. Two findings are serious enough to fix first:

- **Noise, texture and glass are lost on every `.fig` round trip.** Export drops them; import keeps only the effect type and loses every parameter. This is a silent data-loss bug on main today, beyond being a gap.
- **None of the 9 follow-ups that the PR #49 review asked for were filed.** I checked all issue titles up to #105.

### Epistemic Ledger
- **FACT:** what I read this session:
  - the 7 planning docs, plus the sibling drafts `plans/261017-epics-full-scope/draft-e4-e6-e9.md` and `draft-e7-e8-e10.md`, so nothing is duplicated;
  - code, read on `origin/main` (11dc1f4) and on the PR branches `task/30`, `task/31` and `task/35`;
  - issues and PRs through 3 `gh api` calls: the list with all bodies (#6–#105) and the PR #49 review comments.
- **Your local `main` is 3 commits behind `origin/main`** (#84, #94, #55). All citations below point at `origin/main`.
- **INFERENCE:** sizes, priorities, and saying "no UI found" from greps that returned no hits.
- **ASSUMPTION:** Figma behaviour that REA has not confirmed. Each such row is marked.
- **Not verified:** nothing was compiled or run, and no Figma.app REA was done this session.

Tags: [F] fact · [I] inference · [A] assumption. Sizes: S < 1 day · M 1–3 days · L 1–2 weeks · XL multi-week. Hot files: `fig.rs`, `fig_export.rs`, `kiwi.rs`, `rest_import.rs`.

---

## E1 Styles & Variables (#14)

### 1. Checklist
| Capability | Status | Evidence |
|---|---|---|
| Colour styles (fill + stroke) and text styles | DONE [F] | `designos-app/src/styles.rs:35,85-107` |
| Effect and grid styles | DONE [F] | `styles.rs:107-111`, `shell/tests/effect_grid_styles.rs` |
| Create / apply / detach / rename / update style | DONE [F] | `designos-doc/src/styles_ops.rs:93-186` |
| Style edit + propagate | DONE [F] | #36 closed, PR #44 merged |
| Library style read-only rename | IN-FLIGHT [F] | #67 → PR #85; #100 TASKED |
| Scrub in style popover; duplicate prefix letters | IN-FLIGHT / TASKED [F] | #69 → PR #89; #101 |
| Recolour rich-text runs on colour-style apply | TASKED, blocked by #45 [F] | #68 |
| Delete style | MISSING [I] | no delete op in `styles_ops.rs`; only `history.rs:39` mentions delete |
| Style reorder / folders (`sortPosition=320`) | MISSING [F] | `StyleMeta` = name/key/description/property only (`lib.rs:742-750`) |
| Style description editing UI | MISSING [I] | field exists (`lib.rs:747`); 0 hits in `style_edit.rs` |
| Variables bound inside styles (colour style → variable, etc.) | MISSING [F] | `Style::Fill(Vec<Paint>)`; `Paint` has no binding (`lib.rs:754`, `rest_import.rs:97-101`) |
| Collections, modes, aliases, scopes, hidden, description, groups | DONE [F] | `variables.rs:14-117`, `variables_edit.rs:95-284` |
| Code syntax; opacity variables | DONE [F] | wave A/B; `variables_apply.rs:455-529` |
| Variables in prototypes | DONE [I] | `protovars.rs`, `prototype_expr.rs` |
| Binding: gradient stops, effect radius/spread/offset, glass params, min/max size, font family/style, paragraph spacing/indent, component props | MISSING [F] | `property_kind` covers only the keys at `variables_apply.rs:21-31`; effects bind colour + opacity only (`fig_export.rs:721`) |
| Variables JSON import/export | PARTIAL [F/A] | exists (`designos-fig/src/variables.rs:158,215`, `variables/mod.rs:328-350`). Same format as Figma's native JSON: ASSUMPTION, unverified |
| Extended collections | MISSING [F] | `VariableCollection` has no parent fields (`variables.rs:20-29`) |
| Colour spaces: P3 / OKLCH / OKLAB | MISSING [F] | `Color` is straight sRGB (`paint.rs:4-11`); picker is Hex/RGB/CSS/HSL/HSB (`widgets/picker.rs:32-42`); `documentColorProfile` is only kept in metadata (`fig.rs:663`) |
| Animation styles (StyleType ANIMATION=7) | MISSING [F] | not modelled; the sibling draft assigns it to E1 |
| Easing / timing variables | MISSING [F] | `VariableKind` = Color/Number/String/Boolean |

### 2. New task drafts
| ID | Task | Size / Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| E1-1 | Delete style: layers keep their values and are detached | S / P1 | `styles_ops.rs`, `style_picker.rs` | doc unit test | (1) linked layers keep their values and lose the ref, one undo step; (2) a library style refuses (A13 wording); (3) `.fig` export has no orphan style node | — |
| E1-2 | Style ordering and folders: drag-reorder, group by "/", rename/ungroup folder, `sortPosition` round trip | M / P1 | `lib.rs` StyleMeta, `style_picker.rs`, `fig.rs`, `fig_export.rs` | doc + fig round-trip | (1) the order survives native save and `.fig`; (2) a folder rename renames every member, one undo step; (3) old files load | REA E1-a |
| E1-3 | Style description editor plus a hover tooltip in the picker | S / P2 | `style_edit.rs`, `style_picker.rs` | shell test | (1) edit, then save, then the text shows on hover; (2) disabled for library styles | — |
| E1-4 | Variables inside styles: colour/effect style paints and text-style numbers can hold a variable alias | L / P1 | `lib.rs` Style, `styles.rs`, `fig.rs:927,957`, `fig_export.rs`, `variables_apply.rs` | doc mode-switch test + fig round-trip | (1) a mode switch recolours the layers linked to a variable-bound colour style; (2) `.fig` style node `colorVar` round-trips; (3) Check designs counts it as bound | REA E1-b |
| E1-5a | Bind variables to paints and effects: gradient stops, effect radius/spread/offset X/Y, glass params (scopes EFFECT_FLOAT, EFFECT_COLOR) | M / P1 | `variables_apply.rs`, `fig.rs:1168-1260`, `fig_export.rs:721,1362`, variables bind picker | doc + fig | (1) each key resolves per mode; (2) scope filtering is enforced; (3) round trip | after #38 and #37 (effect fields) |
| E1-5b | Bind variables to layout and text: min/max W/H, fontFamily/fontStyle (string), paragraph spacing/indent | M / P1 | same files plus the text model | doc + fig | (1) a string variable switches the font family per mode; (2) min/max feeds the solver; (3) round trip | #47 |
| E1-5c | Bind variables to component properties (boolean, text, variant) | M / P2 | `components.rs`, `variables_apply.rs`, `fig.rs` | doc test | (1) a boolean variable toggles the prop under a mode switch; (2) round trip | REA E1-c |
| E1-6 | Extended collections: parent collection/mode, per-variable overrides, inherit/reset UI, `.fig` (`parentVariableSetId=4`, `parentModeId=5`, `isCollectionExtendable=385`), library parent | L / P2 | `variables.rs`, `variables_edit.rs`, `variables/view.rs`, `fig/variables.rs`, `fig.rs` | doc resolution + fig round-trip | (1) a parent change flows to non-overridden values; (2) reset restores the parent; (3) round trip | REA E1-d |
| E1-7 | Document colour profile (sRGB / Display P3): model, `.fig` `documentColorProfile`, conversion in CPU+GPU, P3 export tag | L / P2 | `lib.rs`, `paint.rs`, `fig.rs`, `fig_export.rs`, render, gpu, export | pixel diff, CPU = GPU | (1) a P3 file renders the same as Figma on a P3 display (screenshot); (2) sRGB files are unchanged (bit-exact); (3) round trip | decision D-E1-1, REA E1-e |
| E1-8 | OKLCH / OKLAB picker modes; CSS codegen `oklch()` / `color(display-p3 …)` | M / P2 | `widgets/picker.rs`, `designos-export/css.rs` | unit conversion tests | (1) known-value conversions within 1/255; (2) a P3 document emits `color(display-p3)` | E1-7 |
| E1-9 | Variables JSON in Figma-native format (import + export), plus DTCG if D-E1-2 says so | M / P2 | `designos-fig/src/variables.rs` | fixture round-trip | (1) a Figma-exported JSON imports losslessly; (2) our export re-imports into Figma (manual check) | REA E1-f |
| E1-10 | Animation styles (ANIMATION=7): model, picker, `.fig` | M / P2 | `lib.rs` Style, `styles.rs`, `fig.rs` | fig round-trip | (1) round trip; (2) apply links it to a layer | E6-1 |
| E1-11 | Easing/timing variables (new kind) bound to transitions and Motion | M / P2 | `variables.rs`, `prototype.rs` | doc test | (1) a bound easing switches per mode | E6-1, X2 |

### 3. Owner decisions
- **D-E1-1 Colour-space depth.**
  - **(a) Recommended: store and render Display P3 per document.** A P3 Figma file treated as sRGB shifts colour visibly [I].
  - (b) Picker-only OKLCH stored as sRGB. M instead of L, but wide-gamut colour is lost.
- **D-E1-2 Variables JSON format.**
  - **(a) Recommended: Figma-native only.** That is the parity target.
  - (b) Also W3C DTCG tokens. +S effort, helps code teams.

### 4. Technical decisions taken
- No mode or collection count cap. Figma's limits are plan tiers.
- Extended collections are not gated by plan.
- Delete follows Figma: values are kept and the layer is detached [A, REA confirm].
- E1-10 and E1-11 are listed here but are scheduled after E6-1.

### 5. REA reads needed
- E1-a `sortPosition` and folder semantics.
- E1-b `colorVar` on style nodes, and text-style variable fields.
- E1-c the component-prop variable binding path.
- E1-d how VARIABLE_SET extension overrides are stored.
- E1-e the `documentColorProfile` enum and the colour encoding in P3 files.
- E1-f the Figma variables JSON schema.

---

## E2 Auto layout & Grid 2.0 (#15)

### 1. Checklist
| Capability | Status | Evidence |
|---|---|---|
| Direction, wrap (horizontal and vertical), gaps, padding, hug/fill/fixed, absolute, min/max, reverse z, strokes included | DONE [F] | `node.rs:669-713`; vertical wrap shipped in wave B |
| Space Between / Around / Evenly; counter spread | DONE [F] | `node.rs:633-655` |
| Text wrap Balance/Pretty | DONE [F] | `node.rs:563-579` |
| CSS-aligned version 1 semantics + "Update layout" | TASKED [F] | #26 (field already exists at `node.rs:683`) |
| Child margins | TASKED | #29 |
| One shared solver | IN-FLIGHT | #30 → PR #47; follow-ups #59–#62 TASKED (they depend on `layout_solver.rs` from #47) |
| More than 8 tracks | TASKED [F] | #24 (`[GridTrack; 8]` at `node.rs:693-695`) |
| Auto-position and auto rows | TASKED / partial [F] | #25. `gridAutoTracks` is metadata-only (`fig.rs:729`) and #25 does not mention it |
| Track drag-reorder, add/remove/resize UI | MISSING [F] | #24 lists reorder as a non-goal; the canvas only labels tracks (`canvas/grids.rs:31-47`) |
| Grid child alignment within a cell | MISSING [F] | `gridChildVerticalAlign` metadata-only (`fig.rs:661`); no field on `LayoutChild` (`node.rs:699-713`) |
| Grid gap variables | DONE [I] | `GRID_ROW_GAP` → `counterAxisSpacing` (`fig.rs:1196`); GAP scope (`variables_apply.rs:48`) |
| CSS grid in Dev Mode | PARTIAL [F] | `css.rs:97` always writes `repeat(n, minmax(0,1fr))` and ignores Fixed/Hug tracks; #63 fixes the axis only |
| NaN in track strings | TASKED | #71 |
| Constraints | DONE [I] | `node.rs` `Constraints` |
| Layout guides: alignment Stretch/Min/Center/Max, fixed width, auto count, per-guide visibility | PARTIAL [F] | `LayoutGrid` = size/count/gutter/offset/colour only (`paint.rs:520-524`) |
| Device frame presets incl. iPhone 17 | PARTIAL [F] | only iPhone 15 / 15 Pro Max (`proto/mod.rs:38-39`) |
| Per-child align-self in version 1 | UNKNOWN [A] | import only reads STRETCH (`fig.rs:1016`) |

### 2. New task drafts
| ID | Task | Size / Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| E2-1 | Grid track editing: insert/delete/resize tracks from canvas pills and the panel; drag-reorder a track, carrying its placed children | M / P1 | `canvas/grids.rs`, `canvas/draw.rs`, `panels/properties.rs` | shell test with literal track lists + screenshot | (1) moving track 2 to 4 moves the items anchored there, one undo step; (2) deleting a track re-flows auto-placed items; (3) Figma labels | #24, #25 |
| E2-2 | Grid child cell alignment (H/V): model, solver, UI, `.fig` | M / P1 | `node.rs`, `layout_solver_grid.rs`, `fig.rs`, `fig_export.rs`, `rest_import.rs` | solver parity, literal positions | (1) a centred child sits centred in a 100×100 cell; (2) round trip | #47, REA E2-a |
| E2-3 | CSS/Tailwind/React grid codegen from real tracks (fr/px/auto), placement `grid-column: n / span k`, gaps | M / P1 | `designos-export/css.rs:43-97`, `codegen.rs` | golden export tests | (1) `[1fr, 120px, Hug]` → `1fr 120px auto`; (2) an explicit anchor is emitted | #24, #63 |
| E2-4 | Layout guide parity: alignment, fixed section width, Auto count, per-guide visible, multiple guides; grid styles carry it | M / P1 | `paint.rs:520`, `panels/layoutguide.rs`, `fig.rs`, `fig_export.rs` `encode_grid`, render | render pixel + fig round-trip | (1) centred 12 × 60 px columns draw at literal x; (2) a hidden guide is not drawn but round-trips; (3) old files load | REA E2-b |
| E2-5 | Per-child align-self (MAX/CENTER) if REA confirms a version-1 UI | S / P2 | `node.rs`, solver, `fig.rs` | solver parity | (1) literal positions; (2) round trip | REA E2-c, #47 |
| E2-6 | Frame/device preset refresh to Figma's current list | S / P2 | `proto/mod.rs:38`, frame tool presets | unit test of the list | (1) iPhone 17 family present with Figma sizes [A] | REA E2-d |
| amend #25 | Add: map `gridAutoTracks` (`fig.rs:729`) to the model and back | — | — | — | — | — |

### 3. Owner decision
- **D-E2-1 Layout version for new frames.**
  - **(a) Recommended: version 1 (CSS-aligned, border inside the box) for new frames.** Imported frames keep their own version. This matches what Figma 2026 does for new frames [A].
  - (b) Keep version 0. No visual change for existing users, but drifts from Figma.

### 4. Technical decisions taken
- Track cap: 64 or more, per #24.
- A reorder moves children with their track, as in CSS and Figma [A].
- E2-2 and E2-5 wait for PR #47 so the solvers are not edited three times.

### 5. REA reads needed
- E2-a the `gridChildHorizontal/VerticalAlign` enums.
- E2-b layout-guide fields (alignment, sectionSize, count = −1 for auto).
- E2-c the align-self UI in version 1, plus Figma's default layout version for new frames.
- E2-d the preset list.
- The `gridAutoTracks` string format.

---

## E3 Slots & Check designs (#16)

### 1. Checklist
| Capability | Status | Evidence |
|---|---|---|
| Slot prop kind / SlotContent override (legacy) | DONE, partial [F] | `node.rs:188-210`, `components.rs:207-230` |
| Slot model + `.fig` round trip | IN-FLIGHT, FIX-FIRST [F] | #31 → PR #49 (H1, L1, M1 open) |
| Slot authoring UI; edit slot content in instances | TASKED | #32, #34 |
| Real `preferredValues` object (SLOT + INSTANCE_SWAP) | MISSING [F] | PR #49 review M3, not filed |
| Slot content id per property | MISSING [F] | PR #49 follow-up, not filed |
| Paste remap of slot/preferred ids | MISSING [F] | M2 (`clipboard.rs:177`), not filed |
| Figma's real numbering in exported schema (SLOT = 7, not 4) | MISSING [F] | PR #49 follow-up, not filed |
| Untrusted slot-list raw-number fallback | MISSING [F] | PR #49 follow-up, not filed |
| `duplicate_subtree` / `apply_reference` slot semantics; unknown `SlotPropConfig` keys | MISSING [F] | L2 + acceptance ruling, not filed |
| Empty-slot placeholder, "+" preferred-instance picker, stretch-on-insert behaviour | MISSING [I] | not in the #32/#34 acceptance criteria |
| Migrate a library to slots | MISSING [F] | catalog §6 |
| Slots in codegen / Dev Mode | MISSING [I] | — |
| Component playground (Dev Mode) | MISSING [F] | gap list put it at P3; under the no-MVP ruling it becomes a decision |
| Check designs: colour hard-coded, contrast AA, detached, one-click fix, Fix all | IN-FLIGHT [F] | #35 → PR #45 (merge-verified per your brief, still open) |
| Check designs inside instances; contrast recovery | TASKED | #52, #65 |
| Hard-coded **numbers** (spacing/radius/size) → number variables | MISSING [F] | the index holds colour variables only (`task/35 hardcoded.rs:33`) |
| Library variables/styles as suggestions | MISSING [I] | — |
| Whole-page scan (no 20k cut-off) | MISSING [F] | `MAX_SCANNED_LAYERS = 20_000` (`task/35 check_designs/mod.rs:30`) |
| Full Figma rule set (AAA, large text, dismiss/ignore) | UNKNOWN [A] | REA |

### 2. New task drafts
| ID | Task | Size / Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| E3-1 | `preferredValues` object `{stringValues, instanceSwapValues[{type,key}], parameterValues}` for SLOT and INSTANCE_SWAP | M / P1 | `fig.rs`, `fig_export.rs`, `rest_import.rs`, `components.rs` | fig fixture with the real shape | (1) preferred keys round-trip for both prop types; (2) the old guid-list form still imports | #31 |
| E3-2 | Slot content id per property (saved-format change plus migration) | M / P1 | `node.rs`, `fig.rs:1321`, `fig_export.rs` | doc migration + fig | (1) two slot props on one node keep separate content; (2) old files migrate | #31 |
| E3-3 | Paste remap of `slot_configs[*].preferred`, InstanceSwap preferred, `PropValue::Slot` | S / P1 | `designos-app/src/clipboard.rs:177` | clipboard test | (1) pasted ids point at the pasted nodes; (2) none point at source ids | #31 |
| E3-4 | Exported schema uses Figma's numbering (`ComponentPropType`, NodeChange tags) | M / P1 | `fig_export.rs`, `kiwi.rs` | importer test with Figma numbering + manual Figma.app open (screenshot) | (1) Figma.app opens the export with slots intact; (2) existing round trips pass | #31 |
| E3-5 | Reject the raw-number fallback in the slot-list parser | S / P1 | `rest_import.rs` | negative fixture | (1) an untrusted number cannot bind an unrelated node | #31 |
| E3-6 | Slot doc-op semantics plus unknown `SlotPropConfig` keys kept in the native format | S / P2 | `components.rs`, `node.rs` | doc tests | (1) duplicate remaps `slot_content_id`; (2) `apply_reference` mirrors `is_slot_content`; (3) an unknown key survives a save | #31 |
| E3-7 | Slot canvas UX: empty placeholder (`displayByDefault`), "+" picker limited to preferred instances, stretch-on-insert | M / P1 | canvas, `panels/components.rs` | shell test + screenshot | (1) an empty slot shows a placeholder in the main component and in instances; (2) the picker lists preferred-only when set; (3) an inserted child stretches when the flag is on | #32, #34 |
| E3-8 | Migrate to slots: convert a region driven by instance swap or a boolean into a slot, keeping each instance's content | M / P2 | `comp_ops.rs`, `components.rs` | doc test | (1) instance content becomes slot content, one undo step; (2) a no-op on unsupported shapes, with an honest message | #32, REA E3-c |
| E3-9 | Slots in codegen: React `children`/ReactNode prop, SwiftUI `@ViewBuilder`, Compose slot lambda; Dev Mode inspect shows the slot | M / P2 | `designos-export/codegen.rs`, `react.rs` | golden exports | (1) each target emits a slot parameter | #31 |
| E3-10 | Check designs: number variables (gap, padding, radius, stroke, font size, line height) | M / P1 | `check_designs/hardcoded.rs`, `fix.rs` | literal fixtures | (1) a 16 px gap that matches `space/4` is flagged with a Bind fix; (2) ambiguous matches are reported without a fix | #45 |
| E3-11 | Check designs: suggestions from enabled libraries | M / P2 | `hardcoded.rs`, `libraries/` | fixture with a library | (1) a library variable is suggested; (2) the fix imports and binds in one undo step | #45 |
| E3-12 | Check designs at scale: incremental scan off the UI thread, no cut-off | M / P1 | `check_designs/mod.rs`, `state.rs` | perf test on 723k fixture (budgets unchanged) | (1) the full page completes with no frame over budget; (2) progress is shown | #45 |
| E3-13 | Check designs rule parity per REA (thresholds, dismiss/ignore) | M / P2 | `check_designs/*` | fixtures | per REA list | REA E3-d, #45 |
| E3-14 | Component playground in Dev Mode: toggle props on a scratch instance, read-only | M / P2 | `designos-app/src/dev/` | shell test + screenshot | (1) prop toggles never mutate the document; (2) codegen updates live | D-E3-2 |

### 3. Owner decisions
- **D-E3-1 Real Figma file with slots.**
  - **(a) Recommended: you supply one.** The storage is partly inferred; 0 hits in 6,045 cached files per the PR #49 review.
  - (b) Synthetic fixtures only. Risk: E3-4 cannot be proven against Figma.
- **D-E3-2 Component playground.**
  - **(a) Recommended: build it (E3-14).** Local, no network, required by the no-MVP ruling.
  - (b) Decline it explicitly.
- **D-E3-3 Check designs suggestions for numbers.**
  - **(a) Recommended: exact value matches only, ambiguous ones reported without a fix.** No false fixes.
  - (b) Nearest token within a tolerance. Catches more, but risks wrong binds.

### 4. Technical decisions taken
- File the 9 PR #49 follow-ups as E3-1 to E3-6.
- Check designs ships to all users, with no Org/Enterprise gating.
- E3-1, E3-2, E3-4 and E3-5 run one at a time, because all touch `fig.rs` / `fig_export.rs`.

### 5. REA reads needed
- E3-a the `SlotNode` / SLOT node type id.
- E3-b the exact `preferredValues` schema.
- E3-c migrate-to-slots command semantics.
- E3-d the Check designs rule list (number tokens, AAA/large text, ignore list, library sources).

---

## E5 Effects & Shaders (#18)

### 1. Checklist
| Capability | Status | Evidence |
|---|---|---|
| Drop/inner shadow, layer/background blur, show-behind | DONE [F] | `paint.rs:267-287` |
| Shadow blend mode | MISSING [F] | no blend field on the shadows (`paint.rs:268-281`) |
| Effect count limits (8 drop / 8 inner / 1 layer blur / 1 background blur) | MISSING [I] | 0 hits |
| Noise / texture / glass: model + CPU/GPU render | DONE [F] | `paint.rs:292-363`; `render/tests/fx.rs`, `gpu/tests/fidelity.rs` |
| Noise / texture / glass: **`.fig` round trip** | **BROKEN [F]** | import `convert_effect` copies only shadow/blur keys (`fig.rs:1849-1860`); export `encode_effect` `_ => return None` (`fig_export.rs:1661`). REST round trip works (`rest_import.rs:122-155`) |
| Glass splay; GRAIN→texture; noise type/seed/density | TASKED | #38 (acceptance says "round-trip" but does not name `.fig`) |
| Noise/texture independent X/Y | MISSING [F] | `size: f32` scalar (`paint.rs:328`) |
| Progressive blur | TASKED | #37 |
| Variables bound to effects (incl. glass) | MISSING → E1-5a | — |
| Custom shaders: preserve refs | TASKED | #39 |
| Custom shaders: author + render | MISSING [F] | owner ruling: full scope |
| Image fill adjustments (exposure, contrast, saturation, temperature, tint, highlights, shadows), rotation, tile scale, crop transform | MISSING [F] | `PaintKind::Image{image_ref, scale_mode}` (`paint.rs:207-210`) |
| Video fill | MISSING [F] | `rest_import.rs:95` drops VIDEO |
| Pattern fill | Owned by E4-7 (sibling draft) | — |
| AI image tools | MISSING, needs a remote model | → decision |
| 3D transforms | MISSING, Figma early access | → decision |
| Paint/layer blend modes | DONE [F] | `paint.rs:150-170` |

### 2. New task drafts
| ID | Task | Size / Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| amend #38 | Acceptance must say **`.fig` kiwi** round trip (`convert_effect` + `encode_effect`), plus noise/texture X/Y size | +M | `fig.rs:1849`, `fig_export.rs:1640` | fig round-trip | (1) a NOISE/GRAIN/GLASS fixture keeps every field through `.fig`; (2) effect styles carry them | REA E5-b |
| amend #37 | Same: kiwi round trip, effect-style support, record the offset units | — | — | — | — | REA E5-a |
| E5-1 | Shadow blend mode (drop + inner) | S / P1 | `paint.rs`, render, gpu, `fig.rs`, `fig_export.rs`, `rest_import.rs`, `panels/effects.rs` | pixel test (multiply), CPU = GPU, round-trip | (1) a multiply shadow darkens the backdrop as a literal pixel; (2) round trip; (3) old files default to Normal | REA E5-c |
| E5-2 | Effect count limits in the "+" control (8/8/1/1); imports are never trimmed | S / P2 | `panels/effects.rs` | shell test | (1) the 9th drop shadow is disabled, with a tooltip; (2) an imported file with more keeps them all | — |
| E5-3 | Image fill adjustments + rotation + tile scale + crop transform | L / P1 | `paint.rs`, render paint, gpu `paintconv.rs`, `fig.rs`, `fig_export.rs`, `rest_import.rs`, image paint UI | pixel tests, CPU = GPU, round-trip | (1) exposure +1 brightens a literal pixel equally on CPU and GPU; (2) a 90° rotation is exact; (3) round trip | REA E5-d |
| E5-4 | Video fill: import/keep, poster frame on canvas, playback in Present | L / P2 | `paint.rs`, `video.rs`, render, `fig.rs` | fig round-trip + Present test | (1) round trip lossless; (2) the canvas shows the first frame; (3) Present plays it | D-E5-4, E6 video |
| E5-5 | Custom shader authoring: WGSL paint/effect asset, code panel, uniforms from props, GPU render | XL / P2 | new `designos-doc` shader asset, gpu programs, app panel | gpu render test | (1) a user WGSL gradient renders; (2) a compile error is shown inline, never a crash; (3) obeys the 64-program cap and the VRAM budget | #39, D-E5-1 |
| E5-6 | CPU path for shaders per D-E5-1 | L / P2 | `designos-render` | CPU/GPU diff or exclusion list | per decision | E5-5 |

### 3. Owner decisions
- **D-E5-1 Custom shaders.** Figma's shader source is a server-side asset (R5 FACT), so imported Figma shaders cannot render whatever is chosen here.
  - **(a) Recommended: author our own WGSL shaders. The GPU renders them; the CPU and headless paths use a cached GPU readback, and shader nodes are excluded from the bit-exact parity suite.** Reason: real authoring without an XL interpreter.
  - (b) Interpret a WGSL subset on the CPU through naga. Full parity, XL+.
  - (c) Preserve only (#39). Needs your explicit sign-off as a cut.
- **D-E5-2 AI image tools** (remove background, isolate, expand, erase, vectorize). They need a model.
  - **(a) Recommended: a bundled on-device segmentation model for remove background and isolate (offline, larger binary), and route generative expand/erase through the user's own agent CLI.**
  - (b) Agent CLI only.
  - (c) Decline.
- **D-E5-3 3D transforms** (early access in Figma).
  - **(a) Recommended: wait for Figma GA, then run REA.** The format may change.
  - (b) Build now from the 126.9.13 schema.
- **D-E5-4 Video decode for fills.**
  - **(a) Recommended: OS decoders (AVFoundation / Media Foundation).** Licence-clean, and the same choice as the E6 encoder decision.
  - (b) Keep the optional openh264 feature. The default build cannot show MP4 fills.

### 4. Technical decisions taken
- Glass/effect variable binding lives in E1-5a, so there is one binding system.
- Pattern fill stays in E4-7, with no duplicate here.
- Effect limits are enforced in the UI only; imported data is never dropped (law: nothing vanishes silently).
- #39 may use sibling X1 (typed kiwi passthrough) when X1 lands first.

### 5. REA reads needed
- E5-a progressive blur offset units.
- E5-b the noise X/Y fields, and `PaintType NOISE=9` versus the NOISE effect.
- E5-c the `Effect.blendMode` field id.
- E5-d image paint filter and transform fields.
- The video paint fields.
- The CUSTOM uniform schema.

---

## Cross-epic technical decision
Only one task may touch `fig.rs`, `fig_export.rs`, `kiwi.rs` or `rest_import.rs` at a time, the same rule as the sibling draft. Suggested hot-file order:
1. PR #49 fixes;
2. #38 amended;
3. #37;
4. E3-1 to E3-5;
5. E1-4 and E1-5;
6. E2-2 and E2-4.

## Assumptions (confidence; what would change them)
- Figma gives new frames layout version 1 (medium; REA E2-c).
- Deleting a style detaches its layers and keeps their values (medium; REA).
- Figma image fills expose the 7 adjustment sliders (high; REA E5-d gives the field names).
- The variables JSON export is not in Figma's native format (low; REA E1-f).

## Unresolved questions
Owner choices, for your AskUserQuestion round:
- D-E1-1, D-E1-2
- D-E2-1
- D-E3-1, D-E3-2, D-E3-3
- D-E5-1, D-E5-2, D-E5-3, D-E5-4

Status: DONE_WITH_CONCERNS: no REA was run, sizes and priorities are inferred, and the local `main` is 3 commits behind `origin/main`.
Summary: Audited E1, E2, E3 and E5 against the Figma catalog and the code on `origin/main`; drafted 41 new or amended tasks and 10 owner decisions. Fix first: the `.fig` round trip drops noise, texture and glass (`fig.rs:1849`, `fig_export.rs:1661`), and none of the 9 PR #49 slot follow-ups were filed.