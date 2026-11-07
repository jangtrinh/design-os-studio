//! What each key of an instance's own entry (the `symbolOverrides` entry whose path is the instance's main component
//! alone) says about the instance, in one table that the loader and the `.fig` writer both read.
//!
//! The loader flags the fields the entry states when a file is opened; a save restates a flagged field from the node when
//! the user edited it, and removes the keys that stated it from the record. Both must agree on which key states which
//! field, so neither keeps a list of its own: a key one of them knew and the other did not was a field that was flagged
//! but not restated, or restated but never flagged. Every key of the writer schema's `NodeChange` is named below; a test
//! fails when the schema gains a key the table does not name, or the table names a key the schema does not have.

use designos_doc::OverrideField;

/// What a key of an instance's own entry means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnKey {
    /// The key states this field of the instance. The field is flagged on open, and a save that finds it edited writes it
    /// from the node and removes this key from the record.
    States(OverrideField),
    /// A baked outline of this field (`fillGeometry`, `strokeGeometry`). It is no statement of its own, so it never flags
    /// the field, but a save that restates the field removes it: an outline of the old shape must not outlive the edit.
    Derived(OverrideField),
    /// The entry may carry the key, but no save restates it: it is written as the file recorded it.
    LeftAsRecorded,
    /// The key names the node or belongs to another kind of node; an own entry does not carry it.
    NotInOwnEntry,
}

