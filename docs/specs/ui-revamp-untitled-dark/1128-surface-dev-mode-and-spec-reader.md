# G Dev mode (status, sections, code, playground, Compare changes, annotations) and the spec reader in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1128 (epic #1118, spec key G) |
| Size | M. Three days: 11 owned files, 52 literal colour sites (42 in `specs.rs`, 10 in `dev/`), the syntax palette, one Dev accent role used by four other specs, R5 sizes on the Dev buttons, badges and tabs. The spec reader part is half a day of it and can ship separately (G2) because the reader is not reachable in the app today (section 2). |
| Depends on | A (colour roles and `theme.rs` colour constants, including the `SYNTAX_*` and `DEV_*` values this spec chooses), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). |
| Unblocks | D (Dev toggle and the Dev tab badge), E (Dev section in the left panel), F (Share and the Plugins row in Dev mode): they use the Dev accent role decided here. M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/dev/sections.rs` (`:16-34` status pill, `:36-42` primary button, `:45-99` status header, `:151-330` annotations and links, `:332-380` box model, `:386-419` code block, `:421-468` code section, `:485-683` variables and assets); `dev/highlight.rs` (`:33-45` token colours); `dev/list.rs` (`:40-77` tabs, `:81-168` Ready for dev list); `dev/compare.rs` (`:53-70` pane, `:83-195` Compare changes window); `dev/playground.rs` (`:315-335` toggle switch, `:451-577` playground section); `dev/annot.rs` (`:65-161` canvas callouts, colours unchanged); `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` (no colour or size change; listed for ownership); `crates/designos-app/src/specs.rs` (`:29-91` reader palette, `:2278-2632` left explorer, `:2711-2909` reader, `:2910-3768` right column and role views). Reads only: `theme.rs:555-582` (`SYNTAX_*`, `DEV_*`, `STATUS_INFO_*`, `BOX_*`, `ANNOTATION_*`, `MEASURE`; A writes them). |

Code baseline: HEAD `ec47da26` (the local `main` this spec was written on). `origin/main` is `35a65796`, four commits ahead (release 0.6.1); it changes `dev/mod.rs` (7 changed lines) and `specs.rs` (46) (`git diff --stat ec47da26 35a65796 -- crates/designos-app/src/dev crates/designos-app/src/specs.rs`). The colour-site counts in section 2 are identical at `35a65796`; only line numbers move. Re-anchor every cited line against the HEAD you branch from and report drift before writing code.

Colour names follow the plan's naming contract: Untitled token name plus the dark-mode hex, resolved this session from the kit export (`1_ Color modes`, Dark mode and Light mode). Brand roles carry the R4 remap. Provisional names wait for spec A's token contract, which wins on conflict. Names Untitled does not have are marked "new name".

## 1. Summary and user story

As a developer in Dev mode, I see Studio's Inspect panel, the Ready for dev list, the Compare changes window and the generated code in Untitled UI dark: neutral surfaces, Untitled badges and buttons, a syntax palette from Untitled's utility colours with no purple, and one Dev accent colour wherever Dev mode is shown. Canvas callouts, box-model tints and measurements keep the colours developers know from Figma. The spec reader, when it is shown again, uses the same Untitled roles instead of its own palette.

## 2. Current state (FACT unless labelled)

- **Where Dev mode is drawn.** Toolbar Dev toggle and tab badge (spec D), Dev section of the left panel (`panels/left.rs:93-160`, spec E, calling `dev/list.rs`), Share and the Plugins row (spec F), Inspect panel shell (`panels/inspect.rs`, spec F) calling `dev/sections.rs` (`inspect.rs:392-395`, `:582-586`, `:691`) and `dev/playground.rs`, Compare changes window (`dev/compare.rs::show`, `shell.rs:1546`), canvas callouts (`dev/annot.rs::overlay`, `canvas/mod.rs:400`), canvas status badges and measurements (`canvas/draw.rs`, not owned by D–K).
- **Dev accent today is five greens.** Theme `DEV_READY` `#14AE5C` (`theme.rs:565`) for the Ready status; literal `#1BC47D` for the Dev-mode accent in `tabs.rs:1003`, `tabs.rs:1090`, `panels/toolbar.rs:272`, `:315`, `panels/right.rs:66`, `:203`, `:274`, `panels/left.rs:97`, `:289`; `#22D489` for the toolbar toggle hover (`toolbar.rs:438`); `#34D399` text on a `#14AE5C` 15 % tint for the Dev list count badge (`dev/list.rs:68-69`); and `updater.rs:43` `UI3_EMERALD` `#1BC47D` (not Dev). White on `#1BC47D` is 2.27:1 and white on `#14AE5C` 2.90:1.
- **Status pill** (`sections.rs:25-34`): 20 px pill, status colour at 22 % (`gamma_multiply(0.22)`), 3.5 px dot, `TEXT` label in `font_caption_medium` 10. Colours `DEV_READY` `#14AE5C`, `DEV_COMPLETED` `#06B6D4`, `DEV_CHANGED` `#FFA629` (`theme.rs:565-567`). On the target surfaces (`#0A0A0A`, `#171717`) a `#FAFAFA` label reaches 10.97:1 or more on every tint (computed this session).
- **Primary buttons** ("Ready for dev", "Mark completed", "Compare changes", "Back to ready", "Save", "Add link"; `sections.rs:36-42`, `:62-93`, `:165-170`, `:266-270`): 28 px pills in `BG_BRAND` `#E36323` with a `TEXT` (`#F4F4F5`) label, 3.14:1.
- **Code block** (`sections.rs:386-419`): `BG_SECONDARY` fill, `RADIUS_CONTROL`, `font_mono` 11, line height `CODE_LINE_HEIGHT` = `LINE_BODY`; token colours from `highlight.rs:33-45` mapping to `SYNTAX_*` (`theme.rs:555-563`): plain `TEXT`, property `#7CC4F8`, keyword `#C792EA` (violet, against `design/soul.md` line 15: "Never use purple or violet for chrome, badges, focus rings, gradients or syntax colours"), string `#9CD67D`, number `#F2A365`, type `#FFCB6B`, function `#82AAFF`, comment `#8A8A8A`, punctuation `#B3B3B3`.
- **Box model** (`sections.rs:332-380`): devtools tints `BOX_MARGIN` `#F9CB9C`, `BOX_PADDING` `#B6D7A8`, `BOX_CONTENT` `#9FC5E8` with `BOX_TEXT` `#1E1E1E` (9.23:1 or more).
- **Ready for dev list** (`list.rs`): tabs 24 px (`:51`), rows 32 px (`:78`), count badge `#14AE5C` at alpha 38 with `#34D399` text (`:68-69`).
- **Compare changes** (`compare.rs`): full-window scrim `from_black_alpha(140)` (`:97`), two image panes (`:53-70`; `:62` passes `Color32::WHITE` as the image tint).
- **Playground** (`playground.rs`): switch knob literal white (`:332`), preview stage `#1E1E1E` (`:470`), dot grid white at alpha 18 (`:478`), preview image tint white (`:499`).
- **Canvas callouts** (`annot.rs:65-161`): `ANNOTATION_BG` `#FFE08A`, `ANNOTATION_TEXT` `#1E1E1E`, `ANNOTATION_LINE` `#FFB800` (`theme.rs:578-580`); no literals.
- **Spec reader** (`specs.rs`). Its own palette, not the theme: ten dark constants `TOKEN_*` (`:32-41`), three status dots `DOT_*` (`:44-46`), a reader light theme (`:66-75`, toggled by `specs-theme-toggle`, `:2741-2745`) and the dark sheet colour (`:80`). Accent literal `#0D99FF` in 12 sites: active-row bars (`:2431`, `:2493`, `:2967`), tab underline (`:2814`), ask-agent sparkle and send button (`:2849`, `:2853`; the same line holds the `#25A8FF` hover, and `:2855-2857` draw the arrow in `Color32::WHITE`), "Reference @spec in Agent" outline (`:3100`), status pill outline and text (`:3231`, `:3233`), live canvas embed outline and text (`:3289`, `:3295`), user-story quote bar (`:3467`). Highlight `#FEF08A` / `#F5C542` at alpha 46 (`:3272`). 20 literal font sizes (`FontId::proportional(15|16|20|22)`).
- **The spec reader is not reachable in the app (FACT by code reading; capture G-05 below).** `shell.rs:1424` hard-codes `let specs_reader_open = false;`, so `specs::show_reader` and `specs::show_right` (`shell.rs:1432`, `:1518`) never run; `specs::show_left` is called only from tests (`specs.rs:4024`, `:4070`); `RailItem::Specs` is left out of the rail (`panels/rail.rs:10-18`). The Home "+ Spec bundle" button and template card (`home/mod.rs:1266-1268`, `:1532`) and `--panel specs` (`shell.rs:310-314`) still set `RailItem::Specs`. Tests that draw the reader: `specs::tests::renders_specs_reader_and_left_panel_without_lint_issues` (`specs.rs:4007`) and `specs::tests::specs_interactive_controls_mutate_state_and_dispatch_to_agent` (`:4045`).

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Literal sites | Site lines | Computed sites |
|---|---|---|---|---|
| `specs.rs` | 128 | 42 | 32-41, 44-46, 66-75, 80, 2431, 2493, 2814, 2849, 2853 (×2), 2855, 2856, 2857, 2967, 3100, 3231, 3233, 3272 (×2), 3289, 3295, 3467 | none |
| `dev/playground.rs` | 32 | 4 | 332, 470, 478, 499 | none |
| `dev/compare.rs` | 24 | 2 | 62, 97 | none |
| `dev/list.rs` | 23 | 2 | 68, 69 | none |
| `dev/sections.rs` | 73 | 0 | | 481 (document colour) |
| `dev/annot.rs`, `dev/highlight.rs` | 13, 9 | 0 | | none |
| `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` | 0 | 0 | | none |
| **Total** | 302 | **52** | | 1 |

