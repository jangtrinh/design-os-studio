# E15-42 Import globals.css and Tailwind themes as variable collections with modes

- Issue: #632 (task E15-42, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-export/src/tokens/{mod.rs,css_scan.rs,css_values.rs,css_import.rs,css_export.rs,tailwind_config.rs,tests.rs}` (new), `crates/designos-doc/src/variables.rs` (`Variable.web_notation`), `crates/designos-app/src/cli.rs` (`designos tokens` subcommand), `crates/designos-mcp/src/engine.rs` tool list (`designos_import_tokens`, `designos_export_tokens`).

## 1. Summary and user story

As a designer joining a coded product, I want to import the project's `globals.css` (`:root`, `.dark`, `[data-theme]`, `prefers-color-scheme`, Tailwind v4 `@theme`) or a static Tailwind v3 theme, so design starts from the real tokens with their modes and aliases; and exporting back must reproduce the same custom property names and values.

## 2. Current state

- FACT: variables support collections with modes and `Alias` values (`crates/designos-doc/src/variables.rs:20-31,52-59`), plus per-platform `code_syntax` printed verbatim by CSS codegen (`variables.rs:94-110`; `crates/designos-export/src/codegen.rs:184-189`).
- FACT: CSS codegen maps a variable name to a custom property by `css_var_name` (`Color/Primary` → `--Color-Primary`, `codegen.rs:119-160`). There is no CSS token export (`:root { … }`) and no CSS token import today.
- FACT: Figma variables JSON import/export exists in `designos-fig` (`crates/designos-fig/src/variables.rs:92,168,200,241`); #138 adds DTCG.
- FACT: the `designos` CLI is flag-based (`crates/designos-app/src/cli.rs:186-266`); there are no subcommands.
- FACT: `svgtypes` 0.15 (CSS colour syntax) and `simplecss` 0.2 are already in `Cargo.lock`; no full CSS parser crate is.

## 3. Behaviour rules

1. **Scanner.** `css_scan` walks the stylesheet text with a comment-, string- and brace-aware tokenizer and yields rule blocks with their selector or at-rule prelude and their custom property declarations (`--name: value;`). Nested at-rules (`@media`, `@supports`, `@layer`, `@theme`) are followed. Everything that is not a custom property declaration is ignored without warning (ordinary CSS is not tokens).
2. **Mode mapping.**

| Source | Mode |
|---|---|
| `:root`, `html`, `:host` | the default mode, named `Light` when a dark source exists in the file, else `Default` |
| `.dark`, `[data-theme="dark"]`, `[data-mode="dark"]`, `@media (prefers-color-scheme: dark) { :root { … } }` | `Dark` |
| `[data-theme="<x>"]`, `.theme-<x>` | `<X>` (title case) |
| Tailwind v4 `@theme { … }` and `@theme inline { … }` | collection `Theme`, single mode `Default` |
| any other selector | not imported; listed in warnings as `selector_ignored` with the selector |

   One collection per source file for selector-based tokens, named after the file stem (`globals`). A property defined in some modes only gets the default mode's value in the others, with warning `value_inherited`.
3. **Names.** Each property becomes a variable whose name groups by kind for the Variables panel and whose WEB code syntax keeps the exact property:
   - Tailwind v4 namespaces map to groups: `--color-*` → `Color/<rest>`, `--font-*` → `Font/<rest>`, `--text-*` → `Text/<rest>`, `--font-weight-*` → `Font weight/<rest>`, `--leading-*` → `Leading/<rest>`, `--tracking-*` → `Tracking/<rest>`, `--radius-*` → `Radius/<rest>`, `--shadow-*` → `Shadow/<rest>`, `--spacing-*` and `--spacing` → `Spacing/<rest or "base">`, `--breakpoint-*` → `Breakpoint/<rest>`;
   - other properties group by resolved kind: colours → `Color/<property without -->`, lengths and numbers → `Number/<…>`, other text → `String/<…>`. Example: `--primary` → `Color/primary`;
   - every imported variable gets `code_syntax[WEB] = "var(--<exact property>)"`, so codegen (§2) prints the original name.
