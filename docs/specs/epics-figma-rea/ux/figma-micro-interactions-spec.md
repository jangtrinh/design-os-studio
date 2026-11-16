---
id: UX-MI-001
type: ux-micro-interactions
title: "Figma Desktop 126.9.13 Micro-Interactions — REA Evidence & State-First Behavior Specification (MI-01..MI-45)"
status: current
trace:
  req: [REQ-04, REQ-08, REQ-10, REQ-18, REQ-21]
  us: [US-06, US-11, US-13, US-17]
  skill: es:micro-interactions
---

# Figma Desktop 126.9.13 Micro-Interactions — REA Evidence & State-First Behavior Specification (`MI-01` .. `MI-45`)

> **Skill Discipline:** `es:micro-interactions` (`Full` Tier — State-First + Gesture Lifecycles + Interruption & Rollback + Motion/Feedback Tokens)  
> **Target Parity Baseline:** Figma Desktop `126.9.13` (`compiled_wasm.wasm` + `app.asar` renderer bundle)  
> **Target Runtime:** `design:os studio` (`crates/designos-app` + `crates/designos-doc`, Rust `egui` + `wgpu`, `< 1ms` frame budget)

---

## Layer 1: REA Evidence Ledger (Reverse-Engineered Figma Desktop 126.9.13)

### 1.1 Binary & Bundle Provenance (`FACT`)

| Artifact | Path | Size / Metrics | SHA-256 / REA Evidence ID |
|---|---|---|---|
| **Figma Fullscreen Engine (WASM)** | `/tmp/figma_wasm_re/compiled_wasm.wasm` (`/tmp/figma_wasm_re/compiled_wasm_interface.wasm`) | `52,059,318` bytes · `571` types · `1,783` imports · `51,410` functions · `4,983` exports | `457fca35e6e0e8f79867fd345e7a3d21ad2c20848c880468ceefdc3ef42886fd` · `ev_039a0815aebb02925201b706cc27d29aaf3dbcf550ef733c9e6b83455a9dffee` |
| **WASM Data Strings & C++ Paths** | `/tmp/figma_wasm_re/wasm_data_strings.txt` · `/tmp/figma_wasm_re/cpp_files.txt` | `150,412` string literals · `1,482` C++ compilation units | Extracted via `rea inspect-wasm-artifact` & string table dump |
| **TSAPI WASM Bridge** | `/tmp/figma_wasm_re/tsapi_exports.txt` | `4,983` `_TsApiGenerated_*` / `_Internal_*` bindings | Direct JS ↔ C++ interaction state machine bridge |
| **Electron Renderer (`app.asar` / Web)** | `/tmp/rea-figma-asar` · `/tmp/figma_wasm_re/js/*.min.js` | `0c62c2fd-cb7ba5a41c48c85a.min.js` + inspector input bundles | `ScrubbableControl`, `valueBeforePreview`, `updatePreview`, `F="Mixed"` |

### 1.2 C++ Behavior Hierarchy & State Machines in `compiled_wasm.wasm` (`FACT`)

Figma's fullscreen engine implements every canvas and inspector micro-interaction as a deterministic C++ `Behavior` state machine wrapped by `FGTsMouseBehaviorWrapper.cpp` and committed through transactional `EditScopeSnapshot` frames (`FGEditScopeSnapshot.cpp` / `FGEditScopeStack.cpp`):

| C++ Compilation Unit (`cpp_files.txt`) | Core Symbols / TSAPI Exports (`wasm_data_strings.txt` / `tsapi_exports.txt`) | Mapped Micro-Interactions |
|---|---|---|
| `fullscreen/lib/editor-ui/interactions/FGTsMouseBehaviorWrapper.cpp` | `Figma::TSMouseBehavior::handleMouseDown`, `handleMouseDrag`, `handleMouseUp`, `handleMouseMove`, `handleMouseLeave`, `render`, `renderUnderEditModeUI`, `resetHoverVisualizations` | Base pointer lifecycle (`Idle -> Hover -> Pressed -> Dragging -> Commit / Cancel`) across `MI-06..MI-14` |
| `fullscreen/lib/scenegraph/FGEditScopeSnapshot.cpp`, `FGEditScopeStack.cpp` | `EditScopeSnapshot`, `stagedEditScopeSnapshot`, `clearEditScopeStack` | Transactional undo/redo coalescing & `Escape` mid-gesture rollback (`MI-03`, `MI-06..MI-12`) |
| `fullscreen/lib/editor-ui/interactions/FGResizeSelectionBehavior.cpp` | `ResizeSelectionBehavior`, `ScaleToolObserver` | `MI-11` (`Shift` aspect lock, `Alt` center resize, `Cmd/Ctrl` ignore constraints) |
| `fullscreen/lib/editor-ui/interactions/FGMoveSelectionBehavior.cpp`, `move-selection-delegates/FGMoveSelectionLayoutHelpers.cpp` | `MoveSelectionBehavior`, `SelectionBehavior` (`IGNORE_LOCKED`, `DEFAULT`) | `MI-12`, `MI-14`, `MI-20` (`Space` suppress auto-parenting, `Alt+Drag` duplicate, `Shift` axis lock) |
| `fullscreen/lib/editor-ui/interactions/FGCornerRotationBehavior.cpp` | `CornerRotationBehavior`, `RotationOriginBehavior`, `KeyboardRotationBehavior` | `MI-11` (`15°` `Shift` rotation snap, corner hover rotation zone) |
| `EditShapeBehavior` / `MutableEditShapeBehaviorState` | `_AppStateTsApi_editShapeBehaviorState`, `_EditShapeBehaviorState_Internal___getProperty_isAnyHandleBeingDragged`, `CornerRadiusValues`, `topLeftCornerRadius..bottomRightCornerRadius`, `rectangleCornerToolIndependent`, `selectionCornerRadius`, `smartSelectionHandleCornerRadius` | `MI-06` (Corner radius handle visibility, `Alt` independent single-corner drag, max `min(W,H)/2` clamp), `MI-07` (Ellipse Arc/Donut/StartAngle), `MI-08` (Polygon/Star count & inner ratio) |
| `StackAndGridControlDragBehavior`, `StackAndGridControlClickBehavior`, `InspectStackBehavior`, `fullscreen/lib/scenegraph/FGStackFacet.cpp` | `_StackFacetTsApiGenerated_getStackPaddingTop`, `_getStackPaddingRight`, `_getStackPaddingBottom`, `_getStackPaddingLeft`, `_setStackPadding*`, `_getStackSpacing`, `_setStackSpacing`, `_StackBindingsUiCpp_tabActiveStackRegion`, `_setHoverStackPanelInput`, `_setFocusStackPanelInput`, `_clearActiveStackRegion` | `MI-09` (Auto-Layout pink padding handles, `Alt` opposite-side mirror, `Shift` all-4-sides, click-to-edit inline input + `Tab` region cycling), `MI-10` (Auto-Layout pink gap handles) |
| `fullscreen/lib/editor-ui/interactions/FGReorderListItemBehavior.cpp`, `FGGridTrackBehavior.cpp` | `ReorderListItemBehavior`, `SwapGridItemBehavior`, `ChangeListSpacingBehavior`, `GridTrackBehavior`, `_InteractionCpp_getTableSpanSnapper` | `MI-10` (Smart Selection / Auto-Layout slot reorder & live gap drag) |
| `MutableCanvasSnapper`, `DevHandoffMeasurementMouseBehavior`, `DevHandoffStackMouseBehavior`, `FGHitTest.cpp` | `fsSnappingOverlay`, `dragStartSnapTarget`, `clickSnapTarget`, `dragEndSnapTarget`, `cacheTemporarySnapTargetsForGaps`, `cacheTemporarySnapTargetsForImpliedFrameMargins`, `cacheTemporarySnapTargetsForLayoutGrids`, `canvasSpacingBetweenItems` | `MI-13` (`Alt/Option` red distance guides `#F24822` to hovered node or parent container), `MI-14` (Edge/center snap lines + equal-spacing distribution pills) |
| `KeyboardSelectBehavior`, `EditorPreferences` | `KeyboardSelectBehavior::selectableChild`, `_EditorPreferences_Internal___getProperty_smallNudgeAmount` (`1px`), `_EditorPreferences_Internal___getProperty_bigNudgeAmount` (`10px`) | `MI-01` (`1`/`10` step preferences), `MI-15` (`0–9` rapid opacity buffer), `MI-16` (`Enter`/`Shift+Enter`/`Tab`/`N`/`Home`/`End` traversal) |
| `fullscreen/lib/editor-ui/FGColorState.cpp`, `EyedropperBindings` | `_EyedropperBindings_destinationTool`, `canUseGlobalEyedropper`, `dropperPointerCursor`, `_PrototypingEditorState_Internal___getProperty_colorPickerEyedropperVarData` | `MI-04`, `MI-05` (Hex color channel stepping & shorthand expansion), `MI-19` (`I` Eyedropper magnifier loupe + live `#RRGGBB` sample pill) |
| Webpack Inspector Bundles (`0c62c2fd-cb7ba5a41c48c85a.min.js`) | `ScrubbableControl`, `valueBeforePreview`, `updatePreview`, `F="Mixed"` | `MI-01` (ArrowUp/Down stepping with full selection retention), `MI-02` (Infix math evaluator + `Mixed` relative math), `MI-03` (Prefix label scrubbing `1×`/`10×`/`0.1×` + `Alt+Wheel` + `Escape` rollback to `valueBeforePreview`) |

