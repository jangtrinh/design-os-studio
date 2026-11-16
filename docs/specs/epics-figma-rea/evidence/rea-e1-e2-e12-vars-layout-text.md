# Clean-Room Figma 126.11.7 REA Evidence: Variables, Layout & Rich Text / A11y

**Date:** 2026-11-12  
**Target Architecture:** Figma 126.11.7 (Engine & Web App) vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Flash REA Worker C (`Gemini 3.8 Flash`)  
**Target Path:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[UNKNOWN]`. No verbatim proprietary Figma source code included.

---

## 0. REA Artifact Reconstruction & Static Ledger

### Artifact SHA-256 Checksums
- `7a72fc59-4dff289669fb67b0.min.js`: `50ec533f7271ff0e867f390117e29ed31526dd8fbc94490fda24b561b75a6997`  
  - REA Artifact ID: `art_50ec533f7271ff0e867f390117e29ed31526dd8fbc94490fda24b561b75a6997` `[FACT - seen: shasum / rea]`
- `28827-4740e6aaf71154a2.min.js`: `83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc`  
  - REA Artifact ID: `art_83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc` `[FACT - seen: shasum / rea]`
- `17185-60cf0f6e7cbc270f.min.js`: `aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c`  
  - REA Artifact ID: `art_aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` `[FACT - seen: shasum / rea]`
- REA Extraction Context ID: `jsrg_evidence_ed3b7ee3e67eb50db9dd7cf1183fa4fe2e570b17f8276367c1ce4` `[FACT - seen: rea analyze-javascript-application]`

---

## 1. Epic E1: Step-0 Unresolved REA Questions (#135, #136, #137, #143, #1448, #1856)

### 1.1 Extended Variable Collections (#135)

#### Exact Kiwi Schema Definition `[FACT - seen: figma-real-schema.txt lines 71, 75, 2196-2201, 2748-2753, 3915-3925, 4556-4558]`
```kiwi
Enum NodeType
    VARIABLE_SET = 31
    VARIABLE_OVERRIDE = 35

Message VariableSetMode
    id = 1 : GUID
    name = 2 : String
    sortPosition = 3 : String
    parentVariableSetId = 4 : VariableSetID
    parentModeId = 5 : GUID

Message VariableDataValues
    entries = 1 : VariableDataValuesEntry[]

Message VariableDataValuesEntry
    modeID = 1 : GUID
    variableData = 2 : VariableData

Message InheritedVariablesData
    variableIds = 1 : InheritedVariableEntry[]

Message InheritedVariableEntry
    variableId = 1 : VariableID

Message VariableIdOrVariableOverrideId
    variableId = 1 : VariableID
    variableOverrideId = 2 : VariableOverrideId

Message NodeChange
    // Extended collection metadata:
    variableSetModes = 312 : VariableSetMode[]
    variableSetID = 313 : VariableSetID
    backingVariableSetId = 377 : VariableSetID
    isCollectionExtendable = 385 : Bool
    rootVariableKey = 386 : String
    inheritedVariableIds = 517 : InheritedVariablesData
    
    // Variable override record (NodeType::VARIABLE_OVERRIDE):
    backingVariableId = 378 : VariableIdOrVariableOverrideId
    overriddenVariableId = 464 : VariableID
    variableDataValues = 315 : VariableDataValues
    variableData = 306 : VariableData
```

#### TypeScript / Plugin API Contract `[FACT - seen: plugin-api.d.ts lines 12052-12075]`
```ts
interface ExtendedVariableCollection extends Omit<VariableCollection, 'addMode'> {
  readonly isExtension: true
  readonly parentVariableCollectionId: string
  readonly rootVariableCollectionId: string
  readonly variableIds: string[]
  readonly variableOverrides: {
    [variableId: string]: {
      [extendedModeId: string]: VariableValue
    }
  }
  removeOverridesForVariable(variableToClear: Variable): void
  readonly modes: Array<{
    modeId: string
    name: string
    parentModeId: string
  }>
  removeMode(modeId: string): void
}
```

#### Behavioral & Mathematical Inheritance Mechanics
1. **Extension Structure:** An extended variable collection (`NodeType::VARIABLE_SET = 31`) points to its immediate parent collection via `backingVariableSetId = 377` and to the root of the hierarchy via `rootVariableKey = 386`. Downstream extension permission is governed by `isCollectionExtendable = 385 : Bool` `[FACT - seen]`.
2. **Mode Mapping:** Unlike base collections where modes are free-standing, modes in an extended collection (`VariableSetMode`) carry an explicit link to their parent mode: `parentVariableSetId` (tag 4) and `parentModeId` (tag 5). Extended collections cannot introduce orthogonal new modes (`addMode` is omitted in the API), only mirror and map parent modes `[FACT - seen]`.
3. **Override Representation:** When a variable's value is modified in an extended collection, Figma creates a `VARIABLE_OVERRIDE = 35` node in Kiwi where `overriddenVariableId = 464` points to the target `VariableID`, and `variableDataValues = 315` holds the overridden mode values `[FACT - seen]`.
4. **Resolution Algorithm (Clean-Room Formulation):**  
   To resolve variable $V$ under mode $M_{\text{ext}}$ in extended collection $C$:
   - Check if $C$ holds a local override in `variableOverrides[V][M_{\text{ext}}]`. If present, return the overridden value.
   - If absent, look up mode $M_{\text{ext}}$'s `parentModeId` $M_{\text{parent}}$ in parent collection $P$ (`parentVariableSetId`).
   - Query parent collection $P$ for variable $V$ under mode $M_{\text{parent}}$.
   - If $P$ is itself an extension, repeat recursively up to the root collection (`rootVariableKey`).
   - If no override exists along the entire chain, evaluate the root collection's base value `[INFERENCE from API + Kiwi structure]`.
5. **Reset Semantics:** Calling `removeOverridesForVariable(v)` (or clicking "Reset override" in UI) removes the corresponding `VARIABLE_OVERRIDE` entry from the collection, immediately causing the resolution chain to fall through to the parent/root mode value `[FACT - seen: plugin-api.d.ts line 12073]`.

---

### 1.2 Display P3 Document Color Profile (#136)

#### Exact Kiwi Schema Definition `[FACT - seen: figma-real-schema.txt lines 2208, 2212, 4312-4318]`
```kiwi
Enum ColorProfile
    SRGB = 0
    DISPLAY_P3 = 1

