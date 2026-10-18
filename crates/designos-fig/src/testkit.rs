//! Test support: a Figma-like kiwi schema and a synthetic message, so `.fig`
//! files can be synthesized without any real Figma file. Not part of the
//! stable API.

use crate::container::{self, Compression};
use crate::kiwi::{DefKind, FieldType, KiwiValue as V, Native, Schema, SchemaBuilder};
use std::collections::BTreeMap;

fn enum_def(b: &mut SchemaBuilder, name: &str, variants: &[&str]) {
    let d = b.define(name, DefKind::Enum);
    for (i, v) in variants.iter().enumerate() {
        b.variant(d, v, i as u32);
    }
}

/// A reduced version of Figma's schema (same names for the fields we read and write).
pub fn figma_like_schema() -> Schema {
    use Native::*;
    let mut b = SchemaBuilder::new();
    // Enum index 0 must exist: enum fields reference definition 0 in the binary form.
    enum_def(&mut b, "Phase", &["CREATED", "REMOVED"]);
    enum_def(&mut b, "MessageType", &["JOIN_START", "NODE_CHANGES", "USER_CHANGES", "JOIN_END", "SIGNAL", "STYLE", "BROADCAST"]);
    enum_def(
        &mut b,
        "NodeType",
        &[
            "NONE",
            "DOCUMENT",
            "CANVAS",
            "GROUP",
            "FRAME",
            "BOOLEAN_OPERATION",
            "VECTOR",
            "STAR",
            "LINE",
            "ELLIPSE",
            "RECTANGLE",
            "REGULAR_POLYGON",
            "ROUNDED_RECTANGLE",
            "TEXT",
            "SLICE",
            "SYMBOL",
            "INSTANCE",
            "STICKY",
            "SHAPE_WITH_TEXT",
            "CONNECTOR",
            "CODE_BLOCK",
            "WIDGET",
            "STAMP",
            "MEDIA",
            "HIGHLIGHT",
            "SECTION",
            "SECTION_OVERLAY",
            "WASHI_TAPE",
            "VARIABLE",
            "VARIABLE_SET",
            "ASSISTED_LAYOUT",
            "TABLE",
            "TABLE_CELL",
            "VARIABLE_OVERRIDE",
            "SLIDE",
            "SLIDE_ROW",
            "SLIDE_GRID",
            "MODULE",
            "INTERACTIVE_SLIDE_ELEMENT",
            "RESPONSIVE_SET",
            "CODE_COMPONENT",
            "CODE_INSTANCE",
            "CODE_LIBRARY",
            "CODE_FILE",
            "CODE_LAYER",
            "BRUSH",
            "MANAGED_STRING",
            "TRANSFORM",
            "CMS_RICH_TEXT",
            "REPEATER",
            "JSX",
            "EMBEDDED_PROTOTYPE",
            "REACT_FIBER",
            "RESPONSIVE_NODE_SET",
            "WEBPAGE",
            "KEYFRAME",
            "ANIMATION_PRESET_INSTANCE",
            "TEXT_PATH",
            "VECTOR_OPERATION",
            "KEYFRAME_TRACK",
            "DESIGN_TO_REACT",
            "SLIDE_THEME_MAP",
            "MAKE_CHAT",
            "AI_EDIT",
            "CANVAS_ROW",
            "MULTI_REV_MIGRATION",
        ],
    );
    enum_def(&mut b, "PaintType", &["SOLID", "GRADIENT_LINEAR", "GRADIENT_RADIAL", "GRADIENT_ANGULAR", "GRADIENT_DIAMOND", "IMAGE"]);
    enum_def(&mut b, "StrokeAlign", &["CENTER", "INSIDE", "OUTSIDE", "OFFSET"]);
    enum_def(&mut b, "StrokeJoin", &["MITER", "BEVEL", "ROUND"]);
    enum_def(&mut b, "StrokeCap", &["NONE", "ROUND", "SQUARE", "ARROW_LINES", "ARROW_EQUILATERAL"]);
    enum_def(
        &mut b,
        "EffectType",
        &["INNER_SHADOW", "DROP_SHADOW", "FOREGROUND_BLUR", "BACKGROUND_BLUR", "REPEAT", "SYMMETRY", "GRAIN", "NOISE", "GLASS", "CUSTOM", "TEXTURE"],
    );
    enum_def(&mut b, "NoiseType", &["MULTITONE", "MONOTONE", "DUOTONE"]);
    enum_def(&mut b, "StackMode", &["NONE", "HORIZONTAL", "VERTICAL", "GRID"]);
    enum_def(&mut b, "StackAlign", &["MIN", "CENTER", "MAX", "SPACE_BETWEEN", "BASELINE", "SPACE_EVENLY"]);
    // Figma's own values: SPACE_EVENLY (3) is the legacy spacing, SPACE_EVENLY_CSS (6) the CSS-aligned one.
    let justify = b.define("StackJustify", DefKind::Enum);
    for (name, value) in [("MIN", 0), ("CENTER", 1), ("MAX", 2), ("SPACE_EVENLY", 3), ("SPACE_BETWEEN", 4), ("SPACE_AROUND", 5), ("SPACE_EVENLY_CSS", 6)] {
        b.variant(justify, name, value);
    }
    enum_def(&mut b, "StackCounterAlignContent", &["AUTO", "SPACE_BETWEEN"]);
    enum_def(&mut b, "TextWrapStyle", &["AUTO", "BALANCE", "PRETTY"]);
    enum_def(&mut b, "StackSize", &["FIXED", "RESIZE_TO_FIT", "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE"]);
    enum_def(&mut b, "StackCounterAlign", &["MIN", "CENTER", "MAX", "STRETCH", "AUTO", "BASELINE"]);
    enum_def(&mut b, "StackPositioning", &["AUTO", "ABSOLUTE"]);
    enum_def(&mut b, "StackWrap", &["NO_WRAP", "WRAP"]);
    enum_def(&mut b, "TextAlignHorizontal", &["LEFT", "CENTER", "RIGHT", "JUSTIFIED"]);
    enum_def(&mut b, "TextAlignVertical", &["TOP", "CENTER", "BOTTOM"]);
    enum_def(&mut b, "TextAutoResize", &["NONE", "WIDTH_AND_HEIGHT", "HEIGHT", "TRUNCATE"]);
    enum_def(&mut b, "TextCase", &["ORIGINAL", "UPPER", "LOWER", "TITLE", "SMALL_CAPS", "SMALL_CAPS_FORCED"]);
    enum_def(&mut b, "TextDecoration", &["NONE", "UNDERLINE", "STRIKETHROUGH"]);
    enum_def(&mut b, "NumberUnits", &["RAW", "PIXELS", "PERCENT"]);
    enum_def(
        &mut b,
        "BlendMode",
        &[
            "PASS_THROUGH",
            "NORMAL",
            "DARKEN",
            "MULTIPLY",
            "LINEAR_BURN",
            "COLOR_BURN",
            "LIGHTEN",
            "SCREEN",
            "LINEAR_DODGE",
            "COLOR_DODGE",
            "OVERLAY",
            "SOFT_LIGHT",
            "HARD_LIGHT",
            "DIFFERENCE",
            "EXCLUSION",
            "HUE",
            "SATURATION",
            "COLOR",
            "LUMINOSITY",
        ],
    );
    enum_def(&mut b, "TextTruncation", &["DISABLED", "ENDING"]);
    enum_def(&mut b, "StyleType", &["NONE", "FILL", "TEXT", "EFFECT", "EXPORT", "GRID", "STROKE"]);
    enum_def(&mut b, "GridPattern", &["GRID", "COLUMNS", "ROWS"]);
    enum_def(&mut b, "ImageScaleMode", &["STRETCH", "FIT", "FILL", "TILE"]);
    enum_def(&mut b, "WindingRule", &["NONZERO", "ODD", "EVENODD"]);
    enum_def(&mut b, "ConstraintType", &["MIN", "CENTER", "MAX", "STRETCH", "SCALE"]);
    enum_def(&mut b, "BooleanOperation", &["UNION", "INTERSECT", "SUBTRACT", "XOR", "EXCLUDE"]);
    enum_def(&mut b, "ExportImageType", &["PNG", "JPEG", "SVG", "PDF"]);
    enum_def(&mut b, "ExportConstraintType", &["CONTENT_SCALE", "CONTENT_WIDTH", "CONTENT_HEIGHT", "SCALE", "WIDTH", "HEIGHT"]);
    enum_def(&mut b, "GuideAxis", &["X", "Y"]);
    enum_def(&mut b, "VariableResolvedDataType", &["BOOLEAN", "FLOAT", "STRING", "COLOR", "MAP", "SYMBOL_ID", "FONT_STYLE"]);
    enum_def(&mut b, "VariableDataType", &["BOOLEAN", "FLOAT", "STRING", "COLOR", "ALIAS", "NODE_FIELD_ALIAS", "FONT_STYLE"]);
    enum_def(&mut b, "ComponentPropType", &["BOOL", "TEXT", "INSTANCE_SWAP", "VARIANT", "SLOT"]);
    enum_def(&mut b, "ComponentPropNodeField", &["VISIBLE", "TEXT_DATA", "OVERRIDDEN_SYMBOL_ID", "SLOT_CONTENT_ID"]);
    enum_def(&mut b, "HandoffStatusType", &["NONE", "BUILD", "READY_FOR_DEV", "COMPLETED"]);

    let nat = FieldType::Native;
    let def = |b: &SchemaBuilder, n: &str| FieldType::Def(b.index(n));

    let g = b.define("GUID", DefKind::Struct);
    b.field(g, "sessionID", nat(Uint), false, 1);
    b.field(g, "localID", nat(Uint), false, 2);
    let vec = b.define("Vector", DefKind::Struct);
    b.field(vec, "x", nat(Float), false, 1);
    b.field(vec, "y", nat(Float), false, 2);
    let mx = b.define("Matrix", DefKind::Struct);
    for (i, n) in ["m00", "m01", "m02", "m10", "m11", "m12"].iter().enumerate() {
        b.field(mx, n, nat(Float), false, i as u32 + 1);
    }
    let c = b.define("Color", DefKind::Struct);
    for (i, n) in ["r", "g", "b", "a"].iter().enumerate() {
        b.field(c, n, nat(Float), false, i as u32 + 1);
    }
    let pi = b.define("ParentIndex", DefKind::Struct);
    let gt = def(&b, "GUID");
    b.field(pi, "guid", gt, false, 1);
    b.field(pi, "position", nat(String), false, 2);
    let cs = b.define("ColorStop", DefKind::Struct);
    let ct = def(&b, "Color");
    b.field(cs, "color", ct, false, 1);
    b.field(cs, "position", nat(Float), false, 2);
    let img = b.define("Image", DefKind::Message);
    b.field(img, "hash", nat(Byte), true, 1);
    b.field(img, "name", nat(String), false, 2);
    let num = b.define("Number", DefKind::Struct);
    b.field(num, "value", nat(Float), false, 1);
    let nu = def(&b, "NumberUnits");
    b.field(num, "units", nu, false, 2);
    let fnm = b.define("FontName", DefKind::Struct);
    b.field(fnm, "family", nat(String), false, 1);
    b.field(fnm, "style", nat(String), false, 2);
    b.field(fnm, "postscript", nat(String), false, 3);
    let arc = b.define("ArcData", DefKind::Struct);
    b.field(arc, "startingAngle", nat(Float), false, 1);
    b.field(arc, "endingAngle", nat(Float), false, 2);
    b.field(arc, "innerRadius", nat(Float), false, 3);

    let aref = b.define("AssetRef", DefKind::Message);
    b.field(aref, "key", nat(String), false, 1);
    b.field(aref, "version", nat(String), false, 2);
    let sid = b.define("StyleId", DefKind::Message);
    let (gt, art) = (def(&b, "GUID"), def(&b, "AssetRef"));
    b.field(sid, "guid", gt, false, 1);
    b.field(sid, "assetRef", art, false, 2);

    // Variable data structures (defined before Paint/Effect so colorVar can reference VariableData).
    let vd = b.define("VariableData", DefKind::Message);
    let vdv = b.define("VariableDataValue", DefKind::Message);
    let fsv = b.define("FontStyleVarValue", DefKind::Message);
    b.field(fsv, "asFloat", FieldType::Def(vd), false, 1);
    b.field(fsv, "asString", FieldType::Def(vd), false, 2);
    let (ct, sidt) = (def(&b, "Color"), def(&b, "StyleId"));
    b.field(vdv, "boolValue", nat(Bool), false, 1);
    b.field(vdv, "floatValue", nat(Float), false, 2);
    b.field(vdv, "textValue", nat(String), false, 3);
    b.field(vdv, "colorValue", ct, false, 4);
    b.field(vdv, "alias", sidt, false, 5);
    b.field(vdv, "fontStyleValue", FieldType::Def(fsv), false, 6);
    let (vdt, vrdt) = (def(&b, "VariableDataType"), def(&b, "VariableResolvedDataType"));
    b.field(vd, "dataType", vdt, false, 1);
    b.field(vd, "resolvedDataType", vrdt, false, 2);
    b.field(vd, "value", FieldType::Def(vdv), false, 3);

    let vce = b.define("VariableConsumptionEntry", DefKind::Message);
    b.field(vce, "variableData", FieldType::Def(vd), false, 1);
    b.field(vce, "variableField", nat(String), false, 2);
    let vcm = b.define("VariableConsumptionMap", DefKind::Message);
    b.field(vcm, "entries", FieldType::Def(vce), true, 1);

    let vmse = b.define("VariableModeBySetEntry", DefKind::Message);
    let (sidt, gt) = (def(&b, "StyleId"), def(&b, "GUID"));
    b.field(vmse, "variableSetID", sidt, false, 1);
    b.field(vmse, "variableModeID", gt, false, 2);
    let vmsm = b.define("VariableModeBySetMap", DefKind::Message);
    b.field(vmsm, "entries", FieldType::Def(vmse), true, 1);

    let vsm = b.define("VariableSetMode", DefKind::Message);
    let gt = def(&b, "GUID");
    b.field(vsm, "id", gt, false, 1);
    b.field(vsm, "name", nat(String), false, 2);
    b.field(vsm, "sortPosition", nat(String), false, 3);

    let vdve = b.define("VariableDataValueEntry", DefKind::Message);
    let gt = def(&b, "GUID");
    b.field(vdve, "modeID", gt, false, 1);
    b.field(vdve, "variableData", FieldType::Def(vd), false, 2);
    let vdvs = b.define("VariableDataValues", DefKind::Message);
    b.field(vdvs, "entries", FieldType::Def(vdve), true, 1);

    enum_def(&mut b, "CodeSyntaxPlatform", &["WEB", "ANDROID", "iOS"]);
    let cse = b.define("CodeSyntaxMapEntry", DefKind::Message);
    let cspt = def(&b, "CodeSyntaxPlatform");
    b.field(cse, "platform", cspt, false, 1);
    b.field(cse, "value", nat(String), false, 2);
    let csm = b.define("CodeSyntaxMap", DefKind::Message);
    b.field(csm, "entries", FieldType::Def(cse), true, 1);

    let paint = b.define("Paint", DefKind::Message);
    let t = def(&b, "PaintType");
    b.field(paint, "type", t, false, 1);
    let ct = def(&b, "Color");
    b.field(paint, "color", ct, false, 2);
    b.field(paint, "opacity", nat(Float), false, 3);
    b.field(paint, "visible", nat(Bool), false, 4);
    let t = def(&b, "BlendMode");
    b.field(paint, "blendMode", t, false, 5);
    let cst = def(&b, "ColorStop");
    b.field(paint, "stops", cst, true, 6);
    let t = def(&b, "Matrix");
    b.field(paint, "transform", t, false, 7);
    let t = def(&b, "Image");
    b.field(paint, "image", t, false, 8);
    let t = def(&b, "ImageScaleMode");
    b.field(paint, "imageScaleMode", t, false, 9);
    b.field(paint, "colorVar", FieldType::Def(vd), false, 10);
    b.field(paint, "imageHash", nat(String), false, 11);
    // Figma's own field id (rea-findings: Paint.opacityVar = 38).
    b.field(paint, "opacityVar", FieldType::Def(vd), false, 38);

    let eff = b.define("Effect", DefKind::Message);
    let t = def(&b, "EffectType");
    b.field(eff, "type", t, false, 1);
    let ct = def(&b, "Color");
    b.field(eff, "color", ct, false, 2);
    let vt = def(&b, "Vector");
    b.field(eff, "offset", vt, false, 3);
    b.field(eff, "radius", nat(Float), false, 4);
    b.field(eff, "visible", nat(Bool), false, 5);
    let bm = def(&b, "BlendMode");
    b.field(eff, "blendMode", bm, false, 6);
    b.field(eff, "spread", nat(Float), false, 7);
    b.field(eff, "showShadowBehindNode", nat(Bool), false, 8);
    b.field(eff, "colorVar", FieldType::Def(vd), false, 10);
    b.field(eff, "noiseSize", vt, false, 22);
    b.field(eff, "clipToShape", nat(Bool), false, 23);
    b.field(eff, "secondaryColor", ct, false, 24);
    b.field(eff, "density", nat(Float), false, 25);
    let nt = def(&b, "NoiseType");
    b.field(eff, "noiseType", nt, false, 26);
    b.field(eff, "opacity", nat(Float), false, 27);
    b.field(eff, "seed", nat(Uint), false, 29);
    b.field(eff, "refractionRadius", nat(Float), false, 30);
    b.field(eff, "specularAngle", nat(Float), false, 31);
    b.field(eff, "specularIntensity", nat(Float), false, 32);
    b.field(eff, "bevelSize", nat(Float), false, 33);
    b.field(eff, "chromaticAberration", nat(Float), false, 34);
    b.field(eff, "reflectionDistance", nat(Float), false, 35);
    b.field(eff, "refractionIntensity", nat(Float), false, 36);
    // Figma's own field id (rea-findings: Effect.effectOpacityVar = 53).
    b.field(eff, "effectOpacityVar", FieldType::Def(vd), false, 53);

    let fg = b.define("FillGeometry", DefKind::Message);
    let t = def(&b, "WindingRule");
    b.field(fg, "windingRule", t, false, 1);
    b.field(fg, "commandsBlob", nat(Uint), false, 2);

    let lg = b.define("LayoutGrid", DefKind::Message);
    let t = def(&b, "GridPattern");
    b.field(lg, "pattern", t, false, 1);
    b.field(lg, "sectionSize", nat(Float), false, 2);
    b.field(lg, "gutterSize", nat(Float), false, 3);
    b.field(lg, "offset", nat(Float), false, 4);
    b.field(lg, "count", nat(Uint), false, 5);
    let ct = def(&b, "Color");
    b.field(lg, "color", ct, false, 6);
    b.field(lg, "visible", nat(Bool), false, 7);
    let gp = b.define("GuidPath", DefKind::Message);
    let gt = def(&b, "GUID");
    b.field(gp, "guids", gt, true, 1);

    let so = b.define("StyleOverride", DefKind::Message);
    b.field(so, "styleID", nat(Uint), false, 1);
    b.field(so, "fontSize", nat(Float), false, 2);
    let t = def(&b, "FontName");
    b.field(so, "fontName", t, false, 3);
    let t = def(&b, "Paint");
    b.field(so, "fillPaints", t, true, 4);
    let t = def(&b, "TextDecoration");
    b.field(so, "textDecoration", t, false, 5);
    let (tc, nm, sc, sj) = (def(&b, "TextCase"), def(&b, "Number"), def(&b, "StrokeCap"), def(&b, "StrokeJoin"));
    b.field(so, "textCase", tc, false, 6);
    b.field(so, "letterSpacing", nm, false, 7);
    b.field(so, "lineHeight", nm, false, 8);
    b.field(so, "strokeCap", sc, false, 9);
    b.field(so, "strokeJoin", sj, false, 10);
    b.field(so, "cornerRadius", nat(Float), false, 11);
    b.field(so, "handleMirroring", nat(String), false, 12);

    let vdata = b.define("VectorData", DefKind::Message);
    let (vt, sot) = (def(&b, "Vector"), def(&b, "StyleOverride"));
    b.field(vdata, "vectorNetworkBlob", nat(Uint), false, 1);
    b.field(vdata, "normalizedSize", vt, false, 2);
    b.field(vdata, "styleOverrideTable", sot, true, 3);

    let exc = b.define("ExportConstraint", DefKind::Message);
    let ect = def(&b, "ExportConstraintType");
    b.field(exc, "type", ect, false, 1);
    b.field(exc, "value", nat(Float), false, 2);
    let exs = b.define("ExportSettings", DefKind::Message);
    let (eit, exct) = (def(&b, "ExportImageType"), def(&b, "ExportConstraint"));
    b.field(exs, "suffix", nat(String), false, 1);
    b.field(exs, "imageType", eit, false, 2);
    b.field(exs, "constraint", exct, false, 3);

    let gd = b.define("Guide", DefKind::Message);
    let gat = def(&b, "GuideAxis");
    b.field(gd, "axis", gat, false, 1);
    b.field(gd, "offset", nat(Float), false, 2);

    let opt_sz = b.define("OptionalSize", DefKind::Message);
    b.field(opt_sz, "x", nat(Float), false, 1);
    b.field(opt_sz, "y", nat(Float), false, 2);

    let cptv = b.define("ComponentPropTextValue", DefKind::Message);
    b.field(cptv, "characters", nat(String), false, 1);
    let cpv = b.define("ComponentPropValue", DefKind::Message);
    let (gt, cptvt) = (def(&b, "GUID"), def(&b, "ComponentPropTextValue"));
    b.field(cpv, "boolValue", nat(Bool), false, 1);
    b.field(cpv, "textValue", cptvt, false, 2);
    b.field(cpv, "guidValue", gt, false, 3);
    let spc = b.define("SlotPropConfig", DefKind::Message);
    b.field(spc, "stretchChildOnInsert", nat(Bool), false, 1);
    b.field(spc, "displayByDefault", nat(Bool), false, 2);
    b.field(spc, "minChildren", nat(Uint), false, 3);
    b.field(spc, "maxChildren", nat(Uint), false, 4);
    b.field(spc, "allowPreferredValuesOnly", nat(Bool), false, 5);
    let cpd = b.define("ComponentPropDef", DefKind::Message);
    let (gt, cptt, cpvt, spct) = (def(&b, "GUID"), def(&b, "ComponentPropType"), def(&b, "ComponentPropValue"), def(&b, "SlotPropConfig"));
    b.field(cpd, "id", gt, false, 1);
    b.field(cpd, "name", nat(String), false, 2);
    b.field(cpd, "type", cptt, false, 3);
    b.field(cpd, "initialValue", cpvt, false, 4);
    b.field(cpd, "parentPropDefId", gt, false, 5);
    b.field(cpd, "preferredValues", gt, true, 6);
    b.field(cpd, "slotPropConfig", spct, false, 7);
    // Real schema: `SlotContentId { guid }`. Stored on a node under the non-kiwi `jsonSerialized`
    // bag, key `slot-content-id` (INFERENCE from the editor, not seen in a real file).
    let sci = b.define("SlotContentId", DefKind::Message);
    b.field(sci, "guid", gt, false, 1);
    let jsf = b.define("JsonSerializedFields", DefKind::Message);
    let scit = def(&b, "SlotContentId");
    b.field(jsf, "slot-content-id", scit, false, 1);
    let cpa = b.define("ComponentPropAssignment", DefKind::Message);
    b.field(cpa, "defID", gt, false, 1);
    b.field(cpa, "value", cpvt, false, 2);
    let cpr = b.define("ComponentPropRef", DefKind::Message);
    let cpnf = def(&b, "ComponentPropNodeField");
    b.field(cpr, "defID", gt, false, 1);
    b.field(cpr, "componentPropNodeField", cpnf, false, 2);
    let sgpvo = b.define("StateGroupPropertyValueOrder", DefKind::Message);
    b.field(sgpvo, "property", nat(String), false, 1);
    b.field(sgpvo, "values", nat(String), true, 2);

    let hs = b.define("HandoffStatus", DefKind::Message);
    let hst = def(&b, "HandoffStatusType");
    b.field(hs, "status", hst, false, 1);
    let hse = b.define("HandoffStatusEntry", DefKind::Message);
    let (gt, hsm_t) = (def(&b, "GUID"), def(&b, "HandoffStatus"));
    b.field(hse, "guid", gt, false, 1);
    b.field(hse, "handoffStatus", hsm_t, false, 2);
    let hsm = b.define("HandoffStatusMap", DefKind::Message);
    b.field(hsm, "entries", FieldType::Def(hse), true, 1);

    enum_def(&mut b, "LineType", &["PLAIN", "ORDERED_LIST", "UNORDERED_LIST"]);
    enum_def(&mut b, "Directionality", &["AUTO", "LTR", "RTL"]);
    enum_def(&mut b, "LeadingTrim", &["NONE", "CAP_HEIGHT"]);
    enum_def(
        &mut b,
        "OpenTypeFeature",
        &[
            "LIGA", "DLIG", "CLIG", "CALT", "KERN", "SS01", "SS02", "SS03", "SS04", "SS05", "SS06", "SS07", "SS08", "SS09", "SS10", "TNUM", "ONUM", "LNUM",
            "PNUM", "FRAC", "AFRC", "ZERO", "SMCP", "C2SC", "PCAP", "C2PC", "UNIC", "TITL", "ORDN", "SWSH", "SALT", "SUPS", "SUBS", "CASE",
        ],
    );
    enum_def(&mut b, "FontVariantNumericFigure", &["NORMAL", "LINING", "OLDSTYLE"]);
    enum_def(&mut b, "FontVariantNumericSpacing", &["NORMAL", "PROPORTIONAL", "TABULAR"]);
    enum_def(&mut b, "FontVariantNumericFraction", &["NORMAL", "DIAGONAL", "STACKED"]);
    enum_def(&mut b, "FontVariantCaps", &["NORMAL", "SMALL", "ALL_SMALL", "PETITE", "ALL_PETITE", "UNICASE", "TITLING"]);
    enum_def(&mut b, "FontVariantPosition", &["NORMAL", "SUB", "SUPER"]);
    enum_def(&mut b, "BrushType", &["PLAIN", "CHARCOAL", "WAX", "MARKER", "STRETCH", "SCATTER"]);
    let fv = b.define("FontVariation", DefKind::Message);
    b.field(fv, "axisTag", nat(Uint), false, 1);
    b.field(fv, "axisName", nat(String), false, 2);
    b.field(fv, "value", nat(Float), false, 3);
    let (otf_so, fvt_so, fvnf_so, fvns_so, fvnfr_so, fvc_so, fvp_so) = (
        def(&b, "OpenTypeFeature"),
        def(&b, "FontVariation"),
        def(&b, "FontVariantNumericFigure"),
        def(&b, "FontVariantNumericSpacing"),
        def(&b, "FontVariantNumericFraction"),
        def(&b, "FontVariantCaps"),
        def(&b, "FontVariantPosition"),
    );
    b.field(so, "toggledOnOTFeatures", otf_so, true, 13);
    b.field(so, "toggledOffOTFeatures", otf_so, true, 14);
    b.field(so, "fontVariations", fvt_so, true, 15);
    b.field(so, "fontVariantCommonLigatures", nat(Bool), false, 16);
    b.field(so, "fontVariantContextualLigatures", nat(Bool), false, 17);
    b.field(so, "textTracking", nat(Float), false, 18);
    b.field(so, "fontVariantNumericFigure", fvnf_so, false, 19);
    b.field(so, "fontVariantNumericSpacing", fvns_so, false, 20);
    b.field(so, "fontVariantNumericFraction", fvnfr_so, false, 21);
    b.field(so, "fontVariantCaps", fvc_so, false, 22);
    b.field(so, "fontVariantPosition", fvp_so, false, 23);
    let tl = b.define("TextLine", DefKind::Message);
    let t = def(&b, "LineType");
    b.field(tl, "lineType", t, false, 1);
    b.field(tl, "indentationLevel", nat(Uint), false, 2);
    let t = def(&b, "Directionality");
    b.field(tl, "sourceDirectionality", t, false, 3);
    let td = b.define("TextData", DefKind::Message);
    b.field(td, "characters", nat(String), false, 1);
    b.field(td, "characterStyleIDs", nat(Uint), true, 2);
    let t = def(&b, "StyleOverride");
    b.field(td, "styleOverrideTable", t, true, 3);
    let t = def(&b, "TextLine");
    b.field(td, "lines", t, true, 4);

    let nc = b.define("NodeChange", DefKind::Message);
    let mut id = 0;
    let mut f = |b: &mut SchemaBuilder, name: &str, ty: FieldType, arr: bool| {
        id += 1;
        b.field(nc, name, ty, arr, id);
    };
    let (gt, pit, nt, ph) = (def(&b, "GUID"), def(&b, "ParentIndex"), def(&b, "NodeType"), def(&b, "Phase"));
    f(&mut b, "guid", gt, false);
    f(&mut b, "phase", ph, false);
    f(&mut b, "parentIndex", pit, false);
    f(&mut b, "type", nt, false);
    f(&mut b, "name", nat(String), false);
    f(&mut b, "visible", nat(Bool), false);
    f(&mut b, "locked", nat(Bool), false);
    f(&mut b, "opacity", nat(Float), false);
    let (v, m, bm) = (def(&b, "Vector"), def(&b, "Matrix"), def(&b, "BlendMode"));
    f(&mut b, "size", v, false);
    f(&mut b, "transform", m, false);
    f(&mut b, "blendMode", bm, false);
    let (p, e) = (def(&b, "Paint"), def(&b, "Effect"));
    f(&mut b, "fillPaints", p, true);
    f(&mut b, "strokePaints", p, true);
    f(&mut b, "effects", e, true);
    f(&mut b, "strokeWeight", nat(Float), false);
    let (sa, sj, sc) = (def(&b, "StrokeAlign"), def(&b, "StrokeJoin"), def(&b, "StrokeCap"));
    f(&mut b, "strokeAlign", sa, false);
    f(&mut b, "strokeJoin", sj, false);
    f(&mut b, "strokeCap", sc, false);
    f(&mut b, "dashPattern", nat(Float), true);
    f(&mut b, "cornerRadius", nat(Float), false);
    f(&mut b, "cornerSmoothing", nat(Float), false);
    f(&mut b, "rectangleCornerRadiiIndependent", nat(Bool), false);
    for n in ["rectangleTopLeftCornerRadius", "rectangleTopRightCornerRadius", "rectangleBottomLeftCornerRadius", "rectangleBottomRightCornerRadius"] {
        f(&mut b, n, nat(Float), false);
    }
    f(&mut b, "mask", nat(Bool), false);
    f(&mut b, "frameMaskDisabled", nat(Bool), false);
    f(&mut b, "internalOnly", nat(Bool), false);
    f(&mut b, "isStateGroup", nat(Bool), false);
    let c = def(&b, "Color");
    f(&mut b, "backgroundColor", c, false);
    let (sm, al, sz) = (def(&b, "StackMode"), def(&b, "StackAlign"), def(&b, "StackSize"));
    f(&mut b, "stackMode", sm, false);
    f(&mut b, "stackSpacing", nat(Float), false);
    f(&mut b, "stackCounterSpacing", nat(Float), false);
    f(&mut b, "stackHorizontalPadding", nat(Float), false);
    f(&mut b, "stackVerticalPadding", nat(Float), false);
    f(&mut b, "stackPaddingRight", nat(Float), false);
    f(&mut b, "stackPaddingBottom", nat(Float), false);
    let (justify, content) = (def(&b, "StackJustify"), def(&b, "StackCounterAlignContent"));
    f(&mut b, "stackPrimaryAlignItems", justify, false);
    f(&mut b, "stackCounterAlignItems", al, false);
    f(&mut b, "stackCounterAlignContent", content, false);
    f(&mut b, "stackLayoutVersion", nat(Uint), false);
    f(&mut b, "stackPrimarySizing", sz, false);
    f(&mut b, "stackCounterSizing", sz, false);
    let (ca, sp) = (def(&b, "StackCounterAlign"), def(&b, "StackPositioning"));
    f(&mut b, "stackChildAlignSelf", ca, false);
    f(&mut b, "stackChildPrimaryGrow", nat(Float), false);
    f(&mut b, "stackPositioning", sp, false);
    let cn = def(&b, "ConstraintType");
    f(&mut b, "horizontalConstraint", cn, false);
    f(&mut b, "verticalConstraint", cn, false);
    let fgt = def(&b, "FillGeometry");
    f(&mut b, "fillGeometry", fgt, true);
    let ad = def(&b, "ArcData");
    f(&mut b, "arcData", ad, false);
    f(&mut b, "count", nat(Uint), false);
    f(&mut b, "starInnerScale", nat(Float), false);
    let bo = def(&b, "BooleanOperation");
    f(&mut b, "booleanOperation", bo, false);
    let sd = b.define("SymbolData", DefKind::Message);
    let gt = def(&b, "GUID");
    b.field(sd, "symbolID", gt, false, 1);
    let sdt = def(&b, "SymbolData");
    f(&mut b, "symbolData", sdt, false);
    let tdt = def(&b, "TextData");
    f(&mut b, "textData", tdt, false);
    let (fnt, nm) = (def(&b, "FontName"), def(&b, "Number"));
    f(&mut b, "fontName", fnt, false);
    f(&mut b, "fontSize", nat(Float), false);
    f(&mut b, "letterSpacing", nm, false);
    f(&mut b, "lineHeight", nm, false);
    let (ah, av, ar, tc, tdc) =
        (def(&b, "TextAlignHorizontal"), def(&b, "TextAlignVertical"), def(&b, "TextAutoResize"), def(&b, "TextCase"), def(&b, "TextDecoration"));
    f(&mut b, "textAlignHorizontal", ah, false);
    f(&mut b, "textAlignVertical", av, false);
    f(&mut b, "textAutoResize", ar, false);
    f(&mut b, "textCase", tc, false);
    f(&mut b, "textDecoration", tdc, false);
    let (lt, otf) = (def(&b, "LeadingTrim"), def(&b, "OpenTypeFeature"));
    f(&mut b, "leadingTrim", lt, false);
    f(&mut b, "paragraphIndent", nat(Float), false);
    f(&mut b, "toggledOnOTFeatures", otf, true);
    f(&mut b, "toggledOffOTFeatures", otf, true);
    let sidt = def(&b, "StyleId");
    f(&mut b, "styleIdForFill", sidt, false);
    for n in ["styleIdForStrokeFill", "styleIdForText", "styleIdForEffect", "styleIdForGrid"] {
        f(&mut b, n, sidt, false);
    }
    f(&mut b, "strokeGeometry", fgt, true);
    let lgt = def(&b, "LayoutGrid");
    f(&mut b, "layoutGrids", lgt, true);
    f(&mut b, "maxLines", nat(Uint), false);
    let (ttr, sty, gpt) = (def(&b, "TextTruncation"), def(&b, "StyleType"), def(&b, "GuidPath"));
    f(&mut b, "textTruncation", ttr, false);
    f(&mut b, "styleType", sty, false);
    f(&mut b, "styleDescription", nat(String), false);
    f(&mut b, "key", nat(String), false);
    f(&mut b, "guidPath", gpt, false);
    // Overrides are NodeChange-shaped messages.
    b.field(sd, "symbolOverrides", FieldType::Def(nc), true, 2);
    b.field(sd, "uniformScaleFactor", nat(Float), false, 3);
    f(&mut b, "derivedSymbolData", FieldType::Def(nc), true);
    // Additional NodeChange fields for 1:1 .fig export & import parity:
    let (gt, swt, oszt, exst, gdt, vdt_node, cpdt, cpat, cprt, sgpvot, vcmt, vmsmt, vsmt, vrdtt, vdtt, vdvt, hsmt, fvt) = (
        def(&b, "GUID"),
        def(&b, "StackWrap"),
        def(&b, "OptionalSize"),
        def(&b, "ExportSettings"),
        def(&b, "Guide"),
        def(&b, "VectorData"),
        def(&b, "ComponentPropDef"),
        def(&b, "ComponentPropAssignment"),
        def(&b, "ComponentPropRef"),
        def(&b, "StateGroupPropertyValueOrder"),
        def(&b, "VariableConsumptionMap"),
        def(&b, "VariableModeBySetMap"),
        def(&b, "VariableSetMode"),
        def(&b, "VariableResolvedDataType"),
        def(&b, "VariableDataType"),
        def(&b, "VariableDataValues"),
        def(&b, "HandoffStatusMap"),
        def(&b, "FontVariation"),
    );
    f(&mut b, "overrideKey", gt, false);
    f(&mut b, "overriddenSymbolID", gt, false);
    f(&mut b, "componentKey", nat(String), false);
    f(&mut b, "sourceLibraryKey", nat(String), false);
    f(&mut b, "strokeMiterAngle", nat(Float), false);
    f(&mut b, "miterLimit", nat(Float), false);
    f(&mut b, "borderStrokeWeightsIndependent", nat(Bool), false);
    for n in ["borderTopWeight", "borderRightWeight", "borderBottomWeight", "borderLeftWeight"] {
        f(&mut b, n, nat(Float), false);
    }
    f(&mut b, "stackPadding", nat(Float), false);
    f(&mut b, "stackPaddingLeft", nat(Float), false);
    f(&mut b, "stackPaddingTop", nat(Float), false);
    f(&mut b, "stackWrap", swt, false);
    f(&mut b, "bordersTakeSpace", nat(Bool), false);
    f(&mut b, "stackReverseZIndex", nat(Bool), false);
    f(&mut b, "minSize", oszt, false);
    f(&mut b, "maxSize", oszt, false);
    f(&mut b, "exportSettings", exst, true);
    f(&mut b, "guides", gdt, true);
    f(&mut b, "vectorData", vdt_node, false);
    f(&mut b, "paragraphSpacing", nat(Float), false);
    f(&mut b, "listSpacing", nat(Float), false);
    f(&mut b, "hangingPunctuation", nat(Bool), false);
    f(&mut b, "hangingList", nat(Bool), false);
    f(&mut b, "componentPropDefs", cpdt, true);
    f(&mut b, "componentPropAssignments", cpat, true);
    f(&mut b, "componentPropRefs", cprt, true);
    f(&mut b, "stateGroupPropertyValueOrders", sgpvot, true);
    f(&mut b, "variableConsumptionMap", vcmt, false);
    f(&mut b, "parameterConsumptionMap", vcmt, false);
    f(&mut b, "variableModeBySetMap", vmsmt, false);
    f(&mut b, "variableSetModes", vsmt, true);
    f(&mut b, "variableSetID", sidt, false);
    f(&mut b, "variableResolvedType", vrdtt, false);
    f(&mut b, "variableDataType", vdtt, false);
    f(&mut b, "variableScopes", nat(String), true);
    // Figma's own field id (rea-findings: NodeChange.codeSyntax = 358); the sequential ids above stay below it.
    let csmt = def(&b, "CodeSyntaxMap");
    b.field(nc, "codeSyntax", csmt, false, 358);
    f(&mut b, "variableDataValues", vdvt, false);
    f(&mut b, "sortPosition", nat(String), false);
    f(&mut b, "description", nat(String), false);
    f(&mut b, "handoffStatusMap", hsmt, false);
    f(&mut b, "fontVariations", fvt, true);
    let cep = b.define("ConnectorEndpoint", DefKind::Message);
    let (gt_cep, vt_cep) = (def(&b, "GUID"), def(&b, "Vector"));
    b.field(cep, "endpointNodeId", gt_cep, false, 1);
    b.field(cep, "position", vt_cep, false, 2);
    b.field(cep, "magnet", nat(String), false, 3);
    let hl_def = b.define("Hyperlink", DefKind::Message);
    b.field(hl_def, "url", nat(String), false, 1);
    b.field(hl_def, "guid", gt_cep, false, 2);
    let pd_def = b.define("PluginData", DefKind::Message);
    b.field(pd_def, "pluginID", nat(String), false, 1);
    b.field(pd_def, "value", nat(String), false, 2);
    b.field(pd_def, "key", nat(String), false, 3);
    let tar_def = b.define("TargetAspectRatio", DefKind::Message);
    b.field(tar_def, "value", vt_cep, false, 1);
    let (cept, hst_info, sc_cap, hlt, pdt, tart) = (
        def(&b, "ConnectorEndpoint"),
        def(&b, "HandoffStatus"),
        def(&b, "StrokeCap"),
        def(&b, "Hyperlink"),
        def(&b, "PluginData"),
        def(&b, "TargetAspectRatio"),
    );
    f(&mut b, "gridColumns", nat(Uint), false);
    f(&mut b, "gridRows", nat(Uint), false);
    f(&mut b, "gridColumnSpan", nat(Uint), false);
    f(&mut b, "gridRowSpan", nat(Uint), false);
    f(&mut b, "gridColumnAnchorIndex", nat(Uint), false);
    f(&mut b, "gridRowAnchorIndex", nat(Uint), false);
    f(&mut b, "gridColumnAnchor", nat(Uint), false);
    f(&mut b, "gridRowAnchor", nat(Uint), false);
    f(&mut b, "gridColumnGap", nat(Float), false);
    f(&mut b, "gridRowGap", nat(Float), false);
    f(&mut b, "gridColumnsSizing", nat(String), false);
    f(&mut b, "gridRowsSizing", nat(String), false);
    f(&mut b, "isSoftDeleted", nat(Bool), false);
    f(&mut b, "sectionStatusInfo", hst_info, false);
    f(&mut b, "authorVisible", nat(Bool), false);
    f(&mut b, "shapeWithTextType", nat(String), false);
    f(&mut b, "connectorStart", cept, false);
    f(&mut b, "connectorEnd", cept, false);
    f(&mut b, "connectorLineStyle", nat(String), false);
    f(&mut b, "connectorType", nat(String), false);
    f(&mut b, "connectorStartStrokeCap", sc_cap, false);
    f(&mut b, "connectorEndStrokeCap", sc_cap, false);
    f(&mut b, "proportionsConstrained", nat(Bool), false);
    f(&mut b, "targetAspectRatio", tart, false);
    f(&mut b, "hyperlink", hlt, false);
    f(&mut b, "pluginData", pdt, true);
    f(&mut b, "symbolDescription", nat(String), false);
    f(&mut b, "propsAreBubbled", nat(Bool), false);
    f(&mut b, "fontVariantCommonLigatures", nat(Bool), false);
    f(&mut b, "fontVariantContextualLigatures", nat(Bool), false);
    f(&mut b, "textTracking", nat(Float), false);

    let vps_def = b.define("VariantPropSpec", DefKind::Message);
    b.field(vps_def, "propDefId", gt_cep, false, 1);
    b.field(vps_def, "value", nat(String), false, 2);
    b.field(vps_def, "propertyName", nat(String), false, 3);
    b.field(vps_def, "name", nat(String), false, 4);
    let ei_def = b.define("EditInfo", DefKind::Message);
    b.field(ei_def, "timestampIso8601", nat(String), false, 1);
    b.field(ei_def, "userId", nat(String), false, 2);
    b.field(ei_def, "lastEditedAt", nat(Uint), false, 3);
    b.field(ei_def, "createdAt", nat(Uint), false, 4);
    let wp_def = b.define("WidgetPointer", DefKind::Message);
    b.field(wp_def, "nodeId", gt_cep, false, 1);
    let prd_def = b.define("PluginRelaunchData", DefKind::Message);
    b.field(prd_def, "pluginID", nat(String), false, 1);
    b.field(prd_def, "message", nat(String), false, 2);
    b.field(prd_def, "command", nat(String), false, 3);
    b.field(prd_def, "isDeleted", nat(Bool), false, 4);
    let ngd_def = b.define("NodeGenerationData", DefKind::Message);
    b.field(ngd_def, "useFineGrainedSyncing", nat(Bool), false, 1);
    let (vpst, eit_msg, wpt, prdt, ngdt, btt, fvnf, fvns, fvnfr, fvc, fvp) = (
        def(&b, "VariantPropSpec"),
        def(&b, "EditInfo"),
        def(&b, "WidgetPointer"),
        def(&b, "PluginRelaunchData"),
        def(&b, "NodeGenerationData"),
        def(&b, "BrushType"),
        def(&b, "FontVariantNumericFigure"),
        def(&b, "FontVariantNumericSpacing"),
        def(&b, "FontVariantNumericFraction"),
        def(&b, "FontVariantCaps"),
        def(&b, "FontVariantPosition"),
    );
    f(&mut b, "autoRename", nat(Bool), false);
    f(&mut b, "textDecorationSkipInk", nat(Bool), false);
    let wrap_style = def(&b, "TextWrapStyle");
    f(&mut b, "textWrapStyle", wrap_style, false);
    f(&mut b, "overrideLevel", nat(Uint), false);
    f(&mut b, "detachedSymbolId", sidt, false);
    f(&mut b, "variantPropSpecs", vpst, true);
    f(&mut b, "fontVariantNumericFigure", fvnf, false);
    f(&mut b, "fontVariantNumericSpacing", fvns, false);
    f(&mut b, "fontVariantNumericFraction", fvnfr, false);
    f(&mut b, "fontVariantCaps", fvc, false);
    f(&mut b, "fontVariantPosition", fvp, false);
    f(&mut b, "brushType", btt, false);
    f(&mut b, "userFacingVersion", nat(String), false);
    f(&mut b, "sharedSymbolVersion", nat(String), false);
    f(&mut b, "publishedVersion", nat(String), false);
    f(&mut b, "version", nat(String), false);
    f(&mut b, "isSymbolPublishable", nat(Bool), false);
    f(&mut b, "isPublishable", nat(Bool), false);
    f(&mut b, "publishID", gt_cep, false);
    f(&mut b, "widgetCachedAncestor", wpt, false);
    f(&mut b, "editInfo", eit_msg, false);
    f(&mut b, "pluginRelaunchData", prdt, true);
    f(&mut b, "nodeGenerationData", ngdt, false);
    // Style bookkeeping: legacy GUID refs to effect / grid style nodes, and the soft-delete flag of a style node.
    let guid_ty = def(&b, "GUID");
    f(&mut b, "inheritEffectStyleID", guid_ty, false);
    f(&mut b, "inheritGridStyleID", guid_ty, false);
    f(&mut b, "isSoftDeletedStyle", nat(Bool), false);
    // Child margins (rea-findings: stackChildMarginTop/Right/Bottom/Left = 606..609).
    b.field(nc, "stackChildMarginTop", nat(Float), false, 606);
    b.field(nc, "stackChildMarginRight", nat(Float), false, 607);
    b.field(nc, "stackChildMarginBottom", nat(Float), false, 608);
    b.field(nc, "stackChildMarginLeft", nat(Float), false, 609);
    let jsft = def(&b, "JsonSerializedFields");
    f(&mut b, "isSlot", nat(Bool), false);
    f(&mut b, "isSlotContent", nat(Bool), false);
    f(&mut b, "jsonSerialized", jsft, false);

    // Draw, Motion, Table, Slide, Layout, Slot, Widget, and remaining NodeChange passthrough types:
    enum_def(&mut b, "MaskType", &["ALPHA", "VECTOR", "LUMINANCE"]);
    enum_def(&mut b, "ScrollDirection", &["NONE", "HORIZONTAL", "VERTICAL", "BOTH"]);
    enum_def(&mut b, "LockMode", &["NONE", "ALL", "ASPECT_RATIO", "FIXED_SIZE"]);
    enum_def(&mut b, "GridChildAlign", &["AUTO", "MIN", "CENTER", "MAX", "STRETCH"]);
    enum_def(&mut b, "ColorProfile", &["SRGB", "DISPLAY_P3"]);
    enum_def(&mut b, "CodeObjectType", &["COMPONENT", "FILE", "LIBRARY", "INSTANCE", "LAYER"]);
    enum_def(&mut b, "BrushOrientation", &["FORWARD", "REVERSE"]);
    enum_def(&mut b, "ScatterBrushRotation", &["NONE", "DIRECTION"]);
    enum_def(&mut b, "TransformModifierType", &["REPEAT", "SYMMETRY", "SKEW"]);
    enum_def(&mut b, "TransformModifierCountType", &["NUMBER", "GAP"]);
    enum_def(&mut b, "SymmetryMode", &["HORIZONTAL", "VERTICAL", "RADIAL", "HORIZONTAL_AND_VERTICAL"]);
    enum_def(&mut b, "KeyframeInterpolationType", &["LINEAR", "HOLD", "BEZIER", "SPRING", "EASE"]);
    enum_def(
        &mut b,
        "EasingType",
        &[
            "NONE",
            "IN_CUBIC",
            "OUT_CUBIC",
            "IN_OUT_CUBIC",
            "LINEAR",
            "IN_BACK_CUBIC",
            "OUT_BACK_CUBIC",
            "IN_OUT_BACK_CUBIC",
            "CUSTOM_CUBIC",
            "SPRING",
            "GENTLE_SPRING",
            "CUSTOM_SPRING",
            "SPRING_PRESET_ONE",
            "SPRING_PRESET_TWO",
            "SPRING_PRESET_THREE",
            "OUT_QUAD",
            "OUT_QUART",
            "OUT_QUINT",
            "OUT_SINE",
            "OUT_EXPO",
            "OUT_CIRC",
            "OUT_BACK",
            "IN_QUAD",
            "IN_QUART",
            "IN_QUINT",
            "IN_SINE",
            "IN_EXPO",
            "IN_CIRC",
            "IN_BACK",
            "IN_OUT_QUAD",
            "IN_OUT_QUART",
            "IN_OUT_QUINT",
            "IN_OUT_SINE",
            "IN_OUT_EXPO",
            "IN_OUT_CIRC",
            "IN_OUT_BACK",
            "CUSTOM_CUBIC_BEZIER",
        ],
    );
    enum_def(&mut b, "InterpolationColorSpace", &["SRGB", "OKLCH"]);
    enum_def(
        &mut b,
        "KeyframeOperation",
        &[
            "TRANSLATE_X",
            "TRANSLATE_Y",
            "ROTATE",
            "SCALE_X",
            "SCALE_Y",
            "OPACITY",
            "FILL",
            "STROKE",
            "CORNER_RADIUS",
            "STROKE_WEIGHT",
            "LINE_HEIGHT",
            "LETTER_SPACING",
            "FONT_SIZE",
            "EFFECT",
            "PATH_TRIM_START",
            "PATH_TRIM_END",
            "PATH_TRIM_OFFSET",
            "FONT_VARIATION",
        ],
    );
    enum_def(&mut b, "TrackLoopMode", &["ONCE", "REPEAT", "PING_PONG"]);
    enum_def(&mut b, "TextElementDelayBehaviour", &["NONE", "BY_CHARACTER", "BY_WORD", "BY_LINE"]);
    enum_def(
        &mut b,
        "TransitionType",
        &[
            "INSTANT_TRANSITION",
            "DISSOLVE",
            "FADE",
            "SLIDE_FROM_LEFT",
            "SLIDE_FROM_RIGHT",
            "SLIDE_FROM_TOP",
            "SLIDE_FROM_BOTTOM",
            "PUSH_FROM_LEFT",
            "PUSH_FROM_RIGHT",
            "PUSH_FROM_TOP",
            "PUSH_FROM_BOTTOM",
            "MOVE_FROM_LEFT",
            "MOVE_FROM_RIGHT",
            "MOVE_FROM_TOP",
            "MOVE_FROM_BOTTOM",
            "MOVE_OUT_TO_LEFT",
            "MOVE_OUT_TO_RIGHT",
            "MOVE_OUT_TO_TOP",
            "MOVE_OUT_TO_BOTTOM",
            "SMART_ANIMATE",
            "SCROLL_ANIMATE",
            "SLIDE_OUT_TO_LEFT",
            "SLIDE_OUT_TO_RIGHT",
            "SLIDE_OUT_TO_TOP",
            "SLIDE_OUT_TO_BOTTOM",
        ],
    );
    enum_def(
        &mut b,
        "InteractionType",
        &[
            "ON_CLICK",
            "AFTER_TIMEOUT",
            "MOUSE_IN",
            "MOUSE_OUT",
            "ON_HOVER",
            "MOUSE_DOWN",
            "MOUSE_UP",
            "ON_PRESS",
            "NONE",
            "DRAG",
            "ON_KEY_DOWN",
            "ON_VOICE",
            "ON_MEDIA_HIT",
            "ON_MEDIA_END",
        ],
    );
    enum_def(&mut b, "ObjectAnimationPhase", &["IN", "OUT", "EMPHASIS"]);
    enum_def(&mut b, "ConnectorTextSection", &["MIDDLE", "START", "END"]);
    enum_def(&mut b, "WidgetInputBehavior", &["WRAP", "TRUNCATE", "MULTILINE"]);
    enum_def(&mut b, "WidgetPropertyMenuItemType", &["ACTION", "SEPARATOR", "COLOR_SELECTOR", "DROPDOWN", "WALLPAPER_SELECTOR", "LINK", "TOGGLE"]);

    let vwp = b.define("VariableWidthPoint", DefKind::Struct);
    b.field(vwp, "asymmetricStroke", nat(Bool), false, 1);
    b.field(vwp, "position", nat(Float), false, 2);
    b.field(vwp, "leadingWidth", nat(Float), false, 3);
    b.field(vwp, "trailingWidth", nat(Float), false, 4);

    let dss = b.define("DynamicStrokeSettings", DefKind::Message);
    b.field(dss, "frequency", nat(Float), false, 1);
    b.field(dss, "wiggle", nat(Float), false, 2);
    b.field(dss, "smoothen", nat(Float), false, 3);

    let sss = b.define("ScatterStrokeSettings", DefKind::Message);
    let sbr = def(&b, "ScatterBrushRotation");
    b.field(sss, "gap", nat(Float), false, 1);
    b.field(sss, "wiggle", nat(Float), false, 2);
    b.field(sss, "angularJitter", nat(Float), false, 3);
    b.field(sss, "sizeJitter", nat(Float), false, 4);
    b.field(sss, "rotation", sbr, false, 5);

    let stss = b.define("StretchStrokeSettings", DefKind::Message);
    let bo_ty = def(&b, "BrushOrientation");
    b.field(stss, "orientation", bo_ty, false, 1);

    let ptrim = b.define("PathTrim", DefKind::Message);
    b.field(ptrim, "start", nat(Float), false, 1);
    b.field(ptrim, "end", nat(Float), false, 2);
    b.field(ptrim, "offset", nat(Float), false, 3);

    let sdata = b.define("StrokeData", DefKind::Message);
    let vwpt = def(&b, "VariableWidthPoint");
    b.field(sdata, "dynamicStrokeSeed", nat(Uint), false, 1);
    b.field(sdata, "variableWidthPoints", vwpt, true, 2);

    let tmod = b.define("TransformModifier", DefKind::Message);
    let (tm_ty, tmc_ty, nu_ty, vec_ty, sym_ty) =
        (def(&b, "TransformModifierType"), def(&b, "TransformModifierCountType"), def(&b, "NumberUnits"), def(&b, "Vector"), def(&b, "SymmetryMode"));
    b.field(tmod, "type", tm_ty, false, 1);
    b.field(tmod, "count", nat(Uint), false, 2);
    b.field(tmod, "countType", tmc_ty, false, 3);
    b.field(tmod, "unit", nu_ty, false, 4);
    b.field(tmod, "offset", vec_ty, false, 5);
    b.field(tmod, "symmetryMode", sym_ty, false, 6);
    b.field(tmod, "symmetryOrigin", vec_ty, false, 7);
    b.field(tmod, "rotation", nat(Float), false, 8);
    b.field(tmod, "scale", vec_ty, false, 9);
    b.field(tmod, "skew", vec_ty, false, 10);

    let tldef = b.define("TimelineDefinition", DefKind::Message);
    b.field(tldef, "id", guid_ty, false, 1);
    b.field(tldef, "name", nat(String), false, 2);

    let tlass = b.define("TimelineAssignment", DefKind::Message);
    b.field(tlass, "timelineId", guid_ty, false, 1);
    b.field(tlass, "targetNodeId", guid_ty, false, 2);

    let kfv = b.define("KeyframeValue", DefKind::Message);
    let (col_ty, pt_ty, eff_ty, fv_ty, vdt_ty) = (def(&b, "Color"), def(&b, "Paint"), def(&b, "Effect"), def(&b, "FontVariation"), def(&b, "VectorData"));
    b.field(kfv, "floatValue", nat(Float), false, 1);
    b.field(kfv, "vectorValue", vec_ty, false, 2);
    b.field(kfv, "colorValue", col_ty, false, 3);
    b.field(kfv, "paintValues", pt_ty, true, 4);
    b.field(kfv, "effectValues", eff_ty, true, 5);
    b.field(kfv, "fontVariationValues", fv_ty, true, 6);
    b.field(kfv, "vectorDataValue", vdt_ty, false, 7);

    let bzh = b.define("BezierHandles", DefKind::Message);
    b.field(bzh, "inTangent", vec_ty, false, 1);
    b.field(bzh, "outTangent", vec_ty, false, 2);

    let efcb = b.define("EasingFunctionCubicBezier", DefKind::Struct);
    b.field(efcb, "x1", nat(Float), false, 1);
    b.field(efcb, "y1", nat(Float), false, 2);
    b.field(efcb, "x2", nat(Float), false, 3);
    b.field(efcb, "y2", nat(Float), false, 4);

    let efsp = b.define("EasingFunctionSpring", DefKind::Struct);
    b.field(efsp, "mass", nat(Float), false, 1);
    b.field(efsp, "stiffness", nat(Float), false, 2);
    b.field(efsp, "damping", nat(Float), false, 3);
    b.field(efsp, "initialVelocity", nat(Float), false, 4);

    let edata = b.define("EasingData", DefKind::Message);
    let (et_ty, efcbt, efspt) = (def(&b, "EasingType"), def(&b, "EasingFunctionCubicBezier"), def(&b, "EasingFunctionSpring"));
    b.field(edata, "easingType", et_ty, false, 1);
    b.field(edata, "easingFunctionCubicBezier", efcbt, false, 2);
    b.field(edata, "easingFunctionSpring", efspt, false, 3);

    let apres = b.define("AnimationPreset", DefKind::Message);
    let (edatat, kfot) = (def(&b, "EasingData"), def(&b, "KeyframeOperation"));
    b.field(apres, "presetGuid", guid_ty, false, 1);
    b.field(apres, "delay", nat(Float), false, 2);
    b.field(apres, "duration", nat(Float), false, 3);
    b.field(apres, "easingData", edatat, false, 4);
    b.field(apres, "excludedOperations", kfot, true, 5);

    let trcp = b.define("TableRowColumnPositionEntry", DefKind::Message);
    b.field(trcp, "id", guid_ty, false, 1);
    b.field(trcp, "position", nat(String), false, 2);

    let trcs = b.define("TableRowColumnSizeEntry", DefKind::Message);
    b.field(trcs, "id", guid_ty, false, 1);
    b.field(trcs, "size", nat(Float), false, 2);

    let tmce = b.define("TableMergedCellEntry", DefKind::Message);
    b.field(tmce, "rowId", guid_ty, false, 1);
    b.field(tmce, "colId", guid_ty, false, 2);
    b.field(tmce, "rowSpan", nat(Uint), false, 3);
    b.field(tmce, "colSpan", nat(Uint), false, 4);

    let tmc = b.define("TableMergedCells", DefKind::Message);
    let tmcet = def(&b, "TableMergedCellEntry");
    b.field(tmc, "entries", tmcet, true, 1);

    let oat = b.define("ObjectAnimationTiming", DefKind::Message);
    let it_ty = def(&b, "InteractionType");
    b.field(oat, "type", it_ty, false, 1);
    b.field(oat, "delay", nat(Float), false, 2);

    let oanim = b.define("ObjectAnimation", DefKind::Message);
    let (tt_ty, oatt, oapt) = (def(&b, "TransitionType"), def(&b, "ObjectAnimationTiming"), def(&b, "ObjectAnimationPhase"));
    b.field(oanim, "target", guid_ty, false, 1);
    b.field(oanim, "transitionType", tt_ty, false, 2);
    b.field(oanim, "transitionDuration", nat(Float), false, 3);
    b.field(oanim, "easingType", et_ty, false, 4);
    b.field(oanim, "easingFunctionCubicBezier", efcbt, false, 5);
    b.field(oanim, "easingFunctionSpring", efspt, false, 6);
    b.field(oanim, "timing", oatt, false, 7);
    b.field(oanim, "phase", oapt, false, 8);

    let slink = b.define("SymbolLink", DefKind::Message);
    b.field(slink, "uri", nat(String), false, 1);
    b.field(slink, "displayName", nat(String), false, 2);
    b.field(slink, "displayText", nat(String), false, 3);

    let wevt = b.define("WidgetEvent", DefKind::Message);
    b.field(wevt, "eventId", nat(String), false, 1);

    let whst = b.define("WidgetHoverStyle", DefKind::Message);
    b.field(whst, "fillPaints", pt_ty, true, 1);
    b.field(whst, "strokePaints", pt_ty, true, 2);
    b.field(whst, "opacity", nat(Float), false, 3);
    b.field(whst, "areFillPaintsSet", nat(Bool), false, 4);
    b.field(whst, "areStrokePaintsSet", nat(Bool), false, 5);
    b.field(whst, "isOpacitySet", nat(Bool), false, 6);

    let ctm = b.define("ConnectorTextMidpoint", DefKind::Message);
    let cts_ty = def(&b, "ConnectorTextSection");
    b.field(ctm, "section", cts_ty, false, 1);
    b.field(ctm, "offset", nat(Float), false, 2);

    let ccp = b.define("ConnectorControlPoint", DefKind::Message);
    b.field(ccp, "position", vec_ty, false, 1);
    b.field(ccp, "axis", vec_ty, false, 2);

    let gat_def = b.define("GridAutoTracks", DefKind::Message);
    b.field(gat_def, "gridColumnsSizing", nat(String), false, 1);
    b.field(gat_def, "gridRowsSizing", nat(String), false, 2);

    let psp = b.define("PrototypeStartingPoint", DefKind::Message);
    b.field(psp, "name", nat(String), false, 1);
    b.field(psp, "description", nat(String), false, 2);

    let csi = b.define("CodeSourceInfo", DefKind::Message);
    b.field(csi, "sourcePath", nat(String), false, 1);
    b.field(csi, "lineNumber", nat(Uint), false, 2);

    let scc = b.define("SourceControlConfig", DefKind::Message);
    b.field(scc, "repoUrl", nat(String), false, 1);
    b.field(scc, "branch", nat(String), false, 2);

    let wmeta = b.define("WidgetMetadata", DefKind::Message);
    b.field(wmeta, "pluginID", nat(String), false, 1);
    b.field(wmeta, "pluginVersionID", nat(String), false, 2);
    b.field(wmeta, "widgetName", nat(String), false, 3);

    let rse = b.define("RenderedSyncedStateEntry", DefKind::Message);
    b.field(rse, "key", nat(String), false, 1);
    b.field(rse, "value", nat(String), false, 2);
    let rss = b.define("RenderedSyncedState", DefKind::Message);
    let rset = def(&b, "RenderedSyncedStateEntry");
    b.field(rss, "entries", rset, true, 1);

    let acat = b.define("AnnotationCategory", DefKind::Message);
    b.field(acat, "id", guid_ty, false, 1);
    b.field(acat, "label", nat(String), false, 2);
    b.field(acat, "color", col_ty, false, 3);

    let mcs = b.define("MakeContentState", DefKind::Message);
    b.field(mcs, "prompt", nat(String), false, 1);
    b.field(mcs, "status", nat(String), false, 2);

    let wpmi = b.define("WidgetPropertyMenuItem", DefKind::Message);
    let wpmit = def(&b, "WidgetPropertyMenuItemType");
    b.field(wpmi, "propertyName", nat(String), false, 1);
    b.field(wpmi, "tooltip", nat(String), false, 2);
    b.field(wpmi, "itemType", wpmit, false, 3);
    b.field(wpmi, "icon", nat(String), false, 4);
    b.field(wpmi, "isToggled", nat(Bool), false, 5);

    let dtd_def = b.define("DerivedTextData", DefKind::Message);
    b.field(dtd_def, "layoutSize", vec_ty, false, 1);
    b.field(dtd_def, "fontDigest", nat(Byte), true, 2);

    let difd_def = b.define("DerivedImmutableFrameData", DefKind::Message);
    b.field(difd_def, "version", nat(Uint), false, 1);

    let (
        mt_ty,
        sd_ty,
        lm_ty,
        gca_ty,
        cp_ty,
        cot_ty,
        sc_ty,
        vwpt_nc,
        dsst,
        ssst,
        stsst,
        mx_ty,
        ptrimt,
        sdatat,
        tmodt,
        tldeft,
        tlasst,
        kfvt,
        kfit,
        bzht,
        edatat_nc,
        et_ty_nc,
        icst,
        kfot_nc,
        tlmt,
        sidt_nc,
        aprest,
        tedbt,
        trcpt,
        trcst,
        tmct,
        oanimt,
        slinkt,
        wevtt,
        whstt,
        ctmt,
        ccpt,
        gatt,
        pspt,
        csit,
        wibt,
        scct,
        wmetat,
        rsst,
        acatt,
        mcst,
        wpmit_nc,
        dtdt,
        difdt,
    ) = (
        def(&b, "MaskType"),
        def(&b, "ScrollDirection"),
        def(&b, "LockMode"),
        def(&b, "GridChildAlign"),
        def(&b, "ColorProfile"),
        def(&b, "CodeObjectType"),
        def(&b, "StrokeCap"),
        def(&b, "VariableWidthPoint"),
        def(&b, "DynamicStrokeSettings"),
        def(&b, "ScatterStrokeSettings"),
        def(&b, "StretchStrokeSettings"),
        def(&b, "Matrix"),
        def(&b, "PathTrim"),
        def(&b, "StrokeData"),
        def(&b, "TransformModifier"),
        def(&b, "TimelineDefinition"),
        def(&b, "TimelineAssignment"),
        def(&b, "KeyframeValue"),
        def(&b, "KeyframeInterpolationType"),
        def(&b, "BezierHandles"),
        def(&b, "EasingData"),
        def(&b, "EasingType"),
        def(&b, "InterpolationColorSpace"),
        def(&b, "KeyframeOperation"),
        def(&b, "TrackLoopMode"),
        def(&b, "StyleId"),
        def(&b, "AnimationPreset"),
        def(&b, "TextElementDelayBehaviour"),
        def(&b, "TableRowColumnPositionEntry"),
        def(&b, "TableRowColumnSizeEntry"),
        def(&b, "TableMergedCells"),
        def(&b, "ObjectAnimation"),
        def(&b, "SymbolLink"),
        def(&b, "WidgetEvent"),
        def(&b, "WidgetHoverStyle"),
        def(&b, "ConnectorTextMidpoint"),
        def(&b, "ConnectorControlPoint"),
        def(&b, "GridAutoTracks"),
        def(&b, "PrototypeStartingPoint"),
        def(&b, "CodeSourceInfo"),
        def(&b, "WidgetInputBehavior"),
        def(&b, "SourceControlConfig"),
        def(&b, "WidgetMetadata"),
        def(&b, "RenderedSyncedState"),
        def(&b, "AnnotationCategory"),
        def(&b, "MakeContentState"),
        def(&b, "WidgetPropertyMenuItem"),
        def(&b, "DerivedTextData"),
        def(&b, "DerivedImmutableFrameData"),
    );

    f(&mut b, "maskType", mt_ty, false);
    f(&mut b, "scrollDirection", sd_ty, false);
    f(&mut b, "lockMode", lm_ty, false);
    f(&mut b, "gridChildVerticalAlign", gca_ty, false);
    f(&mut b, "gridChildHorizontalAlign", gca_ty, false);
    f(&mut b, "codeExportName", nat(String), false);
    f(&mut b, "documentColorProfile", cp_ty, false);
    f(&mut b, "codeObjectType", cot_ty, false);
    f(&mut b, "connectorStartCap", sc_ty, false);
    f(&mut b, "connectorEndCap", sc_ty, false);
    f(&mut b, "resizeToFit", nat(Bool), false);
    f(&mut b, "shapeTruncates", nat(Bool), false);
    f(&mut b, "isPageDivider", nat(Bool), false);
    f(&mut b, "backgroundEnabled", nat(Bool), false);
    f(&mut b, "gridReflowEnabled", nat(Bool), false);
    f(&mut b, "isCollectionExtendable", nat(Bool), false);
    f(&mut b, "shouldHideCursorsOnWidgetHover", nat(Bool), false);
    f(&mut b, "isWidgetStickable", nat(Bool), false);
    f(&mut b, "shapeUserHeight", nat(Float), false);
    f(&mut b, "backgroundOpacity", nat(Float), false);
    f(&mut b, "connectorVersion", nat(Uint), false);
    f(&mut b, "vectorOperationVersion", nat(Uint), false);
    f(&mut b, "variableWidthPoints", vwpt_nc, true);
    f(&mut b, "dynamicStrokeSettings", dsst, false);
    f(&mut b, "scatterStrokeSettings", ssst, false);
    f(&mut b, "stretchStrokeSettings", stsst, false);
    f(&mut b, "scatterBrushTransforms", mx_ty, true);
    f(&mut b, "strokeBrushGuid", guid_ty, false);
    f(&mut b, "strokeSeed", nat(Uint), false);
    f(&mut b, "pathTrim", ptrimt, false);
    f(&mut b, "strokeOffset", nat(Float), false);
    f(&mut b, "strokeData", sdatat, false);
    f(&mut b, "transformModifiers", tmodt, true);
    f(&mut b, "textPathStart", nat(Float), false);
    f(&mut b, "timelineDefinitions", tldeft, true);
    f(&mut b, "timelineAssignments", tlasst, true);
    f(&mut b, "timelinePosition", nat(Float), false);
    f(&mut b, "keyframeValue", kfvt, false);
    f(&mut b, "interpolationType", kfit, false);
    f(&mut b, "bezierHandles", bzht, false);
    f(&mut b, "easingData", edatat_nc, false);
    f(&mut b, "easingType", et_ty_nc, false);
    f(&mut b, "interpolationColorSpace", icst, false);
    f(&mut b, "keyframeOperation", kfot_nc, false);
    f(&mut b, "trackLoopMode", tlmt, false);
    f(&mut b, "timelineDuration", nat(Float), false);
    f(&mut b, "timelineOffset", nat(Float), false);
    f(&mut b, "timelineDisabled", nat(Bool), false);
    f(&mut b, "styleIdsForAnimation", sidt_nc, true);
    f(&mut b, "animationPresets", aprest, true);
    f(&mut b, "textElementDelayBehaviour", tedbt, false);
    f(&mut b, "tableRowPositions", trcpt, true);
    f(&mut b, "tableColumnPositions", trcpt, true);
    f(&mut b, "tableRowHeights", trcst, true);
    f(&mut b, "tableColumnWidths", trcst, true);
    f(&mut b, "tableMergedCells", tmct, false);
    f(&mut b, "slideSpeakerNotes", nat(String), false);
    f(&mut b, "isSkippedSlide", nat(Bool), false);
    f(&mut b, "slideNumber", nat(String), false);
    f(&mut b, "slideNumberSeparator", nat(String), false);
    f(&mut b, "objectAnimations", oanimt, true);
    f(&mut b, "symbolLinks", slinkt, true);
    f(&mut b, "widgetEvents", wevtt, true);
    f(&mut b, "widgetHoverStyle", whstt, false);
    f(&mut b, "connectorTextMidpoint", ctmt, false);
    f(&mut b, "ancestorPathBeforeDeletion", guid_ty, true);
    f(&mut b, "connectorControlPoints", ccpt, true);
    f(&mut b, "gridAutoTracks", gatt, false);
    f(&mut b, "prototypeStartingPoint", pspt, false);
    f(&mut b, "codeSourceInfo", csit, false);
    f(&mut b, "widgetInputBehavior", wibt, false);
    f(&mut b, "sourceControlConfig", scct, false);
    f(&mut b, "widgetMetadata", wmetat, false);
    f(&mut b, "renderedSyncedState", rsst, false);
    f(&mut b, "annotationCategories", acatt, true);
    f(&mut b, "makeContentState", mcst, false);
    f(&mut b, "widgetSyncedState", rsst, false);
    f(&mut b, "widgetPropertyMenuItems", wpmit_nc, true);
    f(&mut b, "derivedTextData", dtdt, false);
    f(&mut b, "derivedImmutableFrameData", difdt, false);

    let blob = b.define("Blob", DefKind::Message);
    b.field(blob, "bytes", nat(Byte), true, 1);
    let msg = b.define("Message", DefKind::Message);
    let (nct, bt, mtt) = (def(&b, "NodeChange"), def(&b, "Blob"), def(&b, "MessageType"));
    b.field(msg, "nodeChanges", nct, true, 1);
    b.field(msg, "blobs", bt, true, 2);
    b.field(msg, "type", mtt, false, 3);
    b.field(msg, "sessionID", nat(Uint), false, 4);
    b.field(msg, "ackID", nat(Uint), false, 5);
    b.build()
}