---

## Layer 2: State-First Micro-Interaction Behavior Specifications (`MI-01` .. `MI-20`)

Every micro-interaction below is specified using the canonical `es:micro-interactions` **Full Tier** schema:
1. **Intent & Context**
2. **State Set & Gesture Lifecycle**
3. **State Transition Table (`From State | Event | Guard | To State | Side Effect | Feedback`)**
4. **Interruption, Rollback & Edge Cases**
5. **`design:os studio` Codebase Audit Status**

---

### Domain 1: Inspector Numeric, Math, Scrubbing & Color Micro-Interactions (`MI-01` .. `MI-05`)

#### `MI-01`: Numeric Field Keyboard Stepping (`ArrowUp` / `ArrowDown` + Modifiers)
- **Intent:** Allow designers to nudge any numeric property (`X`, `Y`, `W`, `H`, `Rotation`, `Corner Radius`, `Opacity`, `Font Size`, `Line Height`, `Letter Spacing`, `Padding`, `Gap`, `Stroke Weight`) by `±1` (`smallNudgeAmount`), `±10` (`Shift`, `bigNudgeAmount`), or `±0.1` (`Alt/Option`, fine nudge) without losing focus or text highlight.
- **Context:** Any `widgets::field::num_field_full` or inline numeric input in the Right Properties Panel (`crates/designos-app/src/widgets/field.rs`).
- **States:** `Idle`, `FocusedSelected`, `EditingDraft`, `SteppingLive`, `Committed`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | `PointerClick` on field | `enabled == true` | `FocusedSelected` | Record `valueBeforePreview`; select all text in buffer | `1px` `#0D99FF` focus ring; full text highlight |
| `FocusedSelected` / `EditingDraft` | `KeyDown(ArrowUp)` | `Modifiers == None` | `SteppingLive` | Evaluate current buffer (or `base_val`), apply `+1.0`, clamp to `[min, max]`, commit to `Document` | Text updates to formatted `val + 1`; **full text selection stays highlighted** |
| `FocusedSelected` / `EditingDraft` | `KeyDown(ArrowDown)` | `Modifiers == None` | `SteppingLive` | Apply `-1.0`, clamp to `[min, max]`, commit to `Document` | Text updates to formatted `val - 1`; full text selection retained |
| `FocusedSelected` / `EditingDraft` | `KeyDown(ArrowUp \| ArrowDown)` | `Shift == true` | `SteppingLive` | Apply `±10.0`, clamp to `[min, max]`, commit to `Document` | Live canvas update + full text selection retained |
| `FocusedSelected` / `EditingDraft` | `KeyDown(ArrowUp \| ArrowDown)` | `Alt == true` | `SteppingLive` | Apply `±0.1` (rounded to `1e-2` to prevent float drift), clamp to `[min, max]` | Fine decimal readout (`12.1`) + full text selection retained |
| `SteppingLive` | `KeyDown(Escape)` | `true` | `Idle` | Restore `valueBeforePreview` on all selected nodes; drop focus | Field reverts to pre-focus value |
| `SteppingLive` | `KeyDown(Enter)` or `Blur` | `true` | `Committed` | Finalize undo group; advance or release focus | Focus ring clears |

- **Audit Status in `design:os studio`:** **IN_FLIGHT_STAGE_3** (`crates/designos-app/src/widgets/field.rs` — being upgraded by Inspector Worker).

---

#### `MI-02`: Multi-Operator Expression & `Mixed` Relative Math Evaluator
- **Intent:** Let users type full arithmetic expressions (`100+24*2`, `(320-32)/3`, `50%`, `+16`, `*1.5`, `/2`) in any numeric field, including when multiple selected nodes have heterogeneous (`Mixed`) values.
- **Context:** `widgets::field::eval_math_expression` / `eval_mixed_expression` (`crates/designos-app/src/widgets/field.rs`).
- **States:** `Idle`, `MixedPlaceholder`, `TypingExpression`, `EvaluatedCommit`, `RejectedRollback`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | Multi-selection has differing values | `values.windows(2).any(\|w\| w[0] != w[1])` | `MixedPlaceholder` | Display `"Mixed"` placeholder in muted text | Muted `"Mixed"` label |
| `FocusedSelected` / `MixedPlaceholder` | User types expression (`100+20*2`, `(400-40)/3`, `+20`, `*2`, `50%`) | Valid characters `0-9 . + - * / ( ) %` | `TypingExpression` | Buffer stores raw expression string | Monospace/tabular expression text |
| `TypingExpression` | `KeyDown(Enter)` or `Blur` | Unary relative operator (`+N`, `-N`, `*N`, `/N`, `N%`) on `Mixed` or single value | `EvaluatedCommit` | For each selected node `i`, compute `new_val[i] = apply_relative(node_val[i], expr)` clamped to `[min, max]` | Each node updates proportionally; field shows new value (or `"Mixed"` if still heterogeneous) |
| `TypingExpression` | `KeyDown(Enter)` or `Blur` | Full infix expression (`Shunting-Yard` with `+`, `-`, `*`, `/`, parentheses, unary `±`, `%`) | `EvaluatedCommit` | Parse & evaluate with operator precedence (`* /` > `+ -`); apply result to selection | Field replaces expression with formatted numeric result |
| `TypingExpression` | `KeyDown(Enter)` or `Blur` | Division by zero, NaN, or syntax error (`100/0`, `abc`) | `RejectedRollback` | Do NOT mutate document; restore `valueBeforePreview` | Field smoothly reverts to previous valid number |

- **Audit Status in `design:os studio`:** **IN_FLIGHT_STAGE_3** (`crates/designos-app/src/widgets/field.rs` previously had single-operator `eval_simple`; full Shunting-Yard + `Mixed` relative math is in Stage 3 scope).

---

