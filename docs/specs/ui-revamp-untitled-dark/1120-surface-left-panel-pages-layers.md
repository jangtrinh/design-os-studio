# E Left panel: file header, Pages list and Layers tree in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1120 (epic #1118, spec key E) |
| Size | S/M. About one and a half days: 3 owned files, 2 chrome colour sites, row states moved onto C's tree-row anatomy, the R5 row and type sizes (Pages rows 24 → 32). |
| Depends on | A (colour roles and `theme.rs` colour constants), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). Also #1108 (Pages/Layers separator, in flight), see section 12. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/left.rs` (`:12-190` panel, `:192-194` section title, `:196-307` pages list, `:309-331` page drag, `:333-360` page menu, `:362-458` file header); `crates/designos-app/src/panels/layers.rs` (`:239-245` type-label colour, `:317-656` rows, drop indicator and scrollbar call); `crates/designos-app/src/find/filter.rs` (`:50-62` match highlight, `:64-105` layer filter field). Reads only: the left-panel constants in `theme.rs` (`:279-303`, `:305-316`), owned by A and B. |

Code baseline: HEAD `ec47da26` (the local `main` this spec was written on). `origin/main` is `35a65796`, four commits ahead (release 0.6.1): it changes `panels/layers.rs` (152 changed lines); `panels/left.rs` and `find/filter.rs` are unchanged (`git diff --stat ec47da26 35a65796 -- <files>`). The colour-site counts in section 2 are identical at `35a65796`. Re-anchor every cited line against the HEAD you branch from and report drift before writing code.

Colour names follow the plan's naming contract: Untitled token name plus the dark-mode hex, resolved this session from the kit export. Brand roles carry the R4 remap. Provisional names wait for spec A's token contract, which wins on conflict. Names Untitled does not have are marked "new name".

## 1. Summary and user story

As a designer, I read and work the left panel (file name, Pages, Layers) in Untitled UI dark: neutral row fills, Untitled type at the small end of the scale, C's tree-row states and focus ring, Studio orange only for live state, and component layers still in cyan. The panel keeps its width (240), its order (header, Pages, optional Dev section, Layers) and every control where it is (R1). Page rows grow to the R5 row height.

## 2. Current state (FACT unless labelled)

