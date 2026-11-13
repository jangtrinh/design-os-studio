# Review of the 29 "True Figma Implementation Specs" (2026-11-12)

All specs stay usable as reference; this review lists what to correct before relying on each one.

Three independent read-only reviews checked every spec against `origin/main` (`b731e4359`), the Kiwi schema copy in the repo,
issue state on GitHub and the three evidence-backed specs of the same day
([nested copy identity](../finalize-e1-e2-e3/spec-nested-copy-identity.md),
[layout drift tail](../finalize-e1-e2-e3/spec-layout-drift-tail.md),
[bound variant follow-ups](../finalize-e1-e2-e3/spec-bound-variant-followups.md)). Full detail:

- [E1, E2, E19, E20](review-e1-e2-e19-e20.md)
- [E4, E5, E6, E7](review-e4-e5-e6-e7.md)
- [E8 to E13](review-e8-to-e13.md)

Every reviewed spec carries a review status banner at its top and a row in the "Review status" table of the
[index](../EPIC-FIGMA-SPECS-INDEX.md). The status words mean:

- READY AFTER FIXES: usable once the corrections in the review are applied.
- NEEDS EVIDENCE: statements about Figma's behaviour lack a checkable source; verify them before relying on them.
- NEEDS OWNER DECISION: contains choices the owner has not made yet (see the decisions below).
- SUPERSEDED: the work is already on main or another spec covers it; useful as background, and a named exception may still be worth taking.
- SPLIT: some slices are worth taking and the rest are not; the review says which.
- NOT REVIEWED: verify symbols and statements about Figma against main and the schema before relying on them.

Three pull requests were merged after the review was written: the per-property variant picks (pull request 1950), the first
auto-layout slice, legacy auto spacing (1946), and nine MCP fixes (1947). The nested copy link fix (issue 1925) is still open.
Sentences that these merges change are updated or marked in the review files.

## Result in one table (22 spec files; READMEs follow their specs)

| Verdict | Count | Specs |
|---|---|---|
| Can be implemented as written | 0 | none |
| Usable after the listed corrections | 9 | E4 shapes and strokes; E6 bounds, export settings, easing; E7 annotations and Code Connect; E8 PPTX; E9 slides; E11 prototype crate and Smart Animate; E12 accessibility; E12 fonts and rich text; E13 Sites |
| Needs Figma evidence first | 3 | E2 layout and grid; E4 draw engine; E5 video, shaders, mesh, 3D |
| Needs an owner decision first | 3 | E9 connectors, mind map, Mermaid; E11 video and Present; E13 code layers |
| Mostly re-specifies shipped work | 5 | E1/E2 master plan; E1 step 0; E19 slots, clip, hydration; E5 image filters; E10 diff and merge |
| Split by slice | 2 | E20 save, load, quit; E6 motion and Lottie |

Counts made by the reviewers for E4 to E13: 236 code references, 140 exact (59%), 53 moved or renamed, 43 not found;
251 statements about Figma, 136 with a checkable source (54%), 90 stated without one, 25 contradicted by the repo or by public docs.
Kiwi field numbers and enum values were the reliable part (130 of 131 checked match).

## Patterns to fix before a second version

1. **Read the base commit.** Every spec names base `e0e44f3c2`, yet many slices ask for work that commit already contains
   (extended collections, Display P3, OKLCH, timing variables, image filters, inside and outside strokes, the motion model,
   the branch diff engine, connectors of 0.7.10). Check `git log` and the CHANGELOG before writing a slice.
2. **A "FACT" needs a Figma source.** Studio's own code, Studio's own bug reports and arithmetic are not Figma evidence.
   Examples: export bounds cite `bounds.rs` (issue 528 says it is off by up to 84 px); the tilted-child rule and the sticky
   formula are inference in their own dossiers; the Gentle spring numbers are wrong.
3. **Symbols that do not exist.** `symbolFingerprint`, `header.rs`, `FileKind`, `probe_zip_kind`, `oxc_transpiler`,
   Kiwi tags `isSlot` / `isSlotContent` / `isExtension` / `styleOverrideTable` in the index.
4. **GPU cost is not free.** The GPU renderer has no stencil; an inside or outside stroke clip opens an offscreen mask Program
   per node. Any spec that promises "zero new Programs" must be re-costed against the Program cap and the Metal limit.
5. **Wrong issue numbers.** The numbers 521, 1328, 1881, 1886, 1891 do not match what the specs say they are.
6. **Standing rulings.** Connector export in design files stays a vector until the owner checks Figma Design (connector spec of 2026-11-04);
   the plugin platform was dropped (code layers that run user code need a decision); new features go to v0.8.0.

## Slices that can go ahead now (bug fixes, with the listed corrections)

These bug fixes belong to milestone v0.7.14.

| Slice | Issue | Condition |
|---|---|---|
| Component property defaults | 1856 | corrected colour-space enum; the per-property variant picks that shared a file with it have landed (pull request 1950) |
| Ring arc repair | 1907 | correct override field (`Arc`); after the nested copy link fix (issue 1925, still open) lands (shared document marker) |
| Quit and save fixes | 1715, 1716, rest of 1648 | must not block where main is deliberately non-blocking; the other four slices come from contributor drafts 1786, 1785, 1783, 1684 |

Held: layout rules for 1885 and 1895 (wait for observations in issue 1942; the first auto-layout slice, legacy auto spacing
for issue 1920, has landed in pull request 1946); compact pages (1686) and lazy reader (1712) need a
design that an older release can still open without losing pages (risk inferred from code, not tested).

## Decisions the owner must make before these specs can be implemented

The ten decisions are listed in the [E8 to E13 review](review-e8-to-e13.md#owner-decisions): connector export form, new node kinds,
Mermaid parser, one video pipeline, font embedding, code execution in code layers, loopback preview server, F6 landmark count,
wording for PPTX and branch review labels, closing the E10 spec as shipped.