#### `MI-03`: Prefix / Icon Label Scrubbing & `Alt + ScrollWheel` Scrub
- **Intent:** Allow continuous drag-scrubbing on any numeric field's prefix label (`X`, `Y`, `W`, `H`, `R`, `∠`, `%`, `Pad`, `Gap`) and hover-wheel scrubbing (`Alt + ScrollWheel`) with live canvas preview and single-step undo coalescing.
- **Context:** `widgets::field::scrub_prefix` (`crates/designos-app/src/widgets/field.rs`).
- **States:** `Idle`, `HoverPrefix`, `PointerDownPending`, `ScrubbingActive`, `FocusedInlineEdit`, `Committed`, `Cancelled`.
- **Gesture Lifecycle:**
  - `Begin`: `PointerDown` on prefix zone (`18px` left zone); capture `valueBeforePreview` and `drag_origin_x`.
  - `Threshold`: `|pointer.x - drag_origin_x| >= 2.0px` transitions `PointerDownPending -> ScrubbingActive`; releasing before `< 2.0px` transitions `PointerDownPending -> FocusedInlineEdit` (focuses the text input and selects all).
  - `Update`: `delta_val = dx * step * multiplier` where `multiplier = 10.0` (`Shift`), `0.1` (`Alt`), or `1.0` (default).
  - `Commit / Cancel`: `PointerUp` commits coalesced `Tx` to undo stack; `Escape` during drag reverts to `valueBeforePreview` and cancels drag.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | `PointerHover` over prefix | `true` | `HoverPrefix` | Set cursor `CursorIcon::ResizeHorizontal` (`ew-resize`) | Prefix label brightens to primary text color |
| `HoverPrefix` | `PointerDown(Primary)` | `true` | `PointerDownPending` | Snapshot `valueBeforePreview = current_val`, `start_x = pointer.x` | Cursor remains `ResizeHorizontal` |
| `PointerDownPending` | `PointerUp(Primary)` | `\|pointer.x - start_x\| < 2.0` | `FocusedInlineEdit` | Request focus on text edit; select all characters | `1px` `#0D99FF` focus ring + selected text |
| `PointerDownPending` / `ScrubbingActive` | `PointerMove(dx)` | `\|pointer.x - start_x\| >= 2.0` | `ScrubbingActive` | Compute `multiplier = if shift { 10.0 } else if alt { 0.1 } else { 1.0 }`; update live value clamped to `[min, max]` | Live canvas geometry/style preview; cursor locked to `ResizeHorizontal` |
| `ScrubbingActive` | `KeyDown(Escape)` | `true` | `Cancelled` | Revert node property to `valueBeforePreview`; abort drag | Canvas & field snap back to initial value |
| `ScrubbingActive` | `PointerUp(Primary)` | `true` | `Committed` | Commit single coalesced undo entry | Normal cursor restored |
| `Idle` / `HoverPrefix` | `MouseWheel(dy)` | `Alt == true` and pointer over field | `Committed` | Step value by `sign(dy) * step * (if shift { 10.0 } else { 1.0 })` | Immediate value + canvas update without stealing keyboard focus |

- **Audit Status in `design:os studio`:** **IN_FLIGHT_STAGE_3** (`crates/designos-app/src/widgets/field.rs`).

---

#### `MI-04`: Hex Color Field Per-Channel (`RR`/`GG`/`BB`) Keyboard Stepping
- **Intent:** When editing a 6-digit hex color (`#RRGGBB`), pressing `ArrowUp` or `ArrowDown` increments or decrements the specific color channel (`R`, `G`, or `B`) where the caret is positioned (or all three channels together—luminance nudge—when the entire hex string is selected).
- **Context:** Fill/Stroke hex input in `panels/properties.rs` and `widgets/picker.rs`.
- **States:** `Idle`, `HexAllSelected`, `HexCaretInChannel(Channel)`, `SteppingHex`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | `Focus` hex input | `true` | `HexAllSelected` | Select all 6 hex chars (`0..6`) | Full `#RRGGBB` text highlighted |
| `HexAllSelected` | `KeyDown(ArrowUp \| ArrowDown)` | Selection spans `>= 3` chars | `SteppingHex` | Step `R`, `G`, and `B` simultaneously by `±1` (or `±10` with `Shift`), clamped to `[0x00, 0xFF]` | All 3 channels brighten/darken uniformly; full selection preserved |
| `HexAllSelected` | `PointerClick` or `ArrowLeft/Right` inside text | Caret at index `0..=1` (`R`), `2..=3` (`G`), or `4..=5` (`B`) | `HexCaretInChannel(ch)` | Track active channel `ch ∈ {R, G, B}` | Caret placed at target channel |
| `HexCaretInChannel(ch)` | `KeyDown(ArrowUp \| ArrowDown)` | Caret in `R` (`0..=2`), `G` (`2..=4`), or `B` (`4..=6`) | `SteppingHex` | Step only channel `ch` by `±1` (`±10` with `Shift`), clamped to `[0x00, 0xFF]` | Target channel pair updates (`#1E293B -> #1E333B`); caret position preserved |

- **Audit Status in `design:os studio`:** **IN_FLIGHT_STAGE_3** (`crates/designos-app/src/widgets/picker.rs` / `widgets/field.rs`).

---

#### `MI-05`: Smart Hex Shorthand Expansion (`1–8` Digits + CSS Named Colors)
- **Intent:** Expand any shorthand hex or CSS color name typed into a color input into canonical `#RRGGBB` (plus optional alpha `AA`) on commit, matching Figma's `parseColor` behavior.
- **Context:** `widgets::picker::parse_smart_hex` (`crates/designos-app/src/widgets/picker.rs` & `panels/properties.rs`).
- **Expansion Rules (`FACT` from Figma color parser):**
  - `1` hex char `X` (`"f"`): `XXXXXX` -> `#FFFFFF`
  - `2` hex chars `XY` (`"80"`): `XYXYXY` -> `#808080`
  - `3` hex chars `RGB` (`"f0a"`): `RRGGBB` -> `#FF00AA`
  - `4` hex chars `RGBA` (`"f0a8"`): `RRGGBB` + `alpha = 0x88 / 255.0` -> `#FF00AA` (`53%`)
  - `6` hex chars `RRGGBB` (`"0d99ff"`): `#0D99FF`
  - `8` hex chars `RRGGBBAA` (`"0d99ff80"`): `#0D99FF` + `alpha = 0x80 / 255.0` (`50%`)
  - CSS Named Colors (`"red"`, `"blue"`, `"white"`, `"black"`, `"transparent"`, `"coral"`, `"rebeccapurple"`, etc.): resolved to canonical `[R, G, B, A]`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `EditingHex` | `KeyDown(Enter)` or `Blur` | Input matches `1, 2, 3, 4, 6, 8` hex digits or CSS color name | `Committed` | Update node fill/stroke RGB (and opacity if 4/8-digit or `"transparent"`); format display string as uppercase `RRGGBB` | Swatch & canvas update; input normalizes to `6`-char uppercase hex |
| `EditingHex` | `KeyDown(Enter)` or `Blur` | Input is invalid (`"zzzz"`) | `RejectedRollback` | Revert to previous valid hex color | Input restores prior `RRGGBB` |

- **Audit Status in `design:os studio`:** **IN_FLIGHT_STAGE_3** (`crates/designos-app/src/widgets/picker.rs`).

---

### Domain 2: Canvas On-Object Direct Manipulation Handles (`MI-06` .. `MI-12`)