/// The class of `key` if the table names it. A name is matched whole: `bordersTakeSpace` is a layout key although
/// `borderTopWeight` is a stroke key.
fn named_class(key: &str) -> Option<OwnKey> {
    use OwnKey::*;
    Some(match key {
        "name" => States(OverrideField::Name),
        "visible" => States(OverrideField::Visible),
        "locked" => States(OverrideField::Locked),
        "proportionsConstrained" => States(OverrideField::LockAspect),
        "opacity" => States(OverrideField::Opacity),
        "blendMode" => States(OverrideField::Blend),
        "size" => States(OverrideField::Size),
        "transform" => States(OverrideField::Transform),
        "fillPaints" | "styleIdForFill" => States(OverrideField::Fills),
        "strokePaints"
        | "strokeWeight"
        | "strokeAlign"
        | "strokeJoin"
        | "strokeCap"
        | "dashPattern"
        | "miterLimit"
        | "borderStrokeWeightsIndependent"
        | "borderTopWeight"
        | "borderRightWeight"
        | "borderBottomWeight"
        | "borderLeftWeight"
        | "styleIdForStrokeFill" => States(OverrideField::Stroke),
        "effects" | "styleIdForEffect" | "inheritEffectStyleID" => States(OverrideField::Effects),
        "cornerRadius"
        | "rectangleCornerRadiiIndependent"
        | "rectangleTopLeftCornerRadius"
        | "rectangleTopRightCornerRadius"
        | "rectangleBottomLeftCornerRadius"
        | "rectangleBottomRightCornerRadius" => States(OverrideField::CornerRadii),
        "strokeGeometry" => Derived(OverrideField::Stroke),
        "fillGeometry" => Derived(OverrideField::CornerRadii),
        // Names the node, ties it to the library or file, or belongs to another kind of node.
        "guid"
        | "phase"
        | "parentIndex"
        | "type"
        | "isPublishable"
        | "description"
        | "key"
        | "styleID"
        | "styleType"
        | "styleDescription"
        | "version"
        | "userFacingVersion"
        | "sortPosition"
        | "isSoftDeletedStyle"
        | "isStateGroup"
        | "stateGroupPropertyValueOrders"
        | "isSymbolPublishable"
        | "sharedSymbolVersion"
        | "symbolDescription"
        | "sourceLibraryKey"
        | "publishID"
        | "componentKey"
        | "publishedVersion"
        | "componentPropDefs"
        | "componentPropRefs"
        | "variantPropSpecs"
        | "symbolData"
        | "derivedSymbolData"
        | "sectionStatusInfo"
        | "variableSetModes"
        | "variableSetID"
        | "variableResolvedType"
        | "variableDataValues"
        | "variableScopes"
        | "codeSyntax"
        | "isCollectionExtendable"
        | "isSoftDeleted"
        | "editInfo"
        | "handoffStatusMap"
        | "documentColorProfile"
        | "widgetSyncedState"
        | "widgetCachedAncestor"
        | "widgetInputBehavior"
        | "widgetHoverStyle"
        | "isWidgetStickable"
        | "shouldHideCursorsOnWidgetHover"
        | "widgetMetadata"
        | "widgetEvents"
        | "widgetPropertyMenuItems"
        | "tableRowPositions"
        | "tableColumnPositions"
        | "tableRowHeights"
        | "tableColumnWidths"
        | "tableMergedCells"
        | "slideSpeakerNotes"
        | "isSkippedSlide"
        | "slideNumber"
        | "slideNumberSeparator"
        | "isPageDivider"
        | "connectorStart"
        | "connectorEnd"
        | "connectorLineStyle"
        | "connectorStartCap"
        | "connectorEndCap"
        | "connectorControlPoints"
        | "connectorTextMidpoint"
        | "connectorType"
        | "connectorVersion"
        | "annotationCategories"
        | "shapeWithTextType"
        | "shapeUserHeight"
        | "shapeTruncates"
        | "codeExportName"
        | "codeObjectType"
        | "codeSourceInfo"
        | "textElementDelayBehaviour"
        | "timelinePosition"
        | "keyframeValue"
        | "interpolationType"
        | "bezierHandles"
        | "easingData"
        | "interpolationColorSpace"
        | "keyframeOperation"
        | "trackLoopMode"
        | "timelineDuration"
        | "timelineOffset"
        | "timelineDisabled"
        | "timelineDefinitions"
        | "timelineAssignments"
        | "animationPresets"
        | "styleIdsForAnimation"
        | "sourceControlConfig"
        | "makeContentState"
        | "easingType"
        | "objectAnimations"
        | "prototypeStartingPoint"
        | "pluginRelaunchData"
        | "ancestorPathBeforeDeletion"
        | "symbolLinks"
        | "renderedSyncedState"
        | "derivedImmutableFrameData"
        | "nodeGenerationData"
        | "internalOnly" => NotInOwnEntry,
        // Kept as the record states it; no save restates them.
        "inheritGridStyleID"
        | "styleIdForText"
        | "styleIdForGrid"
        | "overriddenSymbolID"
        | "componentPropAssignments"
        | "propsAreBubbled"
        | "guidPath"
        | "overrideLevel"
        | "isSlot"
        | "isSlotContent"
        | "fontSize"
        | "paragraphIndent"
        | "paragraphSpacing"
        | "textAlignHorizontal"
        | "textAlignVertical"
        | "textCase"
        | "textDecoration"
        | "isOverrideOverTextStyle"
        | "semanticWeight"
        | "semanticItalic"
        | "lineHeight"
        | "fontName"
        | "textData"
        | "derivedTextData"
        | "fontVariantCommonLigatures"
        | "fontVariantContextualLigatures"
        | "fontVariantNumericFigure"
        | "fontVariantNumericSpacing"
        | "fontVariantNumericFraction"
        | "fontVariantCaps"
        | "fontVariantPosition"
        | "letterSpacing"
        | "leadingTrim"
        | "hangingPunctuation"
        | "hangingList"
        | "maxLines"
        | "toggledOnOTFeatures"
        | "toggledOffOTFeatures"
        | "hyperlink"
        | "fontVariations"
        | "textTruncation"
        | "textWrapStyle"
        | "lockMode"
        | "mask"
        | "maskType"
        | "backgroundOpacity"
        | "backgroundColor"
        | "textDecorationSkipInk"
        | "transformModifiers"
        | "strokeData"
        | "targetAspectRatio"
        | "bordersTakeSpace"
        | "horizontalConstraint"
        | "stackMode"
        | "stackSpacing"
        | "stackPadding"
        | "stackHorizontalPadding"
        | "stackVerticalPadding"
        | "stackPrimarySizing"
        | "stackPrimaryAlignItems"
        | "stackCounterAlignItems"
        | "stackChildPrimaryGrow"
        | "stackPaddingRight"
        | "stackPaddingBottom"
        | "stackChildAlignSelf"
        | "stackPositioning"
        | "stackReverseZIndex"
        | "stackWrap"
        | "stackCounterSpacing"
        | "minSize"
        | "maxSize"
        | "stackCounterAlignContent"
        | "stackLayoutVersion"
        | "stackChildMarginTop"
        | "stackChildMarginRight"
        | "stackChildMarginBottom"
        | "stackChildMarginLeft"
        | "gridRows"
        | "gridColumns"
        | "gridRowGap"
        | "gridColumnGap"
        | "gridRowAnchor"
        | "gridColumnAnchor"
        | "gridRowSpan"
        | "gridColumnSpan"
        | "gridColumnsSizing"
        | "gridRowsSizing"
        | "gridChildVerticalAlign"
        | "gridChildHorizontalAlign"
        | "gridAutoTracks"
        | "gridReflowEnabled"
        | "pluginData"
        | "autoRename"
        | "backgroundEnabled"
        | "textTracking"
        | "verticalConstraint"
        | "exportSettings"
        | "textAutoResize"
        | "layoutGrids"
        | "frameMaskDisabled"
        | "resizeToFit"
        | "booleanOperation"
        | "handleMirroring"
        | "count"
        | "cornerSmoothing"
        | "starInnerScale"
        | "arcData"
        | "vectorData"
        | "vectorOperationVersion"
        | "textPathStart"
        | "guides"
        | "scrollDirection"
        | "overrideKey"
        | "stackCounterSizing"
        | "listSpacing"
        | "variableConsumptionMap"
        | "variableModeBySetMap"
        | "parameterConsumptionMap"
        | "strokeBrushGuid"
        | "strokeSeed"
        | "variableWidthPoints"
        | "dynamicStrokeSettings"
        | "scatterStrokeSettings"
        | "stretchStrokeSettings"
        | "scatterBrushTransforms"
        | "brushType"
        | "pathTrim"
        | "strokeOffset"
        | "detachedSymbolId" => LeftAsRecorded,
        _ => return None,
    })
}