4. **Values.**
   - Colours: hex (3, 4, 6, 8 digits), `rgb()`/`rgba()`, `hsl()`/`hsla()`, named colours, `transparent` (through `svgtypes`); `oklch()`, `oklab()`, `lab()`, `lch()`, `color(srgb …)`, `color(display-p3 …)` (our converter, CSS Color 4 formulas). Values convert into the document's colour profile (SC-6); out-of-gamut values are mapped by OKLCH chroma reduction (CSS Color 4 gamut mapping) and listed as `gamut_mapped`.
   - Lengths: `px` → Number; `rem` and `em` → Number × root size (16 px, CLI `--rem-px`); unitless numbers → Number.
   - Font stacks and other text → String (quotes kept as written).
   - `var(--x)` alone → `VariableValue::Alias(id of --x)` (same or another imported collection, or an existing document variable whose WEB code syntax is `var(--x)`); `var(--x, fallback)` → alias, fallback recorded in warning `alias_fallback_dropped`.
   - Anything else (`calc()`, `color-mix()`, multiple values like shadows, `%`) → a String variable holding the raw text, warning `unsupported_value` naming the property and value. Nothing is dropped silently (AC3).
5. **Notation memory.** `Variable.web_notation: Option<String>` records how the value was written (`hex`, `rgb`, `hsl`, `oklch`, `oklab`, `lab`, `lch`, `p3`, `px`, `rem`, `em`, `raw`). Export writes values back in that notation, so an untouched import exports the same text up to number formatting (shortest round-trip, trailing zeros trimmed).
6. **Export.** `designos_export_tokens({ format: "css" | "tailwind-v4", collection_ids? })` writes, per collection: the default mode under `:root`, the `Dark` mode under `.dark`, other modes under `[data-theme="<mode lower-case>"]`. The single-mode collection named `Theme` (what an `@theme` import creates, rule 2) is written back as a Tailwind v4 theme block in **both** formats, so a `css` export of the shadcn fixture keeps its `--color-*` properties and the round trip is lossless: its variables whose value contains `var(` (aliases and raw `calc(var(--x) …)` strings) go under `@theme inline`, the others under `@theme`, each block in the collection's variable order, and an empty block is not written. Format `tailwind-v4` additionally writes every other single-mode collection under `@theme` / `@theme inline` by the same `var(` rule instead of `:root`; multi-mode collections are written as in `css`. (Emitting the theme block only for `tailwind-v4` would make the `css` export drop every `Theme` property, so AC2's property set would not survive.) Property names come from the WEB code syntax when it is `var(--x)`, else from `css_var_name`. Aliases export as `var(--target)`. AC2: importing then exporting reproduces the same property names and values (values compared after normalising number formatting and colour notation as in rule 5).
7. **Tailwind v3 config (no code execution).** `tailwind_config` parses `tailwind.config.{js,cjs,mjs,ts}` or `.json` as a restricted object literal: `module.exports = {…}`, `export default {…}`, optionally `satisfies Config` or wrapped in `defineConfig({…})`. Accepted values: object literals, string, number and boolean literals, arrays of literals. Identifiers, function calls, spreads, template literals with expressions, `require`/`import` → warning `config_value_unsupported` with the key path; the key is skipped. Only `theme` and `theme.extend` are read; nested colour objects flatten with `-` (`colors.primary.DEFAULT` → `--color-primary`, `colors.primary.foreground` → `--color-primary-foreground`) and are named as in rule 3 with the corresponding v4 property names in code syntax. `extend` values override base values of the same key.
8. **Merge into existing collections.** Re-importing the same file updates variables matched by WEB code syntax in the collection named after the file; new properties are added; properties no longer present are kept and listed as `stale` in the report (never deleted).
9. **One undo step.** An import is one `Command::Batch`. `dry_run: true` returns the report only.