#### `MI-06`: On-Canvas Corner Radius Circular Handles (`EditShapeBehavior` / `CornerRadiusValues`)
- **Intent:** Display 4 inset circular handles inside a selected Rectangle/Frame/Component/Instance when hovered or zoomed in, allowing direct drag to round all 4 corners simultaneously or a single corner independently with `Alt/Option`.
- **REA Evidence:** `EditShapeBehavior`, `MutableEditShapeBehaviorState`, `CornerRadiusValues`, `topLeftCornerRadius..bottomRightCornerRadius`, `rectangleCornerToolIndependent`.
- **Context:** `canvas/handles.rs:38-70` (`corner_radius_handle_positions`, `hit_corner_radius_handle`), `canvas/interact.rs:474-484, 667-685` (`Interaction::CornerRadius`), `canvas/draw.rs:466-474, 506-510`.
- **States:** `Hidden`, `VisibleIdle`, `HoverHandle(CornerIdx)`, `DraggingUniform`, `DraggingSingleCorner(CornerIdx)`, `Committed`, `Cancelled`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Hidden` | Single `Rectangle`/`Frame`/`Component`/`Instance` selected | `min(screen_w, screen_h) >= 40.0px` | `VisibleIdle` | Compute 4 inset handle positions at `max(10px / zoom, radius + 4px / zoom)` from each corner | 4 `4.5px` white circles with `1.5px` `#0D99FF` stroke |
| `VisibleIdle` | `PointerDown` on corner handle `i` | `Alt == false` | `DraggingUniform` | Snapshot `Document`; start `Interaction::CornerRadius { single_corner: false }` | Cursor `Crosshair`; live `R <val>` pill badge (`#0D99FF`) |
| `VisibleIdle` | `PointerDown` on corner handle `i` | `Alt == true` | `DraggingSingleCorner(i)` | Snapshot `Document`; start `Interaction::CornerRadius { single_corner: true }` | Only corner `i` rounds; `CornerRadius::PerCorner` written |
| `DraggingUniform` / `DraggingSingleCorner` | `PointerMove` | `true` | `Dragging*` | `r = ((dx_inward + dy_inward) / sqrt(2)).clamp(0.0, min(W, H) / 2.0)`; toggle `single_corner` live if `Alt` pressed/released mid-drag | Live corner rounding + floating `R <r>` badge near active corner |
| `Dragging*` | `KeyDown(Escape)` | `true` | `Cancelled` | `app.cancel_interaction()` restores pre-drag snapshot | Geometry & badge revert immediately |
| `Dragging*` | `PointerUp` | `true` | `Committed` | Push single undo entry via `finish_snapshot` | Badge hides; handles settle at new inset positions |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`canvas/handles.rs:38-70`, `canvas/interact.rs:474-484, 667-685`, `canvas/draw.rs:466-474, 506-510`).

---

#### `MI-07`: On-Canvas Ellipse Arc Sweep, Donut Inner-Ratio & Start-Angle Handles
- **Intent:** Allow direct manipulation of an `Ellipse` node's **Arc Sweep** (`angle_end`, `0°..360°`), **Donut Inner Ratio** (`inner_radius`, `0%..100%`), and **Start Angle** (`angle_start`, `-180°..180°`) via on-canvas circular handles.
- **REA Evidence:** `EditShapeBehavior`, `ArcSweep`, `arcData` (`startingAngle`, `endingAngle`, `innerRadius`).
- **Context:** `canvas/handles.rs:154-174` (`ShapeControlHandle::EllipseSweep`, `EllipseRatio`, `EllipseStartAngle`), `canvas/interact.rs`, `canvas/draw.rs:497-502`.
- **States:** `VisibleIdle`, `DraggingSweep`, `DraggingInnerRatio`, `DraggingStartAngle`, `Committed`, `Cancelled`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `VisibleIdle` | `PointerDown` on `EllipseSweep` handle (at `angle_end` on outer rim) | `min(screen_w, screen_h) >= 32px` | `DraggingSweep` | Snapshot `Document`; track polar angle `atan2(dy, dx)` around ellipse center (`Shift` snaps angle to `15°` increments) | Ellipse splits into open sector/arc; floating `Sweep <deg>°` badge |
| `VisibleIdle` | `PointerDown` on `EllipseRatio` handle (at center or `inner_radius` along bisector) | `min(screen_w, screen_h) >= 32px` | `DraggingInnerRatio` | Snapshot `Document`; compute `inner_radius = (dist_from_center / outer_radius).clamp(0.0, 0.95)` | Ellipse hollows into a donut ring; floating `Ratio <pct>%` badge |
| `VisibleIdle` | `PointerDown` on `EllipseStartAngle` handle (at `angle_start` on rim when arc is open) | `(angle_end - angle_start).abs() < 360.0` | `DraggingStartAngle` | Snapshot `Document`; update `angle_start` from polar angle (`Shift` snaps to `15°`) | Start edge rotates around center; floating `Start <deg>°` badge |
| `Dragging*` | `KeyDown(Escape)` | `true` | `Cancelled` | Revert snapshot via `cancel_interaction` | Ellipse restores prior arc/donut state |
| `Dragging*` | `PointerUp` | `true` | `Committed` | Commit single undo entry via `finish_snapshot` | Floating badge hides |

- **Audit Status in `design:os studio`:** **STAGE_3_GAP (Visual handles rendered, Drag interaction missing!)** — `canvas/handles.rs:154-174` computes `EllipseSweep` and `EllipseRatio` positions and `canvas/draw.rs:497-502` draws them, **but `try_start_special_handle` in `canvas/interact.rs:470-517` does NOT hit-test or drag `ShapeControlHandle` yet!**

---

#### `MI-08`: On-Canvas Polygon & Star Point-Count (`3..60`) and Inner-Radius Ratio (`0%..100%`) Handles
- **Intent:** Allow dragging on-canvas control handles on a `Polygon` or `Star` to adjust vertex count (`point_count`, `3..=60`) and, for `Star`, the inner vertex radius ratio (`inner_radius`, `0.05..=1.0`).
- **REA Evidence:** `EditShapeBehavior`, `StarInnerScale`, `count` handle in Figma's shape editor.
- **Context:** `canvas/handles.rs:175-195` (`ShapeControlHandle::PolygonPoints`, `StarPoints`, `StarInnerRatio`), `canvas/interact.rs`, `canvas/draw.rs:497-502`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `VisibleIdle` | `PointerDown` on `PolygonPoints` / `StarPoints` handle | `min(screen_w, screen_h) >= 32px` | `DraggingPointCount` | Snapshot `Document`; map vertical/angular drag delta (`12px` per vertex step) to `point_count = (initial_count + delta_steps).clamp(3, 60)` | Live polygon/star vertex regeneration + floating `Count <N>` badge |
| `VisibleIdle` | `PointerDown` on `StarInnerRatio` valley handle | `NodeKind::Star` | `DraggingStarRatio` | Snapshot `Document`; set `inner_radius = (dist_from_center / outer_radius).clamp(0.05, 1.0)` | Star spikes sharpen or flatten live + floating `Ratio <pct>%` badge |
| `Dragging*` | `KeyDown(Escape)` | `true` | `Cancelled` | Restore snapshot | Revert to initial vertex count / ratio |
| `Dragging*` | `PointerUp` | `true` | `Committed` | Commit single undo entry | Badge hides |

- **Audit Status in `design:os studio`:** **STAGE_3_GAP (Visual handles rendered, Drag interaction missing!)** — `canvas/handles.rs:175-195` and `canvas/draw.rs:497-502` draw the handles, **but `canvas/interact.rs` does not hit-test or drag `ShapeControlHandle` yet.**

---

