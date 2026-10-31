---
id: detail-1144-surface-agent-terminal-and-agent-ca
type: note
title: "I — Surface: Agents panel, terminal and agent cards (Untitled UI dark)"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1144-surface-agent-terminal-and-agent-cards.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1144 (epic #1118, spec key I) |
| Size | L (about 2–3 days: 1 file with 51 chrome colour sites, 4 files with 1–10 sites, about 25 files that only re-point sizes; no behaviour change) |
| Depends on | A, B, C and L |
| Unblocks | M (docs and screenshot refresh of the Agents pages) |
| Touches | `crates/designos-app/src/terminal.rs` (palette `:29-56`, tab provider marks `:4232-4309`, `show_left` `:4991` with its body in `show_left_with_doc` `:5051-5538`, `paint_turn_receipt_row` `:5547`, `render_vt100_viewport` `:5608-6159`); `agent_approval_card.rs`; `agent_composer.rs`; `agent_mention_popup.rs`; `agent_open_flow.rs`; `agent_picker.rs`; `agent_sample_prompts.rs`; `agent_selection_chip.rs`; `agent_session_failure.rs`; `agent_start_failure.rs`; `agent_stop.rs`; `agent_turn_card.rs`; `agent_presence.rs`; `agent_busy_frames.rs`; `canvas/agent_banner.rs`; `canvas/agent_changes_chip.rs`; `canvas/agent_overlay.rs`; `skills/mod.rs`; `skills/pills.rs`; `image_jobs.rs`; tests: new `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name) |

**Code baseline.** Every `file:line` below is at `origin/main` `205c72ef`, read with `git show 205c72ef:<path>` (no checkout). Between local `main` `ec47da26` and `205c72ef` I's files moved (`git diff --stat ec47da26 205c72ef -- <I files>`: `terminal.rs` 1,404 lines, `agent_picker.rs` 405, `agent_composer.rs` 192, `image_jobs.rs` 123, `agent_open_flow.rs` 75, `agent_approval_card.rs` 44, `agent_turn_card.rs` 13, `agent_selection_chip.rs` 2, `agent_stop.rs` 2). The largest change: release 0.6.1 (#1017) includes #864 "remove the terminal dock and the >_ rail button", so the dock, its grip and the header pop-out button no longer exist (`terminal.rs:6629` asserts the pop-out is gone). Every line was mapped from `ec47da26` by matching its code text; lines whose code changed were placed by hand: the header row (`:5092` is now the stop controls), the popover calls (`:5382-5388`), the search bar (`:6094-6107`), the legacy header (`:5398-5454`), the add-button rect read (`agent_picker.rs:611`) and the test pins (`terminal.rs:9612-9630`). Re-anchor every line against the branch HEAD before editing (search for the quoted code, not the number) and re-run the census of section 2; report drift in the PR. Colour names are the token contract's (`specs/token-contract.md`, spec A and B); Untitled role names are given beside them for review only.

## 1. Summary and user story

As a designer running coding agents inside Studio, I want the Agents panel, its terminal and every agent card to look like the rest of the Untitled-styled app, so the agent area no longer reads as a separate product with its own greys, blue chips and white-on-orange buttons.

The panel keeps its regions, widths and order (R1). Colours move to the token contract's chrome constants; buttons, tabs, chips, badges, popovers and dialogs take spec C's widgets; sizes follow R5 (editor panel: text-xs rows and labels, button xs 32, radius-md 8). What the terminal prints (ANSI colours, cell font, cursor, cell selection) is content and stays as it is (controller default).

## 2. Current state

FACT unless labelled. Screenshots of today are in section 10 (`I-01`..`I-13`).

**Where the panel is drawn.** `terminal::show_left` (`terminal.rs:4991`, body in `show_left_with_doc` `:5051`) is called for the left Agents panel; when the active tab runs a PTY CLI it paints the bar, tabs and viewport (`:5070-5393`); otherwise it paints the legacy chat panel (`:5395-5538`, reachable through `agent_composer.rs:1045` and the Dev chat entry points `specs.rs:2895`, `:3195`; the default for a new tab is PTY, `terminal.rs:1483`). The viewport is `render_vt100_viewport` (`:5608`), called once at `:5357`. There is no bottom dock at `205c72ef` (removed by #864).

**Inventory (geometry today).**

| Element | Code | Today |
|---|---|---|
| Header bar | `terminal.rs:5074` (`bar_h = 36.0`), fill `:5089` `TERM_BG_BAR`; stop controls drawn from the bar's right edge `:5092` | 36 high |
| Stop controls | `agent_stop.rs:384` (`paint_header_stop_controls`), `btn_h = CONTROL_STANDARD` `:394`, width `CONTROL_ROW + SPACE_SECTION` `:400` | 28 high, 64 wide |
| Session tabs | `:5117` (24 high), widths `:5108-5113`, close × `:5118` (12), active underline `TERM_ORANGE` `:5151`, provider mark `paint_orca_tab_status_and_provider` `:4232-4309` (provider colours `:4283-4302`) | 24 |
| Add `+` | `:5200` (28×24), glyph `Color32::WHITE` `:5210` | 28×24 |
| Skills chip (header) | `:5231-5236` (`CONTROL_STANDARD`), painter `skills/mod.rs:779` (`paint_chip`), colours `:789`, `:791` | 28 |
| Context strip | `:5238-5242` (`ctx_h = CONTROL_DENSE`), fill `BG_PRIMARY`, hairline `BORDER`; selection chip `agent_selection_chip.rs:160` | 24 |
| Turn receipt row | `:5344` (`receipt_h = 24.0`), painter `:5547` | 24 |
| Runtime dropdown / sessions / skills popovers | `:5382-5384` → `agent_picker.rs:467`; `:5385-5387` → `agent_open_flow.rs:455`; `:5388` → `skills/mod.rs:816` (fill `#1F2024` `:836`, stroke `#3E3E46` `:837`, title `WHITE` `:848`, checkbox `:925`, `:934`, name `WHITE` `:954`) | menus and popovers |
| Legacy chat panel | `:5395-5538`: panel `#2C2C2C`, header 48 (`:5398-5454`: Back `:5405`, AI `:5438`), runtime bar 28 (`:5458-5512`) | 17 literal sites |
| Viewport | `:5641` fill `TERM_BG_VIEWPORT`; cell font `FontId::new(11.5, Monospace)` `:5905`; inverse video `:5954-5957`; cell selection `TERM_SELECTION_BG` `:5967`; search match `#5C4A00` `:5969`; cursor `:6045`, `:6049`, `:6061`; scrollbar `:6065-6072`; search bar 280×28 `#171717` `:6094-6107` (fill `:6102`); search text `WHITE` `:6155` | content + chrome |
| Approval card | `agent_approval_card.rs:23-28` (`approval_card_outer_height`: Claude 104, others 72, plus 2 × `SPACE_ROW`), header badge 24 `:61-69` (`STATUS_WARN_BG/BORDER`), summary row 28 `:100`, action row 28 `:112`, buttons `paint_card_button` `:137` (28 high, min width 40 `:148-149`) | Claude 112 outer, others 80 outer |
| Permissions modal | `agent_approval_card.rs:168` (`paint_permissions_modal`, 360 wide), option row `CONTROL_ROW + CONTROL_STANDARD` = 68 `:174`, options `:219-249` | dialog |
| Turn cards | `agent_turn_card.rs:82`, `:105`, `:112`, `:178`, `:214` | theme constants only |
| Mention popup | `agent_mention_popup.rs:283` (popup), `:444` (mention button) | theme constants only |
| Composer | `agent_composer.rs:345` (`composer_box_height` = `CONTROL_ROW` × 2 + extra lines × `LINE_BODY` + chips `CONTROL_DENSE + SPACE_ROW`), `show_composer` `:811`, uses of the terminal palette `:1107`, `:1118` | 80 + lines |
| Session failure banner / start failure / sample prompts | `agent_session_failure.rs:852`; `agent_start_failure.rs:239` (buttons `CONTROL_STANDARD` `:234`, computed mix `:408`); `agent_sample_prompts.rs:250`, `:378` | theme constants |
| Open-session flow | `agent_open_flow.rs:338`; status dots `:347` `#EAB308`, `:359` `#F97316`, `:364` `#10B981`; provider badges `:382-397` | 7 literal sites |
| Runtime picker | `agent_picker.rs:260`, `:283`, `:295`, `:301`, `:467`, `:622`; "Save" label `Color32::WHITE` on `BG_BRAND` `:711` | 1 literal site |
| AI image jobs (expand, erase) | `image_jobs.rs:1214`, `:1287`, `:1293`, `:1333`, `:1439`, `:1554`; "Expand" and "Erase" labels `Color32::WHITE` on `BG_BRAND` `:1425`, `:1536` | 2 literal sites |
| Canvas agent banner / changes chip / overlay | `canvas/agent_banner.rs:22`; `canvas/agent_changes_chip.rs:388` (computed colour `:373`); `canvas/agent_overlay.rs:24` (computed `:20`) | theme constants; presence colours `AGENT_1..4` |

**Terminal palette** (`terminal.rs`, its own constants, consumed by other files):

| Constant | Line | Value | Uses outside its line (`grep -rnw <NAME> crates/designos-app/src`, minus the `pub const` line) |
|---|---|---|---|
| `TERM_BG_VIEWPORT` | `:30` | `#0E0E12` | 5 (terminal 3, `cmdk.rs` 2: import and `:579`) |
| `TERM_BG_BAR` | `:31` | `#16171B` | 2 |
| `TERM_BG_ELEVATED` | `:32` | `#1F2024` | 6 |
| `TERM_BG_HOVER` | `:33` | `#26272C` | 14 |
| `TERM_BORDER` | `:34` | `#2A2B31` | 14 |
| `TERM_ORANGE` | `:35` | `#FF6A3D` | 1 (`:5151`, active tab underline) |
| `TERM_ORANGE_BG` | `:37` | `#361A12` | 0 (dead) |
| `TERM_FG_DEFAULT` | `:38` | `#E4E4E7` | 5 (all cell painting or tests: `:5954`, `:6035`, `:6166`, `:6734`, `:6736`) |
| `TERM_SELECTION_BG` | `:39` | `#5A7898` | 1 (`:5967`, cell selection) |
| `STATUS_OK_TEXT/BG/BORDER` | `:42-44` | `#4CC58B` / `#17302A` / `#2C5F4A` | text 12 |
| `STATUS_WARN_TEXT/BG/BORDER` | `:46-48` | `#E0A53C` / `#35291A` / `#5C4720` | text 20 |
| `STATUS_BAD_TEXT/BG/BORDER` | `:50-52` | `#FF7A8C` / `#3B1D23` / `#6A2E39` | text 13 |
| `STATUS_INFO_TEXT/BG/BORDER` | `:54-56` | `#8FB0FF` / `#1B2640` / `#2F4577` | text 17 (name collides with `theme::STATUS_INFO_*`, `theme.rs:568-570`, `#0D99FF`) |

Files that import them (`grep -rlw` on the names): `agent_approval_card.rs`, `agent_picker.rs`, `agent_composer.rs`, `skills/mod.rs` (I), `cmdk.rs` (J, `:14-16`, used `:579-625`), `panels/check_designs.rs` (K, `:7`, used `:81`, `:83`, `:84`, `:85`, `:511`; spec K rule 11 re-points it to theme colours, so after K it no longer imports them), `canvas/draw.rs` (D or L; uses the `theme::STATUS_INFO_*` names).

**Chrome colour census** (spec D's command, used unchanged by D–K; a "chrome site" is a `Color32` constructor with literal arguments, a `from_gray` / `from_black_alpha` / `from_white_alpha` call, or a named `Color32` other than `TRANSPARENT`, in non-test code as `scripts/rust_literal_scan.clean` strips it; "computed" is a constructor with non-literal arguments):

```bash
cd /Users/jangtrinh/Products/design-os-studio
# REV=205c72ef reads that commit with `git show` (no checkout); drop it to read your working tree
REV=205c72ef PYTHONDONTWRITEBYTECODE=1 python3 - terminal.rs agent_approval_card.rs agent_composer.rs agent_mention_popup.rs \
  agent_open_flow.rs agent_picker.rs agent_sample_prompts.rs agent_selection_chip.rs agent_session_failure.rs \
  agent_start_failure.rs agent_stop.rs agent_turn_card.rs agent_presence.rs agent_busy_frames.rs \
  canvas/agent_banner.rs canvas/agent_changes_chip.rs canvas/agent_overlay.rs skills/mod.rs skills/pills.rs image_jobs.rs <<'PY'
import re, sys, os, subprocess
sys.path.insert(0, "scripts")
from rust_literal_scan import clean
REV = os.environ.get("REV")  # set: read that commit with `git show` (no checkout); unset: read the working tree
def read(rel):
    p = "crates/designos-app/src/" + rel
    if REV:
        return subprocess.run(["git", "show", f"{REV}:{p}"], capture_output=True, text=True, check=True).stdout
    return open(p, encoding="utf-8").read()
HEX = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
                 r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)|Color32::from_(?:black|white)_alpha\("
                 r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b")
CONV = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\((?!\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,)")
for rel in sys.argv[1:]:
    code = clean(read(rel)).split("\n")
    hexs = [i + 1 for i, l in enumerate(code) for _ in HEX.finditer(l)]
    conv = [i + 1 for i, l in enumerate(code) for _ in CONV.finditer(l)]
    print(rel, "theme_lines=%d" % sum("theme::" in l for l in code), "chrome_sites=%d" % len(hexs), sorted(set(hexs)), "computed=%s" % sorted(set(conv)))
PY
```

Result at `205c72ef` (run 2026-10-25 with the block above, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `terminal.rs` | 58 | **51** | palette `:30-35`, `:37-39`, `:42-44`, `:46-48`, `:50-52`, `:54-56` (21); provider `:4283`, `:4287`, `:4291`, `:4298`, `:4302` (5); `+` glyph `:5210`; legacy chat `:5395`, `:5402`, `:5410`, `:5419`, `:5425`, `:5440`, `:5444`, `:5447`, `:5448`, `:5461`, `:5462`, `:5479`, `:5481`, `:5482`, `:5501`, `:5503`, `:5504` (17); match `:5969`; cursor `:6045`, `:6049`, `:6061`; scrollbar `:6072`; search bar `:6102`; `:6155` | `:6171`, `:6199`, `:6207`, `:6211` (ANSI conversion, content) |
| `agent_open_flow.rs` | 71 | 7 | `:347`, `:359`, `:364`, `:382`, `:385`, `:393`, `:397` | — |
| `skills/mod.rs` | 75 | 10 | `:789`, `:791` (×3), `:836`, `:837`, `:848`, `:925`, `:934`, `:954` | — |
| `image_jobs.rs` | 52 | 2 | `:1425`, `:1536` | — |
| `agent_picker.rs` | 123 | 1 | `:711` | — |
| `canvas/agent_changes_chip.rs` | 31 | 0 | — | `:373` |
| `canvas/agent_overlay.rs` | 11 | 0 | — | `:20` |
| the other 13 files | 3–77 | 0 | — | — |

Total at `205c72ef`: **71** chrome sites in I's files (74 at `ec47da26`: the dock grip and the two pop-out icons, PTY header and legacy header, went with #864). (plan.md's `grep -c 'Color32::from_rgb('` counts include tests; the census above is the contract.)

**Gates today.** No gate counts colour literals. `.github/workflows/design-gates.yml` runs the design gates with `continue-on-error` (`:4-5`, `:21`), so only the macOS test, Clippy and Rustfmt jobs block a merge. `scripts/zero_fake_ui_gate.py` checks Agents invariants Z01–Z04, Z15 and Z17 on these files; `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (passed on the local `ec47da26` working tree, exit 0; not re-run at `205c72ef`, which would need a checkout). Tests that pin terminal colours: `terminal.rs:6752-6754` (ANSI palette, content, unchanged) and `:9612-9630` (`theme_has_info_success_warning_in_token_parity`, pins `theme::INFO`, `SUCCESS`, `WARNING` and token JSON; spec A moves the `SUCCESS` and `WARNING` pins to `#22C55E` / `#EAB308`, token contract section (e)).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Every region keeps its x, width and order: header bar 36, session tabs 24 (R10.1), context strip 24, receipt row 24, legacy chat header 48 and runtime bar 28, the viewport, the composer. Heights change only where R5 raises a control inside a region (table 3, section 7). The viewport's top and bottom edges do not move at the same window size, so the PTY keeps the same columns and rows (cell font `11.5` monospace `:5905` is content and does not change).
2. **Terminal content stays.** ANSI colours (`vt100_color_to_egui`, `ansi_256_to_color32`, computed `:6171-6211`), `TERM_FG_DEFAULT`, `TERM_SELECTION_BG`, the cursor (`:6045`, `:6049`, `:6061`) and the search match fill (`:5969`) keep their values. They are the only colour literals left in `terminal.rs`: 6 allowlisted sites, matched by code text: the `TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines (`:38`, `:39`; their values have no contract constant), the match fill and the 3 cursor lines (section 10). Technical call: the match fill stays content because it sits under arbitrary ANSI glyph colours; re-tinting it would need a per-glyph contrast check that this spec does not add.
3. **Palette constants become zero-literal aliases** of contract constants (table 1, section 7). The status triples alias the contract's tinted badge roles (decided: controller default "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2"): `STATUS_OK_BG/BORDER/TEXT` = `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`, `STATUS_WARN_*` = `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`, `STATUS_BAD_*` = `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700`, `STATUS_INFO_*` = `theme::STATUS_INFO_TEXT/BG/BORDER` (kept info family). In I's own files a status colour used as plain text, not on a badge fill (invalid skill name, composer error `agent_composer.rs:1118`), uses `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR` instead of the alias. `cmdk.rs` (J) keeps compiling through the aliases and picks up the new values without edits; `panels/check_designs.rs` (K) stops importing them (K rule 11). The aliases stay `pub` until no other file reads them: never delete them in this PR.
4. **Buttons.** Every button in I's files is spec C's `Button/*` at `Size::Xs` (32): Approve, Deny, Open tab, Save, Cancel, Expand, Erase, Retry, Stop, sample prompts; labels text-sm 14 semibold 600 (R8.6, buttons only). A filled brand button's label is `TEXT_ON_BRAND` (dark label, R4), never `Color32::WHITE` (`agent_picker.rs:711`, `image_jobs.rs:1425`, `:1536`, `terminal.rs:5210`). Stop is `Button/Destructive`: `BG_ERROR_SOLID`, hover `BG_ERROR_SOLID_HOVER` `#B91C1C` (R8.7), pressed `BG_ERROR_SOLID_PRESSED` (N38), label `TEXT_WHITE`.
5. **Tabs.** Session tabs keep today's height 24 (`:5117`), centred in the 36 bar (decided: R10.1 "Editor tab heights stay as today: … I session tabs 24 … (R1 wins over R5 here)"); they take C's `Tabs/Pill` colours and radius at that height. The active tab keeps its underline, drawn in `BORDER_SELECTED` (border-brand). Close × stays a 12 px glyph inside a hit box of at least `TARGET_MIN` 24.
6. **Badges and chips.** Status pills (approval provider badge, needs-approval, working, done, failed) are C's `Badge/StatusCount` in its status colours, each the contract's tinted triple (rule 3; N24–N32). The selection chip, skills chip and image-job chip are C's `Tag/Chip` (height `BADGE_MD` 24). Skills count becomes a gray `Badge/StatusCount` (`UTILITY_NEUTRAL_50` / `UTILITY_NEUTRAL_200` / `UTILITY_NEUTRAL_700`, N21–N23; blue is reserved for the info family the contract keeps: agent Working).
7. **Popovers and menus.** Runtime dropdown, sessions popover, skills popover and mention popup use C's `popover_surface` / `menu_surface` (fill `BG_MENU`, stroke `BORDER_MENU`, radius `RADIUS_FRAME` 16 with item pills radius 8 at inset 8 (R8.5), no shadow: every kit dark shadow resolves to alpha 0). Rows are 32 (`MENU_ITEM_HEIGHT` after B). The skills checkbox is C's `Checkbox/Box` (checked = `BG_BRAND` with a `TEXT_ON_BRAND` tick; unchecked 1 px `BORDER_STRONG` `#404040`, R10.7), not `STATUS_INFO_TEXT` blue.
8. **Dialogs.** The permissions modal (`agent_approval_card.rs:168`) is a dialog: C's `Dialog/Modal` with R5 dialog sizes (text-sm 14/20; footer buttons md 40 per the controller default "Dialog footer buttons md 40"), radius `RADIUS_FRAME` 16 (kit Modals radius-2xl, checked read-only in the live kit, R9), scrim `BG_OVERLAY` (N33, `#262626` at 70 %) with no background blur (spec J decides Studio does not draw the kit's blur); its options are C's `Radio/Card` (unselected border `BORDER_STRONG`, R10.7).
9. **Focus.** Every focusable control in I's files shows C's focus ring (`FOCUS_RING`, 2 px, 2 px gap) on keyboard focus; the terminal viewport's own focus state (caret visible) is unchanged.
10. **Agent identity colours.** Canvas presence colours (`AGENT_1..4`, `AGENT_COLORS`) and the info family (`STATUS_INFO_*`) keep their values (contract (e)). Provider marks on tabs (`terminal.rs:4282-4302`) and in the open flow (`agent_open_flow.rs:382-397`) are identity content (decided: controller default "provider colours are identity content (contract G-2) — I OQ1"): both files read `theme::PROVIDER_CLAUDE` `#D97757`, `PROVIDER_CODEX` `#FAFAFA` (Codex and OpenCode), `PROVIDER_GROK` `#E5E5E5` (was `#E4E4E7`), `PROVIDER_GEMINI` `#60A5FA` (Antigravity and Gemini) (contract N34–N37); Shell and DESIGN:OS help marks (`#A1A1AA` today, `:4302`) use `ICON_TERTIARY`.
11. **No behaviour change.** Clicks, keyboard shortcuts, PTY input, approval decisions, queue order and accessibility labels (`agent_a11y.rs`) are untouched. Every `widgets::record` key keeps its name.

## 4. Data model / schema changes

None. No document, settings or session field changes.

## 5. Contracts

- **Colour constants consumed** (token contract (a); names exactly as there): `BG`, `BG_PRIMARY`, `BG_SECONDARY`, `BG_SECONDARY_HOVER` (new in the contract), `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BG_ERROR_SOLID`, `BG_ERROR_SOLID_HOVER` (N20, R8.7), `BG_ERROR_SOLID_PRESSED` (N38), `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_SELECTED`, `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_BRAND`, `TEXT_BRAND`, `TEXT_ERROR`, `TEXT_WARNING`, `TEXT_SUCCESS`, `TEXT_WHITE`, `TEXT_SECONDARY_HOVER`, `ICON`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `DANGER`, `WARNING`, `SUCCESS`, `INFO`, `STATUS_INFO_TEXT/BG/BORDER`, `FOCUS_RING`, `VAR_SCROLL_THUMB` (scrollbar thumb role), `AGENT_1..4`, `AGENT_LABEL_TEXT`.
- **Contract rows from the W1 additions** (no open gaps): tinted status triples `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`, `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`, `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700` (N24–N32, also C's `Badge/StatusCount` status colours); gray count badge `UTILITY_NEUTRAL_50` / `UTILITY_NEUTRAL_200` / `UTILITY_NEUTRAL_700` (N21–N23); provider marks `PROVIDER_CLAUDE`, `PROVIDER_CODEX`, `PROVIDER_GROK`, `PROVIDER_GEMINI` (N34–N37, identity content, in `theme.rs` so the colour-literal gate does not count them); modal scrim `BG_OVERLAY` (N33); tooltips follow the contract's tooltip rule (`BG_PRIMARY_SOLID` fill, title `TEXT` `#FAFAFA`, supporting text `TEXT_SECONDARY`) through C.
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32, `CONTROL_SM` 36, `CONTROL_MD` 40 (renamed from `CONTROL_ROW`), `BADGE_MD` 24, `BADGE_LG` 28, `MENU_ITEM_HEIGHT` 32, `RADIUS_CONTROL` 8, `RADIUS_FRAME` 16 (popovers, permissions modal), `LINE_BODY` 18, `FONT_SIZE_BODY` / `FONT_SIZE_CAPTION` 12, `TARGET_MIN` 24, spacing ladder unchanged. `CONTROL_DENSE` and `CONTROL_STANDARD` are legacy: every site in I's files is re-pointed (count must fall, spec B's ratchet).
- **Widgets consumed** (spec C, `widgets/kit/`, all new names there): `kit::button` with `Hierarchy::{Primary, Secondary, Tertiary, Destructive}` and `Size::Xs`; `Button/Icon` (`+`, close); `Tabs/Pill` (`kit::tabs::tab`); `Tag/Chip`, `Badge/StatusCount`, `status_dot` (`kit::badge`); `Loading/Inline` (`kit::feedback::loading_ring`, image-job and selection chips); `Input/Search` (terminal search bar); `Menu/Panel`, `Popover/Panel` (`kit::surface`); `Checkbox/Box`, `Radio/Card` (`kit::choice`); `Dialog/Modal`; `EmptyState/Default` (sample prompts, `agent_sample_prompts.rs:250-262`, C lists it as a duplicate D–K retire).
- **Rust surface.** No public signature changes. `approval_card_outer_height` keeps its signature; its value changes (table 3). The terminal palette names stay `pub` (rule 3). New local constant `TERMINAL_RECEIPT_H = 24.0` (new name) replaces the literal at `:5344`.

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system (`design/principles.md`, `design/soul.md`, `ui ds context --dir .`) and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (contract constant first; hex is the contract's dark value):

| Element | Today | Contract constant (Untitled role) | Hex |
|---|---|---|---|
| Viewport and panel background | `TERM_BG_VIEWPORT #0E0E12` | `TERM_BG_VIEWPORT = theme::BG` (bg-primary) | `#0A0A0A` |
| Header bar, context strip | `TERM_BG_BAR #16171B`, `BG_PRIMARY` | `TERM_BG_BAR = theme::BG_PRIMARY` (bg-primary) | `#0A0A0A` |
| Cards on the panel (approval, turn, failure banner, receipt) | `TERM_BG_ELEVATED #1F2024` | `TERM_BG_ELEVATED = theme::BG_SECONDARY` (bg-secondary) | `#171717` |
| Hover on the bar / on a card | `TERM_BG_HOVER #26272C` | `TERM_BG_HOVER = theme::BG_HOVER` (bg-primary_hover); on a `BG_SECONDARY` card use `BG_SECONDARY_HOVER` | `#171717` / `#262626` |
| Hairlines, card outlines | `TERM_BORDER #2A2B31` | `TERM_BORDER = theme::BORDER` (border-secondary); fields and secondary buttons get `BORDER_STRONG` from C | `#262626` / `#404040` |
| Active tab underline | `TERM_ORANGE #FF6A3D` | `TERM_ORANGE = theme::BORDER_SELECTED` (border-brand) | `#FB923C` |
| Dead brand tint | `TERM_ORANGE_BG #361A12` | deleted (0 uses) | — |
| Tab label current / other | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_QUATERNARY` (kit non-current tab text) | `#FAFAFA` / `#A3A3A3` |
| `+`, close glyphs | `Color32::WHITE` (`:5210`) | `ICON_TERTIARY`, hover `ICON_SECONDARY` (C's `Button/Icon`) | `#A3A3A3` / `#D4D4D4` |
| Provider marks (tabs, open flow) | `#D97757`, `#FAFAFA`, `#E4E4E7`, `#60A5FA`, `#A1A1AA` literals | `PROVIDER_CLAUDE`, `PROVIDER_CODEX`, `PROVIDER_GROK`, `PROVIDER_GEMINI`; Shell and help `ICON_TERTIARY` | `#D97757`, `#FAFAFA`, `#E5E5E5`, `#60A5FA`; `#A3A3A3` |
| Status badge success / warning / error (fill / border / text) | `STATUS_OK_*`, `STATUS_WARN_*`, `STATUS_BAD_*` | `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`; `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`; `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700` | `#052E16` / `#166534` / `#86EFAC`; `#422006` / `#854D0E` / `#FDE047`; `#450A0A` / `#991B1B` / `#FCA5A5` |
| Status badge info (agent Working) | terminal `STATUS_INFO_* #8FB0FF/#1B2640/#2F4577` | aliases of `theme::STATUS_INFO_TEXT/BG/BORDER` (kept info family) | `#0D99FF`, α 0x24, α 0x59 |
| Open-flow dots: working / waiting / done | `#EAB308`, `#F97316`, `#10B981` | `WARNING`, `TEXT_BRAND`, `SUCCESS` | `#EAB308`, `#F97316`, `#22C55E` |
| Brand button (Approve, Save, Expand, Erase, Run) | `BG_BRAND` + `Color32::WHITE` | `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` + `TEXT_ON_BRAND` | `#E36323` / `#F97316` / `#EA580C` + `#0A0A0A` |
| Secondary button (Deny, Cancel) | `BG_TERTIARY` / `BG_HOVER` + `BORDER` | C's `Button/Secondary`: `BG` + `BORDER_STRONG`, label `TEXT_SECONDARY`; hover `BG_HOVER`, label `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Stop (destructive) | theme constants (`agent_stop.rs:384-410`) | C's `Button/Destructive` xs: `BG_ERROR_SOLID`, hover `BG_ERROR_SOLID_HOVER` (R8.7), pressed `BG_ERROR_SOLID_PRESSED`, label `TEXT_WHITE` | `#DC2626` / `#B91C1C` / `#991B1B` + `#FFFFFF` |
| Skills chip, selection chip, image-job chip | `TERM_BG_HOVER` + `TERM_BORDER` + `WHITE`; `#28282C` / `#3C3C42` / `#B3B3B3` | C's `Tag/Chip`: `BG` + `BORDER_STRONG`, text `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; count = gray `Badge/StatusCount` (`UTILITY_NEUTRAL_50` + `UTILITY_NEUTRAL_200` + `UTILITY_NEUTRAL_700`) | `#0A0A0A` + `#404040`, `#D4D4D4`, `#737373`; count `#171717` / `#404040` / `#D4D4D4` |
| Popovers (runtime, sessions, skills, mention) | `#1F2024` + `#3E3E46`, title `WHITE` | `BG_MENU` + `BORDER_MENU` at radius 16 (R8.5), title `TEXT`, rows hover `BG_MENU_HOVER`, selected `BG_SELECTED` | `#0A0A0A` + `#262626`, `#FAFAFA`, `#171717`, `#262626` |
| Permissions modal | theme constants | C's `Dialog/Modal`: `BG_MENU` + `BORDER_MENU` at `RADIUS_FRAME` 16, scrim `BG_OVERLAY` (no blur) | `#0A0A0A` + `#262626`; `#262626` at 70 % |
| Skills checkbox | `STATUS_INFO_TEXT` / `#16171B`, tick `#0E0E12` | C's `Checkbox/Box`: checked `BG_BRAND` + tick `TEXT_ON_BRAND`; unchecked `BG` + `BORDER_STRONG` (R10.7) | `#E36323` + `#0A0A0A`; `#0A0A0A` + `#404040` |
| Invalid skill name | `STATUS_BAD_TEXT` | `TEXT_ERROR` | `#F87171` |
| Legacy chat panel | `#2C2C2C` and 17 more literals `:5395-5504` | same mapping as the PTY branch: panel `BG`, header and runtime bar `BG_PRIMARY`, hairlines `BORDER`, controls from C | as above |
| Terminal search bar | `#171717` fill `:6102`, text `WHITE` `:6155` | C's `Input/Search` xs: `BG` + `BORDER_STRONG`, text `TEXT`, placeholder `TEXT_PLACEHOLDER`, focus `FOCUS_RING` | `#0A0A0A` + `#404040`, `#FAFAFA` |
| Scrollbar thumb | literal `:6072` | `VAR_SCROLL_THUMB` (fg-quaternary, non-text 3:1) | `#737373` |
| Failure banners (session, start) | theme `DANGER` family | fill `BG_ERROR_PRIMARY` (N26) with a `UTILITY_RED_200` border; text `UTILITY_RED_700`; icon `DANGER` | `#450A0A` + `#991B1B`; `#FCA5A5`; `#EF4444` |
| Canvas agent banner, changes chip, overlay | theme constants, presence `AGENT_*` | chrome parts (fill, border, text) as cards above; presence colours unchanged; labels on presence colours `AGENT_LABEL_TEXT` | `#0A0A0A` label |
| Cell colours, cursor, cell selection, search match | literals | unchanged (content) | — |

**Table 2: type** (B's constants; R5 editor panel): tab labels, chips, badges, receipt and context strip text `FONT_SIZE_CAPTION` 12 / `LINE_CAPTION` 18 (today 10/14); body and menu rows `FONT_SIZE_BODY` 12 / `LINE_BODY` 18 (today 11/16); button labels at every size text-sm 14 semibold 600 (R8.6: "Applies to buttons only; row, label and tab text stays text-xs 12 per R5"); popover and card titles `FONT_SIZE_LARGE` 14 (today 13); permissions modal title text-md 16 semibold, body text-sm 14 (C's `Dialog/Modal`); weights `UI_FONT_WEIGHT` 400 / `UI_FONT_WEIGHT_STRONG` 600. Terminal cells stay `11.5` monospace (content). Numbers (usage readout, queue "1 of N") stay in `UI_MONO_FAMILY`.

**Table 3: sizes** (R5; "Δ" is the change at 1600×1088):

| Element | Today | After | Δ |
|---|---|---|---|
| Header bar | 36 | 36 (region, R1) | 0 |
| Session tab | 24 high | 24 (R10.1), centred in the 36 bar | 0 |
| Tab close × | glyph 12 | glyph 12, hit ≥ `TARGET_MIN` 24 | 0 |
| Add `+` | 28×24 | 32×32 (`Button/Icon` xs) | +4 w, +8 h |
| Skills chip (header) | `CONTROL_STANDARD` 28 | `CONTROL_XS` 32 | +4 |
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
| Radii | `RADIUS_CONTROL` 6, `RADIUS_FRAME` 14 | 8 / 16 (B; popovers and the permissions modal 16, R8.5 and the kit Modals) | — |

**States** (every interactive element; looks come from C's state tables): default, hover, pressed (`BG_PRESSED`, only `TEXT` / `TEXT_SECONDARY` on it), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5), selected (`BG_SELECTED`; active tab adds the `BORDER_SELECTED` underline). Agent tab statuses: Working (info badge + `kit::feedback::loading_ring`), Needs approval (warning dot + `TEXT_WARNING` label, as in `I-05`), Done (success dot), Failed (error badge: `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700`; `TEXT_ERROR` only where the status is plain text).

## 8. Edge cases

1. **Many tabs.** Tabs elide titles (`:5108-5113`); tab height (24) and width rules are unchanged, so the overflow point is unchanged unless the `+` button's 32 width moves it by 4 px. Check with 8 tabs: no tab overlaps `+`, Skills or the stop controls.
2. **Narrow panel** (left panel at its minimum width): the header still fits the stop controls, `+` and at least one tab; Skills label may elide; no control is clipped below 24 px wide.
3. **Approval queue with long tool input.** The summary row elides at 32 high; the queue count "1 of N" stays right-aligned; the card grows by 8 so the viewport's top content shifts down by 8 when a card is shown (the card overlays the viewport top, `viewport_rect` inset, not a layout region).
4. **Inverse video** (`:5954-5957`) uses `TERM_BG_VIEWPORT` as glyph colour: with the alias it becomes `#0A0A0A` on `#E4E4E7`, contrast 15.60:1 (was 15.18:1). No change in meaning.
5. **Legacy chat branch** must be restyled even though new tabs default to PTY; reach it with `--panel agents-chat` (`I-03`).
6. **No skills attached**: the Skills chip shows "Skills" without a count badge (unchanged logic; only the look changes).
7. **Disabled Approve** (decision in flight): 50 % opacity on fill and label, still `TEXT_ON_BRAND`.
8. **Shell and DESIGN:OS help tabs** have no provider identity today (`#A1A1AA`, `:4302`): they use `ICON_TERTIARY` (rule 10).
9. **Light-on-dark labels on presence colours** (`AGENT_LABEL_TEXT`) change to the dark label `#0A0A0A` by the contract; `AGENT_LABEL_TEXT` on each of `AGENT_1..4` is not yet a row of the contract's C-1 table: the PR computes it with the C-1 formula (4.5:1) until W1 adds the rows.
10. **Image-job states** (queued, working, failed): the chip uses `Loading/Inline` for queued and working, `TEXT_ERROR` for failed; Expand and Erase dialogs keep their sizes except buttons to `CONTROL_XS`.

## 9. Non-goals

- Terminal content: ANSI palette, cell font and size, cursor shape and colour, cell selection, search match fill.
- Canvas presence colours (`AGENT_1..4`), the info family (`STATUS_INFO_*`), canvas overlays drawn by `canvas/draw.rs` (not mine).
- Any behaviour, keyboard, PTY, approval, queue, session or accessibility change.
- `cmdk.rs` (J) and `panels/check_designs.rs` (K) edits: J's `cmdk.rs` keeps reading the aliases; K re-points `panels/check_designs.rs` itself.
- Light mode, icons (R6 keeps the set; only colour changes), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name), registered with one `mod agent_chrome;` line in the module list of `crates/designos-app/src/shell/tests.rs` (`:1114-1180`). It uses the shared census helper `chrome_colour_sites(src: &str) -> Vec<usize>` (new name) from `shell/tests/chrome_census.rs` (new name, spec H section 10 defines it; whichever of H–K lands first adds it) and the existing recorded-rect hook (`widgets::record`, `widgets/mod.rs:247`), read with `Sim::rect_of` (`shell/tests.rs:193`) or `ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name))))` as `agent_picker.rs:611` does.

| Test | Asserts | Red at `205c72ef` |
|---|---|---|
| `agent_sources_paint_no_chrome_colour_literals` | `chrome_colour_sites` over the 20 files of section 2 returns only the 6 `terminal.rs` allowlist sites (`TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines, the match fill, 3 cursor lines; identified by their code text, not line numbers); 0 everywhere else | yes (71) |
| `terminal_palette_aliases_contract_roles` | `TERM_BG_VIEWPORT == theme::BG`, `TERM_BG_BAR == theme::BG_PRIMARY`, `TERM_BG_ELEVATED == theme::BG_SECONDARY`, `TERM_BG_HOVER == theme::BG_HOVER`, `TERM_BORDER == theme::BORDER`, `TERM_ORANGE == theme::BORDER_SELECTED`, terminal `STATUS_INFO_* == theme::STATUS_INFO_*`; `STATUS_OK_BG/BORDER/TEXT == theme::BG_SUCCESS_PRIMARY / UTILITY_GREEN_200 / UTILITY_GREEN_700`, and the same for `WARN` (yellow) and `BAD` (red) | yes |
| `terminal_content_colours_unchanged` | `TERM_FG_DEFAULT == #E4E4E7`, `TERM_SELECTION_BG == #5A7898`; existing ANSI pins `terminal.rs:6752-6754` still pass | no (guard) |
| `approval_card_height_follows_control_xs` | `approval_card_outer_height(Claude) == 120.0`, other presets `== 84.0` | yes (112 / 80) |
| `agents_header_controls_are_control_xs` | render the Agents panel (`--panel agents` fixture in the `Sim` harness); the recorded rects of `agents-runtime-add-btn` and `agents-header-skills` are `CONTROL_XS` high and lie inside the 36 bar; the active session tab stays 24 high (guard, R10.1); add a `widgets::record` call where a key is missing | yes (24 / 28) |
| `provider_marks_read_theme_constants` | `paint_orca_tab_status_and_provider` and `agent_open_flow::paint_session_row_indicators` take provider colours from `theme::PROVIDER_*` (source scan of `terminal.rs:4232-4309` and `agent_open_flow.rs:338-397`: no `Color32::from_rgb`) | yes (5 + 4 literals) |
| `agents_viewport_rect_unchanged` | at 1600×1088 the viewport rect equals the value captured before the change (L's baseline file) | no (guard, R1) |
| `brand_buttons_use_dark_label` | `kit::button_look(Hierarchy::Primary, State::Default).text == theme::TEXT_ON_BRAND` and no I file paints `Color32::WHITE` (covered by the census) | yes (C's look missing; WHITE at 4 sites) |
| `legacy_chat_panel_uses_contract_roles` | `--panel agents-chat` renders; census for `:5395-5538` region is 0 | yes (17) |

Existing tests that must stay green: `terminal.rs` test module (including `chat_header_draws_no_control_that_does_nothing` `:6559`, which asserts at `:6629` that the removed pop-out stays gone), `agent_picker.rs:865`, `:880`, `agent_turn_ledger.rs:1193`, `shell/tests/live_agent_edits.rs:119` (renders `agents`, `terminal`, `agents-chat`), `shell/tests/home.rs:483-488`. `terminal.rs:9612-9630` pins move with spec A, not here.

Commands (repo root): `cargo test -p designos-app --bin designos agent_chrome`, then `cargo test -p designos-app --bin designos terminal`, then `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z01–Z04, Z15, Z17 stay OK); `python3 scripts/check_spacing_relationships.py` (exit 0; lower the baseline with `--write-baseline` only if counts fell); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence** (before = commands below on a release build of the local `ec47da26` working tree, run 2026-10-25, so they still show the header pop-out that #864 removed; L re-captures the before-set at the branch base; after = the same commands on the PR branch; both sets go in the PR). Setup: `export PATH="$HOME/.cargo/bin:$PATH"`; seed a library once with `cargo run -q --release -p designos-app --bin designos -- --seed-library --library "$LIB"` (exit 0); `FILE=$(ls "$LIB"/projects/*/files/*.designos | sed -n 3p)` (the "Design system" file; in this session it resolved to `…/files/18e1b21f9d7260280003.designos`, the seed creates one project with 4 files). Each run: `cargo run -q --release -p designos-app --bin designos -- "$FILE" <args> --screenshot <out>.png --size 1600x1088 --no-toast`.

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

1. The census command of section 2 prints `chrome_sites=0` for every I file except `terminal.rs`, which prints exactly the 6 allowlisted content sites (`TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines, match fill, 3 cursor lines).
2. The 9 tests above pass; the existing tests listed above pass.
3. `CONTROL_DENSE` and `CONTROL_STANDARD` references in I's files: 0 (`grep -rnwE 'CONTROL_(DENSE|STANDARD)' <I files>` prints nothing).
4. `I-01`..`I-13` after-shots show: no blue chip outside the info family, no white label on orange, session tabs still 24 inside the 36 bar (R10.1), and the viewport at the same position as before (L's layout gate passes for the Agents captures).
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/check_spacing_relationships.py` exit 0.
6. The PR lists the re-anchored line numbers and the before/after census.
7. **Contrast**: every text and icon pair I paints passes `specs/token-contract.md` section (f) "Contrast check C-1" (its python block exits 0). I's pairs there: C-1.1, C-1.2, C-1.3, C-1.4, C-1.5, C-1.7, C-1.8, C-1.9, C-1.10, C-1.12, C-1.13, C-1.14, C-1.15, C-1.16, C-1.17, C-1.18, C-1.19, C-1.20, C-1.21, C-1.22, C-1.23, C-1.24, C-1.25, C-1.26, C-1.27, C-1.28, C-1.29. Pairs I paints that C-1 does not list yet (W1 adds them; until then the PR computes them with the same formula): `AGENT_LABEL_TEXT` on `AGENT_1..4` (4.5), `TEXT_QUATERNARY` on `BG_PRIMARY` (other tabs, 4.5), `STATUS_INFO_TEXT` on `STATUS_INFO_BG` over `BG` (Working badge, 4.5), `PROVIDER_*` and `ICON_TERTIARY` marks on `BG_PRIMARY` (3.0), `TERM_FG_DEFAULT`-on-`TERM_BG_VIEWPORT` inverse video (content, information only).

## 11. Performance budgets

No new allocation per frame: aliases are `const`, C's looks are pure functions over constants. The viewport paint path (`render_vt100_viewport`) changes no code except the scrollbar and search-bar colours. Idle CPU stays 0.00 % (no new `request_repaint`). Measure frame time with the Agents panel open on the 1600×1088 window before and after; it must not rise by more than 0.5 ms (the change is colour and a few rect sizes).

## 12. Dependencies, ownership and build order

**Build order.** L captures the Agents baseline (`I-01`..`I-13` are also L inputs) → A adds the contract constants (including N20–N38) → B lands sizes and renames (`CONTROL_ROW` → `CONTROL_MD`) → C lands `widgets/kit/` → this spec. If C is not merged, do not hand-paint anatomy in I's files: wait, because C retires duplicates in `agent_approval_card.rs:137-160`, `:168-200`, `:219-249`, `agent_selection_chip.rs:160-200`, `image_jobs.rs:1241-1262`, `agent_sample_prompts.rs:250-262` (C section 5.1).

**Technical decisions recorded here** (the owner may reverse): (1) terminal palette names stay as aliases instead of being renamed (`cmdk.rs` keeps compiling; the aliases are deleted only after their last outside reader moves); (2) context strip and receipt row stay 24 (status strips, not rows or buttons); (3) the terminal search-match fill stays content (rule 2); (4) the Skills count badge becomes gray (blue stays reserved for the info family); (5) plain-text status colours in I's own files use `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR`, badge text the `UTILITY_*_700` aliases.

**File ownership across H–K** (identical table in I, J and K; H gives the same assignments with a note per file):

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/leave.rs` and `home/unsaved.rs` (new on `origin/main`, no painting), `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs` (26 files at `205c72ef`, plus any `agent_*` module added later), `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs` (controller ruling), `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `panels/check_designs.rs` (controller ruling), `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests/chrome_census.rs` (new; the `chrome_colour_sites` helper of spec H section 10; whichever of H–K lands first adds it exactly as H defines it) | H |

Not mine (D–G or the kit specs): `theme.rs`, `design/*`, `scripts/*` (A, B); `widgets/*` and `widgets/kit/*` (C, except `widgets/picker.rs`, F); `tabs.rs`, `shell.rs` (toast painted by D with J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs` (including the command palette `:901-1079`), `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, `panels/properties.rs`, `panels/inspect.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G; `specs.rs:2895`, `:3195` call I's legacy chat entry, no edit needed). `shell/tests.rs` is L's (`Sim`, `:69-146`; `Sim::rect_of` `:193`): each H–K PR only appends one `mod` line to its module list (`:1114-1180`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Unassigned (flag to the controller, recommended owner in brackets): `canvas/draw.rs` [D or L; uses `theme::STATUS_INFO_*`], `decks/ui.rs` [D], `board/*` [D], `icons.rs` [C], `lint.rs` [K].

**Open PRs touching I files** (`gh pr list --state open`, 2026-10-25; file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): #1051 (`agent_session_failure.rs`), #1070 (`agent_turn_status.rs`, `terminal.rs`), #1075 (`agent_session_files.rs`, `terminal.rs`), #1081 (new `agent_cli_writers_tests.rs`), #1084 (`agent_open_flow.rs`, `agent_sample_prompts.rs`, `terminal.rs`), #1085 (`agent_composer.rs`, `agent_permissions.rs`, `agent_session_files.rs`, new `agent_approval_channel.rs`, `agent_hook_outputs.rs`); stale #765, #768, #770 (`mergeable: CONFLICTING`) also touch agent files, `terminal.rs` and `image_jobs.rs`. Land or rebase these before this spec's PR; re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor panels: text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius md 8; dialogs: text-sm 14/20, button sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- R8.1 placeholder `#737373` (named exception) · R8.2 selected `#262626` · R8.3 `TEXT_BRAND` `#F97316` · R8.5 floating chrome radius 16, items 8 at inset 8 · R8.6 button xs label text-sm 14 semibold (buttons only) · R8.7 destructive hover `#B91C1C`.
- R9: the live kit may be read with figma-agent, read-only; the permissions modal radius 16 rests on that check (kit Modals, radius-2xl 16).
- R10.1: "Editor tab heights stay as today: D file tabs 26 in the 32 bar, I session tabs 24, F Inspector segmented strip 24 (R1 wins over R5 here)".
- R10.7: unchecked checkbox and radio border = kit 1 px `BORDER_STRONG` `#404040`.
- Controller defaults (21:35 sweep): terminal ANSI palette is content; chrome around it adopts Untitled; canvas presence colours keep their values; "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2; provider colours are identity content (contract G-2) — I OQ1"; "Dialog footer buttons md 40".
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions (all decided)

None remain for I.

1. **Provider identity colours** — decided: kept as identity content, read from `theme::PROVIDER_*` (rule 10). Source: controller default "provider colours are identity content (contract G-2) — I OQ1"; contract N34–N37 (Grok becomes Neutral-200 `#E5E5E5`).
2. **Status badge look** — decided: Untitled tinted badges (rules 3 and 6). Source: controller default "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2".
3. **Session tab height** — decided: stays 24 (rule 5). Source: R10.1 "Editor tab heights stay as today: D file tabs 26 in the 32 bar, I session tabs 24, F Inspector segmented strip 24 (R1 wins over R5 here)" (owner, board 3: "A · Giữ như hiện tại").
