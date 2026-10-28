# I — Surface: Agents panel, terminal and agent cards (Untitled UI dark)

| Field | Value |
|---|---|
| Issue | #1144 (epic #1118, spec key I) |
| Size | L (about 2–3 days: 1 file with 50 chrome colour sites, 4 files with 1–10 sites, about 25 files that only re-point sizes; no behaviour change) |
| Depends on | A, B, C and L (L's hermetic capture mode and `Sim` seams, tests H1–H11, land before I's `Sim` tests and captures) |
| Unblocks | M (docs and screenshot refresh of the Agents pages) |
| Touches | `crates/designos-app/src/terminal.rs` (palette `:30-57`, tab provider marks `:4731-4808`, `show_left` `:5678` with its body in `show_left_with_doc` `:5738-6233`, `paint_turn_receipt_row` `:6242`, `render_vt100_viewport` `:6303-6950`); `agent_approval_card.rs`; `agent_composer.rs`; `agent_mention_popup.rs`; `agent_open_flow.rs`; `agent_picker.rs`; `agent_sample_prompts.rs`; `agent_selection_chip.rs`; `agent_session_failure.rs`; `agent_start_failure.rs`; `agent_stop.rs`; `agent_turn_card.rs`; `agent_presence.rs`; `agent_busy_frames.rs`; `canvas/agent_banner.rs`; `canvas/agent_changes_chip.rs`; `canvas/agent_overlay.rs`; `skills/mod.rs`; `skills/pills.rs`; `image_jobs.rs`; tests: new `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name) |

**Code baseline.** Every `file:line` below is at `origin/main@8dfaee93` (v0.7.0 merged), read with `git show 8dfaee93:<path>` (no checkout). The earlier drafts cited `ec47da26` and then `205c72ef`; fix round 3 (2026-10-28) moved every citation by a line diff of `205c72ef..8dfaee93`: a line that did not change keeps its text and gets its new number, and a line whose code changed was placed by code text (marked "placed by hand": the cell and cursor lines of the viewport, `agent_turn_card.rs:294`, and the scrollbar, which is gone). The cross-check: the census at `8dfaee93` (section 2) prints exactly the mapped lines. Release 0.6.1 (#1017, #864) had already removed the terminal dock and the header pop-out (`terminal.rs:7477` asserts the pop-out is gone). Re-anchor every line against the branch HEAD before editing (search for the quoted code, not the number) and re-run the census of section 2; report drift in the PR. Colour names are the token contract's (`specs/token-contract.md`, spec A and B) and are Studio's names (R14); Untitled role names are references only.

**Drift note (`205c72ef` to `8dfaee93`).** `terminal.rs` 12,936 to 14,154 lines (+1,784 / −566; the panel code from `show_left` `:5678` to the legacy chat `:6233` only moved by +687 to +695; the viewport changed), `agent_turn_card.rs` 380 to 585, `agent_approval_card.rs` 307 to 344, `agent_stop.rs` +26, `agent_composer.rs` +64, `agent_session_failure.rs` +24, `image_jobs.rs` +97 (session-file plumbing, no paint); `agent_open_flow.rs`, `agent_picker.rs`, `agent_selection_chip.rs`, `agent_start_failure.rs`, `agent_sample_prompts.rs`, `agent_mention_popup.rs`, `skills/mod.rs` and the three `canvas/agent_*.rs` files did not change. Top-level `agent_*.rs` files: 26 to 30 (`agent_approval_channel.rs`, `agent_hook_outputs.rs`, `agent_project_config.rs` and a test file; none paints). Census: 70 sites (was 71): `terminal.rs` 50, because the old literal scrollbar thumb is gone. What changed that I must style: (1) **Approval card** (#656, #1195): `approval_banner_target()` shows the card for the first tab with a pending approval (the "N" of "1 of N" counts every tab); Claude on the active tab keeps Approve / Deny; Codex, Grok, Gemini, Antigravity and Claude on another tab read "<CLI> needs approval in its terminal" with **Show prompt** (in that tab's terminal) or **Open tab** (another tab, `CardAction::OpenTab(tab_id)`); keyboard focus on a card button draws a `BORDER_SELECTED` stroke today. (2) **Stop** is drawn only for a stoppable tab (`is_tab_stoppable`: a CLI with an interrupt row; Gemini CLI has none). (3) **Turn card** (#1173): the changed-layer list is a scroll area with a pinned Show all / Show less toggle and Revert / Keep row, and the card height is capped to the panel. (4) **Terminal scrollbar** (#1171): a 24 px interactive track with a 4 px thumb in `TEXT_TERTIARY` / `TEXT_SECONDARY` (already theme roles, no literal), records `term-scrollbar-track` and `-thumb`; the cell loop is wrapped in `with_scrollback_safe_screen` (re-indented, same colours). (5) Gemini CLI: its dialog is read from the screen (`gemini_dialog.rs`, `gemini_screen.rs`, `gemini_status_hooks.rs`, no paint) and the composer treats an open Gemini dialog as waiting (`agent_composer.rs`, `gemini_dialog_open`). (6) A new test, `shell/tests/agent_turn_card_scroll.rs`, finds the terminal and viewport rects by the `TERM_BG_VIEWPORT` fill; after rule 3's alias that fill is `BG` and also the window and panel fill, so I re-keys that lookup to recorded rects in the same PR (listed in section 10). Uncovered surfaces: see section 12.

## 1. Summary and user story

As a designer running coding agents inside Studio, I want the Agents panel, its terminal and every agent card to look like the rest of the Untitled-styled app, so the agent area no longer reads as a separate product with its own greys, blue chips and white-on-orange buttons.

The panel keeps its regions, widths and order (R1). Colours move to the token contract's chrome constants; buttons, tabs, chips, badges, popovers and dialogs take spec C's widgets; sizes follow R5 (editor panel: text-xs rows and labels, button xs 32, radius-md 8). What the terminal prints (ANSI colours, cell font, cursor, cell selection) is content and stays as it is (controller default).

## 2. Current state

FACT unless labelled. Captures of today (`I-01`..`I-13`) come from L's hermetic baseline (section 10); the spec writer's own earlier captures were withdrawn as not hermetic.

**Where the panel is drawn.** `terminal::show_left` (`terminal.rs:5678`, body in `show_left_with_doc` `:5738`) is called for the left Agents panel; when the active tab runs a PTY CLI it paints the bar, tabs and viewport (`:5757-6080`); otherwise it paints the legacy chat panel (`:6082-6233`, reachable through `agent_composer.rs:1079` and the Dev chat entry points `specs.rs:2895`, `:3195`; the default for a new tab is PTY, `terminal.rs:1765`). The viewport is `render_vt100_viewport` (`:6303`), called once at `:6044`. There is no bottom dock at `8dfaee93` (removed by #864).

**Inventory (geometry today).**

| Element | Code | Today |
|---|---|---|
| Header bar | `terminal.rs:5761` (`bar_h = 36.0`), fill `:5776` `TERM_BG_BAR`; stop controls drawn from the bar's right edge `:5779` | 36 high |
| Stop controls | `agent_stop.rs:383` (`paint_header_stop_controls`; shown only when `is_tab_stoppable` `:132` holds, so a Gemini tab has none), `btn_h = CONTROL_STANDARD` `:393`, width `CONTROL_ROW + SPACE_SECTION` `:399`, fill `BG_SECONDARY` (hover `BG_HOVER`) + `BORDER` `:402-404`, record `agents-header-stop-btn` `:412` | 28 high, 64 wide |
| Session tabs | `:5804` (24 high), widths `:5795-5800`, close × `:5805` (12), active underline `TERM_ORANGE` `:5838`, provider mark `paint_orca_tab_status_and_provider` `:4731-4808` (provider colours `:4782-4801`) | 24 |
| Sessions button | `agent_open_flow.rs:414-420` (`render_sessions_button_in_bar`: ghost button between the tabs and `+`, height `CONTROL_DENSE` centred in the bar, width `SESSIONS_BTN_W`), record `agents-sessions-btn` `:439` | 24 |
| Add `+` | `:5887` (28×24), glyph `Color32::WHITE` `:5897`, record `agents-runtime-add-btn` `:5902` | 28×24 |
| Skills chip (header) | `:5918-5923` (`CONTROL_STANDARD`; `paint_chip(…, "agents-header-skills")` `:5923`), painter `skills/mod.rs:779` (`paint_chip`), colours `:789`, `:791` | 28 |
| Context strip | `:5925-5929` (`ctx_h = CONTROL_DENSE`), fill `BG_PRIMARY`, hairline `BORDER`; selection chip `agent_selection_chip.rs:160` | 24 |
| Turn receipt row | `:6031` (`receipt_h = 24.0`), painter `:6242` | 24 |
| Runtime dropdown / sessions / skills popovers | `:6069-6071` → `agent_picker.rs:467`; `:6072-6074` → `agent_open_flow.rs:455`; `:6075` → `skills/mod.rs:816` (fill `#1F2024` `:836`, stroke `#3E3E46` `:837`, title `WHITE` `:848`, checkbox `:925`, `:934`, name `WHITE` `:954`) | menus and popovers |
| Legacy chat panel | `:6082-6233`: panel `#2C2C2C`, header 48 (`:6085-6141`: Back `:6092`, AI `:6125`), runtime bar 28 (`:6145-6199`) | 17 literal sites |
| Viewport | `:6336` fill `TERM_BG_VIEWPORT`; cell font `FontId::new(11.5, Monospace)` `:6685`; inverse video `:6734-6737`; cell selection `TERM_SELECTION_BG` `:6747`; search match `#5C4A00` `:6749`; cursor `:6825`, `:6829`, `:6841`; scrollbar (#1171; interactive 24 px track, 4 px thumb in `TEXT_TERTIARY` / `TEXT_SECONDARY` theme roles, `:6845-6874`; no literal); search bar 280×28 `#171717` `:6885-6898` (fill `:6893`); search text `WHITE` `:6946` | content + chrome |
| Approval card | `agent_approval_card.rs:24-29` (`approval_card_outer_height`: Claude 104, others 72, plus 2 × `SPACE_ROW`), `paint_approval_card_for_tab` `:51` (the first tab with a pending approval: Claude on the active tab = Approve / Deny; Codex, Grok, Gemini, Antigravity or Claude on another tab = one of Show prompt `:149-150` or Open tab `:162`), header badge 24 `:79-87` (`STATUS_WARN_BG/BORDER`), summary row 28 `:119`, action row 28 `:131`, buttons `paint_card_button` `:172` (28 high, min width 40 `:183-184`; keyboard focus draws a `BORDER_SELECTED` stroke `:193`) | Claude 112 outer, others 80 outer |
| Permissions modal | `agent_approval_card.rs:205` (`paint_permissions_modal`, 360 wide), scrim `SHADOW_COLOR_STRONG` `:229`, `menu_panel` (shadow and menu fill) `:234`, then fill `BG_SECONDARY` + `BORDER` `:235-236`, option row `CONTROL_ROW + CONTROL_STANDARD` = 68 `:211`, options `:256-286` | dialog |
| Turn cards | `agent_turn_card.rs:153`, `:176`, `:183`, `:258`, `:294` (placed by hand); #1173: the changed-layer list is a scroll area (rows `CONTROL_DENSE` 24), a Show all / Show less toggle (24) and the Revert / Keep row (`CONTROL_STANDARD` 28) stay pinned, and the card height is capped to the panel | theme constants only; card 8 inner padding |
| Mention popup | `agent_mention_popup.rs:283` (popup; `elevation_shadow` `:332`, fill `BG_MENU` + `BORDER_MENU` `:333-334`), `:444` (mention button) | theme constants only |
| Composer | `agent_composer.rs:345` (`composer_box_height` = `CONTROL_ROW` × 2 + extra lines × `LINE_BODY` + chips `CONTROL_DENSE + SPACE_ROW`), `show_composer` `:847`, uses of the terminal palette `:1141`, `:1152` | 80 + lines |
| Session failure banner / start failure / sample prompts | `agent_session_failure.rs:876`; `agent_start_failure.rs:239` (buttons `CONTROL_STANDARD` `:234`, computed mix `:408`); `agent_sample_prompts.rs:250`, `:378` | theme constants |
| Open-session flow | `agent_open_flow.rs:338`; status dots `:347` `#EAB308`, `:359` `#F97316`, `:364` `#10B981`; provider badges `:382-397` | 7 literal sites |
| Runtime picker | `agent_picker.rs:260`, `:283`, `:295`, `:301`, `:467`, `:622`; "Save" label `Color32::WHITE` on `BG_BRAND` `:711` | 1 literal site |
| AI image jobs (expand, erase) | `image_jobs.rs:1215`, `:1288`, `:1294`, `:1555`; popovers `show_expand_popover` `:1334` and `show_erase_popover` `:1440`, each drawn with `menu_panel` (`:1359`, `:1468`); "Expand" and "Erase" labels `Color32::WHITE` on `BG_BRAND` `:1426`, `:1537` | 2 literal sites |
| Canvas agent banner / changes chip / overlay | `canvas/agent_banner.rs:22`; `canvas/agent_changes_chip.rs:388` (computed colour `:373`); `canvas/agent_overlay.rs:24` (computed `:20`) | theme constants; presence colours `AGENT_1..4` |

**Terminal palette** (`terminal.rs`, its own constants, consumed by other files):

| Constant | Line | Value | Uses outside its line (`grep -rnw <NAME> crates/designos-app/src`, minus the `pub const` line) |
|---|---|---|---|
| `TERM_BG_VIEWPORT` | `:31` | `#0E0E12` | 7 (terminal 3, `cmdk.rs` 2: import and `:579`, and the new test `shell/tests/agent_turn_card_scroll.rs` 2) |
| `TERM_BG_BAR` | `:32` | `#16171B` | 2 |
| `TERM_BG_ELEVATED` | `:33` | `#1F2024` | 6 |
| `TERM_BG_HOVER` | `:34` | `#26272C` | 14 |
| `TERM_BORDER` | `:35` | `#2A2B31` | 14 |
| `TERM_ORANGE` | `:36` | `#FF6A3D` | 1 (`:5838`, active tab underline) |
| `TERM_ORANGE_BG` | `:38` | `#361A12` | 0 (dead) |
| `TERM_FG_DEFAULT` | `:39` | `#E4E4E7` | 5 (all cell painting or tests: `:6734`, `:6815`, `:6957`, `:7582`, `:7584`) |
| `TERM_SELECTION_BG` | `:40` | `#5A7898` | 1 (`:6747`, cell selection) |
| `STATUS_OK_TEXT/BG/BORDER` | `:43-45` | `#4CC58B` / `#17302A` / `#2C5F4A` | text 12 |
| `STATUS_WARN_TEXT/BG/BORDER` | `:47-49` | `#E0A53C` / `#35291A` / `#5C4720` | text 20 |
| `STATUS_BAD_TEXT/BG/BORDER` | `:51-53` | `#FF7A8C` / `#3B1D23` / `#6A2E39` | text 13 |
| `STATUS_INFO_TEXT/BG/BORDER` | `:55-57` | `#8FB0FF` / `#1B2640` / `#2F4577` | text 17 (name collides with `theme::STATUS_INFO_*`, `theme.rs:576-578`, `#0D99FF`) |

Files that import them (`grep -rlw` on the names): `agent_approval_card.rs`, `agent_picker.rs`, `agent_composer.rs`, `skills/mod.rs` (I), `cmdk.rs` (J, `:14-16`, used `:579-625`), `panels/check_designs.rs` (K, `:7`, used `:81`, `:83`, `:84`, `:85`, `:511`; spec K rule 11 re-points it to theme colours, so after K it no longer imports them), `canvas/draw.rs` (D or L; uses the `theme::STATUS_INFO_*` names), and the test `shell/tests/agent_turn_card_scroll.rs` (new at v0.7.0; it identifies the terminal and viewport rects by their `TERM_BG_VIEWPORT` fill).

**Chrome colour census** (spec D's command, used unchanged by D–K; a "chrome site" is a `Color32` constructor with literal arguments, a `from_gray` / `from_black_alpha` / `from_white_alpha` call, or a named `Color32` other than `TRANSPARENT`, in non-test code as `scripts/rust_literal_scan.clean` strips it; "computed" is a constructor with non-literal arguments):

```bash
cd /Users/jangtrinh/Products/design-os-studio
# REV=8dfaee93 reads that commit with `git show` (no checkout); drop it to read your working tree
REV=8dfaee93 PYTHONDONTWRITEBYTECODE=1 python3 - terminal.rs agent_approval_card.rs agent_composer.rs agent_mention_popup.rs \
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

Result at `8dfaee93` (run 2026-10-28 with the block above, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `terminal.rs` | 67 | **50** | palette `:31-36`, `:38-40`, `:43-45`, `:47-49`, `:51-53`, `:55-57` (21); provider `:4782`, `:4786`, `:4790`, `:4797`, `:4801` (5); `+` glyph `:5897`; legacy chat `:6082`, `:6089`, `:6097`, `:6106`, `:6112`, `:6127`, `:6131`, `:6134`, `:6135`, `:6148`, `:6149`, `:6166`, `:6168`, `:6169`, `:6188`, `:6190`, `:6191` (17); match `:6749`; cursor `:6825`, `:6829`, `:6841`; search bar `:6893`; `:6946` | `:6962`, `:6990`, `:6998`, `:7002` (ANSI conversion, content) |
| `agent_open_flow.rs` | 71 | 7 | `:347`, `:359`, `:364`, `:382`, `:385`, `:393`, `:397` | — |
| `skills/mod.rs` | 75 | 10 | `:789`, `:791` (×3), `:836`, `:837`, `:848`, `:925`, `:934`, `:954` | — |
| `image_jobs.rs` | 52 | 2 | `:1426`, `:1537` | — |
| `agent_picker.rs` | 123 | 1 | `:711` | — |
| `canvas/agent_changes_chip.rs` | 31 | 0 | — | `:373` |
| `canvas/agent_overlay.rs` | 11 | 0 | — | `:20` |
| the other 13 files | 3–77 | 0 | — | — |

Total at `8dfaee93`: **70** chrome sites in I's files (71 at `205c72ef`: the literal scrollbar thumb is gone since #1171; 74 at `ec47da26`: the dock grip and the two pop-out icons, PTY header and legacy header, went with #864). (plan.md's `grep -c 'Color32::from_rgb('` counts include tests; the census above is the contract.)

**Gates today.** No gate counts colour literals. `.github/workflows/design-gates.yml` runs the design gates with `continue-on-error` (`:4-5`, `:21`), so only the macOS test, Clippy and Rustfmt jobs block a merge. `scripts/zero_fake_ui_gate.py` checks Agents invariants Z01–Z04, Z15 and Z17 on these files; `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (passed on the local `ec47da26` working tree, exit 0; not re-run at `8dfaee93`, which would need a checkout). Tests that pin terminal colours: `terminal.rs:7600-7602` (ANSI palette, content, unchanged) and `:10727-10745` (`theme_has_info_success_warning_in_token_parity`, pins `theme::INFO`, `SUCCESS`, `WARNING` and token JSON; spec A moves the `SUCCESS` and `WARNING` pins to `#22C55E` / `#EAB308`, token contract section (e)).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Every region keeps its x, width and order: header bar 36, session tabs 24 (R10.1), context strip 24, receipt row 24, legacy chat header 48 and runtime bar 28, the viewport, the composer. Heights change only where R5 raises a control inside a region (table 3, section 7). The viewport's top and bottom edges do not move at the same window size, so the PTY keeps the same columns and rows (cell font `11.5` monospace `:6685` is content and does not change).
2. **Terminal content stays.** ANSI colours (`vt100_color_to_egui`, `ansi_256_to_color32`, computed `:6962-7002`), `TERM_FG_DEFAULT`, `TERM_SELECTION_BG`, the cursor (`:6825`, `:6829`, `:6841`) and the search match fill (`:6749`) keep their values. They are the only colour literals left in `terminal.rs`: 6 allowlisted sites, matched by code text: the `TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines (`:39`, `:40`; their values have no contract constant), the match fill and the 3 cursor lines (section 10). Technical call: the match fill stays content because it sits under arbitrary ANSI glyph colours; re-tinting it would need a per-glyph contrast check that this spec does not add.
3. **Palette constants become zero-literal aliases** of contract constants (table 1, section 7). The status triples alias the contract's tinted badge roles (decided: controller default "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2"): `STATUS_OK_BG/BORDER/TEXT` = `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`, `STATUS_WARN_*` = `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`, `STATUS_BAD_*` = `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700`, `STATUS_INFO_*` = `theme::STATUS_INFO_TEXT/BG/BORDER` (kept info family). In I's own files a status colour used as plain text, not on a badge fill (invalid skill name, composer error `agent_composer.rs:1152`), uses `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR` instead of the alias. `cmdk.rs` (J) keeps compiling through the aliases and picks up the new values without edits; `panels/check_designs.rs` (K) stops importing them (K rule 11). The aliases stay `pub` until no other file reads them: never delete them in this PR.
4. **Buttons.** Every button in I's files is spec C's `Button/*` at `Size::Xs` (32), except the controls in the header tab bar (rule 5): Approve, Deny, Show prompt, Open tab, Save, Cancel, Expand, Erase, Retry, sample prompts; labels text-sm 14 semibold 600 (R8.6, buttons only). A filled brand button's label is `TEXT_ON_BRAND` (dark label, R4), never `Color32::WHITE` (`agent_picker.rs:711`, `image_jobs.rs:1426`, `:1537`, `terminal.rs:5897`). Stop takes C's `Button/Destructive` look (`BG_ERROR_SOLID`, hover `BG_ERROR_SOLID_HOVER` `#B91C1C` (R8.7), pressed `BG_ERROR_SOLID_PRESSED` (N38), label `TEXT_WHITE`) at the tab-bar height of rule 5.
5. **Tabs.** Session tabs keep today's height 24 (`:5804`), centred in the 36 bar (decided: R10.1 "Editor tab heights stay as today: … I session tabs 24 … (R1 wins over R5 here)"); they take C's `Tabs/Pill` colours and radius at that height. The active tab keeps its underline, drawn in `BORDER_SELECTED` (border-brand). Close × stays a 12 px glyph inside a hit box of at least `TARGET_MIN` 24. **Controls in the tab bar match its tabs** (contract (d); plan.md C-K: "agent header "Sessions", `+`, Skills, Stop stay 24"): Sessions (C `Button/Tertiary` look, `TARGET_MIN` 24, width `SESSIONS_BTN_W` unchanged), `+` (C `Button/Icon`, 28×24 box unchanged), the Skills chip (C `Tag/Chip` at `BADGE_MD` 24) and Stop (rule 4 look, `TARGET_MIN` 24, 64 wide; drawn only for a stoppable tab, `is_tab_stoppable`) are 24 high and share the tabs' vertical centre in the 36 bar. Their labels are text-xs 12 medium like the tab labels: R8.6's 14 px label belongs to C's button sizes (xs 32 and up), and none of these four is a C size. C draws its looks at the rect height I passes (listed for W2).
6. **Badges and chips.** Status pills (approval provider badge, needs-approval, working, done, failed) are C's `Badge/StatusCount` sm 22 in its status colours, each the contract's tinted triple (rule 3; N24–N32); in the approval card header the 22 badge is centred in today's 24 slot, so the card height does not change because of it. The selection chip, skills chip and image-job chip are C's `Tag/Chip` (height `BADGE_MD` 24). Skills count becomes a gray `Badge/StatusCount` (`UTILITY_NEUTRAL_50` / `UTILITY_NEUTRAL_200` / `UTILITY_NEUTRAL_700`, N21–N23; blue is reserved for the info family the contract keeps: agent Working).
7. **Popovers and menus.** Runtime dropdown, sessions popover, skills popover and mention popup use C's `popover_surface` / `menu_surface` (fill `BG_MENU`, stroke `BORDER_MENU`, radius `RADIUS_FRAME` 16 with item pills radius 8 at inset 8 (R8.5), no shadow: every kit dark shadow resolves to alpha 0). Rows are 32 (`MENU_ITEM_HEIGHT` after B). Per C T10 I deletes its own shadow and old-panel calls: `widgets::elevation_shadow` at `agent_mention_popup.rs:332` and `widgets::menu_panel` at `image_jobs.rs:1359` and `:1468` (the Expand and Erase popovers, now C `popover_surface`). The skills checkbox is C's `Checkbox/Box` (checked = `BG_BRAND` with a `TEXT_ON_BRAND` tick; unchecked 1 px `BORDER_STRONG` `#404040`, R10.7), not `STATUS_INFO_TEXT` blue.
8. **Dialogs.** The permissions modal (`agent_approval_card.rs:205`) is a dialog: C's `Dialog/Modal` with R5 dialog sizes (text-sm 14/20; footer buttons md 40 per the controller default "Dialog footer buttons md 40"), fill `BG`, **no border**, radius `RADIUS_FRAME` 16, scrim `BG_OVERLAY` (N33, `#262626` at 70 %) with no background blur (spec J decides Studio does not draw the kit's blur), all drawn by C's `kit::surface::modal_surface`, which replaces the scrim `SHADOW_COLOR_STRONG` (`:229`), the `menu_panel` call (`:234`) and the fill and stroke (`:235-236`). Kit receipt (read-only, R9: `figma-agent context 4057:415359 --dir <scratch>`, fileKey `VEffelx01mDl5BJJanwxja`, Modals page `172:4293`, 2026-10-25): `Modal` frame `4057:415360` binds radius-2xl 16 (variable `6298:46481`), fill bg-primary, no stroke; its `Background overlay` `4896:411057` is bg-overlay at layer opacity 0.7 with a background blur (plan.md C-K: "dialogs/modals = kit Modals"); its options are C's `Radio/Card` (unselected border `BORDER_STRONG`, R10.7).
9. **Focus.** Every focusable control in I's files shows C's focus ring (`FOCUS_RING`, 2 px, 2 px gap) on keyboard focus (the approval card's `BORDER_SELECTED` focus stroke, `agent_approval_card.rs:193`, is replaced by it); the terminal viewport's own focus state (caret visible) is unchanged.
10. **Agent identity colours.** Canvas presence colours (`AGENT_1..4`, `AGENT_COLORS`) and the info family (`STATUS_INFO_*`) keep their values (contract (e)). Provider marks on tabs (`terminal.rs:4781-4801`) and in the open flow (`agent_open_flow.rs:382-397`) are identity content (decided: controller default "provider colours are identity content (contract G-2) — I OQ1"): both files read `theme::PROVIDER_CLAUDE` `#D97757`, `PROVIDER_CODEX` `#FAFAFA` (Codex and OpenCode), `PROVIDER_GROK` `#E5E5E5` (was `#E4E4E7`), `PROVIDER_GEMINI` `#60A5FA` (Antigravity and Gemini) (contract N34–N37); Shell and DESIGN:OS help marks (`#A1A1AA` today, `:4801`) use `ICON_TERTIARY`.
11. **No behaviour change.** Clicks, keyboard shortcuts, PTY input, approval decisions, queue order and accessibility labels (`agent_a11y.rs`) are untouched. Every `widgets::record` key keeps its name.

## 4. Data model / schema changes

None. No document, settings or session field changes.

## 5. Contracts

- **Colour constants consumed** (token contract (a); names exactly as there): `BG`, `BG_PRIMARY`, `BG_SECONDARY`, `BG_SECONDARY_HOVER` (new in the contract), `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BG_ERROR_SOLID`, `BG_ERROR_SOLID_HOVER` (N20, R8.7), `BG_ERROR_SOLID_PRESSED` (N38), `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_SELECTED`, `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_BRAND`, `TEXT_BRAND`, `TEXT_ERROR`, `TEXT_WARNING`, `TEXT_SUCCESS`, `TEXT_WHITE`, `TEXT_SECONDARY_HOVER`, `ICON`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `ICON_BRAND_PRIMARY` (N14, open-flow waiting dot), `DANGER`, `WARNING`, `SUCCESS`, `INFO` (open-flow working spinner), `STATUS_INFO_TEXT/BG/BORDER`, `FOCUS_RING`, `VAR_SCROLL_THUMB` (scrollbar thumb role), `AGENT_1..4`, `AGENT_LABEL_TEXT`.
- **Contract rows from the W1 additions** (no open gaps): tinted status triples `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`, `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`, `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700` (N24–N32, also C's `Badge/StatusCount` status colours); gray count badge `UTILITY_NEUTRAL_50` / `UTILITY_NEUTRAL_200` / `UTILITY_NEUTRAL_700` (N21–N23); provider marks `PROVIDER_CLAUDE`, `PROVIDER_CODEX`, `PROVIDER_GROK`, `PROVIDER_GEMINI` (N34–N37, identity content, in `theme.rs` so the colour-literal gate does not count them); modal scrim `BG_OVERLAY` (N33); tooltips follow the contract's tooltip rule (`BG_PRIMARY_SOLID` fill, title `TEXT` `#FAFAFA`, supporting text `TEXT_SECONDARY`) through C.
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32, `CONTROL_SM` 36, `CONTROL_MD` 40 (renamed from `CONTROL_ROW`; permissions-modal footer), `BADGE_SM` 22 (status badges), `BADGE_MD` 24 (chips, the header Skills chip), `TARGET_MIN` 24 (Sessions and Stop in the tab bar, close hit box), `MENU_ITEM_HEIGHT` 32, `RADIUS_CONTROL` 8, `RADIUS_FRAME` 16 (popovers, permissions modal), `LINE_BODY` 18, `FONT_SIZE_BODY` / `FONT_SIZE_CAPTION` 12, `TARGET_MIN` 24, spacing ladder unchanged. `CONTROL_DENSE` and `CONTROL_STANDARD` are legacy: every site in I's files is re-pointed (count must fall, spec B's ratchet).
- **Widgets consumed** (spec C, `widgets/kit/`, all new names there): `kit::button` with `Hierarchy::{Primary, Secondary, Tertiary, Destructive}` and `Size::Xs`; `Button/Icon` (`+`, close); `Tabs/Pill` (`kit::tabs::tab`); `Tag/Chip`, `Badge/StatusCount`, `status_dot` (`kit::badge`); `Loading/Inline` (`kit::feedback::loading_ring`, image-job and selection chips); `Input/Search` (terminal search bar); `Menu/Panel`, `Popover/Panel` (`kit::surface::menu_surface`, `popover_surface`); `Checkbox/Box`, `Radio/Card` (`kit::choice`); `Dialog/Modal` (`kit::surface::modal_surface`); `EmptyState/Default` (sample prompts, `agent_sample_prompts.rs:250-262`, C lists it as a duplicate D–K retire).
- **Rust surface.** No public signature changes. `approval_card_outer_height` keeps its signature; its value changes (table 3). The terminal palette names stay `pub` (rule 3). New local constant `TERMINAL_RECEIPT_H = 24.0` (new name) replaces the literal at `:6031`.

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system (`design/principles.md`, `design/soul.md`, `ui ds context --dir .`) and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (the Studio constant is the name that goes into the store and the code, R14; the Untitled role in brackets is a reference only; hex is the contract's dark value):

| Element | Today | Studio constant (contract; Untitled reference in brackets) | Hex |
|---|---|---|---|
| Viewport and panel background | `TERM_BG_VIEWPORT #0E0E12` | `TERM_BG_VIEWPORT = theme::BG` (bg-primary) | `#0A0A0A` |
| Header bar, context strip | `TERM_BG_BAR #16171B`, `BG_PRIMARY` | `TERM_BG_BAR = theme::BG_PRIMARY` (bg-primary) | `#0A0A0A` |
| Cards on the panel (approval, turn, failure banner, receipt) | `TERM_BG_ELEVATED #1F2024` | `TERM_BG_ELEVATED = theme::BG_SECONDARY` (bg-secondary) | `#171717` |
| Hover on the bar / on a card | `TERM_BG_HOVER #26272C` | `TERM_BG_HOVER = theme::BG_HOVER` (bg-primary_hover); on a `BG_SECONDARY` card use `BG_SECONDARY_HOVER` | `#171717` / `#262626` |
| Hairlines, card outlines | `TERM_BORDER #2A2B31` | `TERM_BORDER = theme::BORDER` (border-secondary); fields and secondary buttons get `BORDER_STRONG` from C | `#262626` / `#404040` |
| Active tab underline | `TERM_ORANGE #FF6A3D` | `TERM_ORANGE = theme::BORDER_SELECTED` (border-brand) | `#FB923C` |
| Dead brand tint | `TERM_ORANGE_BG #361A12` | deleted (0 uses) | — |
| Tab label current / other | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_QUATERNARY` (kit non-current tab text) | `#FAFAFA` / `#A3A3A3` |
| `+`, close glyphs | `Color32::WHITE` (`:5897`) | `ICON_TERTIARY`, hover `ICON_SECONDARY` (C's `Button/Icon`) | `#A3A3A3` / `#D4D4D4` |
| Provider marks (tabs, open flow) | `#D97757`, `#FAFAFA`, `#E4E4E7`, `#60A5FA`, `#A1A1AA` literals | `PROVIDER_CLAUDE`, `PROVIDER_CODEX`, `PROVIDER_GROK`, `PROVIDER_GEMINI`; Shell and help `ICON_TERTIARY` | `#D97757`, `#FAFAFA`, `#E5E5E5`, `#60A5FA`; `#A3A3A3` |
| Status badge success / warning / error (fill / border / text) | `STATUS_OK_*`, `STATUS_WARN_*`, `STATUS_BAD_*` | `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`; `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`; `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700` | `#052E16` / `#166534` / `#86EFAC`; `#422006` / `#854D0E` / `#FDE047`; `#450A0A` / `#991B1B` / `#FCA5A5` |
| Status badge info (agent Working) | terminal `STATUS_INFO_* #8FB0FF/#1B2640/#2F4577` | aliases of `theme::STATUS_INFO_TEXT/BG/BORDER` (kept info family) | `#0D99FF`, α 0x24, α 0x59 |
| Open-flow status: working spinner / waiting dot / done dot (`agent_open_flow.rs:347`, `:359`, `:364`) | `#EAB308`, `#F97316`, `#10B981` | `INFO` (one status, one colour: the tab's Working state is the info family, contract row 101 `STATUS_INFO_TEXT` "agent Working badge"), `ICON_BRAND_PRIMARY` (N14, fg-brand-primary: a mark, not text), `SUCCESS` | `#0D99FF`, `#F97316`, `#22C55E` |
| Brand button (Approve, Save, Expand, Erase, Run) | `BG_BRAND` + `Color32::WHITE` | `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` + `TEXT_ON_BRAND` | `#E36323` / `#F97316` / `#EA580C` + `#0A0A0A` |
| Secondary button (Deny, Cancel) | `BG_TERTIARY` / `BG_HOVER` + `BORDER` | C's `Button/Secondary`: `BG` + `BORDER_STRONG`, label `TEXT_SECONDARY`; hover `BG_HOVER`, label `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Stop (destructive) | `BG_SECONDARY` / `BG_HOVER` + `BORDER` (`agent_stop.rs:402-404`) | C's `Button/Destructive` look at 24 (rule 5): `BG_ERROR_SOLID`, hover `BG_ERROR_SOLID_HOVER` (R8.7), pressed `BG_ERROR_SOLID_PRESSED`, label `TEXT_WHITE` | `#DC2626` / `#B91C1C` / `#991B1B` + `#FFFFFF` |
| Sessions ghost button | theme constants (`agent_open_flow.rs:414-439`) | C's `Button/Tertiary` look at 24: no fill, label `TEXT_SECONDARY`; hover `BG_HOVER`, label `TEXT_SECONDARY_HOVER` | —, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Skills chip, selection chip, image-job chip | `TERM_BG_HOVER` + `TERM_BORDER` + `WHITE`; `#28282C` / `#3C3C42` / `#B3B3B3` | C's `Tag/Chip`: `BG` + `BORDER_STRONG`, text `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; count = gray `Badge/StatusCount` (`UTILITY_NEUTRAL_50` + `UTILITY_NEUTRAL_200` + `UTILITY_NEUTRAL_700`) | `#0A0A0A` + `#404040`, `#D4D4D4`, `#737373`; count `#171717` / `#404040` / `#D4D4D4` |
| Popovers (runtime, sessions, skills, mention, image-job Expand and Erase) | `#1F2024` + `#3E3E46`, title `WHITE`; `menu_panel` / `elevation_shadow` | C `popover_surface` / `menu_surface`: `BG_MENU` + `BORDER_MENU` at radius 16 (R8.5), no shadow, title `TEXT`, rows hover `BG_MENU_HOVER`, selected `BG_SELECTED` | `#0A0A0A` + `#262626`, `#FAFAFA`, `#171717`, `#262626` |
| Permissions modal | scrim `SHADOW_COLOR_STRONG`, `menu_panel`, then `BG_SECONDARY` + `BORDER` (`agent_approval_card.rs:229-236`) | C's `Dialog/Modal` (`modal_surface`): fill `BG`, no border (live kit Modal, receipt in rule 8), `RADIUS_FRAME` 16, scrim `BG_OVERLAY` (no blur) | `#0A0A0A`; `#262626` at 70 % |
| Skills checkbox | `STATUS_INFO_TEXT` / `#16171B`, tick `#0E0E12` | C's `Checkbox/Box`: checked `BG_BRAND` + tick `TEXT_ON_BRAND`; unchecked `BG` + `BORDER_STRONG` (R10.7) | `#E36323` + `#0A0A0A`; `#0A0A0A` + `#404040` |
| Invalid skill name | `STATUS_BAD_TEXT` | `TEXT_ERROR` | `#F87171` |
| Legacy chat panel | `#2C2C2C` and 17 more literals `:6082-6191` | same mapping as the PTY branch: panel `BG`, header and runtime bar `BG_PRIMARY`, hairlines `BORDER`, controls from C | as above |
| Terminal search bar | `#171717` fill `:6893`, text `WHITE` `:6946` | C's `Input/Search` xs: fill `BG` (kit Input bg-primary, plan.md C-F) + 1 px `BORDER_STRONG` (E-2), text `TEXT`, placeholder `TEXT_PLACEHOLDER` (E-1), focus `FOCUS_RING` | `#0A0A0A` + `#404040`, `#FAFAFA`, `#737373` |
| Scrollbar thumb (#1171) | already theme roles since v0.7.0 (`TEXT_TERTIARY` at rest, `TEXT_SECONDARY` on hover and drag, `:6845-6874`); the literal `:6072` is gone | unchanged (non-text on `BG`: 7.85:1 and 13.36:1); no `VAR_SCROLL_THUMB` re-point | `#A3A3A3` / `#D4D4D4` |
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
| Sessions button | `CONTROL_DENSE` 24 | `TARGET_MIN` 24 (rule 5, contract (d), plan.md C-K) | 0 |
| Add `+` | 28×24 | 28×24 (`Button/Icon` look; rule 5) | 0 |
| Skills chip (header) | `CONTROL_STANDARD` 28 | `BADGE_MD` 24 (`Tag/Chip`; rule 5) | −4 (inside the 36 bar; no layout move) |
| Stop controls | 28 high (`agent_stop.rs:393`), 64 wide | `TARGET_MIN` 24, 64 wide (rule 5) | −4 (inside the 36 bar; no layout move) |
| Context strip | 24 | 24 (region); chip `BADGE_MD` 24 | 0 |
| Turn receipt row | 24 literal | `TERMINAL_RECEIPT_H` 24 | 0 |
| Approval card (inner; Claude on the active tab, others and Claude on another tab) | Claude 104, others 72 | Claude 112, others 76 (summary row and action row 28 → 32; the header slot stays 24 and holds the sm 22 `Badge/StatusCount`, centred) | +8 / +4 |
| Approval card (outer, `approval_card_outer_height`) | 112 / 80 | 120 / 84 | +8 / +4 |
| Approval buttons | 28 high, min w 40 | 32 high, min w `CONTROL_MD` 40 | +4 h |
| Approval buttons Show prompt / Open tab (non-Claude, or Claude on another tab) | 28 high, min w 40 | `CONTROL_XS` 32, min w `CONTROL_MD` 40 (same `paint_card_button`) | +4 h (card 72 to 76 inner, 80 to 84 outer) |
| Turn card (`agent_turn_card.rs`, #1173): layer rows, Show all / Show less, Revert / Keep | rows 24 (`CONTROL_DENSE`), toggle 24, buttons 28 (`CONTROL_STANDARD`) | rows 24 and toggle 24 stay (`TARGET_MIN`; rows of a card, technical decision 2), Revert and Keep `CONTROL_XS` 32 (rule 4) | +4 (the card is capped to the panel height, the list scrolls, so no region moves) |
| Terminal search bar | 280×28 | 280×32 | +4 |
| Popover and menu rows | 24–28 | `MENU_ITEM_HEIGHT` 32 | +4 to +8 per row |
| Composer | `CONTROL_ROW` × 2 + n × 16 + 28 | `CONTROL_MD` × 2 + n × 18 + (`BADGE_MD` + 4) | +2 px per extra line (B's `LINE_BODY`) |
| Legacy chat header / runtime bar | 48 / 28 | 48 / 28 (regions); controls inside 24 → `BADGE_MD` 24 for chips, buttons that do not fit 32 stay 24 (`TARGET_MIN`) | 0 |
| Permissions modal | 360 wide, option row 68 | 360 wide; options C's `Radio/Card` (pad 16, radius-xl 12); footer buttons `CONTROL_MD` 40 (controller default "Dialog footer buttons md 40", rule 8) | measured in the PR |
| Radii | `RADIUS_CONTROL` 6, `RADIUS_FRAME` 14 | 8 / 16 (B; popovers and the permissions modal 16, R8.5 and the kit Modals) | — |

**States** (every interactive element; looks come from C's state tables): default, hover, pressed (`BG_PRESSED`, only `TEXT` / `TEXT_SECONDARY` on it), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5), selected (`BG_SELECTED`; active tab adds the `BORDER_SELECTED` underline). Agent tab statuses: Working (info badge + `kit::feedback::loading_ring`), Needs approval (warning dot + `TEXT_WARNING` label, as in `I-05`), Done (success dot), Failed (error badge: `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700`; `TEXT_ERROR` only where the status is plain text).

## 8. Edge cases

1. **Many tabs.** Tabs elide titles (`:5795-5800`); tab height (24) and width rules are unchanged, and `+`, Sessions and Stop keep their widths, so the overflow point is unchanged. Check with 8 tabs: no tab overlaps Sessions, `+`, Skills or the stop controls.
2. **Narrow panel** (left panel at its minimum width): the header still fits the stop controls, `+` and at least one tab; Skills label may elide; no control is clipped below 24 px wide.
3. **Approval queue with long tool input.** The summary row elides at 32 high; the queue count "1 of N" stays right-aligned; the card grows by 8 so the viewport's top content shifts down by 8 when a card is shown (the card overlays the viewport top, `viewport_rect` inset, not a layout region).
4. **Inverse video** (`:6734-6737`) uses `TERM_BG_VIEWPORT` as glyph colour: with the alias it becomes `#0A0A0A` on `#E4E4E7`, contrast 15.60:1 (was 15.18:1). No change in meaning.
5. **Legacy chat branch** must be restyled even though new tabs default to PTY; reach it with `--panel agents-chat` (`I-03`).
6. **No skills attached**: the Skills chip shows "Skills" without a count badge (unchanged logic; only the look changes).
7. **Disabled Approve** (decision in flight): 50 % opacity on fill and label, still `TEXT_ON_BRAND`.
8. **Shell and DESIGN:OS help tabs** have no provider identity today (`#A1A1AA`, `:4801`): they use `ICON_TERTIARY` (rule 10).
9. **Light-on-dark labels on presence colours** (`AGENT_LABEL_TEXT`) change to the dark label `#0A0A0A` by the contract; the four pairs are C-1.41–C-1.44 (10.12, 6.82, 5.88 and 9.24:1).
10. **Approval on another tab** (#1195): the card shows `Open tab` for the tab that waits, at the same 84 outer height as the other non-Claude cards; with several pending approvals the "1 of N" count stays right-aligned. A Claude approval on the active tab keeps the 120 outer height.
11. **Gemini tab**: a Gemini CLI tab has no Stop control (no interrupt row) and its approval card is the non-Claude card (Show prompt or Open tab); the tab status "Needs approval" uses the same warning look as the other CLIs.
12. **Image-job states** (queued, working, failed): the chip uses `Loading/Inline` for queued and working, `TEXT_ERROR` for failed; the Expand and Erase popovers keep their sizes except buttons to `CONTROL_XS`.

## 9. Non-goals

- Terminal content: ANSI palette, cell font and size, cursor shape and colour, cell selection, search match fill.
- Canvas presence colours (`AGENT_1..4`), the info family (`STATUS_INFO_*`), canvas overlays drawn by `canvas/draw.rs` (not mine).
- Any behaviour, keyboard, PTY, approval, queue, session or accessibility change.
- `cmdk.rs` (J) and `panels/check_designs.rs` (K) edits: J's `cmdk.rs` keeps reading the aliases; K re-points `panels/check_designs.rs` itself.
- Light mode, icons (R6 keeps the set; only colour changes), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/agent_chrome.rs` (new name), registered with one `mod agent_chrome;` line in the module list of `crates/designos-app/src/shell/tests.rs` (`:1124-1201`). It uses the shared census helper `chrome_colour_sites(src: &str) -> Vec<usize>` (new name) from `shell/tests/chrome_census.rs` (new name, spec H section 10 defines it; whichever of H–K lands first adds it). The helper ports `scripts/rust_literal_scan.clean` exactly: it blanks each `#[cfg(test)]` item and keeps the shipped code after it, which matters here because `terminal.rs:550` and `skills/mod.rs:503` hold early test items; it returns raw sites, and this spec's test removes its own allowlist by code text and the existing recorded-rect hook (`widgets::record`, `widgets/mod.rs:254`), read with `Sim::rect_of` (`shell/tests.rs:202`) or `ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name))))` as `agent_picker.rs:611` does.

| Test | Asserts | Red at `8dfaee93` |
|---|---|---|
| `agent_sources_paint_no_chrome_colour_literals` | `chrome_colour_sites` over the 20 files of section 2, minus the 6 `terminal.rs` allowlist sites (`TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines, the match fill, 3 cursor lines; identified by their code text, not line numbers), returns nothing | yes (70, the same lines as section 2) |
| `terminal_palette_aliases_contract_roles` | `TERM_BG_VIEWPORT == theme::BG`, `TERM_BG_BAR == theme::BG_PRIMARY`, `TERM_BG_ELEVATED == theme::BG_SECONDARY`, `TERM_BG_HOVER == theme::BG_HOVER`, `TERM_BORDER == theme::BORDER`, `TERM_ORANGE == theme::BORDER_SELECTED`, terminal `STATUS_INFO_* == theme::STATUS_INFO_*`; `STATUS_OK_BG/BORDER/TEXT == theme::BG_SUCCESS_PRIMARY / UTILITY_GREEN_200 / UTILITY_GREEN_700`, and the same for `WARN` (yellow) and `BAD` (red) | yes |
| `terminal_content_colours_unchanged` | `TERM_FG_DEFAULT == #E4E4E7`, `TERM_SELECTION_BG == #5A7898`; existing ANSI pins `terminal.rs:7600-7602` still pass | no (guard) |
| `approval_card_native_cli_buttons_are_control_xs` | `Sim` with a non-Claude approval injected (state, as L's `agents-approval-open-tab`): the recorded `agent-approval-show-prompt` (active terminal) and `agent-approval-open-tab` (other tab) rects are `CONTROL_XS` 32 high, use C's `Button/Secondary` look, and keyboard focus draws `FOCUS_RING` | yes (28, `BORDER_SELECTED`) |
| `terminal_scrollbar_keeps_theme_roles` | guard: `term-scrollbar-track` is `TARGET_MIN` 24 wide, `term-scrollbar-thumb` `SPACE_FACT` 4 wide, and the code at `:6845-6874` reads `theme::` roles only | no (guard, #1171) |
| `approval_card_height_follows_control_xs` | `approval_card_outer_height(Claude) == 120.0`, other presets `== 84.0` | yes (112 / 80) |
| `agents_header_controls_match_tabs` | in L's `Sim` tier (stub CLI dir through `ScopedCliDirs`, scratch library, injected running-turn state for Stop), the recorded rects `agents-sessions-btn`, `agents-runtime-add-btn`, `agents-header-skills` (recorded by `paint_chip`, `skills/mod.rs:803`) and `agents-header-stop-btn` are 24 high and share the active session tab's vertical centre inside the 36 bar; the active tab stays 24 (R10.1) | yes (Skills 28, Stop 28) |
| `agent_surfaces_draw_no_shadow` | cleaned source of I's files has no `elevation_shadow(`, `elevation_400(` or `menu_panel(` call; the permissions modal and the Expand and Erase popovers call C's `modal_surface` / `popover_surface` | yes (4 calls: `agent_approval_card.rs:234`, `agent_mention_popup.rs:332`, `image_jobs.rs:1359`, `:1468`) |
| `permissions_modal_footer_is_control_md` | `Sim` with the permissions modal open (state injected, as L's `agent-permissions` case): the recorded footer buttons (add `widgets::record` keys where missing) are `CONTROL_MD` 40 high | yes |
| `provider_marks_read_theme_constants` | `paint_orca_tab_status_and_provider` and `agent_open_flow::paint_session_row_indicators` take provider colours from `theme::PROVIDER_*` (source scan of `terminal.rs:4731-4808` and `agent_open_flow.rs:338-397`: no `Color32::from_rgb`) | yes (5 + 4 literals) |
| `agents_viewport_rect_unchanged` | at 1600×1088 the viewport rect equals the value captured before the change (L's baseline file) | no (guard, R1) |
| `brand_buttons_use_dark_label` | `kit::button_look(Hierarchy::Primary, State::Default).text == theme::TEXT_ON_BRAND` and no I file paints `Color32::WHITE` (covered by the census) | yes (C's look missing; WHITE at 4 sites) |
| `legacy_chat_panel_uses_contract_roles` | the legacy chat panel renders from injected state in L's `Sim` tier (L case `i-agents-chat`, state-driven); census for `:6082-6233` region is 0 | yes (17) |

Existing tests that must stay green (I re-keys `shell/tests/agent_turn_card_scroll.rs` `terminal_rects`, which finds the terminal and viewport rects by their `TERM_BG_VIEWPORT` fill, to the recorded rects of `show_left_with_doc`, because after the alias that fill is `BG`, which the window and the panels use too): `terminal.rs` test module (including `chat_header_draws_no_control_that_does_nothing` `:7407`, which asserts at `:7477` that the removed pop-out stays gone), `agent_picker.rs:865`, `:880`, `agent_turn_ledger.rs:1216`, `shell/tests/live_agent_edits.rs:119` (renders `agents`, `terminal`, `agents-chat`), `shell/tests/home.rs:483-488`. `terminal.rs:10727-10745` pins move with spec A, not here.

Commands (repo root): `cargo test -p designos-app --bin designos agent_chrome`, then `cargo test -p designos-app --bin designos terminal`, then `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z01–Z04, Z15, Z17 stay OK); `python3 scripts/check_spacing_relationships.py` (exit 0; lower the baseline with `--write-baseline` only if counts fell); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence.** Before and after captures run only through L's capture script (`scripts/layout_baseline_capture.py`, hermetic mode: `env -i`, `DESIGNOS_AGENT_CLI_DIR=<stubs>`, scratch `HOME`, `DESIGNOS_LIBRARY` and `TMPDIR` with markers; `--only-spec I`). I adds no harness of its own and never runs `designos --panel agents*`, `terminal`, `agent-*` or `ai-image-*` on the host `PATH`. Before = L's baseline blessed on the branch base; after = the same cases on the PR build; both sets, the colour and wireframe sheets and `invocations.log` (only stub lines) go in the PR. Planning-round CLI runs use `tools/designos-latest` (v0.7.0, the controller's build for this round; the 0.6.2 pin of plan.md 22:40 is superseded); no I capture was run in fix rounds 2 and 3.

| Id | `--panel` value | L case |
|---|---|---|
| I-01 | `agents` | L's `agents` case (id per `cases.json`) |
| I-02 | `terminal` | L's `terminal` case |
| I-03 | `agents-chat` | `i-agents-chat` (state-driven, `spawns`) |
| I-04 | `agents-picker` | L's `agents-picker` case |
| I-05 | `agents-approval-command` | L's case for that panel |
| I-06 | `agents-approval-queued` | `agents-approval-queued` |
| I-07 | `agents-approval-open-tab` | `agents-approval-open-tab` |
| I-08 | `agent-permissions` | L's case for that panel |
| I-09 | `agents-skills` | `agents-skills` |
| I-10 | `agents-start-failure` | `i-agents-start-failure` (state-driven, `spawns`) |
| I-11 | `ai-image-menu` | `ai-image-menu` |
| I-12 | `ai-image-expand` | `ai-image-expand` |
| I-13 | `ai-image-erase` | `ai-image-erase` |

**Withdrawn.** The 13 runs this spec recorded on 2026-10-25 (a release build of `ec47da26`, setup `--seed-library --library "$LIB"`) were not hermetic: they used the host `PATH` and `HOME`, so agent presets could resolve real CLIs (L rule 5), and the setup line seeds `./--library` because `--seed-library` takes the next argument as its directory (`cli.rs:364`). Their PNGs and exit codes must not be used as evidence.

**States that need agent output** (session failure banner, turn cards with a ledger, mention popup, canvas agent banner and changes chip, image-job working chip, the Stop control of a running turn) are rendered from injected state in L's `Sim` tier, or listed in the PR as "not captured — covered by test <name>" (section 10 tests). No capture comes from a live agent session.

**Acceptance criteria**

1. The census command of section 2 prints `chrome_sites=0` for every I file except `terminal.rs`, which prints exactly the 6 allowlisted content sites (`TERM_FG_DEFAULT` and `TERM_SELECTION_BG` constant lines, match fill, 3 cursor lines). Census red at base: on the branch base the shared helper prints the same 70 lines as section 2 (spec H acceptance 9, helper/Python parity), and `agent_sources_paint_no_chrome_colour_literals` fails with them.
2. The 13 tests above pass; the existing tests listed above pass. The `Sim` tests ran only after L's hermetic tests H1–H11 were green on the base.
3. `CONTROL_DENSE` and `CONTROL_STANDARD` references in I's files: 0 (`grep -rnwE 'CONTROL_(DENSE|STANDARD)' <I files>` prints nothing); the 24 px tab-bar controls read `TARGET_MIN` or `BADGE_MD` (rule 5).
4. `I-01`..`I-13` after-shots (hermetic, L's script) show: no blue chip outside the info family, no white label on orange, session tabs and the Sessions, `+`, Skills and Stop controls 24 inside the 36 bar (R10.1, plan.md C-K), and the viewport at the same position as before (L's layout gate passes for the Agents captures); `invocations.log` holds stub lines only.
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/check_spacing_relationships.py` exit 0.
6. The PR lists the re-anchored line numbers and the before/after census.
7. **Contrast**: every text and icon pair I paints passes `specs/token-contract.md` section (f) "Contrast check C-1" (its python block exits 0; `ui ds a11y` is not a gate). I's rows: C-1.1, 1.2, 1.3, 1.4, 1.5, 1.7, 1.8, 1.9, 1.10, 1.12, 1.13, 1.14, 1.15, 1.16, 1.17, 1.18, 1.19, 1.20, 1.21, 1.22, 1.23, 1.24, 1.27, 1.28, 1.29, and I's own rows C-1.41–C-1.44 (`AGENT_LABEL_TEXT` on `AGENT_1..4`), C-1.45 (`TEXT_QUATERNARY` on `BG_PRIMARY`, other tabs), C-1.46 (Working badge `STATUS_INFO_TEXT` on `STATUS_INFO_BG`), C-1.47–C-1.50 (provider marks), C-1.51 (`ICON_TERTIARY` tab-bar icon). Named exceptions, which are not C-1 passes: E-1 (placeholder `#737373` in the terminal search field, R8.1) and E-2 (1 px `BORDER_STRONG` `#404040` on the search field, the skills checkbox, the permissions `Radio/Card`, secondary buttons and chips, R10.7 and plan.md C-F; field fill `BG`, the kit Input's bg-primary). The open-flow `INFO` spinner and `ICON_BRAND_PRIMARY` dot are non-text on `BG` (6.62:1 and 7.06:1, over 3.0). The inverse-video pair `TERM_FG_DEFAULT` on `TERM_BG_VIEWPORT` is content (information only).

## 11. Performance budgets

No new allocation per frame: aliases are `const`, C's looks are pure functions over constants. The viewport paint path (`render_vt100_viewport`) changes no code except the scrollbar and search-bar colours. Idle CPU stays 0.00 % (no new `request_repaint`). Measure frame time with the Agents panel open on the 1600×1088 window before and after; it must not rise by more than 0.5 ms (the change is colour and a few rect sizes).

## 12. Dependencies, ownership and build order

**Build order.** L lands its hermetic mode and `Sim` seams (tests H1–H11; H7 `scoped_guard_applies_only_inside_a_gate_case`: test builds resolve no host CLI outside the gate's guard) and captures the Agents baseline in hermetic mode (`I-01`..`I-13`) → A adds the contract constants (including N20–N38) → B lands sizes and renames (`CONTROL_ROW` → `CONTROL_MD`) → C lands `widgets/kit/` → this spec. I's `Sim` tests (`agents_header_controls_match_tabs`, `permissions_modal_footer_is_control_md`, `legacy_chat_panel_uses_contract_roles`, `agents_viewport_rect_unchanged`) land only after L's H1–H11 are green. If C is not merged, do not hand-paint anatomy in I's files: wait, because C retires duplicates in `agent_approval_card.rs:172-197`, `:205-237`, `:256-286`, `agent_selection_chip.rs:160-200`, `image_jobs.rs:1242-1263`, `agent_sample_prompts.rs:250-262` (C section 5.1).

**Technical decisions recorded here** (the owner may reverse): (1) terminal palette names stay as aliases instead of being renamed (`cmdk.rs` keeps compiling; the aliases are deleted only after their last outside reader moves); (2) context strip and receipt row stay 24 (status strips, not rows or buttons); (3) the terminal search-match fill stays content (rule 2); (4) the Skills count badge becomes gray (blue stays reserved for the info family); (5) plain-text status colours in I's own files use `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR`, badge text the `UTILITY_*_700` aliases.

**File ownership across H–K** (identical table in I, J and K; H gives the same assignments with a note per file):

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/leave.rs` and `home/unsaved.rs` (new on `origin/main`, no painting), `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs` (30 top-level files at `8dfaee93`: 26 at `205c72ef` plus `agent_approval_channel.rs`, `agent_hook_outputs.rs`, `agent_project_config.rs` and one test file, none of which paints; plus any `agent_*` module added later), `gemini_dialog.rs`, `gemini_screen.rs`, `gemini_status_hooks.rs` (no paint), `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs` (controller ruling), `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `panels/check_designs.rs` (controller ruling), `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests/chrome_census.rs` (new; the `chrome_colour_sites` helper of spec H section 10; whichever of H–K lands first adds it exactly as H defines it) | H |

Not mine (D–G or the kit specs): `theme.rs`, `design/*`, `scripts/*` (A, B); `widgets/*` and `widgets/kit/*` (C, except `widgets/picker.rs`, F); `tabs.rs`, `shell.rs` (toast painted by D with J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs` (including the command palette `:907-1085`), `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, `panels/properties.rs`, `panels/inspect.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G; `specs.rs:2895`, `:3195` call I's legacy chat entry, no edit needed). `shell/tests.rs` is L's (`Sim`, `:69-154`; `Sim::rect_of` `:202`): each H–K PR only appends one `mod` line to its module list (`:1124-1201`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Assigned by the controller (plan.md, H–K check rulings, 2026-10-29 01:05: `lint.rs` K, `canvas/*` and `decks/ui.rs`, `board/*` D, `canvas/proto.rs` F, `icons.rs` C; were unassigned, recommendation in brackets): `canvas/draw.rs` [D or L; uses `theme::STATUS_INFO_*`], `decks/ui.rs` [D], `board/*` [D], `canvas/proto.rs` (4 chrome sites, the prototype-flow overlay; named by no spec) [F or L], `icons.rs` [C], `lint.rs` (`:297`, a non-chrome fallback) [K]. `shell/toast_layout.rs` is D's; `panels/left_sep.rs` is E's; `dev/code_view.rs` is G's.

**Open PRs touching I files** (`gh pr list --state open`, 2026-10-28, 57 open; file lists from the same call). Live feature PRs: #1312 (Ask for risky only mode and per-session grants: `agent_approval_card.rs`, `agent_open_flow.rs`, `agent_picker.rs`, `agent_turn_card.rs`, `agent_permissions.rs`; `CONFLICTING`) and #1318 (MCP risk gate: the same approval-card and picker files; `CONFLICTING`): both change this surface, so the PM orders them before I and I re-anchors again. Draft PRs of the revamp itself: #1206 (L's hermetic mode: `terminal.rs`, `agent_session_failure.rs`) and #1207 (A: `terminal.rs`). Smaller drafts: #1198 (`image_jobs.rs`), #1186 (`agent_turn_card.rs`, on `release/v0.7.0`), #1169 (`agent_approval_card.rs`), #1162 (`agent_picker.rs`, `terminal.rs`), #1150, #1132 (terminal scrollback clamp), #1084 (`agent_open_flow.rs`, `agent_sample_prompts.rs`, `terminal.rs`), #1075, #1070; stale #765, #768, #770 (`CONFLICTING`). Several of these are already inside v0.7.0 by the release merge; the PM confirms which are still open before I starts, and the census is re-run after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R14 (owner, 2026-10-28 23:05): Studio's design store (tokens, registry, soul) uses Studio's own names and values only. I follows it: colours are the contract's Studio constants (the `TERM_*` and `STATUS_*` palette names stay as Studio aliases), widgets are C's names; Untitled role and component names appear only as references (table 1 header, brackets) and in the kit receipt.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor panels: text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius md 8; dialogs: text-sm 14/20, button sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- R8.1 placeholder `#737373` (named exception) · R8.2 selected `#262626` · R8.3 `TEXT_BRAND` `#F97316` · R8.5 floating chrome radius 16, items 8 at inset 8 · R8.6 button xs label text-sm 14 semibold (buttons only) · R8.7 destructive hover `#B91C1C`.
- R9: the live kit may be read with figma-agent, read-only; the permissions modal radius 16, `bg-primary` fill and missing border rest on that check (receipt in rule 8).
- R10.1: "Editor tab heights stay as today: D file tabs 26 in the 32 bar, I session tabs 24, F Inspector segmented strip 24 (R1 wins over R5 here)".
- R10.7: unchecked checkbox and radio border = kit 1 px `BORDER_STRONG` `#404040`.
- Controller defaults (21:35 sweep): terminal ANSI palette is content; chrome around it adopts Untitled; canvas presence colours keep their values; "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2; provider colours are identity content (contract G-2) — I OQ1"; "Dialog footer buttons md 40".
- plan.md C-F (controller, 22:30): "Field fill = kit Input `bg-primary`; E-2 covers every 1 px kit `border-primary` `#404040` control boundary: unchecked checkbox and radio (R10.7), text field and select, secondary button".
- plan.md C-K (controller, 22:35): "Radii and variants follow the live kit (R2), each with a recorded read-only receipt: dialogs/modals = kit Modals; command bar = kit Command menu; toasts = kit Notification; menus/dropdowns/popovers 16 (R8.5). Segmented controls outside R10.1's three strips keep today's height (R1). Controls inside a tab bar match its tabs (contract (d); agent header "Sessions", `+`, Skills, Stop stay 24). Home "Empty trash" uses the kit's secondary-destructive button (C adds it)".
- plan.md (owner, 22:40, superseded 2026-10-28): the planning round first pinned the 0.6.2 build; fix round 3 uses `tools/designos-latest` (v0.7.0), and code citations are at `origin/main@8dfaee93`.
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions (all decided)

None remain for I. Open question 4 of the HIJK review (agent header heights) is decided by plan.md C-K: Sessions, `+`, Skills and Stop stay 24 (rule 5).

1. **Provider identity colours** — decided: kept as identity content, read from `theme::PROVIDER_*` (rule 10). Source: controller default "provider colours are identity content (contract G-2) — I OQ1"; contract N34–N37 (Grok becomes Neutral-200 `#E5E5E5`).
2. **Status badge look** — decided: Untitled tinted badges (rules 3 and 6). Source: controller default "Status badges = Untitled tinted badges (R2; contract N24–N37) — I OQ2".
3. **Session tab height** — decided: stays 24 (rule 5). Source: R10.1 "Editor tab heights stay as today: D file tabs 26 in the 32 bar, I session tabs 24, F Inspector segmented strip 24 (R1 wins over R5 here)" (owner, board 3: "A · Giữ như hiện tại").

## Changelog (fix round 2, curator review `reviews/HIJK-curator-review.md`)

- I-1 (blocker): section 10 evidence rewritten: captures only through L's hermetic capture script (case list with L ids), no `designos --panel agents*` on the host `PATH`, agent-output states from injected `Sim` state or "not captured — covered by test <name>", no live-session capture; the 13 earlier runs and the "Exit code (this session)" column withdrawn as not hermetic (PNGs must not be used); build order: I's `Sim` tests after L's H1–H11 (H7).
- I-2 (plan.md C-K, contract (d)): Sessions button added to section 2 and table 3 (`agent_open_flow.rs:414-420`, `agents-sessions-btn` `:439`); Sessions, `+`, Skills and Stop stay 24 (`TARGET_MIN` / `BADGE_MD`; Skills and Stop −4, `+` 28×24 unchanged); labels text-xs 12 like the tabs; test renamed `agents_header_controls_match_tabs`.
- I-3: permissions-modal footer `CONTROL_MD` 40 in table 3; test `permissions_modal_footer_is_control_md`.
- I-4: helper wording now refers to H's exact port of `rust_literal_scan.clean` (no "first `#[cfg(test)]`"); acceptance 1 adds "census red at base: helper prints the same 71 lines".
- I-5: the misparsed `--seed-library --library "$LIB"` setup is gone; seeding happens inside L's script.
- I-6: permissions modal = `modal_surface` (`BG`, no border, radius 16, `BG_OVERLAY` scrim) replacing `agent_approval_card.rs:192-199`, kit receipt in rule 8; deletes `elevation_shadow` `agent_mention_popup.rs:332` and `menu_panel` `image_jobs.rs:1358`, `:1467`; test `agent_surfaces_draw_no_shadow`.
- I-7: status badges are `Badge/StatusCount` sm 22, centred in the approval card's 24 slot; chips stay `Tag/Chip` 24.
- I-8: acceptance 7 cites C-1.41–C-1.51 and drops the "PR computes them" fallback (edge case 9 too); retired C-1.25/C-1.26 replaced by E-1.
- I-9: open-flow waiting dot `ICON_BRAND_PRIMARY`; working spinner `INFO` (info family, contract row 101), so one status has one colour.
- Review "Same edit for I": acceptance 7 cites E-1 and E-2 with plan.md C-F; the search field is `BG` + `BORDER_STRONG`.
- Coordinator amendment: 0.6.2 build for planning-round CLI runs (plan.md 22:40); C-F and C-K quoted in section 13.


## Changelog (fix round 3, 2026-10-28: re-anchor to `origin/main@8dfaee93`, plan.md C-K / C-R, R14)

Review re-check against `reviews/HIJK-curator-review.md` (each id read on the spec text):

- I-1 (blocker): present: captures only through L's hermetic capture script, no `designos --panel agents*` on the host `PATH`, agent-output states from `Sim` or "not captured — covered by test <name>", the 13 earlier runs withdrawn, build order after L's H1–H11. Unchanged by v0.7.0.
- I-2: present and now ruled (plan.md C-K): Sessions, `+`, Skills and Stop stay 24 inside the 36 bar (rule 5, table 3, test `agents_header_controls_match_tabs`); the agent header controls stay 24. Stop is drawn only for a stoppable tab (new at v0.7.0).
- I-3: present: permissions-modal footer `CONTROL_MD` 40.
- I-4: present: acceptance 1 says the helper prints the same lines as section 2; the count is now 70 at `8dfaee93`.
- I-5: present: no misparsed setup remains (seeding happens inside L's script).
- I-6: present: permissions modal = `modal_surface` (`BG`, no border, radius 16, `BG_OVERLAY`), `agent_mention_popup.rs:332` and the two `image_jobs.rs` `menu_panel` calls listed (now `:1359`, `:1468`).
- I-7: present: status badges `Badge/StatusCount` sm 22 centred in the 24 slot.
- I-8: present: acceptance 7 cites C-1.41–C-1.51 (rows exist in the recorded C-1 run, `evidence/AB-fix-round-261028/c1-contrast-check-at-8dfaee93.txt` section 1, 70/70 pass, exit 0).
- I-9: present: waiting dot `ICON_BRAND_PRIMARY`, working spinner `INFO`.
- X-1 to X-5 (cross-spec findings of the same review): answered one line each in H's fix round 3 changelog; the ownership table here matches plan.md 22:20 and D's refreshed list, and the unassigned list names `lint.rs` (K) and `canvas/proto.rs`.
- Re-anchor: every citation moved from `205c72ef` to `8dfaee93` by a line diff (cell, cursor and match lines placed by code text: `:6685`, `:6734-6737`, `:6747`, `:6749`, `:6815`, `:6825`, `:6829`, `:6841`; `agent_turn_card.rs:294`); census re-run (exit 0): 70 sites, `terminal.rs` 50; palette use counts and importers refreshed; drift note added.
- Drift that changed the surface: approval card for any tab with Show prompt / Open tab and a focus stroke (rule 4, rule 9, table 3, test `approval_card_native_cli_buttons_are_control_xs`, edge cases 10 and 11); Stop only for a stoppable tab (rule 5); turn card scroll area (table 3); scrollbar #1171 already on theme roles (table 1, guard `terminal_scrollbar_keeps_theme_roles`); `agent_turn_card_scroll.rs` re-keyed (section 10).
- R14: table 1 header and ruling say Studio constants first, Untitled roles as references; the `TERM_*` / `STATUS_*` names stay Studio aliases.
- plan.md C-K / C-R: controls inside a tab bar match its tabs, agent header controls stay 24 (rule 5, acceptance 4); open PRs refreshed (#1312 and #1318 change this surface); unassigned list adds `canvas/proto.rs`.
