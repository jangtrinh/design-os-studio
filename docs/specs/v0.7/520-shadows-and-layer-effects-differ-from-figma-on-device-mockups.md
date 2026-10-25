# #520 Drop shadows and layer effects differ from Figma on shadowed device mockups

| Field | Value |
|---|---|
| Issue | #520 (bug; symptom bucket from the kit audit of 2026-10-18: 53 frames on 11 pages) |
| Size | M (diagnosis is S; the fix is S once the live read names the rule) |
| Depends on | the owner-manual live Figma read in section 10 (gates the fix, not the tests' structure); #161 and #37 touch the same render functions (section 12) |
| Unblocks | the "match by eye" acceptance of the 8 listed frames |
| Touches | `crates/designos-render/src/lib.rs` (`draw_silhouette` `:513-591`), `crates/designos-render/src/effects.rs` (`drop_shadow_with` `:107-136`), `crates/designos-gpu/src/compile.rs` (`silhouette` `:1134-1189`, `drop_shadow` `:1252-1294`), tests in `designos-render/tests/render.rs` and `designos-gpu/tests/fidelity.rs`. No hot file. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (re-implementation of the shadow shape on a synthetic frame pair; the plugin typings 1.142.0 description of the "show shadow behind node" flag). Figma ground truth was not available: the plugin was not connected and the kit is not on the research machine.

## 1. Summary and user story

As a designer opening a kit with device mockups, I want a frame's drop shadow to look as Figma draws it, so that a screen inside a shadowed device frame has no faint ring, missing shadow or stray edge.

## 2. Current state (FACT)

- The issue's line references (`lib.rs:507-587`, `compile.rs:998-1050`) are from 0.3.0 and have moved: the CPU silhouette is `draw_silhouette` at `crates/designos-render/src/lib.rs:513-591` and the GPU one is `silhouette` at `crates/designos-gpu/src/compile.rs:1134-1189`.
- CPU silhouette rules (`lib.rs:541-590`): when the node has a visible fill, a visible stroke, or the call is the opaque one on a fillable shape, only the node's own shape is drawn and children are ignored (`:542`, `return` at `:583`). Only when none of that holds does it draw the children as the silhouette (`:586-590`).
- The knockout used when "show behind" is off draws the silhouette with `opaque = true`: `sil(&mut knock, ..., 0.0, true)` (`effects.rs:129-132`, the analytic path `shadow.rs:279-282`, the GPU `compile.rs:1291`). On a fillable shape with no fills, the opaque branch paints the whole frame rectangle (`lib.rs:543-546`; GPU `compile.rs:1150-1151`).
- Consequence (FACT by reading, not yet observed on a render of the kit): for a frame with no fill, the shadow itself comes from the children (`lib.rs:586`), but the knockout erases the whole frame rectangle, so the shadow inside the frame is erased and only the part outside the frame remains.
- For a frame with a visible fill the shadow comes from the frame's own shape and the children are ignored (`lib.rs:542-583`).
- A solid single-fill rounded rectangle without strokes uses the analytic path (`lib.rs:389-410`, `shadow.rs:195-287`, GPU `drop_shadow_analytic` `compile.rs:1196-1247`); it is not affected by the fill-less rule.
- Studio's blur kernel is a Gaussian with standard deviation radius / 2 (`effects.rs:121`; GPU `compile.rs:1270`). FACT (REA 2026-10-19, Figma fullscreen wasm shader source): Figma's shadow and blur shaders use a tent filter. The difference on a synthetic edge is at most 0.036 of coverage, about 4/255 at alpha 0.5, so it is minor and not the main cause (INFERENCE: measured on a synthetic frame, not on the kit).
- INFERENCE (REA 2026-10-19, plugin typings 1.142.0): the typings describe `showShadowBehindNode` as knocking the shadow out of the node's geometry (its fills and strokes). A frame without a fill has no geometry, which supports "no knockout at all" or "knockout of the children only", and contradicts "knockout of the whole frame rectangle".
- INFERENCE: this hypothesis explains "faint edges and shadow shapes" on device mockups, but "attribution is weakest on these frames, so the bucket may mix causes" (issue). The bucket may also hold #37 (progressive blur), #38 (glass, noise) and #161 (blend) effects.
- UNKNOWN: Figma's actual rule for a fill-less frame (no knockout, or knockout of the children). Closing check: the owner-manual live Figma read in section 10.

## 3. Behaviour rules

1. **Diagnosis first.** No renderer change is merged before the live read of section 10 names the rule. The PR body records the read (screenshot of both Figma exports and both Studio renders) and the kernel difference paragraph (the 0.036 coverage figure above).
2. **Candidate rule F1 ("no knockout")**: a fill-less, stroke-less frame with "show behind" off has no knockout, because its geometry is empty. Children keep painting over the shadow.
3. **Candidate rule F2 ("children knockout")**: the knockout of a fill-less, stroke-less frame is the union of its children's own silhouettes (opaque), not the frame rectangle.
4. The read decides between F1 and F2 (section 10 decision table). The unchosen candidate is not implemented.
5. A frame with a visible fill or stroke is unchanged: its own shape, children ignored.
6. CPU and GPU implement the same rule; `silhouette(.., opaque = true)` keeps the meaning "ignore fill alpha" for nodes that have a shape and a fill.
7. Text nodes and nodes that are not fillable are unchanged.

## 4. Data model / schema changes

None.

## 5. Contracts

- `Sil` (`effects.rs:14`) keeps its signature `(pixmap, dx, dy, grow, opaque)`; the fix lives inside the silhouette functions.
- CPU and GPU keep producing the same pixels within the existing tolerance of `designos-gpu/tests/fidelity.rs` (strict mismatch under 2% at tolerance 8).
- `.fig` and REST round trip are untouched (`showShadowBehindNode` is read and written already: `rest_import.rs:118`, `fig_export.rs:1896`).

## 6. File-format mapping

None.

## 7. UI

No UI change. Evidence in the PR is renders only: the two fixture frames (section 10) before and after on CPU and GPU, and the 8 listed kit frames if the kit is available, desktop 1440 or the frame's own size. Responsive: SKIPPED (owner default). No es-designer step (no UI file changes).

## 8. Edge cases

- A fill-less frame that clips content: children are clipped by the frame in the silhouette as they are in the render.
- A fill-less frame whose children are hidden or fully transparent: no shadow (the silhouette is empty).
- A fill-less frame with a visible stroke: the stroke branch applies (`lib.rs:542`), unchanged.
- A group (not a frame) with a shadow: same fill-less rule; the group has no geometry.
- A child with its own shadow inside a fill-less parent with a shadow: the nested renderer draws the child's effects into the silhouette (`lib.rs:589`); keep as is and note it in the PR if the read shows Figma differs.
- A translucent child: under F2 the shadow is knocked out beneath it, under F1 it shows through. This is the pixel that tells F1 from F2.
- A frame with a fill whose alpha is 0: its silhouette has alpha 0 (`Paint::silhouette_alpha`, `designos-doc/src/paint.rs:228-234`), so no shadow; unchanged.
- Show behind ON: no knockout in any case; unchanged.
- Zoom and tiles: the knockout is built per region, so a tile boundary must not change the result (guard test with a tile render).

## 9. Non-goals

- Replacing the Gaussian with the tent kernel (see the open question; #37 decides the kernel for blur work).
- Progressive blur, blend modes on shadows, glass and noise (#37, #161, #38).
- Fixing the other possible causes in the 53-frame bucket before they are named.
- The kit frame ids of the issue as a pass/fail suite (the by-eye acceptance stays a human check).

## 10. Test plan (red first)

Fixture pair (two frames written by the test, parameters fixed here so the Figma read and the Studio test use the same pair): frame size 240 x 160; drop shadow colour black at 50%, offset (0, 12), blur 24, spread 0, show behind off; child A an opaque rectangle 120 x 60 at (60, 50); child B a rectangle 40 x 30 at (20, 110) with a 50% opaque fill. Frame 1 has a white fill. Frame 2 has no fill and no stroke.

Probes (device pixels at scale 1): P1 inside the frame, below child A and clear of child B, where child A's shadow falls; P2 beneath the centre of child B; P3 outside the frame, below its bottom edge.

| Case | Expected | Fails before |
|---|---|---|
| `fillless_frame_shadow_is_not_erased_inside_the_frame` | P1 holds shadow (alpha above 0.05) on frame 2 | yes (P1 is erased by the frame-rectangle knockout) |
| `filled_frame_shadow_comes_from_its_own_shape` | frame 1: P1 and P3 follow the frame's shape; child A's shadow does not appear | guard |
| F1 `fillless_frame_translucent_child_shows_the_shadow_through` | P2 holds shadow | yes |
| F2 `fillless_frame_translucent_child_knocks_out_its_shadow` | P2 holds no shadow | yes |
| `cpu_and_gpu_match_on_the_fixture_pair` | CPU = GPU within the fidelity tolerance | yes if the GPU still erases the frame rectangle |
| `show_behind_on_never_knocks_out` | shadow visible under the children | guard |
| `tile_render_equals_full_render_for_the_fixture_pair` | identical | guard |
| existing shadow tests in `designos-render/tests/render.rs`, `designos-render/src/shadow_tests.rs`, `designos-gpu/tests/fidelity.rs` | green | guard |

Only the F1 or the F2 row is added, as the read decides. Run: `cargo test -p designos-render`, `cargo test -p designos-gpu --test fidelity`. Evidence: red/green capture, the four renders (CPU and GPU, frames 1 and 2), the Figma exports, and the kit frames by eye if the kit is available.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** Build the two fixture frames in a scratch Figma file, export both as PNG at 1x, and read the pixels at P1, P2 and P3 (or send the two exports). Decision table:

| Observed in Figma (frame 2) | Rule |
|---|---|
| P1 has no shadow | the knockout of the whole frame rectangle is right (this spec's hypothesis is wrong; stop and re-diagnose the bucket) |
| P1 has shadow and P2 has shadow | F1 |
| P1 has shadow and P2 has none | F2 |

## 11. Performance budgets

- F1 removes work (one fewer silhouette program on a fill-less frame). F2 draws the children's silhouette once, as the shadow already does.
- GPU: no new offscreen program beyond today's two silhouettes per shadow (`compile.rs:1279`); the program cap of 64 and the VRAM budget of CLAUDE.md section 4 are unchanged.
- No idle cost; no `app.changed()`.

## 12. Dependencies and build order

No hard task dependency, but the fix is gated by the owner-manual read. #161 changes the composite step of the same functions (`effects.rs:135`, `compile.rs:1293`) and #37 adds blur variants in `effects.rs` and `compile.rs`. Recommended order in the render files: #37, then #161, then this fix, so the knockout change rebases onto the final composite code (README-part2). No hot-file position.

## 13. Owner rulings

- Acceptance (issue): Figma's rule for shadow silhouettes is confirmed against Figma exports and documented in the PR; the listed mockup frames match Figma's shadows by eye on CPU and GPU.
- Delivery law 6: a hole is first assumed to be a decision; the code reads "children ignored when the node has a fill" on purpose, and this task changes only the fill-less case after evidence.

## Open questions for the owner (recommended first)

1. Run the live Figma read (two fixture frames, three pixels). Recommended: yes, about 10 minutes; nothing else in this task can be decided without it.
2. Should uniform blur and shadows move from the Gaussian to Figma's tent kernel? Recommended: no for this task (difference at most 4/255); decide after #37's calibration shows the blur radius mapping.
3. If the read shows Studio's shadows differ in more ways than the fill-less case, split the rest of the bucket by cause into new tasks. Recommended: yes.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: the shadow knockout is a mask pass. In binary mode (flag `knockout_shadow_binary_mask`), any non-zero mask alpha removes the shadow at that pixel. The non-binary mode blends by mask alpha.
- FACT: `showShadowBehindNode` is effect field 8 and defaults to false.
- UNKNOWN: what draws the mask for a frame without fills: the frame geometry, the fill alpha or the children. The two-frame live read stays the deciding step.
