# Report — DEV · #1142 (UI Revamp Spec A, P0): Untitled UI Dark Colour Roles, Token Store, `theme.rs` Colours, Soul and Principles

## 1. What Could NOT Be Verified (Stated First)

1. **`.agents/agents/figma-jang-designos-studio.md:17` retired hex reference (`#121215`, `#18181B`)**:
   - **FACT:** Spec A §5.4's check `grep -rniE '#(121215|18181b|1e1e22|232329|2e2e36|2b1d16)|Zinc' .agents .claude design/soul.md design/principles.md` returns 0 matches across `.agents/agents/{curator,designer,librarian}-jang-designos-studio.md`, `.claude/`, `design/soul.md`, and `design/principles.md`, but matches 1 line in `.agents/agents/figma-jang-designos-studio.md:17` (`#121215 canvas, #18181B bg`).
   - **Reason not edited:** `.agents/agents/figma-jang-designos-studio.md` is outside the allowed `Touches` list (`{curator,designer,librarian}-jang-designos-studio.md`). Strictly obeyed the file-ownership boundary; flagged here for follow-up.
2. **`G09` (`design_os_project_audit` on `docs/index.html`) inside `scripts/design_os_self_evolve.py`**:
   - **FACT:** In `python3 scripts/design_os_self_evolve.py`, gates `G01` through `G08` all pass (`8/9`), while `G09` (`design-os audit .`) reports 1 error in `docs/index.html` (`raw-hex-when-token-exists` on `#121215`, `#3F3F46`, `#D4D4D8`, `#5CB8FF` after `design/design.tokens.json` moved from Zinc to Untitled Neutral).
   - **Reason not edited:** `docs/index.html` is outside Spec A's `Touches` list and is owned by Spec O (public landing/docs site). Spec A §10 Acceptance item 4 explicitly scopes the `design_os_self_evolve.py` requirement to `G01`, `G05`, `G06`, and `G08` green (with `G04` as supplementary evidence).
3. **Responsive breakpoints**:
   - `responsive: SKIPPED (owner default)` per Spec A §7 and Controller Ruling 7 (desktop `1600x1088` captures only).

---

## 2. Conclusion & Weakest Link

