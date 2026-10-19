# #249 Add the Vue codegen target

| Field | Value |
|---|---|
| Issue | #249 (task T7.28, epic #20; milestone v0.7 Effects & Export) |
| Size | M (the issue says S; slots, props, escaping and a new validator project add the rest) |
| Depends on | issue: none. Expected on `main` before v0.7 opens (v0.6): T7.3 (one `CodeLanguage` list in new `crates/designos-export/src/language.rs`) and T7.29 (the HTML target: class-name rules, and the Node validator job). Section 12 says what to do if either is missing. |
| Unblocks | the `vue` option of `designos_generate_code` and of T7.4's design context; sibling order with #251 and #252 (section 12) |
| Touches | new `crates/designos-export/src/vue.rs`, `crates/designos-export/src/lib.rs` (`mod vue`, `pub use vue::vue_for`, beside `:22` and `:39`), `crates/designos-export/src/slots.rs` (`Lang` `:13`: add `Vue`, which uses the JS reserved-word list), `crates/designos-export/src/language.rs` (T7.3: add `Vue`; on `main` today the list is the app's `Language`, `crates/designos-app/src/dev/mod.rs:19-56`), new `crates/designos-export/tests/golden/vue/` (golden SFC, stub components, `package.json`, `tsconfig.json`), `.github/workflows/ci.yml` (the `codegen-validators` job of T7.29: one Vue step; create the job if T7.29 has not), and the MCP language lists only if T7.3 has not unified them (`crates/designos-mcp/src/engine.rs:112-126`, `:747`, `:971`) |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a developer or agent, I generate a Vue 3 single-file component for a frame: `<script setup lang="ts">`, a `<template>` that mirrors the layers, and a `<style scoped>` block with the same CSS the CSS target prints. Instances become component tags, slots become `<slot>`s, and bound variables print as their web code syntax with a resolved fallback. The output type-checks with `vue-tsc` in strict template mode.

## 2. Current state (FACT)

1. **Targets today.** Dev Mode has seven: CSS, Tailwind, SwiftUI, UIKit, Compose, Android XML, React (`crates/designos-app/src/dev/mod.rs:19-32`, order comment at `:31`). The MCP tool `designos_generate_code` has the same list again, as a `match` and an enum (`crates/designos-mcp/src/engine.rs:112-126`, `:747`) and a third time in a test (`:971`). `designos-export` has no `vue.rs` or `language.rs` (`crates/designos-export/src/lib.rs:14-30`). T7.3's accepted spec merges the three lists into `CodeLanguage::ALL` (`docs/specs/v0.6/T7.3-shared-design-context-data.md:42-46`); T7.29's accepted spec adds `Html` there and defines the class-name rules and a CI job `codegen-validators` (`docs/specs/v0.6/T7.29-html-codegen.md`, rule 3 and section 10).
2. **React is the closest sibling.** `react_with` writes a function component with one JSX element per layer (`crates/designos-export/src/react.rs:25-36`); styles come from `css_for` lines turned into a style object, the root without `position`, `left` and `top` (`react.rs:79-106`); vector layers become `<img src="/assets/<kebab>.svg" ...>` (`react.rs:158-171`); instances become `<Name prop="value" />` through `instance_attrs` (`crates/designos-export/src/tailwind.rs`); slots go through `slot_infos`, `slot_site`, `instance_slots` and `react_slot_signature` (`crates/designos-export/src/slots.rs:242`, `:287`, `:363`, `:429`).
3. **Variables are already right for web.** `css_for` prints a bound value as `var(--Color-Primary, #0D99FF)`, with the variable's WEB code syntax printed verbatim when set (`css_var_display` `crates/designos-export/src/codegen.rs:195`, `css_var_with_fallback` `:205-215`; the React golden shows `'var(--Color-Text-Muted, #666)'`, `react.rs:198-278`). That is rulings A5 and A7 for web targets (`docs/specs/v0.6/T7.4-get-design-context.md:138`). A Vue target that reuses `css_for` meets AC 2 by construction.
4. **The fixture.** `fixture::card()` (`codegen.rs:799`, `#[cfg(test)] pub(crate)` at `:724-725`) is a 312 x 200 auto-layout card with a stroke, a drop shadow, two texts (one bound to `Color/Text/Muted`), a row with a bound gap (`Spacing/Md`) and a bound fill (`Color/Primary`), an instance of component `Button` with `Label="Save"` and `Disabled`, and a red ellipse. Existing goldens are inline strings in unit tests (`react.rs:198`, `compose.rs:454`). Because the fixture is `cfg(test)`, golden tests live inside the crate, not in `tests/`.
5. **No Node tooling is in the repository.** `git ls-files` lists no `package.json`; CI has the jobs `test`, `lint` and `fmt` only (`.github/workflows/ci.yml:17`, `:54`, `:78`).
6. **vue-tsc, checked on this machine (2026-10-19).** A hand-written exemplar of the output below (an SFC with `<script setup lang="ts">`, an imported `Button.vue` with `defineProps<{ label?: string; disabled?: boolean }>()`, a named `<slot>` with default content, a scoped style using `var(--Color-Primary, #0D99FF)`) passes `vue-tsc --noEmit` 3.3.13 with `vue` 3.5.44 and `typescript` 5.9.3 (Node v22.23.1). Three findings that shape the rules:
   - with `typescript` 7.0.2 (the version `npm view typescript version` returned today) `vue-tsc` 3.3.13 stops at start with `ERR_PACKAGE_PATH_NOT_EXPORTED` for `typescript/lib/tsc` although its peer range is `>=5.0.0`; the validator must pin `typescript` 5.9.3;
   - by default a prop the child does not declare is accepted without an error; with `"vueCompilerOptions": { "strictTemplates": true }` the same call fails with `TS2353`. The validator must turn `strictTemplates` on, or it cannot catch a wrong prop;
   - the Vue template compiler treats `{{ x }}` as interpolation, a lone `}}` and single braces as text, and `{{ '{{' }}x}}` as the text `{{x}}` (`@vue/compiler-dom` 3.5.44 output).
7. **Slots and props already have shared helpers** (`slots.rs:297-345`: `prop_params`, `PropDefault`; `:276-295`: `SlotSite`). SwiftUI and Compose use `prop_params` so a component with slots declares every property an instance passes (`compose.rs:25-60`); React declares slot props only.

## 3. Behaviour rules

1. **API.** `pub fn vue_for(doc: &Document, node: NodeId) -> String` returns the text of one `.vue` file. No options. An unknown node returns an empty string (like the other targets).
2. **File layout.** Blocks in this order, one blank line between them: `<script setup lang="ts">` (omitted when it would be empty), `<template>`, `<style scoped>` (omitted when no rule is needed). Indent: two spaces; the template content is indented one level.
3. **Template.** One root element, the exported layer. Elements: frames, groups, components, instances without a component tag, sections -> `<div>` (`<section>` for a section, as React does); text -> `<p>` (a paragraph break becomes `<br />`, as React does); a vector asset -> `<img src="/assets/<kebab>.svg" alt="<layer name>" width="W" height="H" />` (the same path rule as React); ellipses -> `<div>` with `border-radius: 50%` from `css_for`. Hidden layers are omitted.
4. **Classes and styles.** One class per layer from the class-name rule of T7.29 (lower-case `[a-z0-9-]`, `n-` prefix when it starts with a digit, `-2`, `-3` suffixes for repeats), one rule per class in the `<style scoped>` block, built from the declarations of `css_for(doc, id)` in their printed order. The root's `position`, `left` and `top` are left out (its parent places it), as React does. Because CSS and Vue use the same function, they never disagree.
5. **Variables (A5, A7).** Bound properties print what `css_for` prints: `var(--<name>, <resolved fallback>)`, the WEB code syntax verbatim when set. No new variable logic. No `:root` block (the HTML target declares one; a component relies on the app's tokens, like React).
6. **Escaping.** Text: `&` `<` `>` become entities; every `{{` becomes `{{ '{{' }}` (the only sequence the Vue compiler reads as interpolation). Attribute values escape `"` and `&`. Styles: every `<` in a declaration value is written as the CSS escape `\3C ` so a value (a variable's code-syntax string is user input) can never close the `<style>` block; the existing tests of `crates/designos-export/tests/codegen_escapes_variable_code_syntax.rs` show the precedent for XML and JS. No `v-html`, no event handlers, no external URLs.
7. **Instances.** An instance of a component prints `<Name class="..." prop="value" />`; `Name` is `view_name` (`codegen.rs:103`). For each distinct instance component the script imports it: `import Name from './Name.vue';`, under the comment `// Adjust these import paths to where your components live.` Props come from `instance_args`: text and variant -> `label="Save"`; boolean true -> the bare attribute `disabled`; boolean false -> `:disabled="false"`; number -> `:count="3"`. An instance of the exported component itself imports nothing (Vue resolves a file's own name).
8. **Slots.** A component with slots prints `<slot name="icon">default content</slot>` (`<slot>` when the component has one slot, which is the default slot, as React's `children`). An instance that fills slots prints `<Name><template #icon>...</template></Name>` (the default slot as plain children). Slot names are camelCase from `slot_ident` with the JS reserved-word suffix rule (`Lang::Vue`).
9. **Component properties.** When the exported root is a component or component set, its non-slot properties (`prop_params`) are declared: `defineProps<{ label?: string; disabled?: boolean }>()` (wrapped in `withDefaults(..., { ... })` when a default exists), so an instance elsewhere type-checks against it. An instance-swap property becomes a named slot whose default content is the default component tag.
10. **Auto layout, sizes, text styles, images, rotation** follow `css_for` and the web size rules (`web_sizes`); nothing Vue-specific is added. A layer that CSS cannot express is a comment in the template, as the other targets do.
11. **Determinism.** Same document, same bytes. Class suffixes follow tree order.

## 4. Data model / schema changes

`CodeLanguage::Vue` (`id: "vue"`, `label: "Vue"`), placed after React and the HTML target in the web group of the Dev Mode dropdown. No document change.

## 5. Contracts

- `designos_generate_code` with `language: "vue"` (the enum is generated from `CodeLanguage::ALL`, T7.3); the reply shape `{ node_id, language, code }` is unchanged.
- Shape of the fixture output (the committed golden fixes the bytes; this is the structure):

  ```vue
  <script setup lang="ts">
  // Adjust these import paths to where your components live.
  import Button from './Button.vue';
  </script>

  <template>
    <div class="card">
      <p class="..."> ... </p>
      <div class="...">
        <Button class="button" label="Save" disabled />
        <div class="ellipse"></div>
      </div>
    </div>
  </template>

  <style scoped>
  .card { display: flex; width: 312px; ... background: #FFF; }
  .button { width: 80px; height: 32px; ... }
  </style>
  ```
- Validator project (committed under `crates/designos-export/tests/golden/vue/`): `package.json` pinning `vue` 3.5.44, `vue-tsc` 3.3.13 and `typescript` 5.9.3 (re-checked at implementation, never `latest`); `tsconfig.json` with `strict: true`, `noEmit: true`, `moduleResolution: "Bundler"`, `lib: ["ES2020","DOM"]` and `vueCompilerOptions.strictTemplates: true`; a stub `Button.vue` declaring `label` and `disabled`; the golden `Card.vue`.

## 6. File-format mapping

None. The target reads the document only.

## 7. UI

Dev Mode's language dropdown gains "Vue" (from `CodeLanguage::ALL`); the code panel shows the output with the existing code view and copy button. No other UI change. Evidence: a desktop capture at 1440 of Dev Mode with Vue selected on the fixture card, and the `vue-tsc` terminal capture. Responsive: SKIPPED (owner default). Load `es-designer` before touching the dropdown code. Update Figma frames from the capture first (owner rule).

## 8. Edge cases

1. A layer name that yields an empty class (emoji only): `n-<id>`.
2. Text containing `{{`, `}}`, `<script>` or quotes: escaped per rule 6; a test renders the compiled template and reads the text back. A WEB code syntax of `</style><script>...`: printed as inert CSS text, never closing the block.
3. Two layers with the same name: `-2` suffix on the second.
4. An instance whose component has no name characters usable in a tag: `Layer`-style fallback of `pascal` (`codegen.rs:70-77`).
5. A component with two slots and an instance filling one: the other slot keeps its default content (the instance passes nothing for it).
6. A property named like a JS reserved word (`default`, `class`): suffixed by `slot_ident` for slots; for `defineProps` the name is quoted if it is not a valid identifier.
7. Variable with no code syntax and an invalid CSS name: sanitised by `css_var_name` (`codegen.rs:130`), unchanged.
8. Very large subtree (5,000 nodes): generated; T7.4 applies its own size fallback.
9. Vector layers reference `/assets/<kebab>.svg`; the files themselves come from `vector_assets` (`codegen.rs:268`) and are the caller's to write, as for React.
10. A frame with rotation or a non-uniform scale: `css_for`'s transform lines; no Vue-specific handling.

## 9. Non-goals

- Options API, Vue 2, JSX, Pinia, router, `v-model`, emits.
- Tailwind classes in Vue (the Tailwind target is separate).
- Writing the component files an instance imports, or the SVG assets.
- Interactions and prototype links; Code Connect mapping.
- Pixel parity with the Studio render beyond what CSS gives; the contract is valid, idiomatic output with the same declarations as the CSS target.

## 10. Test plan (red first)

Seams: unit golden tests inside `vue.rs` (the fixture is `cfg(test)`), plus the `vue-tsc` job over the committed golden directory.

| Case | Expected | Fails before |
|---|---|---|
| `card_golden` | `vue_for(fixture::card())` equals the committed `tests/golden/vue/Card.vue` (read with `include_str!`) | yes (function absent) |
| `card_passes_vue_tsc_strict` | CI job: `npm ci` then `npx vue-tsc --noEmit` in the golden directory exits 0 with `strictTemplates` on | yes |
| `validator_rejects_a_wrong_prop` | the same project with `:count="3"` added to the `Button` tag exits non-zero (proves `strictTemplates` is on); a script step in the job | yes |
| `bound_variables_print_code_syntax` | a fill bound to a variable whose WEB code syntax is `var(--brand-primary)` prints `var(--brand-primary, #0D99FF)`-style per `css_for`; without code syntax: `var(--Color-Primary, #0D99FF)` | guard (shared code) |
| `auto_layout_to_flex_rules` | row with gap 12 and padding 16: rule has `display: flex`, `flex-direction: row`, `gap: 12px`, `padding: 16px` | yes |
| `text_escaping` | text `{{x}} <b>&` renders back as the same text through `@vue/compiler-dom` (the CI job script) | yes |
| `slots_become_slot_tags` | component with slot `icon` and default content: `<slot name="icon">...</slot>`; its instance with a fill: `<template #icon>` | yes |
| `component_props_are_declared` | root component with `Label` and `Disabled`: `defineProps<{ label?: string; disabled?: boolean }>()` with defaults | yes |
| `code_syntax_cannot_close_the_style_block` | WEB code syntax with `</style><script>`: the output holds no raw `</style>` before the block's own end, and `@vue/compiler-sfc` parses exactly one `<style>` block | yes |
| `class_names_are_unique_and_valid` | duplicate and digit-leading names | yes |
| `output_is_deterministic` | two runs equal | yes |
| `language_listed_once` | `CodeLanguage::ALL` contains `Vue`; the MCP enum contains `vue` with no hand-edited list; `Language` in the app has a label for it; the Dev Mode dropdown test of `shell/tests/dev.rs:81-97` still fits the menu | yes |

Run: `cargo test -p designos-export vue`, `cargo test -p designos-mcp`, `cargo test -p designos-app dev`, and the CI job (locally: `cd crates/designos-export/tests/golden/vue && npm ci && npx vue-tsc --noEmit`).

## 11. Performance budgets

Same order as `css_for`: at most 20 ms for a 2,000-node frame (T7.29's budget), measured with `test_timing::fastest_within` on a synthetic frame; the output is built in one pass over the subtree with one `css_for` call per layer. The CI Vue step finishes in under 2 minutes including `npm ci` (UNKNOWN until the first run; record it).

## 12. Dependencies and build order

- **T7.3 and T7.29 missing.** If T7.3 has not landed, this task adds `Vue` to the three hand-kept lists (`dev/mod.rs`, `engine.rs:112-126`, `:747`, `:971`); the next task to land unifies them. If T7.29 has not landed, this task creates the `codegen-validators` job (Node 22 via `actions/setup-node`, pinned lockfile) and writes the class-name function in a shared module `crates/designos-export/src/class_names.rs` for the HTML target to reuse.
- **Siblings.** #251 (Web Components) and #252 (Flutter) add their own variant to the same list and their own step in the same job. Land them one after another: #249, #251, #252. Each rebases on the previous; `lib.rs`, `language.rs`, `ci.yml` and the MCP enum are the shared lines.
- `crates/designos-mcp/src/engine.rs` is a hot spot of the pack: no change when T7.3 has landed.
- The `node_modules` directory is never committed; `package-lock.json` is.

## 13. Owner rulings

- A5 and A7: bound variables print code syntax (WEB for web targets) with resolved fallbacks. (Issue text; `docs/specs/v0.6/T7.4-get-design-context.md:138`.)
- Full scope, never MVP: complete component output (layout, text, images, vectors, variables, slots, props) in one task.
- Acceptance: the fixture card output validates with `vue-tsc`. (Issue.)

## Open questions for the owner (recommended first)

1. **Where styles go.**
   - A (recommended): a `<style scoped>` block with one class per layer, the idiomatic Vue 3 way and the same declarations as the CSS target.
   - B: inline `:style` objects per element, like the React target's default. Smaller output, but harder to read and override.
2. **Import paths of instances.** Recommended: `./<Name>.vue` next to the file, with a one-line comment telling the reader to adjust. The generator cannot know the project layout. Alternative: no imports and rely on global registration, which fails strict type checking.
3. **Pinned validator versions.** Recommended: pin `vue-tsc` 3.3.13 with `typescript` 5.9.3 until `vue-tsc` supports TypeScript 7 (checked 2026-10-19: it does not start with 7.0.2). Raising the pin later is a deliberate PR.