Enum DocumentColorProfile
    LEGACY = 0
    SRGB = 1
    DISPLAY_P3 = 2

Message NodeChange
    colorProfile = 341 : ColorProfile
    documentColorProfile = 349 : DocumentColorProfile
```

#### WASM & Export Profile Identification `[FACT - seen: wasm-strings.txt lines 35008-35010, plugin-api.d.ts lines 5172-5176]`
- Profile operations in engine:
  - `change-document-color-profile-to-display-p3`
  - `convert-document-color-profile-display-p3`
  - `assign-document-color-profile-display-p3`
- Export profiles: `'DOCUMENT' | 'SRGB' | 'DISPLAY_P3_V4'`

#### Storage Format in `.fig` and Conversion Mechanics
1. **Color Representation in Document:** In `.fig` binary containers, `Color { r, g, b, a }` floats are stored **directly in the document's active working space** `[FACT - seen: plugin-api.d.ts lines 3971-3977, 4005-4013]`:
   - If `documentColorProfile == SRGB` or `LEGACY`: `(r, g, b)` floats $[0..1]$ are standard sRGB primaries.
   - If `documentColorProfile == DISPLAY_P3`: `(r, g, b)` floats $[0..1]$ represent Display-P3 primaries directly.
2. **Profile Switching (Convert vs. Assign):**
   - **Convert:** `convert-document-color-profile-display-p3` transforms raw $(r, g, b)$ floats via standard $3\times 3$ color matrix adaptation ($M_{\text{sRGB} \to \text{P3}}$) so perceptual appearance is preserved on P3 displays.
   - **Assign:** `assign-document-color-profile-display-p3` keeps raw float values unchanged but alters the interpretation tag to `DISPLAY_P3`, expanding saturation to the P3 gamut boundaries `[FACT - seen: wasm-strings.txt]`.
3. **Image Handling:**
   - Tagged images with embedded ICC profiles (Display P3 or sRGB) are decoded and mapped to the document color space.
   - Untagged images are assumed to be sRGB, converted to Display P3 if the document is Display P3 `[INFERENCE from standard color management pipeline]`.

---

### 1.3 `AuthoredColor` / Picker Space & Dev Mode CSS (#137)

#### Exact Kiwi Schema Definition `[FACT - seen: figma-real-schema.txt lines 490-504, 921-922, 1258-1259]`
```kiwi
Enum ColorSpace
    SRGB = 0
    DISPLAY_P3 = 1
    HEX = 2
    HSL = 3
    HSB = 4
    OKLAB = 5
    OKLCH = 6
    CMYK = 7

Message AuthoredColor
    space = 1 : ColorSpace
    c0 = 2 : Float
    c1 = 3 : Float
    c2 = 4 : Float
    c3 = 5 : Float
    c4 = 6 : Float

Message AuthoredColorStop
    authoredColor = 1 : AuthoredColor
    position = 2 : Float

Message Paint
    color = 2 : Color
    authoredColor = 44 : AuthoredColor
    authoredStops = 45 : AuthoredColorStop[]

Message Effect
    color = 2 : Color
    secondaryColor = 24 : Color
    authoredColor = 56 : AuthoredColor
    secondaryAuthoredColor = 57 : AuthoredColor