## 4. Data model / schema

```rust
// variables.rs on Variable (back-compat: default None, skipped when None)
#[serde(default, skip_serializing_if = "Option::is_none")] pub web_notation: Option<String>,

pub struct TokenImportReport { pub collections: Vec<String>, pub modes: BTreeMap<String, Vec<String>>,
    pub created: u32, pub updated: u32, pub aliases: u32, pub stale: Vec<String>, pub warnings: Vec<TokenWarning> }
pub struct TokenWarning { pub name: String /* selector_ignored | value_inherited | unsupported_value | … */,
    pub property: Option<String>, pub detail: String, pub line: u32 }
pub fn import_css(doc: &Document, css: &str, opts: &CssImportOptions) -> Result<(Vec<Command>, TokenImportReport), TokenError>;
pub fn export_css(doc: &Document, format: TokenFormat, collections: Option<&[String]>) -> String;
pub fn import_tailwind_config(doc: &Document, src: &str, opts: &CssImportOptions) -> Result<(Vec<Command>, TokenImportReport), TokenError>;
```

## 5. Contracts

- CLI: `designos tokens import <file> --into <doc.designos> [--rem-px 16] [--dry-run] [--json]` and `designos tokens export <doc.designos> --format css|tailwind-v4 [--out file]`. `tokens` as the first argument selects the subcommand; all existing flags keep working. Exit 0 ok, 1 imported with warnings (`--strict` makes warnings fail), 2 parse or usage error.
- MCP `designos_import_tokens({ path | text, kind: "css" | "tailwind-config", dry_run? })` → `{ "changed": true, "report": { … } }`; `designos_export_tokens({ format, collection_ids? })` → `{ "format": "css", "text": ":root {\n  --background: oklch(1 0 0);\n…" }`.
- Errors: `tokens_parse_failed` (details: line, column, snippet; `safe_to_retry: false`), `tokens_path_invalid` (outside the workspace, `server.rs:319-328` rules), `tokens_empty` (no custom properties found; corrected example: point at the file that holds `:root`).

## 6. File-format mapping

| CSS | Document |
|---|---|
| `:root { --primary: oklch(0.21 0.006 285.9); }` | collection `globals`, mode `Light`, variable `Color/primary`, value converted colour, `code_syntax[WEB] = var(--primary)`, `web_notation = oklch` |
| `.dark { --primary: oklch(0.92 0.004 286.3); }` | same variable, mode `Dark` |
| `--ring: var(--primary);` | `Color/ring` = `Alias(Color/primary)` |
| `@theme inline { --color-primary: var(--primary); }` | collection `Theme`, `Color/primary` (code syntax `var(--color-primary)`) aliasing `globals` `Color/primary`; exports back under `@theme inline` (rule 6) |
| `--radius: 0.625rem;` | `Number/radius` = 10, `web_notation = rem` |

## 7. UI

Studio: `File → Import → Tokens from CSS or Tailwind config…` opens a file picker, then a report sheet (collections, modes, counts, warnings list with line numbers) with `Import` and `Cancel`; `Import` applies as one undo step. Evidence: 1440 capture of the report sheet and the Variables panel after import.

## 8. Edge cases

1. The same property twice in one block: the last wins (CSS cascade), warning `duplicate_property`.
2. `!important` in a value: stripped, warning `important_ignored`.
3. Circular aliases (`--a: var(--b); --b: var(--a)`): both imported as raw String values, warning `alias_cycle`.
4. Alias to a property that is never defined: raw String value, warning `alias_unresolved`.
5. Huge files (≥ 5 MB): scanned in one pass; a 10 MB limit returns `tokens_too_large`.
6. CRLF line endings and a BOM: handled; line numbers count either.
7. A `.dark` block nested inside `@media print`: not a mode source (only the listed patterns); warning `selector_ignored`.
8. A plain `@theme { … }` block that holds a `var(` value: it imports as in rule 2, but the export writes it under `@theme inline` (rule 6 derives the form from the value, the collection stores no source form); the import adds the warning `theme_inline_normalised` naming the block's first line, so the change is never silent. A `@theme inline` block with only literal values exports under `@theme` and warns the same way.
9. A user-made collection named `Theme` with one mode is exported as a theme block too (the name is the marker).

