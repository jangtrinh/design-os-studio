# `.fig` (fig-kiwi) field names

What the importer reads from the kiwi `NodeChange` messages, and whether each
name has been seen in a real file. Only field names and value shapes are
recorded here; nothing from the files' content.

Real files checked: a library-heavy product UI kit file (`private-ui-kit.fig`,
about 11k node changes, 7 libraries, 112 variables) and the shadcn/ui community
file (`shadcn-ui.fig`, about 5.4k node changes, everything local). Both are
private local copies and are never committed. "Confirmed" means seen in at
least one of them. "Unconfirmed" means the name is a guess (kept because it is
harmless when absent) or was never present in these files.

Dump any file with `cargo run --release -p designos-fig --example fig2json -- file.fig`
and render its frames with `--example fig_render`.

## Structure

| Feature | Fields | Status |
|---|---|---|
| Tree | `guid {sessionID, localID}`, `parentIndex {guid, position}` (fractional index string), `type`, `phase` | confirmed |
| Pages | `CANVAS` nodes under `DOCUMENT`; `backgroundColor`, `internalOnly` | confirmed |
| Internal canvas | `internalOnly: true` canvas holds the remote SYMBOLs, state-group FRAMEs, style nodes, VARIABLE and VARIABLE_SET nodes the file uses (many thousands of nodes) | confirmed |
| Node types | DOCUMENT CANVAS FRAME SYMBOL INSTANCE TEXT VECTOR ELLIPSE ROUNDED_RECTANGLE LINE STAR BOOLEAN_OPERATION SECTION VARIABLE VARIABLE_SET | confirmed. `RECTANGLE`, `REGULAR_POLYGON`, `GROUP` were not present (kept) |
| Placement | `transform {m00..m12}`, `size {x, y}`, `opacity`, `visible`, `locked`, `blendMode`, `mask` | confirmed except `mask` (unconfirmed) |
| Constraints | `horizontalConstraint` / `verticalConstraint`: `MAX`, `CENTER`, `STRETCH`, `SCALE`; absent = min | confirmed |
| Corners | `cornerRadius`, `rectangle{TopLeft,TopRight,BottomLeft,BottomRight}CornerRadius`, `rectangleCornerRadiiIndependent`, `cornerSmoothing` | confirmed |
| Strokes | `strokePaints`, `strokeWeight`, `strokeAlign` (INSIDE/OUTSIDE/CENTER), `strokeCap` (only ROUND seen), `strokeJoin`, `dashPattern`, `border{Top,Right,Bottom,Left}Weight`, `borderStrokeWeightsIndependent`, `miterLimit` | confirmed |
| Frames | `frameMaskDisabled` (absent = clips) | confirmed |
| Auto layout | `stackMode` HORIZONTAL/VERTICAL, `stackSpacing`, `stackCounterSpacing`, `stackHorizontalPadding`, `stackVerticalPadding`, `stackPaddingRight`, `stackPaddingBottom`, `stackPrimaryAlignItems` (CENTER/MAX/SPACE_EVENLY), `stackCounterAlignItems`, `stackChildAlignSelf` (STRETCH/MAX/CENTER), `stackChildPrimaryGrow`, `stackReverseZIndex`, `resizeToFit` | confirmed |
| Auto layout sizing | `stackPrimarySizing`: only `FIXED` is ever written, absent means hug. `stackCounterSizing`: only `RESIZE_TO_FIT_WITH_IMPLICIT_SIZE` is written, absent means fixed. (Checked against stored sizes: absent primary hugs its children.) Fixed in this change; it used to read absent primary as fixed | confirmed |
| Wrap / absolute / min-max | `stackWrap`, `stackPositioning`, `minSize` / `maxSize` | unconfirmed |
| Star | `count`, `starInnerScale` | confirmed |
| Arc | `arcData {startingAngle, endingAngle, innerRadius}` | confirmed |
| Boolean ops | `booleanOperation`: UNION, SUBTRACT, INTERSECT, XOR (mapped to EXCLUDE) | confirmed |
| Layout grids | `layoutGrids` (`pattern` GRID/STRIPES, `axis` X/Y, `type` MIN/CENTER/STRETCH/MAX, `numSections` (negative = automatic), `sectionSize`, `gutterSize`, `offset`, `color`, `visible`) | confirmed (VSF - PCP, BW - DLS, Untitled UI kit). The importer also reads the `COLUMNS` / `ROWS` pattern and `count` that Studio wrote before 0.5 |
| Guides | `guides`: present as an empty list on symbols and instances; element shape (`axis`, `offset`) | field confirmed, element shape unconfirmed |
| Export settings | `exportSettings [{imageType PNG/SVG, constraint {type CONTENT_SCALE, value}, suffix, contentsOnly, colorProfile, svg*}]` | confirmed (JPEG/PDF, `CONTENT_WIDTH/HEIGHT` unconfirmed) |
| Dev status / annotations | `sectionStatusInfo {status, prevStatus, lastUpdateUnixTimestamp}` exists on symbols; not mapped. No `devStatus` or annotation fields appear | unconfirmed |
| Plugin / edit metadata | `pluginData [{pluginID, key, value}]`, `editInfo {createdAt, lastEditedAt, userId}` | confirmed, ignored |