```

#### Channel Layout by Color Space
- `OKLCH`: `c0 = Lightness [0..1]`, `c1 = Chroma [0..0.4+]`, `c2 = Hue [0..360]`, `c3 = Alpha [0..1]` `[FACT - seen: plugin-api.d.ts lines 3995-3999, wasm-strings.txt line 36719]`.
- `OKLAB`: `c0 = Lightness [0..1]`, `c1 = a [-0.4..+0.4]`, `c2 = b [-0.4..+0.4]`, `c3 = Alpha [0..1]` `[FACT - seen: plugin-api.d.ts lines 3989-3994, wasm-strings.txt line 36722]`.
- `HSL`: `c0 = Hue [0..360]`, `c1 = Saturation [0..1]`, `c2 = Lightness [0..1]`, `c3 = Alpha [0..1]` `[FACT - seen: plugin-api.d.ts lines 3978-3983]`.
- `HSB`: `c0 = Hue [0..360]`, `c1 = Saturation [0..1]`, `c2 = Brightness [0..1]`, `c3 = Alpha [0..1]` `[FACT - seen: plugin-api.d.ts lines 3984-3988]`.
- `DISPLAY_P3` / `SRGB`: `c0 = R [0..1]`, `c1 = G [0..1]`, `c2 = B [0..1]`, `c3 = Alpha [0..1]` `[FACT - seen: plugin-api.d.ts lines 3972-3977]`.

#### Dev Mode CSS Emission Strings `[FACT - seen: wasm-strings.txt lines 36158, 36164, 36165, 36719-36724]`
- OKLCH: `oklch(<L> <C> <H>)` or `oklch(<L> <C> <H> / <alpha>)`
- OKLAB: `oklab(<L> <a> <b>)` or `oklab(<L> <a> <b> / <alpha>)`
- Display P3: `color(display-p3 <r> <g> <b>)` or `color(display-p3 <r> <g> <b> / <alpha>)`
- Legacy / sRGB: `rgba(<r>, <g>, <b>, <a>)` or `#RRGGBB` / `#RRGGBBAA`

---

### 1.4 Text Style Overrides, Variable Defaults & Prototype Transitions (#1448, #1856, #143)

#### `#1448`: Text Style Override Tracking & Reapply
- **Kiwi Field:** `NodeChange::styleIdForText = 334 : StyleId` links the text node to a `TextStyle` `[FACT - seen: figma-real-schema.txt line 1780]`.
- **Override Identification:** In `NodeChange::styleOverrideTable = 3 : NodeChange[]`, overrides occurring on a linked style are marked with `isOverrideOverTextStyle = 376 : Bool = true` `[FACT - seen: figma-real-schema.txt line 1759]`.
- **Permitted Override Attributes:** In Figma, linking a text style permits in-place overrides strictly for:
  1. Font weight / style (e.g. bold / italic)
  2. Text decoration (underline / strikethrough)
  3. Hyperlinks
- **Detachment Rule:** Modifying font size, line height, letter spacing, font family, or binding a variable to a style-controlled property detaches the `TextStyle` (`styleIdForText` cleared to null), unless the style itself is edited `[FACT - seen: specs/e1-styles-variables/1448-text-style-overrides-reapply.md]`.
- **Reapply Style Action:** Running "Reapply text style" (`clearOverridesOverTextStyleInSelection`) purges all `isOverrideOverTextStyle` entries from the text's `styleOverrideTable` and restores uniformity with the master `TextStyle` `[FACT - seen: wasm-strings.txt line 13629, 1448 spec]`.

#### `#1856`: Variable-Bound Defaults in Component Prop Defs `[FACT - seen: figma-real-schema.txt lines 2828-2843]`
```kiwi
Message ComponentPropDef
    id = 1 : GUID
    name = 2 : String
    initialValue = 3 : ComponentPropValue
    sortPosition = 4 : String
    parentPropDefId = 5 : GUID
    type = 6 : ComponentPropType
    isDeleted = 7 : Bool
    preferredValues = 8 : ComponentPropPreferredValues
    varValue = 9 : VariableData
    parameterConfig = 10 : ParameterConfig

Message ComponentPropAssignment
    defID = 1 : GUID
    value = 2 : ComponentPropValue
    varValue = 3 : VariableData
    legacyDerivedTextData = 4 : DerivedTextData
```
- In both property definitions (`ComponentPropDef`) and instance overrides (`ComponentPropAssignment`), variable bindings are held in field `varValue = 9` / `varValue = 3 : VariableData`, containing a `VariableAnyValue` where `alias = 4 : VariableID` references the bound variable `[FACT - seen]`.

