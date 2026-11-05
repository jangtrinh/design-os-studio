# UI spec part 1 — Project tab shell and reading page

Owner ruling 2026-11-02: "UI thì cứ bế từ bên design studio sang cho khỏi phải quan tâm nhiều." Copy design-studio's layout and behaviour; do not redesign.
Part 2 (editor, dialogs, states, agent co-editing, KPI, narrow windows): [ui-spec-part-2-editor-dialogs-and-states.md](ui-spec-part-2-editor-dialogs-and-states.md). Mockups for both parts: [ui-mockups/](ui-mockups/README.md).

Source: live demo https://itspatmorgan.com/design-studio-demo/ (v0.1.3, MIT), measured at 1440×900 with computed styles on 2026-11-02. Reference captures: `plans/reports/design-studio-doc-ui-evidence/ds-*.jpg`.

## How design-studio values were reconciled with Studio's design principles
Studio's own principles (`design/principles.md` P3 spacing and pointer floor, P4 radii, P5 type) are earlier owner rulings and still bind. **Authority for chrome (ruling R0, [ui-polish-rulings-r1.md](ui-polish-rulings-r1.md)): Studio's shipped widget kit (`crates/designos-app/src/widgets/kit/*.rs`) and `theme.rs` beat this spec wherever they disagree.** Rule used here:
- **Chrome** (tree, tab, "On this page", card headers, menus, dialogs, buttons) uses Studio's existing constants. Where a design-studio value is off Studio's scale it snaps to the nearest Studio role.
- **Document content** (what the Markdown says: titles, headings, paragraphs, tables, lists) keeps design-studio's type scale. P5 yields to document text.

| design-studio value | Why it cannot stay | Studio value |
|---|---|---|
| Tree row 23 px | under `TARGET_MIN` 24 (P3); Studio rows are `CONTROL_XS` | `LAYER_ROW_HEIGHT` 32 |
| Sidebar 219 px | Studio's left panel width | `LEFT_PANEL_WIDTH` 240 |
| Card radius 10 | not a radius role (P4) | `RADIUS_XL` 12 |
| Row radius 8 | — | `RADIUS_CONTROL` 8 (same) |
| Chrome text 13 px (contents list) | not on the type scale (P5) | 12/18 |
| Font Geist | Studio chrome font is Inter (P5) | Inter |

## Layout at 1440 × 900
| Region | x | width | Studio constant / rule |
|---|---|---|---|
| Tab strip (existing, `tabs.rs`) | 0 | 1440 | 32 px bar; Home tab 72 × 24 at x 8, y 4 (`tabs.rs:983`), then a divider; project tabs 240 wide, 26 tall at y 3 (`tabs.rs:1027`), the + tab 24 × 24 at y 4, radius 8, 14 px kind tile (radius 2) with an 11 px glyph, label 12/18, 18 px close plate, a separator between inactive tabs, "+" button 24. The Project tab uses `TILE_SPEC_BG` / `TILE_SPEC_GLYPH` with the book glyph (ruling Q3) and the label "<project> · Project" |
| App rail (existing, `panels/rail.rs`) | 0 | 48 | `RAIL_WIDTH`; menu button, separator, File / Agents / Assets / Tools tiles, separator, Variables / Plugins, help at the bottom. File is the selected tile while the doc tree is open; the rail gets no new item |
| Doc tree | 48 | 240 | `LEFT_PANEL_WIDTH`, bg `BG`, 1 px `BORDER` right |
| Main region | 288 | 1152 | bg `BG` |
| Reading column | 399 | 690 | the 930 px block (690 + 48 gap + 192) is centred in the main region |
| "On this page" | 1137 | 192 | top 48 below the tab strip; hidden under 1200 px window width (part 2 §11) |
| Top padding of reading column | — | 48 | `SPACE_REGION` |

## Doc tree
- **Header** (`LEFT_HEADER_HEIGHT` 65, inset `INSET_PANEL` 16): project name 14/20 600 `TEXT`; second line "N files" 12/18 400 `TEXT_TERTIARY`; a 24 px "…" icon button at right (menu: Link spec folder, Open in Home, Reveal in Finder).
- **Panel title row** (`PANEL_TITLE_ROW_HEIGHT` 40): "Documents" 14/20 600 `TEXT`; right: search and collapse-all icon buttons (`PANEL_ACTION_BUTTON` 24) and "+" (new page menu).
- **Group row** (Overview, Spec, Screens, States, Discovery): height 32, 12/18 500 `TEXT_SECONDARY`, 24 px chevron box left (`LAYER_CHEVRON_BOX`), count pill at right (kit `count_badge`, see Count pill below; never "(3)").
- **Item row**: height 32, inset 8 from the panel edge (`PAGE_ROW_INSET`), radius `RADIUS_CONTROL` 8, 12/18 400 `TEXT_SECONDARY`, indent 16 per depth (`LAYER_INDENT`), 16 px icon + `SPACE_FACT` 4 + label. Icons: page, frame (screen), state, diagram, linked-spec file (page icon with a small link mark).
- **States** (kit Pages row, ruling Q2): rest text `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER`; selected `BG_SELECTED` + `TEXT_SECONDARY_HOVER`, weight 600; keyboard focus: the kit 2 px `FOCUS_RING` band drawn inside the row (no room outside a full-width row); drag in progress: 2 px `TEXT` insertion line between rows.
- **Icon buttons** in the tree header and title row (24 px, radius 8): rest `ICON_QUATERNARY`, hover `BG_HOVER` + `ICON_QUATERNARY_HOVER`, pressed or menu open `BG_SELECTED` + `ICON_SECONDARY_HOVER`.
- **Count pill**: the kit `count_badge` neutral (`widgets/kit/badge.rs`, `count_badge_look(Neutral)`): `BADGE_NEUTRAL_BG` fill, 1 px `BADGE_NEUTRAL_BORDER`, label `BADGE_NEUTRAL_TEXT` in Inter 12/18 500 (`text-xs` Medium), `BADGE_SM` 22 tall, minimum width 22, 6 px side padding, full radius. The agent-chip count pill (part 2 §9) is a different component (16 tall, `BG_TERTIARY`, mono).
- Overview group shows its pages without a chevron (it is never collapsed).