#### `MI-09`: Auto-Layout On-Canvas Pink Padding Handles (`[Top, Right, Bottom, Left]`)
- **Intent:** Render 4 pink (`#EC4899`) pill handles inside a selected Auto-Layout frame's padding edges; dragging adjusts single-side padding, `Alt+Drag` mirrors opposite sides (`Top+Bottom` or `Left+Right`), `Shift+Alt+Drag` (or `Shift+Drag`) adjusts all 4 sides uniformly, and **clicking without dragging opens an inline numeric input** on canvas.
- **REA Evidence:** `StackAndGridControlDragBehavior`, `StackAndGridControlClickBehavior`, `_StackFacetTsApiGenerated_getStackPaddingTop/Right/Bottom/Left`, `_setStackPadding*`, `_StackBindingsUiCpp_tabActiveStackRegion`.
- **Context:** `canvas/handles.rs:74-150`, `canvas/interact.rs:485-516, 686-734` (`Interaction::AutoLayoutDrag`), `canvas/draw.rs:477-495, 511-521`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `VisibleIdle` | `PointerDown` on `AutoLayoutHandle::Padding(side)` | `true` | `PendingClickOrDrag` | Snapshot `Document`; record `start_pos` and `initial_padding` | Pink padding band highlights (`rgba(236,72,153,0.18)`) |
| `PendingClickOrDrag` | `PointerMove` | `dist >= 2.0px` | `DraggingPadding` | Compute inward `delta >= 0`; if `Shift` -> set all 4 sides; else if `Alt` -> set `side` + `opposite(side)`; else set `side`; call `doc.relayout_tree(id)` | Live child reflow + pink `Pad <val>` pill badge |
| `PendingClickOrDrag` | `PointerUp` | `dist < 2.0px` | `InlinePaddingEdit` | Open inline numeric input anchored at the clicked pink padding bar (`Tab` cycles `Top -> Right -> Bottom -> Left`) | Floating `#EC4899`-bordered inline input |
| `DraggingPadding` | `KeyDown(Escape)` | `true` | `Cancelled` | Revert snapshot & relayout | Children snap back to initial padding |
| `DraggingPadding` | `PointerUp` | `true` | `Committed` | Commit single undo entry | Pink badge hides |

- **Audit Status in `design:os studio`:** **PARTIAL (Drag + `Alt`/`Shift` modifiers + pink badge implemented; Click-to-inline-edit & Inspector-hover pink preview are Stage 3 gaps)**.

---

#### `MI-10`: Auto-Layout On-Canvas Pink Item-Spacing (`Gap`) Handle
- **Intent:** Render pink (`#EC4899`) gap struts between consecutive flow children in an Auto-Layout frame; dragging any gap strut adjusts `item_spacing` (`>= 0`) for the entire stack with live reflow.
- **REA Evidence:** `StackAndGridControlDragBehavior`, `ChangeListSpacingBehavior`, `_StackFacetTsApiGenerated_getStackSpacing`, `_setStackSpacing`.
- **Context:** `canvas/handles.rs:116-148` (`AutoLayoutHandle::Gap`), `canvas/interact.rs:495-516, 725-734`, `canvas/draw.rs:487-495, 515-521`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `VisibleIdle` | Flow children count `>= 2` | `true` | `VisibleIdle` | Compute gap rects between adjacent flow children along primary axis | Pink `2px` center strut (`#EC4899`) between children |
| `VisibleIdle` | `PointerDown` on `AutoLayoutHandle::Gap(idx)` | `true` | `DraggingGap` | Snapshot `Document`; record `initial_gap = al.item_spacing` | Pink gap region shaded (`rgba(236,72,153,0.16)`) |
| `DraggingGap` | `PointerMove` | `true` | `DraggingGap` | `al.item_spacing = (initial_gap + primary_axis_delta).max(0.0).round()`; `doc.relayout_tree(id)` | Live stack expansion/contraction + pink `Gap <val>` pill badge |
| `DraggingGap` | `KeyDown(Escape)` | `true` | `Cancelled` | Restore snapshot | Stack reverts to initial spacing |
| `DraggingGap` | `PointerUp` | `true` | `Committed` | Commit single undo entry | Pink badge hides |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (Drag + live `relayout_tree` + pink `Gap <val>` badge implemented in `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs`).

---

#### `MI-11`: Resize Gesture Live Modifiers (`Shift` Aspect Lock, `Alt` Center Origin, `Cmd/Ctrl` Ignore Constraints)
- **Intent:** Dynamically recompute node resize geometry on every pointer move AND modifier key toggle (`Shift`, `Alt`, `Cmd/Ctrl`) mid-drag (`FGResizeSelectionBehavior`).
- **REA Evidence:** `FGResizeSelectionBehavior.cpp`, `ResizeSelectionBehavior`, `ScaleToolObserver`.
- **Context:** `canvas/handles.rs:235-296` (`resize_local`), `canvas/interact.rs:885-975` (`update_resize`), `ops.rs:950-1005` (`apply_resize`).
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | `PointerDown` on 8-way resize handle | `true` | `Resizing` | Snapshot `Document`; store `orig_bounds`, `orig_aspect = W / H`, and per-node initial transforms | Live `W × H` dimension pill below selection |
| `Resizing` | `PointerMove` or `ModifierChange` | `Shift == true` | `Resizing` | Lock `new_w / new_h == orig_aspect` | Proportional resize; `W × H` badge updates |
| `Resizing` | `PointerMove` or `ModifierChange` | `Alt == true` | `Resizing` | Mirror delta symmetrically across `orig_bounds.center()` (`2×` delta from center) | Symmetric center-anchored resize |
| `Resizing` | `PointerMove` or `ModifierChange` | `Cmd/Ctrl == true` | `Resizing` | Pass `apply_constraints = false` into `apply_resize` so child nodes do NOT scale/pin while the parent frame bounds resize | Parent frame resizes freely without moving/stretching children |
| `Resizing` | `KeyDown(Escape)` | `true` | `Cancelled` | Revert snapshot | Bounds & children restore |
| `Resizing` | `PointerUp` | `true` | `Committed` | Commit single undo entry | `W × H` badge settles |

- **Audit Status in `design:os studio`:** **PARTIAL (`Shift` aspect lock and `Alt` center-origin are IMPLEMENTED; `Cmd/Ctrl` ignore child constraints during `update_resize` is a Stage 3 gap)**.

---

#### `MI-12`: Draw / Create & Move Gesture Live Modifiers (`Shift`, `Alt`, `Space` Reposition Origin / Suppress Auto-Parent)
- **Intent:** While drawing a new shape (`R`, `O`, `F`, `L`) or marquee-selecting, holding `Space` translates the entire shape/marquee origin (`start_pos += pointer_delta`) instead of resizing it; while dragging an existing selection (`Interaction::Drag`), holding `Space` suppresses automatic reparenting into hovered Frames/Auto-Layout containers.
- **REA Evidence:** `FGMoveSelectionBehavior.cpp` (`MoveSelectionBehavior`), `CreateShapeBehavior`.
- **Context:** `canvas/create.rs:64-100` (`draw_geom`), `canvas/interact.rs:820-884` (`update_drag`), `canvas/interact.rs:976-1020` (`update_draw`, `update_marquee`).
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `DrawingShape` / `MarqueeSelecting` | `PointerMove(delta)` | `Space == true` | `TranslatingOrigin` | Translate `start_world += delta_world` so `(width, height)` stay frozen while `(x, y)` moves with cursor | Ghost shape / marquee box slides across canvas at fixed size |
| `DrawingShape` | `PointerMove` | `Space == false` | `DrawingShape` | Recompute `draw_geom(start, cur, shift_square, alt_center)` | Live ghost shape + `W × H` pill |
| `DraggingSelection` | `PointerMove` | `Space == true` | `DraggingNoReparent` | Move selection by `delta` but skip `reparent_selection_under_pointer` / Auto-Layout insertion slot preview | Node floats over frames without being swallowed into them |
| `DraggingSelection` | `PointerMove` | `Alt == true` (on drag start or mid-drag) | `DraggingDuplicate` | Clone selected nodes at drag origin; drag the clones | Original nodes remain at origin; clones move under cursor |

- **Audit Status in `design:os studio`:** **PARTIAL (`Shift`, `Alt` center-draw, and `Alt+Drag` duplicate are IMPLEMENTED; holding `Space` to pan origin during `Draw`/`Marquee` and holding `Space` to suppress auto-reparenting during `Drag` are Stage 3 gaps)**.

---

### Domain 3: Canvas Spatial Feedback, Snapping & Traversal (`MI-13` .. `MI-16`)