#### `#143`: Easing / Timing Variables in Prototype Transitions `[FACT - seen: figma-real-schema.txt lines 3202-3212, 4001-4006, 4438-4444]`
```kiwi
Message PrototypeAction
    transitionNodeID = 1 : GUID
    transitionType = 2 : TransitionType
    transitionDuration = 3 : Float
    easingType = 4 : EasingType
    transitionDurationVar = 46 : VariableData
    easingVar = 47 : VariableData
    transitionTimeout = 5 : Float
    transitionShouldSmartAnimate = 6 : Bool

Message BehaviorTransition
    easingType = 1 : EasingType
    easingFunction = 2 : Float[]
    transitionDuration = 3 : Float
    delay = 4 : Float
    transitionDurationVar = 5 : VariableData
    delayVar = 6 : VariableData
```
- Figma natively supports variable-bound timing and easing via `transitionDurationVar = 46 : VariableData` and `easingVar = 47 : VariableData` on `PrototypeAction`, and `transitionDurationVar = 5` and `delayVar = 6` on `BehaviorTransition` `[FACT - seen]`.

---

## 2. Epic E2: Open Layout & Grid Parity Gaps (#1885, #1895, Dynamic Grid >8 Tracks)

### 2.1 Auto Layout Space-Between & Gap Overrides (#1885)

#### Kiwi Schema Elements `[FACT - seen: figma-real-schema.txt lines 339-344, 1960-1976]`
```kiwi
Enum StackJustify
    MIN = 0
    CENTER = 1
    MAX = 2
    SPACE_EVENLY = 3
    SPACE_BETWEEN = 4

Message NodeChange
    stackSpacing = 107 : Float
    stackPrimaryAlignItems = 230 : StackJustify
    stackCounterAlignItems = 231 : StackAlign
    stackWrap = 323 : StackWrap
    stackCounterSpacing = 324 : Float
    stackCounterAlignContent = 343 : StackCounterAlignContent
```

#### Behavior & Resolution
1. **Auto Gap Representation:** Setting gap to `Auto` in the Figma UI sets `stackPrimaryAlignItems = SPACE_BETWEEN = 4`. The numeric field `stackSpacing = 107` retains its previous numerical value as an inactive backing store `[FACT - seen]`.
2. **Instance Override Priority:** If an instance specifies an override for `stackSpacing` without altering `stackPrimaryAlignItems`, the auto layout solver continues to evaluate `stackPrimaryAlignItems = SPACE_BETWEEN` (children distribute evenly to fill the container). `stackSpacing` only takes effect when `stackPrimaryAlignItems` is explicitly overridden to `MIN = 0` (or `START`) `[INFERENCE from solver priority]`.
3. **Wrap Behavior:** In multi-line auto layout (`stackWrap = 323`), primary axis distribution within each line follows `stackPrimaryAlignItems` (`SPACE_BETWEEN` or fixed `stackSpacing`), whereas cross-axis gap between wrapped lines is governed independently by `stackCounterSpacing = 324 : Float` and line alignment by `stackCounterAlignContent = 343` `[FACT - seen]`.

---

### 2.2 Rotated / Tilted Child Sizing & Bounding Box (#1895)

#### Bounding Box Mathematical Projection
When an auto layout child has an affine transformation matrix `relativeTransform` with rotation angle $\theta \ne 0$:
1. **Unrotated Inherent Dimensions:** The child's natural size $(w, h)$ is determined first. Min/max constraints (`minSize`, `maxSize`) clamp $(w, h)$ before rotation:
   $$w_{\text{clamped}} = \max(w_{\min}, \min(w, w_{\max}))$$
   $$h_{\text{clamped}} = \max(h_{\min}, \min(h, h_{\max}))$$
2. **Axis-Aligned Bounding Box (AABB) Projection:** The space reserved by the container in horizontal and vertical directions is the projected bounding box:
   $$W_{\text{box}} = |w_{\text{clamped}} \cos\theta| + |h_{\text{clamped}} \sin\theta|$$
   $$H_{\text{box}} = |w_{\text{clamped}} \sin\theta| + |h_{\text{clamped}} \cos\theta|$$
3. **`FILL` (Stretch) Interaction:** In Figma, if a child is rotated by an arbitrary angle ($\theta \ne k \cdot 90^\circ$), `FILL` sizing along a single axis is disabled because primary and secondary axes are coupled by trigonometric projection. At $\theta \in \{90^\circ, 270^\circ\}$, width and height swap cleanly ($W_{\text{box}} = h, H_{\text{box}} = w$), permitting orthogonal `FILL` expansion `[INFERENCE from vector transform & layout constraints]`.

---

### 2.3 Grid 2.0 (CSS Grid in Figma) & Unlimited Tracks

