# Spec A — Onboard Untitled UI dark with the design:os CLI: colour roles, token store, `theme.rs` colours, soul, principles, design agents

| Field | Value |
|---|---|
| Issue | #1142 (epic #1118, spec key A) |
| Size | M. About two days: 41 token re-values by script, ~70 `theme.rs` colour lines, two gate scripts edited, one gate script added, soul and principles replaced, four agent files edited, tests, evidence. |
| Depends on | none (phase 0, first) |
| Unblocks | B (same files, next PR), C–K (they name colours by Untitled token; this spec maps them to constants), M (copies its merged table) |
| Touches | `crates/designos-app/src/theme.rs` (colours :20-112, `apply()` :619-657, tests :770-913), `crates/designos-app/src/terminal.rs` (colour pins :10019-10021), `crates/designos-app/src/cmdk.rs`/`dev/list.rs` (none; those two are B's), `design/design.tokens.json`, `design/ds.manifest.json`, `design/tokens.json`, `design/specimen.html`, `design/DESIGN.md`, `design/soul.md`, `design/principles.md`, `scripts/design_os_self_evolve.py` (:109, :111-134, :137-158, :160-163, G04 :365), new `scripts/check_chrome_colour_literals.py` (new name) + `scripts/chrome_colour_baseline.json` (new name) + its test, `.agents/agents/{curator,designer,librarian}-jang-designos-studio.md` |

Code baseline: `ec47da26` (local `main`). Drift: `origin/main@35a65796` adds `TOAST_REVEAL_SECONDS` after `theme.rs:463` (+2 lines), moves the `terminal.rs` colour pins to `:9613-9615`, shifts `tabs.rs` by 2 lines and edits `zero_fake_ui_gate.py` Z04/Z09. Re-anchor every cited line against HEAD and report drift before writing code.

The value tables live in `specs/token-contract.md` (this folder); this spec says how to apply them and how to prove them. Kit values come from the commands listed in token-contract.md section 0; the raw kit export is never committed (paid kit, public repo).

## 1. Summary and user story

As the owner, I want Studio's chrome to wear the Untitled UI PRO v8.0 dark colour system with Studio orange as the brand (R1, R2, R4), onboarded with Studio's own tools and the design:os CLI, never Figma (R3), so that every surface spec (C–K) can name a colour by its Untitled token and get one exact Rust constant and one token-store value.

As a design agent (designer, curator, figma hand, librarian), I want `ui ds context` to hand me the new soul, the new principles and the re-valued token store, so that I judge and generate against Untitled dark, not the retired Zinc palette.

Outcome: `theme.rs` colour constants and `design/design.tokens.json` hold the Untitled dark values in the token-contract mapping; G01 parity, the Purple Ban, AA on every chrome text×surface pair, the soul check and the agents check are green; the design agents pick the new stance up without being regenerated.

## 2. Current state (FACT unless labelled)

**Colours in code.** `theme.rs` declares 116 `Color32` colour constants (`grep -cE '^pub const [A-Z0-9_]+: (Color32|\[Color32; ?[0-9]+\]) =' theme.rs`) on the Zinc palette: `BG` `#121215` (:22), `BG_PRIMARY` `#18181B` (:23), `BG_SECONDARY` `#1E1E22` (:24), `BG_HOVER` `#232329` (:26), `BG_PRESSED` `#2E2E36` (:27), `BG_SELECTED = BG_PRESSED` (:31), `TEXT_SECONDARY` white at 70% (:51), `TEXT_TERTIARY` white at 40% (:52), `DANGER` `#FE5938` (:60), `SUCCESS` `#14AE5C` (:62), `WARNING` `#F59E0B` (:63). `apply()` (:619-657) maps egui visuals: `panel_fill = BG`, `window_fill = BG_MENU`, `extreme_bg_color`/`faint_bg_color = BG_SECONDARY`, `selection.bg_fill = BG_TEXT_SELECTION`, `selection.stroke = BORDER_SELECTED`, widgets inactive `BG_SECONDARY`, hovered `BG_HOVER`, active `BG_PRESSED`. Five colour constants have 0 references outside `theme.rs`: `BG_ELEVATED`, `BG_DISABLED`, `TEXT_ON_LIGHT_CANVAS_SECONDARY`, `TEXT_ON_DARK_CANVAS`, `LOGO_GRADIENT_A` (`grep -rnw <NAME> crates/designos-app/src --exclude=theme.rs`).

**Purple Ban breach at HEAD.** `SYNTAX_KEYWORD` (:557) is `#C792EA`, hue 276°, saturation 0.68: violet. The G01 list `BANNED_PURPLES` (`design_os_self_evolve.py:109`) holds only `#9747ff`, `#8755e9`, `#c4a1ff`, `#a855f7`, so no gate sees it. A hue scan of every `theme.rs` literal (band 255–300°, saturation > 0.25) finds exactly `SYNTAX_KEYWORD` and the board content colour `BOARD_PALETTE` `#D5C3FF`.

**Literal colours outside `theme.rs`.** 410 chrome sites in 51 files (census with spec D's regex, comments blanked and `#[cfg(test)]` dropped by `scripts/rust_literal_scan.clean`; top: `terminal.rs` 54, `specs.rs` 42, `home/mod.rs` 38, `canvas/draw.rs` 26, `panels/inspect.rs` 20). No gate counts them; specs D, E, F and G ask this spec for one (`check_chrome_colour_literals.py`, new name).

**Token store.** `design/design.tokens.json`: 48 colours, 8 typography, 12 spacing, 6 radius, 3 durations (77 tokens, `ui ds status`: generation 28). Each colour carries `$extensions.design-os.role`. `design/ds.manifest.json` names the store `designos-studio`. `design/tokens.json` is the flat file it was imported from (no script reads it; `grep -rn "design/tokens.json" scripts crates` finds nothing).

**Gates.** G01 (`design_os_self_evolve.py`) parses only literal `pub const X: Color32 = rgb(0x……)` (:223-225) for 22 `CORE_RUST_CONSTANTS` (:111-134) through `RUST_TO_DESIGN_TOKEN_MAP` (:137-158), where `DEV_READY`→`success` (:149) and `SUCCESS`→`success` (:156) collide. G04 (:365) runs `ui ds a11y` with no pairs: it checks the 15 `{role}/{role}-foreground` pairs + 8 state pairs, not the chrome text×surface pairs. G05 (:380) needs `ui ds soul check` with 0 findings and `status: ratified`. G06 (:395) runs `ui agents check` and checks that the four `.agents/agents/` files exist (content is not checked). G08 (:476) runs `ui gate design/specimen.html` and `ui ds-usage-lint` on the committed specimen.

**Design agents.** `.claude/agents/{designer,curator,figma}-jang-designos-studio.md` bake no palette values; their first action is `ui ds context`. `.agents/agents/` (cross-runtime copies) bake Zinc values: `curator…md:20-23` (D2 field 24px, D3 10/11/13 px, D4 `#121215`/`#18181B`/`#1E1E22`, D5 hover `#232329`, pressed `#2E2E36`, selected `#2B1D16`), `designer…md:16` (Purple Ban list of four), `librarian…md:23` ("14 core constants"). `ui agents check --dir .` on the repo: "0 findings".

**Onboarding without Figma (R3), measured in scratch copies only** (`<scratch>` = this agent's scratchpad; nothing below touched the repo):

| Run | What | Result |
|---|---|---|
| E1 | `ui ds import "<kit export>/1_ Color modes.Dark mode.tokens.json" --dir <scratch>/e1` (raw DTCG) | 295 tokens (273 colour, 22 number), 54 categories, 82 roles recognised, 8 gaps (`card`, `popover`, `secondary`, `muted`, `accent`, `input`, `info`, `neutral`), 132 sub-fields skipped, 233 aliases left dangling (`{_Primitives…}`); `ui ds a11y`: "0 contrast pairs measurable … (false-green risk)" |
| E2 | resolve aliases (`evidence/resolve-untitled-tokens.py`), flatten to `{category:{name:value}}`, `ui ds import <flat>.json --dir <scratch>/e2` | 107 tokens (78 colour, 29 dimension), 74 roles recognised, 12 gaps; `_` becomes `-` (`bg-primary_hover` → `bg-primary-hover`); `border-primary`/`border-secondary` get no role; default `ui ds a11y` builds a cartesian product on Untitled names: 1392 pairs, 731 "failing", meaningless without explicit `--pairs` |
| proj-r | `ui ds import … --force` onto a copy of the real store | refused: "would wipe 12 registered component(s) … pass --reset-registry"; with `--reset-registry`: generation 1, changelog 0, 0 components (soul and principles untouched) |
| proj-v | re-value in place on a copy of the real store: 41 × `ui ds change-token colors.<key> --value <hex> --reason "<Untitled token>: …"` (token-contract.md (b)) | 41/41 succeeded; generation 28 → 69, changelog 27 → 68 entries; every `$extensions.design-os.role` kept; 12 components kept |
| e3 | reference store with Untitled names and Studio orange (44 tokens), `ui ds a11y --pairs <94 pairs>` | 94 pairs, 14 below AA, all of them placeholder `#737373`, error `#EF4444`, tertiary or accent text on `bg-quaternary`/`bg-tertiary`; every other chrome pair passes (rules 5–8 below) |

Conclusion (INFERENCE from the runs): `ui ds import` cannot onboard the kit onto this project without losing the component registry and changelog, and its raw DTCG path measures nothing; re-valuing the existing role keys with `change-token` is the only path that keeps history, roles and components. Tool gaps and owners are in section 8.

## 3. Behaviour rules

1. **Values.** Every `theme.rs` colour constant takes the action and value in token-contract.md (a): 55 re-valued, 52 kept, 4 aliases kept, 5 deleted, 13 added (new names). Every `design.tokens.json` colour takes token-contract.md (b): 41 re-valued, 7 kept.
2. **Literals for parity.** Every constant in the G01 map is a plain `rgb(0x……)` / `rgba(…)` literal. `BG_SELECTED` stops being `= BG_PRESSED` and becomes `rgb(0x262626)`.
3. **egui visuals in `apply()`.** `panel_fill = BG` (`#0A0A0A`), `window_fill = BG_MENU` (`#0A0A0A`), `extreme_bg_color`/`faint_bg_color = BG_SECONDARY`, widgets `inactive = BG_SECONDARY`, `hovered = BG_SECONDARY_HOVER` (new name, `#262626`; `BG_HOVER` equals `BG_SECONDARY` in the kit and would make hover invisible), `active = BG_PRESSED` (`#404040`), `selection.bg_fill = BG_TEXT_SELECTION` (`#431407`), `selection.stroke = BORDER_SELECTED` (`#FB923C`), widget stroke `BORDER_STRONG` for inactive fields (kit inputs use `border-primary` `#404040`). Window and popup shadows: `Shadow::NONE` (kit dark shadows are alpha 0).
4. **Brand (R4).** `BG_BRAND` stays `#E36323`; hover `#F97316`, pressed `#EA580C`; the label on all three is `TEXT_ON_BRAND` `#0A0A0A` (5.76 / 7.06 / 5.56:1); the focus ring is `FOCUS_RING` `#F97316`. The kit's purple brand values never enter the code.
5. **Pressed fill.** On `BG_PRESSED` (`bg-quaternary` `#404040`) only `TEXT` and `TEXT_SECONDARY` may sit (`text-tertiary` 4.11:1, accent 3.70:1, placeholder 2.19:1 fail).
6. **Error text.** Text uses `TEXT_ERROR` `#F87171`; `DANGER` `#EF4444` is for icons, dots and strokes only (4.02:1 on `bg-tertiary`).
7. **Placeholder.** `TEXT_PLACEHOLDER` (new name) is `#A3A3A3` pending OQ-1 (kit `#737373` is 3.78:1 on `#171717`).
8. **Non-text graphics.** `ICON_QUATERNARY` and `VAR_SCROLL_THUMB` (`#737373`) need 3:1 and never sit on `BG_PRESSED` (2.19:1).
9. **Disabled.** No disabled colour token (v8 has none); a disabled control draws at opacity 0.5 (kit `Buttons/Button` State=Disabled). `TEXT_DISABLED` = `text-secondary` at 50% for text-only cases.
10. **No shadows.** `SHADOW_COLOR`, `SHADOW_COLOR_STRONG`, `ELEVATION_400_SHADOW`, `ELEVATION_400_TOP` become transparent; the surface specs drop the draw calls. `ELEVATION_400_RING` and `INNER_HIGHLIGHT` become the `border-secondary_alt` hairline `#262626`.
11. **Purple Ban.** No chrome colour in `theme.rs` has hue 255–300° with saturation > 0.25; `BOARD_*` (board content) is exempt. `SYNTAX_KEYWORD` becomes `utility-pink-600` `#F472B6`. `BANNED_PURPLES` gains `#a259ff`, `#c792ea`, `#7f56d9`, `#9e77ed`, `#b692f6`, `#6941c6`.
12. **Unchanged.** Canvas overlays, Dev overlay, info family, component cyan, agent presence, avatar, checker, board and the `u32` document defaults keep their values (controller defaults; token-contract.md (e)).
13. **Colour-literal gate.** New `scripts/check_chrome_colour_literals.py` (new name) counts chrome sites per file with spec D section 2's regex over `rust_literal_scan.clean` output, against `scripts/chrome_colour_baseline.json` (new name, written once from HEAD: 410 sites in 51 files). A file's count may only fall; an allowlist names identity-tint and canvas-exception lines (D, F and G list theirs). It joins the pre-commit list next to `check_spacing_relationships.py`. This spec retires no call-site literal; surface specs do.
14. **Store mirrors.** `design/tokens.json` is rewritten from `design.tokens.json` values (same keys) so no file prints a retired value. `design/specimen.html` is not regenerated with `ui ds preview` (section 8, gap T2); its printed values are refreshed in place.
15. **Agents.** No `ui agents init --force`: the project name (`designos-studio`) and studio name do not change, and `.claude/agents/*` read the soul at run time through `ui ds context`. The `.agents/agents/` copies are hand-edited (section 5.4).

## 4. Data model / schema changes

No schema change. `design.tokens.json` keeps its 48 colour keys and their roles; 41 values change, generation 28 → 69 (41 changelog entries, one per token, each reason naming the Untitled token). `theme.rs` loses 5 constants and gains 13 (new names): `BG_SECONDARY_HOVER`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY_HOVER`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `ICON_QUATERNARY`, `TEXT_ERROR`, `TEXT_WARNING`, `TEXT_SUCCESS`, `TEXT_ON_BRAND`, `FOCUS_RING`, `BG_ERROR_SOLID`, `TEXT_WHITE`. Untitled names with no store key live in `theme.rs` only (the store has no add path, kernel gap T1).

## 5. Contracts

### 5.1 Onboarding procedure (verified end to end in scratch; run it on the PR branch)

```bash
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
# 0. Kit values (local only, never committed): see token-contract.md section 0.
# 1. Re-value the store in place, one sanctioned mutation per token.
#    colours.tsv / scale.tsv = the tsv blocks in token-contract.md (b) and (c); B runs the scale loop.
while IFS=$'\t' read -r key value reason; do
  ui ds change-token "colors.$key" --value "$value" --reason "Untitled UI v8 dark: $reason" --dir . || exit 1
done < colours.tsv
ui ds status --dir .                       # expect: generation 69, tokens 77, components 12
# 2. Mirror the flat source file.
python3 -c 'import json;t=json.load(open("design/design.tokens.json"));f=json.load(open("design/tokens.json"));[f["colors"].__setitem__(k,t["colors"][k]["$value"]) for k in f["colors"]];json.dump(f,open("design/tokens.json","w"),indent=2);open("design/tokens.json","a").write("\n")'
# 3. Regenerate the component docs; refresh the specimen's printed values (no ds preview, gap T2).
ui ds docs --dir . --out design/DESIGN.md
#    specimen: rewrite every `  --colors-<key>: <hex>;` line, each swatch `<code>#bg / #fg · r:1</code>`,
#    the `generation N` header and `min r:1` from design.tokens.json + ds.manifest.json (WCAG ratio).
# 4. Prove it.
ui ds a11y --dir .
ui ds a11y --dir . --pairs "$(cat design/a11y-chrome-pairs.txt)"     # new file, 5.3
ui gate design/specimen.html --tokens design/design.tokens.json --json
ui ds-usage-lint design/specimen.html --dir . --json
ui ds soul check --dir .
ui agents check --dir .
python3 scripts/design_os_self_evolve.py && python3 scripts/zero_fake_ui_gate.py && design-os evolution
```

Pasted results from the scratch run (`proj-v` = copy of `design/` at `ec47da26`, re-valued with steps 1–3, drafts of section 5.5 copied in):

```text
$ ui ds status --dir .
ds: designos-studio (gen 79)          # 69 after the 41 colour re-values; 79 after B's 10 scale re-values
persona: imported / imported
tokens:  77 (semantic + primitive)
components: 12
$ ui ds a11y --dir .
ds a11y: 15 text×surface pair(s) checked ({role}/{role}-foreground pairs) + 8 interaction-state pair(s), 0 below AA (4.5:1).
$ ui ds a11y --dir . --pairs "<50 chrome pairs, 5.3>"
ds a11y: 50 text×surface pair(s) checked, 0 below AA (4.5:1).
$ ui ds a11y --dir . --pairs "<68 pairs: 4 text roles × 15 surfaces + brand/status>"     # proves the gate can fail
ds a11y: 68 text×surface pair(s) checked, 19 below AA (4.5:1).
  ✗ colors.neutral on colors.secondary-hover — 2.19:1 … (15 × colors.neutral, a fill not a text role)
  ✗ colors.accent-foreground on colors.secondary-hover — 3.7:1   ✗ … on colors.secondary-active — 3.7:1
  ✗ colors.muted-foreground on colors.secondary-hover — 4.11:1   ✗ … on colors.secondary-active — 4.11:1
$ ui gate design/specimen.html --tokens design/design.tokens.json --json   → pass true, errors 0, warnings 0
$ ui ds-usage-lint design/specimen.html --dir . --json                     → 0 errors (specimen refreshed in place)
$ ui ds soul check --dir .                                                  → 0 findings
$ ui agents check --dir .                                                   → 0 findings
$ ui ds context --dir . | grep -c Untitled                                  → 11 (the new soul and principles are served)
$ PYTHONDONTWRITEBYTECODE=1 python3 <g01 harness: repo G01 functions + the 5.2 edits> <scratch theme.rs> <scratch store>
G01 ok = True
35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 29 non-colour constants match
```

The same 50-pair list passes on the HEAD store too (`ui ds a11y --dir . --pairs …` on the repo, read-only: 50 checked, 0 below AA), so the list is a regression gate, not a one-off.

### 5.2 `scripts/design_os_self_evolve.py` (G01)

Exactly the edits in token-contract.md (d): `BANNED_PURPLES` +6, `CORE_RUST_CONSTANTS` +13 (35), 14 map rows changed or added, `DEV_READY` moved to `EXPECTED_STANDALONE_HEX` `#14ae5c`. Without them G01 fails on the scratch theme ("BG_PRESSED vs bg-active", "DEV_READY vs success").

### 5.3 G04 explicit chrome pairs

New `design/a11y-chrome-pairs.txt` (new name, one comma-separated line): `bg-foreground`, `muted-foreground`, `accent-foreground` × {`canvas`, `bg`, `bg-hover`, `bg-active`, `surface`, `surface-hover`, `surface-active`, `card`, `popover`, `input`, `muted`, `secondary`, `accent`}; `bg-foreground` × {`secondary-hover`, `secondary-active`}; `primary-foreground` × {`primary`, `primary-hover`, `primary-active`}; `destructive-foreground:destructive`, `success-foreground:success`, `warning-foreground:warning`, `info-foreground:info`, `component-foreground:component`, `neutral-foreground:neutral` — every path prefixed `colors.` (without the prefix `ui ds a11y` reports "not checked"). `check_ui_ds_a11y` (:365) runs both the default and the `--pairs` call and needs 0 below AA in each.

### 5.4 Design agents

- `.claude/agents/*-jang-designos-studio.md`: no edit. `ui agents check --dir .` must stay "0 findings".
- `.agents/agents/curator-jang-designos-studio.md:20-23`: D2 sizes to B's values (rows 32, fields 32, tool buttons 32, panel header 40); D3 to "Inter text-xs 12/18, text-sm 14/20, text-xl 20/30; 9px canvas only"; D4 to "Untitled Neutral dark (`#0A0A0A` bg-primary, `#171717` bg-secondary, `#262626` bg-tertiary), `#E36323` Studio orange, `#06B6D4` component cyan; Purple Ban incl. `#7F56D9`/`#9E77ED`"; D5 to "hover `#171717`/`#262626`, pressed `#404040`, selected `#262626`, focus ring `#F97316`".
- `.agents/agents/designer-jang-designos-studio.md:16`: Purple Ban list = `BANNED_PURPLES` after 5.2.
- `.agents/agents/librarian-jang-designos-studio.md:23`: "14 core constants" → "35 core constants".
- Check: `grep -rniE '#(121215|18181b|1e1e22|232329|2e2e36|2b1d16)|Zinc' .agents .claude design/soul.md design/principles.md` prints nothing.

### 5.5 Soul and principles

`design/soul.md` = `specs/A-soul-draft.md` verbatim; `design/principles.md` = `specs/A-principles-draft.md` verbatim (copy, no edit). Soul check on the drafts (scratch copies `<scratch>/proj/design/` and `<scratch>/proj-v/design/`):

```text
$ ui ds soul check --dir <scratch>/proj
ds soul check: <scratch>/proj/design/soul.md — 0 findings.        (exit 0)
$ ui ds soul check --dir <scratch>/proj --json   → errorCount 0, warningCount 0, findings 0
$ ui ds soul check --dir <scratch>/proj-v
ds soul check: <scratch>/proj-v/design/soul.md — 0 findings.
$ ui ds soul check --dir .   (repo HEAD, read-only, baseline)
ds soul check: …/design-os-studio/design/soul.md — 0 findings.
```

`ui ds soul check` lints the structure of `soul.md` only (sections present, not empty, no placeholder, status); it reads neither evidence paths nor `principles.md`. Evidence paths were checked separately: every path named in the soul draft exists at `ec47da26`; the principles draft names one path that this spec creates (`scripts/check_chrome_colour_literals.py`).

## 6. File-format mapping

No `.fig` / `.designos` change. Token mapping is DTCG-to-DTCG: Untitled names (kit `1_ Color modes`, Dark mode, aliases resolved) map to Studio role keys per token-contract.md (b). `ui ds import` normalises `_` to `-`; this spec never imports, so store keys stay Studio's.

## 7. UI

- UI implementers load the `es-designer` skill first, then read `design/principles.md`, `design/soul.md`, `ui ds context --dir .` and `theme.rs`; the project's own design system wins over any generic default.
- Visible change: every chrome surface turns from Zinc to Untitled Neutral, status colours move, syntax colours change, shadows disappear. No layout change (R1); B owns sizes.
- Visual evidence goes to the PR, not Figma. Commands (run from the repo root; outputs under `plans/261025-1247-untitled-ui-dark-revamp/evidence/A/`):

```bash
cargo run -q --release -p designos-app --bin designos -- --home --screenshot evidence/A/home.png --size 1600x1088 --no-toast
cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --screenshot evidence/A/editor-kit.png --size 1600x1088 --no-toast
for p in cmdk terminal assets variables picker dev-compare; do
  cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --panel "$p" --screenshot "evidence/A/panel-$p.png" --size 1600x1088 --no-toast
done
```

`$KIT_COPY` is a `.designos` copy of the Untitled kit (never the original). Compare against `evidence/baseline-home.png` and `evidence/baseline-editor-with-kit.png`.
- responsive: SKIPPED (owner default).

## 8. Edge cases

- **T1 — no add path in the sealed store (kernel gap, owner: `ui` kernel in `ease-design`, related to dogfood spec P).** Untitled names without a Studio key cannot be added. Workaround: re-value existing keys; keep the rest as `theme.rs` constants guarded by tests and `RUST_ONLY`.
- **T2 — `ui ds preview` output fails `ui ds-usage-lint` on this store (kernel gap, owner: `ease-design`).** Regenerating the specimen at HEAD in scratch gives 69 errors (`var(--color-ring)`, `var(--font-size-xs)` … "declared nowhere", plus hard-coded hex), because the generator writes canonical `--color-*`/`--font-size-*` names while the store's groups are `colors.*`/`typography.*`. The committed specimen passes (0 errors). Workaround: refresh its printed values in place (5.1 step 3); measured: 51 custom properties, 15 swatches, 6 radius chips rewritten, `ui gate` pass with 0 errors, usage-lint 0 errors, no retired hex left.
- **T3 — `ui ds import --force` wipes the registry and changelog (by design).** Never used here.
- **T4 — default `ui ds a11y` on an Untitled-named store builds cartesian pairs (1392, 731 "failing") or, on raw DTCG, measures 0 pairs (false green).** Always pass `--pairs` with `colors.` paths.
- `BG_HOVER == BG_SECONDARY` (`#171717`): any surface that is `BG_SECONDARY` uses `BG_SECONDARY_HOVER` for hover (rule 3); surface specs own the call sites.
- `BG_SELECTED == BG_SELECTED_HOVER` (`#262626`, OQ-2): the kit keeps the fill under the pointer; no extra state colour.
- `TEXT_SECONDARY`/`TEXT_TERTIARY` turn from alpha to opaque: text drawn over images or the canvas (none found in chrome) would no longer blend.
- `terminal.rs` pins `SUCCESS` and `WARNING` hex in a test (:10019-10021): update the two literals in the same commit.
- Constants deleted with 0 references: a reference added between HEAD and the PR re-anchor breaks the build; re-run the grep before deleting.

## 9. Non-goals

- Sizes, type, spacing, radii (spec B). Widget anatomy and states (spec C). Re-pointing call-site literals to roles (D–K). Canvas and Dev overlay colours (controller default). Light mode. Untitled icons (R6, spec Q). Fixing kernel gaps T1/T2 in `ease-design` (recorded, not fixed here). Regenerating `.claude/agents` (`ui agents init`).

## 10. Test plan (red first)

Red at HEAD, green after:

1. `theme.rs` `no_chrome_colour_is_purple` (new test): hue scan of every `Color32` constant except `BOARD_*`; red at HEAD (`SYNTAX_KEYWORD` `#C792EA`, 276°).
2. `theme.rs` `brand_label_passes_aa` (new test): WCAG ratio of `TEXT_ON_BRAND` on `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED` ≥ 4.5; red at HEAD (no `TEXT_ON_BRAND` constant exists).
3. `theme.rs` `chrome_text_passes_aa` (new test): `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER`, `TEXT_ERROR` on `BG`, `BG_SECONDARY`, `BG_TERTIARY`, `BG_SELECTED` ≥ 4.5, and `TEXT`/`TEXT_SECONDARY` on `BG_PRESSED`; red at HEAD (`TEXT_TERTIARY` `rgba(0xF4F4F5, 0x66)` blends to 3.84:1 on `BG`).
4. Existing selection-fills-neutral test (:900-906) keeps passing with the new values.
5. `terminal.rs` colour pins updated to `#22C55E`/`#EAB308`; `INFO` pin unchanged.
6. G01: `python3 scripts/design_os_self_evolve.py` green with 35 core constants; a fixture run with `SYNTAX_KEYWORD` set back to `#C792EA` fails on the Purple Ban.
7. `scripts/check_chrome_colour_literals.py`: a test injects one `Color32::from_rgb(1, 2, 3)` into a temp copy of a file and expects FAIL; HEAD passes against the written baseline.
8. G04 with 5.3 pairs green; the 68-pair list shows it can fail.

Acceptance criteria:

1. `theme.rs` colour constants equal token-contract.md (a) (116 rows + 13 new), checked by the G01 run and by `grep` of each new literal.
2. `ui ds status --dir .` → generation 69 after this PR, 77 tokens, 12 components; `design.tokens.json` colours equal token-contract.md (b).
3. `ui ds a11y --dir .` and `ui ds a11y --dir . --pairs "$(cat design/a11y-chrome-pairs.txt)"` → 0 below AA.
4. `python3 scripts/design_os_self_evolve.py` all gates green, G01 "35/35 … 0 Purple Ban violations".
5. `ui ds soul check --dir .` → 0 findings, `status: ratified`; `design/soul.md` and `design/principles.md` byte-equal the drafts.
6. `ui agents check --dir .` → 0 findings; the 5.4 grep prints nothing.
7. `ui gate design/specimen.html …` pass and `ui ds-usage-lint` 0 errors; no retired hex printed in `design/`.
8. `cargo test -p designos-app` green incl. tests 1–3 and 5; `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/check_chrome_colour_literals.py`, `design-os evolution` green.
9. Evidence PNGs of section 7 attached to the PR; "responsive: SKIPPED (owner default)" stated in the PR.

## 11. Performance budgets

Constants only; no runtime cost. Idle CPU stays `0.00%` (`CLAUDE.md` §4 method). Gate runtime: the new colour-literal gate scans `crates/designos-app/src` in under 2 s (same scan cost as `check_spacing_relationships.py`).

## 12. Dependencies and build order

1. This PR (A) first. 2. Spec B edits the same `theme.rs`, `design.tokens.json` and `non_colour_token_parity.py`: one writer, B's PR opens after A merges (sequential). 3. C, then D–K re-point call sites using the constants named here. 4. M copies the merged table. Kernel gaps T1/T2 go to `ease-design` as issues (engineering writer, spec P covers the export side).

## 13. Owner rulings that apply

R1 (adopt 100% Untitled style, keep layout), R2 (take everything from the kit), R3 (dogfood, no Figma: onboarding uses `designos --export-variables` and `ui ds`), R4 (Studio orange `#E36323`, dark label `#0A0A0A`, focus ring `#F97316`, Purple Ban stays), R6 (icons keep their set; colours from `fg-*`), R7 (spec-driven). Controller defaults: canvas, Dev overlay, info, component cyan and document content keep their values; chrome status colours move to Untitled.

## Open questions for the owner

1. **OQ-1 Placeholder colour.** Recommended: `#A3A3A3` (Neutral-400, 7.11:1 on `#171717`). Alternative: kit `text-placeholder` `#737373` (3.78:1 on `#171717`, 4.18:1 on `#0A0A0A`: fails AA).
2. **OQ-2 Selected row fill.** Recommended: `#262626` (`bg-secondary_hover`, one step above hover so selection reads). Alternative: kit-literal `bg-secondary` `#171717`, which equals hover in dark mode.
3. **OQ-3 Accent text (`TEXT_BRAND`).** Recommended: Studio orange `#F97316` (7.06:1 on `#0A0A0A`; marks New/Fix/undo steps). Alternative: kit-literal `text-brand-secondary`, which is neutral `#D4D4D4` in dark mode.

## Decisions on the open questions (designer, 2026-10-25)

- Re-value in place with `ui ds change-token`, never `ui ds import --force` (keeps 12 components, changelog and roles; proj-r/proj-v runs).
- Store keys stay Studio's role names; Untitled names live in the change-token reasons, token-contract.md and `theme.rs` doc comments.
- `BG_PRESSED` maps to `secondary-active`, `BG_SELECTED` to `bg-active`; `DEV_READY` leaves the G01 map for `EXPECTED_STANDALONE_HEX`.
- `SUCCESS`/`WARNING`/`DANGER` move to Untitled `fg-*` (controller default); `INFO` stays.
- Syntax colours take the kit's dark utility steps; `SYNTAX_KEYWORD` pink-400 fixes the HEAD Purple Ban breach.
- The colour-literal gate requested by D/E/F/G is adopted here with a falling per-file baseline.
- Specimen refreshed in place, not regenerated (gap T2); `design/tokens.json` mirrored.
- The soul keeps `status: ratified`: its stance is the owner's rulings R1–R7; the owner's PR approval is the ratification act (Jang approves every merge).
- No `ui agents init --force`; `.agents/agents/` copies are hand-edited.