/// The class of `key`. A key the table does not name (a later file format) is left as recorded: never flagged and never
/// removed from the record.
pub(crate) fn classify_own_key(key: &str) -> OwnKey {
    named_class(key).unwrap_or(OwnKey::LeftAsRecorded)
}

/// The field `key` states, if it states one.
pub(crate) fn field_stated_by(key: &str) -> Option<OverrideField> {
    match classify_own_key(key) {
        OwnKey::States(field) => Some(field),
        OwnKey::Derived(_) | OwnKey::LeftAsRecorded | OwnKey::NotInOwnEntry => None,
    }
}

/// The field whose restate removes `key` from the record: the field it states, or the one its baked outline derives from.
pub(crate) fn field_removed_with(key: &str) -> Option<OverrideField> {
    match classify_own_key(key) {
        OwnKey::States(field) | OwnKey::Derived(field) => Some(field),
        OwnKey::LeftAsRecorded | OwnKey::NotInOwnEntry => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_self_digest::SELF_FIELDS;
    use crate::testkit::figma_like_schema;

    fn node_change_keys() -> Vec<String> {
        let schema = figma_like_schema();
        let def = schema.find("NodeChange").expect("NodeChange in the writer schema");
        schema.defs[def].fields.iter().map(|f| f.name.to_string()).collect()
    }

    #[test]
    fn every_key_of_the_writer_schema_is_named_by_the_table() {
        let unnamed: Vec<String> = node_change_keys().into_iter().filter(|k| named_class(k).is_none()).collect();
        assert!(unnamed.is_empty(), "classify these NodeChange keys in own_entry_keys.rs: {unnamed:?}");
    }

    #[test]
    fn every_key_the_table_names_is_a_key_of_the_writer_schema() {
        let schema_keys = node_change_keys();
        // The table's source text is its own list: any name the match arms hold must be a schema key.
        let source = include_str!("own_entry_keys.rs");
        let table = &source[source.find("fn named_class").expect("table")..source.find("_ => return None").expect("end of table")];
        let named: Vec<&str> = table.split('"').skip(1).step_by(2).collect();
        assert!(named.len() > 200, "the table was not read: {} names", named.len());
        let unknown: Vec<&&str> = named.iter().filter(|k| !schema_keys.iter().any(|s| s == **k)).collect();
        assert!(unknown.is_empty(), "not keys of NodeChange: {unknown:?}");
    }

    #[test]
    fn a_key_is_classified_once() {
        let source = include_str!("own_entry_keys.rs");
        let table = &source[source.find("fn named_class").expect("table")..source.find("_ => return None").expect("end of table")];
        let mut named: Vec<&str> = table.split('"').skip(1).step_by(2).collect();
        named.sort_unstable();
        let repeated: Vec<&&str> = named.windows(2).filter(|w| w[0] == w[1]).map(|w| &w[0]).collect();
        assert!(repeated.is_empty(), "named twice: {repeated:?}");
    }

    #[test]
    fn every_field_a_save_restates_has_a_key_that_states_it() {
        for field in SELF_FIELDS {
            let keys: Vec<String> = node_change_keys().into_iter().filter(|k| field_stated_by(k) == Some(field)).collect();
            assert!(!keys.is_empty(), "no key states {field:?}");
        }
    }

    #[test]
    fn a_layout_key_that_shares_a_prefix_with_a_stroke_key_is_not_a_stroke_key() {
        assert_eq!(classify_own_key("bordersTakeSpace"), OwnKey::LeftAsRecorded);
        assert_eq!(classify_own_key("borderTopWeight"), OwnKey::States(OverrideField::Stroke));
        assert_eq!(classify_own_key("strokeSeed"), OwnKey::LeftAsRecorded);
        assert_eq!(classify_own_key("rectangleCornerRadiiIndependent"), OwnKey::States(OverrideField::CornerRadii));
    }

    #[test]
    fn baked_outlines_and_smoothing_state_no_field_but_outlines_go_with_their_field() {
        for key in ["fillGeometry", "strokeGeometry", "cornerSmoothing"] {
            assert_eq!(field_stated_by(key), None, "{key} must not flag a field");
        }
        assert_eq!(field_removed_with("fillGeometry"), Some(OverrideField::CornerRadii));
        assert_eq!(field_removed_with("strokeGeometry"), Some(OverrideField::Stroke));
        assert_eq!(field_removed_with("cornerSmoothing"), None);
    }

    #[test]
    fn a_style_link_that_no_schema_key_has_is_not_in_the_table() {
        for gone in ["inheritFillStyleID", "inheritStrokeStyleID"] {
            assert!(named_class(gone).is_none(), "{gone} is not a NodeChange key");
        }
    }
}