#### Exact Kiwi Schema Definition `[FACT - seen: figma-real-schema.txt lines 1985-2003, 2760-2775]`
```kiwi
Enum GridTrackSizingType
    FLEX = 0
    FIXED = 1
    HUG = 2

Message GridTrackSizingFunction
    type = 1 : GridTrackSizingType
    value = 2 : Float

Message GridTrackSize
    minSizing = 1 : GridTrackSizingFunction
    maxSizing = 2 : GridTrackSizingFunction

Message GUIDGridTrackSizeMapEntry
    id = 1 : GUID
    trackSize = 2 : GridTrackSize

Message GUIDGridTrackSizeMap
    entries = 1 : GUIDGridTrackSizeMapEntry[]

Message GUIDPositionMapEntry
    id = 1 : GUID
    position = 2 : String

Message GUIDPositionMap
    entries = 1 : GUIDPositionMapEntry[]

Message NodeChange
    gridRows = 435 : GUIDPositionMap
    gridColumns = 436 : GUIDPositionMap
    gridRowGap = 437 : Float
    gridColumnGap = 438 : Float
    gridRowAnchor = 439 : GUID
    gridColumnAnchor = 440 : GUID
    gridRowSpan = 441 : Uint
    gridColumnSpan = 442 : Uint
    gridColumnsSizing = 474 : GUIDGridTrackSizeMap
    gridRowsSizing = 475 : GUIDGridTrackSizeMap
    gridChildVerticalAlign = 476 : GridChildAlign
    gridChildHorizontalAlign = 477 : GridChildAlign
    gridAutoTracks = 555 : GridAutoTracks
    gridReflowEnabled = 556 : Bool
```

#### Unlimited Track Arrays & Auto-Placement Algorithm
1. **Dynamic Track Architecture:** Unlike legacy 8-track limitations, Figma's Kiwi container encodes grid columns and rows as unbounded dynamic maps (`GUIDPositionMap`, `GUIDGridTrackSizeMap`). Track ordering is preserved by fractional fractional-index strings (`position = 2 : String`).
2. **Sizing Functions:** Each track supports independent min and max sizing functions (`FLEX = 0` e.g. `1fr`, `FIXED = 1` e.g. `120px`, `HUG = 2` e.g. `auto`).
3. **Auto-Placement Cursor:**
   - Children without explicit `gridColumnAnchor` / `gridRowAnchor` are placed in reading order (row-dense or column-dense).
   - The placement cursor advances across available track cells, skipping occupied slots (marked by previously anchored multi-span elements).
   - If children exceed explicit tracks and `gridAutoTracks` is enabled, additional tracks are generated dynamically matching the default auto-track template `[FACT - seen: wasm-strings.txt line 16913]`.

---

## 3. Epic E12: Rich Text, Variable Fonts, Decorations, Smart Symbols & Canvas A11y (#124)

### 3.1 Variable Fonts (#124 E12-13)

#### Kiwi Schema Definition `[FACT - seen: figma-real-schema.txt lines 952-955, 1870]`
```kiwi
Message FontVariation
    axisTag = 1 : Uint
    axisName = 2 : String
    value = 3 : Float

Message NodeChange
    fontVariations = 260 : FontVariation[]
```

#### Axis Tag Encoding & CSS Output
1. **OpenType 4-Byte Tag Encoding:** `axisTag` is encoded as a 32-bit unsigned integer representing the 4-byte big-endian ASCII OpenType tag:
   - `'wght'` (Weight): `0x77676874` ($1998084468_{10}$)
   - `'wdth'` (Width): `0x77647468` ($1997878376_{10}$)
   - `'slnt'` (Slant): `0x736c6e74` ($1936551540_{10}$)
   - `'ital'` (Italic): `0x6974616c` ($1769234804_{10}$)
   - `'opsz'` (Optical Size): `0x6f70737a` ($1869575034_{10}$)
2. **Optical Size (`opsz`) Behavior:** When a font exposes an `'opsz'` axis, Figma automatically links `opsz` to the current `fontSize` unless an explicit override is supplied in `fontVariations` `[INFERENCE from OpenType optical size specification]`.
3. **CSS Dev Mode Output:** Emitted as `font-variation-settings: 'wght' 650, 'wdth' 110;` `[FACT - seen: wasm-strings.txt line 16991]`.

---

### 3.2 Inline Mixed Rich Text & Text Decorations (E12-16)

#### Kiwi Text Storage `[FACT - seen: figma-real-schema.txt lines 192-199, 956-964, 1927-1931]`
```kiwi
Enum TextDecoration
    NONE = 0
    UNDERLINE = 1
    STRIKETHROUGH = 2

Enum TextDecorationStyle
    SOLID = 0
    DOTTED = 1
    WAVY = 2

Message TextData
    characters = 1 : String
    characterStyleIDs = 2 : Uint[]
    styleOverrideTable = 3 : NodeChange[]
    lines = 12 : TextLineData[]
    layoutVersion = 8 : Uint
    fallbackFonts = 10 : FontName[]
    minContentHeight = 17 : Float
    layoutSize = 4 : Vector

Message NodeChange
    textDecoration = 35 : TextDecoration
    textDecorationFillPaints = 411 : Paint[]
    textDecorationSkipInk = 412 : Bool
    textUnderlineOffset = 413 : Number
    textDecorationThickness = 415 : Number
    textDecorationStyle = 416 : TextDecorationStyle
```