## Paints, effects, geometry

| Feature | Fields | Status |
|---|---|---|
| Paints | `fillPaints` entries: `type` SOLID / GRADIENT_LINEAR / GRADIENT_RADIAL / IMAGE, `color {r,g,b,a}`, `opacity`, `visible`, `blendMode`, `authoredColor`, `colorVar` | confirmed. ANGULAR / DIAMOND gradients unconfirmed |
| Gradient | `stops [{color, position}]` (`stopsVar` adds `colorVar`), `transform {m00..m12}` | confirmed |
| Image | `image {hash, name}`, `imageScaleMode` FILL / STRETCH; hashes match `images/` entries of the zip | confirmed (FIT / TILE unconfirmed) |
| Effects | `effects [{type, color, offset, radius, spread, showShadowBehindNode, blendMode, visible, colorVar}]`: DROP_SHADOW, INNER_SHADOW | confirmed. LAYER_BLUR / BACKGROUND_BLUR (`FOREGROUND_BLUR`) and NOISE / TEXTURE / GLASS names and their parameters are **unconfirmed** |
| Geometry | `fillGeometry` / `strokeGeometry [{commandsBlob, styleID, windingRule}]`, blobs index `message.blobs` | confirmed (`NONZERO` only) |
| Vectors | `vectorData {vectorNetworkBlob, normalizedSize, styleOverrideTable [{styleID, ...}]}` | confirmed |
| Text outlines | `derivedTextData {baselines, glyphs [{commandsBlob, position, fontSize}], fontMetaData, layoutSize, ...}` (unused: the renderer lays text out itself) | confirmed |

## Text

| Feature | Fields | Status |
|---|---|---|
| Content | `textData {characters, lines [{lineType, indentationLevel, isFirstLineOfList, listStartOffset, sourceDirectionality, styleId}]}` | confirmed |
| Line types | `lineType` `PLAIN`, `UNORDERED_LIST` (`ORDERED_LIST` unconfirmed) | partly confirmed |
| Runs | `textData.characterStyleIDs`, `textData.styleOverrideTable [{styleID, fontName, fontSize, styleIdForFill, fontVariant*}]` | confirmed |
| Font | `fontName {family, style, postscript}`, `fontSize`, `fontVersion`, `fontVariations` | confirmed |
| Linked text style | `styleIdForText` resolves through the shared style node before character overrides. Node-level font values can retain stale pre-link values (48px while the linked style and derived glyphs use 18px). Preserve node paints and alignment; resolve percentage run tracking against the linked font size. | confirmed |
| Line height | `lineHeight {value, units}`: `PIXELS` (almost all) and `PERCENT`; `RAW` unconfirmed | confirmed |
| Letter spacing | `letterSpacing {value, units}`: always `PERCENT` here; `textTracking` is a redundant float fraction | confirmed |
| Alignment | `textAlignHorizontal` (absent = left), `textAlignVertical`, `textAutoResize` WIDTH_AND_HEIGHT / HEIGHT (`NONE`, `TRUNCATE` unconfirmed) | confirmed |
| Decoration | `textDecoration` UNDERLINE; `textCase` not present | partly confirmed |
| OpenType | `toggledOffOTFeatures ["LIGA"]` (and the guessed `toggledOnOTFeatures`); `fontVariantCommonLigatures` / `fontVariantContextualLigatures` are booleans, `fontVariantNumericFigure` / `Spacing` / `Fraction` / `Caps` are enums with value `NORMAL` | confirmed. Non-default enum values unconfirmed |
| Other | `leadingTrim` `CAP_HEIGHT`, `paragraphSpacing`, `hyperlink {url}`, `emojiImageSet`, `textBidiVersion`, `autoRename`, `textTruncation` / `maxLines` | `leadingTrim`, `paragraphSpacing`, `hyperlink` confirmed; truncation unconfirmed |

## Components and instances