#### `MI-13`: `Alt/Option` Hover Distance Measurement Guides (`DevHandoffMeasurementMouseBehavior`)
- **Intent:** When `Alt/Option` is held with a non-empty selection while idle on the canvas, render crisp `#F24822` (Figma red-orange) distance lines and pixel readout pills from the selection bounds to the hovered target node (or to the 4 inner edges of the parent Frame when hovering empty space inside the parent).
- **REA Evidence:** `DevHandoffMeasurementMouseBehavior`, `MutableCanvasSnapper`, `fsSnappingOverlay`.
- **Context:** `canvas/snap.rs:191-250` (`compute_alt_measures`), `canvas/interact.rs:1486-1505`, `canvas/draw.rs:643-659`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `SelectionIdle` | `KeyHold(Alt)` + `PointerHover` over other node `T` | `!selection.contains(T)` | `MeasuringToTarget` | `compute_alt_measures(sel_bounds, Some(T_bounds), parent_bounds)` computes horizontal/vertical gap lines & perpendicular projection ticks | `#F24822` `1px` measurement lines + `#F24822` pill badges with white `<px>` text |
| `SelectionIdle` | `KeyHold(Alt)` + `PointerHover` on empty canvas/parent | `selection has parent Frame P` | `MeasuringToContainer` | `compute_alt_measures(sel_bounds, None, Some(P_bounds))` computes 4 distances (`Top`, `Right`, `Bottom`, `Left`) to parent frame edges | 4 `#F24822` distance lines + pixel pills to parent edges |
| `Measuring*` | `KeyUp(Alt)` or `SelectionCleared` | `true` | `SelectionIdle` | `app.canvas.measures.clear()` | Measurement overlay disappears immediately |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`canvas/snap.rs:191-250`, `canvas/interact.rs:1486-1505`, `canvas/draw.rs:643-659`).

---

#### `MI-14`: Smart Alignment Guides & Equal-Spacing Distribution Badges (`MutableCanvasSnapper`)
- **Intent:** During `Drag` or `Resize`, snap selection edges and centers (`left`, `center_x`, `right`, `top`, `center_y`, `bottom`) to sibling/parent bounds within `5px / zoom`, and detect equal horizontal/vertical gaps between 3+ colinear siblings.
- **REA Evidence:** `MutableCanvasSnapper`, `cacheTemporarySnapTargetsForGaps`, `canvasSpacingBetweenItems`.
- **Context:** `canvas/snap.rs:103-187` (`snap_rect`, `measurements`), `canvas/draw.rs:630-659`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Dragging` / `Resizing` | `PointerMove` | `\|edge_delta\| <= 5.0 / zoom` | `SnappedToGuide` | Offset delta by `snap.dx, snap.dy`; populate `app.canvas.guides` and `app.canvas.measures` | `#F24822` alignment lines with `×` endpoint ticks + distance pills |
| `Dragging` / `Resizing` | `PointerMove` | `Alt == true` (disable snap) or `\|edge_delta\| > 5.0 / zoom` | `FreeMove` | Clear guides | Unsnapped smooth cursor tracking |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`canvas/snap.rs:103-187`, `canvas/draw.rs:630-659`).

---

#### `MI-15`: Rapid `0–9` Opacity Shortcut Key Buffer (`400ms` Two-Digit Window)
- **Intent:** When nodes are selected and no text input is focused, pressing `1`..`9` sets layer opacity to `10%..90%`, `0` sets `100%`, `00` within `400ms` sets `0%`, and any two digits `AB` within `400ms` (e.g. `4` then `5`) sets `45%`.
- **Context:** `ops.rs:1008-1028` (`apply_opacity_digit`), `shell.rs:593-619`, `app.rs:181` (`opacity_buffer: Option<(u8, f64)>`).
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `BufferEmpty` | `KeyDown(d ∈ 0..=9)` | `!selection.is_empty()` & no text focus | `FirstDigitLatched(d, t0)` | Set `opacity = if d == 0 { 1.0 } else { d as f32 / 10.0 }`; store `opacity_buffer = Some((d, now))` | Selected nodes & Appearance `Opacity %` field update immediately |
| `FirstDigitLatched(prev, t0)` | `KeyDown(d ∈ 0..=9)` | `now - t0 <= 0.400s` | `BufferEmpty` | Set `opacity = if prev == 0 && d == 0 { 0.0 } else { (prev * 10 + d) as f32 / 100.0 }`; clear `opacity_buffer = None` | Exact 2-digit opacity (`45%` or `0%`) applied |
| `FirstDigitLatched(prev, t0)` | `KeyDown(d ∈ 0..=9)` | `now - t0 > 0.400s` | `FirstDigitLatched(d, now)` | Treat as fresh single digit (`0 -> 100%`, `d -> d*10%`) | Single-digit decade opacity applied |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`ops.rs:1008-1028`, `shell.rs:593-619`).

---

#### `MI-16`: Hierarchy & Spatial Keyboard Traversal (`Enter`, `Shift+Enter`, `\`, `Tab`, `Shift+Tab`, `N`, `Shift+N`, `Home`, `End`, `PageUp`, `PageDown`)
- **Intent:** Traverse the scene graph and canvas frames entirely from the keyboard (`KeyboardSelectBehavior::selectableChild`).
- **REA Evidence:** `KeyboardSelectBehavior::selectableChild`.
- **Context:** `canvas/interact.rs:1087-1120` (`navigate_hierarchy_enter`), `shell.rs:524-592`, `ops.rs`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `ContainerSelected` | `KeyDown(Enter)` | Selected node has children | `ChildrenSelected` | Replace selection with direct children of selected container(s) | Selection bounding boxes jump to children |
| `LeafTextOrVectorSelected` | `KeyDown(Enter)` | Single `Text` or `Vector`/shape selected | `EditingLeaf` | Enter inline text editing (`begin_text_edit`) or vector network edit (`vector::enter`) | Text caret or vector vertex handles appear |
| `NodeSelected` | `KeyDown(Shift+Enter)` or `KeyDown(\)` | Selected node has non-Page parent | `ParentSelected` | Replace selection with parent container(s) | Selection outline expands to parent frame/group |
| `NodeSelected` | `KeyDown(Tab)` / `KeyDown(Shift+Tab)` | `!mods.alt` | `SiblingSelected` | Select next (`Tab`) or previous (`Shift+Tab`) sibling in parent's `children` list (wrapping around) | Selection cycles across siblings |
| `Any` | `KeyDown(N)` / `KeyDown(Shift+N)` | Design mode | `FrameFocused` | Select next/previous top-level Frame in reading order (`top-to-bottom, left-to-right`) and pan/fit viewport | Viewport centers on target frame |
| `Any` | `KeyDown(PageUp)` / `KeyDown(PageDown)` | Multi-page doc | `PageSwitched` | Switch to previous/next Page in `doc.pages` | Left Layers panel & canvas switch active page |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`shell.rs:524-592`, `canvas/interact.rs:1087-1120`, `ops.rs`).

---

### Domain 4: Layers Panel, Eyedropper & Component/Instance Micro-Interactions (`MI-17` .. `MI-20`)

#### `MI-17`: Bi-Directional Layer ↔ Canvas Hover Outline
- **Intent:** Hovering a row in the Left Layers Panel draws a crisp `1.5px` `#0D99FF` hover outline around that node on the canvas (`app.hover_layer`); conversely, hovering a node on the canvas (`app.hover`) subtly highlights the corresponding row in the Left Layers Panel so the user always sees cross-surface correspondence.
- **Context:** `panels/layers.rs:365-380`, `canvas/draw.rs:567-573`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `Idle` | `PointerHover` on Layer Row `id` | `true` | `LayerHovered(id)` | Set `app.hover_layer = Some(id)` | Canvas draws `1.5px` `#0D99FF` outline on `id`; Layer row shows `bg_hover` |
| `Idle` | `PointerHover` on Canvas Node `id` | `app.hover == Some(id)` | `CanvasHovered(id)` | Layer row for `id` checks `resp.hovered() \|\| app.hover == Some(row.id)` | Corresponding row in Left Layers Panel renders subtle hover fill (`bg_hover`) and `1px` left accent bar |