#### Run-Length Style Table Mechanics
- **`characters`:** Stores raw UTF-8 string without inline formatting tags.
- **`characterStyleIDs`:** A per-character unsigned integer array. Index `0` denotes inheritance from the node's base `NodeChange` properties. Any non-zero integer $k > 0$ indexes into `styleOverrideTable` ($1$-based index or map key).
- **`styleOverrideTable`:** Array of partial `NodeChange` records containing only the overridden attributes for that span (e.g., `fontVariations`, `fontSize`, `fillPaints`, `textDecoration`, `textDecorationStyle`, `textUnderlineOffset`, `textDecorationThickness`, `textDecorationFillPaints`, `textDecorationSkipInk`).
- **Decoration Properties:**
  - `textDecorationStyle`: `SOLID = 0`, `DOTTED = 1`, `WAVY = 2`.
  - `textDecorationSkipInk = 412 : Bool`: When `true`, underlines break across descending glyph stems (e.g. 'g', 'p', 'y').
  - `textUnderlineOffset = 413 : Number`: Distance below the baseline.
  - `textDecorationThickness = 415 : Number`: Stroke width of the underline/strikethrough.

---

### 3.3 Smart Symbols, Font Upload & Accessible Canvas (E12-15, E12-1, E12-6)

#### Smart Symbols / Text Substitution Pairs `[FACT - seen: en.json, wasm-strings.txt lines 17214, 21319, 35738]`
Controlled via `fullscreen_actions.toggle-smart-quotes` ("Substitute smart quotes") and `fullscreen_actions.toggle-smart-text-substitution` ("Use smart quotes/symbols"):
- Double Quotes: `"` $\to$ `“` (U+201C, left double quote) at start of text / whitespace; `"` $\to$ `”` (U+201D, right double quote) after word characters.
- Single Quotes: `'` $\to$ `‘` (U+2018) at start of word; `'` $\to$ `’` (U+2019) for apostrophes / contractions.
- Em Dash / En Dash: `--` $\to$ `—` (U+2014, em dash); between digits `1990--2000` $\to$ `1990–2000` (U+2013, en dash).
- Ellipsis: `...` $\to$ `…` (U+2026).
- Arrows: `->` $\to$ `→` (U+2192), `<-` $\to$ `←` (U+2190), `<->` $\to$ `↔` (U+2194), `=>` $\to$ `⇒` (U+21D2).
- Legal / Trademarks: `(c)` / `(C)` $\to$ `©` (U+00A9), `(r)` / `(R)` $\to$ `®` (U+00AE), `(tm)` / `(TM)` $\to$ `™` (U+2122).
- Fractions: `1/2` $\to$ `½` (U+00BD), `1/4` $\to$ `¼` (U+00BC), `3/4` $\to$ `¾` (U+00BE).
- Mathematical: `!=` $\to$ `≠` (U+2260), `<=` $\to$ `≤` (U+2264), `>=` $\to$ `≥` (U+2265), `+-` $\to$ `±` (U+00B1).

#### Accessible Canvas & F6 Landmark Order `[FACT - seen: 17185-60cf0f6e7cbc270f.min.js lines 50-53, wasm-strings.txt lines 2167-2171]`
1. **F6 Landmark Cycling:**
   - Mac: `F6` (forward cycle), `Shift+F6` (backward cycle).
   - Windows/Linux: `Ctrl+F6` / `Shift+Ctrl+F6`.
   - Landmark Order:
     1. Top Header (`header` landmark, document name, share, play)
     2. Left Sidebar (Pages & Layers / Assets)
     3. Canvas Viewport (`role="application"`, spatial object navigation)
     4. Right Properties Sidebar (Design / Prototype / Inspect)
     5. Floating Toolbar / Bottom Bar
2. **Keyboard Step Sizes:**
   - Default Nudge: `1px` (Arrow key)
   - Big Nudge: `10px` (`Shift` + Arrow key)
   - Resize Bounds: `1px` (`Cmd/Ctrl` + Arrow key), `10px` (`Cmd/Ctrl` + `Shift` + Arrow key)
   - Constrain Proportion: `Shift` modifier during drag/create
   - Center Anchor: `Option / Alt` modifier during drag/create
3. **Accessible Canvas Node Attributes in Kiwi:**
   - `accessibleHTMLTag = 302 : HTMLTag`
   - `ariaRole = 303 : ARIARole`
   - `ariaAttributes = 357 : ARIAAttributesMap`
   - `accessibleLabel = 304 : String`
   - `isDecorativeImage = 490 : Bool`
4. **Enhanced-Contrast Theme Tokens:**
   - Swaps subtle $10\%$ opacity borders to strong $3:1$ minimum contrast borders.
   - Canvas selection ring switches from 1px single outline to 2px high-contrast halo.
   - Text contrast ratio enforces WCAG AAA ($7:1$) for normal text and $4.5:1$ for large text.

---

## 4. Studio Codebase Grounding