| Feature | Fields | Status |
|---|---|---|
| Component | `SYMBOL` node; `componentKey`, `sourceLibraryKey` (`lk-...`, one per library), `overrideKey`, `publishID`, `isSymbolPublishable`, `symbolDescription` | confirmed |
| Component set | `FRAME` with `isStateGroup: true`, `stateGroupPropertyValueOrders [{property, values}]`; variants carry `variantPropSpecs [{propDefId, value}]` | confirmed |
| Instance | `symbolData {symbolID, symbolOverrides [...], uniformScaleFactor}`, `derivedSymbolData [...]`, `derivedSymbolDataLayoutVersion`, `propsAreBubbled`. **Instances never store children** | confirmed |
| Override paths | `guidPath {guids [..]}` holds `overrideKey` values, not node guids (1 to 3 entries; nested instances take two or more) | confirmed |
| Override content | any NodeChange fields: `size`, `transform`, `fillPaints`, `strokePaints`, `strokeWeight`, `textData`, `fontName`, `styleIdForFill` (`{assetRef}`), `fillGeometry`, `strokeGeometry`, ... | confirmed |
| Derived data | per path: `size`, `transform`, `fillGeometry`, `strokeGeometry` | confirmed |
| Scale | `uniformScaleFactor` != 1 on instances made with the scale tool: copied contents scale by it | confirmed |
| Property defs | `componentPropDefs [{id, name, type BOOL/TEXT/VARIANT/INSTANCE_SWAP, initialValue {boolValue / textValue{characters} / guidValue}, varValue, preferredValues, sortPosition}]` on the set; variants carry bare `{id, parentPropDefId}` aliases | confirmed |
| Property values | instance `componentPropAssignments [{defID, value {boolValue / textValue / guidValue}, varValue}]` | confirmed |
| Property refs | node `componentPropRefs [{defID, componentPropNodeField VISIBLE / TEXT_DATA / OVERRIDDEN_SYMBOL_ID}]`, with `parameterConsumptionMap` repeating them. All defIDs go through `parentPropDefId` | confirmed |
| Detached | `detachedSymbolId {assetRef}` | confirmed, ignored |

## Styles and libraries

| Feature | Fields | Status |
|---|---|---|
| Style nodes | any node with `styleType` FILL / TEXT / EFFECT (GRID unconfirmed), `key` (library key, absent in local files), `version`, `styleDescription`, `sortPosition`, `isSoftDeleted` | confirmed. The guessed `sharedStyleMasterData.styleKey` does not exist; `key` replaces it |
| Style refs | `styleIdForFill`, `styleIdForStrokeFill`, `styleIdForText`, `styleIdForEffect`: `{assetRef: {key, version}}` for library styles, `{guid}` for styles of the same file (`styleIdForGrid` unconfirmed) | confirmed |
| Libraries | `{assetRef: {key, version}}` is how every library asset is referenced; `key` equals the target node's `key` | confirmed |

## Variables

| Feature | Fields | Status |
|---|---|---|
| Sets | `VARIABLE_SET`: `name`, `key`, `variableSetModes [{id, name, sortPosition}]` | confirmed |
| Variables | `VARIABLE`: `name`, `key`, `variableSetID` (`{assetRef: {key}}`, or `{guid}` when local), `variableResolvedType` COLOR / FLOAT / STRING (BOOLEAN unconfirmed), `variableScopes`, `variableDataValues {entries [{modeID, variableData {dataType, resolvedDataType, value}}]}`, `sortPosition` | confirmed |
| Values | `value`: `colorValue {r,g,b,a}`, `floatValue`, `textValue` (string), `alias {assetRef {key}}` (a local `{guid}` alias is unconfirmed) | confirmed |
| Bindings | node `variableConsumptionMap {entries [{variableField, variableData}]}` with fields such as `STACK_PADDING_*`, `STACK_SPACING`, `RECTANGLE_*_CORNER_RADIUS`, `CORNER_RADIUS`, `WIDTH`, `HEIGHT`, `FONT_SIZE`, `FONT_FAMILY`, `FONT_STYLE`; paint, stop and effect colors use `colorVar` | confirmed |
| Modes | node `variableModeBySetMap {entries [{variableModeID, variableSetID}]}` | confirmed (empty in most nodes) |

## Prototypes

`prototypeInteractions [{id, isDeleted, event {interactionType ON_CLICK / ON_HOVER / MOUSE_IN / MOUSE_OUT}, actions [{connectionType INTERNAL_NODE, navigationType SWAP_STATE, transitionNodeID, transitionType DISSOLVE, transitionDuration, easingType, easingFunction}]}]`: confirmed. URL / overlay / scroll fields (`connectionURL`, `overlay*`, `overflowDirection`, `flowStartingPoint`) are unconfirmed.