- **Audit Status in `design:os studio`:** **PARTIAL (Layers → Canvas outline in `canvas/draw.rs:567` is IMPLEMENTED; Canvas → Layers row highlight in `panels/layers.rs:367` is a Stage 3 gap)**.

---

#### `MI-18`: Layer Row Hover Quick-Actions (`Eye` / `Lock`) & Inline Rename (`Cmd+R` / Double-Click / `Tab` Next Layer)
- **Intent:** Keep layer rows clean at rest (only showing `Eye` or `Lock` icons when hidden or locked), reveal both `Eye` and `Lock` toggle buttons on row hover, and allow rapid batch renaming where pressing `Tab` (or `Shift+Tab`) inside the inline rename input commits the current name and immediately opens inline rename on the next (or previous) visible layer row.
- **Context:** `panels/layers.rs:413-474`, `shell.rs:437` (`Cmd+R` -> `Action::Rename`).
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `RowIdle` | `PointerHover` on row | `true` | `RowHovered` | Reveal `Lock` (`Unlock`) and `Eye` (`EyeOff`) icon buttons at right edge of row | `14×14` icon buttons appear at right of row |
| `RowHovered` | `Click(Eye)` / `Click(Lock)` | `true` | `RowHovered` | Toggle `node.visible` / `node.locked` in undoable `edit_nodes` | Node hides/locks on canvas; icon stays visible even when unhovered if `!visible \|\| locked` |
| `RowSelected` | `DoubleClick` on label or `Cmd+R` | `true` | `RenamingRow(id)` | Set `app.layers.renaming = Some(id)`, `rename_buf = node.name`, `rename_focus_requested = true` | Inline `TextEdit` replaces row label with full text selected |
| `RenamingRow(id)` | `KeyDown(Tab)` / `KeyDown(Shift+Tab)` | Next/prev visible row `next_id` exists | `RenamingRow(next_id)` | Commit `rename_buf` to `id`; select `next_id`; set `renaming = Some(next_id)`, `rename_buf = next_name`, `rename_focus_requested = true` | Seamless spreadsheet-style rename across consecutive layers |
| `RenamingRow(id)` | `KeyDown(Enter)` or `Blur` | `true` | `RowSelected` | Commit `rename_buf` to `id`; clear `renaming = None` | Updated layer name shown |
| `RenamingRow(id)` | `KeyDown(Escape)` | `true` | `RowSelected` | Discard `rename_buf`; clear `renaming = None` | Original layer name preserved |

- **Audit Status in `design:os studio`:** **PARTIAL (Hover `Eye`/`Lock`, `Cmd+R`, and double-click rename are IMPLEMENTED; `Tab`/`Shift+Tab` inside inline rename to advance to the next/previous layer row is a Stage 3 gap in `panels/layers.rs:413-436`)**.

---

#### `MI-19`: On-Canvas Eyedropper (`I`) Magnifier Loupe & Live Hex Pill
- **Intent:** Pressing `I` on the canvas (or clicking the Eyedropper icon in the Color Picker) enters Eyedropper sampling mode with a floating circular zoomed pixel-grid **Magnifier Loupe** (`9×9` magnified pixel cells, center `1px` target reticle, outer ring tinted with the sampled color, and a dark `#1E1E1E` pill badge below reading `#RRGGBB`); clicking applies the sampled color to the selected nodes' active fill (or open picker target), and `Escape` cancels.
- **REA Evidence:** `_EyedropperBindings_destinationTool`, `canUseGlobalEyedropper`, `dropperPointerCursor`, `FGColorState.cpp`.
- **Context:** `shell.rs`, `panels/properties.rs:1458-1473`, `canvas/draw.rs`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `CanvasIdle` | `KeyDown(I)` (no modifiers) or `Click(PickerEyedropper)` | `!text_editing` | `EyedropperActive` | Enable `app.canvas.eyedropper_active = true` (or `pk.eyedrop = true`) | Cursor `Crosshair`; floating circular Magnifier Loupe (`72px` diameter, `9×9` pixel grid + `#RRGGBB` pill) tracks pointer |
| `EyedropperActive` | `PointerMove(p)` | `canvas_rect.contains(p)` | `EyedropperActive` | Sample `3×3` / `9×9` neighborhood around `p` via scene hit/raster sampler | Loupe grid & `#RRGGBB` pill update at `60fps` |
| `EyedropperActive` | `PointerDown(Primary)` | `canvas_rect.contains(p)` | `Committed` | If picker open -> update picker color; else apply sampled `[R,G,B]` to selected nodes' primary solid fill (`edit_nodes`); exit eyedropper | Selected nodes adopt sampled color; loupe closes |
| `EyedropperActive` | `KeyDown(Escape)` | `true` | `Cancelled` | Exit eyedropper without mutating document | Loupe closes; prior cursor restored |

- **Audit Status in `design:os studio`:** **STAGE_3_GAP (Picker eyedropper click-sampling exists in `panels/properties.rs:1458-1473`, but standalone `I` shortcut on canvas and the visual zoomed Magnifier Loupe + `#RRGGBB` badge in `canvas/draw.rs` are missing)**.

---

#### `MI-20`: Component / Instance & Auto-Layout Structural Micro-Gestures (`Shift+A`, `Alt+Shift+A`, `Alt+Drag`, Double-Click Drill-In)
- **Intent:** Provide instant 1-gesture structural wrapping/unwrapping (`Shift+A` wrap selection in Auto-Layout inferring horizontal vs vertical direction and average gap; `Alt+Shift+A` unwrap Auto-Layout preserving world positions), `Alt+Drag` duplication, and double-click container drill-in.
- **Context:** `shell.rs:575-576`, `ops.rs`, `canvas/interact.rs:334-365, 753-776`.
- **State Transition Table:**

| From State | Event | Guard | To State | Side Effect | Feedback |
|---|---|---|---|---|---|
| `NodesSelected` | `KeyDown(Shift+A)` | `!selection.is_empty()` | `AutoLayoutWrapped` | Infer direction (`Horizontal` if `dx > dy`, else `Vertical`), average gap, and padding; wrap in Auto-Layout Frame | Pink Auto-Layout handles appear immediately on new frame |
| `AutoLayoutSelected` | `KeyDown(Alt+Shift+A)` | Selected Frame has `auto_layout.is_some()` | `AutoLayoutRemoved` | Convert Frame to manual positioning while preserving child world coordinates | Pink handles disappear; standard resize handles remain |
| `ContainerSelected` | `DoubleClick` on canvas | Hit child inside Group/Frame/Component/Instance | `ChildDrilledIn` | Select direct child under cursor inside the current container scope | Selection drills one level deeper per double-click |

- **Audit Status in `design:os studio`:** **IMPLEMENTED** (`shell.rs:575-576`, `ops.rs`, `canvas/interact.rs:334-365, 753-776`).

---

## Layer 3: Motion/Feedback Tokens, Architecture Mapping & QA Contract

### 3.1 Spatial, Timing & Color Tokens (`design:os studio` ↔ Figma 126.9.13)