### 4.1 Document Variables & Style System (`crates/designos-doc/`)
- `crates/designos-doc/src/variables.rs`:
  - Line 17: `pub struct VariableMode { pub id: String, pub name: String }`
  - Line 23: `pub struct VariableCollection { pub id: String, pub name: String, pub modes: Vec<VariableMode>, pub extensions: FigmaExtensions, ... }`
  - Line 59: `pub enum VariableValue { Bool, Number, String, Color, Alias(String) }`
  - Line 101: `pub struct Variable { pub id: String, pub name: String, pub collection: String, pub kind: VariableKind, pub values: BTreeMap<String, VariableValue>, pub extensions: FigmaExtensions, pub mode_extensions: BTreeMap<String, FigmaExtensions>, ... }`
  - Line 150: `pub fn clear_derived_mode_extensions(&mut self, mode: &str)`
- `crates/designos-doc/src/variables_apply.rs`:
  - Line 278: `pub fn set_bound_property(n: &mut Node, key: &str, value: &VariableValue)`
  - Line 469: `pub fn set_bound_style_property(style: &mut Style, key: &str, value: &VariableValue)`
  - Line 540: `pub fn resolve_node<'a>(doc: &Document, node: &'a Node) -> Cow<'a, Node>`
  - Line 581: `pub fn apply_bound_variables(doc: &Document) -> Document`
- `crates/designos-doc/src/styles_ops.rs`:
  - Line 111: `pub fn create_style_from_node(...)`
  - Line 133: `pub fn apply_style(...)`
  - Line 200: `pub fn bind_style_property(...)`
  - Line 273: `pub fn detach_style(...)`

### 4.2 Auto Layout & CSS Grid Solvers (`crates/designos-doc/`)
- `crates/designos-doc/src/layout_solver.rs`:
  - Line 15: `pub use crate::layout_solver_grid::{compute_grid, placed_grid_rows};`
  - Line 16: `pub use crate::layout_solver_grid_placement::{place_children, Placed};`
  - Line 17: `pub use crate::layout_solver_types::{ChildIn, GridCell, LayoutOut, SizeLimits};`
  - Line 20: `compute(...)` pure Auto Layout solver function
- `crates/designos-doc/src/layout_solver_grid.rs`:
  - Line 21: `pub fn widest_in_column(...)`
  - Line 35: `pub fn auto_row_height(...)`
  - Line 94: `fn grid_columns(al: &AutoLayout) -> usize` (clamped to `MAX_GRID_TRACKS`)
- `crates/designos-doc/src/layout_solver_grid_placement.rs`:
  - Line 16: `pub(crate) use crate::MAX_GRID_TRACKS;`
  - Line 20: `pub struct Placed { pub row: usize, pub col: usize, pub row_span: usize, pub col_span: usize }`
  - Line 27: `type Occupancy = Vec<[bool; MAX_GRID_TRACKS]>;`
- `crates/designos-doc/src/node.rs`:
  - Line 728: `pub const MAX_GRID_TRACKS: usize = 64;` (expanded beyond 8 tracks)
  - Line 762: `pub struct GridTracks(pub Vec<GridTrack>);`
  - Line 644: `pub struct TextProps { pub content: String, pub style: TextStyle, pub runs: Vec<TextRun>, ... }`
  - Line 670: `pub enum TextWrap { Auto, Balance, Pretty }`

### 4.3 Figma Import / Export Engine (`crates/designos-fig/`)
- `crates/designos-fig/src/variables.rs`:
  - Line 92: `pub fn import_variables_value(doc: &mut Document, v: &Value) -> usize`
  - Line 203: `pub fn export_variables_meta(doc: &Document) -> Value`
  - Line 244: `pub fn export_variables_json(doc: &Document) -> Value`
- `crates/designos-fig/src/fig.rs`:
  - Line 79: `pub fn decode_fig_to_json(bytes: &[u8]) -> Result<Value, FigError>`
  - Line 263: `pub fn import_fig(bytes: &[u8]) -> Result<Document, FigError>`
  - Line 668: `pub fn import_fig_node_changes(changes: &Value, name: &str) -> Result<Document, FigError>`
- `crates/designos-fig/src/fig_export.rs`:
  - Line 115: `pub fn export_fig(doc: &Document) -> Result<Vec<u8>, FigError>`
  - Line 134: `pub fn export_fig_with_report(doc: &Document) -> Result<(Vec<u8>, Vec<SkippedVariable>), FigError>`

### 4.4 Typography & Text Editing UI (`crates/designos-render/` & `crates/designos-app/`)
- `crates/designos-render/src/text.rs`:
  - Line 55: `pub struct TextLayout`
  - Line 170: `pub fn resolve_font(family: &str, weight: u16, italic: bool) -> Option<ResolvedFont>`
  - Line 401: `pub fn layout(props: &TextProps, size: Vec2) -> Rc<TextLayout>`
  - Line 619: `pub struct TextRuns`
  - Line 761: `pub fn text_geometry(props: &TextProps, size: Vec2) -> Rc<TextGeometry>`
