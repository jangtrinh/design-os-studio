# UI spec — Project tab, ported from design-studio

Owner ruling 2026-11-02: "UI thì cứ bế từ bên design studio sang cho khỏi phải quan tâm nhiều." Copy their layout and behaviour; do not redesign.

Source: live demo https://itspatmorgan.com/design-studio-demo/ (v0.1.3, MIT), measured at 1440×900 with computed styles on 2026-11-02. Reference captures: `plans/reports/design-studio-doc-ui-evidence/ds-*.jpg`. Every UI PR attaches a 1440 capture of the same screen next to the matching `ds-*.jpg`.

## Layout (1440 wide)
| Region | x | width | Notes |
|---|---|---|---|
| App rail (existing Studio rail) | 0 | 48 | theirs: bg #171717, 1 px right border white/10 %, padding 12 px 0 |
| Doc tree sidebar | 48 | 219 | padding 12 px 8 px; header block 52 px tall, padding 0 10 px 0 12 px |
| Reading column | 389 | 690 | starts 48 px from top; centred in the remaining space left of the contents column |
| "On this page" column | 1127 | 192 | top 48 px; hidden below ~1200 px window width |

## Sidebar (doc tree)
- Header: project name 14 px / 600 / line 17.5; second line "Files · N" style meta 12 px tertiary; "…" menu at right.
- Section label ("Artifacts" → ours "Documents"): 12 px / 600, with search and collapse-all icons at right.
- Group label (Spec, Screens, States, Discovery): 12 px / 500, chevron left, indent 0.
- Row: height 23 px, 12 px / 400, line 15, text TEXT at 80 % alpha, radius 8, padding 4 px 6 px 4 px (24 + 16 × depth) px, icon 14 px before label (doc / frame / state / diagram).
- Selected row: bg white 10 % (`BG_TRANSPARENT_HOVER`), text TEXT, weight 500. Hover: same bg.
- Rows are drag-reorderable within a group; F2 renames; context menu: Open, Rename, Move up/down, Delete (to Trash), Edit source.

## Reading page
- Title H1: 30 px / 600 / line 36, TEXT.
- Description line under title: 16 px / 400 / line 26, TEXT_TERTIARY, margin-top 12.
- H2: 20 px / 700 / line 28, margins 32 top / 16 bottom (first H2: 0 top after intro).
- H3: 16 px / 600 / line 24, margins 24 / 8.
- Body: 14 px / 400 / line 24, TEXT_SECONDARY; lists indent 22 px, item gap 4 px.
- Links: TEXT, underline.
- Tables: header row BG_TERTIARY, 1 px BORDER_TRANSLUCENT grid, 13 px; wide tables scroll horizontally inside the column (owner rule: tables scroll horizontally).
- Callout: bg BG_SECONDARY, radius 10, padding 12 16, icon left.
- Checklist: 14 px box, radius 4, checked = TEXT fill.

## Embed card (frame, state, diagram)
- Card: radius 10, 1 px border white 10 % (`BORDER_TRANSLUCENT`), bg BG, full column width (690).
- Header: 36 px, padding 0 12, bg BG_TERTIARY at 60 %, 12 px title "<frame> · <file>" left, open-in-canvas icon right.
- Body: live render, fit-width, max height 420 px, centred; loading state "Rendering…" in TEXT_TERTIARY (theirs: "Opening canvas"); broken reference: "Frame not found — Relink" card, never blank.

## "On this page"
- Title 12 px / 600 / line 16.
- Items 13 px / 500 / line 19.5, padding 4 8 4 12, 1 px left rule BORDER_TRANSLUCENT; active item rule and text TEXT, others TEXT_TERTIARY. Indent H3 by 12 px. Scroll-spy.

## Colour mapping (both apps use the same neutral scale)
| design-studio (oklch) | hex | Studio token |
|---|---|---|
| 0.145 page bg | #0A0A0A | `BG` |
| 0.205 rail/sidebar | #171717 | `BG_SECONDARY` |
| 0.269 embed header | #262626 | `BG_TERTIARY` |
| white / 10 % borders, selected row | — | `BORDER_TRANSLUCENT`, `BG_TRANSPARENT_HOVER` |
| 0.985 text | #FAFAFA | `TEXT` |
| 0.87 body | #D4D4D4 | `TEXT_SECONDARY` |
| 0.708 muted | #A3A3A3 | `TEXT_TERTIARY` |

Font: design-studio uses Geist; Studio keeps Inter (bundled app chrome font). Sizes and weights above apply as is.

## Not ported
Their global left nav (Prototypes / Systems / Manual), Team mode avatars, the source-code file tree, and any purple accent.
