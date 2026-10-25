# I — Surface: Agents panel, terminal and agent cards (Untitled UI dark)

| Field | Value |
|---|---|
| Issue | #1144 (epic #1118, spec key I) |
| Size | L (about 2–3 days: 1 file with 54 chrome colour sites, 6 files with 1–10 sites, about 25 files that only re-point sizes; no behaviour change) |
| Depends on | A, B, C and L |
| Unblocks | M (docs and screenshot refresh of the Agents pages) |
| Touches | `crates/designos-app/src/terminal.rs` (palette `:29-56`, tab provider marks `:4180-4253`, `show_left` `:5001-5530`, `paint_turn_receipt_row` `:5535`, `show_dock_with_doc` `:5609`, `render_terminal_surface` `:5654`, `render_vt100_viewport` `:5972-6524`, `draw_bar_icon_pill` `:6525`); `agent_approval_card.rs`; `agent_composer.rs`; `agent_mention_popup.rs`; `agent_open_flow.rs`; `agent_picker.rs`; `agent_sample_prompts.rs`; `agent_selection_chip.rs`; `agent_session_failure.rs`; `agent_start_failure.rs`; `agent_stop.rs`; `agent_turn_card.rs`; `agent_presence.rs`; `agent_busy_frames.rs`; `canvas/agent_banner.rs`; `canvas/agent_changes_chip.rs`; `canvas/agent_overlay.rs`; `skills/mod.rs`; `skills/pills.rs`; `image_jobs.rs`; tests: new `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name) |

**Code baseline.** Every `file:line` below is at `ec47da26` (local `main`, 2026-10-25). `origin/main` is `205c72ef` and has moved these files (`git diff --stat ec47da26 origin/main -- <files>`): `terminal.rs` 1,404 lines changed, `agent_picker.rs` 405, `agent_sessions.rs` 274, `agent_composer.rs` 192, `image_jobs.rs` 123, `agent_permissions.rs` 113, `agent_open_flow.rs` 75, smaller changes in 6 more agent files. Re-anchor every line against the branch HEAD before editing (search for the quoted code, not the number) and re-run the census of section 2; report drift in the PR. Colour names are the token contract's (`specs/token-contract.md`, spec A and B); Untitled role names are given beside them for review only.

## 1. Summary and user story

As a designer running coding agents inside Studio, I want the Agents panel, its terminal and every agent card to look like the rest of the Untitled-styled app, so the agent area no longer reads as a separate product with its own greys, blue chips and white-on-orange buttons.

The panel keeps its regions, widths and order (R1). Colours move to the token contract's chrome constants; buttons, tabs, chips, badges, popovers and dialogs take spec C's widgets; sizes follow R5 (editor panel: text-xs rows and labels, button xs 32, radius-md 8). What the terminal prints (ANSI colours, cell font, cursor, cell selection) is content and stays as it is (controller default).

## 2. Current state

FACT unless labelled. Screenshots of today are in section 10 (`I-01`..`I-13`).

**Where the panel is drawn.** `terminal::show_left` (`terminal.rs:5001`) is called for the left Agents panel; when the active tab runs a PTY CLI it paints the bar, tabs and viewport (`:5044-5367`); otherwise it paints the legacy chat panel (`:5369-5526`, reachable through `agent_composer.rs:1043` and the Dev chat entry points `specs.rs:2886`, `:3187`; the default for a new tab is PTY, `terminal.rs:1454`). The bottom dock is `show_dock_with_doc` (`:5609`) and both call `render_terminal_surface` (`:5654`) → `render_vt100_viewport` (`:5972`).

**Inventory (geometry today).**

| Element | Code | Today |
|---|---|---|
| Header bar | `terminal.rs:5048` (`bar_h = 36.0`), fill `:5063` `TERM_BG_BAR` | 36 high |
| Pop-out button | `:5067` (24×24), icon `#D4D4D4` literal `:5072` | 24 |
| Stop controls | `agent_stop.rs:384` (`paint_header_stop_controls`), `btn_h = CONTROL_STANDARD` `:394`, width `CONTROL_ROW + SPACE_SECTION` `:400` | 28 high, 64 wide |
| Session tabs | `:5105` (24 high), widths `:5096-5101`, close × `:5106` (12), active underline `TERM_ORANGE` `:5132`, provider mark `paint_orca_tab_status_and_provider` `:4180-4253` (provider colours `:4227-4246`) | 24 |
| Add `+` | `:5181` (28×24), glyph `Color32::WHITE` `:5191` | 28×24 |
| Skills chip (header) | `:5207-5212` (`CONTROL_STANDARD`), painter `skills/mod.rs:757` (`paint_chip`), colours `:789`, `:791` | 28 |
| Context strip | `:5214-5218` (`ctx_h = CONTROL_DENSE`), fill `BG_PRIMARY`, hairline `BORDER`; selection chip `agent_selection_chip.rs:160` | 24 |
| Turn receipt row | `:5320` (`receipt_h = 24.0`), painter `:5535` | 24 |
| Runtime dropdown / sessions / skills popovers | `:5355-5357` → `agent_picker.rs:440`; `:5359-5360` → `agent_open_flow.rs:433`; `:5362` → `skills/mod.rs:816` (fill `#1F2024` `:836`, stroke `#3E3E46` `:837`, title `WHITE` `:848`, checkbox `:925`, `:934`, name `WHITE` `:954`) | menus and popovers |
| Legacy chat panel | `:5369-5526`: panel `#2C2C2C`, header 48 (`:5371-5440`), runtime bar 28 (`:5446-5500`) | 18 literal sites |
| Viewport | `:6005` fill `TERM_BG_VIEWPORT`; cell font `FontId::new(11.5, Monospace)` `:6268`; inverse video `:6317-6320`; cell selection `TERM_SELECTION_BG` `:6330`; search match `#5C4A00` `:6332`; cursor `:6408`, `:6412`, `:6424`; scrollbar `:6428-6435`; search bar 280×28 `#171717` `:6455-6470`; search text `WHITE` `:6518` | content + chrome |
| Dock grip | `:5623` (`#52525B`) | — |
| Approval card | `agent_approval_card.rs:23-28` (`approval_card_outer_height`: Claude 104, others 72, plus 2 × `SPACE_ROW`), header badge 24 `:61-69` (`STATUS_WARN_BG/BORDER`), summary row 28 `:100`, action row 28 `:112`, buttons `paint_card_button` `:137` (28 high, min width 40 `:148-149`) | Claude 112 outer, others 80 outer |
| Permissions modal | `agent_approval_card.rs:168` (`paint_permissions_modal`, 360 wide), option row `CONTROL_ROW + CONTROL_STANDARD` = 68 `:174`, options `:219-249` | dialog |
| Turn cards | `agent_turn_card.rs:82`, `:105`, `:112`, `:165`, `:201` | theme constants only |
| Mention popup | `agent_mention_popup.rs:283` (popup), `:444` (mention button) | theme constants only |
| Composer | `agent_composer.rs:345` (`composer_box_height` = `CONTROL_ROW` × 2 + extra lines × `LINE_BODY` + chips `CONTROL_DENSE + SPACE_ROW`), `show_composer` `:811`, uses of the terminal palette `:1105`, `:1116` | 80 + lines |
| Session failure banner / start failure / sample prompts | `agent_session_failure.rs:852`; `agent_start_failure.rs:239` (buttons `CONTROL_STANDARD` `:234`, computed mix `:408`); `agent_sample_prompts.rs:250`, `:378` | theme constants |
| Open-session flow | `agent_open_flow.rs:316`; status dots `:325` `#EAB308`, `:337` `#F97316`, `:342` `#10B981`; provider badges `:360-375` | 7 literal sites |
| Runtime picker | `agent_picker.rs:244`, `:259`, `:271`, `:277`, `:440`, `:592`; "Save" label `Color32::WHITE` on `BG_BRAND` `:663` | 1 literal site |
| AI image jobs (expand, erase) | `image_jobs.rs:1202`, `:1275`, `:1281`, `:1321`, `:1427`, `:1542`; "Expand" and "Erase" labels `Color32::WHITE` on `BG_BRAND` `:1413`, `:1524` | 2 literal sites |
| Canvas agent banner / changes chip / overlay | `canvas/agent_banner.rs:22`; `canvas/agent_changes_chip.rs:388` (computed colour `:373`); `canvas/agent_overlay.rs:24` (computed `:20`) | theme constants; presence colours `AGENT_1..4` |