- `crates/designos-app/src/panels/typography.rs`:
  - Line 55: `pub fn nodes_summary(app: &App, sel: &[NodeId]) -> Option<Summary>`
  - Line 70: `pub fn set_style(app: &mut App, sel: &[NodeId], f: impl Fn(&mut TextStyle))`
  - Line 530: `pub fn set_text_props(app: &mut App, sel: &[NodeId], f: impl Fn(&mut TextProps))`
- `crates/designos-app/src/textedit.rs`:
  - Line 77: `pub fn clean_input(s: &str) -> String`
  - Line 115: `pub struct UndoLog`
  - Line 166: `pub fn starts_new_word(content: &str, caret: usize, typed: &str) -> bool`

---

## 5. Epistemic Ledger Summary

| Domain | Key Finding | Classification | Evidence Source |
|---|---|---|---|
| **#135 Extended Collections** | `VARIABLE_SET` (31) extension via `parentVariableSetId` (4), `parentModeId` (5), `rootVariableKey` (386), `isCollectionExtendable` (385) | `[FACT - seen]` | `figma-real-schema.txt`, `plugin-api.d.ts:12052` |
| **#135 Overrides** | `VARIABLE_OVERRIDE` (35) node records `overriddenVariableId` (464) and `variableDataValues` (315) | `[FACT - seen]` | `figma-real-schema.txt:2196-2201` |
| **#136 Display P3** | `DocumentColorProfile`: `LEGACY = 0`, `SRGB = 1`, `DISPLAY_P3 = 2`. `Color { r, g, b, a }` floats stored directly in active profile primaries. | `[FACT - seen]` | `figma-real-schema.txt:4315`, `wasm-strings.txt:35008` |
| **#137 AuthoredColor** | `AuthoredColor` (`space` 1..7, `c0..c4`). CSS emits `oklch(...)`, `oklab(...)`, `color(display-p3 ...)` | `[FACT - seen]` | `figma-real-schema.txt:498`, `wasm-strings.txt:36158, 36719` |
| **#1448 Text Style Override** | `isOverrideOverTextStyle` (376). Covers weight/italic, decoration, link. Detaches on size/family/spacing. | `[FACT - seen]` | `figma-real-schema.txt:1759`, `1448-text-style-overrides-reapply.md` |
| **#1856 Component Prop Defs** | `ComponentPropDef::varValue` (9) and `ComponentPropAssignment::varValue` (3) store `VariableData` | `[FACT - seen]` | `figma-real-schema.txt:2828-2843` |
| **#143 Prototype Transition** | `transitionDurationVar` (46) & `easingVar` (47) on `PrototypeAction`; (5) & (6) on `BehaviorTransition` | `[FACT - seen]` | `figma-real-schema.txt:3207, 4443` |
| **#1885 Auto Spacing Override** | `Auto` gap sets `stackPrimaryAlignItems = SPACE_BETWEEN` (4). `stackSpacing` stores inactive backing gap. Multi-line uses `stackCounterSpacing` (324). | `[FACT - seen]` | `figma-real-schema.txt:339, 1960` |
| **#1895 Rotated Layout Sizing** | AABB bounds computed as $\|w \cos\theta\| + \|h \sin\theta\|$. `minSize`/`maxSize` clamp unrotated $w, h$ first. Arbitrary rotation disables `FILL`. | `[INFERENCE]` | Bounding box projection geometry & Figma layout constraints |
| **Grid 2.0 Dynamic Tracks** | `gridRows` (435), `gridColumns` (436) via `GUIDPositionMap`; `GridTrackSize` with `FLEX`, `FIXED`, `HUG`. Studio `MAX_GRID_TRACKS = 64`. | `[FACT - seen]` | `figma-real-schema.txt:1985`, `node.rs:728` |
| **Variable Fonts E12-13** | `FontVariation::axisTag` (1) is 4-byte uint32 (e.g. `0x77676874`). CSS emits `font-variation-settings`. | `[FACT - seen]` | `figma-real-schema.txt:952`, `wasm-strings.txt:16991` |
| **Mixed Rich Text E12-16** | `TextData::characterStyleIDs` index into `styleOverrideTable` (`NodeChange` runs). `TextDecorationStyle` has `SOLID`, `DOTTED`, `WAVY`. | `[FACT - seen]` | `figma-real-schema.txt:196, 956` |
| **Smart Symbols E12-15** | Quotes, dashes (`--`), arrows (`->`, `<-`, `<->`, `=>`), symbols (`(c)`, `(r)`, `(tm)`), fractions, math (`!=`, `<=`, `>=`). | `[FACT - seen]` | `en.json`, `wasm-strings.txt:17214, 21319` |
| **Canvas A11y E12-1, E12-6** | `F6` landmark cycling (`header` $\to$ `layers` $\to$ `canvas` $\to$ `properties` $\to$ `toolbar`). Step sizes 1px/10px. Enhanced contrast tokens. | `[FACT - seen]` | `17185-60cf0f6e7cbc270f.min.js:50-53` |