- **Panel frame** (`left.rs:12-16`): fill `theme::BG`, right hairline `BORDER`. The Agents/Tools branch (`:17-41`) hands the panel to the agent terminal (spec I) and the Assets branch (`:43-46`) to Assets (spec K); neither is restyled here.
- **File header** (`left.rs:52-61`, `:362-458`): 65 px (`LEFT_HEADER_HEIGHT`, `theme.rs:280`); "Project / File name" in `font_body` + `font_body_strong` with a chevron, rename field in `BG_SECONDARY` (`:393`), hover pill `BG_HOVER` (`:409`), sublabel row (`:426-454`) in `TEXT_SECONDARY`, link-style action in `TEXT_BRAND` (`:442`); hide-sidebar button 24 (`:53-59`).
- **Pages** (`left.rs:63-89`, `:196-307`): title row 40 (`PANEL_TITLE_ROW_HEIGHT`) with "Pages" in `font_body_strong` 11 (`:192-194`), search and `+` buttons 24 (`:66-80`); rows 24 (`PAGE_ROW_HEIGHT` = `CONTROL_DENSE`, `theme.rs:290`), at most 5 visible (`theme.rs:293`, `left.rs:83-86`), pill inset 8; selected page `BG_SECONDARY` (`:272-273`), hover `BG_HOVER` (`:274-275`); divider pages as a hairline (`:265`); group-label pages in `font_caption_strong` `TEXT_TERTIARY` (`:279-282`); page names `font_body`/`font_body_strong` `TEXT` (`:283-286`); a Ready-for-dev icon in literal `#1BC47D` (`:288-291`); inline rename field `BG_SECONDARY` (`:229-236`); drop line `DROP_INDICATOR` 2 px (`:309-321`); page menu via `popup_menu` (`:333-360`).
- **Dev section** (`left.rs:91-142`, Dev mode only): 52 px block, literal `#1BC47D` (`:97`) for icon, count and a 24 px "Mark ready" button tinted 16 %/24 % (`:116-132`). Its colour role is decided in spec G.
- **Find takes the place of Layers** (`left.rs:145-148`): `find::panel` is spec J's.
- **Layers header** (`left.rs:150-187`): 40 px (`LAYER_HEADER_HEIGHT`), "Layers" title (or Dev tabs, `dev::list::tabs`, spec G), search and collapse-all buttons 24, filter row 32 (`:182-187`, `find/filter.rs:64-105`: search icon `ICON_SECONDARY`, `TextEdit` in `TEXT` with hint `TEXT_TERTIARY`, clear button).
- **Layer rows** (`layers.rs:317-656`): rows 32 (`LAYER_ROW_HEIGHT` = `CONTROL_TOOL`, `theme.rs:296`), indent 16, chevron box 24, icon 16, toggles 24 (`theme.rs:296-303`). Row states share one pill at the frame inset (`:400-414`): selected `BG_SELECTED`, selected + hover or canvas hover `BG_SELECTED_HOVER`, child of a selected layer `BG_SELECTED_SECONDARY`, canvas hover a 1 px `BORDER_SELECTED` outline (Studio orange), pointer hover a 1 px `BORDER` outline. Chevron colour `ICON_SELECTED`/`ICON`/`ICON_TERTIARY` (`:421-428`). Kind icon `TEXT_COMPONENT` (cyan `#22D3EE`) for component-like layers, else `ICON_SELECTED`/`ICON_TERTIARY` (`:465-472`). Name colour: hidden `TEXT_TERTIARY`, component `TEXT_COMPONENT`, else `TEXT` (`:477-483`); type label `font_caption` in `TEXT_COMPONENT` at 75 % or `TEXT_TERTIARY` (`:239-245`, `:549-557`). Locked rows show only the lock icon; there is no locked name colour (`:577-589`). Inline rename: 24 px field, `BG` fill, `BORDER_SELECTED` outline, radius `RADIUS_MARK` (`:484-520`). Eye and lock toggles 14 px icons, `ICON_SECONDARY` → `ICON` on hover (`:561-589`). Multi-select is shift/cmd click (`:610-627`) and paints the same selected pill on every selected row. Drop indicator: inside = 2 px `DROP_INDICATOR` outline, before/after = 2 px line (`:637-654`). Filter matches: `find/filter.rs:50-62` paints `FIND_HIGHLIGHT` (`#0D99FF` at 35 %) behind matched characters.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/left.rs` | 77 | 2 | 97, 289 | none |
| `panels/layers.rs` | 51 | 0 | | none |
| `find/filter.rs` | 6 | 0 | | none |
| **Total** | 134 | **2** | | 0 |

Command (run from the repo root; writes nothing):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - panels/left.rs panels/layers.rs find/filter.rs <<'PY'
import re, sys
sys.path.insert(0, "scripts")
from rust_literal_scan import clean
HEX = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
                 r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)|Color32::from_(?:black|white)_alpha\("
                 r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b")
CONV = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\((?!\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,)")
for rel in sys.argv[1:]:
    src = open("crates/designos-app/src/" + rel, encoding="utf-8").read()
    code = clean(src).split("\n")
    hexs = [i + 1 for i, l in enumerate(code) for _ in HEX.finditer(l)]
    conv = [i + 1 for i, l in enumerate(code) for _ in CONV.finditer(l)]
    print(rel, "theme_lines=%d" % sum("theme::" in l for l in code), "chrome_sites=%d" % len(hexs), sorted(set(hexs)), "computed=%s" % sorted(set(conv)))
PY
```

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`) gives `left.rs` 2, `layers.rs` 0, `filter.rs` 0. With tests counted, `layers.rs` has 68 `theme::` lines (51 outside tests).

Gates today: no colour-literal gate (spec D section 5 proposes one, owned by A). `scripts/check_spacing_relationships.py` ratchets geometry and font literals; `left.rs` and `layers.rs` have entries in `scripts/spacing_baseline.json`. Theme tests pin `CONTROL_DENSE` 24 and the control family (`theme.rs:843-848`) and that selection fills are neutral (`:900-906`).

## 3. Behaviour rules

1. **Colours from roles only.** The 2 chrome sites go (`left.rs:97`, `:289`); both are the Dev green and take spec G's Dev role.
2. **Role map** (provisional names until A lands; A wins):

| Element | State | Role (Untitled dark) | Today |
|---|---|---|---|
| Panel fill | default | `bg-primary` `#0A0A0A` (A's role for `BG`) | `BG` `#121215` |
| Panel edge, section hairlines (`left.rs:15`, `:61`, `:89`, `:141`) | default | `border-secondary` `#262626` | `BORDER` |
| Section titles "Pages", "Layers" | default | `text-primary` `#FAFAFA`, `text-sm` 14/20 semibold | `TEXT`, 11 strong |
| File name / project prefix / sublabel | default, hover | name `text-primary`; prefix and sublabel `text-tertiary` `#A3A3A3` → `text-secondary` on hover | `TEXT`, `TEXT_SECONDARY` |
| Sublabel link action | default, hover | `text-brand-secondary` per R4 (A decides; provisional `fg-brand-primary` `#F97316`) | `TEXT_BRAND` `#FF9E66` |
| Page and layer row | hover | C `_Tree view item` Hover (provisional `bg-primary_hover` `#171717`); the 1 px hover outline goes | `BG_HOVER` fill (pages), `BORDER` outline (layers) |
| Page and layer row | selected, multi-selected | C `_Tree view item` Selected, a neutral fill (P2); text `text-primary`, icon `fg-primary` | pages `BG_SECONDARY` (not `BG_SELECTED`, `left.rs:273`); layers `BG_SELECTED` |
| Layer row | selected + hover | C Selected-Hover (A's role for `BG_SELECTED_HOVER`) | `BG_SELECTED_HOVER` |
| Layer row | child of a selected layer | C's secondary-selected tint (A's role for `BG_SELECTED_SECONDARY`; "new name" if C has none) | `BG_SELECTED_SECONDARY` |
| Layer row | hovered on the canvas | 1 px `border-brand` outline per R4 (provisional `#FB923C`, C's TOK) | `BORDER_SELECTED` `#E36323` |
| Layer row | focused (keyboard) | `focus-ring` `#F97316`, drawn per C | none |
| Layer name | default / hidden / locked | `text-secondary` `#D4D4D4`; hidden `text-quaternary` `#A3A3A3` with the eye-off icon (7.85:1 on `#0A0A0A`); locked keeps the name colour, lock icon `fg-quaternary` (unchanged rule) | `TEXT`, `TEXT_TERTIARY` |
| Component, instance, component set | any | Studio component cyan stays (controller default): name `TEXT_COMPONENT` `#22D3EE` (9.92:1 on `#171717`), icon `CANVAS_COMPONENT`/`TEXT_COMPONENT`, type label at 75 % | unchanged |
| Kind icon, chevron | default, hover, selected | `fg-quaternary` `#737373` → `fg-quaternary_hover` `#A3A3A3` → `fg-primary` `#FFFFFF` | `ICON_TERTIARY`, `ICON`, `ICON_SELECTED` |
| Type label ("Auto layout", "Frame") | default | `text-quaternary` `#A3A3A3`, `text-xs` 12/18 | `TEXT_TERTIARY`, caption 10 |
| Eye / lock toggles | default, hover | `fg-quaternary` → `fg-quaternary_hover` | `ICON_SECONDARY`, `ICON` |
| Group-label page | default | `text-quaternary` `#A3A3A3`, `text-xs` 12/18 semibold | `TEXT_TERTIARY`, caption 10 strong |
| Divider page | default | `border-secondary` hairline | `BORDER` |
| Inline rename (page, layer, file name) | editing | C `Input field` Focused: `bg-primary` fill, `border-brand` outline, `focus-ring` (R4) | `BG`/`BG_SECONDARY` fill, `BORDER_SELECTED` |
| Drop indicator (pages, layers) | dragging | `bg-brand-solid` `#E36323` (R4; live state) | `DROP_INDICATOR` `#E36323` |
| Filter match highlight | default | brand tint (`bg-brand-solid` at A's tint alpha); one role shared with J's Find results (`FIND_HIGHLIGHT`, `theme.rs:313`), see decision 2 | `#0D99FF` at 35 % |
| Ready-for-dev icon on a page row; Dev section | default | spec G's Dev role (controller default `DEV_READY` `#14AE5C`) | `#1BC47D` literal |

3. **Widgets (spec C).** Page rows and layer rows: `_Tree view item` (C; rows at C's tree-row height, chevron, leading icon, label, trailing actions). Section titles: C's panel header (`text-sm` 14/20, action buttons). Search, `+`, collapse-all and hide-sidebar buttons: `Buttons/Button · Size=xs · Hierarchy=Tertiary` icon-only, see rule 5 for their size. Layer filter row: `Input field · Size=sm` with a leading search icon and a clear button. Inline rename: C's `Input field` in its Focused state, sized to the row. Page menu: `Dropdown menu`. Every hover tip: `Tooltip`. Scrollbars: C's scrollbar (`panels/mod.rs:49`, C's file).
4. **States.** Rows show Default, Hover, Selected, Selected-Hover, Focused (keyboard) and, for layers, Child-of-selected and Canvas-hover. Hidden and locked are content states on top of these. Disabled has no row form today and gets none.
5. **Sizes (R5) and their layout effect.** Width 240, region order and positions stay (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| File header | `theme.rs:280` | 65 | 65 | 0 | the name moves to `text-sm` 14/20 semibold, centred at y+24 (`left.rs:369`): the 20 px line spans y+14..y+34 inside the 65 px header |
| Section title rows | `theme.rs:286`, `:295` | 40 | 40 | 0 | title type 11 → 14 |
| Page rows | `theme.rs:290` | 24 | row 32 (R5) | +8 per row | the Pages list grows by 8 × visible rows, at most 5: +40 px; Layers starts up to 40 px lower and shows about one fewer row at 1088 px |
| Layer rows | `theme.rs:296` | 32 | row 32 | 0 | none |
| Filter row | `left.rs:183`, `:186` | 32 | `Input field · Size=sm` height from C (C draft: 32) | 0 if 32 | none |
| Panel action buttons (search, `+`, collapse, hide) | `PANEL_ACTION_BUTTON` = `TARGET_MIN` 24 (`theme.rs:287`) | 24 | 24 icon button, or button `xs` 32 if C has no 24 px icon button (Open question 1) | 0 or +8 width | none in height (they sit in 40 px rows); at 32 the two-button cluster widens by 16 px and the title row keeps its order |
| Row names, sublabel, type labels | `font_body` 11, `font_caption` 10 | 10–11 | `text-xs` 12/18 | 0 | names elide about 8 % earlier at the fixed name column (`layers.rs:543`); the type label hides sooner (`split_row_text`, `layers.rs:230-236`) |
| Dev section | `left.rs:95`, `:140` | 52 / 54 | per spec G | per G | per G |

6. **Radii.** Row pills and inline fields take B's control radius (`radius-md` 8 per R5); the rename field's `RADIUS_MARK` 2 becomes the field radius; nested shapes stay concentric (P4).
7. **No behaviour change.** Click, double-click, shift/cmd-click, Alt-click on a chevron, drag and drop, rename with Tab, filter and keyboard flow stay as they are. Widget ids recorded through `widgets::record` (`left-dev-ready-section`, `left-dev-mark-ready`, `layers-search`, `layers-collapse-all`, `layers-filter`) stay.

## 4. Data model / schema changes

None.

## 5. Contracts

**File ownership across D–K.** E owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `crates/designos-app/src/panels/left.rs` | E | the Agents/Tools branch (`:17-41`) is spec I's surface and the Assets branch (`:43-46`) spec K's: E does not restyle them. The Dev section (`:91-142`) and the page-row Ready icon (`:288-291`) are Dev surfaces: E applies spec G's Dev role in this file. #1108 adds the Pages/Layers separator here first. |
| `crates/designos-app/src/panels/layers.rs` | E | the image-job progress chip in a row (`:535`, `crate::image_jobs`) is not restyled here |
| `crates/designos-app/src/find/filter.rs` | E | the match highlight role is shared with J's `find/panel.rs`; the role lives in `theme.rs` (A) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/*` except `widgets/picker.rs`, and `panels/mod.rs` (C); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); right-panel files (F); `dev/*` including `dev/list.rs` (Ready-for-dev list and tabs drawn in the left panel), `specs.rs` (G); `terminal.rs`, `agent_*.rs` (I); `find/panel.rs`, `find/mod.rs`, `find/select.rs`, `panels/context.rs` (J); `panels/assets.rs` (K).

**Colour-literal gate.** As spec D section 5: A's proposed `scripts/check_chrome_colour_literals.py` (new name); until it exists, the section 2 command must print `chrome_sites=0` for the three files.

## 6. File-format mapping

None. The Pages height chosen by #1108 is a preference, not file content (that issue decides where it persists).

## 7. UI

- UI implementers load the `es-designer` skill first and follow the project's design system: `design/principles.md`, `design/soul.md`, A's token contract, B's scale and C's widget kit win over generic defaults. Purple Ban stays (R4).
- Component, instance and component-set rows stay cyan (controller default; P2 names cyan the component colour).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. A file with more than 5 pages: the list scrolls inside 5 × 32 = 160 px (was 120). The scrollbar keeps working (`left.rs:307`, C's scrollbar).
2. With #1108 merged, the user's dragged Pages height is in pixels; after rows grow to 32 a stored height that fitted 5 rows of 24 shows 3.75 rows of 32. #1108's clamps (at least 1 row + header, at most the content height) still apply; E re-checks the minimum with the new row height.
3. Long layer names at 12 px: the name elides first, then the type label hides (`split_row_text`); never overlap the eye and lock toggles (`layers.rs:474-476`).
4. A selected component row: the selected pill is neutral and the name stays cyan (not white).
5. Canvas hover on a selected row: Selected-Hover fill, not the orange outline (today's order, `layers.rs:403-414`).
6. Hidden and locked together: eye-off and lock both stay visible without hover, as today.
7. Filter on: drag and drop is off (`layers.rs:629`); matched characters keep the highlight under the selected pill.
8. Dev mode with Ready rows: the Layers header swaps to the Dev tabs (`left.rs:153-157`, spec G); the Layers rows below keep E's styling.

## 9. Non-goals

- The Pages/Layers separator and resizing (#1108); the agent terminal in the left panel (I); Assets (K); Find and replace (J); the Ready-for-dev list body (`dev/list.rs`, G).
- Canvas overlays, document content and icon art (R6).
- Changing the left panel width (240) or its resize handle.

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for `panels/left.rs`, `panels/layers.rs` and `find/filter.rs`.
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json`.
3. Spec L's layout gate shows the same regions, panel width and control order as the baseline; the only height change is the Pages list (+8 per visible page row, at most +40) and what follows from it.
4. Every role in section 3 traces to an A constant; `theme::` tests pass with B's updated invariants.
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/design_os_self_evolve.py` pass.
6. Contrast: names `#D4D4D4` and hidden names `#A3A3A3` reach at least 4.5:1 on `#0A0A0A` and on A's selected fill (computed this session: `#A3A3A3` on `#0A0A0A` 7.85, on `#262626` 6.00); cyan `#22D3EE` on `#171717` 9.92.
7. Before/after captures of every command below are in the PR with exit codes.

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | `chrome_sites=0` | yes (2 sites) |
| `panels::layers::tests::*` (`drop_zones`, `flatten_lists_top_first_and_honors_expansion`, `rows_cache_not_invalidated_by_canvas_dirty_or_transform_drag`) | pass; `drop_zones` uses `LAYER_ROW_HEIGHT`, unchanged at 32 | guard |
| new `panels::left::tests::pages_list_is_five_rows_of_the_row_height` (new name) | the list rect is `min(n, 5) × PAGE_ROW_HEIGHT` and `PAGE_ROW_HEIGHT` equals the R5 row (32) | yes (24) |
| new `panels::layers::tests::row_states_use_tree_row_roles` (new name) | paints a selected, a hovered, a hidden and a component row in a test `Context`; asserts the fills and text colours equal A's roles and that the component name is the cyan role | yes |
| new `find::filter::tests::match_highlight_uses_the_shared_role` (new name) | `paint_highlights` fills with the shared match role | yes |
| #1108's separator tests, after that PR | pass with the 32 px page row | per #1108 |

Run: `cargo test -p designos-app --bin designos panels::layers`, `cargo test -p designos-app --bin designos panels::left`, `cargo test -p designos-app --bin designos find::filter`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** The exit codes were recorded through the spec writer's wrapper (`run-shot.sh` in the writer's scratch folder, not in the repo), which ran `cargo run -q --release -p designos-app --bin designos -- <the same arguments>` from the repo root with an isolated `HOME` and absolute scratch paths. Each was run once on HEAD `ec47da26` by the spec writer with an isolated `HOME`; exit codes shown. `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original). `FIX` is a small fixture built with `--apply-spec` from the JSON below (component, instance, hidden layer, nested group, long name); the CLI cannot lock a layer (NodeSpec has `visible` but no `locked`).

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
FIX="$S/e-layers.designos"
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
cat > "$S/e-layers.spec.json" <<'JSON'
{"kind":"frame","name":"Revamp fixture","width":480,"height":360,"fill":"#FFFFFF","layout":{"direction":"column","gap":12,"padding":[24,24,24,24]},
 "children":[
  {"kind":"component","name":"Button","layout":{"direction":"row","padding":[8,12,8,12]},"fill":"#E36323","children":[{"kind":"text","name":"Label","text":"Share"}]},
  {"kind":"instance","name":"Button instance","component":"Button"},
  {"kind":"rectangle","name":"Hidden layer","width":120,"height":40,"fill":"#D9D9D9","visible":false},
  {"kind":"frame","name":"Group A","layout":{"direction":"column","gap":4},"children":[{"kind":"text","name":"Nested text","text":"Nested"},{"kind":"rectangle","name":"Nested box","width":60,"height":20,"fill":"#A3A3A3"}]},
  {"kind":"text","name":"A very long layer name that has to elide in the layers panel","text":"Long"}
 ]}
JSON
FIX="$S/e-layers.designos"
RUN "$FIX" --apply-spec "$S/e-layers.spec.json"                                                         # exit 0: builds the fixture
RUN "$FIX" --screenshot "$OUT/E-03-fixture-rows.png" --size 1600x1088 --no-toast --select "Revamp fixture"   # exit 0: selected top-level row; the tree stays collapsed
RUN "$FIX" --screenshot "$OUT/E-03b-fixture-rows-expanded.png" --size 1600x1088 --no-toast --select "Nested text"   # exit 1 once ("designos: screenshot was not captured"): selecting a nested layer expands its ancestors: component, instance, hidden, nested, long-name rows
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                      # exit 0: header, Pages (group label, divider), Layers (shared with spec D)
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # exit 0: selected layer row (shared with spec D)
RUN "$KIT" --screenshot "$OUT/E-04-dev-left.png" --size 1600x1088 --no-toast --dev css --dev-demo --select "Welcome to Untitled UI"  # exit 1 once ("designos: screenshot was not captured"): Dev section, Ready icon on the page row, Dev tabs
```

Not capturable from the CLI today: hover, keyboard focus, multi-select (`--select` takes one name), locked rows, the layer filter row (no flag opens it), inline rename and drag indicators. The PR covers them with the new paint tests above and a manual window capture, labelled manual.

## 11. Performance budgets

- The layers list stays virtualised (`layers.rs:354-355`: only rows in view are painted); a taller page row does not change the per-frame row count of the layers list.
- No allocation added per row; roles are `const`. No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). `rows_cache` invalidation rules (`layers.rs:254-315`) are unchanged.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the PR. Then: roles in `left.rs` and `layers.rs`, then `find/filter.rs`, then R5 sizes (page row 32, type), then evidence.
- **#1108 (Pages/Layers separator, P1, open).** An Argon worker has the worktree `agy-1108-separator` (cut from `35a65796`, no commits yet; spec `plans/261025-0915-agy-argon-wave-1/spec-g-pages-layers-separator-1108.md`). It edits `panels/left.rs` and adds `theme.rs` layout constants, a brand-colour drag highlight and a focus ring. Order: #1108 merges first; E rebases on it and maps its highlight to `bg-brand-solid` and its focus ring to `focus-ring` (R4). E's Pages minimum and default heights use the 32 px row.
- **#1106 (icon-only rail).** No E file; only the panel's x position follows the rail width (handled by D and L).
- **Open PRs touching E files** (`gh pr list --state open`, 2026-10-25): #770, #768, #765 (stale `task/5xx` branches touching `panels/left.rs` and `panels/layers.rs`). No other open PR touches them.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R4 (Studio orange, dark label, focus ring Orange-500 `#F97316`, Purple Ban), R5 (`text-xs` 12/18 rows and labels, `text-sm` 14/20 panel titles, row 32, `radius-md` 8), R6 (keep icons), R3 (no Figma), R7 (spec-driven).
- Controller default (2026-10-25): component/instance tint stays cyan; canvas overlays keep Figma-parity colours.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. The pointer-hover outline on layer rows becomes C's hover fill, so pages and layers hover the same way; the canvas-hover outline stays as an outline (it marks a different source of hover).
2. The filter match highlight leaves chrome blue for a brand tint, and it is one role with J's Find results, so both lists look the same. A owns the constant (`FIND_HIGHLIGHT` today); if J's spec chooses differently, A settles it once.
3. Hidden layer names take `text-quaternary` `#A3A3A3`, one step below the `text-secondary` names, and the eye-off icon carries the state. Untitled has no disabled text role in this export (no `*-disabled` key in `1_ Color modes.Dark mode`), and `text-placeholder` `#737373` fails 4.5:1 (4.18 on `#0A0A0A`, 3.19 on `#262626`).
4. Page rows take the R5 row (32) like layer rows; the five-row cap stays.

## Open questions for the owner

1. **24 px panel icon buttons.** The search, `+`, collapse-all and hide-sidebar buttons are 24 px (`TARGET_MIN`); Untitled's smallest button is `xs` 32. Recommended: keep 24 px icon buttons (C's compact icon button) inside the 40 px title rows, so the title row and its two-button cluster keep their geometry (R1). Alternative: button `xs` 32, the cluster widens by 16 px.
2. **Pages list height.** Rows at 32 make a five-page list 40 px taller and push Layers down. Recommended: accept it (R5 names rows 32; #1108 lets the user drag the split). Alternative: cap the default Pages height at today's 120 px (3.75 rows of 32) and scroll.