// ---- value builders

pub fn msg(fields: Vec<(&str, V)>) -> V {
    V::Message(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn st(fields: Vec<(&str, V)>) -> V {
    V::Struct(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn en(n: &str) -> V {
    V::Enum(n.into())
}
pub fn fl(x: f32) -> V {
    V::Float(x)
}
pub fn guid(s: u32, l: u32) -> V {
    st(vec![("sessionID", V::Uint(s)), ("localID", V::Uint(l))])
}
pub fn vec2(x: f32, y: f32) -> V {
    st(vec![("x", fl(x)), ("y", fl(y))])
}
pub fn matrix(tx: f32, ty: f32) -> V {
    st(vec![("m00", fl(1.0)), ("m01", fl(0.0)), ("m02", fl(tx)), ("m10", fl(0.0)), ("m11", fl(1.0)), ("m12", fl(ty))])
}
pub fn color(r: f32, g: f32, b: f32, a: f32) -> V {
    st(vec![("r", fl(r)), ("g", fl(g)), ("b", fl(b)), ("a", fl(a))])
}
pub fn solid(r: f32, g: f32, b: f32) -> V {
    msg(vec![("type", en("SOLID")), ("color", color(r, g, b, 1.0)), ("opacity", fl(1.0)), ("visible", V::Bool(true))])
}
pub fn node(id: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, mut extra: Vec<(&str, V)>) -> V {
    let mut f = vec![
        ("guid", guid(id.0, id.1)),
        ("parentIndex", st(vec![("guid", guid(parent.0, parent.1)), ("position", V::String(pos.into()))])),
        ("type", en(ty)),
        ("name", V::String(name.into())),
    ];
    f.append(&mut extra);
    msg(f)
}

/// Commands blob for a closed triangle: M(0,0) L(10,0) L(5,8) Z.
pub fn triangle_blob() -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, &[0.0f32, 0.0][..]), (2, &[10.0, 0.0]), (2, &[5.0, 8.0])] {
        b.push(op);
        for p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b.push(0);
    b
}

pub const IMAGE_HASH: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

/// A small but representative document. Siblings are deliberately listed out
/// of z-order to exercise fractional-index sorting.
pub fn sample_message() -> V {
    let doc = node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]);
    let page = node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![("backgroundColor", color(0.9, 0.9, 0.9, 1.0))]);
    let internal = node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]);
    let frame = node(
        (1, 1),
        (0, 1),
        "a",
        "FRAME",
        "Card",
        vec![
            ("size", vec2(200.0, 300.0)),
            ("transform", matrix(40.0, 50.0)),
            ("fillPaints", V::Array(vec![solid(1.0, 1.0, 1.0)])),
            ("cornerRadius", fl(12.0)),
            (
                "effects",
                V::Array(vec![msg(vec![
                    ("type", en("DROP_SHADOW")),
                    ("color", color(0.0, 0.0, 0.0, 0.25)),
                    ("offset", vec2(0.0, 4.0)),
                    ("radius", fl(8.0)),
                    ("spread", fl(0.0)),
                    ("visible", V::Bool(true)),
                ])]),
            ),
            ("stackMode", en("VERTICAL")),
            ("stackSpacing", fl(8.0)),
            ("stackHorizontalPadding", fl(16.0)),
            ("stackVerticalPadding", fl(12.0)),
            ("stackPrimarySizing", en("RESIZE_TO_FIT")),
            ("stackCounterAlignItems", en("CENTER")),
        ],
    );
    let rect = node(
        (1, 2),
        (1, 1),
        "b", // listed first, but sorts after the text
        "ROUNDED_RECTANGLE",
        "Image rect",
        vec![
            ("size", vec2(100.0, 40.0)),
            ("transform", matrix(16.0, 12.0)),
            (
                "fillPaints",
                V::Array(vec![msg(vec![("type", en("IMAGE")), ("image", msg(vec![("hash", V::Bytes(IMAGE_HASH.to_vec()))])), ("imageScaleMode", en("FILL"))])]),
            ),
            ("strokePaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
            ("strokeWeight", fl(2.0)),
            ("strokeAlign", en("OUTSIDE")),
            ("stackChildAlignSelf", en("STRETCH")),
            ("blendMode", en("MULTIPLY")),
        ],
    );
    let text = node(
        (1, 3),
        (1, 1),
        "a",
        "TEXT",
        "Title",
        vec![
            ("size", vec2(120.0, 20.0)),
            ("transform", matrix(16.0, 60.0)),
            ("fillPaints", V::Array(vec![solid(0.1, 0.2, 0.3)])),
            ("fontSize", fl(16.0)),
            (
                "fontName",
                st(vec![("family", V::String("Inter".into())), ("style", V::String("Medium".into())), ("postscript", V::String("Inter-Medium".into()))]),
            ),
            ("letterSpacing", st(vec![("value", fl(10.0)), ("units", en("PERCENT"))])),
            ("lineHeight", st(vec![("value", fl(24.0)), ("units", en("PIXELS"))])),
            ("textAlignHorizontal", en("CENTER")),
            ("textAutoResize", en("WIDTH_AND_HEIGHT")),
            (
                "textData",
                msg(vec![
                    ("characters", V::String("Hi \u{1F600}!".into())),
                    // 'H','i',' ', surrogate pair (2 units), '!' -> UTF-16 length 6
                    ("characterStyleIDs", V::Array([0u32, 0, 0, 1, 1, 0].iter().map(|x| V::Uint(*x)).collect())),
                    (
                        "styleOverrideTable",
                        V::Array(vec![msg(vec![
                            ("styleID", V::Uint(1)),
                            (
                                "fontName",
                                st(vec![
                                    ("family", V::String("Inter".into())),
                                    ("style", V::String("Bold Italic".into())),
                                    ("postscript", V::String("".into())),
                                ]),
                            ),
                            ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
                            ("textDecoration", en("UNDERLINE")),
                        ])]),
                    ),
                ]),
            ),
        ],
    );
    let vector = node(
        (1, 4),
        (1, 1),
        "c",
        "VECTOR",
        "Tri",
        vec![
            ("size", vec2(10.0, 8.0)),
            ("transform", matrix(0.0, 100.0)),
            ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("fillGeometry", V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(0))])])),
        ],
    );
    let comp = node(
        (1, 5),
        (1, 1),
        "d",
        "SYMBOL",
        "Button",
        vec![("size", vec2(80.0, 30.0)), ("transform", matrix(0.0, 120.0)), ("fillPaints", V::Array(vec![solid(0.0, 0.5, 1.0)]))],
    );
    let comp_child = node((1, 6), (1, 5), "a", "RECTANGLE", "Bg", vec![("size", vec2(80.0, 30.0)), ("fillPaints", V::Array(vec![solid(0.2, 0.2, 0.2)]))]);
    let inst = node(
        (1, 7),
        (1, 1),
        "e",
        "INSTANCE",
        "Button copy",
        vec![("size", vec2(80.0, 30.0)), ("transform", matrix(100.0, 120.0)), ("symbolData", msg(vec![("symbolID", guid(1, 5))]))],
    );
    let ell = node(
        (1, 8),
        (0, 1),
        "b",
        "ELLIPSE",
        "Pie",
        vec![
            ("size", vec2(50.0, 50.0)),
            ("transform", matrix(300.0, 50.0)),
            ("arcData", st(vec![("startingAngle", fl(0.0)), ("endingAngle", fl(3.0)), ("innerRadius", fl(0.5))])),
        ],
    );
    let removed = V::Message(match node((1, 9), (1, 1), "z", "RECTANGLE", "Removed", vec![("phase", en("REMOVED"))]) {
        V::Message(f) => f,
        _ => unreachable!(),
    });
    let star = node((1, 10), (0, 1), "c", "STAR", "Star", vec![("size", vec2(40.0, 40.0)), ("count", V::Uint(6)), ("starInnerScale", fl(0.5))]);
    let sticky = node((1, 11), (0, 1), "d", "STICKY", "Note", vec![("size", vec2(40.0, 40.0))]);

    msg(vec![
        // Child before parent and shuffled positions on purpose.
        ("nodeChanges", V::Array(vec![inst, rect, text, vector, comp_child, comp, frame, ell, removed, star, sticky, internal, page, doc])),
        ("blobs", V::Array(vec![msg(vec![("bytes", V::Bytes(triangle_blob()))])])),
    ])
}