## Reading page (document content)
| Element | Size / line / weight | Colour | Spacing |
|---|---|---|---|
| Page title (H1 or front-matter title) | 30/36 600 | `TEXT` | — |
| Description (front matter `description`) | 16/26 400 | `TEXT_TERTIARY` | 12 above |
| H2 | 20/28 700 | `TEXT` | 32 above (0 for the first block after the description: 32 instead of 48), 16 below |
| H3 | 16/24 600 | `TEXT` | 24 above, 8 below |
| Paragraph | 14/24 400 | `TEXT_SECONDARY` | 16 between blocks |
| Bulleted / numbered list | 14/24 | `TEXT_SECONDARY` | indent 24, 4 between items |
| Link | inherits | `TEXT`, underline | hover: `TEXT_SECONDARY` underline |
| Inline code | 13/24 `Roboto Mono` | `TEXT` on `BG_TERTIARY`, radius 6 | inset 2 6 |
| Code block | 13/20 `Roboto Mono` | `TEXT_SECONDARY` on `BG_SECONDARY` | radius 12, inset 16, horizontal scroll |
| Table | header 12/18 600 `TEXT_SECONDARY` on `BG_SECONDARY`; cells 14/20 400 | 1 px `BORDER` grid, outer radius 12 | cell inset 8 12; column width = widest cell + 24, 120 to 320, longer text wraps (part 2 §4); wider than 690 → horizontal scroll inside the column |
| Callout | 14/24 | bg `BG_SECONDARY`, radius 12, 16 px icon tinted by kind: note `INFO`, tip `SUCCESS`, warning `WARNING`, danger `DANGER` | inset 12 16, icon gap 8 |
| Checklist item | 14/24 | box: Studio's checkbox widget 16 px in a 24 px hit area; checked text stays `TEXT_SECONDARY` (no strike-through) | 4 between items |
| Divider (`---`) | 1 px `BORDER` | — | 24 above and below |

## Embed card (frame, state, diagram)
- Card: full column width, radius `RADIUS_XL` 12, 1 px `BORDER_TRANSLUCENT`, bg `BG`; 24 above and below.
- Header: 36 tall (`CONTROL_SM`), inset 0 8 0 16 (the left 16 matches the body inset), bg `BG_TERTIARY` at 60 %; left: 16 px kind icon + title 12/18 500 `TEXT` + " · " + file name 12/18 400 `TEXT_TERTIARY` as one text run ("Payment · Checkout-web"); right: "Open in canvas" icon button 24 with the frame glyph (the external-link glyph is reserved for links that leave the app) and the tooltip "Open in canvas".
- Body: inset 16; the render is fit to width, max height 420, centred on `BG`.
- States are specified in part 2 §6.

## "On this page"
- Title "On this page" 12/18 600 `TEXT_SECONDARY`, 8 below.
- Items: 12/18 500, inset 4 8 4 12 (row 26 ≥ `TARGET_MIN`), 1 px left rule `BORDER`; H3 items indent 12 more.
- Active item (scroll-spy on the H2/H3 nearest the top): rule and text `TEXT`; others `TEXT_TERTIARY`; hover `TEXT_SECONDARY`. Item text never wraps beyond 192 px (ellipsis). Click scrolls smoothly (200 ms, ease-out); reduced motion → jump.
- Hidden when the page has fewer than 2 headings.

## Colour mapping (both apps use the same neutral scale)
| design-studio (oklch) | hex | Studio token |
|---|---|---|
| 0.145 page bg | #0A0A0A | `BG` |
| 0.205 rail | #171717 | `BG_SECONDARY` |
| 0.269 embed header | #262626 | `BG_TERTIARY` |
| white 10 % borders, selected row | — | `BORDER_TRANSLUCENT`, `BG_SELECTED` |
| 0.985 text | #FAFAFA | `TEXT` |
| 0.87 body | #D4D4D4 | `TEXT_SECONDARY` |
| 0.708 muted | #A3A3A3 | `TEXT_TERTIARY` |

The one accent per view is Studio Orange (`BG_BRAND`) for the primary button only (and a checked checkbox, a state the principles allow). No purple anywhere (P2).

## Not ported
Their global left nav (Prototypes / Systems / Manual), Team mode avatars, the source-code file tree, the Geist font.
