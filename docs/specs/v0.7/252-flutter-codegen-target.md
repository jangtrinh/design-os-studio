# #252 Add the Flutter codegen target

| Field | Value |
|---|---|
| Issue | #252 (task T7.31, epic #20; milestone v0.7 Effects & Export) |
| Size | L (the issue says M; the widest mapping of the three targets, a Dart escaping layer, `const` rules and a Flutter SDK validator add the rest) |
| Depends on | issue: none. Expected on `main` before v0.7 opens (v0.6): T7.3 (one `CodeLanguage` list). Same fallback as #249, section 12. Land after #251. |
| Unblocks | the `flutter` option of `designos_generate_code` and of T7.4's design context |
| Touches | new `crates/designos-export/src/flutter.rs`, `crates/designos-export/src/codegen.rs` (new `dart_quote` beside `quote` `:450` and `swift_quote` `:468`), `crates/designos-export/src/lib.rs` (`mod flutter`, `pub use`, beside `:22` and `:35`), `crates/designos-export/src/language.rs` (T7.3: add `Flutter`; today the app's `Language`, `crates/designos-app/src/dev/mod.rs:19-56`), `crates/designos-export/src/slots.rs` (`Lang` `:13`: add `Dart` with the Dart reserved words), new `crates/designos-export/tests/golden/flutter/` (golden widget, stubs, `pubspec.yaml`, `analysis_options.yaml`), `.github/workflows/ci.yml` (one step or job with a Flutter SDK), the MCP language lists only if T7.3 has not unified them (`crates/designos-mcp/src/engine.rs:112-126`, `:747`, `:971`) |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a developer or agent, I generate a Flutter widget for a frame: a Dart file with one `StatelessWidget` whose `build` returns the layers as `Container`, `Column`, `Row`, `Stack`, `Text` and friends, bound variables as token constants, instances as widget calls and slots as `Widget?` parameters. The file passes `flutter analyze` with the standard Flutter lints.

## 2. Current state (FACT)

1. **No such target.** Dev Mode has seven languages and the MCP tool has three hand-kept lists (`crates/designos-app/src/dev/mod.rs:19-32`; `crates/designos-mcp/src/engine.rs:112-126`, `:747`, `:971`); `designos-export` has no `flutter.rs` (`crates/designos-export/src/lib.rs:14-30`). T7.3's accepted spec provides the single list (`docs/specs/v0.6/T7.3-shared-design-context-data.md:42-46`).
2. **The native targets print fragments, not compilable files.** `compose_for` returns one composable body, with `Tokens.spacingMd` and `MaterialTheme.colorScheme.primary` as placeholders and no imports (`crates/designos-export/src/compose.rs:13-23`, golden `:454-505`); SwiftUI is the same. Nothing compiles them in CI. The Flutter target must compile, so it needs imports and a validator (rules 1, 8, section 10).
3. **Variables on native targets.** Each native target prints a variable's code syntax for its platform verbatim, else its own token reference: `token_text(v, platform, fallback)` with `Ios` for SwiftUI and UIKit and `Android` for Compose and XML (`crates/designos-export/src/codegen.rs:217`; `crates/designos-export/src/compose.rs:64`, `:71`; `crates/designos-export/src/swiftui.rs:87`, `:145`; `crates/designos-export/tests/variable_code_syntax_per_platform.rs`). The platforms are an enum of three: `Web`, `Android`, `Ios` (`crates/designos-doc/src/variables.rs:64-71`), the same three Figma stores. **There is no Flutter or Dart platform.** The accepted ruling A7 says "WEB for web, iOS and ANDROID verbatim for native targets" (`docs/specs/v0.6/T7.4-get-design-context.md:138`). The issue's acceptance criterion 2 says Flutter "prints code_syntax (A5, A7)". Neither rule names which of the three a Dart target should read. Open question 1.
4. **Token names.** `token_camel(v, drop_color)` returns the camelCase token name (`codegen.rs:222-229`): `Spacing/Md` gives `spacingMd`; `Color/Primary` gives `colorPrimary` (or `primary` with `drop_color`).
5. **Shared helpers** for the tree walk: `visible_children`, `is_vector_asset`, `vector_assets`, `instance_args`, `view_name`, `snake`, `pascal`, `camel`, `sizes`, `web_sizes`, `offset`, `rotation_deg`, `drop_shadows`, `grid_*`, and the slot and prop helpers `slot_infos`, `slot_site`, `instance_slots`, `prop_params` (`codegen.rs:234`, `:244`, `:268`, `:296`, `:103`, `:97`, `:70`, `:79`, `:356`, `:366`, `:426`, `:433`, `:486`, `:500-600`; `slots.rs:242`, `:287`, `:363`, `:315`). `Lang` in `slots.rs:13` has `React`, `Swift`, `Kotlin` only; Dart has its own reserved words.
6. **Compose and SwiftUI treat grid and wrap loosely.** A grid frame is emitted as a column; a wrapping auto layout is a comment (`compose.rs:383-410`; `swiftui.rs:410-411`; `android_xml.rs:352-353`). A grid child exports its solved size and never a fill (`compose.rs:518-530`). Flutter's `Wrap` does express wrapping.
7. **String escaping helpers exist for other languages.** `quote` is double-quoted and escapes `"`, `\`, newline and `$` (`codegen.rs:450-463`, for Swift and Kotlin); `swift_quote` also escapes control characters (`:468-485`). Dart needs its own: single quotes are the Flutter convention and `$` starts interpolation.
8. **Flutter checked on this machine (2026-10-19).** A hand-written exemplar of the output below (a `StatelessWidget` with `Container`, `Column`, `Row`, `DecoratedBox`, `Padding`, `SizedBox`, `Text`, an imported `Button` widget and a token class, a `Widget?` slot parameter) passes `flutter analyze` ("No issues found") on Flutter 3.38.7 with Dart 3.10.7 and `flutter_lints` ^6.0.0, using `import 'package:flutter/widgets.dart'` only. The token stub used `static final` members (not `const`); with them, a `const` ancestor of a token reference fails with `invalid_constant`, which is why rule 8 bans it. A class named `Card` passes (`widgets.dart` does not export Material's `Card`). An undefined token getter fails with `undefined_getter`, so the validator can fail. `Row` and `Column` take `spacing:` since Flutter 3.27.0 (Flutter 3.27.0 release notes: "Introduce `double` `Flex.spacing` parameter for `Row`/`Column` spacing").
9. **No Flutter in CI or in the repository** (`ci.yml` has jobs `test`, `lint`, `fmt`; no `pubspec.yaml` is tracked). Which action installs the SDK on `macos-14` is UNKNOWN; check: the first CI run (candidate: `subosito/flutter-action`, pinned to a stable Flutter version).
10. **Code-syntax and name strings are user input** (`crates/designos-export/tests/codegen_escapes_variable_code_syntax.rs`). Layer names reach Dart as string literals, comments and identifiers.

## 3. Behaviour rules

1. **API.** `pub fn flutter_for(doc: &Document, node: NodeId) -> String` returns the text of one Dart library file. No options. Unknown node: empty string.
2. **File shape.** In order: `import 'package:flutter/widgets.dart';`; a blank line; one `import` per distinct instance component (`import 'button.dart';`, file name `snake(view_name)`) and one `import 'tokens.dart';` when any token is used; a blank line; the widget class. Only `widgets.dart` is imported: no Material dependency. A first-line comment states the floor: `// Requires Flutter 3.27 or newer (Row and Column spacing).` A comment above the project imports says to adjust the paths.
3. **Class.** `class <Name> extends StatelessWidget` with `const <Name>({super.key, ...})`, one `final` field per slot (`Widget?`) and per non-slot component property (`bool`, `String`, or `Widget?` for an instance swap, with the declared default, as `compose.rs` and `swiftui.rs` do through `prop_params`), and `@override Widget build(BuildContext context)` returning the root widget. `<Name>` is `view_name`; if it equals a name the generator itself emits or a common `widgets.dart` class (`Text`, `Row`, `Column`, `Container`, `Stack`, `Padding`, `SizedBox`, `Image`, `Icon`, `Opacity`, `Transform`, `Align`, `Center`, `Expanded`, `Flexible`, `Wrap`, `Positioned`, `Builder`, `Table`, `Form`, `Placeholder`), it gets the suffix `Widget` (the fixture's `Card` keeps its name because `widgets.dart` does not export it; the validator catches any clash the list misses).
4. **Layout mapping.**
   - Auto layout row or column -> `Row` or `Column` with `mainAxisAlignment`, `crossAxisAlignment`, `spacing:` and, for padding, a `Padding` or the `padding:` of a `Container`. Wrapping auto layout -> `Wrap` with `spacing`, `runSpacing`, `alignment` and `runAlignment`.
   - Grid -> a `Wrap` of the children at their solved sizes (`spacing` = column gap, `runSpacing` = row gap) with a comment `// grid: approximated with Wrap`; non-default tracks add `// custom tracks not expressed`. Flutter's SDK has no grid widget (non-goal).
   - No auto layout -> `SizedBox(width, height, child: Stack(clipBehavior: Clip.none, children: [Positioned(left, top, child: ...)]))`.
   - Sizing: fixed -> `width` and `height`; hug -> no size; fill on the main axis -> `Expanded`; fill on the cross axis -> `SizedBox(width: double.infinity)` (or `height`).
   - A single `Container` carries size, padding, decoration and clip for a layer, as one widget, so that the tree has no `Container` whose only argument is `child` (the `avoid_unnecessary_containers` lint).
5. **Paint and shape.** Solid fill -> `color`; linear and radial gradients -> `LinearGradient` and `RadialGradient` in `BoxDecoration.gradient`; image fill -> a comment and a neutral placeholder (as Compose does, `compose.rs:90`); strokes -> `Border.all(color:, width:)`; corner radius -> `BorderRadius.circular` or `BorderRadius.only`; a square ellipse -> `BoxShape.circle`, a non-square one -> `Radius.elliptical`; `clips_content` -> `clipBehavior: Clip.antiAlias`; drop shadows -> `BoxShadow(color, offset, blurRadius, spreadRadius)`; opacity -> `Opacity`; rotation -> `Transform.rotate(angle:)` in radians. Inner shadows, blurs and blend modes are not expressed: a comment names the effect (parity with Compose, which prints no blur).
6. **Text.** `Text('content', style: TextStyle(color, fontFamily, fontSize, fontWeight, height, letterSpacing), textAlign: ..., maxLines: ..., overflow: ...)`; `height` is line height divided by font size; weights map to `FontWeight.w100`..`w900`; truncation -> `TextOverflow.ellipsis` with `maxLines`.
7. **Vectors.** A vector asset is a `SizedBox(width, height)` with the comment `// vector: export assets/<kebab>.svg and draw it with your SVG package`. The SDK cannot draw SVG; no package is assumed.
8. **Variables (A5; A7 pending open question 1).** A bound property prints `Tokens.<token_camel>` (colour -> `Tokens.colorPrimary`, number -> `Tokens.spacingMd`), and the file imports `tokens.dart`. A subtree that uses a token is not `const`; a subtree of literals is `const` everywhere it can be (the `prefer_const_constructors` lint). The token class is the caller's: `abstract final class Tokens` with `static final` members; the validator ships a stub.
9. **Instances.** `Button(label: 'Save', disabled: true)` inside the layer's `SizedBox` when it has a size; named arguments from `instance_args`; slot fills as named `Widget` arguments. An instance of the exported class itself imports nothing.
10. **Slots.** The class field `final Widget? icon;`; where the slot sits, `icon ?? <default content>` (`icon ?? const SizedBox.shrink()` when there is no default). Slot and property names are camelCase with the Dart reserved-word suffix `Slot` (`Lang::Dart`).
11. **Escaping.** Strings are single-quoted by `dart_quote`: it escapes `\`, `'`, `$`, newline, carriage return, tab and every other control character (as `\u{..}`). Layer names and code-syntax strings never reach code unescaped: comments are one line (newlines replaced by spaces), identifiers come from `camel` and `pascal` and are prefixed with `v` when they would start with a digit, string literals go through `dart_quote`.
12. **Determinism and format.** Same document, same bytes; two-space indent; a trailing comma after every argument and list element (the Dart formatter's style). Byte equality with `dart format` is not promised (non-goal).

## 4. Data model / schema changes

`CodeLanguage::Flutter` (`id: "flutter"`, `label: "Flutter"`), placed last in the dropdown, after the web group. No document change.

## 5. Contracts

- `designos_generate_code` with `language: "flutter"` (enum generated from `CodeLanguage::ALL`); reply shape unchanged; the text is a `.dart` file.
- Shape of the fixture output (the golden fixes the bytes):

  ```dart
  // Requires Flutter 3.27 or newer (Row and Column spacing).
  import 'package:flutter/widgets.dart';

  // Adjust these import paths to where your widgets live.
  import 'button.dart';
  import 'tokens.dart';

  class Card extends StatelessWidget {
    const Card({super.key});

    @override
    Widget build(BuildContext context) {
      return Container(
        width: 312,
        height: 200,
        padding: const EdgeInsets.all(16),
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
          color: const Color(0xFFFFFFFF),
          border: Border.all(color: const Color(0xFFE5E5E5)),
          borderRadius: BorderRadius.circular(8),
          boxShadow: const [BoxShadow(color: Color(0x40000000), offset: Offset(0, 4), blurRadius: 8)],
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          spacing: 8,
          children: [ /* texts, a DecoratedBox row with Tokens.colorPrimary and Tokens.spacingMd,
                         Button(label: 'Save', disabled: true), a circle */ ],
        ),
      );
    }
  }
  ```

- Validator project under `crates/designos-export/tests/golden/flutter/`: `pubspec.yaml` (`flutter` SDK dependency, dev dependency `flutter_lints` ^6.0.0, `environment.sdk` matching the pinned Flutter), `analysis_options.yaml` (`include: package:flutter_lints/flutter.yaml`), `lib/card.dart` (the golden), `lib/button.dart` (stub with `label` and `disabled`), `lib/tokens.dart` (stub with `static final` members for `colorPrimary`, `colorTextMuted`, `spacingMd`).

## 6. File-format mapping

None. Dart has no Figma code-syntax platform (fact 3), so there is no mapping to store.

## 7. UI

Dev Mode's language dropdown gains "Flutter" (from `CodeLanguage::ALL`), last in the list; the menu has 12 or 11 rows depending on whether the HTML target is present, still short of the screen (the dropdown test, `crates/designos-app/src/shell/tests/dev.rs:81-97`, sizes the menu from `ALL.len()`). The code panel shows the file with the existing code view and copy button. Evidence: a desktop capture at 1440 of Dev Mode with Flutter selected on the fixture card, and the `flutter analyze` terminal capture. Responsive: SKIPPED (owner default). Load `es-designer` before touching the dropdown code. Update Figma frames from the capture first (owner rule).

## 8. Edge cases

1. A layer name that is `Text`, `Row` or another emitted widget: the class gets the `Widget` suffix (rule 3).
2. A name that starts with a digit or has only symbols: `pascal`'s fallbacks (`V123`, `Layer`, `codegen.rs:70-77`).
3. Text containing `'`, `$name`, `${x}`, backslashes, newlines or emoji: each stays inert through `dart_quote`; a test compiles it.
4. A layer name with a newline used in a comment: one line.
5. Two sibling layers with the same name: nothing depends on layer names inside a class body, so there is no clash; instance classes are by component.
6. Rotation and opacity together: `Opacity` outside `Transform.rotate`, each only when non-default.
7. A fill that is hug in one axis and fixed in the other: only the fixed axis gets a size.
8. A gradient with more than two stops: `colors` and `stops` lists of equal length.
9. A bound variable with no usable name: the `unnamed` fallback of `css_var_name` is not used here; `token_camel` falls back to the variable id (`codegen.rs:130-136`).
10. A component set (variants): the class is the set's name; the variant property becomes a `String` field with the first option as the default.
11. The SDK floor: `spacing` needs 3.27; the header comment says so. Older SDKs fail analysis with an undefined named parameter, which is the correct signal.

## 9. Non-goals

- Material or Cupertino widgets, `ThemeData`, routing, state, animations, interactions, prototype links, Code Connect mapping.
- Drawing SVG vectors or images (placeholders and comments only), inner shadows, blurs and blend modes.
- Exact grid track placement (Flutter's SDK has no grid widget).
- Writing the widgets an instance imports, the token class or the assets.
- Byte equality with `dart format`; `flutter test` golden images.

## 10. Test plan (red first)

Seams: unit golden tests inside `flutter.rs` (the fixture is `cfg(test)`, `codegen.rs:724-725`), plus `flutter analyze` over the committed golden directory in CI.

| Case | Expected | Fails before |
|---|---|---|
| `card_golden` | `flutter_for(fixture::card())` equals the committed `tests/golden/flutter/lib/card.dart` (`include_str!`) | yes (function absent) |
| `card_passes_flutter_analyze` | CI: `flutter pub get`, `flutter analyze` in the golden directory exits 0 with `flutter_lints` | yes |
| `validator_rejects_an_undefined_token` | the same project with one token renamed in the stub exits non-zero with `undefined_getter` (a script step; proves the check can fail) | yes |
| `tokens_print_as_token_constants` | a fill bound to `Color/Primary` prints `Tokens.colorPrimary`; a gap bound to `Spacing/Md` prints `Tokens.spacingMd`; the file imports `tokens.dart`; a subtree with a token has no `const` ancestor | yes |
| `literal_subtrees_are_const` | a subtree of literals is `const` where possible (the lint passes) | yes |
| `auto_layout_to_row_column_wrap` | row with gap 12, padding 16 -> `Row` with `spacing: 12` inside `padding`; wrapping layout -> `Wrap` with `spacing` and `runSpacing`; grid -> `Wrap` plus the comment | yes |
| `stack_for_absolute_children` | a frame without auto layout -> `Stack` with `Positioned` and a sized parent | yes |
| `paint_and_shape_mapping` | gradient, per-corner radius, ellipse circle, stroke, shadow, opacity, rotation each map as in rule 5 | yes |
| `dart_quote_escaping` | `'`, `$x`, `${x}`, `\`, newline, tab, a control character and emoji: the output compiles under `flutter analyze` and the string round-trips (CI script) | yes |
| `names_cannot_break_out` | a layer name with a newline and `*/` in a comment and as a class name: one-line comment, valid identifier | yes |
| `class_name_collisions_get_a_suffix` | layers named `Text`, `Row`, `Container`: classes `TextWidget`, `RowWidget`, `ContainerWidget`; the generated file analyzes clean | yes |
| `slots_and_props` | slot `icon`: `final Widget? icon;` and `icon ?? ...`; an instance fill is a named argument; `Label` and `Disabled` become typed fields with defaults | yes |
| `output_is_deterministic` | two runs equal | yes |
| `language_listed_once` | `CodeLanguage::ALL` contains `Flutter`; the MCP enum has `flutter` with no hand-edited list; the dropdown test still fits | yes |

Run: `cargo test -p designos-export flutter`, `cargo test -p designos-mcp`, `cargo test -p designos-app dev`, and locally `cd crates/designos-export/tests/golden/flutter && flutter pub get && flutter analyze`.

## 11. Performance budgets

At most 20 ms for a 2,000-node frame (T7.29's budget for the other text targets, `test_timing::fastest_within`); one pass over the subtree. The CI Flutter step is the slowest validator: UNKNOWN until the first run (SDK download and `pub get`); the goal is under 5 minutes with a cache, recorded in the PR. It runs in parallel with the other jobs.

## 12. Dependencies and build order

- **T7.3 missing:** same fallback as #249 (add `Flutter` to the three hand-kept lists; the next task unifies them).
- **Order:** after #249 and #251, one at a time. The shared lines are `lib.rs`, `language.rs`, `slots.rs` (`Lang`), `codegen.rs` (this task adds `dart_quote` only), `ci.yml` and the MCP enum.
- `crates/designos-mcp/src/engine.rs` is a hot spot of the pack: untouched when T7.3 has landed.
- If the CI Flutter step is too slow or flaky on `macos-14`, run it on the ubuntu runner (Flutter analysis is platform independent); that is a CI placement detail, not a spec change.

## 13. Owner rulings

- A5 and A7: bound variables print code syntax with resolved fallbacks; iOS and ANDROID verbatim for native targets. (Issue text; `docs/specs/v0.6/T7.4-get-design-context.md:138`.) Which platform Flutter reads is not ruled (open question 1).
- Full scope, never MVP.
- Acceptance: the fixture card output passes `flutter analyze`. (Issue.)

## Open questions for the owner (recommended first)

1. **Which code syntax does Flutter print?** The issue says it prints code syntax, but Figma stores only WEB, ANDROID and iOS, and each of those is text in another language (CSS, Kotlin, Swift) that does not compile in Dart.
   - A (recommended): none of them. Print the generated token reference (`Tokens.colorPrimary`) and say so in the spec's contract. The output always compiles, and a Dart name can be set later through a new platform if Figma adds one.
   - B: read the ANDROID syntax verbatim (the closest to a mobile token system), accepting that it is usually a Kotlin expression.
   - C: read the WEB syntax verbatim. Same problem.
   Choosing A means acceptance criterion 2 is met for A5 (a resolved token reference) but not literally for "prints code_syntax".
2. **Material or plain widgets.** Recommended: import only `widgets.dart`, so the output has no Material dependency and no `Card`-style name clashes. Alternative: Material widgets (`Card`, `Material`, `Theme`), which look idiomatic in a Material app but tie the output to it.
3. **Minimum Flutter version.** Recommended: 3.27 (needed for `spacing:`), stated in the file's first line. Alternative: emit `SizedBox` gaps instead of `spacing:` to support older SDKs, at the cost of noisier, less idiomatic code.