/// Builds a complete `.fig` byte stream from the sample message.
pub fn sample_fig(how: Compression, zipped: bool) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &sample_message()).unwrap();
    let canvas = container::write_container(&schema.encode(), &body, how, 101);
    if !zipped {
        return canvas;
    }
    let mut images = BTreeMap::new();
    images.insert(crate::kiwi::hex(&IMAGE_HASH), vec![0x89, b'P', b'N', b'G']);
    container::write_zip(&canvas, &images, Some(&serde_json::json!({"file_name": "Sample file"})))
}

// ---- upgrade fixture: geometry, styles, grids, text clamping, instance overrides

/// Commands blob for a closed square: M(0,0) L(10,0) L(10,10) L(0,10) Z.
pub fn square_blob() -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, [0.0f32, 0.0]), (2, [10.0, 0.0]), (2, [10.0, 10.0]), (2, [0.0, 10.0])] {
        b.push(op);
        for p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b.push(0);
    b
}

fn path_of(guids: &[(u32, u32)]) -> V {
    msg(vec![("guids", V::Array(guids.iter().map(|g| guid(g.0, g.1)).collect()))])
}

fn ov(path: &[(u32, u32)], mut fields: Vec<(&str, V)>) -> V {
    let mut f = vec![("guidPath", path_of(path))];
    f.append(&mut fields);
    msg(f)
}