## 9. Non-goals

- Running JavaScript or TypeScript configs.
- DTCG and Figma JSON (#138).
- Binding existing nodes to the imported tokens (E15-11 lint and E15-13 auto-fix; E15-43 for captures).

## 10. Test plan

Seam: fixture round-trip tests in `cargo test -p designos-export tokens`, CLI test in `designos-app`, MCP test.

Fail-before: no import exists.

Fixtures (ours, written for these tests, under `crates/designos-export/tests/fixtures/tokens/`): `shadcn-style-globals.css` (`:root` and `.dark` with ~30 oklch colours, `--radius`, an `@theme inline` block aliasing them), `prefers-dark.css`, `data-theme.css`, `tailwind.config.ts` (static), `tailwind-dynamic.config.js` (with a function value), `unsupported.css`.

| Test | Input | Expected |
|---|---|---|
| `shadcn_style_imports_with_modes_and_aliases` (AC1) | shadcn-style-globals.css | collection `globals` with modes Light, Dark; `Theme` collection; every `@theme` variable is an alias into `globals` |
| `export_round_trips_names_and_values` (AC2) | import then export css (`--format css`) | same property set, including every `--color-*` property of the `@theme inline` block; values equal after normalisation; selectors `:root`, `.dark`, `@theme inline` present |
| `css_export_writes_theme_inline_for_alias_values` | shadcn fixture, `format: "css"` | the output has exactly one `@theme inline` block holding the alias and `calc(var(...))` properties; no `Theme` property appears under `:root`; no empty `@theme` block |
| `tailwind_v4_export_writes_single_mode_collections_as_theme` | collection `Spacing` (single mode, literal values) plus the shadcn fixture, `format: "tailwind-v4"` | `Spacing` under `@theme`; `Theme` aliases under `@theme inline`; `globals` under `:root` and `.dark` |
| `plain_theme_with_var_value_warns_when_normalised` | `@theme { --color-x: var(--y); }` | import warning `theme_inline_normalised`; export puts `--color-x` under `@theme inline` |
| `unsupported_listed_not_dropped` (AC3) | `--shadow-md: 0 1px 2px rgb(0 0 0 / .1), 0 4px 8px …; --x: calc(1rem + 2px);` | String variables with raw text; two `unsupported_value` warnings |
| `prefers_color_scheme_is_dark_mode` | prefers-dark.css | mode `Dark` holds the values |
| `tailwind_v3_static_config` | tailwind.config.ts | `Color/primary`, `Color/primary-foreground` with code syntax `var(--color-primary)` … |
| `tailwind_function_value_warned` | tailwind-dynamic.config.js | key skipped; `config_value_unsupported` with its path |
| `reimport_updates_and_reports_stale` | import, remove one property, import again | updated values; removed property kept and listed `stale` |
| `one_undo_step` | import then undo | document hash restored |

Visual evidence: terminal capture of the round trip diff (empty) and the Variables panel capture.

## 11. Performance budgets

- 1 MB stylesheet ≤ 150 ms import (release).
- Import runs off the UI thread in Studio; the result applies as one batch (one revision bump, CLAUDE.md §4.1).

## 12. Dependencies and build order

- Needs #138 (E1-9) for the shared variables JSON and DTCG work in the same area.
- Unblocks E15-43.

## 13. Owner rulings that apply

- Variables JSON is two separate formats (owner ruling 2026-10-19, #138): `figma-native` exactly as Figma writes it, and strict DTCG with `org.designos` metadata. This task adds CSS as a third, code-side format and reads tokens through those two importers without mixing them.
- Colour is Display P3 per document: imported colours convert into the document profile.
- Nothing dropped silently.