- **Conclusion (`FACT`):** Spec A (#1142) is implemented and verified against `token-contract.md` and `1142-onboard-untitled-dark-colour-roles-and-soul.md`. All 41 colour roles in `design/design.tokens.json` were re-valued in place via `ui ds change-token` (`gen 28 → 69`, `77` tokens, `12` components preserved), mirrored to `design/tokens.json`, documented in `design/DESIGN.md`, and refreshed in place in `design/specimen.html`. All 116 existing colour constants in `crates/designos-app/src/theme.rs` were updated (`55` re-valued, `52` kept, `4` aliases kept, `5` zero-ref constants deleted) and `46` new colour constants (`N1`..`N46`) were added (`157` active `Color32` constants total), `theme::apply()` was updated, `crates/designos-app/src/terminal.rs` colour pins were updated in the same commit, `design/soul.md` and `design/principles.md` were replaced verbatim (`cmp` exit 0), `.agents/agents/{curator,designer,librarian}-jang-designos-studio.md` were updated, `scripts/design_os_self_evolve.py` (`G01` + `G04`) was updated, and `scripts/check_chrome_colour_literals.py` + `scripts/chrome_colour_baseline.json` (`406` sites across `50` files at `HEAD`) + unit tests were added.
- **Weakest link in one line:** Surface call-site colour literals (`406` sites across `50` files, ratcheted by `scripts/check_chrome_colour_literals.py`) remain on legacy inline `Color32` values until surface specs `D`–`K` retire them.

---

## 3. Drift Table (Re-anchored Against `HEAD` `3c0818a6` Before Code)

The spec baseline is `ec47da26` with noted drift at `35a65796`. Our worktree `HEAD` is `origin/main` at `3c0818a6b09112a53dbc2576bf725703a0298385` (15 commits ahead of `ec47da26`). Every cited file and line was re-anchored before editing:

| File | Spec citation (`ec47da26` / `35a65796`) | Re-anchored at `HEAD` (`3c0818a6`) | Cause of drift / observation (`FACT`) |
|---|---|---|---|
| `crates/designos-app/src/theme.rs` (top colours) | `:20-112` | `:21-112` (`BG` at `:22` .. `AGENT_LABEL_TEXT` at `:112`) | 0 line shift in top colour block |
| `crates/designos-app/src/theme.rs` (`FIND_HIGHLIGHT*`) | `:313-314` | `:318-319` | `+5` lines (`LEFT_DEV_READY_SECTION_STEP` and `LAYER_FILTER_ROW_HEIGHT` added in `#1106`/`#1131`) |
| `crates/designos-app/src/theme.rs` (`BG_MENU_SELECTED`, `MENU_SHORTCUT_HIGHLIGHT`) | `:424`, `:443` | `:429`, `:448` | `+5` lines |
| `crates/designos-app/src/theme.rs` (`COMMENT_PIN_RESOLVED`, `AVATAR_COLORS`) | `:470`, `:472` | `:477`, `:479` | `+7` lines (`TOAST_REVEAL_SECONDS` `+2` at `:470`) |
| `crates/designos-app/src/theme.rs` (`RULER_*`, `PIXEL_GRID_*`, `TEXT_SELECTION`, `FIND_CANVAS_MATCH*`) | `:528-547` | `:535-554` | `+7` lines |
| `crates/designos-app/src/theme.rs` (`SYNTAX_*`, `DEV_*`, `STATUS_INFO_*`, `BOX_*`, `ANNOTATION_*`, `MEASURE`) | `:555-582` | `:562-589` | `+7` lines |
| `crates/designos-app/src/theme.rs` (`PROTO_BLUE`, `PRESENT_BG`) | `:587`, `:596` | `:594`, `:603` | `+7` lines |
| `crates/designos-app/src/theme.rs` (`apply()`) | `:619-657` | `:626-664` | `+7` lines |
| `crates/designos-app/src/theme.rs` (`HOME_THUMB_BG`, `VAR_SCROLL_THUMB`, `BOARD_DOT`) | `:702`, `:724`, `:746` | `:709`, `:731`, `:753` | `+7` lines |
| `crates/designos-app/src/theme.rs` (`mod tests`) | `:770-913` | `:777-920` (`selection_fills_are_neutral` at `:907-913`) | `+7` lines |
| `crates/designos-app/src/terminal.rs` (`theme_has_info_success_warning_in_token_parity`) | `:10019-10037` (`:9613-9630` at `35a65796`) | `:9615-9634` (pins at `:9617-9619` and `:9631-9633`) | `+2` lines vs `35a65796` |
| `design/design.tokens.json` & `design/ds.manifest.json` | `gen 28`, `compiledHash: sha256-1izY1I1zaVGRQzwiik2vdeZJYcwzWuxTWW-GHSBOnVE` | `spacing.rail-width` changed `"56px"` → `"48px"` in commit `b598a4992` (`#1131`) without updating `ds.manifest.json` `compiledHash` | Re-anchored `compiledHash` to `sha256-ev7pnlT8h7z2mmIB_g2eTKuoV77rsXZ_MJGdAugnVuA` before running the 41 `ui ds change-token` mutations so `spacing.rail-width: "48px"` and `gen 69` (`28 + 41`) are preserved |
| `scripts/design_os_self_evolve.py` | `:109`, `:111-134`, `:137-158`, `:160-163`, `G04` `:365` | `:109`, `:111-134`, `:137-158`, `:160-163`, `G04` `:365` | 0 line shift at `HEAD`; `non_colour_token_parity.py` verifies `27` non-colour constants at `HEAD` (with 2 known drifts `FONT_SIZE_BODY` and `FONT_SIZE_LARGE` deferred to Spec B) |
| `scripts/chrome_colour_baseline.json` (literal census) | `410` sites in `51` files at `ec47da26` | `406` sites in `50` files at `HEAD` (`3c0818a6`) | `panels/rail.rs` retired 4 literal sites in `#1131` (`b598a4992`), leaving `406` chrome colour sites across `50` files |

---

## 4. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT` (Kit variable verification, Step 0b):** Exporting variables from `out/untitled-ui-copy.fig` in both `figma-rest` (`out/kit/all.json`, 10 collections) and `dtcg` (`out/kit/dtcg`) via `designos` and resolving all aliases up to 16 hops produced `Dark mode 273`, `Light mode 273`, `Mode 1 40`, `Value 4`, `Style 343`, and `compared 589 identical 589 diffs 0` between the `figma-rest` and `dtcg` exports.
- **`FACT` (5 deleted constants):** Grepping `BG_ELEVATED`, `BG_DISABLED`, `TEXT_ON_LIGHT_CANVAS_SECONDARY`, `TEXT_ON_DARK_CANVAS`, and `LOGO_GRADIENT_A` across `crates/designos-app/src --exclude=theme.rs` confirmed 0 references before deletion.
- **`FACT` (G01 Purple Ban falsification catch):** In `scripts/design_os_self_evolve.py` at `HEAD`, `check_rust_token_parity()` only checked `BANNED_PURPLES` against `CORE_RUST_CONSTANTS` (35 constants) and `ds_colors`, missing non-core `theme.rs` constants such as `SYNTAX_KEYWORD`. When we ran the required falsification test with `SYNTAX_KEYWORD` set to `#C792EA`, G01 initially returned `ok=True`. We fixed `check_rust_token_parity()` to check every parsed constant in `rust_consts` against `BANNED_PURPLES`, after which the `SYNTAX_KEYWORD=#C792EA` fixture failed as required (`ok=False | summary=purple_ban_violations=['SYNTAX_KEYWORD=#c792ea']`).
- **`ASSUMPTION` (Unwrapped soul/principles drafts):** `origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1142-{soul,principles}-draft.md` prepended an 11-line Spec-Driven v2 metadata header (`status: draft`) onto the verbatim files from `origin/docs/specs-untitled-ui-dark:docs/specs/ui-revamp-untitled-dark/1142-{soul,principles}-draft.md`. Because `ui ds soul check` requires `status: ratified` at line 1 and fails with `soul-draft-status` on the 11-line wrapper, we used the unwrapped body (which is byte-identical to `origin/docs/specs-untitled-ui-dark:docs/specs/ui-revamp-untitled-dark/1142-{soul,principles}-draft.md` and to the `pm/spec-ui-revamp` files with the 11-line pack header stripped), yielding `cmp` exit 0 and `ui ds soul check --dir .` → `0 findings`.

---

## 5. Skills Loaded & Actions Taken Because of Them

1. **`/es:dev` → `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed directly in the DEV phase using Spec A + Controller Rulings as the ratified plan, keeping scope strictly within the `Touches` row.
2. **`/es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept all new `theme.rs` constants zero-cost `const Color32` values, avoided any runtime allocation, and verified `cargo clippy --workspace --all-targets --locked -- -D warnings`.
3. **`/es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Preserved all canvas overlay, selection, ruler, and Dev-mode box-model colours unchanged (Controller Ruling 5) and kept `designos-gpu` / `designos-render` untouched.
4. **`es-designer`** (`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`): Ran `ui ds context --dir .` to verify the project's updated Design Soul and Principles (`11` Untitled references served) before editing `crates/designos-app/src/theme.rs`, and verified all 62 chrome contrast pairs (`C-1.1`..`C-1.62`) + `ui gate design/specimen.html`.
5. **`es:fable-thinking` & `es-astra-work`**: Applied red-first testing and falsification checks, which uncovered and fixed the `check_rust_token_parity` loop omission on non-core `theme.rs` constants.

---

## 6. Acceptance Criteria & Test Plan Verification (Exact Commands & Pasted Output)

### Summary Table (PM AC-01..AC-05 & Spec §10)

| ID | Criterion | Verdict |
|---|---|---|
| **AC-01** | `design/design.tokens.json` 41 colour roles re-valued via `ui ds change-token`; `ui ds status` → `gen 69, 77 tokens, 12 components`; `design/tokens.json` mirrored; `design/specimen.html` refreshed in place (`ui gate` & `ui ds-usage-lint` 0 errors) | **PASS** |
| **AC-02** | `theme.rs` 116 existing colour actions (`55` revalue, `52` keep, `4` alias, `5` delete) + `46` new constants (`157` total) match `token-contract.md (a)` line-by-line; `apply()` updated; `terminal.rs` pins updated | **PASS** |
| **AC-03** | Contrast check C-1 (`62/62` rows exit 0, falsification with C-1.26 at `4.5` exits 1) + Rust `chrome_contrast_pairs` (`62/62`), `brand_label_passes_aa`, `no_chrome_colour_is_purple`, `selection_fills_are_neutral` pass | **PASS** |
| **AC-04** | `design/soul.md` & `design/principles.md` byte-equal drafts (`cmp` exit 0); `ui ds soul check` (`0 findings`, `status: ratified`); `ui agents check` (`0 findings`); `.agents/agents/{curator,designer,librarian}*` updated | **PASS** |
| **AC-05** | `scripts/design_os_self_evolve.py` G01 `35/35` + `0 Purple Ban violations` (and `SYNTAX_KEYWORD #C792EA` fixture fails), G04 `50` explicit chrome pairs `0 below AA`, G05, G06, G08 pass; `scripts/check_chrome_colour_literals.py` + baseline + unit test pass; before/after screenshots + contact sheet captured | **PASS** |

---

### 6.1 Step 0b — Dogfood Kit Variable Export & Resolution Check (No Figma, R3)

```text
$ ./out/cargo/release/designos "out/untitled-ui-copy.fig" --export-variables out/kit/all.json --format figma-rest
$ ./out/cargo/release/designos "out/untitled-ui-copy.fig" --export-variables out/kit/dtcg --format dtcg
$ python3 out/resolve-untitled-tokens.py out/kit/dtcg out/kit/resolved
Dark mode 273
Light mode 273
Mode 1 40
Value 4
Style 343

$ python3 -B - <<'PY'
# Compare figma-rest resolved values across all 10 collections against dtcg resolved values
...
PY
compared 589 identical 589 diffs 0
```

---

### 6.2 Red-First Tests (Before Updating `theme.rs` Constants & Injected Literal Test)

#### (a) `theme::tests::{no_chrome_colour_is_purple, brand_label_passes_aa, chrome_contrast_pairs}` on `HEAD` `theme.rs` (RED)

```text
$ cargo test -p designos-app --bin designos --locked theme::tests::
running 22 tests
...
test theme::tests::brand_label_passes_aa ... FAILED
test theme::tests::chrome_contrast_pairs ... FAILED
test theme::tests::no_chrome_colour_is_purple ... FAILED

failures:

---- theme::tests::brand_label_passes_aa stdout ----
thread 'theme::tests::brand_label_passes_aa' panicked at crates/designos-app/src/theme.rs:998:13:
brand label on BG_BRAND fails AA: 3.14:1 < 4.5:1

---- theme::tests::chrome_contrast_pairs stdout ----
thread 'theme::tests::chrome_contrast_pairs' panicked at crates/designos-app/src/theme.rs:1005:9:
C-1.7 TEXT_TERTIARY on BG fails AA: 3.57:1 < 4.5:1

---- theme::tests::no_chrome_colour_is_purple stdout ----
thread 'theme::tests::no_chrome_colour_is_purple' panicked at crates/designos-app/src/theme.rs:986:13:
SYNTAX_KEYWORD (#C7_92_EA_FF) violates Purple Ban: hue=276.1°, sat=0.38

 failures:
    theme::tests::brand_label_passes_aa
    theme::tests::chrome_contrast_pairs
    theme::tests::no_chrome_colour_is_purple

test result: FAILED. 19 passed; 3 failed; 0 ignored; 0 measured; 2946 filtered out; finished in 0.00s
```

#### (b) `scripts/check_chrome_colour_literals.py` Unit Test (Injected `Color32::from_rgb(1, 2, 3)` Fails; Baseline Passes)

```text
$ python3 -B -m unittest scripts/tests/test_check_chrome_colour_literals.py -v
test_allowlisted_canvas_or_identity_line_is_exempt (test_check_chrome_colour_literals.TestCheckChromeColourLiterals.test_allowlisted_canvas_or_identity_line_is_exempt)
An allowlisted line does not count toward chrome_sites. ... ok
test_head_passes_against_baseline (test_check_chrome_colour_literals.TestCheckChromeColourLiterals.test_head_passes_against_baseline)
Repo HEAD passes against scripts/chrome_colour_baseline.json. ... ok
test_injected_colour_literal_fails_ratchet (test_check_chrome_colour_literals.TestCheckChromeColourLiterals.test_injected_colour_literal_fails_ratchet)
Injecting one Color32::from_rgb(1, 2, 3) into a temp copy fails the gate. ... ok

----------------------------------------------------------------------
Ran 3 tests in 0.629s

OK
```

---

### 6.3 Green `theme.rs` & `terminal.rs` Tests + Line-by-Line Contract Check

```text
$ cargo test -p designos-app --bin designos --locked theme::tests::
running 22 tests
test theme::tests::concentric_grows_for_a_negative_inset ... ok
test theme::tests::concentric_never_returns_nan ... ok
test theme::tests::a_group_sits_at_least_twice_its_inner_gap_from_its_neighbours ... ok
test theme::tests::concentric_subtracts_the_inset ... ok
test theme::tests::a_child_inset_never_exceeds_its_parent_inset ... ok
test theme::tests::concentric_clamps_at_zero_when_the_inset_reaches_the_arc ... ok
test theme::tests::menu_pills_inset_by_the_frame_inset ... ok
test theme::tests::only_three_ui_weights_exist ... ok
test theme::tests::radius_full_clamps_a_negative_height ... ok
test theme::tests::chrome_contrast_pairs ... ok
test theme::tests::the_ladder_never_inverts ... ok
test theme::tests::radius_full_is_half_the_height ... ok
test theme::tests::selection_fills_are_neutral ... ok
test theme::tests::the_monospace_role_is_the_bundled_roboto_mono ... ok
test theme::tests::control_heights_are_the_ratified_family ... ok
test theme::tests::type_roles_keep_their_order_and_the_canvas_size_is_the_only_one_below_the_chrome_floor ... ok
test theme::tests::nested_radii_are_concentric ... ok
test theme::tests::brand_label_passes_aa ... ok
test theme::tests::pill_radius_is_half_the_height ... ok
test theme::tests::line_heights_fit_their_roles ... ok
test theme::tests::ladder_insets_and_controls_sit_on_the_4px_grid ... ok
test theme::tests::no_chrome_colour_is_purple ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 2946 filtered out; finished in 0.00s

$ cargo test -p designos-app --bin designos --locked theme_has_info_success_warning_in_token_parity
running 1 test
test terminal::tests::theme_has_info_success_warning_in_token_parity ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2967 filtered out; finished in 0.00s

$ grep -cE '^pub const [A-Z0-9_]+: (Color32|\[Color32; ?[0-9]+\]) =' crates/designos-app/src/theme.rs
157

$ python3 -B - <<'PY'
# Line-by-line verification of all 116 existing + 46 new constants in token-contract.md (a) vs crates/designos-app/src/theme.rs
...
PY
Verified 116 existing ({'revalue': 55, 'keep': 52, 'alias': 4, 'delete': 5}) + 46 new = 157 active Color32 constants in theme.rs: 0 mismatches.
```

---

### 6.4 Contrast Check C-1 (62/62 Pass + C-1.26 Falsification at `4.5`)

```text
$ python3 - out/specs/token-contract.md <<'PY'
import re, sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
bad = 0
for m in re.finditer(r"^\| (C-1\.\d+) \| `\w+` #(\w{6}) \| `\w+` #(\w{6}) \| ([\d.]+) \|", open(sys.argv[1]).read(), re.M):
    hi, lo = sorted((lum(m[2]), lum(m[3])), reverse=True)
    r = (hi + 0.05) / (lo + 0.05)
    ok = r >= float(m[4]); bad += not ok
    print(f"{m[1]:7} {r:5.2f}:1 (min {m[4]}) {'ok' if ok else 'FAIL'}")
sys.exit(1 if bad else 0)
PY
C-1.1   18.97:1 (min 4.5) ok
C-1.2   17.18:1 (min 4.5) ok
C-1.3   14.50:1 (min 4.5) ok
C-1.4    9.93:1 (min 4.5) ok
C-1.5   13.36:1 (min 4.5) ok
C-1.6    6.99:1 (min 4.5) ok
C-1.7    7.85:1 (min 4.5) ok
C-1.8    7.11:1 (min 4.5) ok
C-1.9    6.00:1 (min 4.5) ok
C-1.10   7.06:1 (min 4.5) ok
C-1.11   6.40:1 (min 4.5) ok
C-1.12   7.16:1 (min 4.5) ok
C-1.13   6.48:1 (min 4.5) ok
C-1.14   5.73:1 (min 4.5) ok
C-1.15   7.06:1 (min 4.5) ok
C-1.16   5.56:1 (min 4.5) ok
C-1.17   4.83:1 (min 4.5) ok
C-1.18   6.47:1 (min 4.5) ok
C-1.19  14.50:1 (min 4.5) ok
C-1.20  10.21:1 (min 4.5) ok
C-1.21  10.62:1 (min 4.5) ok
C-1.22  11.06:1 (min 4.5) ok
C-1.23   8.51:1 (min 4.5) ok
C-1.24  12.09:1 (min 4.5) ok
C-1.25   4.18:1 (min 3.0) ok
C-1.26   3.78:1 (min 3.0) ok
C-1.27   6.00:1 (min 3.0) ok
C-1.28   3.78:1 (min 3.0) ok
C-1.29   7.06:1 (min 3.0) ok
C-1.30   7.96:1 (min 3.0) ok
C-1.31   5.78:1 (min 4.5) ok
C-1.32   8.97:1 (min 4.5) ok
C-1.33   8.55:1 (min 4.5) ok
C-1.34   6.92:1 (min 4.5) ok
C-1.35   6.82:1 (min 4.5) ok
C-1.36   6.82:1 (min 4.5) ok
C-1.37   8.79:1 (min 3.0) ok
C-1.38   6.72:1 (min 3.0) ok
C-1.39  10.21:1 (min 4.5) ok
C-1.40   6.55:1 (min 4.5) ok
C-1.41   9.89:1 (min 4.5) ok
C-1.42   6.82:1 (min 4.5) ok
C-1.43   5.48:1 (min 4.5) ok
C-1.44   9.59:1 (min 4.5) ok
C-1.45   7.85:1 (min 4.5) ok
C-1.46   6.18:1 (min 4.5) ok
C-1.47   5.74:1 (min 3.0) ok
C-1.48  18.97:1 (min 3.0) ok
C-1.49  15.53:1 (min 3.0) ok
C-1.50   7.67:1 (min 3.0) ok
C-1.51   7.85:1 (min 3.0) ok
C-1.52  14.06:1 (min 4.5) ok
C-1.53   7.85:1 (min 4.5) ok
C-1.54  17.18:1 (min 4.5) ok
C-1.55   7.85:1 (min 4.5) ok
C-1.56   7.81:1 (min 4.5) ok
C-1.57  13.36:1 (min 4.5) ok
C-1.58   6.00:1 (min 4.5) ok
C-1.59  10.42:1 (min 3.0) ok
C-1.60   5.25:1 (min 3.0) ok
C-1.61   6.88:1 (min 3.0) ok
C-1.62   8.57:1 (min 3.0) ok
```

#### Falsification — Raising `C-1.26` Minimum to `4.5`:

```text
$ python3 - out/specs/token-contract.md <<'PY' || echo "Exited with code $?"
... # C-1.26 min replaced with 4.5
PY
C-1.26   3.78:1 (min 4.5) FAIL
Exited with code 1
```

---

### 6.5 Token Store, Specimen, Soul, Principles & Agents Checks

```text
$ ui ds status --dir .
ds: designos-studio (gen 69)
persona: imported / imported
tokens:  77 (semantic + primitive)
components: 12

$ ui ds a11y --dir .
ds a11y: 15 text×surface pair(s) checked ({role}/{role}-foreground pairs) + 8 interaction-state pair(s), 0 below AA (4.5:1).

$ ui ds a11y --dir . --pairs "$(tr -d '\n' < design/a11y-chrome-pairs.txt)"
ds a11y: 50 text×surface pair(s) checked, 0 below AA (4.5:1).

$ ui gate design/specimen.html --tokens design/design.tokens.json --json
{"ok":true,"command":"gate","data":{"file":"design/specimen.html",...,"errorCount":0,"warningCount":0,"advisoryCount":0,"pass":true}}

$ ui ds-usage-lint design/specimen.html --dir . --json
{"ok":true,"command":"ds-usage-lint","data":{"file":"design/specimen.html","errorCount":0,"warningCount":0,"findings":[]}}

$ cmp -s design/soul.md out/specs/docs-soul-draft.md && echo "cmp design/soul.md: exit $?"
cmp design/soul.md: exit 0

$ cmp -s design/principles.md out/specs/docs-principles-draft.md && echo "cmp design/principles.md: exit $?"
cmp design/principles.md: exit 0

$ ui ds soul check --dir .
ds soul check: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1142-untitled-colours/design/soul.md — 0 findings.

$ ui agents check --dir .
agents check: 4 agent file(s) in /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1142-untitled-colours/.claude/agents — 0 findings.

$ grep -rniE '#(121215|18181b|1e1e22|232329|2e2e36|2b1d16)|Zinc' \
  .agents/agents/curator-jang-designos-studio.md \
  .agents/agents/designer-jang-designos-studio.md \
  .agents/agents/librarian-jang-designos-studio.md \
  .claude design/soul.md design/principles.md || echo "0 matches (exit $?)"
0 matches (exit 1)
```

---

### 6.6 G01 Parity & `SYNTAX_KEYWORD #C792EA` Falsification + Gates (`zero_fake_ui_gate.py`, `check_spacing_relationships.py`, `check_chrome_colour_literals.py`, `design-os evolution`)

```text
$ python3 -B - <<'PY'
import sys, tempfile, pathlib
sys.path.insert(0, "scripts")
import design_os_self_evolve as dse

ok0, sum0, _ = dse.check_rust_token_parity()
print(f"G01 normal: ok={ok0} | summary={sum0}")

orig = dse.THEME_RS.read_text(encoding="utf-8")
with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False, encoding="utf-8") as tf:
    tf.write(orig.replace("pub const SYNTAX_KEYWORD: Color32 = rgb(0xF472B6);", "pub const SYNTAX_KEYWORD: Color32 = rgb(0xC792EA);"))
    tmp_path = pathlib.Path(tf.name)
try:
    dse.THEME_RS = tmp_path
    ok1, sum1, det1 = dse.check_rust_token_parity()
    print(f"G01 fixture (SYNTAX_KEYWORD=#C792EA): ok={ok1} | summary={sum1} | purple_violations={det1.get('purple_violations')}")
finally:
    tmp_path.unlink(missing_ok=True)
PY
G01 normal: ok=True | summary=35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match
G01 fixture (SYNTAX_KEYWORD=#C792EA): ok=False | summary=purple_ban_violations=['SYNTAX_KEYWORD=#c792ea'] | purple_violations=['SYNTAX_KEYWORD=#c792ea']

$ python3 -B scripts/design_os_self_evolve.py
====================================================================================================================
 design:os studio — Self-Evolution Engine & Live Token Parity Audit (mode=EVOLVE)
====================================================================================================================
ID   | STATUS | GATE                     | EVIDENCE & VERDICT
--------------------------------------------------------------------------------------------------------------------
G01  | PASS   | check_rust_token_parity  | 35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match
G02  | PASS   | zero_fake_ui_gate        | 17/17 Zero-Fake-UI invariants PASS (DIR-28 / BR-31 / REQ-25)
G03  | PASS   | ui_ds_status             | DS manifest active (12 registered components, hash=sha256-_8lZw-38NH0N…)
G04  | PASS   | ui_ds_a11y               | 0 contrast failures across 15 foreground/surface pairs + 8 state pairs + 50 explicit chrome pairs
G05  | PASS   | ui_ds_soul_check         | 0 soul findings; design/soul.md status=ratified
G06  | PASS   | ui_agents_check          | 4 stamped .claude/agents + 4 cross-runtime .agents/agents fresh (0 errors)
G07  | PASS   | design_os_evolution      | verdict=ALIVE, proof.level=IMPROVING, findings=0, ledger_events=352, recurrent_insights=2 (harvest+heartbeat+compile)
G08  | PASS   | ui_gate_specimen         | 6/6 families PASS (errors=0, warnings=0); ds-usage-lint errors=0
G09  | FAIL   | design_os_project_audit  | audit_exit=1, targets=16, excluded=2, tools_run=83, tools_failed=0, audit_errors=1, failing=[docs/index.html[errors=1]], nodes_violations=0, flow_errors=0 ()

$ python3 -B scripts/zero_fake_ui_gate.py
Gate Verdict: PASS (17/17 invariants satisfied)

$ python3 -B scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline

$ python3 -B scripts/check_chrome_colour_literals.py
Chrome colour literal gate PASSED (50 files with findings, 406/406 baseline chrome sites).

$ design-os evolution
evolution ──────────────────────────────────────────────── ALIVE
  living-agent proof: IMPROVING (sources=4, applications=4, defects=2, comparisons=3)
  ledger diversity: 9 distinct type(s) over 352 event(s) — autofix_applied=1, component_registered=2, gap=3, harvested=3, insight=9, lint_run=330, taste_verdict=1, token_change=2, user_pick=1
  learning events: 9 insight(s), 3 gap(s)
  graph insights: 9 total, 2 recurrent (seen>1)
  soul: status=ratified, ratified=True, evidence citations=30
  heartbeat: wired (4 task(s)), fired, last run at 2026-10-26T17:12:56.698396+07:00
  taste votes: no votes.jsonl found
  DS role coverage: 40/77 tokens carry a design-os.role
  registry: 12 component(s) in component-registry.json, 2 component_registered event(s) in ledger
```

---

### 6.7 Mandatory Rust Test Harness Output

```text
$ cargo fmt --all -- --check
(exit 0)

$ cargo clippy --workspace --all-targets --locked -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) (exit 0, 0 warnings)

$ export PATH="/Users/jangtrinh/.local/bin:$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. (all workspace lib crates passed, 0 failed)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 2968 tests
...
test result: ok. 2960 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 186.43s
```

---

### 6.8 Visual Evidence (`plans/reports/agy-1142-untitled-colours-evidence/`)

All 8 desktop (`1600x1088`, `--no-toast`) **Before** (`HEAD` Zinc) and **After** (Untitled UI PRO v8.0 Dark) screenshots were captured on `$KIT_COPY` (`out/untitled-ui-copy.designos`) and assembled with the spec baseline into `plans/reports/agy-1142-untitled-colours-evidence/contact-sheet.png` (kept in the evidence folder, not committed to git per Controller Ruling 7):

- `plans/reports/agy-1142-untitled-colours-evidence/before/{home,editor-kit,panel-cmdk,panel-terminal,panel-assets,panel-variables,panel-picker,panel-dev-compare}.png`
- `plans/reports/agy-1142-untitled-colours-evidence/after/{home,editor-kit,panel-cmdk,panel-terminal,panel-assets,panel-variables,panel-picker,panel-dev-compare}.png`
- `plans/reports/agy-1142-untitled-colours-evidence/contact-sheet.png`
- `responsive: SKIPPED (owner default)`

---

## 7. Kernel Gaps `T1` and `T2` (For Controller Filing in `ease-design`)

1. **`T1` — No `add-token` / `delete-token` path in the sealed `ui ds` token store (`ease-design` kernel gap, related to dogfood Spec P #1140)**:
   - `ui ds` provides `change-token` to re-value existing keys in `design/design.tokens.json`, and `import --force` to overwrite the entire store (which wipes the 12 registered components in `component-registry.json`, resets generation to 1, and clears the changelog). There is no `ui ds add-token` command to add new Untitled semantic or utility roles (`bg-secondary_hover`, `text-quaternary`, `text-placeholder`, `bg-error-solid_hover`, `utility-*`, etc.) into an existing store without losing registry/history.
   - **Workaround applied in Spec A:** Re-valued the 41 existing colour role keys in place with `ui ds change-token` and declared the 46 additional Untitled roles as `theme.rs` constants guarded by `chrome_contrast_pairs` and `no_chrome_colour_is_purple`.
2. **`T2` — `ui ds preview` regenerates `design/specimen.html` with canonical `--color-*` / `--font-size-*` custom property names that fail `ui ds-usage-lint` on stores using `colors.*` / `typography.*` groups (`ease-design` kernel gap)**:
   - Running `ui ds preview` on `designos-studio`'s store produces 69 `ui ds-usage-lint` errors (`var(--color-ring)`, `var(--font-size-xs)` declared nowhere, plus raw hexes) because the store's top-level groups are plural (`colors.*`, `typography.*`) while the preview template emits singular `--color-*` references.
   - **Workaround applied in Spec A:** Refreshed the committed `design/specimen.html`'s printed custom properties, swatch hexes, WCAG ratios, `generation 69` header, and `min 4.54:1` summary in place; verified `ui gate` and `ui ds-usage-lint` pass with `0 errors, 0 warnings`.

---

## 8. Unresolved Questions

None.

Status: DONE
