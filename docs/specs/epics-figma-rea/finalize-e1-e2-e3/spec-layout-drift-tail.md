# Spec: auto layout drift tail on the Untitled UI kit (#1920, with #1885 and #1895)

Base `origin/main` at `e0e44f3c2`, census harness with the slot-write patch applied (reproduces **2,039 of 194,397**
moved layers, `evidence/raw/base-real/run.log`). No product code is committed. Figma desktop 126.11.7, engine build
`d947dbc0…`. RE notes: `evidence/re/re-notes-layout-drift-tail.md`.

## 1. What could not be verified

1. **No runtime observation of Figma was made.** Every rule below comes from static artifacts (UI strings, editor JS,
   engine WASM) plus the kit's stored geometry. Nothing was checked by editing a frame in Figma.
2. **The largest fixable bucket has no Figma source.** That bucket is `cross_in_frame`: 473 layers, 23.2%, a hugging
   frame clamped by its own max size below its content. Its rule is UNKNOWN. The kit agrees with one candidate (61 roots, 0 new movers), but that is evidence of what Figma produced, not of the rule. No prototype was made.
3. **Hidden subtrees (596 layers in buckets, 1,141 moved layers sit in an invisible subtree)** are read as "the stored
   geometry is not a layout result". This is INFERRED from engine scope names, not observed.
4. **#1885**: no Figma output for the override case exists in the kit (0 matching overrides). The rule is INFERRED.
5. **#1895**: UNKNOWN. No source and no kit case.
6. **REA could not open the engine.** `rea inspect-wasm-artifact` refuses inputs over 8 MiB. The WABT 1.0.42 tools that REA wraps were used directly instead.
7. **Two WASM predicates are named by inference.** In the single-child detector, function 12037 is taken as "visible and not absolute" and the f32 at child+140 as "primary grow". Both are medium confidence.
8. **S1 for a Fill child is inferred, not seen.** The WASM upgrade detector ignores a lone Fill child. Centring a lone Fill child clamped by its max in legacy rests on the UI string plus one kit frame (`1271:641`, 15 layers). Version 1 placement of that child is unknown.
9. **Mixed buckets.** `raw_al`, `unprobed`, `hidden_take_space` and `fixed_frame` each mix several questions. They are sized but not split further in this run.

## 2. Buckets

Layers are census layers attributed to the top-most frame whose child moved locally, the same way the census does.
When that root frame's own solve is clean, the layers go to the bucket of the child frame that resized itself.
"Probe" means a counterfactual solve in a test-local copy of the solver (`crates/designos-fig/tests/kit_drift_probe/mod.rs`).
A probe only says where to look. It does not set any rule's status.
Sum = 2,039 (`evidence/raw/probes4/bucket-summary.txt`). The share column is of 2,039.