| Token Name | Value | Usage Across `MI-01..MI-20` |
|---|---|---|
| `token.mi.selection_blue` | `#0D99FF` (`rgb(13, 153, 255)`) | Selection bounds, corner-radius handle strokes, focus rings, `W × H` and `R <val>` badges, layer hover outline (`MI-01`, `MI-06`, `MI-11`, `MI-17`) |
| `token.mi.snap_red` | `#F24822` (`rgb(242, 72, 34)`) | Smart alignment guides, equal-spacing distribution bars, `Alt` distance measurement lines & pills (`MI-13`, `MI-14`) |
| `token.mi.autolayout_pink` | `#EC4899` (`rgb(236, 72, 153)`) | Auto-Layout padding bars, gap struts, `Pad <val>` / `Gap <val>` pills (`MI-09`, `MI-10`) |
| `token.mi.component_purple` | `#9747FF` (`rgb(151, 71, 255)`) | Component /ComponentSet / Instance selection bounds & badges (`MI-20`) |
| `token.mi.scrub_threshold_px` | `2.0px` | Pointer travel threshold separating click-to-focus (`< 2px`) from continuous scrub drag (`>= 2px`) (`MI-03`, `MI-09`) |
| `token.mi.snap_tolerance_px` | `5.0px / zoom` | Screen-space snapping capture radius for edges, centers, and equal gaps (`MI-14`) |
| `token.mi.opacity_buffer_ms` | `400ms` (`0.4s`) | Two-digit rapid opacity keystroke coalescing window (`MI-15`) |
| `token.mi.nudge_small` | `1.0` (`0.1` with `Alt`) | Default and fine numeric/canvas nudge step (`MI-01`, `MI-03`) |
| `token.mi.nudge_big` | `10.0` (`Shift`) | Coarse numeric/canvas nudge step (`MI-01`, `MI-03`) |
| `token.mi.frame_budget_ms` | `< 1.0ms` (`120fps` ready) | Zero heap allocation on idle hover; single coalesced undo snapshot per drag gesture |

### 3.2 `design:os studio` Implementation & Gap Matrix (`MI-01..MI-20`)

| ID | Micro-Interaction | Primary Rust Module(s) | Current Status | Stage 3 Action Required |
|---|---|---|---|---|
| **MI-01** | Numeric Field `ArrowUp`/`ArrowDown` (`±1`/`±10`/`±0.1`) + selection retention | `crates/designos-app/src/widgets/field.rs` | `IN_FLIGHT_STAGE_3` | Complete `ArrowUp`/`ArrowDown` modifier stepping with full text highlight retention |
| **MI-02** | Multi-Operator Infix Math (`Shunting-Yard`) + `Mixed` relative math | `crates/designos-app/src/widgets/field.rs` | `IN_FLIGHT_STAGE_3` | Complete `eval_math_expression` & `eval_mixed_expression` + invalid input rollback |
| **MI-03** | Prefix Scrubbing (`1×`/`10×`/`0.1×`), `Alt+Wheel`, `<2px` Click-to-Focus, `Escape` Cancel | `crates/designos-app/src/widgets/field.rs` | `IN_FLIGHT_STAGE_3` | Complete `Alt` `0.1×` scrub, `Alt+Wheel`, `<2px` click-to-focus, `Escape` revert |
| **MI-04** | Hex Color Field Per-Channel (`RR`/`GG`/`BB`) `ArrowUp`/`ArrowDown` Stepping | `crates/designos-app/src/widgets/picker.rs`, `panels/properties.rs` | `IN_FLIGHT_STAGE_3` | Step `R`, `G`, or `B` pair at caret position (or all 3 when all selected) |
| **MI-05** | Smart Hex Shorthand (`1–8` digits + CSS named colors) | `crates/designos-app/src/widgets/picker.rs` | `IN_FLIGHT_STAGE_3` | Expand `1, 2, 3, 4, 6, 8` hex digits & CSS names on commit |
| **MI-06** | On-Canvas Corner Radius Handles (`Alt` single-corner + `R <val>` badge) | `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `IMPLEMENTED` | Verified (`Interaction::CornerRadius`) |
| **MI-07** | On-Canvas Ellipse Arc Sweep, Donut Ratio & Start-Angle Drag Handles | `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `STAGE_3_GAP` | Wire `hit_shape_control_handle` & `Interaction::ShapeControl` drag in `canvas/interact.rs` + live badge in `canvas/draw.rs` |
| **MI-08** | On-Canvas Polygon & Star Point-Count (`3..60`) & Inner-Ratio Drag Handles | `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `STAGE_3_GAP` | Wire `PolygonPoints`, `StarPoints`, `StarInnerRatio` drag in `canvas/interact.rs` + live badge in `canvas/draw.rs` |
| **MI-09** | Auto-Layout Pink Padding Handles (`Drag`, `Alt` mirror, `Shift` all-4, `Click` inline input) | `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `PARTIAL` | Drag + `Alt`/`Shift` implemented; add `<2px` click-to-inline-edit & inspector hover highlight |
| **MI-10** | Auto-Layout Pink Gap Strut Drag (`item_spacing` + live reflow) | `canvas/handles.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `IMPLEMENTED` | Verified (`AutoLayoutHandle::Gap`) |
| **MI-11** | Resize Modifiers (`Shift` aspect, `Alt` center, `Cmd/Ctrl` ignore constraints) | `canvas/handles.rs`, `canvas/interact.rs`, `ops.rs` | `PARTIAL` | Pass `!inp.mods.command` as `apply_constraints` in `update_resize` (`canvas/interact.rs`) |
| **MI-12** | Draw/Move Modifiers (`Space` pan draw/marquee origin, `Space` suppress reparent) | `canvas/create.rs`, `canvas/interact.rs` | `PARTIAL` | Add `inp.space` origin translation in `update_draw`/`update_marquee` & reparent suppression in `update_drag` |
| **MI-13** | `Alt/Option` Hover Distance Measurement Guides (`#F24822`) | `canvas/snap.rs`, `canvas/interact.rs`, `canvas/draw.rs` | `IMPLEMENTED` | Verified (`compute_alt_measures`) |
| **MI-14** | Smart Alignment Guides & Equal-Spacing Distribution Pills | `canvas/snap.rs`, `canvas/draw.rs` | `IMPLEMENTED` | Verified (`snap_rect`, `measurements`) |
| **MI-15** | Rapid `0–9` Opacity Key Buffer (`400ms` two-digit window, `00` -> `0%`) | `ops.rs`, `shell.rs` | `IMPLEMENTED` | Verified (`apply_opacity_digit`) |
| **MI-16** | Hierarchy & Spatial Navigation (`Enter`, `Shift+Enter`, `Tab`, `N`, `Home`, `End`, `PageUp/Down`) | `canvas/interact.rs`, `shell.rs`, `ops.rs` | `IMPLEMENTED` | Verified (`navigate_hierarchy_enter`, `select_sibling`, `step_frame`) |
| **MI-17** | Bi-Directional Layer ↔ Canvas Hover Highlight | `panels/layers.rs`, `canvas/draw.rs` | `PARTIAL` | Highlight Layer row in `panels/layers.rs:367` when `app.hover == Some(row.id)` |
| **MI-18** | Layer Hover Quick-Actions (`Eye`/`Lock`) & `Tab` Next-Row Inline Rename | `panels/layers.rs`, `shell.rs` | `PARTIAL` | Handle `Key::Tab` / `Shift+Tab` inside `panels/layers.rs:413-436` inline rename to commit and advance to next/prev row |
| **MI-19** | On-Canvas Eyedropper (`I`) Magnifier Loupe (`9×9` pixel grid + `#RRGGBB` pill) | `shell.rs`, `canvas/draw.rs`, `panels/properties.rs` | `STAGE_3_GAP` | Bind `Key::I` in `shell.rs` & render zoomed Magnifier Loupe + `#RRGGBB` pill in `canvas/draw.rs` |
| **MI-20** | Component/Instance & Auto-Layout Micro-Gestures (`Shift+A`, `Alt+Shift+A`, `Alt+Drag`) | `shell.rs`, `ops.rs`, `canvas/interact.rs` | `IMPLEMENTED` | Verified (`remove_auto_layout`, `Alt+Drag` duplicate, double-click drill-in) |