Command (run from the repo root; writes nothing):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - dev/mod.rs dev/annot.rs dev/color.rs dev/compare.rs dev/data.rs dev/highlight.rs dev/list.rs dev/playground.rs dev/preset.rs dev/sections.rs specs.rs <<'PY'
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

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' crates/designos-app/src/specs.rs`, tests included) reports `specs.rs` 37, which is the plan's "37 colour literals"; the census adds the 5 named `Color32::WHITE` sites.

Classification of the 52 sites: 2 identity image tints (`compare.rs:62`, `playground.rs:499`; allowlisted) and 50 chrome colour choices. The one computed site (`sections.rs:481`) converts a document colour for a variable swatch and stays.

Gates today: no colour-literal gate (spec D section 5 proposes one, owned by A). `scripts/check_spacing_relationships.py` ratchets geometry and font literals (`specs.rs` and `dev/*` have baseline entries). `scripts/zero_fake_ui_gate.py` Z05–Z07 cover `specs.rs`; Z11 covers the Dev actions in the Inspect panel. `scripts/design_os_self_evolve.py` G01 checks 15 theme colours against `design/design.tokens.json` and the banned purples (`#9747FF`, `#8755E9`, `#C4A1FF`, `#A855F7`); `SYNTAX_KEYWORD` `#C792EA` is violet but not on that list, so no gate catches it today.