| Bucket (question) | Layers | Share | Figma rule, status, evidence | Studio rule (file:line) | Prototype on the kit | New movers |
|---|---|---|---|---|---|---|
| H. Frame or ancestor hidden: is the stored geometry a layout output? (`HIDDEN_CHAIN:*`) | 596 | 29.2% | **Inferred:** Figma defers building invisible instance subtrees. Evidence: WASM scope names `defer_invisible_children`, `should-defer-invisible-children`, `culled-via-defer-invisible-children`, `force-materialize-invisible-instance-subtrees`, plus the assertion "Invisible descendants should be materialized prior to applying nested overrides" (RE R3). Kit: `6277:292929` is `visible:false`, 92 wide around 129-wide children | Not a solver rule. The census counts these layers | None (census change only, slice 3) | n/a |
| X. Counter alignment in a hugging frame clamped by its own max below its content (`cross_in_frame`) | 473 | 23.2% | **Unknown.** No REA location. Kit candidate: align in the frame's inner box (36 − 2×8 = 20), not in the content's 22 | `layout_solver.rs:236-245`: the line extent is reset to the frame only when the frame is larger than the content | Not prototyped (unknown) | probe: 0 |
| U. Unprobed, mixed (`unprobed`) | 317 | 15.5% | **Unknown.** Measured splits: instance text height 108; instance hidden or visibility override about 108; rotated connectors and avatars 69; `1487:2054` Fill child 303 wide centred 20; frames that only resize with hidden children 12. See §6 items 10-13 | `rest_import.rs` instance materialization (text, visibility); `layout_rotated.rs` | Not prototyped | n/a |
| V. A raw value differs from the bound variable: scaled instance, or an override of a bound field (`raw_al`) | 220 | 10.8% | **Unknown.** Kit: `7460:159056` stores gap −3.2 = −4 × 0.8 (scale tool) and `n125626` stores padding-left 36 against bound 8 | `rest_import.rs:2482-2486` scales spacing; `variables_apply.rs:840` `resolve_node` then re-resolves the binding and drops the scale or override | Not prototyped | probe: 89 frames start moving |
| S1. Legacy auto spacing with a lone child (`sb_single_center`) | 103 | 5.1% | **Seen** for a lone non-Fill child (88 layers, `1266:28122`); **inferred** for a lone Fill child clamped by its max (15 layers, `1271:641`, string + kit; the WASM detector skips children with grow, so it is silent on them); **unknown** for version 1 with a clamped lone Fill child. Legacy (`stackLayoutVersion` 0) centres a lone child under auto spacing. en.json `fullscreen.properties_panel.stack_panel.apr26_shift.space_between_single_child_shift`; JS offset ~3,515,104 (`apr26LayoutShiftsForNode`); WASM export `h9`, func 44479 → 26661 → 27082, byte 3 gated on justify == 3 (`SPACE_EVENLY`) and exactly one layout child | `layout_solver.rs:338` handles k > 1 only, so one child falls to `:344` (start) | **A1 run 1** (centre only a non-Fill lone child, the WASM detector's condition): 2,039 → 1,954 (−85), 1 pair fixed. **A1 run 2** (condition dropped after run 1 left `1271:641` moving): 2,039 → 1,939 (−100), 2 pairs fixed. Run 1's raw folder was overwritten by run 2; its numbers are from the session log | **0** in both |
| Hd. A visible frame whose stored geometry fits "hidden child takes space" (`hidden_take_space`) | 88 | 4.3% | **Inferred (kit probe only):** taking hidden children into layout would move 6,109 clean frames (`evidence/raw/probes4/probe-new-movers.csv`), and Studio already skips them. The 88 layers are instance-data questions (which sublayers are visible, which derived sizes apply), filed with U | `layout_apply.rs:90` skips invisible children (correct) | None | n/a |
| T. A stretched child that is itself a hugging auto-layout frame on that axis (`stretch_hug_wins`) | 86 | 4.2% | **Unknown.** Kit: `1316:591` child `STRETCH` with a hugging height keeps 368 and is centred at 136 in 640 | `layout_solver_types.rs:76-90` turns align-self Stretch into cross Fill | Not prototyped | probe: 0 |
| F. The frame's own hug size differs (`fixed_frame` only) | 67 | 3.3% | **Unknown** (mixed) | `layout_solver.rs:150-250` | Not prototyped | n/a |
| S2. Auto spacing with a Fill child: is item spacing reserved? (`sb_fill_zero_gap`) | 28 | 1.4% | **Inferred:** in auto spacing modes the stored spacing is not used. JS offset 3,520,097 lists the auto modes; JS ~3,523,560: the spacing field reads null and refuses writes in those modes; plus R1.2. Kit: `1377:10738` Fill child 888 wide at x 328, gap 0 | `layout_solver.rs:269` reserves `item_spacing × (k−1)` before Fill | **B:** 2,039 → 2,011 (−28) | **0** |
| C. A Fill (cross) child counts with its own size for the hug (`fill_cross_counts`) | 26 | 1.3% | **Unknown.** This also touches a recorded Studio decision (`layout_solver.rs:209-233`) | `layout_solver.rs:217-233` | Not prototyped | probe: 0 |
| W. Wrap frame with cross slack: where does the block of lines sit? (`wrap_block_align`) | 16 | 0.8% | **Unknown.** Kit: `1334:2212` lines centred (y 33, not 32) | `layout_solver.rs:260` starts lines at the padding | Not prototyped | probe: 0 |
| R. Root is a component set without auto layout | 10 | 0.5% | **Cascade, no rule question:** the moved layers sit under a component set that has no auto layout and move because a descendant frame resized (`evidence/raw/probes4/frames.csv`, rows with `NOT_AUTOLAYOUT`) | n/a | n/a | n/a |
| B. Baseline alignment (`counter_align = Baseline`) | 9 | 0.4% | **Unknown.** UI names: "Align text baseline", "Align left/center/right baseline"; the Sites lint says baseline alignment is unsupported "when a text layer is nested in another frame" | `layout_solver.rs:356-364`: Baseline behaves like Auto | Not prototyped | n/a |
| S3. Legacy auto spacing that overflows: overlap? | 0 | 0% | **Seen (string) + inferred (WASM):** legacy overlaps, version 1 clamps the gap at 0. en.json `…apr26_shift.space_between_negative_gap_clamp`; WASM 27082 byte 4 gated on justify == 3, at least 2 children, and an overflow comparison | `layout_solver.rs:338` clamps at `min(item_spacing, 0)` | **A2:** 2,039 → 2,039 (no kit case) | **0** |

Overlaps between probes (first probe wins in the table, `bucket-summary.txt`): 473 cross_in_frame + fixed_frame;
34 stretch_hug_wins + fill_cross_counts + fixed_frame; 12 fill_cross_counts + fixed_frame; 5 hidden_take_space + fixed_frame.

Combined prototype A1 + A2 + B: **2,039 → 1,911** (−128), 0 newly moving (frame, child) pairs. Visible moved layers go from
898 to 807 (`evidence/raw/proto-combined-a1-a2-b/run.log`).

Coverage against the 80% aim: rules with a seen or inferred status cover S1 + S2 + S3 + H = 727 layers (35.7%); Hd (88) is inferred only as "not a solver question".
X, T, C, W, V, B and F are unknown (897 layers, 44.0%); U is mixed and unknown (317, 15.5%); R is a cascade (10). The 80% aim is not met without a runtime observation of X and V.

## 3. Test vectors from Figma

Each vector is a kit frame whose stored geometry is Figma's output. Positions are parent-relative px.

**S1, legacy auto spacing, lone child (seen)**
1. `1266:28122` "Section": vertical, `SPACE_EVENLY`, version 0, fixed 720 × 960, padding 0. One child, "Container"
   720 × 484 (Fill width, hug height). Figma: child y = **238**, which is (960 − 484) / 2. Studio today: 0.
2. `1271:641` "Container": horizontal, `SPACE_EVENLY`, version 0, fixed 1280 wide, padding left/right 32. One child, a Fill
   width with max 360, 360 wide. Figma: x = **460**, which is 32 + (1216 − 360) / 2. Studio today: 32.
3. A version 1 frame with a lone non-Fill child puts it at the start (en.json string). This is a rule, not a kit vector: the kit has no version 1 frames among the movers. Version 1 with a lone Fill child clamped by its max: unknown.

**S2, auto spacing with a Fill child (inferred)**
4. `1377:10738` "Content": horizontal, `SPACE_EVENLY`, fixed 1216, `stackSpacing` 96 (hidden in this mode). Children:
   "Image" fixed 328, then "Quote wrap" Fill (grow 1). Figma: Quote wrap x = **328**, w = **888** (gap 0). Studio
   today: x 424, w 792.

**S3, legacy overflow (seen as a string, no kit vector)**
5. Rule vector (spec, not observed): a version 0 auto-spacing frame 100 long with two children 80 long sits at 0 and
   20 (gap −60). The same frame in version 1 sits at 0 and 80 (gap 0).

**X, unknown (kit candidates, for a runtime check)**
6. `7879:434781` "Content": horizontal, centre counter, height hug with max 36, padding 8. Children are 20, 22 and 16
   high. Figma y = **8, 7, 10**, centred in 20. Studio y = 9, 8, 11, centred in 22.
7. `3288:570945` "Badge=True, …": horizontal, centre, height hug with max 20, padding 0. Children are 20 and 22 high.
   Figma y = **0, −1**. Studio y = 1, 0.

**W, unknown**
8. `1334:2212` "Contents": horizontal wrap, centre counter, fixed 560 × 274, padding 32, gap 32, counter gap
   non-finite (Auto). Two lines of 88 → block 208 inside 210. Figma line 1 y = **33**, line 2 y = **153**. Studio 32 / 152.

**T, unknown**
9. `1316:591` "Container": horizontal, centre, hug height. Child "Content" `1316:593`: `STRETCH`, a vertical stack with
   hugging height (368). Sibling fixed 640 high. Figma: Content y = **136**, h = **368**. Studio: y 0, h 640.

**B, unknown**
10. `5268:380567`: horizontal, `BASELINE`, Fixed 592, item spacing 80. Text A: 18 / 28 px. Text B: 36 / 44 px, two lines,
    88 high. Figma: A y = **14**, B y = 0. Studio: A y = 0.

**V, unknown**
11. `n164209` "Stars" (inside an instance scaled with the scale tool; factor 0.8 inferred from 12.8 = 16 × 0.8): `itemSpacing` is bound to a variable valued 2 and stored as 1.6.
    Children 12.8 wide. Figma pitch **14.4** (gap 1.6). Studio pitch 14.8 (gap 2).
12. `n125626` "Content" (instance sublayer): `paddingLeft` is bound to a variable valued 8 and stored as 36. Figma child x = **36**.
    Studio x = 8.

**H, inferred (not layout outputs; do not use as solver vectors)**
13. `6277:292929` "Column": `visible:false`. Stored 92 × 774 around 129-wide children with pitch 73 at gap 0.

## 4. Fix slices (one pull request each)

### Slice 1: legacy auto spacing (closes part of #1920)

Files: `crates/designos-doc/src/layout_solver.rs` and its tests module. Nothing else.

Rules:
- In layout version 0, a space-between line with exactly one flow child centres it on the primary axis. Seen for a non-Fill child; inferred for a Fill child clamped by its max (vector 2). Owner or controller call: ship the Fill case now, or wait for §6 item 1b.
- In version 0, k > 1 uses gap = free / (k − 1) unclamped, so children overlap. In version 1 and up the gap is clamped at 0.
- In the auto spacing modes (space-between, around, evenly), Fill distribution reserves no item spacing.

Acceptance tests, written red first:
1. Version 0, vertical, space-between, fixed 720 × 960, one 720 × 484 child → y 238 (vector 1).
2. Version 0, horizontal, space-between, 1280 wide, padding 32/32, one Fill child with max 360 → x 460 (vector 2).
3. Version 1, the same as test 1 (non-Fill child) → y 0. Do not write a version 1 test for a clamped lone Fill child (unknown).
4. Version 0, space-between, frame 100, two 80 children → y 0 and 20. Version 1 → 0 and 80. **Rewrite**
   `space_distribution_modes_keep_overflowing_content_anchored_at_start` (it builds a version 0 frame and asserts the
   version 1 result) so it runs on `AutoLayout::v1()` for space-between. Leave around/evenly as they are.
5. Version 0 and version 1, space-between, fixed 1216, item spacing 96, a 328 fixed child and a Fill child → Fill x 328, w 888.
   The same holds for space-around and space-evenly with a Fill child.
6. Start, center and end packing and wrap frames give the same output as before. The existing tests must pass unchanged.

Kit target: census **1,911** (from 2,039) with **0** newly moving (frame, child) pairs. Prototype patch:
`evidence/patches/proto-combined-a1-a2-b.patch`, measured.
What changes for version 1 frames (Studio creates its frames as version 1 through `AutoLayout::v1()`, 32 call sites):
- Space-between with a negative item spacing: the gap used to be clamped at `min(item_spacing, 0)`. It is now clamped
  at 0, so a negative spacing no longer overlaps children (R1.2 and R2). Red-first test 7: version 1, space-between,
  item spacing −10, frame 100, two 80 children → 0 and 80. Today: 0 and 70.
- Auto spacing with a Fill child reserves no item spacing (R2). Red-first test 5 covers version 1 too.
Both follow seen or inferred rules, so they are intended. Must NOT change: Start, Center and End packing, wrap
frames, grid frames, and the lone-child placement in version 1. Before merging, grep for production code that builds
`AutoLayout { .. Default::default() }` (version 0): those frames would now get the legacy rules.

### Slice 2: wrap counter gap stays "Auto" (#1885)

Files: `crates/designos-doc/src/node.rs` (AutoLayout gets an "auto counter spacing" state), `crates/designos-fig/src/fig.rs:3324-3326`
and `rest_import.rs:4445` (keep Auto on import instead of copying the item spacing), `rest_import.rs:3477-3482`
(override), `layout_solver.rs` (resolve Auto to `item_spacing` when solving), plus the `.fig` export round trip.

Rule (inferred, RE R4): a missing or non-finite `stackCounterSpacing` on a wrap stack means "follow the item spacing".
It is resolved when the frame lays out, so an override of item spacing alone also moves the line gap.

Acceptance tests, red first:
1. Import a wrap frame with `stackSpacing` 32 and a non-finite `stackCounterSpacing`. Override `itemSpacing` to 12 on an instance. The second line starts at the first line's height + 12. Today it is + 32.
2. The same with the field absent.
3. An explicit counter spacing of 0 stays 0 under the same override.
4. Export the auto state to `.fig` and read it back: the result is still auto.

Kit target: census unchanged at the slice's base number (no kit case). Must NOT change wrap frames that store a number (475 in the kit).
Status: no prototype. Run one Figma observation of the override (§6) before building it, or accept the inferred rule.

### Slice 3 (proposal for the owner): census also reports moved layers outside invisible subtrees

Files: `crates/designos-fig/tests/kit_layout_drift_measure.rs` and `kit_auto_layout_census.rs` only. No product code.

Change: print the moved layers inside an invisible subtree separately (`moved_in_hidden_subtree`, today 1,141) and
report `moved_visible` (today **898**; 807 after slice 1) as the headline. The local harness already has the code
(`evidence/scripts/patch-harness-hidden-metric.py`).

Acceptance:
1. The base run prints `moved=2039 moved_in_hidden_subtree=1141 moved_visible=898`.
2. After slice 1 it prints `moved_visible=807`.

Must NOT change: the existing `moved=` number and the CSV formats.
Sequencing: `kit_layout_drift_measure.rs` is being changed by another worker for #1921; land this after #1921. Using
`moved_visible` as #1920's headline redefines how the issue is measured, which is the owner's decision: offer it as an
extra line until the owner agrees.

Order: slice 1 first; slice 3 after #1921 lands and the owner agrees; slice 2 after its §6 observation or an explicit acceptance of the inferred rule. Buckets X, T, W, B, V, C and F stay closed until §6 gives a source.

## 5. #1885 and #1895

- **#1885:** rule INFERRED (RE R4). The kit has 477 wrap stacks with a non-finite counter spacing, 8 without one and 475
  with a number. It has no instance override of item spacing on an Auto wrap stack, so there is no Figma vector, and
  #1885 covers 0 census layers. Studio's cause: the importer turns Auto into a number at `rest_import.rs:4445`
  (`fig.rs:3324` forwards only finite values), and the override at `rest_import.rs:3477-3482` then changes `item_spacing` alone.
- **#1895:** UNKNOWN. The only artifact is the Sites lint "A child layer set to Fill can't be rotated between 45 and 135
  degrees", which shows the state exists in design files. It covers 0 census layers; the residual rotated movers
  (about 69 layers inside U) are quarter turns and connectors, not tilted Fill. Studio today, in `layout_rotated.rs`: a
  tilted child's Fill, min and max act as Fixed, and its slot follows its bounding box.

## 6. Unknown buckets: what a runtime observation would need

Each item needs one scratch Figma file. The owner must agree to it first (AskUserQuestion, per [[figma-re-entry-points]]).
Each step reads one number in Figma's layer panel.

1. **X (473 layers):** a version 0 horizontal frame, hug height, max height 36, padding 8, centre aligned, with children
   20, 22 and 16 high. Read the children's y: the kit predicts 8, 7, 10. Repeat with Start and End alignment, then with version 1.
2. **V (220 layers):** (a) a component with spacing bound to a variable valued 2; scale an instance by 0.8 with the scale tool and read
   its gap (predicted 1.6). (b) Override a bound padding on an instance with a raw 36, then read whether the variable
   chip stays and which value applies.
3. **T (86 layers):** a horizontal hug frame with a vertical auto-layout child that hugs its height and is set to stretch
   (as stored in `1316:593`). Read its height and y.
4. **W (16 layers):** a wrap frame, fixed height with slack, centre counter alignment. Read where line 1 starts.
5. **C (26 layers) and F (67 layers):** split further first (each mixes questions). Then the same kind of check.
6. **B (9 layers):** vector 10. Read the baseline offset rule: first line or last, and whether it rounds.
7. **#1885:** an instance of a wrap frame with Auto counter gap; override item spacing. Read the line pitch.
8. **#1895:** a frame with a child rotated 30 degrees and set to Fill, plus min and max. Read its width, height and box.
9. **H:** un-hide `6277:292929` in a copy and check that Figma lays it out again. That confirms the stored data was stale.
10. **S1 Fill case (1b):** a version 0 auto-spacing frame with one Fill child clamped by a max: read x (kit predicts
    centred). Repeat in version 1.
11. **U, instance text height (108):** an instance whose text override changes the paragraph: read the text height and the
    next sibling's y (`n153246`: Studio has the text 344 high, Figma's sibling sits at 112).