fn num(value: f32, units: &str) -> V {
    st(vec![("value", fl(value)), ("units", en(units))])
}

fn text_data(s: &str) -> V {
    msg(vec![("characters", V::String(s.into()))])
}

/// Message exercising the `.fig` upgrade features (see `tests/fig_upgrade.rs`).
/// Blob 0 is the triangle, blob 1 the square.
pub fn upgrade_message() -> V {
    let geo = |rule: &str, blob: u32| V::Array(vec![msg(vec![("windingRule", en(rule)), ("commandsBlob", V::Uint(blob))])]);
    let size = |w: f32, h: f32| ("size", vec2(w, h));
    let doc = node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]);
    let page = node((0, 1), (0, 0), "a", "CANVAS", "Up", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]);
    let internal = node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]);
    let style_node = |l: u32, name: &str, ty: &str, desc: &str, key: Option<&str>| {
        let mut f = vec![("styleType", en(ty)), ("styleDescription", V::String(desc.into())), size(1.0, 1.0)];
        if let Some(k) = key {
            f.push(("key", V::String(k.into())));
        }
        node((2, l), (0, 2), &format!("{l}"), "RECTANGLE", name, f)
    };
    let styles = [
        style_node(100, "Brand/Primary", "FILL", "Main brand fill", Some("key-primary")),
        style_node(101, "Border/Strong", "FILL", "", None),
        style_node(102, "Body", "TEXT", "Body copy", Some("key-body")),
    ];
    let sref = |l: u32| msg(vec![("guid", guid(2, l))]);

    let grids = node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Grids",
        vec![
            size(300.0, 200.0),
            (
                "layoutGrids",
                V::Array(vec![
                    msg(vec![
                        ("pattern", en("COLUMNS")),
                        ("count", V::Uint(4)),
                        ("gutterSize", fl(8.0)),
                        ("offset", fl(2.0)),
                        ("color", color(1.0, 0.0, 0.0, 0.1)),
                        ("visible", V::Bool(true)),
                    ]),
                    msg(vec![("pattern", en("GRID")), ("sectionSize", fl(10.0)), ("color", color(0.0, 0.0, 1.0, 0.2))]),
                    msg(vec![("pattern", en("ROWS")), ("count", V::Uint(3)), ("gutterSize", fl(4.0)), ("visible", V::Bool(false))]),
                ]),
            ),
        ],
    );
    let boolean = node(
        (2, 2),
        (0, 1),
        "b",
        "BOOLEAN_OPERATION",
        "Bool",
        vec![
            size(10.0, 10.0),
            ("fillGeometry", geo("NONZERO", 0)),
            ("strokeGeometry", geo("ODD", 1)),
            ("blendMode", en("LINEAR_BURN")),
            ("styleIdForFill", sref(100)),
            ("styleIdForStrokeFill", sref(101)),
            ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
            ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
        ],
    );
    let star = node(
        (2, 3),
        (0, 1),
        "c",
        "STAR",
        "Star",
        vec![size(10.0, 10.0), ("count", V::Uint(5)), ("fillGeometry", geo("NONZERO", 0)), ("blendMode", en("LINEAR_DODGE"))],
    );
    let text = |l: u32, pos: &str, name: &str, mut extra: Vec<(&str, V)>| {
        let mut f = vec![size(80.0, 40.0), ("fontSize", fl(10.0)), ("textData", text_data(name))];
        f.append(&mut extra);
        node((2, l), (0, 1), pos, "TEXT", name, f)
    };
    let clamp = text(
        4,
        "d",
        "Clamp",
        vec![("textTruncation", en("ENDING")), ("maxLines", V::Uint(2)), ("lineHeight", num(100.0, "PERCENT")), ("styleIdForText", sref(102))],
    );
    let lh_auto = text(5, "e", "LhAuto", vec![("lineHeight", num(1.0, "RAW"))]);
    let lh_pct = text(6, "f", "LhPct", vec![("lineHeight", num(150.0, "PERCENT"))]);
    let lh_none = text(7, "g", "LhNone", vec![]);
    let lh_px = text(8, "h", "LhPx", vec![("lineHeight", num(18.0, "PIXELS"))]);
    let plain = text(9, "i", "Plain", vec![]);
    // Lists: bullet, nested number, plain line, in a right-to-left, cap-trimmed layer.
    let line = |ty: &str, level: u32| msg(vec![("lineType", en(ty)), ("indentationLevel", V::Uint(level)), ("sourceDirectionality", en("RTL"))]);
    let listy = node(
        (2, 50),
        (0, 1),
        "o",
        "TEXT",
        "Listy",
        vec![
            size(80.0, 40.0),
            ("fontSize", fl(10.0)),
            (
                "textData",
                msg(vec![
                    (
                        "characters",
                        V::String(
                            "one
two
three"
                                .into(),
                        ),
                    ),
                    ("lines", V::Array(vec![line("UNORDERED_LIST", 1), line("ORDERED_LIST", 2), line("PLAIN", 0)])),
                ]),
            ),
            ("leadingTrim", en("CAP_HEIGHT")),
            ("paragraphIndent", fl(6.0)),
            ("toggledOnOTFeatures", V::Array(vec![en("TNUM"), en("SS01")])),
            ("toggledOffOTFeatures", V::Array(vec![en("LIGA")])),
        ],
    );

    // Component with a label, background and a nested frame.
    let card = node((2, 10), (0, 1), "j", "SYMBOL", "Card", vec![size(100.0, 50.0), ("transform", matrix(0.0, 300.0))]);
    let label = node(
        (2, 11),
        (2, 10),
        "a",
        "TEXT",
        "Label",
        vec![size(60.0, 12.0), ("fontSize", fl(10.0)), ("textData", text_data("Old")), ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)]))],
    );
    let bg = node((2, 12), (2, 10), "b", "RECTANGLE", "Bg", vec![size(100.0, 50.0), ("fillPaints", V::Array(vec![solid(0.5, 0.5, 0.5)]))]);
    let inner = node((2, 13), (2, 10), "c", "FRAME", "Inner", vec![size(20.0, 20.0)]);
    let dot = node((2, 14), (2, 13), "a", "RECTANGLE", "Dot", vec![size(5.0, 5.0), ("fillPaints", V::Array(vec![solid(0.0, 1.0, 0.0)]))]);

    // Wrapper component containing a nested instance of Card with its own override.
    let wrapper = node((2, 20), (0, 1), "k", "SYMBOL", "Wrapper", vec![size(100.0, 50.0), ("transform", matrix(200.0, 300.0))]);
    let card_in = node(
        (2, 21),
        (2, 20),
        "a",
        "INSTANCE",
        "CardIn",
        vec![
            size(100.0, 50.0),
            (
                "symbolData",
                msg(vec![("symbolID", guid(2, 10)), ("symbolOverrides", V::Array(vec![ov(&[(2, 11)], vec![("textData", text_data("Inner text"))])]))]),
            ),
        ],
    );
    let wrap_inst = node(
        (2, 30),
        (0, 1),
        "l",
        "INSTANCE",
        "WrapInst",
        vec![
            size(100.0, 50.0),
            ("transform", matrix(400.0, 300.0)),
            (
                "symbolData",
                msg(vec![("symbolID", guid(2, 20)), ("symbolOverrides", V::Array(vec![ov(&[(2, 21), (2, 11)], vec![("textData", text_data("Deep"))])]))]),
            ),
        ],
    );

    let card_a = node(
        (2, 40),
        (0, 1),
        "m",
        "INSTANCE",
        "Card A",
        vec![
            size(200.0, 80.0),
            ("transform", matrix(0.0, 400.0)),
            (
                "symbolData",
                msg(vec![
                    ("symbolID", guid(2, 10)),
                    (
                        "symbolOverrides",
                        V::Array(vec![
                            ov(
                                &[(2, 11)],
                                vec![
                                    ("textData", text_data("New \u{1F600}")),
                                    ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
                                    ("letterSpacing", num(50.0, "PERCENT")),
                                    ("opacity", fl(0.75)),
                                ],
                            ),
                            ov(&[(2, 12)], vec![("visible", V::Bool(false)), ("blendMode", en("LINEAR_DODGE")), ("cornerRadius", fl(4.0))]),
                            ov(&[(2, 13), (2, 14)], vec![("fillPaints", V::Array(vec![solid(0.0, 0.0, 1.0)])), size(7.0, 7.0)]),
                            ov(&[(9, 99)], vec![("visible", V::Bool(false))]), // dangling path: ignored
                        ]),
                    ),
                ]),
            ),
            (
                "derivedSymbolData",
                V::Array(vec![
                    ov(
                        &[(2, 12)],
                        vec![
                            size(200.0, 80.0),
                            ("transform", matrix(5.0, 6.0)),
                            ("fillGeometry", geo("NONZERO", 1)),
                            ("name", V::String("ignored by derived".into())),
                        ],
                    ),
                    ov(&[], vec![size(200.0, 80.0)]),
                ]),
            ),
        ],
    );
    let card_b = node(
        (2, 41),
        (0, 1),
        "n",
        "INSTANCE",
        "Card B",
        vec![size(100.0, 50.0), ("transform", matrix(300.0, 400.0)), ("symbolData", msg(vec![("symbolID", guid(2, 10))]))],
    );

    let mut nodes = vec![doc, page, internal];
    nodes.extend(styles);
    nodes.extend([grids, boolean, star, clamp, lh_auto, lh_pct, lh_none, lh_px, plain, listy]);
    nodes.extend([card, label, bg, inner, dot, wrapper, card_in, wrap_inst, card_a, card_b]);
    msg(vec![
        ("nodeChanges", V::Array(nodes)),
        ("blobs", V::Array(vec![msg(vec![("bytes", V::Bytes(triangle_blob()))]), msg(vec![("bytes", V::Bytes(square_blob()))])])),
    ])
}

/// `.fig` bytes for [`upgrade_message`].
pub fn upgrade_fig(how: Compression) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &upgrade_message()).unwrap();
    container::write_container(&schema.encode(), &body, how, 101)
}
