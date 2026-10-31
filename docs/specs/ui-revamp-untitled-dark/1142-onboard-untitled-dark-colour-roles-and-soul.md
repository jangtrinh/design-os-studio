# Spec A — Onboard Untitled UI dark with the design:os CLI: colour roles, token store, `theme.rs` colours, soul, principles, design agents

| Field | Value |
|---|---|
| Issue | #1142 (epic #1118, spec key A) |
| Size | M. About two days: 41 token re-values by script, ~70 `theme.rs` colour lines, two gate scripts edited, one gate script added, soul and principles replaced, four agent files edited, tests, evidence. |
| Depends on | **#1164 (R12) lands first, in its own small PR** (owner 2026-10-28): it re-seals `design/design.tokens.json`, which #1106 (`b598a499`) hand-edited (`spacing.rail-width` 56px → 48px without `change-token`), so at `8dfaee93` (re-run 2026-10-28) `ui ds status` and `ui ds change-token` still exit 1 with "tokens file hash mismatch" and G03 is red (`evidence/AB-fix-round-261028/1164-store-seal-at-8dfaee93.txt`). Step 0c of 5.1 checks it. The values need nothing else: they are verified against both copies of every duplicated kit collection (section 2, token-contract.md section 0); spec P #1140 makes the DTCG export repeatable, and until it lands step 0b of 5.1 is the check. |
| Unblocks | B (same files, next PR), C–K (they name colours by Untitled token; this spec maps them to constants), M (copies its merged table) |
| Touches | `crates/designos-app/src/theme.rs` (colours :20-112, `apply()` :626-664, tests :777-920), `crates/designos-app/src/terminal.rs` (test `theme_has_info_success_warning_in_token_parity` :10727-10745: theme pins :10728-10730, store pins :10743-10745), `crates/designos-app/src/cmdk.rs`/`dev/list.rs` (none; those two are B's), `design/design.tokens.json`, `design/ds.manifest.json`, `design/tokens.json`, `design/specimen.html`, `design/DESIGN.md` (regenerated), `design/component-registry.json` (`Button/Primary`, `Button/Secondary` re-registered, 5.1 step 3), `design/studio-ui-nodes.json` (fill hex rewritten from each `boundToken`, 5.1 step 3), new `design/a11y-chrome-pairs.txt` (new name, 5.3), `design/soul.md`, `design/principles.md`, `scripts/design_os_self_evolve.py` (:109, :111-134, :137-158, :160-163, G04 :365, new `purple_band_constants`), new `scripts/check_chrome_colour_literals.py` (new name) + `scripts/chrome_colour_baseline.json` (new name) + its test, `.agents/agents/{curator,designer,figma,librarian}-jang-designos-studio.md` |

Code baseline: `8dfaee93` (`origin/main` with v0.7.0 merged, 2026-10-28), the implementation base; every cited line was re-read there (the earlier anchors were `ec47da26` and `d0555c82`). Studio and `ui` runs use v0.7.0 (`plans/261025-1247-untitled-ui-dark-revamp/tools/designos-latest`); code is cited at `origin/main`, not at a release branch. The token store is unsealed since #1106, hence #1164 above. Re-anchor every cited line against HEAD and report drift before writing code; `token-contract.md` (g) check D-1 does it for the contract's tables.

**Drift note (`d0555c82` → `8dfaee93`, 215 files under `crates/designos-app/src` changed; checked 2026-10-28).** `theme.rs` is byte-identical, so every theme line, HEAD-value cell, `apply()` line (:626-664) and test line (:777-920) holds. `scripts/design_os_self_evolve.py`, `scripts/non_colour_token_parity.py`, `scripts/check_spacing_relationships.py`, `scripts/spacing_baseline.json`, `design/` and `.agents/` are identical too. What moved: `terminal.rs` (+2350 lines) put the colour-pin test at :10727-10745 and the provider marks at :4781-4799 (same four colours); the chrome-literal census is 406 sites in 53 files (was 408 in 52 when re-counted at `d0555c82`; top `terminal.rs` 50, `specs.rs` 42, `home/mod.rs` 38, `canvas/draw.rs` 27, `panels/properties.rs` 20, `panels/assets.rs` 18); the five dead constants still have 0 references. No rule of this spec is broken by the drift. Rules this round changed against the branch copy (draft PR #1207, which was built from `1142-onboard-untitled-dark-colour-roles-and-soul.md` on `docs/specs-untitled-ui-dark`) are listed in the changelog's last block.

The value tables live in `specs/token-contract.md` (this folder); this spec says how to apply them and how to prove them. Kit values come from the commands listed in token-contract.md section 0; the raw kit export is never committed (paid kit, public repo).

## 1. Summary and user story

As the owner, I want Studio's chrome to wear the Untitled UI PRO v8.0 dark colour system with Studio orange as the brand (R1, R2, R4), onboarded with Studio's own tools and the design:os CLI, never Figma (R3), so that every surface spec (C–K) can name a colour by its Untitled token and get one exact Rust constant and one token-store value.

As a design agent (designer, curator, figma hand, librarian), I want `ui ds context` to hand me the new soul, the new principles and the re-valued token store, so that I judge and generate against Untitled dark, not the retired Zinc palette.

Outcome: `theme.rs` colour constants and `design/design.tokens.json` hold the Untitled dark values in the token-contract mapping; G01 parity, the Purple Ban, AA on every chrome text×surface pair, the soul check and the agents check are green; the design agents pick the new stance up without being regenerated.

## 2. Current state (FACT unless labelled)

**Colours in code.** `theme.rs` declares 116 `Color32` colour constants (`grep -cE '^pub const [A-Z0-9_]+: (Color32|\[Color32; ?[0-9]+\]) =' theme.rs`) on the Zinc palette: `BG` `#121215` (:22), `BG_PRIMARY` `#18181B` (:23), `BG_SECONDARY` `#1E1E22` (:24), `BG_HOVER` `#232329` (:26), `BG_PRESSED` `#2E2E36` (:27), `BG_SELECTED = BG_PRESSED` (:31), `TEXT_SECONDARY` white at 70% (:51), `TEXT_TERTIARY` white at 40% (:52), `DANGER` `#FE5938` (:60), `SUCCESS` `#14AE5C` (:62), `WARNING` `#F59E0B` (:63). `apply()` (:626-664) maps egui visuals: `panel_fill = BG`, `window_fill = BG_MENU`, `extreme_bg_color`/`faint_bg_color = BG_SECONDARY`, `selection.bg_fill = BG_TEXT_SELECTION`, `selection.stroke = BORDER_SELECTED`, widgets inactive `BG_SECONDARY`, hovered `BG_HOVER`, active `BG_PRESSED`. Five colour constants have 0 references outside `theme.rs`: `BG_ELEVATED`, `BG_DISABLED`, `TEXT_ON_LIGHT_CANVAS_SECONDARY`, `TEXT_ON_DARK_CANVAS`, `LOGO_GRADIENT_A` (`grep -rnw <NAME> crates/designos-app/src --exclude=theme.rs`).

**Purple Ban breach at HEAD.** `SYNTAX_KEYWORD` (:564) is `#C792EA`, hue 276°, saturation 0.68: violet. The G01 list `BANNED_PURPLES` (`design_os_self_evolve.py:109`) holds only `#9747ff`, `#8755e9`, `#c4a1ff`, `#a855f7`, so no gate sees it. A hue scan of every `theme.rs` literal (band 255–300°, saturation > 0.25) finds exactly `SYNTAX_KEYWORD` and the board content colour `BOARD_PALETTE` `#D5C3FF`.

**Literal colours outside `theme.rs`.** 406 chrome sites in 53 files at `8dfaee93` (census with spec D's regex, comments blanked and `#[cfg(test)]` dropped by `scripts/rust_literal_scan.clean`; top: `terminal.rs` 50, `specs.rs` 42, `home/mod.rs` 38, `canvas/draw.rs` 27, `panels/properties.rs` 20, `panels/assets.rs` 18). No gate counts them; specs D, E, F and G ask this spec for one (`check_chrome_colour_literals.py`, new name).

**Token store.** `design/design.tokens.json`: 48 colours, 8 typography, 12 spacing, 6 radius, 3 durations (77 tokens, `ui ds status`: generation 28 while sealed, before #1106; at `8dfaee93` `ui ds status` still exits 1 with "tokens file hash mismatch" because #1106 hand-edited `spacing.rail-width`, and the #1164 re-seal through `change-token` makes it generation 29, measured 2026-10-28). Each colour carries `$extensions.design-os.role`. `design/ds.manifest.json` names the store `designos-studio`. `design/tokens.json` is the flat file it was imported from (no script reads it; `grep -rn "design/tokens.json" scripts crates` finds nothing).

**Gates.** G01 (`design_os_self_evolve.py`) parses only literal `pub const X: Color32 = rgb(0x……)` (:223-225) for 22 `CORE_RUST_CONSTANTS` (:111-134) through `RUST_TO_DESIGN_TOKEN_MAP` (:137-158), where `DEV_READY`→`success` (:149) and `SUCCESS`→`success` (:156) collide. G04 (:365) runs `ui ds a11y` with no pairs: it checks the 15 `{role}/{role}-foreground` pairs + 8 state pairs, not the chrome text×surface pairs. G05 (:380) needs `ui ds soul check` with 0 findings and `status: ratified`. G06 (:395) runs `ui agents check` and checks that the four `.agents/agents/` files exist (content is not checked). G08 (:476) runs `ui gate design/specimen.html` and `ui ds-usage-lint` on the committed specimen.

**Design agents.** `.claude/agents/{designer,curator,figma}-jang-designos-studio.md` bake no palette values; their first action is `ui ds context`. `.agents/agents/` (cross-runtime copies) bake Zinc values: `curator…md:20-23` (D2 field 24px, D3 10/11/13 px, D4 `#121215`/`#18181B`/`#1E1E22`, D5 hover `#232329`, pressed `#2E2E36`, selected `#2B1D16`), `designer…md:16` (Purple Ban list of four), `librarian…md:23` ("14 core constants"). `ui agents check --dir .` on the repo: "0 findings".

**Onboarding without Figma (R3), measured in scratch copies only** (runs made before #1106 and #1108; the procedure of 5.1 was re-run at `8dfaee93` on 2026-10-28 with the results pasted there; `<scratch>` = this agent's scratchpad; nothing below touched the repo):

| Run | What | Result |
|---|---|---|
| E1 | `ui ds import "<kit export>/1_ Color modes.Dark mode.tokens.json" --dir <scratch>/e1` (raw DTCG) | 295 tokens (273 colour, 22 number), 54 categories, 82 roles recognised, 8 gaps (`card`, `popover`, `secondary`, `muted`, `accent`, `input`, `info`, `neutral`), 132 sub-fields skipped, 233 aliases left dangling (`{_Primitives…}`); `ui ds a11y`: "0 contrast pairs measurable … (false-green risk)" |
| E2 | resolve aliases (`evidence/resolve-untitled-tokens.py`), flatten to `{category:{name:value}}`, `ui ds import <flat>.json --dir <scratch>/e2` | 107 tokens (78 colour, 29 dimension), 74 roles recognised, 12 gaps; `_` becomes `-` (`bg-primary_hover` → `bg-primary-hover`); `border-primary`/`border-secondary` get no role; default `ui ds a11y` builds a cartesian product on Untitled names: 1392 pairs, 731 "failing", meaningless without explicit `--pairs` |
| proj-r | `ui ds import … --force` onto a copy of the real store | refused: "would wipe 12 registered component(s) … pass --reset-registry"; with `--reset-registry`: generation 1, changelog 0, 0 components (soul and principles untouched) |
| proj-v | re-value in place on a copy of the real store: 41 × `ui ds change-token colors.<key> --value <hex> --reason "<Untitled token>: …"` (token-contract.md (b)) | 41/41 succeeded; generation 28 → 69, changelog 27 → 68 entries; every `$extensions.design-os.role` kept; 12 components kept |
| e3 | reference store with Untitled names and Studio orange (44 tokens), `ui ds a11y --pairs <94 pairs>` | 94 pairs, 14 below AA, all of them placeholder `#737373`, error `#EF4444`, tertiary or accent text on `bg-quaternary`/`bg-tertiary`; every other chrome pair passes (rules 5–8 below) |

**Duplicated kit collections (FACT).** `--export-variables --format dtcg` overwrites collections that share a name (spec P, #1140): the kit has 10 collections, three of them twice (`_Primitives`, `1. Color modes`, `6. Typography`, a local copy and a library copy each), so the DTCG files keep only one copy. Verification run (scratch): `designos "$KIT" --export-variables all.json --format figma-rest` keeps all 10 by id; resolving every variable of both copies (Dark mode / Style, alias chains followed) and diffing against the DTCG-resolved values this spec uses printed "compared 575 identical 575 diffs 0". The library copies hold 6 variables, each with an identical local twin, and the local copies (343 and 273 variables) are the ones the DTCG files kept. Every hex in token-contract.md is therefore verified. A second, independent check against the live kit in Figma (R9, read-only) is in `evidence/figma-live-variable-check.md`: colour variables 273/273 identical in Dark mode and 273/273 in Light mode, numeric variables 76/76. Studio's export lacking 31 of 32 `6. Typography` variables is spec P's (#1140). The overwrite did drop the local `6. Typography` (32 variables, the whole text scale) behind a one-variable library copy; spec B reads it from the REST export.

Conclusion (INFERENCE from the runs): `ui ds import` cannot onboard the kit onto this project without losing the component registry and changelog, and its raw DTCG path measures nothing; re-valuing the existing role keys with `change-token` is the only path that keeps history, roles and components. Tool gaps and owners are in section 8.

## 3. Behaviour rules

1. **Values.** Every `theme.rs` colour constant takes the action and value in token-contract.md (a): 55 re-valued, 52 kept, 4 aliases kept, 5 deleted, 77 added (new names; N14–N23, N38 and N74–N77 are for spec C, N24–N37 close contract gaps G-1..G-3, N39–N46 are the R10.2 Home tiles, N47–N73 are the names the D–G round sent: Dev hover, HUD pointer, playground dots, content-overlay constants and the 15 `READER_LIGHT_*` names). Every `design.tokens.json` colour takes token-contract.md (b): 41 re-valued, 7 kept.
2. **Literals for parity.** Every constant in the G01 map is a plain `rgb(0x……)` / `rgba(…)` literal. `BG_SELECTED` stops being `= BG_PRESSED` and becomes `rgb(0x262626)`.
3. **egui visuals in `apply()`.** `panel_fill = BG` (`#0A0A0A`), `window_fill = BG_MENU` (`#0A0A0A`), `extreme_bg_color = BG` (the text field fill is the kit `Input field` fill `bg-primary`; controller ruling C-F), `faint_bg_color = BG_SECONDARY`, widgets `inactive = BG_SECONDARY`, `hovered = BG_SECONDARY_HOVER` (new name, `#262626`; `BG_HOVER` equals `BG_SECONDARY` in the kit and would make hover invisible), `active = BG_PRESSED` (`#404040`), `selection.bg_fill = BG_TEXT_SELECTION` (`#431407`), `selection.stroke = BORDER_SELECTED` (`#FB923C`), widget stroke `BORDER_STRONG` for resting text fields, selects, secondary buttons and unchecked checkboxes and radios (kit `border-primary` `#404040`; named exception E-2, rule 20). Window and popup shadows: `Shadow::NONE` (kit dark shadows are alpha 0).
4. **Brand (R4).** `BG_BRAND` stays `#E36323`; hover `#F97316`, pressed `#EA580C`; the label on all three is `TEXT_ON_BRAND` `#0A0A0A` (5.73 / 7.06 / 5.56:1); the focus ring is `FOCUS_RING` `#F97316` for every focusable control except a field in its error state and the destructive buttons, which take `FOCUS_RING_ERROR` `#EF4444` (ruling 2026-10-28; N74, C-1.69). The kit's purple brand values never enter the code.
5. **Pressed fill.** On `BG_PRESSED` (`bg-quaternary` `#404040`) only `TEXT` and `TEXT_SECONDARY` may sit (`text-tertiary` 4.11:1, accent 3.70:1, placeholder 2.19:1 fail).
6. **Error text.** Text uses `TEXT_ERROR` `#F87171`; `DANGER` `#EF4444` is for icons, dots and strokes only (4.02:1 on `bg-tertiary`).
7. **Placeholder (R8.1).** `TEXT_PLACEHOLDER` (new name) is the kit's `text-placeholder` `#737373`. Owner ruling R8.1 makes placeholder the one named exception to the AA text law (E-1, floor 3:1, rows E-1.a/b in token-contract.md (f)). Text fields are filled `BG` (bg-primary, kit `Input field`, ruling C-F), so the placeholder reads 4.18:1; a field inside a `bg-secondary` card reads 3.78:1. Never on `BG_TERTIARY` (3.19:1) or `BG_PRESSED` (2.19:1). E-1 is placeholder-only (controller ruling C-R): the Inspector's "Mixed" value is not a placeholder and uses `TEXT_TERTIARY` `#A3A3A3` (passes AA, C-1.7 and C-1.8).
8. **Non-text graphics.** `ICON_QUATERNARY` and `VAR_SCROLL_THUMB` (`#737373`) need 3:1 and never sit on `BG_PRESSED` (2.19:1).
9. **Disabled.** No disabled colour token (v8 has none); a disabled control draws at opacity 0.5 (kit `Buttons/Button` State=Disabled). `TEXT_DISABLED` = `text-secondary` at 50% for text-only cases.
10. **No shadows.** `SHADOW_COLOR`, `SHADOW_COLOR_STRONG`, `ELEVATION_400_SHADOW`, `ELEVATION_400_TOP` become transparent; the surface specs drop the draw calls. `ELEVATION_400_RING` and `INNER_HIGHLIGHT` become the `border-secondary_alt` hairline `#262626`.
11. **Purple Ban.** No chrome colour in `theme.rs` has hue 255–300° with saturation > 0.25; `BOARD_*` (board content) is exempt. `SYNTAX_KEYWORD` becomes `utility-pink-600` `#F472B6`. `BANNED_PURPLES` gains `#a259ff`, `#c792ea`, `#7f56d9`, `#9e77ed`, `#b692f6`, `#6941c6`.
12. **Unchanged.** Canvas overlays, Dev overlay, info family, component cyan, agent presence, avatar, checker, board and the `u32` document defaults keep their values (controller defaults; token-contract.md (e)).
13. **Colour-literal gate.** New `scripts/check_chrome_colour_literals.py` (new name) counts chrome sites per file with spec D section 2's regex over `rust_literal_scan.clean` output, against `scripts/chrome_colour_baseline.json` (new name, written once from HEAD: 410 sites in 51 files). A file's count may only fall; an allowlist names identity-tint and canvas-exception lines (D, F and G list theirs). It joins the pre-commit list next to `check_spacing_relationships.py`. This spec retires no call-site literal; surface specs do.
14. **Store mirrors.** `design/tokens.json` is rewritten from `design.tokens.json` values (same keys) so no file prints a retired value. `design/specimen.html` is not regenerated with `ui ds preview` (section 8, gap T2); its printed values are refreshed in place.
15. **Agents.** No `ui agents init --force`: the project name (`designos-studio`) and studio name do not change, and `.claude/agents/*` read the soul at run time through `ui ds context`. The `.agents/agents/` copies are hand-edited (section 5.4).
16. **Contract gaps closed (token-contract.md (a) N24–N37, (c)).** Status badges use the kit `Badge` anatomy (utility-<colour>-50 fill, -200 stroke, -700 text): `BG_SUCCESS_PRIMARY` `#052E16` / `UTILITY_GREEN_200` `#166534` / `UTILITY_GREEN_700` `#86EFAC`, warning `#422006` / `#854D0E` / `#FDE047`, error `#450A0A` / `#991B1B` / `#FCA5A5` (G-1). Provider marks (Claude `#D97757`, Codex/OpenCode `#FAFAFA`, Grok `#E5E5E5`, Gemini/Antigravity `#60A5FA`) are identity content like `AVATAR_COLORS`: exempt from role mapping, declared once in `theme.rs` as `PROVIDER_*` (new names) and bound by the Purple Ban (G-2). The modal scrim is `BG_OVERLAY` = bg-overlay `#262626` at 70% (`rgba(0x262626, 0xB3)`); the kit adds a background blur, Figma `BACKGROUND_BLUR` radius 16, which Figma's own CSS export writes as `backdrop-filter: blur(8px)` (Gaussian sigma 8); specs I, J and K draw no blur (G-3). `RADIUS_XL` 12 (radius-xl) is spec B's (G-4). Sources: resolved export plus `evidence/figma-live-variable-check.md`; the 70% layer opacity and 16 px blur were read in live Figma (R9, read-only `figma-agent exec-js`: Modals page, node "Background overlay", fill bound to `Colors/Background/bg-overlay`, opacity 0.70, `BACKGROUND_BLUR` 16, `getCSSAsync` → `blur(8px)`).
17. **Tooltip.** Fill `BG_PRIMARY_SOLID` `#262626`, title `TEXT` `#FAFAFA` (14.50:1), supporting text `TEXT_SECONDARY` `#D4D4D4` (10.21:1). This overrides the kit's `text-white` tooltip label (controller ruling C-T, recorded in plan.md; a deviation from R2); `TEXT_WHITE` stays reserved for `BG_ERROR_SOLID` and `BG_ERROR_SOLID_HOVER`.
18. **Destructive hover (R8.7).** `BG_ERROR_SOLID_HOVER` is Red-700 `#B91C1C` (white label 6.47:1); the default stays `BG_ERROR_SOLID` `#DC2626` (4.83:1). R8.7 governs the solid `Button/Destructive` only: the secondary destructive hover follows the kit tint `BG_ERROR_PRIMARY` with the `TEXT_ERROR_HOVER` label (controller ruling in plan.md, C/M rulings; C-1.65).
19. **Contrast check C-1.** The acceptance check for contrast in every spec is token-contract.md (f) "Contrast check C-1": 70 pairs plus 4 exception floor rows (E-1.a/b placeholder, E-2.a/b control boundary), WCAG 2.x. The snippet reads every value from `theme.rs` (aliases followed, `rgba()` made opaque on `BG` as egui renders it), exits 1 when a ratio is under its minimum, when a row's hex differs from `theme.rs`, when a name is missing, or when a row does not parse (row-count guard), and never counts a floor row as a pass. The Rust test `chrome_contrast_pairs` asserts the same rows on the constants. Its companion D-1 (token-contract.md (g)) proves that the contract's line numbers, HEAD-value cells and appendix still describe the HEAD `theme.rs`. `ui ds a11y` is supplementary evidence only, never an acceptance gate (single-mode imports measure 0 pairs, a recorded false green).
20. **Board-3 answers (R10).** Home category tiles use `UTILITY_<HUE>_50` / `UTILITY_<HUE>_600` for blue, amber, green and orange (R10.2); the Dev mode accent is `DEV_READY` `#14AE5C` with the `TEXT_ON_BRAND` label, 6.82:1 (R10.4); the selected Home card border is 2 px `BORDER_SELECTED` (R10.3); the second named exception after the placeholder, to the 3:1 non-text rule, is E-2: the kit's 1 px `border-primary` `BORDER_STRONG` `#404040` (1.91:1 on `bg-primary`, 1.73:1 on `bg-secondary`) as the resting boundary of exactly these controls: unchecked checkbox and radio (owner R10.7), text field and select, and secondary button (controller ruling C-F, from R2 and R10.7; the owner may reverse it), only on `bg-primary` or `bg-secondary` (contract (f) E-2, floors E-2.a/b).
21. **Staged constants.** A new constant that no call site uses yet (the `UTILITY_*`, `PROVIDER_*`, spec C roles and the D–G names N47–N73) carries `#[allow(dead_code)]`, the existing `theme.rs` pattern (:45, :64), so the build adds no `never used` warning (the first review counted 60 on an A+B scratch build without it; the 2026-10-28 scratch builds carry the attribute and print none); the surface spec that first uses one removes the attribute in the same PR. CONTRIBUTING.md:29 asks for `cargo clippy --workspace --all-targets -- -D warnings` locally (CI runs clippy without it), so the attribute is not optional hygiene. **Frozen constants that lose their last reader are deleted, not allowed:** `PAGES_SEARCH_DX` (E) and `RIGHT_PRESENT_FROM_RIGHT` (F) keep their values until the D–G PRs remove their last non-test reader; the PR that does so deletes the constant (no `#[allow(dead_code)]`; a test that reads it does not count) and its appendix cell in token-contract.md; `RIGHT_PRESENT_CHEVRON_W` is still read and stays.
22. **Names and credit (R14).** The design store (token keys, `ui ds change-token` reasons, registry text, `DESIGN.md`, soul, principles) and `theme.rs` use Studio's own names and values; they carry no kit file and no Untitled icon (R6). Untitled role names (`bg-primary_hover`, `fg-quaternary`) appear only in this spec and in `token-contract.md`, as a review aid. The only shipped mention is one credit sentence in the soul and one in the principles (both drafts carry it): the chrome's colour, type, spacing and radius steps are adapted from the Untitled UI design system, with its address. Jang confirms the licence question with Untitled UI in writing (R14; spec Q 7.2); until then nothing here claims permission. Check (5.1 step 4): `grep -rniE 'untitled ?ui|untitledui'` over `design/design.tokens.json`, `design/tokens.json`, `design/ds.manifest.json`, `design/DESIGN.md`, `design/studio-ui-nodes.json`, `design/component-registry.json`, `design/specimen.html` and `theme.rs` prints nothing.

## 4. Data model / schema changes

No schema change. `design.tokens.json` keeps its 48 colour keys and their roles; 41 values change, generation = start + 41 (41 changelog entries, one per token, each reason in Studio's wording, R14), then + 2 registry records (5.1 step 3a). `theme.rs` loses 5 constants and gains 77 (new names): `BG_SECONDARY_HOVER`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY_HOVER`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `ICON_QUATERNARY`, `TEXT_ERROR`, `TEXT_WARNING`, `TEXT_SUCCESS`, `TEXT_ON_BRAND`, `FOCUS_RING`, `BG_ERROR_SOLID`, `TEXT_WHITE`, and for spec C `ICON_BRAND_PRIMARY` (fg-brand-primary, R4 orange `#F97316`), `ICON_SECONDARY_HOVER`, `ICON_QUATERNARY_HOVER`, `ICON_WHITE`, `BG_PRIMARY_ALT`, `BG_PRIMARY_SOLID`, `BG_ERROR_SOLID_HOVER`, `UTILITY_NEUTRAL_50`, `UTILITY_NEUTRAL_200`, `UTILITY_NEUTRAL_700`, and for the contract gaps `BG_SUCCESS_PRIMARY`, `BG_WARNING_PRIMARY`, `BG_ERROR_PRIMARY`, `UTILITY_GREEN_200`, `UTILITY_YELLOW_200`, `UTILITY_RED_200`, `UTILITY_GREEN_700`, `UTILITY_YELLOW_700`, `UTILITY_RED_700`, `BG_OVERLAY`, `PROVIDER_CLAUDE`, `PROVIDER_CODEX`, `PROVIDER_GROK`, `PROVIDER_GEMINI`, then `BG_ERROR_SOLID_PRESSED` (N38) and for R10.2 `UTILITY_BLUE_50`, `UTILITY_BLUE_600`, `UTILITY_AMBER_50`, `UTILITY_AMBER_600`, `UTILITY_GREEN_50`, `UTILITY_GREEN_600`, `UTILITY_ORANGE_50`, `UTILITY_ORANGE_600` (N39–N46), and for specs D–G `DEV_READY_HOVER`, `HUD_POINTER_FILL`, `PLAYGROUND_GRID_DOT`, `SWATCH_PLACEHOLDER`, `TYPE_PREVIEW_BASELINE`, `CONTENT_HANDLE_FILL` (alias of `CANVAS_HANDLE_FILL`), `CONTENT_HANDLE_RING`, `CONTENT_HANDLE_CASING`, `CONTENT_HANDLE_AXIS`, `CONTENT_HANDLE_STOP_IDLE`, `CONTENT_MARK_SHADOW`, `CONTENT_MARK_LIGHT` and `READER_LIGHT_BG_PAGE`, `_DOC_SHEET`, `_SURFACE`, `_SURFACE_2`, `_SURFACE_3`, `_CODE_BG`, `_BORDER`, `_HAIRLINE`, `_TEXT`, `_TEXT_MUTED`, `_TEXT_SUBTLE`, `_DOT_OK`, `_DOT_WARN`, `_DOT_NEUTRAL`, `_HIGHLIGHT` (N47–N73), and for spec C's error roles `FOCUS_RING_ERROR`, `BORDER_ERROR_SUBTLE`, `BORDER_ERROR`, `TEXT_ERROR_HOVER` (N74–N77; 77 in all). `theme.rs` also gains four numeric names in the surface specs' PRs (`DEV_STATUS_TINT`, `DEV_TINT_REST`, `DEV_TINT_HOVER`, `TYPE_LABEL_COMPONENT_ALPHA`) and spec D's `DRAG_DIVIDER_HIT`: token-contract.md (c), not A's. Untitled names with no store key live in `theme.rs` only (the store has no add path, kernel gap T1).

## 5. Contracts

### 5.1 Onboarding procedure (verified end to end in scratch on `design/` of `8dfaee93`, 2026-10-28; run it on the PR branch)

```bash
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
# 0. Kit values (local only, never committed): see token-contract.md section 0.
# 0b. Verify the dark values against both copies of each duplicated collection (until #1140 lands):
#     cargo run -q --release -p designos-app --bin designos -- "$KIT" --export-variables /tmp/kit/all.json --format figma-rest
#     resolve every variable of each '1. Color modes' / '_Primitives' copy by collection id (Dark mode / Style,
#     aliases followed up to 16 hops) and diff with the values in token-contract.md; expect 0 diffs
#     (scratch run 2026-10-25: "compared 575 identical 575 diffs 0"). After #1140: diff the two DTCG bundle files instead.
# 0c. Precondition (#1164): the store is sealed. Stop if this exits 1 ("tokens file hash mismatch").
ui ds status --dir . || exit 1
START=$(python3 -c 'import json;print(json.load(open("design/ds.manifest.json"))["generation"])')
# 1. Re-value the store in place, one sanctioned mutation per token.
#    colours.tsv / scale.tsv = the tsv blocks in token-contract.md (b) and (c); B runs the scale loop.
while IFS=$'\t' read -r key value reason; do
  ui ds change-token "colors.$key" --value "$value" --reason "Studio dark chrome palette: $reason" --dir . || exit 1   # Studio's wording only (R14)
done < colours.tsv
# 2. Mirror the flat source file.
python3 -c 'import json;t=json.load(open("design/design.tokens.json"));f=json.load(open("design/tokens.json"));[f["colors"].__setitem__(k,t["colors"][k]["$value"]) for k in f["colors"]];json.dump(f,open("design/tokens.json","w"),indent=2);open("design/tokens.json","a").write("\n")'
# 3. Refresh every file that prints a colour value (ui gate and ds-usage-lint cannot see printed values).
#    3a. Registry: the two records whose text names retired Zinc values, re-registered with the same category,
#        markup, tokens and states (each register adds one changelog entry and one generation).
printf '<button class="btn-primary" type="button">Create New File</button>\n' | ui registry register Button/Primary --category action --markup - --tokens colors.primary,colors.primary-foreground,colors.primary-hover,colors.primary-active,colors.ring,spacing.space-3,spacing.space-2,radius.radius-md,typography.font-family-sans,typography.font-size-body,duration.duration-fast --variants Size=28px --states default,hover,active,focus,disabled --description 'Studio orange primary CTA button: colors.primary #E36323 fill (R4), colors.primary-foreground dark label, colors.ring focus ring' --force
printf '<button class="btn-secondary" type="button">Import File</button>\n' | ui registry register Button/Secondary --category action --markup - --tokens colors.secondary,colors.secondary-foreground,colors.secondary-hover,colors.secondary-active,colors.border-strong,spacing.space-3,spacing.space-2,radius.radius-md,typography.font-family-sans,typography.font-size-body,duration.duration-fast --variants Tone=Neutral --states default,hover,active,focus,disabled --description 'Neutral secondary action button: colors.secondary fill, colors.secondary-foreground label, 1px colors.border-strong boundary (E-2)' --force
#    3b. Component docs, regenerated from the registry and the store.
ui ds docs --dir . --out design/DESIGN.md
#    3c. design/studio-ui-nodes.json: set every fills[].hex to the current value of its boundToken.
python3 - <<'PY2'
import json
tok = json.load(open("design/design.tokens.json")); path = "design/studio-ui-nodes.json"; nodes = json.load(open(path))
def walk(x):
    if isinstance(x, dict):
        for f in x.get("fills", []) + x.get("strokes", []):
            if isinstance(f, dict) and f.get("boundToken"):
                g, k = f["boundToken"].split(".", 1); f["hex"] = tok[g][k]["$value"].lower()
        for v in x.values(): walk(v)
    elif isinstance(x, list):
        for v in x: walk(v)
walk(nodes); json.dump(nodes, open(path, "w"), indent=2); open(path, "a").write("\n")
PY2
#    3d. Specimen (no ds preview, gap T2): rewrite every `  --colors-<key>: <hex>;` line, each swatch
#        `colors.<bg> / colors.<fg><br><code>#bg / #fg &middot; r:1</code>`, the `generation N` header and `min r:1`
#        from design.tokens.json + ds.manifest.json.
python3 - <<'PY3'
import json, re
tok = json.load(open("design/design.tokens.json")); man = json.load(open("design/ds.manifest.json"))
html = open("design/specimen.html").read(); val = lambda g, k: tok[g][k]["$value"].lower()
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]; c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
def ratio(a, b): hi, lo = sorted((lum(a), lum(b)), reverse=True); return (hi + 0.05) / (lo + 0.05)
html, n_root = re.subn(r"^(  --colors-([a-z0-9-]+): )#[0-9a-fA-F]{6};", lambda m: f"{m[1]}{val('colors', m[2])};", html, flags=re.M)
mins = []
def swatch(m):
    bg, fg = val("colors", m[1]), val("colors", m[2]); mins.append(ratio(bg, fg))
    return f"colors.{m[1]} / colors.{m[2]}<br><code>{bg} / {fg} &middot; {mins[-1]:.2f}:1</code>"
html, n_sw = re.subn(r"colors\.([a-z0-9-]+) / colors\.([a-z0-9-]+)<br><code>#[0-9a-f]{6} / #[0-9a-f]{6} &middot; [\d.]+:1</code>", swatch, html)
html = re.sub(r"generation \d+", f"generation {man['generation']}", html, count=1)
html = re.sub(r"min \d+\.\d+:1", f"min {min(mins):.2f}:1", html, count=1)
open("design/specimen.html", "w").write(html); print(f"root={n_root} swatch={n_sw} generation={man['generation']} min={min(mins):.2f}")
PY3
ui ds status --dir .                       # expect: generation START + 43 (41 colours + 2 registry records), tokens 77, components 12
# 4. Prove it. Contrast: token-contract.md (f) C-1 snippet with THEME=crates/designos-app/src/theme.rs (exit 0),
#    then `cargo test -p designos-app chrome_contrast_pairs`.
#    Printed values (AC-7): nothing may print a retired value. History files are excluded by name, not by pattern:
#    design/ds.manifest.json (changelog), design/memory.*, design/evolution-proof.json, design/harvest-inbox/, design/marketing/.
! grep -rliE '#(09090b|121215|18181b|1e1e22|222226|232329|24242a|27272a|2b1d16|2e2e34|2e2e36|383840|3a3a3a|3f3f46|6e6e77|707070|71717a|82aaff|8a8a8a|9cd67d|a8a8b2|b3b3b3|c792ea|c85217|f06d2b|f07436|f2a365|f4f4f5|f59e0b|f5f5f7|ff9e66|ffcb6b)([^0-9a-f]|$)' design/specimen.html design/component-registry.json design/DESIGN.md design/studio-ui-nodes.json design/tokens.json design/design.tokens.json design/soul.md design/principles.md
#    Names (R14, rule 22): nothing in the store or theme.rs names the kit; the credit sits in the soul and principles only.
! grep -rniE 'untitled ?ui|untitledui' design/design.tokens.json design/tokens.json design/ds.manifest.json design/DESIGN.md design/studio-ui-nodes.json design/component-registry.json design/specimen.html crates/designos-app/src/theme.rs
#    Drift of the contract against the HEAD theme.rs, and contrast on the A theme (token-contract.md (g) D-1 and (f) C-1, exit 0 each).
#    Supplementary only (not a gate): ui ds a11y --dir . ; ui ds a11y --dir . --pairs "$(cat design/a11y-chrome-pairs.txt)"
ui gate design/specimen.html --tokens design/design.tokens.json --json
ui ds-usage-lint design/specimen.html --dir . --json
ui ds soul check --dir .
ui agents check --dir .
python3 scripts/design_os_self_evolve.py && python3 scripts/zero_fake_ui_gate.py && design-os evolution
```

The retired-value list in step 4 is every HEAD colour value that (a) or (b) re-values or deletes and that no constant or token keeps (32 values, computed from token-contract.md (a) and (b) by script).

Pasted results from the scratch run (2026-10-28; `astore2` = `design/`, `scripts/`, `.agents/`, `.claude/agents` and `CLAUDE.md` of `8dfaee93`, `theme.rs` of `8dfaee93` with contract (a) applied by script (116 HEAD-value cells checked, 0 drift, 77 constants appended) and the rule-3 `apply()` lines; the #1164 re-seal emulated with `change-token spacing.rail-width`, then steps 1–3; the drafts of section 5.5 copied in; nothing touched the repo; transcript `evidence/AB-fix-round-261028/a-procedure-at-8dfaee93.txt`, which runs the bash block above exactly as written, minus its last line, the full evolve run):

```text
$ ui ds status --dir .           (before 0c's re-seal)   → ui: tokens file hash mismatch … (exit 1)
$ ui ds status --dir .           (after the re-seal)     → ds: designos-studio (gen 29)
$ <step 1>                                               → applied=41
$ <step 3a>                                              → replaced: Button/Primary, Button/Secondary
$ <step 3b>                                              → wrote docs for 12 component(s) → design/DESIGN.md
$ <step 3c>                                              → fill/stroke hex rewritten: 29
$ <step 3d>                                              → root=44 swatch=15 generation=72 min=4.54
$ ui ds status --dir .                                   → ds: designos-studio (gen 72); tokens: 77; components: 12
$ <step 4 retired-value grep>                            → prints nothing (grep exit 1)
$ <step 4 name grep (R14)>                               → prints nothing (grep exit 1)
$ ui gate design/specimen.html --tokens design/design.tokens.json --json   → pass true, 0 errors (3 advisories on the specimen's repeated sample text)
$ ui ds-usage-lint design/specimen.html --dir . --json                     → ok true
$ ui ds soul check --dir .                               → 0 findings
$ ui agents check --dir .                                → 0 findings (after the 5.4 edits)
$ PYTHONDONTWRITEBYTECODE=1 python3 <G01 on the 5.2 edits> (cwd = astore2)
G01 ok = True
35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match
purple-band hits: 0
```

A changes no non-colour constant, so 27 is HEAD's count (unchanged through #1106, #1108 and v0.7.0); B's map makes it 30.

### 5.2 `scripts/design_os_self_evolve.py` (G01)

Exactly the edits in token-contract.md (d): `BANNED_PURPLES` +6, `CORE_RUST_CONSTANTS` +13 (35), 14 map rows changed or added, `DEV_READY` moved to `EXPECTED_STANDALONE_HEX` `#14ae5c`. Without them G01 fails on the scratch theme ("BG_PRESSED vs bg-active", "DEV_READY vs success"). The new function, called from `check_rust_token_parity` after the core-constant loop (each hit is printed and appended to `purple_violations`):

```python
def purple_band_constants(theme_src: str) -> List[str]:
    """Every Color32 constant except BOARD_*: an rgb()/rgba() literal with hue 255-300 deg and HSL saturation > 0.25."""
    import colorsys
    hits: List[str] = []
    for name, hx in re.findall(r"^pub const ([A-Z0-9_]+): Color32 = rgba?\(0x([0-9A-Fa-f]{6})", theme_src, re.M):
        if name.startswith("BOARD_"):
            continue
        r, g, b = (int(hx[i:i + 2], 16) / 255 for i in (0, 2, 4))
        h, _, sat = colorsys.rgb_to_hls(r, g, b)
        if 255 <= h * 360 <= 300 and sat > 0.25:
            hits.append(f"{name} #{hx.upper()} hue {h * 360:.0f} deg, HSL S {sat:.2f}")
    return hits
```

### 5.3 G04 becomes report-only (supplementary evidence, never a gate)

New `design/a11y-chrome-pairs.txt` (new name, one comma-separated line): `bg-foreground`, `muted-foreground`, `accent-foreground` × {`canvas`, `bg`, `bg-hover`, `bg-active`, `surface`, `surface-hover`, `surface-active`, `card`, `popover`, `input`, `muted`, `secondary`, `accent`}; `bg-foreground` × {`secondary-hover`, `secondary-active`}; `primary-foreground` × {`primary`, `primary-hover`, `primary-active`}; `destructive-foreground:destructive`, `success-foreground:success`, `warning-foreground:warning`, `info-foreground:info`, `component-foreground:component`, `neutral-foreground:neutral` — every path prefixed `colors.` (without the prefix `ui ds a11y` reports "not checked"). `check_ui_ds_a11y` (:365) runs both the default and the `--pairs` call, prints their counts as "supplementary" and always returns `passed=True`, so G04 can never fail `design_os_self_evolve.py` (`main()` returns 1 when any gate fails, `:719-760`). Contrast acceptance is C-1 (rule 19).

### 5.4 Design agents

- `.claude/agents/*-jang-designos-studio.md`: no edit. `ui agents check --dir .` must stay "0 findings".
- `.agents/agents/curator-jang-designos-studio.md:19-23` (R14: Studio's names, no kit word): D1 "56px Rail" → "48px icon-only Rail" (#1106); D2 sizes to B's values (rows 32, fields 32, tool buttons 32, panel header 40); D3 to "Inter 12/18 caption and body, 14/20 heading, 20/30 title; 9px canvas only"; D4 to "Neutral dark (`#0A0A0A` bg, `#171717` surface, `#262626` tertiary), `#E36323` Studio orange, `#06B6D4` component cyan; Purple Ban incl. `#7F56D9`/`#9E77ED`"; D5 to "hover `#171717`/`#262626`, pressed `#404040`, selected `#262626`, focus ring `#F97316`".
- `.agents/agents/designer-jang-designos-studio.md:16-17`: Purple Ban list = `BANNED_PURPLES` after 5.2; `FIELD_HEIGHT = 24.0` → `32.0` (B, R8.4).
- `.agents/agents/librarian-jang-designos-studio.md:23`: "14 core constants" → "35 core constants".
- `.agents/agents/figma-jang-designos-studio.md:17`: the palette `#121215` canvas, `#18181B` bg → `#0A0A0A` bg, `#171717` surface (neutral dark); `#E36323`, `#3582FF` (canvas selection, kept) and `#06B6D4` stay.
- Check: `grep -rniE '#(121215|18181b|1e1e22|232329|2e2e36|2b1d16)|Zinc' .agents .claude design/soul.md design/principles.md` prints nothing.

### 5.5 Soul and principles

`design/soul.md` = `specs/A-soul-draft.md` verbatim; `design/principles.md` = `specs/A-principles-draft.md` verbatim (copy, no edit). Both drafts name every value by its Studio constant (R14) and carry the one credit sentence (rule 22). Soul check on the drafts (scratch copy `astore2`, `design/` of `8dfaee93`, 2026-10-28):

```text
$ ui ds soul check --dir <scratch>
ds soul check: <scratch>/design/soul.md — 0 findings.        (exit 0)
$ ui ds soul check --dir <scratch before the drafts>   (design/soul.md of 8dfaee93, baseline)
ds soul check: <scratch>/design/soul.md — 0 findings.
```

`ui ds soul check` lints the structure of `soul.md` only (sections present, not empty, no placeholder, status); it reads neither evidence paths nor `principles.md`. Evidence paths were checked separately at `8dfaee93` with `git cat-file -e`: every path named in the soul draft exists, and so does every path the principles draft names except `scripts/check_chrome_colour_literals.py`, which this spec creates. The drafts name A's constants (`TEXT_ON_BRAND`, `FOCUS_RING`, `BG_PRIMARY_SOLID`, …) and B's (`CONTROL_XS`, `CONTROL_SM`, `RADIUS_XL`, `FONT_SIZE_TEXT_MD`, …): B's exist once B merges (the two PRs are consecutive, section 12), so the soul states the stance of the finished revamp. Neither draft names `DRAG_DIVIDER_HIT`, which spec D declares.

## 6. File-format mapping

No `.fig` / `.designos` change. Token mapping is DTCG-to-DTCG: Untitled names (kit `1_ Color modes`, Dark mode, aliases resolved) map to Studio role keys per token-contract.md (b). `ui ds import` normalises `_` to `-`; this spec never imports, so store keys stay Studio's.

## 7. UI

- UI implementers load the `es-designer` skill first, then read `design/principles.md`, `design/soul.md`, `ui ds context --dir .` and `theme.rs`; the project's own design system wins over any generic default.
- Visible change: every chrome surface turns from Zinc to Untitled Neutral, status colours move, syntax colours change, shadows disappear. No layout change (R1); B owns sizes.
- Visual evidence goes to the PR, not Figma. Commands (run from the repo root; outputs under `plans/261025-1247-untitled-ui-dark-revamp/evidence/A/`):

```bash
LIB="$(mktemp -d)/lib"   # seeded sample library; never the owner's own library
cargo run -q --release -p designos-app --bin designos -- --seed-library "$LIB"
cargo run -q --release -p designos-app --bin designos -- --library "$LIB" --home --screenshot evidence/A/home.png --size 1600x1088 --no-toast
cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --screenshot evidence/A/editor-kit.png --size 1600x1088 --no-toast
for p in cmdk terminal assets variables picker dev-compare; do
  cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --panel "$p" --screenshot "evidence/A/panel-$p.png" --size 1600x1088 --no-toast
done
```

`$KIT_COPY` is a `.designos` copy of the Untitled kit (never the original). Home is captured on a seeded sample library (`--seed-library`, `--library`), never on the owner's own library. Compare Home against spec L's seeded-library baselines (`scripts/layout_baseline_capture.py --compare ~/designos-layout-baseline/<bless sha>`, case `h-home`, library seeded with `designos --seed-library`) and the editor against `evidence/baseline-editor-with-kit.png` (kit content only). No capture made from the owner's library is used or attached.
- responsive: SKIPPED (owner default).

## 8. Edge cases

- **T1 — no add path in the sealed store (kernel gap, owner: `ui` kernel in `ease-design`, related to dogfood spec P).** Untitled names without a Studio key cannot be added. Workaround: re-value existing keys; keep the rest as `theme.rs` constants guarded by tests and `RUST_ONLY`.
- **T2 — `ui ds preview` output fails `ui ds-usage-lint` on this store (kernel gap, owner: `ease-design`).** Regenerating the specimen at HEAD in scratch gives 69 errors (`var(--color-ring)`, `var(--font-size-xs)` … "declared nowhere", plus hard-coded hex), because the generator writes canonical `--color-*`/`--font-size-*` names while the store's groups are `colors.*`/`typography.*`. The committed specimen passes (0 errors). Workaround: refresh its printed values in place (5.1 step 3); measured: 51 custom properties, 15 swatches, 6 radius chips rewritten, `ui gate` pass with 0 errors, usage-lint 0 errors, no retired hex left.
- **T3 — `ui ds import --force` wipes the registry and changelog (by design).** Never used here.
- **T4 — default `ui ds a11y` on an Untitled-named store builds cartesian pairs (1392, 731 "failing") or, on raw DTCG, measures 0 pairs (false green).** Always pass `--pairs` with `colors.` paths.
- `BG_HOVER == BG_SECONDARY` (`#171717`): any surface that is `BG_SECONDARY` uses `BG_SECONDARY_HOVER` for hover (rule 3); surface specs own the call sites.
- `BG_SELECTED == BG_SELECTED_HOVER` (`#262626`, R8.2): the kit keeps the fill under the pointer; no extra state colour.
- `TEXT_SECONDARY`/`TEXT_TERTIARY` turn from alpha to opaque: text drawn over images or the canvas (none found in chrome) would no longer blend.
- `terminal.rs` pins `SUCCESS` and `WARNING` hex in a test, in `theme.rs` (:10728-10730) and in the store (:10743-10745): update all four literals in the same commit.
- Constants deleted with 0 references: a reference added between HEAD and the PR re-anchor breaks the build; re-run the grep before deleting.

## 9. Non-goals

- Sizes, type, spacing, radii (spec B). Widget anatomy and states (spec C). Re-pointing call-site literals to roles (D–K). Canvas and Dev overlay colours (controller default). Light mode. Untitled icons (R6, spec Q). Fixing kernel gaps T1/T2 in `ease-design` (recorded, not fixed here). Regenerating `.claude/agents` (`ui agents init`).

## 10. Test plan (red first)

Red at HEAD, green after:

1. `theme.rs` `no_chrome_colour_is_purple` (new test): hue scan of every `Color32` constant except `BOARD_*`; red at HEAD (`SYNTAX_KEYWORD` `#C792EA`, 276°).
2. `theme.rs` `brand_label_passes_aa` (new test): WCAG ratio of `TEXT_ON_BRAND` on `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED` ≥ 4.5; red at HEAD (no `TEXT_ON_BRAND` constant exists).
3. `theme.rs` `chrome_contrast_pairs` (new test): one assert per row of token-contract.md (f) C-1, on the constants, with that row's minimum (4.5 for text, 3.0 for the placeholder exception and non-text); red at HEAD (`TEXT_TERTIARY` `rgba(0xF4F4F5, 0x66)` blends to 3.57:1 on `BG` `#121215`, sRGB source-over of the premultiplied `rgba()`; the new constants do not exist). The floor rows E-1.a/b and E-2.a/b are asserted against their floors and their exact hex, not as passes.
4. Existing selection-fills-neutral test (:907-913) keeps passing with the new values.
5. `terminal.rs` colour pins updated to `#22C55E`/`#EAB308` in both the `theme.rs` and the store asserts; `INFO` pins unchanged.
6. G01 hue scan (token-contract.md (d), new function `purple_band_constants`): G01 scans every `Color32` constant in `theme.rs` except `BOARD_*` (hue 255–300°, HSL saturation > 0.25), so a purple outside the 35 core constants fails it. The exact-hex `BANNED_PURPLES` check alone could not go red here: `SYNTAX_KEYWORD` is not a core constant. Scratch run 2026-10-28 (the repo's G01 functions with the 5.2 edits and the scan applied to a scratch copy of the script; `theme.rs` at `8dfaee93` with contract (a) applied; store re-valued by 5.1):

   ```text
   $ g01 <A theme.rs> <A store>                         → G01 ok = True … 27 non-colour constants match; purple-band hits: 0 (exit 0)
   $ g01 <A theme.rs with SYNTAX_KEYWORD = rgb(0xC792EA)> <A store>
   Purple Ban (hue scan): SYNTAX_KEYWORD #C792EA hue 276 deg, HSL S 0.68
   G01 ok = False
   purple-band hits: 1                                                                               (exit 1)
   $ scan of theme.rs at 8dfaee93 (HEAD)                 → ['SYNTAX_KEYWORD #C792EA hue 276 deg, HSL S 0.68']
   ```
7. `scripts/check_chrome_colour_literals.py`: a test injects one `Color32::from_rgb(1, 2, 3)` into a temp copy of a file and expects FAIL; HEAD passes against the written baseline.
8. Contrast check C-1 (token-contract.md (f), `THEME` = the theme.rs under test) exits 0 on the A theme and exits 1 on drift. Scratch runs 2026-10-28 (`theme.rs` at `8dfaee93` with contract (a) applied; transcript `evidence/AB-fix-round-261028/c1-contrast-check-at-8dfaee93.txt`):

   ```text
   A theme                                       → C-1: 70/70 pass, 4/4 exception floors held, 0 failure(s)   (exit 0)
   A theme, TEXT_TERTIARY back to rgba(0xF4F4F5, 0x66)
     C-1.7   table #A3A3A3/#0A0A0A but theme.rs #676768/#0A0A0A
     C-1: 65/70 pass, 4/4 exception floors held, 5 failure(s)                                               (exit 1)
   contract copy with one row's backticks removed → row-count guard: parsed 73 of 74 rows                 (exit 1)
   HEAD theme.rs (no A)                          → C-1: 0/70 pass, 0/4 exception floors held, 74 failure(s)  (exit 1)
   ```

Acceptance criteria:

1. `theme.rs` colour constants equal token-contract.md (a) (116 rows + 77 new), checked by the G01 run and by `grep` of each new literal; before the first edit, token-contract.md (g) D-1 exits 0 on the HEAD `theme.rs` the PR starts from (scratch at `8dfaee93`: "116 colour rows, 25 scale rows, 302 appendix cells, 327 numeric constants, 0 drift line(s)").
2. `ui ds status --dir .` exits 0 → generation = the starting generation + 43 (41 colour re-values + 2 registry records; scratch at `8dfaee93` after the #1164 re-seal: 29 → 72), 77 tokens, 12 components; `design.tokens.json` colours equal token-contract.md (b).
3. Contrast: token-contract.md (f) C-1 with `THEME=crates/designos-app/src/theme.rs` exits 0 with "70/70 pass, 4/4 exception floors held" (scratch run 2026-10-28) and `cargo test -p designos-app chrome_contrast_pairs` passes.
4. `python3 scripts/design_os_self_evolve.py` exits 0: G01 "35/35 … 0 Purple Ban violations" with 0 purple-band hits, G03, G05, G06 and G08 green. G04 (`ui ds a11y`) is report-only (5.3) and cannot fail the run; contrast acceptance is item 3.
5. `ui ds soul check --dir .` → 0 findings, `status: ratified`; `design/soul.md` and `design/principles.md` byte-equal the drafts.
6. `ui agents check --dir .` → 0 findings; the 5.4 grep prints nothing (it covers `.agents`, `.claude`, the soul and the principles; `.agents/agents/figma-jang-designos-studio.md:17` is edited by 5.4).
7. No retired value is printed: the step-4 grep of 5.1 over `design/specimen.html`, `design/component-registry.json`, `design/DESIGN.md`, `design/studio-ui-nodes.json`, `design/tokens.json`, `design/design.tokens.json`, `design/soul.md` and `design/principles.md` prints nothing (scratch: nothing). History and content files are excluded by name: `design/ds.manifest.json` (changelog), `design/memory.*`, `design/evolution-proof.json`, `design/harvest-inbox/`, `design/marketing/`. `ui gate` and `ui ds-usage-lint` on the specimen stay green too, but they cannot see printed values (they passed with 11 retired values present), so they do not prove this item.
8. `cargo test -p designos-app` green incl. tests 1–3 and 5; `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/check_chrome_colour_literals.py`, `design-os evolution` green.
9. Evidence PNGs of section 7 attached to the PR; "responsive: SKIPPED (owner default)" stated in the PR.
10. Names (R14, rule 22): the step-4 name grep of 5.1 prints nothing; `design/soul.md` and `design/principles.md` each carry the credit sentence once and no other "Untitled" word (`grep -c 'Untitled' design/soul.md design/principles.md` prints 1 and 1); every `change-token` reason in `design/ds.manifest.json` written by this PR is in Studio's wording.

## 11. Performance budgets

Constants only; no runtime cost. Idle CPU stays `0.00%` (`CLAUDE.md` §4 method). Gate runtime: the new colour-literal gate scans `crates/designos-app/src` in under 2 s (same scan cost as `check_spacing_relationships.py`).

## 12. Dependencies and build order

0. #1164 (R12: re-seal the token store, its own small PR) lands first. 1. This PR (A). 2. Spec B edits the same `theme.rs`, `design.tokens.json` and `non_colour_token_parity.py`: one writer, B's PR opens after A merges (sequential). 3. C, then D–K re-point call sites using the constants named here. 4. M copies the merged table. Kernel gaps T1/T2 go to `ease-design` as issues (engineering writer, spec P covers the export side).

## 13. Owner rulings that apply

R1 (adopt 100% Untitled style, keep layout), R2 (take everything from the kit), R3 (dogfood, no Figma: onboarding uses `designos --export-variables` and `ui ds`), R4 (Studio orange `#E36323`, dark label `#0A0A0A`, focus ring `#F97316`, Purple Ban stays), R6 (icons keep their set; colours from `fg-*`), R7 (spec-driven), R8.1 (placeholder `#737373`), R8.2 (selected `#262626`), R8.3 (accent text `#F97316`), R8.4 (Inspector field height 32, `CONTROL_XS`), R8.5 (floating chrome radius 16, items radius 8 at inset 8), R8.6 (button xs label 14 Semibold; rows, labels and tabs stay 12), R8.7 (destructive hover `#B91C1C`), R9 (read-only Figma check), R10.1 (editor tab heights stay: file tabs 26 in the 32 bar, session tabs and the Inspector strip 24), R10.2 (Home tiles), R10.3 (selected card border), R10.4 (Dev accent `#14AE5C`), R10.7 (checkbox and radio border `#404040`), R12 (the store re-seal is its own PR), R14 (Studio's own names and values in the design store, no kit file, no Untitled icons, credit Untitled UI). Controller rulings: C-F (field fill bg-primary; E-2 covers checkbox and radio, text field and select, secondary button), C-T (tooltip label `TEXT`), C-R (drag dividers keep their 1 px line and get a 24 px hit band; "Mixed" is `TEXT_TERTIARY`; E-1 stays placeholder-only). Controller defaults: canvas, Dev overlay, info, component cyan and document content keep their values; chrome status colours move to Untitled.

## Open questions for the owner

All decided by the owner on 2026-10-25 (plan.md R8); none open.

1. **OQ-1 Placeholder colour: decided (R8.1).** Kit `text-placeholder` `#737373` (3.78:1 on `#171717`, 4.18:1 on `#0A0A0A`), the one named exception to the AA text law. Owner: "B · #737373 của kit" (the designer had recommended `#A3A3A3`).
2. **OQ-2 Selected row fill: decided (R8.2).** `#262626` bg-secondary_hover. Owner: "A · #262626".
3. **OQ-3 Accent text: decided (R8.3).** `TEXT_BRAND` Orange-500 `#F97316` (7.06:1). Owner: "A · cam #F97316".

## Decisions on the open questions (designer, 2026-10-25)

- Re-value in place with `ui ds change-token`, never `ui ds import --force` (keeps 12 components, changelog and roles; proj-r/proj-v runs).
- Store keys stay Studio's role names; Untitled role names live only in this spec and token-contract.md (R14): the change-token reasons, registry text, soul, principles and `theme.rs` comments use Studio's wording.
- `BG_PRESSED` maps to `secondary-active`, `BG_SELECTED` to `bg-active`; `DEV_READY` leaves the G01 map for `EXPECTED_STANDALONE_HEX`.
- `SUCCESS`/`WARNING`/`DANGER` move to Untitled `fg-*` (controller default); `INFO` stays.
- Syntax colours take the kit's dark utility steps; `SYNTAX_KEYWORD` pink-400 fixes the HEAD Purple Ban breach.
- The colour-literal gate requested by D/E/F/G is adopted here with a falling per-file baseline.
- Specimen refreshed in place, not regenerated (gap T2); `design/tokens.json` mirrored.
- Buttons change from pills to `radius-md` 8 at every size, Home and dialog buttons included (soul and P4 clauses rewritten): R2 ("take everything from Untitled") and the kit FACT that every `Buttons/Button` size is `radius-md` (`--select "Buttons/Button" --export-spec`). Pills stay for pill badges, count pills, filter chips, toggles and avatars.
- `BG_ERROR_SOLID_HOVER` is Red-700 `#B91C1C` by owner ruling R8.7 ("C · #B91C1C Red-700"), not the kit's dark `#EF4444` (3.76:1).
- Provider marks are identity content (G-2), declared in `theme.rs` so no literal stays in `terminal.rs`; Grok's Zinc-200 `#E4E4E7` moves to Neutral-200 `#E5E5E5` because the Zinc family is retired.
- The modal scrim keeps the kit's 70% `bg-overlay`; whether to draw the 16 px background blur is spec J's call (P8 frame budget).
- Contrast acceptance moves from `ui ds a11y` to contrast check C-1 (token-contract.md (f)) plus the Rust test that mirrors it.
- The soul keeps `status: ratified`: its stance is the owner's rulings R1–R7; the owner's PR approval is the ratification act (Jang approves every merge).
- No `ui agents init --force`; `.agents/agents/` copies are hand-edited.

## Open items for the controller (not owner questions)

1. **R14 name audit.** Shipped names must be Studio's own. Today the constants follow Studio's `BG_*`, `TEXT_*`, `ICON_*`, `BORDER_*`, `STATUS_*` vocabulary except the families that borrow a kit ramp word (`UTILITY_<HUE>_<STEP>`, `ICON_BRAND_PRIMARY`, `BG_PRIMARY_ALT`, `BG_PRIMARY_SOLID`, `BG_SUCCESS_PRIMARY`, …; token-contract.md N14–N46). None contains "Untitled". Renaming them is a find-and-replace across `theme.rs`, its tests and specs C–K that changes no value; spec T (ontology registry) is writing the name map, so this spec keeps the names until T's amendments arrive.
2. **Licence confirmation.** R14 says Jang confirms with Untitled UI in writing. Nothing shipped claims permission; the credit sentence only credits.
3. **#1164** lands before A (R12); A's step 0c stops if it has not.

## Changelog (curator fix round, review `reviews/AB-contract-curator-review.md`; 2026-10-25; re-anchored and extended 2026-10-28)

One line per finding. Findings that land only in B are logged in B's changelog; this list names them so none is missing.

- F1 (blocker): #1164 (R12) is a hard predecessor (header "Depends on", 12 step 0, 5.1 step 0c checks it); re-run at `8dfaee93` on 2026-10-28 it still fails (`ui ds status` exit 1, same hash mismatch) and the repair path gives generation 29; AC-2 states the generation relative to the start (start + 43: 41 colour re-values + 2 registry records; scratch 29 → 72 at `8dfaee93`); every cite re-anchored to `origin/main@8dfaee93`.
- F2 (resolved in live): no A change; B lists R8.4–R8.6 and R10.1 in its section 13 (B changelog).
- F3: P3 Statement and Test in `A-principles-draft.md` say control heights are component sizes (button and input {32, 36, 40, 44, 48}, badge {22, 24, 28}, `TARGET_MIN` 24), not spacing steps; B rule 9 and test 1 match (B changelog).
- F4: test 6 is the G01 hue scan `purple_band_constants` (function text in 5.2; token-contract.md (d): every `Color32` constant except `BOARD_*`, hue 255–300°, HSL S > 0.25), red run on `SYNTAX_KEYWORD` `#C792EA` pasted (re-run 2026-10-28); the C-1 snippet reads every value from `theme.rs`, composites `rgba()` over `BG`, and fails when the table and the code differ.
- F5: E-2 covers every 1 px `border-primary` `#404040` control boundary (controller ruling C-F: unchecked checkbox and radio per R10.7, text field and select, secondary button), bounded to bg-primary and bg-secondary; the soul line, P2 and the contract name those surfaces and cite C-F as a controller ruling the owner may reverse; neither draft claims "only two exceptions" for anything else. This round: the registry record `Button/Secondary` now lists `colors.border-strong` (its boundary is E-2), not `colors.border` (5.1 step 3a).
- F6: G04 (`ui ds a11y`) is report-only in 5.3 and AC-4; principles line 3 names contrast check C-1 and the `chrome_contrast_pairs` test; `design/a11y-chrome-pairs.txt` is in Touches.
- F7: AC-7 greps only files that print values with a grep that sees printed hex, and excludes history and content files by name; 5.1 says how the registry (`ui registry register … --force`), `studio-ui-nodes.json` and the specimen (script in step 3d, new this round) are refreshed; AC-6: 5.4 edits `.agents/agents/figma-jang-designos-studio.md:17`.
- F8: field fill is bg-primary everywhere (ruling C-F): rule 3 (`extreme_bg_color` = `BG`), contract (a) #3 and (b) `input`, soul and P2 use one wording.
- F9: P4 radius roles: `RADIUS_FRAME` 16 for menus, dropdowns, popovers and modals (R8.5); `RADIUS_XL` 12 for cards, the command bar, toasts and nested bars.
- F10: R10.1 carve-out (file tabs 26 in the 32 bar, session tabs and the Inspector strip 24) in the soul's Never clause and controls line and in P3 Controls.
- F11: B only (variable `tok`, B changelog).
- F12: A's own break is the terminal pin test (test 5); B lists its breaks in 10.1 (B changelog). Re-measured at `8dfaee93` this round (B:183 and `evidence/AB-fix-round-261028/test-breaks-at-8dfaee93.txt`).
- F13: contract (f): the placeholder pair is floor rows E-1.a/b, E-2.a/b are added (floor 1.7), the snippet prints them as EXC and does not count them as passes, and a row-count guard fails on a dropped row; acceptance 3 reads "70/70 pass, 4/4 exception floors held", exit 0 (re-run at `8dfaee93` after the D–G, C and H rows).
- F14: contract "Shared values (generated)" table, 35 rows (119 names), with the note that `#262626` roles draw at 1.00:1 against each other; re-generated from the A theme at `8dfaee93`: 0 mismatches.
- F15: drift re-anchored to `8dfaee93` (this round; first `d0555c82`); the contract appendix has no #1106-deleted rail constant and holds the #1108 constants and `TOAST_REVEAL_SECONDS`, now proved by D-1 (contract (g), exit 0, red on the theme before #1106).
- F16: 5.1 says "27 non-colour constants match" after A (measured again); test 3 says HEAD `TEXT_TERTIARY` blends to 3.57:1.
- F17, F18, F19: B only (B changelog).
- F20: staged constants carry `#[allow(dead_code)]` (rule 21; B rule 13).
- F21: soul: disabled is the control at opacity 0.5; text-only disabled uses `TEXT_DISABLED` (`TEXT_SECONDARY` at 50%), never a new grey.
- F22: P3 says the 4px grid of P3 v1 becomes the spacing scale (R5); the `CLAUDE.md` "4px rhythm" line belongs to spec M.
- F23: the tooltip label ruling is controller ruling C-T, recorded in plan.md; rule 17 cites it.
- F24: E-2 is bounded to bg-primary and bg-secondary; on `BG_TERTIARY` (1.46:1) and `BG_PRESSED` (1.00:1) it is not allowed.
- F25: plan.md is the coordinator's (line 4 anchor, line 82 ratio 5.73).
- Owner library: section 7 captures Home on a seeded library and compares it with spec L's seeded-library baselines; `evidence/baseline-home.png` is no longer cited.
- D–G review a: the contract lists no rail constant deleted by #1106 and includes the #1108 constants with today's values (D-1 proves both against `theme.rs@8dfaee93`). b: E-2 names text-field and select borders explicitly. c: B rule 6 (B changelog).

### What changed against the branch copy (rule diff for the PR #1207 implementer; computed 2026-10-28)

The branch copy is `docs/specs-untitled-ui-dark:docs/specs/ui-revamp-untitled-dark/1142-onboard-untitled-dark-colour-roles-and-soul.md` at `74610c52` (2026-10-25 22:19), from which draft PR #1207 was built; its soul and principles drafts and `token-contract.md` are on the same branch. "Was" = that copy. Rules 2, 4, 5, 6, 8–15 and 18 are unchanged; the others:

- **Rule 1 (values).** 77 constants added (was 46): N47–N73 are the names specs D–G sent (Dev hover, HUD pointer, playground dots, `CONTENT_*` overlay names, 15 `READER_LIGHT_*`), N74–N77 are spec C's error roles (`FOCUS_RING_ERROR`, `BORDER_ERROR_SUBTLE`, `BORDER_ERROR`, `TEXT_ERROR_HOVER`); the secondary-destructive hover follows the kit tint `BG_ERROR_PRIMARY`, not Red-700 (C-1.65–C-1.69); spec H's star rows replace C-1.40 and add C-1.70–C-1.72 (`NAME@RRGGBB` backgrounds in the snippet).
- **Rule 3 (`apply()`).** `extreme_bg_color = BG` (was `BG_SECONDARY`, text fields are bg-primary, C-F); `faint_bg_color` stays `BG_SECONDARY`; resting widget stroke `BORDER_STRONG` for text fields, selects, secondary buttons and unchecked checkboxes and radios (E-2).
- **Rule 7 (placeholder).** Floor rows E-1.a/b (were C-1.25/26 counted as passes); text fields are `BG`, so 4.18:1, a field in a `BG_SECONDARY` card 3.78:1; E-1 is placeholder-only, so the Inspector's "Mixed" is `TEXT_TERTIARY` (C-R).
- **Rule 16 (G-3).** The overlay blur is documented as Figma radius 16 = CSS `blur(8px)`; specs I, J and K draw none (was "J decides").
- **Rule 17 (tooltip).** Cites controller ruling C-T.
- **Rule 4 (focus ring).** An error-state field takes `FOCUS_RING_ERROR`; every other ring stays `FOCUS_RING` (was: one ring for everything).
- **Rule 19 (C-1).** 70 pairs plus 4 floor rows, read from `theme.rs` with a row-count guard (was 62 pairs including the placeholder pair, hex read from the table); new companion check D-1 (contract (g)).
- **Rule 20 (R10).** E-2 names the controls: unchecked checkbox and radio, text field and select, secondary button, only on bg-primary or bg-secondary (C-F; was checkbox and radio only).
- **Rule 21 (new).** Staged constants carry `#[allow(dead_code)]` (CONTRIBUTING.md:29 runs clippy with `-D warnings`); frozen constants that lose their last reader (`PAGES_SEARCH_DX`, `RIGHT_PRESENT_FROM_RIGHT`) are deleted, not allowed.
- **Rule 22 (new, R14).** Studio's own shipped names; credit sentence; name grep.
- **5.1 procedure.** `change-token` reason prefix `Studio dark chrome palette:` (was `Untitled UI v8 dark:`) and Studio-wording tsv reasons; step 3a registers `Button/Secondary` with `colors.border-strong` (was `colors.border`); step 3d is a script; step 4 adds the name grep and cites C-1 and D-1; generation arithmetic is relative (start + 43; was "generation 69").
- **5.2 / 5.3 / 5.4.** 5.2 prints the `purple_band_constants` function; G04 is report-only and always returns `passed=True` (was "supplementary" but still able to fail the run); 5.4 also edits `.agents/agents/figma-jang-designos-studio.md:17`, the curator's D1 ("56px Rail" → 48 px icon-only), and the designer's `FIELD_HEIGHT`.
- **Soul and principles drafts.** Every value is named by its Studio constant and the drafts carry one credit sentence (was Untitled role names); drag dividers keep their 1 px line with a 24 px hit band; "Mixed" is `TEXT_TERTIARY`.
- **Tests and acceptance (10).** Test 5 pins both the `theme.rs` and the store asserts; test 6 is the G01 hue scan with a red fixture; test 8 has drift and row-count fixtures; AC-1 adds D-1, AC-2 is relative to the start generation, AC-6 and AC-7 are the grep-based forms, AC-10 is the name check.
- **Contract beside A (`token-contract.md`; the branch copy of it is stale in these points).** N47–N77 added (27 from specs D–G, 4 from spec C); C-1 has 70 rows plus 4 floor rows (the branch's C-1.25/26 are retired: they were the placeholder pair, now floor rows E-1.a/b, so a `chrome_contrast_pairs` built from the branch table would assert them as passes); C-1.40 is the starred-star glyph row and C-1.63–C-1.72 are new; the snippet reads `theme.rs` and accepts `NAME@RRGGBB` backgrounds; new check D-1 (g); a numeric new-constants table in (c) (`DRAG_DIVIDER_HIT`, the Dev tints, `TYPE_LABEL_COMPONENT_ALPHA`); the tsv reasons in (b) and (c) are Studio wording; rules after (a) add the error-field ring, the secondary destructive, "Mixed" and drag dividers.
- **Anchors.** Every cite now reads `8dfaee93` (the branch copy cites `ec47da26`: `apply()` was :619-657, now :626-664; `SYNTAX_KEYWORD` :557, now :564; the colour-pin test in `terminal.rs` :10727-10745); census 406 sites in 53 files (was 410 in 51). Because `theme.rs` line numbers in the branch copy and branch contract are 3 to 7 lines off, re-anchor by constant name, and run D-1 on the HEAD the PR starts from.
- Tools: Studio CLI and MCP runs use v0.7.0; code is cited at `origin/main`, not at a release branch.