12. **U and Hd, instance visibility (about 196):** an instance of `_Nav item dropdown base` style rows: read which
    sublayers Figma shows (`n125626`: Figma leaves 36 px before the first visible child).
13. **U, rotated connectors and avatars (69):** a vertical, centred column with a child rotated by a quarter turn: read the
    child's box (`1254:127614`, `n121962`).
14. **U, `1487:2054` (20):** read the third `_Team member` instance's width and x in a 343-wide column.

## 7. Epistemic ledger

- FACT: the base run reproduces 2,039 / 194,397 in all 11 unmodified runs (`evidence/raw/*/run.log`).
- FACT: the six `apr26_shift` strings and the JS toggle that writes `stackLayoutVersion` exist (RE R1).
- FACT: WASM func 27082 sets the single-child byte only under justify == 3 and exactly one child passing 12037 (WAT read).
- INFERENCE: 12037 means visible and not absolute; the gated f32 is primary grow (medium).
- FACT: prototype A1 −100, B −28, A2 0, combined −128; every run has 0 newly moving pairs (`pair-diff.txt`).
- FACT: A2 and the combined patch fail exactly one designos-doc test, the one that encodes version 1 overflow on a
  version 0 frame. The combined run's designos-fig failure `perf_5000_vars_4_modes_within_budget` is a timing test: it
  passed 3 of 3 reruns on base (`perf-test-rerun-on-base.txt`) and passed in the A1, A2 and B runs.
- FACT: taking hidden children into layout would make 6,109 clean frames move.
- INFERENCE: hidden-subtree geometry is not a layout result (R3).
- FACT: all 691 auto-layout root frames of the census are layout version 0 (`probes4/frames.csv`; 3 rows are component sets without auto layout).
- Surprise recorded: A1 run 1 used the WASM detector's "non-Fill" condition and left `1271:641` moving (1,954); the condition was then dropped (1,939). That choice rests on one kit frame, so the Fill case is labelled inferred, not seen.
- Surprise recorded: the first bucketing put 354 layers in S1; the real-solver prototype fixed 100. The probe bucketing
  had credited frames whose own solve was already clean. Bucketing was re-modelled (`bucket-summary2.py`: a clean root
  takes the bucket of its resized child), which gives 103 for S1, in line with the prototype.