## 3. Behaviour rules

1. **Chrome or content.** Syntax highlighting is chrome: it colours code Studio generates, in a Studio panel, and the soul's purple rule names syntax colours. Box-model tints, canvas callouts (`ANNOTATION_*`), measurements (`MEASURE`) and the canvas Dev badges are Dev overlays: they keep their values (controller default 2026-10-25). The spec reader is chrome.
2. **One Dev accent role** (new name `dev-accent`; A names the constant). Every place that says "you are in Dev mode" uses it: toolbar toggle and its hover (D), the Dev tab badge (D), the Dev section icon in the left panel (E), the Plugins row (F), the Dev list count badge (G). Value: `DEV_READY` `#14AE5C` (controller default "Dev ready keeps `#14AE5C`"; Open question 1). A label or glyph on a filled Dev accent is `on-brand-label` `#0A0A0A` (new name for the R4 dark label; `Neutral-950` in spec C's draft; 6.82:1). The hover is a new name derived by A (today `#22D489`). `#1BC47D`, `#22D489` and `#34D399` leave Dev chrome.
3. **Role map** (provisional names until A lands; A wins):

| Element | State | Role (Untitled dark) | Today |
|---|---|---|---|
| Section titles in the Dev sections (Code, Variables, Assets, Annotations, Links, Playground) | default | `text-primary` `#FAFAFA`, `text-sm` 14/20 semibold (via F's `section_title`) | `TEXT`, 11 |
| Captions, dates, token notes | default | `text-tertiary` `#A3A3A3` | `TEXT_TERTIARY` (`#F4F4F5` at 40 %) |
| Status pill | Ready / Completed / Changed | C badge (`Badge · Type=Pill color · Size=sm`, C name TBD) with dot: fill = status colour at 22 %, dot = status colour, label `text-primary`; status colours stay `DEV_READY` `#14AE5C`, `DEV_COMPLETED` `#06B6D4`, `DEV_CHANGED` `#FFA629` (one state, one colour on canvas and panel) | same colours, 20 px pill |
| Dev primary buttons (Ready for dev, Mark completed, Compare changes, Back to ready, Save, Add link) | default, hover, focused, disabled | `Buttons/Button · Size=xs · Hierarchy=Primary`: `bg-brand-solid` `#E36323`, label `on-brand-label` `#0A0A0A` (5.73:1), hover per C, focus ring `#F97316` | `BG_BRAND` + `TEXT` (3.14:1) |
| Secondary actions (copy, download, small buttons from `inspect.rs`) | default, hover | `Buttons/Button · Size=xs · Hierarchy=Secondary` (F owns `small_button`) | `BG_SECONDARY` |
| Annotation and link editors (text fields, property chips) | default, hover, focused, selected chip | C `Input field · Size=sm`; chips C tag, selected = C Selected (neutral) | `BG_SECONDARY`, `BG_SELECTED`, `BORDER_SELECTED` |
| Code block | default | C code snippet surface (C name TBD): `bg-secondary` `#171717`, `border-secondary` `#262626`, radius B's `radius-md` 8, Roboto Mono (P5, D6) | `BG_SECONDARY`, no border |
| Syntax: plain | default | `text-primary` `#FAFAFA` (17.18:1 on `#171717`) | `TEXT` |
| Syntax: property | default | `utility-sky-600` `#38BDF8` (8.37:1) | `#7CC4F8` |
| Syntax: keyword | default | `utility-pink-600` `#F472B6` (6.77:1; hue 329°, outside violet; decision 3) | `#C792EA` violet |
| Syntax: string | default | `utility-green-600` `#4ADE80` (10.29:1) | `#9CD67D` |
| Syntax: number | default | `utility-orange-600` `#FB923C` (7.92:1) | `#F2A365` |
| Syntax: type | default | `utility-yellow-600` `#FACC15` (11.71:1) | `#FFCB6B` |
| Syntax: function | default | `utility-blue-600` `#60A5FA` (7.05:1) | `#82AAFF` |
| Syntax: comment | default | `text-tertiary` `#A3A3A3` (7.11:1) | `#8A8A8A` |
| Syntax: punctuation | default | `text-secondary` `#D4D4D4` (12.09:1) | `#B3B3B3` |
| Box-model diagram | default | Dev overlay: `BOX_*` tints unchanged; band labels `BOX_TEXT` unchanged | same |
| Ready for dev list tabs | default, hover, selected | C segmented control (as F's tab strip) | 24 px, `BG_SECONDARY` |
| Ready for dev list rows | default, hover, selected | E's tree-row roles (C row anatomy) | `BG_HOVER`, `BG_SELECTED` |
| Ready for dev count badge | default | C badge: fill `dev-accent` at 22 % (same rule as the status pill), label `text-primary` | `#14AE5C` α38 + `#34D399` |
| Compare changes scrim | open | C's modal backdrop role (C name TBD; Untitled `bg-overlay` `#262626` at C's opacity) | `from_black_alpha(140)` |
| Compare changes window, panes, change list | default | C modal: `bg-primary` surface, `border-secondary` panes, `text-primary` titles, `text-tertiary` captions | `BG_*`, `TEXT_*` |
| Playground switch | off, on | C toggle: knob `fg-white` `#FFFFFF`, track on `bg-brand-solid` | white knob, `BG_BRAND` |
| Playground preview stage | default | `bg-secondary` `#171717` with the dot grid as a new-name constant (`text-primary` at 7 % alpha, today white at alpha 18) | `#1E1E1E`, white α18 |
| Canvas callouts, measurements, canvas Dev badges | any | Dev overlay constants unchanged (`ANNOTATION_*`, `MEASURE`, `DEV_*`) | same |
| Image tints (`compare.rs:62`, `playground.rs:499`) | n/a | identity tint, allowlisted | `Color32::WHITE` |

4. **Spec reader roles.** The reader keeps its two themes (dark default, light toggle) and its layout. Every `ReaderPalette` field maps to an Untitled role, dark value from A's contract, light value from the kit's Light mode (A adds the light set; new names):

| `ReaderPalette` field | Role | Dark | Light | Today (dark / light) |
|---|---|---|---|---|
| `bg_page` | `bg-secondary_alt` | `#0A0A0A` | `#FAFAFA` | `#141518` / `#F6F6F7` |
| `doc_sheet` | `bg-primary_alt` | `#171717` | `#FFFFFF` | `#17181C` / `#FFFFFF` |
| `surface` | `bg-primary` | `#0A0A0A` | `#FFFFFF` | `#191A1E` / `#FAFAFB` |
| `surface_2` (active row, table head) | `bg-tertiary` | `#262626` | `#F5F5F5` | `#22242A` / `#F1F2F4` |
| `TOKEN_SURFACE_3` (hover, callout) | `bg-primary_hover` | `#171717` | `#FAFAFA` | `#1D1E23` / none |
| `code_bg` | `bg-secondary` | `#171717` | `#FAFAFA` | `#111215` / `#F4F4F6` |
| `border` | `border-secondary` | `#262626` | `#E5E5E5` | `#282A32` / `#E4E5E9` |
| `hairline` | `border-tertiary` | `#262626` | `#F5F5F5` | `#22242B` / `#ECEDF0` |
| `text` | `text-primary` | `#FAFAFA` | `#171717` | `#EDEEF0` / `#18181B` |
| `text_muted` | `text-secondary` | `#D4D4D4` | `#404040` | `#B8BCC4` / `#3F3F46` |
| `text_subtle` | `text-tertiary` | `#A3A3A3` | `#525252` | `#7E838E` / `#71717A` |
| `DOT_OK` / `DOT_WARN` / `DOT_NEUTRAL` | `fg-success-primary` / `fg-warning-primary` / `fg-quaternary` | `#22C55E` / `#EAB308` / `#737373` | `#16A34A` / `#CA8A04` / `#A3A3A3` | `#3ECF8E` / `#E5A93B` / `#7E838E` |

The 12 `#0D99FF` accents (with the `#25A8FF` hover and the white arrow): active-row bars and the tab underline take C's navigation and underline-tab anatomy (active indicator `fg-brand-primary`, R4 orange); the send button is `Buttons/Button · Size=xs · Hierarchy=Primary` icon-only (orange with a `#0A0A0A` arrow); the sparkle is the input's leading icon (`fg-quaternary`); "Reference @spec in Agent" is `Hierarchy=Secondary`; the status pill is a C gray badge; the canvas-embed outline and label take A's link role (as F's MCP URI); the user-story bar is `border-brand`. The highlight (`:3272`) takes `bg-warning-primary` (`#422006` dark, `#FEFCE8` light) under `text-primary`. Reader font literals (15, 16, 20, 22) move to B's text roles (`text-md` 16/24, `text-xl` 20/30, `display-xs` 24/32; B decides the mapping).

5. **Widgets (spec C).** Buttons, badges, toggle, input fields, tags, segmented control, code snippet, modal and scrim, dropdown (code language) and tooltip come from C. No Dev-only widget is added.
6. **States.** Buttons: C's Default, Hover, Pressed, Focused, Disabled. Status pills: one per Dev status, no hover. Reader rows: Default, Hover, Active. No state is added or removed.
7. **Sizes (R5) and their layout effect.** The Inspect panel keeps its 240 px width and section order; the reader keeps its three columns (R1).

| Element | Code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Dev primary buttons | `sections.rs:61-62`, `:165`, `:266` | 28 | button `xs` 32 | +4 each | the status header grows 4 px (one button row); the annotation editor and link editor rows grow 4 px each |
| Status pill | `sections.rs:29` | 20, caption 10 | C badge `sm` (Untitled: `text-xs` 12/18 + 2 px padding = 22) | +2 | the status row stays 28 px tall (`sections.rs:60`); the pill sits 2 px taller inside it |
| Code text | `font_mono` 11, `CODE_LINE_HEIGHT` (= `LINE_BODY` 16, `theme.rs:124`, `:583`) | 11 / 16 | `text-xs` 12/18 mono | +2 per line | a 30-line CSS block grows about 60 px; the Inspect panel already scrolls; fewer columns per line, so `highlight::wrap` wraps more |
| Ready for dev tabs | `list.rs:51` | 24 | C segmented (F Open question 1 decides 24 or 32) | 0 or +8 | at 32 the list starts 8 px lower |
| Ready for dev rows | `list.rs:78` | 32 | row 32 | 0 | none |
| Playground switch | `playground.rs:318` | 28×16 | C toggle `sm` | per C | none |
| Reader text | `specs.rs` font literals | 15, 16, 20, 22 | B's roles | +1 / 0 / 0 / +2 | reader prose reflows inside the same columns |

8. **No behaviour change.** Widget ids (`dev-ready`, `dev-complete`, `dev-compare`, `dev-reopen`, `dev-annot-save`, `dev-link-add`, `dev-playground-preview`, `specs-*`, `hero-copy-ai`, `spec-inline-canvas-embed`), the code generators, copy actions, Compare changes, Z05–Z07 and Z11 stay. Re-enabling the spec reader is out of scope (Open question 2).

## 4. Data model / schema changes

None. Dev status, annotations and links in the document are unchanged.

## 5. Contracts

**File ownership across D–K.** G owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `dev/sections.rs`, `dev/playground.rs` | G | drawn inside F's Inspect panel; F owns the panel shell, `section_title`, `small_button`, `copy`, `download` (`panels/inspect.rs`) |
| `dev/highlight.rs` | G | the `SYNTAX_*` values are chosen here; the constants live in `theme.rs` (A writes them) |
| `dev/list.rs` | G | drawn inside E's left panel (`panels/left.rs:93-160`); E owns the panel and its Dev-section literals (`left.rs:97`, `:289`), which take G's Dev accent |
| `dev/compare.rs` | G | the window and scrim use C's modal anatomy |
| `dev/annot.rs` | G | canvas overlay; colours unchanged |
| `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` | G | no change expected |
| `specs.rs` | G | the reader's `show_left` would draw into E's left-panel slot if re-enabled; still G's file |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/*` and `panels/mod.rs` (C; `widgets/picker.rs` is F's); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/inspect.rs`, `panels/right.rs` and the other inspector files (F); `home/*` (H); `terminal.rs`, `agent_*`, `skills/*`, `cmdk.rs` (I or J; they use `STATUS_INFO_*`, which this spec does not change); `canvas/draw.rs` (canvas, not in D–K: it draws the Dev badges and box tints with the overlay constants), `updater.rs` (`UI3_EMERALD`, not Dev; spec J or unassigned).

**Dev accent contract.** G decides the value and the label rule (rule 2); A adds the constant (new name); D, E and F replace their `#1BC47D` and `#22D489` literals with it. Until A lands, D, E and F reference the role by this spec's name.

**Colour-literal gate.** As spec D section 5 (A's proposed `scripts/check_chrome_colour_literals.py`, new name). G's allowlist: `dev/compare.rs:62` and `dev/playground.rs:499` (identity image tints). Until the gate exists, the section 2 command must print `chrome_sites=0` for every owned file except those two (1 each).

**Purple Ban gate.** A extends `BANNED_PURPLES` or adds a hue check in `scripts/design_os_self_evolve.py` G01 so a violet `SYNTAX_*` value fails (A owns the script change; G only states the need).

## 6. File-format mapping

None. Generated code text is unchanged; only its colours change.

## 7. UI

- UI implementers load the `es-designer` skill first and follow the project's design system: `design/principles.md`, `design/soul.md`, A's token contract, B's scale and C's widget kit win over generic defaults. Purple Ban stays (R4) and now covers the syntax palette.
- One primary action per view: in the Inspect panel the Dev primary button is the only `Hierarchy=Primary` besides Share (F); both are orange. Dev mode is shown by the Dev accent, never by recolouring the primary action.
- Code, token paths and hex values stay in Roboto Mono (P5, D6).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. Long generated code lines at 12 px: `highlight::wrap` wraps by column count; continuations keep their indent (test `dev::highlight::tests::wrapping_keeps_every_character_and_indents_continuations`).
2. A layer with no Dev status: no pill; the "Ready for dev" primary button shows.
3. A Changed layer: amber pill and "Compare changes"; the Compare window opens over the scrim and closes on Escape as today.
4. Very long annotation text and many property chips: wrap inside the 240 px panel; chips wrap to new rows.
5. Light documents in the Compare panes: the panes keep a neutral `border-secondary` frame so a white layer is still bounded on `bg-primary`.
6. Playground with no boolean or variant props: the section shows its empty text in `text-tertiary`.
7. Spec reader light mode: every field resolves to the Light mode column; no dark constant leaks (test below).
8. Kotlin, Swift, CSS, Tailwind, HTML and UIKit code: all token kinds map to a role; no `Tok` falls back to a literal.

## 9. Non-goals

- Re-enabling the spec reader in the rail or the canvas (`shell.rs:1424`, `rail.rs:10`), reader layout, new role views.
- Canvas overlay colours (callouts, measurements, canvas Dev badges, box-model tints), `canvas/draw.rs`.
- Code generation, `dev/color.rs` value strings, Dev data model.
- `STATUS_INFO_*` users in agent surfaces (I, J).

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for every owned file except `dev/compare.rs` and `dev/playground.rs` (1 each, the allowlisted image tints).
2. No `#1BC47D`, `#22D489` or `#34D399` remains in Dev chrome (`grep -rnE '0x1B, 0xC4, 0x7D|0x22, 0xD4, 0x89|0x34, 0xD3, 0x99' crates/designos-app/src --include='*.rs'` prints only `updater.rs:43`, which is not Dev, unless J changes it).
3. Every `SYNTAX_*` value is non-violet and reaches 4.5:1 on the code block fill; `design_os_self_evolve.py` G01 fails if a violet value is put back.
4. `python3 scripts/check_spacing_relationships.py` passes; font and radius literal counts for `specs.rs` and `dev/*` fall or stay.
5. Spec L's layout gate shows the same regions, widths and order as the baseline for the Dev-mode captures; height changes match the section 3 table.
6. `python3 scripts/zero_fake_ui_gate.py` (Z05–Z07, Z11) and `python3 scripts/design_os_self_evolve.py` pass.
7. Before/after captures of every command below are in the PR with exit codes, plus the reader rendered by its test (the reader is not reachable from the CLI).

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | 0 (2 allowlisted) | yes (52 sites) |
| new `dev::highlight::tests::token_colours_are_untitled_utility_roles` (new name) | every `Tok` maps to the role in rule 3; none is violet (hue 260°–300°) | yes (`#C792EA`, hue 276°) |
| new `dev::highlight::tests::token_colours_reach_aa_on_the_code_fill` (new name) | each token colour is at least 4.5:1 on the code block fill | guard (passes today on `#1E1E22`, lowest `#8A8A8A` 4.81:1; must keep passing on `#171717`) |
| new `dev::sections::tests::primary_button_label_is_dark_on_brand` (new name) | the Dev primary button label is `on-brand-label` | yes (`TEXT`, 3.14:1) |
| new `dev::list::tests::count_badge_uses_the_dev_accent` (new name) | the count badge fill is the Dev accent tint and its label `text-primary` | yes (`#34D399`) |
| new `specs::tests::reader_palette_has_no_literals_in_either_mode` (new name) | `ReaderPalette::for_mode(false)` and `(true)` equal the theme role sets | yes |
| `specs::tests::renders_specs_reader_and_left_panel_without_lint_issues`, `specs::tests::specs_interactive_controls_mutate_state_and_dispatch_to_agent` | pass | guard |
| `dev::*` tests (`annot`, `color`, `highlight`, `list`, `playground`), `shell::tests::dev::*` | pass | guard |

Run: `cargo test -p designos-app --bin designos dev::`, `cargo test -p designos-app --bin designos specs::`, `cargo test -p designos-app --bin designos shell::tests::dev`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** The exit codes were recorded through the spec writer's wrapper (`run-shot.sh` in the writer's scratch folder, not in the repo), which ran `cargo run -q --release -p designos-app --bin designos -- <the same arguments>` from the repo root with an isolated `HOME` and absolute scratch paths. Each was run once on HEAD `ec47da26` by the spec writer with an isolated `HOME`; exit codes shown. `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original); `FIX` is the fixture built by spec E's `E-00` command.

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
FIX="$S/e-layers.designos"   # create it first with spec E's fixture JSON and its E-00 command
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"                # exit 1 twice ("designos: screenshot was not captured"): Dev toolbar, Inspect panel, CSS code block (shared with D and F)
RUN "$KIT" --screenshot "$OUT/E-04-dev-left.png" --size 1600x1088 --no-toast --dev css --dev-demo --select "Welcome to Untitled UI"   # exit 1 once ("designos: screenshot was not captured"): Ready status pill, annotation, link, Ready for dev list (shared with E)
RUN "$FIX" --screenshot "$OUT/G-03-dev-compare.png" --size 1600x1088 --no-toast --panel dev-compare --select "Revamp fixture"         # exit 0: Compare changes window and scrim, SwiftUI code
RUN "$FIX" --screenshot "$OUT/G-04-dev-annotate.png" --size 1600x1088 --no-toast --panel dev-annotate --select "Revamp fixture"       # exit 0: annotation editor, canvas callout
RUN "$KIT" --screenshot "$OUT/G-05-panel-specs.png" --size 1600x1088 --no-toast --panel specs                                          # exit 0: what --panel specs shows today (the reader is not drawn)
cargo test -p designos-app --bin designos specs::tests::renders_specs_reader_and_left_panel_without_lint_issues                       # exit 0 (1 passed): the reader's only render path today
```

Not capturable from the CLI today: the spec reader itself (section 2), hover and focus states, the playground (needs a component instance with properties selected in Dev mode), the reader's light mode. The PR covers the reader with the palette test above and, if the owner re-enables the reader (Open question 2), with captures then; the rest with paint tests and a manual window capture labelled manual.

## 11. Performance budgets

- Syntax colours stay `const`; `highlight::color` stays a `match` with no allocation.
- The Dev code cache (`DEV_CODE_CACHE`, `sections.rs`) is untouched; larger code text adds no regeneration.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4).

## 12. Dependencies and build order

- A, B, C first; L's baselines before the first PR. G's Dev accent decision (Open question 1) before D, E and F wire Dev mode. Build order: Dev accent and syntax values handed to A; `highlight.rs` and its tests; `sections.rs` (pill, primary button, code block, editors); `list.rs`; `compare.rs`; `playground.rs`; then `specs.rs` (G2); then R5 sizes; then evidence.
- **Open PRs touching G files** (`gh pr list --state open`, 2026-10-25): stale #770 `task/572-parallel-agents-frame-ownership`, #768 `task/599-auto-fix-settings-and-report-ui`, #765 `task/598-engine-auto-fix` change `dev/data.rs`, `dev/highlight.rs` and `dev/mod.rs`. No open PR touches `specs.rs`. #1106 and #1108 touch no G file.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R4 (Studio orange, dark label `#0A0A0A`, focus ring Orange-500 `#F97316`, Purple Ban), R5 (`text-xs` 12/18 rows and labels, `text-sm` 14/20 titles, button `xs` 32, row 32, `radius-md` 8), R6 (keep icons), R3 (no Figma), R7 (spec-driven).
- Controller default (2026-10-25): canvas and Dev overlays keep Figma-parity colours (Dev ready `#14AE5C`, component cyan `#06B6D4`, guides `#F24822`); chrome status colours move to Untitled.
- `design/soul.md`: no purple or violet in syntax colours. P2: one primary action per view. P5: code in Roboto Mono.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. Syntax highlighting is chrome and takes Untitled `utility-*-600` dark values (the kit's on-dark utility step), all at 6.7:1 or more on `#171717`.
2. Dev statuses keep their three colours in the panel and on the canvas, so one state has one colour everywhere; only the pill anatomy changes to C's badge.
3. Keyword becomes `utility-pink-600` `#F472B6` (hue 329°): it is the only utility hue left that is distinct from the other six token colours and is not violet (260°–300°), not red (read as an error) and not indigo (`#818CF8`, hue 234°, too close to function blue `#60A5FA`). `#C792EA` today is hue 276°.
4. Dev primary buttons stay brand orange (P2, soul: "the primary action is the primary orange"); green marks Dev mode, not actions.
5. The spec reader is restyled in this spec even though it is hidden, because its 42 literals would otherwise be the largest exception to A's literal gate; it ships as G2 and can wait.
6. The reader's light theme maps to Untitled's Light mode values from the kit export; A holds them as a separate light role set (new names).

## Open questions for the owner

1. **Dev accent colour.** Two controller defaults (2026-10-25) pull in different directions here: "canvas and Dev overlays keep Figma-parity colours (Dev ready `#14AE5C`)" and "chrome status colours move to Untitled". The Dev toggle, tab badge and Plugins row are chrome that shows a mode. Recommended: keep Figma's Dev green `#14AE5C` as the one Dev accent, so the mode colour matches the Ready colour on the canvas, with a `#0A0A0A` label (6.82:1); `#1BC47D`, `#22D489` and `#34D399` go. Alternative: Untitled success `bg-success-solid` `#16A34A` (dark label 6.01:1), which makes Dev mode look like "success" in Untitled terms.
2. **Spec reader.** Recommended: restyle it now as G2 (palette and accents only, tested by its render test) and keep it hidden until a separate decision re-enables it. Alternative: leave `specs.rs` out of the revamp and allowlist its 42 literals until the reader comes back.
