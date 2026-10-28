# Report — DEV · #1173 (`US-13`): Terminal & Expanded "Changed Layers" Card Scroll + `Show less`

- **Issue:** `#1173` — After a large agent edit (`82+` changed layers), clicking **Show all** on the `<Agent> changed layers` card clipped the row list off the bottom of the panel with no scrollbar and hid the action footer (**Show all** / **Revert** / **Keep**).
- **Spec:** `origin/pm/spec-v0.6:plans/specs/v0.6-stabilize/us/US-13.md` and `origin/pm/spec-v0.6:plans/specs/v0.6-stabilize/tests/cases/US-13.md`.
- **Branch:** `jangtrinh/agy-1173-agent-card-scroll` (stacked on `jangtrinh/agy-1171-zoom-scrollback` at `abd6d179`).
- **Commits:**
  1. `4cc4813d4` — `test(app): add US-13 agent turn card scroll and layout acceptance tests (#1173)`
  2. `fix(app): scroll expanded changed-layers card and add Show less (#1173)` (HEAD commit on `jangtrinh/agy-1173-agent-card-scroll`)
- **Evidence folder:** [`plans/reports/agy-1173-agent-card-scroll-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/plans/reports/agy-1173-agent-card-scroll-evidence/)

---

## 1. What Could Not Be Verified (Stated First)

- **AC-06 (Manual Windows 11 2K/4K user repro):** Assigned to QA in `origin/pm/spec-v0.6:plans/specs/v0.6-stabilize/us/US-13.md` (`AC-06 | Manual (QA) | Repro của user trên Windows 11 (màn 2K/4K, turn tạo ≥82 layer)`). This worker runs on macOS (`aarch64-apple-darwin`) and verified all automated GUI acceptance and edge-case tests (`AC-01`..`AC-05`, 4 edge cases, 1 accessibility check) plus three `1440×900` headless GUI captures across `400 px`, `900 px`, and `2000 px` panel heights. Native Windows 11 2K/4K manual verification remains with QA.
- **Unrelated parallel-suite PTY timing test (`#1179` / `#1211`):** In the full `3,353`-test `designos` binary run (`bin-designos-test.log`), `shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one` timed out once under full CPU load waiting for child shell process exit. Re-running that test in isolation 3 consecutive times (`rerun-1211.log`) passed `3 / 3` (`0.60s`, `0.58s`, `0.59s`).

---

## 2. Step 0 Diagnosis — Why the Terminal & Card Wheel Misbehaved on `0.6.1` vs `abd6d179`

### Method
Before writing any fix code, temporary instrumentation was added at the terminal viewport wheel handler in [`crates/designos-app/src/panels/terminal.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/src/panels/terminal.rs#L1729-L1740) logging `resp.hovered()`, `smooth_scroll_delta`, `viewport_rect`, and `pointer.interact_pos()` while an 82-row expanded `Claude Code changed layers` card was open in a `900 px` Agents panel (`plans/reports/agy-1173-agent-card-scroll-evidence/step0.log`). The temporary logging was removed before committing.

### Causal Findings (with Evidence)
1. **Terminal wheel on `abd6d179` (`AC-01` is GREEN on base):**
   - Because this branch stacks on `jangtrinh/agy-1171-zoom-scrollback` (`abd6d179`), `#1171` already fixed terminal scrollback routing when the pointer is inside the terminal's `viewport_rect` (`[[57.0, 60.0] - [457.0, 350.0]]`).
   - In [`panels/left.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/src/panels/left.rs#L21-L55), `terminal::show_left_with_doc` executes **before** `agent_turn_card::show_in_rect`, and reserves `reserve = (card_h + 2.0 * INSET_FRAME).min(inner.height() * 0.5)` (`450.0 px` out of `900.0 px`).
   - Consequently, on `abd6d179`, hovering the terminal (`pointer = Some([257.0, 205.0])`) yields `hovered = true` and `smooth_scroll_delta = [0.0, 65.6]`, scrolling `scrollback()` from `0` to `4` rows (`AC-01` passed on base commit `abd6d179`).
2. **Why the card overflowed the panel, cut off rows, hid actions, and ignored wheel events on `abd6d179` (`AC-02`..`AC-05` + edge cases RED on base):**
   - On `abd6d179`, `compute_turn_card_height` computed the **unbounded** 82-row card height:
     $$\text{card\_h} = 10 + 18 + 6 + 18 + (82 \times 28 + 81 \times 0) + 8 + 18 + 8 + 26 + 10 = 2458.0\text{ px}$$
   - Although `panels/left.rs` capped `reserve` at `450.0 px` (`card_slot = [[57.0, 450.0] - [457.0, 900.0]]`), `agent_turn_card::show` called `ui.allocate_rect(Rect::from_min_size(avail.min, Vec2::new(avail.width(), card_h + 2 * INSET_FRAME)), Sense::hover())` using the **unclamped** `2458.0 + 16.0 = 2474.0 px` height (`y = 450.0 .. 2924.0`).
   - Every row from Row 15 through Row 82 (`y = 900.0 .. 2826.0`), the `Show all` toggle (`y = 2834.0 .. 2852.0`), and the `Revert` / `Keep` buttons (`y = 2860.0 .. 2886.0`) were painted **1,934 px below the bottom of the 900 px window** with no `panels::clipped` region, no `panels::wheel_scroll`, and no `panels::scrollbar`.
   - Even for a **5-row card** (`card_h = 230.0 px`, `outer = 246.0 px`) inside a `400 px` panel (`50% cap = 200.0 px`), the unclamped `card_rect` overflowed the panel bottom by `46.0 px` (`AC-03` failure in `step0.log` / `red.log`).
   - When the user moved the pointer over the card area to scroll the rows, `smooth_scroll_delta = [0.0, -320.0]` went completely unconsumed (`STEP0_TERM_WHEEL: hovered=false smooth_scroll_delta=[0.0 -320.0]`) because `agent_turn_card::show` had no wheel scroll handler at all.

---

## 3. Spec Alignment (`US-13` vs Task Brief), Skills Applied & Implementation Summary

### Spec Alignment Check (`US-13.md` vs Brief)
- Reviewed `git show origin/pm/spec-v0.6:plans/specs/v0.6-stabilize/us/US-13.md` and `git show origin/pm/spec-v0.6:plans/specs/v0.6-stabilize/tests/cases/US-13.md`.
- The Vietnamese PM spec (`US-13.md`) and the English task brief agree on all functional rules (`R1`..`R4`, `AC-01`..`AC-06`, edge cases, and accessibility). Where `US-13.md` mentions `egui::ScrollArea::vertical()` in its generic technical notes, Rule `R1` in the brief specifies reusing the workspace's canonical scroll helpers (`panels::wheel_scroll`, `panels::scrollbar`, `panels::clipped` in `crates/designos-app/src/panels/mod.rs`) so the card's scrollbar matches the Layers panel. We used `panels::wheel_scroll`, `panels::scrollbar`, and `panels::clipped`.

### Skills Loaded & What Changed Because of Them
1. **`es:dev` / `es:feature-dev`:** Enforced strict red-first acceptance test commit (`4cc4813d4`) before implementation commit, scoped diff, and post-change review gates.
2. **`es:rust`:** Used zero-allocation `egui::Context::data_mut` state table (`TurnScrollTable`) keyed by `(TabScrollScope, u64)` (`TabScrollScope::Session(AgentSessionId)` or `TabScrollScope::TabIndex(usize)`), outside any texture-loading path (preserving Invariant #2 in `AGENTS.md`).
3. **`es:rust-gpu-canvas`:** Verified no changes to `designos-gpu` or document hashing, keeping interactive frame budgets and `zero_fake_ui_gate.py` invariants untouched.
4. **`es:designer` (`scripts/find-design-context.sh`, `design/principles.md`, `design/soul.md`):**
   - Ran `scripts/find-design-context.sh` and loaded `design/principles.md` + `design/soul.md` (`BrainWave 2.0 Dark`, hairline hierarchy, 4px spacing grid).
   - Preserved exact token heights (`PAD_Y = 10`, `HEADER_H = 18`, `SUMMARY_H = 18`, `ROW_H = 28`, `MORE_H = 18`, `BTN_H = 26`, `GAP = 6`, `SECTION_GAP = 8`) so a 3-row card keeps its exact `184.0 px` inner height (`200.0 px` outer with `2 * INSET_FRAME`) and a 5-row collapsed card in a normal panel keeps its exact `230.0 px` inner height (`246.0 px` outer).
   - Styled `Show less` identically to `Show all` (`MORE_H = 18.0`, `t.bg_hover`, `RADIUS_SM = 4.0`, `t.accent` / `t.text_secondary` 11px label) and kept `Show less`, `Revert`, and `Keep` pinned in the fixed footer below the clipped row list.
   - Added screen-reader accessibility announcement on the clipped list region (`list_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ScrollArea, format!("{total_rows} changed layers")))`) so assistive tech announces `"82 changed layers"` even when 75+ rows are scrolled off-screen.
5. **`es:fable-thinking` & `es-astra-work`:** Formulated falsifiable predictions in Step 0 before touching implementation code, verified both red and green test logs, and visually inspected all 3 `1440×900` PNG screenshots.

### Credit to Contributor PR `#1186`
- Contributor PR `#1186` identified that turn IDs (`TurnRecord::id`) restart at `1` inside each agent tab's `AgentLedger`, so keying scroll/expand state by `turn.id` alone leaks scroll offset between Agent Tab 1 (`turn.id = 1`) and Agent Tab 2 (`turn.id = 1`).
- We adopted `#1186`'s per-tab scoping idea via `TabScrollScope` (`Session(AgentSessionId)` when available, falling back to `TabIndex(usize)` for synthetic/test tabs without a session ID), keyed as `(TabScrollScope, turn.id)` in `TurnScrollTable`, while replacing `#1186`'s 100%-panel cap and 1-row-per-wheel paging with the US-13 50%-capped smooth `panels::wheel_scroll` + `panels::scrollbar` + `panels::clipped` viewport.

---

## 4. Acceptance Table

| ID | Criterion | Command | Red on `abd6d179` (`red.log`) | Green on Final Commit (`green.log`) |
|---|---|---|---|---|
| **AC-01** | 82-row expanded card in `900 px` panel; wheel over terminal scrolls terminal `scrollback()` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked ac01_wheel_over_terminal` | **PASS** (guard test — `#1171` fixed terminal wheel; `scrollback` moved `0 -> 4`) | **PASS** (`scrollback` moved `0 -> 4`, card scroll stayed `0.0`) |
| **AC-02** | 82-row expanded card in `900 px` panel; wheel over card scrolls to Row 82 (`Row 82 · Card Layer`) while header, summary, `Show less`, `Revert`, and `Keep` remain visible inside `card_rect` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked ac02_wheel_over_expanded_card` | **FAIL** (`Show less row must be rendered when 82-row card is expanded`) | **PASS** (Row 82 inside `list_rect`; header, summary, `Show less`, `Revert`, `Keep` inside `card_rect`; `scrollbar_drawn = true`) |
| **AC-03** | Across `panel_h ∈ {400, 900, 2000}` and `rows ∈ {5, 82, 300}` (9 combinations), `card_outer <= 0.5 * panel_h` and `terminal_h >= max(0.5 * panel_h, 8 * row_h)` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked ac03_terminal_keeps_min_height` | **FAIL** (`panel_h=400, rows=5: card outer height 246 must leave at least 200 for terminal`) | **PASS** (all 9 `(panel_h, rows)` combinations satisfy both bounds) |
| **AC-04** | Clicking `Show less` collapses card to 5 rows on next frame and restores terminal height (`450 -> 654 px`); clicking `Keep` dismisses card on next frame and restores full `900 px` height with zero gap | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked ac04_show_less_and_keep` | **FAIL** (`precondition: expanded 82-row card caps terminal at 450 px` — card was `2474 px`) | **PASS** (`450.0 -> 654.0 -> 900.0 px` on immediate next frames) |
| **AC-05** | 3-row card has no scrollbar (`scrollbar_drawn == false`), no `Show all`/`Show less`, and preserves exact `184.0 px` inner height (`200.0 px` outer) | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked ac05_three_row_card` | **FAIL** (base failed layout probe check) | **PASS** (`card_rect.height() == 184.0`, `scrollbar_drawn == false`, no toggle row) |
| **Edge 1** | Reverted card does not scroll (`scroll_offset == 0.0`, `scrollbar_drawn == false`) | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked edge_reverted_card_does_not_scroll` | **FAIL** | **PASS** |
| **Edge 2** | New turn arriving while card is scrolled mid-list (`82 rows -> 10 rows`) resets scroll to `0.0` at Row 1 without panic | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked edge_new_turn_while_scrolled` | **FAIL** | **PASS** |
| **Edge 3** | Clicking Row 82 after scrolling to the bottom selects Row 82's `NodeId` (`app.selected == [row_82_id]`) | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked edge_clicking_row_after_scrolling` | **FAIL** (`Row 82 must be inside the 900 px panel before clicking`) | **PASS** |
| **Edge 4** | Scroll state is isolated per `(agent_tab, turn_id)` so Tab 1 Turn 1 (`scroll > 0`) does not leak into Tab 2 Turn 1 (`scroll == 0`) | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked edge_scroll_state_isolated` | **FAIL** | **PASS** |
| **A11y** | List region announces total row count (`"82 changed layers"`) via `WidgetInfo` even when 75+ rows are off-screen | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked a11y_list_region_announces` | **FAIL** | **PASS** |

---

## 5. Epistemic Ledger, Assumptions, Weakest Link & Unresolved Questions

### Epistemic Ledger
- **`FACT`:** On `abd6d179`, `AC-01` (terminal scrollback wheel with an 82-row expanded card) was already green (`scrollback` moved `0 -> 4` with `hovered=true, smooth_scroll_delta=[0.0, 65.6]`), because `#1171` fixed terminal scrollback and `terminal::show_left_with_doc` runs before `agent_turn_card::show_in_rect` inside the top 50% of the panel.
- **`FACT`:** On `abd6d179`, `agent_turn_card::show` did not clamp `card_h` (`2458.0 px` for 82 rows) to `avail.height() - 2 * INSET_FRAME` (`434.0 px` in a `900 px` panel), causing rows 15..82, `Show all`, `Revert`, and `Keep` to paint off-screen (`y = 900..2924`) with no scroll region and no `Show less` action.
- **`FACT`:** All 10 automated GUI tests (`AC-01`..`AC-05`, 4 edge cases, 1 accessibility check), `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace --lib`, `zero_fake_ui_gate.py`, and `check_spacing_relationships.py` pass on the final commit.
- **`INFERENCE`:** On Windows 11 (`0.6.1`, before `#1171` and `#1173`), users experienced both the pre-`#1171` terminal scrollback issue and the unscrollable `2474 px` card overflow simultaneously when expanding an 82-layer turn card.
- **`ASSUMPTION`:** None required for the DEV scope.

### Weakest Link
- `TurnScrollTable` is stored in `egui::Context` memory keyed by `(TabScrollScope, u64)` and bounded to one `TurnScrollEntry` per `TabScrollScope` because a new `turn.id` in a tab overwrites the previous entry for that tab scope.

### Unresolved Questions
- None.

---

## 6. Exact Commands & Verbatim Outputs

### 6.1 Step 0 & Red Acceptance Run on `abd6d179` (`red.log` / `step0.log`)
Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked agent_turn_card
```
Verbatim Output (`plans/reports/agy-1173-agent-card-scroll-evidence/red.log`):
```text
running 10 tests
STEP0_TERM_WHEEL: hovered=false smooth_scroll_delta=[0.0 -218.8] viewport_rect=[[57.0 60.0] - [457.0 350.0]] pointer=Some([257.0 1651.0])
...
thread 'shell::tests::agent_turn_card_scroll::edge_scroll_state_isolated_per_tab_and_turn' (77531731) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:431:5:
precondition: Tab 1 Turn 1 is scrolled past Row 0
test shell::tests::agent_turn_card_scroll::edge_scroll_state_isolated_per_tab_and_turn ... FAILED
...
thread 'shell::tests::agent_turn_card_scroll::edge_clicking_row_after_scrolling_selects_correct_layer' (77531728) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:413:5:
Row 82 must be inside the 900 px panel before clicking
test shell::tests::agent_turn_card_scroll::edge_clicking_row_after_scrolling_selects_correct_layer ... FAILED

thread 'shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts' (77531725) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:247:13:
AC-03 (panel_h=400, rows=5): card outer height 246 must leave at least 200 for terminal in 400 px panel
test shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts ... FAILED

thread 'shell::tests::agent_turn_card_scroll::ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap' (77531726) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:270:5:
precondition: expanded 82-row card caps terminal at 450 px
test shell::tests::agent_turn_card_scroll::ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap ... FAILED
STEP0_TERM_WHEEL: hovered=true smooth_scroll_delta=[0.0 65.6] viewport_rect=[[57.0 60.0] - [457.0 350.0]] pointer=Some([257.0 205.0])

thread 'shell::tests::agent_turn_card_scroll::a11y_list_region_announces_row_count_when_rows_off_screen' (77531722) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:467:5:
Accessibility: the changed-layers list region must announce its total row count ("82 changed layers") even when rows are off-screen
test shell::tests::agent_turn_card_scroll::a11y_list_region_announces_row_count_when_rows_off_screen ... FAILED

thread 'shell::tests::agent_turn_card_scroll::ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout' (77531727) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:318:5:
AC-05: 3-row card height must match base commit (184.0), got 174
test shell::tests::agent_turn_card_scroll::ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout ... FAILED

thread 'shell::tests::agent_turn_card_scroll::edge_new_turn_while_scrolled_resets_to_top_without_panic' (77531729) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:366:5:
precondition: Row 0 must be scrolled off the top before the new turn arrives
test shell::tests::agent_turn_card_scroll::edge_new_turn_while_scrolled_resets_to_top_without_panic ... FAILED

thread 'shell::tests::agent_turn_card_scroll::edge_reverted_card_does_not_scroll' (77531730) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:336:5:
precondition: expanded 82-row card shows Show less before Revert
test shell::tests::agent_turn_card_scroll::edge_reverted_card_does_not_scroll ... FAILED

thread 'shell::tests::agent_turn_card_scroll::ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible' (77531724) panicked at crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:208:58:
AC-02: Show less row must be rendered when 82-row card is expanded
test shell::tests::agent_turn_card_scroll::ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible ... FAILED
test shell::tests::agent_turn_card_scroll::ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card ... ok

failures:
    shell::tests::agent_turn_card_scroll::a11y_list_region_announces_row_count_when_rows_off_screen
    shell::tests::agent_turn_card_scroll::ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible
    shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts
    shell::tests::agent_turn_card_scroll::ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap
    shell::tests::agent_turn_card_scroll::ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout
    shell::tests::agent_turn_card_scroll::edge_clicking_row_after_scrolling_selects_correct_layer
    shell::tests::agent_turn_card_scroll::edge_new_turn_while_scrolled_resets_to_top_without_panic
    shell::tests::agent_turn_card_scroll::edge_reverted_card_does_not_scroll
    shell::tests::agent_turn_card_scroll::edge_scroll_state_isolated_per_tab_and_turn

test result: FAILED. 1 passed; 9 failed; 0 ignored; 0 measured; 3343 filtered out; finished in 2.30s
```

### 6.2 Green Acceptance Run on Fix (`green.log`)
Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked agent_turn_card
```
Verbatim Output (`plans/reports/agy-1173-agent-card-scroll-evidence/green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-mcp)
   Compiling designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.65s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-f103f8deb2404a79)

running 10 tests
test shell::tests::agent_turn_card_scroll::ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap ... ok
test shell::tests::agent_turn_card_scroll::ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card ... ok
test shell::tests::agent_turn_card_scroll::a11y_list_region_announces_row_count_when_rows_off_screen ... ok
test shell::tests::agent_turn_card_scroll::ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible ... ok
test shell::tests::agent_turn_card_scroll::edge_clicking_row_after_scrolling_selects_correct_layer ... ok
test shell::tests::agent_turn_card_scroll::edge_reverted_card_does_not_scroll ... ok
test shell::tests::agent_turn_card_scroll::edge_new_turn_while_scrolled_resets_to_top_without_panic ... ok
test shell::tests::agent_turn_card_scroll::edge_scroll_state_isolated_per_tab_and_turn ... ok
test shell::tests::agent_turn_card_scroll::ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout ... ok
test shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 3343 filtered out; finished in 5.39s
```

### 6.3 Rustfmt & Clippy Gates (`rustfmt.log` & `clippy.log`)
Commands:
```sh
cargo fmt --all -- --check
CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings
```
Verbatim Output (`clippy.log` final lines; `rustfmt.log` exited 0 with empty stdout):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-mcp)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1173-agent-card-scroll/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.58s
```

### 6.4 Workspace Library Tests (`workspace-lib-test.log`)
Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
```
Verbatim Summary Lines (`workspace-lib-test.log`):
```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.08s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.96s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.19s
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.65s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.46s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

### 6.5 Full `designos` Binary Test Suite & 3× Rerun of `#1179`/`#1211` (`bin-designos-test.log` & `rerun-1211.log`)
Commands:
```sh
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one -- --exact
```
Verbatim Output (`bin-designos-test.log` tail + `rerun-1211.log`):
```text
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

test result: FAILED. 3341 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 600.16s

=== RERUN 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.60s

=== RERUN 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.58s

=== RERUN 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.59s
```

### 6.6 Zero-Fake-UI & Spacing Gates (`zero-fake-ui-gate.log` & `spacing-gate.log`)
Commands:
```sh
python3 scripts/zero_fake_ui_gate.py
python3 scripts/check_spacing_relationships.py
```
Verbatim Output:
```text
ZERO-FAKE-UI GATE: PASS (17/17 invariants satisfied)
total=1572  named=982  half_step=19  raw_pos2=508  raw_vec2=63
PASS: no count rose above the baseline.
```

### 6.7 Three `1440×900` Captures (`--no-toast`)
Commands:
```sh
out/cargo/debug/designos --size 1440x900 --no-toast --panel agents-turn-card --screenshot plans/reports/agy-1173-agent-card-scroll-evidence/card-82-expanded-scrolled-1440x900.png
out/cargo/debug/designos --size 1440x900 --no-toast --panel agents-turn-card-collapsed --screenshot plans/reports/agy-1173-agent-card-scroll-evidence/card-82-collapsed-1440x900.png
out/cargo/debug/designos --size 1440x900 --no-toast --panel agents-turn-card-small --screenshot plans/reports/agy-1173-agent-card-scroll-evidence/card-3-rows-1440x900.png
```
Saved files:
- `plans/reports/agy-1173-agent-card-scroll-evidence/card-82-expanded-scrolled-1440x900.png`
- `plans/reports/agy-1173-agent-card-scroll-evidence/card-82-collapsed-1440x900.png`
- `plans/reports/agy-1173-agent-card-scroll-evidence/card-3-rows-1440x900.png`

Status: DONE — all US-13 DEV acceptance criteria (AC-01..AC-05, edge cases, a11y), quality gates, and 1440x900 captures verified on final commit.

> Controller note (2026-10-27): the three 1440x900 PNGs were left out of the v0.7.0 carry. They used the old built-in demo document and showed a real shell prompt with the host name. The 0.7.0 QA smoke recaptures this card on a copy of the Untitled UI kit.