**Terminal palette** (`terminal.rs`, its own constants, consumed by other files):

| Constant | Line | Value | Uses outside its line (`grep -rnw <NAME> crates/designos-app/src`, minus the `pub const` line) |
|---|---|---|---|
| `TERM_BG_VIEWPORT` | `:30` | `#0E0E12` | 6 (terminal 5, `cmdk.rs:571`) |
| `TERM_BG_BAR` | `:31` | `#16171B` | 4 |
| `TERM_BG_ELEVATED` | `:32` | `#1F2024` | 9 |
| `TERM_BG_HOVER` | `:33` | `#26272C` | 19 |
| `TERM_BORDER` | `:34` | `#2A2B31` | 19 |
| `TERM_ORANGE` | `:35` | `#FF6A3D` | 2 (`:5132`, `:5828`) |
| `TERM_ORANGE_BG` | `:37` | `#361A12` | 0 (dead) |
| `TERM_FG_DEFAULT` | `:38` | `#E4E4E7` | 5 (all cell painting or tests: `:6317`, `:6398`, `:6554`, `:7144`, `:7146`) |
| `TERM_SELECTION_BG` | `:39` | `#5A7898` | 1 (`:6330`, cell selection) |
| `STATUS_OK_TEXT/BG/BORDER` | `:42-44` | `#4CC58B` / `#17302A` / `#2C5F4A` | text 12 |
| `STATUS_WARN_TEXT/BG/BORDER` | `:46-48` | `#E0A53C` / `#35291A` / `#5C4720` | text 21 |
| `STATUS_BAD_TEXT/BG/BORDER` | `:50-52` | `#FF7A8C` / `#3B1D23` / `#6A2E39` | text 14 |
| `STATUS_INFO_TEXT/BG/BORDER` | `:54-56` | `#8FB0FF` / `#1B2640` / `#2F4577` | text 19 (name collides with `theme::STATUS_INFO_*`, `theme.rs:569-571`, `#0D99FF`) |

Files that import them (`grep -rlw` on the names): `agent_approval_card.rs`, `agent_picker.rs`, `agent_composer.rs`, `skills/mod.rs` (I), `cmdk.rs` (J, `:12-14`, used `:571-614`), `panels/check_designs.rs` (unassigned, `:7`, used `:81`, `:83`, `:84`, `:511`), `canvas/draw.rs` (D or L; uses the `theme::STATUS_INFO_*` names).

**Chrome colour census** (spec D's command, used unchanged by D–K; a "chrome site" is a `Color32` constructor with literal arguments, a `from_gray` / `from_black_alpha` / `from_white_alpha` call, or a named `Color32` other than `TRANSPARENT`, in non-test code as `scripts/rust_literal_scan.clean` strips it; "computed" is a constructor with non-literal arguments):

```bash
cd /Users/jangtrinh/Products/design-os-studio
PYTHONDONTWRITEBYTECODE=1 python3 - terminal.rs agent_approval_card.rs agent_composer.rs agent_mention_popup.rs \
  agent_open_flow.rs agent_picker.rs agent_sample_prompts.rs agent_selection_chip.rs agent_session_failure.rs \
  agent_start_failure.rs agent_stop.rs agent_turn_card.rs agent_presence.rs agent_busy_frames.rs \
  canvas/agent_banner.rs canvas/agent_changes_chip.rs canvas/agent_overlay.rs skills/mod.rs skills/pills.rs image_jobs.rs <<'PY'
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

Result at `ec47da26` (run 2026-10-25, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `terminal.rs` | 79 | **54** | palette `:30-35`, `:37-39`, `:42-44`, `:46-48`, `:50-52`, `:54-56` (21); provider `:4227`, `:4231`, `:4235`, `:4242`, `:4246` (5); `:5072`; `:5191`; legacy chat `:5369`, `:5376`, `:5384`, `:5393`, `:5399`, `:5414`, `:5418`, `:5421`, `:5422`, `:5435`, `:5449`, `:5450`, `:5467`, `:5469`, `:5470`, `:5489`, `:5491`, `:5492` (18); grip `:5623`; match `:6332`; cursor `:6408`, `:6412`, `:6424`; scrollbar `:6435`; search bar `:6465`; `:6518` | `:6559`, `:6587`, `:6595`, `:6599` (ANSI conversion, content) |
| `agent_open_flow.rs` | 71 | 7 | `:325`, `:337`, `:342`, `:360`, `:363`, `:371`, `:375` | — |
| `skills/mod.rs` | 75 | 10 | `:789`, `:791` (×3), `:836`, `:837`, `:848`, `:925`, `:934`, `:954` | — |
| `image_jobs.rs` | 52 | 2 | `:1413`, `:1524` | — |
| `agent_picker.rs` | 123 | 1 | `:663` | — |
| `canvas/agent_changes_chip.rs` | 31 | 0 | — | `:373` |
| `canvas/agent_overlay.rs` | 11 | 0 | — | `:20` |
| the other 13 files | 3–77 | 0 | — | — |

Total today: **74** chrome sites in I's files. (plan.md's `grep -c 'Color32::from_rgb('` counts include tests: terminal 60, agent_open_flow 7, skills 7; the census above is the contract.)

**Gates today.** No gate counts colour literals. `.github/workflows/design-gates.yml` runs the design gates with `continue-on-error` (`:4-5`, `:21`), so only the macOS test, Clippy and Rustfmt jobs block a merge. `scripts/zero_fake_ui_gate.py` checks Agents invariants Z01–Z04, Z15 and Z17 on these files; `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (passes at `ec47da26`, exit 0). Tests that pin terminal colours: `terminal.rs:7162-7164` (ANSI palette, content, unchanged) and `:10019-10035` (pins `theme::INFO`, `SUCCESS`, `WARNING` and token JSON; spec A moves the `SUCCESS` and `WARNING` pins to `#22C55E` / `#EAB308`, token contract section (e)).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Every region keeps its x, width and order: header bar 36, context strip 24, receipt row 24, legacy chat header 48 and runtime bar 28, the viewport, the composer, the dock. Heights change only where R5 raises a control inside a region (table 3, section 7). The viewport's top and bottom edges do not move at the same window size, so the PTY keeps the same columns and rows (cell font `11.5` monospace `:6268` is content and does not change).
2. **Terminal content stays.** ANSI colours (`vt100_color_to_egui`, `ansi_256_to_color32`, computed `:6559-6599`), `TERM_FG_DEFAULT`, `TERM_SELECTION_BG`, the cursor (`:6408`, `:6412`, `:6424`) and the search match fill (`:6332`) keep their values. They are the only colour literals left in `terminal.rs` (allowlist, section 10). Technical call: the match fill stays content because it sits under arbitrary ANSI glyph colours; re-tinting it would need a per-glyph contrast check that this spec does not add.
3. **Palette constants become zero-literal aliases** of contract constants (table 1, section 7), so `cmdk.rs`, `panels/check_designs.rs` and the agent files keep compiling and pick up the new values without edits. Each consumer file's owner may later replace the alias with the contract name; this spec does not edit `cmdk.rs` (J) or `panels/check_designs.rs` (unassigned).
4. **Buttons.** Every button in I's files is spec C's `Button/*` at `Size::Xs` (32): Approve, Deny, Open tab, Save, Cancel, Expand, Erase, Retry, Stop, sample prompts. A filled brand button's label is `TEXT_ON_BRAND` (dark label, R4), never `Color32::WHITE` (`agent_picker.rs:663`, `image_jobs.rs:1413`, `:1524`, `terminal.rs:5191`).
5. **Tabs.** Session tabs are C's `Tabs/Pill` at xs 32, centred in the 36 bar (2 px above and below). The active tab keeps its underline, drawn in `BORDER_SELECTED` (border-brand). Close × stays a 12 px glyph inside a hit box of at least `TARGET_MIN` 24.
6. **Badges and chips.** Status pills (approval provider badge, needs-approval, working, done, failed) are C's `Badge/StatusCount` with a status colour (section 7 table 1 and contract gap G-1). The selection chip, skills chip and image-job chip are C's `Tag/Chip` (height `BADGE_MD` 24). Skills count becomes a gray badge (blue is reserved for the info family the contract keeps: agent Working).
7. **Popovers and menus.** Runtime dropdown, sessions popover, skills popover and mention popup use C's `popover_surface` / `menu_surface` (fill `BG_MENU`, stroke `BORDER_MENU`, radius-md 8, no shadow: every kit dark shadow resolves to alpha 0). Rows are 32 (`MENU_ITEM_HEIGHT` after B). The skills checkbox is C's `Checkbox/Box` (checked = `BG_BRAND` with a `TEXT_ON_BRAND` tick), not `STATUS_INFO_TEXT` blue.
8. **Dialogs.** The permissions modal (`agent_approval_card.rs:168`) is a dialog: C's `Dialog/Modal` with R5 dialog sizes (text-sm 14/20, buttons sm 36 / md 40); its options are C's `Radio/Card`.
9. **Focus.** Every focusable control in I's files shows C's focus ring (`FOCUS_RING`, 2 px, 2 px gap) on keyboard focus; the terminal viewport's own focus state (caret visible) is unchanged.
10. **Agent identity colours.** Canvas presence colours (`AGENT_1..4`, `AGENT_COLORS`) and the info family (`STATUS_INFO_*`) keep their values (contract (e)). Provider marks on tabs and in the open flow follow Open question 1.
11. **No behaviour change.** Clicks, keyboard shortcuts, PTY input, approval decisions, queue order and accessibility labels (`agent_a11y.rs`) are untouched. Every `widgets::record` key keeps its name.

## 4. Data model / schema changes

None. No document, settings or session field changes.

## 5. Contracts

- **Colour constants consumed** (token contract (a); names exactly as there): `BG`, `BG_PRIMARY`, `BG_SECONDARY`, `BG_SECONDARY_HOVER` (new in the contract), `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BG_ERROR_SOLID`, `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_SELECTED`, `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_BRAND`, `TEXT_ERROR`, `TEXT_WARNING`, `TEXT_SUCCESS`, `TEXT_WHITE`, `ICON`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `DANGER`, `WARNING`, `SUCCESS`, `INFO`, `STATUS_INFO_TEXT/BG/BORDER`, `FOCUS_RING`, `VAR_SCROLL_THUMB` (scrollbar thumb role), `AGENT_1..4`.
- **Contract gaps this spec needs** (not in `token-contract.md`; requested from spec A, Open question 2 decides the look): G-1 tinted status badge fills and borders — proposed new names `BG_SUCCESS_PRIMARY` (bg-success-primary `#052E16`), `BG_WARNING_PRIMARY` (bg-warning-primary `#422006`), `BG_ERROR_PRIMARY` (bg-error-primary `#450A0A`), `BORDER_SUCCESS_SUBTLE` (utility-green-200 `#166534`), `BORDER_WARNING_SUBTLE` (utility-yellow-200 `#854D0E`), `BORDER_ERROR_SUBTLE` (utility-red-200 `#991B1B`). Badge text uses the contract's `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR` (ASSUMPTION: the kit badge text is utility-*-700; A confirms whether text-*-primary is close enough). G-2 provider identity colours (Open question 1). G-3 a modal scrim constant (C's `modal_surface(+scrim)`; shared with J and K). The same gaps apply to C's `Badge/StatusCount` status colours (C section 3.3: "status: Color=Error|Success|Warning|Orange").
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32, `CONTROL_SM` 36, `CONTROL_MD` 40 (renamed from `CONTROL_ROW`), `BADGE_MD` 24, `BADGE_LG` 28, `MENU_ITEM_HEIGHT` 32, `RADIUS_CONTROL` 8, `RADIUS_FRAME` 16, `LINE_BODY` 18, `FONT_SIZE_BODY` / `FONT_SIZE_CAPTION` 12, `TARGET_MIN` 24, spacing ladder unchanged. `CONTROL_DENSE` and `CONTROL_STANDARD` are legacy: every site in I's files is re-pointed (count must fall, spec B's ratchet).
- **Widgets consumed** (spec C, `widgets/kit/`, all new names there): `kit::button` with `Hierarchy::{Primary, Secondary, Tertiary, Destructive}` and `Size::Xs`; `Button/Icon` (pop-out, `+`, close); `Tabs/Pill` (`kit::tabs::tab`); `Tag/Chip`, `Badge/StatusCount`, `status_dot` (`kit::badge`); `Loading/Inline` (`kit::feedback::loading_ring`, image-job and selection chips); `Input/Search` (terminal search bar); `Menu/Panel`, `Popover/Panel` (`kit::surface`); `Checkbox/Box`, `Radio/Card` (`kit::choice`); `Dialog/Modal`; `EmptyState/Default` (sample prompts, `agent_sample_prompts.rs:250-262`, C lists it as a duplicate D–K retire).
- **Rust surface.** No public signature changes. `approval_card_outer_height` keeps its signature; its value changes (table 3). The terminal palette names stay `pub` (rule 3). New local constant `TERMINAL_RECEIPT_H = 24.0` (new name) replaces the literal at `:5320`.

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system (`design/principles.md`, `design/soul.md`, `ui ds context --dir .`) and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (contract constant first; hex is the contract's dark value):

| Element | Today | Contract constant (Untitled role) | Hex |
|---|---|---|---|
| Viewport and panel background | `TERM_BG_VIEWPORT #0E0E12` | `TERM_BG_VIEWPORT = theme::BG` (bg-primary) | `#0A0A0A` |
| Header bar, context strip, dock tab bar | `TERM_BG_BAR #16171B`, `BG_PRIMARY` | `TERM_BG_BAR = theme::BG_PRIMARY` (bg-primary) | `#0A0A0A` |
| Cards on the panel (approval, turn, failure banner, receipt) | `TERM_BG_ELEVATED #1F2024` | `TERM_BG_ELEVATED = theme::BG_SECONDARY` (bg-secondary) | `#171717` |
| Hover on the bar / on a card | `TERM_BG_HOVER #26272C` | `TERM_BG_HOVER = theme::BG_HOVER` (bg-primary_hover); on a `BG_SECONDARY` card use `BG_SECONDARY_HOVER` | `#171717` / `#262626` |
| Hairlines, card outlines | `TERM_BORDER #2A2B31` | `TERM_BORDER = theme::BORDER` (border-secondary); fields and secondary buttons get `BORDER_STRONG` from C | `#262626` / `#404040` |
| Active tab underline | `TERM_ORANGE #FF6A3D` | `TERM_ORANGE = theme::BORDER_SELECTED` (border-brand) | `#FB923C` |
| Dead brand tint | `TERM_ORANGE_BG #361A12` | deleted (0 uses) | — |
| Tab label current / other | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_QUATERNARY` (kit non-current tab text) | `#FAFAFA` / `#A3A3A3` |
| Pop-out, `+`, close glyphs | `#D4D4D4`, `Color32::WHITE` | `ICON_TERTIARY`, hover `ICON_SECONDARY` (C's `Button/Icon`) | `#A3A3A3` / `#D4D4D4` |
| Status badge success / warning / error | `STATUS_OK_*`, `STATUS_WARN_*`, `STATUS_BAD_*` | text `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR`; fill and border per Open question 2 (G-1 names or the contract-only gray form `BG_SECONDARY` + `BORDER_STRONG` with a `SUCCESS` / `WARNING` / `DANGER` dot) | `#4ADE80` / `#FACC15` / `#F87171` |
| Status badge info (agent Working) | terminal `STATUS_INFO_* #8FB0FF/#1B2640/#2F4577` | aliases of `theme::STATUS_INFO_TEXT/BG/BORDER` (kept info family) | `#0D99FF`, α 0x24, α 0x59 |
| Open-flow dots: working / waiting / done | `#EAB308`, `#F97316`, `#10B981` | `WARNING`, `TEXT_BRAND`, `SUCCESS` | `#EAB308`, `#F97316`, `#22C55E` |
| Brand button (Approve, Save, Expand, Erase, Run) | `BG_BRAND` + `Color32::WHITE` | `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` + `TEXT_ON_BRAND` | `#E36323` / `#F97316` / `#EA580C` + `#0A0A0A` |
| Secondary button (Deny, Cancel) | `BG_TERTIARY` / `BG_HOVER` + `BORDER` | C's `Button/Secondary`: `BG` + `BORDER_STRONG`, label `TEXT_SECONDARY`; hover `BG_HOVER`, label `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Stop (destructive) | theme constants (`agent_stop.rs:384-410`) | C's `Button/Destructive` xs: `BG_ERROR_SOLID` + `TEXT_WHITE` | `#DC2626` + white |
| Skills chip, selection chip, image-job chip | `TERM_BG_HOVER` + `TERM_BORDER` + `WHITE`; `#28282C` / `#3C3C42` / `#B3B3B3` | C's `Tag/Chip`: `BG` + `BORDER_STRONG`, text `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; count = gray `Badge/StatusCount` (`BG_SECONDARY` + `BORDER_STRONG` + `TEXT_SECONDARY`) | `#0A0A0A` + `#404040`, `#D4D4D4`, `#737373` |
| Popovers (runtime, sessions, skills, mention) | `#1F2024` + `#3E3E46`, title `WHITE` | `BG_MENU` + `BORDER_MENU`, title `TEXT`, rows hover `BG_MENU_HOVER`, selected `BG_SELECTED` | `#0A0A0A` + `#262626`, `#FAFAFA`, `#171717`, `#262626` |
| Skills checkbox | `STATUS_INFO_TEXT` / `#16171B`, tick `#0E0E12` | C's `Checkbox/Box`: checked `BG_BRAND` + tick `TEXT_ON_BRAND`; unchecked `BG` + `BORDER_STRONG` | `#E36323` + `#0A0A0A`; `#0A0A0A` + `#404040` |
| Invalid skill name | `STATUS_BAD_TEXT` | `TEXT_ERROR` | `#F87171` |
| Legacy chat panel | `#2C2C2C` and 17 more literals `:5369-5492` | same mapping as the PTY branch: panel `BG`, header and runtime bar `BG_PRIMARY`, hairlines `BORDER`, controls from C | as above |
| Terminal search bar | `#171717` fill `:6465`, text `WHITE` `:6518` | C's `Input/Search` xs: `BG` + `BORDER_STRONG`, text `TEXT`, placeholder `TEXT_PLACEHOLDER`, focus `FOCUS_RING` | `#0A0A0A` + `#404040`, `#FAFAFA` |
| Scrollbar thumb | literal `:6435` | `VAR_SCROLL_THUMB` (fg-quaternary, non-text 3:1) | `#737373` |
| Dock grip | `#52525B` `:5623` | `ICON_QUATERNARY` | `#737373` |
| Failure banners (session, start) | theme `DANGER` family | fill per G-1 (`BG_ERROR_PRIMARY`) or `BG_SECONDARY`; text `TEXT_ERROR`; icon `DANGER` | `#450A0A` or `#171717`; `#F87171`; `#EF4444` |
| Canvas agent banner, changes chip, overlay | theme constants, presence `AGENT_*` | chrome parts (fill, border, text) as cards above; presence colours unchanged; labels on presence colours `AGENT_LABEL_TEXT` | `#0A0A0A` label |
| Cell colours, cursor, cell selection, search match | literals | unchanged (content) | — |

**Table 2: type** (B's constants; R5 editor panel): tab labels, chips, badges, receipt and context strip text `FONT_SIZE_CAPTION` 12 / `LINE_CAPTION` 18 (today 10/14); body, menu rows and buttons `FONT_SIZE_BODY` 12 / `LINE_BODY` 18 (today 11/16); popover and card titles `FONT_SIZE_LARGE` 14 (today 13); permissions modal title text-md 16 semibold, body text-sm 14 (C's `Dialog/Modal`); weights `UI_FONT_WEIGHT` 400 / `UI_FONT_WEIGHT_STRONG` 600. Terminal cells stay `11.5` monospace (content). Numbers (usage readout, queue "1 of N") stay in `UI_MONO_FAMILY`.

**Table 3: sizes** (R5; "Δ" is the change at 1600×1088):

| Element | Today | After | Δ |
|---|---|---|---|
| Header bar | 36 | 36 (region, R1) | 0 |
| Session tab | 24 high | `CONTROL_XS` 32, 2 px inset in the bar | +8 per tab; bar 0 |
| Tab close × | glyph 12 | glyph 12, hit ≥ `TARGET_MIN` 24 | 0 |
| Add `+` | 28×24 | 32×32 (`Button/Icon` xs) | +4 w, +8 h |
| Skills chip (header) | `CONTROL_STANDARD` 28 | `CONTROL_XS` 32 | +4 |
| Pop-out | 24×24 | 32×32 | +8 |
| Stop controls | 28 high (`agent_stop.rs:394`) | `CONTROL_XS` 32 | +4 |
| Context strip | 24 | 24 (region); chip `BADGE_MD` 24 | 0 |
| Turn receipt row | 24 literal | `TERMINAL_RECEIPT_H` 24 | 0 |
| Approval card (inner) | Claude 104, others 72 | Claude 112, others 76 (summary row and action row 28 → 32; header badge stays 24 as `BADGE_MD`) | +8 / +4 |
| Approval card (outer, `approval_card_outer_height`) | 112 / 80 | 120 / 84 | +8 / +4 |
| Approval buttons | 28 high, min w 40 | 32 high, min w `CONTROL_MD` 40 | +4 h |
| Terminal search bar | 280×28 | 280×32 | +4 |
| Popover and menu rows | 24–28 | `MENU_ITEM_HEIGHT` 32 | +4 to +8 per row |
| Composer | `CONTROL_ROW` × 2 + n × 16 + 28 | `CONTROL_MD` × 2 + n × 18 + (`BADGE_MD` + 4) | +2 px per extra line (B's `LINE_BODY`) |
| Legacy chat header / runtime bar | 48 / 28 | 48 / 28 (regions); controls inside 24 → `BADGE_MD` 24 for chips, buttons that do not fit 32 stay 24 (`TARGET_MIN`) | 0 |
| Permissions modal | 360 wide, option row 68 | 360 wide; options C's `Radio/Card` (pad 16, radius-xl 12); buttons `CONTROL_SM` 36 | measured in the PR |
| Radii | `RADIUS_CONTROL` 6, `RADIUS_FRAME` 14 | 8 / 16 (B) | — |

**States** (every interactive element; looks come from C's state tables): default, hover, pressed (`BG_PRESSED`, only `TEXT` / `TEXT_SECONDARY` on it), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5), selected (`BG_SELECTED`; active tab adds the `BORDER_SELECTED` underline). Agent tab statuses: Working (info badge + `kit::feedback::loading_ring`), Needs approval (warning dot + `TEXT_WARNING` label, as in `I-05`), Done (success dot), Failed (error badge + `TEXT_ERROR`).

## 8. Edge cases

1. **Many tabs.** Tabs elide titles (`:5096-5101`); with 32-high tabs the width rules are unchanged, so the overflow point is unchanged. Check with 8 tabs: no tab overlaps `+`, Skills, stop or pop-out.
2. **Narrow panel** (left panel at its minimum width): the header still fits pop-out, stop and at least one tab; Skills label may elide; no control is clipped below 24 px wide.
3. **Approval queue with long tool input.** The summary row elides at 32 high; the queue count "1 of N" stays right-aligned; the card grows by 8 so the viewport's top content shifts down by 8 when a card is shown (the card overlays the viewport top, `viewport_rect` inset, not a layout region).
4. **Inverse video** (`:6317-6320`) uses `TERM_BG_VIEWPORT` as glyph colour: with the alias it becomes `#0A0A0A` on `#E4E4E7`, contrast 15.60:1 (was 15.18:1). No change in meaning.
5. **Legacy chat branch** must be restyled even though new tabs default to PTY; reach it with `--panel agents-chat` (`I-03`).
6. **No skills attached**: the Skills chip shows "Skills" without a count badge (unchanged logic; only the look changes).
7. **Disabled Approve** (decision in flight): 50 % opacity on fill and label, still `TEXT_ON_BRAND`.
8. **Shell and DESIGN:OS help tabs** have no provider identity today (`#A1A1AA`, `:4246`): they use `ICON_TERTIARY` whatever Open question 1 decides.
9. **Light-on-dark labels on presence colours** (`AGENT_LABEL_TEXT`) change to the dark label `#0A0A0A` by the contract; check contrast on each of `AGENT_1..4` in the PR (A's a11y gate).
10. **Image-job states** (queued, working, failed): the chip uses `Loading/Inline` for queued and working, `TEXT_ERROR` for failed; Expand and Erase dialogs keep their sizes except buttons to `CONTROL_XS`.

## 9. Non-goals

- Terminal content: ANSI palette, cell font and size, cursor shape and colour, cell selection, search match fill.
- Canvas presence colours (`AGENT_1..4`), the info family (`STATUS_INFO_*`), canvas overlays drawn by `canvas/draw.rs` (not mine).
- Any behaviour, keyboard, PTY, approval, queue, session or accessibility change.
- `cmdk.rs` and `panels/check_designs.rs` edits (they read the terminal palette; their owners migrate them).
- Light mode, icons (R6 keeps the set; only colour changes), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name), registered with one `mod agent_chrome;` line in the module list of `crates/designos-app/src/shell/tests.rs` (`:1017-1048`). It uses the shared census helper `chrome_colour_sites(src: &str) -> Vec<usize>` (new name, defined in `shell/tests.rs` by whichever of D–K lands first; spec H section 10 gives its rule) and the existing recorded-rect hook (`widgets::record`, `widgets/mod.rs:245`; tests read `ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name))))`, as `agent_picker.rs:581` does).

| Test | Asserts | Red at `ec47da26` |
|---|---|---|
| `agent_sources_paint_no_chrome_colour_literals` | `chrome_colour_sites` over the 20 files of section 2 returns only the `terminal.rs` allowlist (match fill and 3 cursor lines, identified by their code text, not line numbers); 0 everywhere else | yes (74) |
| `terminal_palette_aliases_contract_roles` | `TERM_BG_VIEWPORT == theme::BG`, `TERM_BG_BAR == theme::BG_PRIMARY`, `TERM_BG_ELEVATED == theme::BG_SECONDARY`, `TERM_BG_HOVER == theme::BG_HOVER`, `TERM_BORDER == theme::BORDER`, `TERM_ORANGE == theme::BORDER_SELECTED`, terminal `STATUS_INFO_* == theme::STATUS_INFO_*`, `STATUS_*_TEXT == theme::TEXT_SUCCESS / TEXT_WARNING / TEXT_ERROR` | yes |
| `terminal_content_colours_unchanged` | `TERM_FG_DEFAULT == #E4E4E7`, `TERM_SELECTION_BG == #5A7898`; existing ANSI pins `terminal.rs:7162-7164` still pass | no (guard) |
| `approval_card_height_follows_control_xs` | `approval_card_outer_height(Claude) == 120.0`, other presets `== 84.0` | yes (112 / 80) |
| `agents_header_controls_are_control_xs` | render the Agents panel (`--panel agents` fixture in the `Sim` harness); the recorded rects of the active tab, `agents-runtime-add-btn`, `agents-header-skills` and the pop-out are `CONTROL_XS` high and lie inside the 36 bar; add a `widgets::record` call where a key is missing | yes (24 / 24 / 28 / 24) |
| `agents_viewport_rect_unchanged` | at 1600×1088 the viewport rect equals the value captured before the change (L's baseline file) | no (guard, R1) |
| `brand_buttons_use_dark_label` | `kit::button_look(Hierarchy::Primary, State::Default).text == theme::TEXT_ON_BRAND` and no I file paints `Color32::WHITE` (covered by the census) | yes (C's look missing; WHITE at 4 sites) |
| `legacy_chat_panel_uses_contract_roles` | `--panel agents-chat` renders; census for `:5369-5526` region is 0 | yes (18) |

Existing tests that must stay green: `terminal.rs` test module, `agent_picker.rs:581`, `:817`, `:832`, `agent_turn_ledger.rs:1193`, `shell/tests/live_agent_edits.rs:119` (renders `agents`, `terminal`, `agents-chat`), `shell/tests/home.rs:438-443`. `terminal.rs:10019-10035` pins move with spec A, not here.

Commands (repo root): `cargo test -p designos-app --bin designos agent_chrome`, then `cargo test -p designos-app --bin designos terminal`, then `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z01–Z04, Z15, Z17 stay OK); `python3 scripts/check_spacing_relationships.py` (exit 0; lower the baseline with `--write-baseline` only if counts fell); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence** (before = commands below at `ec47da26`, run 2026-10-25; after = the same commands on the PR branch; both sets go in the PR). Setup: `export PATH="$HOME/.cargo/bin:$PATH"`; seed a library once with `cargo run -q --release -p designos-app --bin designos -- --seed-library --library "$LIB"` (exit 0); `FILE` = the third seeded file ("Design system", `"$LIB"/projects/*/files/*.designos`). Each run: `cargo run -q --release -p designos-app --bin designos -- "$FILE" <args> --screenshot <out>.png --size 1600x1088 --no-toast`.

| Id | `<args>` | Exit code (this session) |
|---|---|---|
| I-01 | `--panel agents` | 0 |
| I-02 | `--panel terminal` | 0 |
| I-03 | `--panel agents-chat` | 1 ("screenshot was not captured"), retry 0 |
| I-04 | `--panel agents-picker` | 0 |
| I-05 | `--panel agents-approval-command` | 0 |
| I-06 | `--panel agents-approval-queued` | 0 |
| I-07 | `--panel agents-approval-open-tab` | 0 |
| I-08 | `--panel agent-permissions` | 0 |
| I-09 | `--panel agents-skills` | 0 |
| I-10 | `--panel agents-start-failure` | 1 ("screenshot was not captured"), retry 0 |
| I-11 | `--panel ai-image-menu` | 0 |
| I-12 | `--panel ai-image-expand` | 0 |
| I-13 | `--panel ai-image-erase` | 0 |

The two exit-1 runs happened while other agents were building in the same checkout; both retries passed. If a run prints "screenshot was not captured", re-run it before judging. Not reachable from the CLI (state needs a live agent): session failure banner, turn cards with a ledger, mention popup, canvas agent banner and changes chip, image-job chip in its working state; capture them from a live session in the PR and say so.

**Acceptance criteria**

1. The census command of section 2 prints `chrome_sites=0` for every I file except `terminal.rs`, which prints exactly the 4 allowlisted content sites (match fill, 3 cursor lines).
2. The 8 tests above pass; the existing tests listed above pass.
3. `CONTROL_DENSE` and `CONTROL_STANDARD` references in I's files: 0 (`grep -rnwE 'CONTROL_(DENSE|STANDARD)' <I files>` prints nothing).
4. `I-01`..`I-13` after-shots show: no blue chip outside the info family, no white label on orange, tabs 32 inside the 36 bar, and the viewport at the same position as before (L's layout gate passes for the Agents captures).
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/check_spacing_relationships.py` exit 0.
6. The PR lists the re-anchored line numbers and the before/after census.

## 11. Performance budgets

No new allocation per frame: aliases are `const`, C's looks are pure functions over constants. The viewport paint path (`render_vt100_viewport`) changes no code except the scrollbar and search-bar colours. Idle CPU stays 0.00 % (no new `request_repaint`). Measure frame time with the Agents panel open on the 1600×1088 window before and after; it must not rise by more than 0.5 ms (the change is colour and a few rect sizes).

## 12. Dependencies, ownership and build order

**Build order.** L captures the Agents baseline (`I-01`..`I-13` are also L inputs) → A adds the contract constants (and G-1..G-3 if accepted) → B lands sizes and renames (`CONTROL_ROW` → `CONTROL_MD`) → C lands `widgets/kit/` → this spec. If C is not merged, do not hand-paint anatomy in I's files: wait, because C retires duplicates in `agent_approval_card.rs:137-160`, `:168-200`, `:219-249`, `agent_selection_chip.rs:160-200`, `image_jobs.rs:1229-1250`, `agent_sample_prompts.rs:250-262` (C section 5.1).

**Technical decisions recorded here** (the owner may reverse): (1) terminal palette names stay as aliases instead of being renamed (17 consumer sites in other owners' files keep compiling); (2) session tabs grow to 32 inside the unchanged 36 bar instead of growing the bar (R1 keeps the region; R5 sets controls to 32); (3) context strip and receipt row stay 24 (status strips, not rows or buttons); (4) the terminal search-match fill stays content (rule 2); (5) the Skills count badge becomes gray (blue stays reserved for the info family).

**File ownership across H–K.** Each file has exactly one owner (same table in H, I, J and K).

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs` (25 files at `ec47da26`, plus any `agent_*` module added later, e.g. by #1085), `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs`, `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests.rs` | shared, append-only: one `mod` line per spec and the `chrome_colour_sites` helper (first to land adds it) |

Not mine (D–G or the kit specs): `theme.rs`, `design/*`, `scripts/*` (A, B); `widgets/*` and `widgets/kit/*` (C, except `widgets/picker.rs`, F); `tabs.rs`, `shell.rs` (toast painted by D with J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs` (including the command palette `:879-1057`), `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, `panels/properties.rs`, `panels/inspect.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G; `specs.rs:2886`, `:3187` call I's legacy chat entry, no edit needed).

Unassigned (flag to the controller, recommended owner in brackets): `panels/check_designs.rs` [K; it reads I's terminal palette and keeps compiling through the aliases], `canvas/draw.rs` [D or L; uses `theme::STATUS_INFO_*`], `decks/ui.rs` [D], `board/*` [D], `icons.rs` [C], `lint.rs` [K].

**Open PRs touching I files** (`gh pr list --state open`, 2026-10-25; file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): #1051 (`agent_session_failure.rs`), #1070 (`agent_turn_status.rs`, `terminal.rs`), #1075 (`agent_session_files.rs`, `terminal.rs`), #1081 (new `agent_cli_writers_tests.rs`), #1084 (`agent_open_flow.rs`, `agent_sample_prompts.rs`, `terminal.rs`), #1085 (`agent_composer.rs`, `agent_permissions.rs`, `agent_session_files.rs`, new `agent_approval_channel.rs`, `agent_hook_outputs.rs`); stale #765, #768, #770 (`mergeable: CONFLICTING`) also touch agent files, `terminal.rs` and `image_jobs.rs`. Land or rebase these before this spec's PR; re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor panels: text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius md 8; dialogs: text-sm 14/20, button sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- Controller defaults: terminal ANSI palette is content; chrome around it adopts Untitled; canvas presence colours keep their values.
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions for the owner

1. **Provider identity colours** (Claude `#D97757`, Codex and OpenCode `#FAFAFA`, Grok `#E4E4E7`, Antigravity and Gemini `#60A5FA`, Shell and DESIGN:OS help `#A1A1AA`; `terminal.rs:4227-4246` by `AgentCliPreset` at `:4226-4246`, `agent_open_flow.rs:360-375`). Recommended: keep them as identity colours (like the kept `AGENT_*` presence colours), defined once by spec A as `PROVIDER_COLORS` (new name, contract gap G-2) and read by both files. Alternative: drop them and draw every provider mark in `ICON_TERTIARY`, closer to Untitled's neutral chrome but harder to tell tabs apart.
2. **Status badge look** (approval provider badge, needs approval, done, failed, image-job failed). Recommended: Untitled's tinted status badges (`Badge · Color=Success|Warning|Error`: dark tint fill, `-200` border, coloured text), which needs the six G-1 constants from spec A. Alternative that needs no contract change: gray badge (`BG_SECONDARY` + `BORDER_STRONG`) with a coloured status dot and coloured text.
3. **Session tab height.** Recommended: 32 (R5 button xs) inside the unchanged 36 bar, as specified. Alternative: keep tabs at 24 to match spec D's file-tab ruling if D's Open question 1 keeps file tabs short; then I follows D for consistency.
