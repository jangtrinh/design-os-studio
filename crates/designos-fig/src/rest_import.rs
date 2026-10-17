//! Figma REST (`GET /v1/files/:key`) JSON -> [`Document`].
//!
//! The `.fig` importer converts its kiwi message into this same JSON shape and
//! reuses this mapper, so every field name below is the REST name.

use crate::svgpath::parse_path;
use crate::FigError;
use designos_doc::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const MAX_DEPTH: usize = 300;

// ------------------------------------------------------------ JSON helpers

pub(crate) fn num(v: &Value) -> Option<f32> {
    v.as_f64().map(|x| x as f32)
}
pub(crate) fn fnum(v: &Value, k: &str) -> Option<f32> {
    v.get(k).and_then(num)
}
/// Per-page deferred instances, pages already hydrated, instance targets and expanded instances.
type DeferredHydration = (HashMap<NodeId, Vec<NodeId>>, HashSet<NodeId>, BTreeMap<NodeId, NodeId>, BTreeSet<NodeId>);

pub(crate) fn fstr<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
pub(crate) fn fbool(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(Value::as_bool)
}
pub(crate) fn farr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub(crate) fn vec2(v: &Value) -> Option<Vec2> {
    Some(Vec2::new(fnum(v, "x")?, fnum(v, "y")?))
}

pub(crate) fn color(v: &Value) -> Option<Color> {
    Some(Color { r: fnum(v, "r")?, g: fnum(v, "g")?, b: fnum(v, "b")?, a: fnum(v, "a").unwrap_or(1.0) })
}

pub(crate) fn blend(s: &str) -> BlendMode {
    match s {
        "PASS_THROUGH" => BlendMode::PassThrough,
        "DARKEN" => BlendMode::Darken,
        "MULTIPLY" => BlendMode::Multiply,
        // No linear burn/dodge in the document model; closest relatives.
        "LINEAR_BURN" => BlendMode::LinearBurn,
        "COLOR_BURN" => BlendMode::ColorBurn,
        "LIGHTEN" => BlendMode::Lighten,
        "SCREEN" => BlendMode::Screen,
        "LINEAR_DODGE" => BlendMode::LinearDodge,
        "COLOR_DODGE" => BlendMode::ColorDodge,
        "OVERLAY" => BlendMode::Overlay,
        "SOFT_LIGHT" => BlendMode::SoftLight,
        "HARD_LIGHT" => BlendMode::HardLight,
        "DIFFERENCE" => BlendMode::Difference,
        "EXCLUSION" => BlendMode::Exclusion,
        "HUE" => BlendMode::Hue,
        "SATURATION" => BlendMode::Saturation,
        "COLOR" => BlendMode::Color,
        "LUMINOSITY" => BlendMode::Luminosity,
        _ => BlendMode::Normal,
    }
}

// ------------------------------------------------------------ paints & effects

pub(crate) fn parse_paint(v: &Value) -> Option<Paint> {
    let t = fstr(v, "type")?;
    let handles = || -> [Vec2; 3] {
        let h = farr(v, "gradientHandlePositions");
        let g = |i: usize, d: Vec2| h.get(i).and_then(vec2).unwrap_or(d);
        [g(0, Vec2::new(0.5, 0.0)), g(1, Vec2::new(0.5, 1.0)), g(2, Vec2::new(1.0, 0.0))]
    };
    let stops = || -> Vec<GradientStop> {
        farr(v, "gradientStops").iter().filter_map(|s| Some(GradientStop { pos: fnum(s, "position")?, color: s.get("color").and_then(color)? })).collect()
    };
    let kind = match t {
        "SOLID" => PaintKind::Solid(v.get("color").and_then(color).unwrap_or(Color::BLACK)),
        "GRADIENT_LINEAR" => PaintKind::LinearGradient { handles: handles(), stops: stops() },
        "GRADIENT_RADIAL" => PaintKind::RadialGradient { handles: handles(), stops: stops() },
        "GRADIENT_ANGULAR" => PaintKind::AngularGradient { handles: handles(), stops: stops() },
        "GRADIENT_DIAMOND" => PaintKind::DiamondGradient { handles: handles(), stops: stops() },
        "IMAGE" => PaintKind::Image {
            image_ref: fstr(v, "imageRef")?.to_string(),
            scale_mode: match fstr(v, "scaleMode") {
                Some("FIT") => ImageScaleMode::Fit,
                Some("TILE") => ImageScaleMode::Tile,
                Some("STRETCH") => ImageScaleMode::Crop,
                _ => ImageScaleMode::Fill,
            },
        },
        _ => return None, // EMOJI, VIDEO, ...
    };
    Some(Paint {
        kind,
        opacity: fnum(v, "opacity").unwrap_or(1.0),
        visible: fbool(v, "visible").unwrap_or(true),
        blend: fstr(v, "blendMode").map(blend).unwrap_or(BlendMode::Normal),
    })
}

pub(crate) fn parse_paints(v: Option<&Value>) -> Vec<Paint> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(parse_paint).collect()).unwrap_or_default()
}

fn parse_effect(v: &Value) -> Option<Effect> {
    let visible = fbool(v, "visible").unwrap_or(true);
    let radius = fnum(v, "radius").unwrap_or(0.0);
    let col = v.get("color").and_then(color).unwrap_or(Color::BLACK.with_alpha(0.25));
    let offset = v.get("offset").and_then(vec2).unwrap_or(Vec2::ZERO);
    let spread = fnum(v, "spread").unwrap_or(0.0);
    Some(match fstr(v, "type")? {
        "DROP_SHADOW" => Effect::DropShadow { color: col, offset, radius, spread, visible, show_behind: fbool(v, "showShadowBehindNode").unwrap_or(false) },
        "INNER_SHADOW" => Effect::InnerShadow { color: col, offset, radius, spread, visible },
        "LAYER_BLUR" => Effect::LayerBlur { radius, visible },
        "BACKGROUND_BLUR" => Effect::BackgroundBlur { radius, visible },
        // Noise / texture / glass: every field is optional; missing ones fall back to the
        // editor defaults. Names match what `export.rs` writes.
        "NOISE" => {
            let primary = v.get("color").and_then(color).unwrap_or(Color::BLACK);
            let mode = match fstr(v, "noiseType").map(str::to_ascii_uppercase).as_deref() {
                Some("DUOTONE") => NoiseMode::Duo { color: primary, secondary: v.get("secondaryColor").and_then(color).unwrap_or(Color::WHITE) },
                Some("MULTITONE") => NoiseMode::Multi,
                _ => NoiseMode::Mono { color: primary },
            };
            Effect::Noise(NoiseEffect {
                mode,
                size: fnum(v, "noiseSize").unwrap_or(1.0),
                density: fnum(v, "density").unwrap_or(0.5).clamp(0.0, 1.0),
                opacity: fnum(v, "opacity").unwrap_or(0.25).clamp(0.0, 1.0),
                blend: fstr(v, "blendMode").map(blend).unwrap_or(BlendMode::Normal),
                seed: fnum(v, "seed").unwrap_or(1.0).max(0.0) as u32,
                visible,
            })
        }
        "TEXTURE" => Effect::Texture(TextureEffect {
            size: fnum(v, "noiseSize").unwrap_or(2.0),
            radius: fnum(v, "radius").unwrap_or(2.0),
            clip_to_shape: fbool(v, "clipToShape").unwrap_or(true),
            seed: fnum(v, "seed").unwrap_or(1.0).max(0.0) as u32,
            visible,
        }),
        "GLASS" => Effect::Glass(GlassEffect {
            light_angle: fnum(v, "lightAngle").unwrap_or(315.0),
            light_intensity: fnum(v, "lightIntensity").unwrap_or(0.5).clamp(0.0, 1.0),
            refraction: fnum(v, "refraction").unwrap_or(0.5).clamp(0.0, 1.0),
            depth: fnum(v, "depth").unwrap_or(10.0),
            dispersion: fnum(v, "dispersion").unwrap_or(0.25).clamp(0.0, 1.0),
            frost: fnum(v, "radius").unwrap_or(4.0),
            visible,
        }),
        _ => return None,
    })
}

/// `exportSettings` of a node: format, suffix and the SCALE / WIDTH / HEIGHT constraint.
pub(crate) fn parse_export_settings(v: &Value) -> Vec<ExportPreset> {
    farr(v, "exportSettings")
        .iter()
        .map(|e| {
            let format = match fstr(e, "format").unwrap_or("PNG").to_ascii_uppercase().as_str() {
                "JPG" | "JPEG" => ExportFileFormat::Jpg,
                "SVG" => ExportFileFormat::Svg,
                "PDF" => ExportFileFormat::Pdf,
                _ => ExportFileFormat::Png,
            };
            let c = e.get("constraint");
            let value = c.and_then(|c| fnum(c, "value")).unwrap_or(1.0);
            let constraint = match c.and_then(|c| fstr(c, "type")) {
                Some("WIDTH") => ExportConstraint::Width(value),
                Some("HEIGHT") => ExportConstraint::Height(value),
                _ => ExportConstraint::Scale(value),
            };
            ExportPreset { format, constraint, suffix: fstr(e, "suffix").unwrap_or("").to_string() }
        })
        .collect()
}

/// `guides` of a page or frame: `[{axis: "X" | "Y", offset}]`.
fn parse_guides(v: &Value, frame: Option<NodeId>) -> Vec<Guide> {
    farr(v, "guides")
        .iter()
        .filter_map(|g| {
            let axis = match fstr(g, "axis")?.to_ascii_uppercase().as_str() {
                "X" => GuideAxis::X,
                "Y" => GuideAxis::Y,
                _ => return None,
            };
            Some(Guide { axis, offset: fnum(g, "offset")?, frame })
        })
        .collect()
}

fn parse_grid(v: &Value) -> Option<LayoutGrid> {
    if fbool(v, "visible") == Some(false) {
        return None; // the model has no per-grid visibility flag
    }
    let col = v.get("color").and_then(color).unwrap_or(Color::rgba(255, 0, 0, 25));
    let count = v.get("count").and_then(Value::as_u64).unwrap_or(5) as u32;
    let gutter = fnum(v, "gutterSize").unwrap_or(0.0);
    let offset = fnum(v, "offset").unwrap_or(0.0);
    match fstr(v, "pattern")? {
        "GRID" => Some(LayoutGrid::Grid { size: fnum(v, "sectionSize").unwrap_or(10.0), color: col }),
        "COLUMNS" => Some(LayoutGrid::Columns { count, gutter, offset, color: col }),
        "ROWS" => Some(LayoutGrid::Rows { count, gutter, offset, color: col }),
        _ => None,
    }
}

// ------------------------------------------------------------ text

/// Byte offset of each UTF-16 unit boundary: `out[u]` is the byte offset where
/// UTF-16 index `u` falls (low surrogates map to the end of their character).
pub(crate) fn utf16_to_byte_offsets(s: &str) -> Vec<usize> {
    let mut out = Vec::with_capacity(s.len() + 1);
    for (b, c) in s.char_indices() {
        out.push(b);
        if c.len_utf16() == 2 {
            out.push(b + c.len_utf8());
        }
    }
    out.push(s.len());
    out
}

/// Applies the (possibly partial) REST `TypeStyle` fields in `v` onto `ts`.
pub(crate) fn apply_type_style(ts: &mut TextStyle, v: &Value) {
    if let Some(f) = fstr(v, "fontFamily") {
        ts.font_family = f.to_string();
    }
    if let Some(w) = fnum(v, "fontWeight") {
        ts.font_weight = w.round().clamp(1.0, 1000.0) as u16;
    }
    if let Some(i) = fbool(v, "italic") {
        ts.italic = i;
    }
    if let Some(s) = fnum(v, "fontSize") {
        ts.font_size = s;
    }
    if let Some(a) = fstr(v, "textAlignHorizontal") {
        ts.align_h = match a {
            "CENTER" => TextAlignH::Center,
            "RIGHT" => TextAlignH::Right,
            "JUSTIFIED" => TextAlignH::Justified,
            _ => TextAlignH::Left,
        };
    }
    if let Some(a) = fstr(v, "textAlignVertical") {
        ts.align_v = match a {
            "CENTER" => TextAlignV::Center,
            "BOTTOM" => TextAlignV::Bottom,
            _ => TextAlignV::Top,
        };
    }
    if let Some(l) = fnum(v, "letterSpacing") {
        ts.letter_spacing = l;
    }
    if let Some(p) = fnum(v, "paragraphSpacing") {
        ts.paragraph_spacing = p;
    }
    // Percentage spacing from a partial `.fig` override: resolved against the final size.
    if let Some(p) = fnum(v, "letterSpacingPercent") {
        ts.letter_spacing = p / 100.0 * ts.font_size;
    }
    match fstr(v, "lineHeightUnit") {
        Some("PIXELS") => {
            if let Some(px) = fnum(v, "lineHeightPx") {
                ts.line_height = LineHeight::Pixels(px);
            }
        }
        Some("FONT_SIZE_%") => {
            if let Some(p) = fnum(v, "lineHeightPercentFontSize").or_else(|| fnum(v, "lineHeightPercent")) {
                ts.line_height = LineHeight::Percent(p);
            }
        }
        Some(_) => ts.line_height = LineHeight::Auto,
        None => {
            if let Some(px) = fnum(v, "lineHeightPx") {
                ts.line_height = LineHeight::Pixels(px);
            }
        }
    }
    if let Some(c) = fstr(v, "textCase") {
        ts.case = match c {
            "UPPER" => TextCase::Upper,
            "LOWER" => TextCase::Lower,
            "TITLE" => TextCase::Title,
            "SMALL_CAPS" | "SMALL_CAPS_FORCED" => TextCase::SmallCaps,
            _ => TextCase::Original,
        };
    }
    if let Some(d) = fstr(v, "textDecoration") {
        ts.decoration = match d {
            "UNDERLINE" => TextDecoration::Underline,
            "STRIKETHROUGH" => TextDecoration::Strikethrough,
            _ => TextDecoration::None,
        };
    }
    if let Some(f) = v.get("opentypeFlags") {
        crate::textfmt::merge_features(&mut ts.features, crate::textfmt::features_from_flags(f));
    }
    if let Some(fvs) = v.get("fontVariations") {
        if let Some(arr) = fvs.as_array() {
            for fv in arr {
                let Some(val) = fnum(fv, "value") else { continue };
                let tag = fstr(fv, "axisName").filter(|s| !s.is_empty()).map(String::from).or_else(|| {
                    let t = fv.get("axisTag")?.as_u64()? as u32;
                    let bytes = t.to_be_bytes();
                    std::str::from_utf8(&bytes).ok().map(|s| s.trim_end_matches(' ').to_string())
                });
                if let Some(tag) = tag.filter(|t| !t.is_empty()) {
                    ts.set_axis(&tag, val);
                }
            }
        } else if let Some(obj) = fvs.as_object() {
            for (tag, val) in obj {
                if let Some(val) = num(val) {
                    ts.set_axis(tag, val);
                }
            }
        }
    }
    if let Some(f) = v.get("fills") {
        ts.fills = parse_paints(Some(f));
    }
}

fn auto_resize(s: &str) -> TextAutoResize {
    match s {
        "WIDTH_AND_HEIGHT" => TextAutoResize::WidthAndHeight,
        "HEIGHT" => TextAutoResize::Height,
        "TRUNCATE" => TextAutoResize::Truncate,
        _ => TextAutoResize::None,
    }
}

// ------------------------------------------------------------ importer

struct Ctx<'a> {
    doc: Document,
    blobs: &'a [&'a [u8]],
    fast_derived_map: Option<&'a mut HashMap<String, Vec<crate::fig::FastDerivedEntry>>>,
    large_file: bool,
    ids: BTreeMap<String, NodeId>,
    /// (instance, REST componentId)
    instances: Vec<(NodeId, String)>,
    /// Instances whose component is not in this file (library components of
    /// other files), by REST componentId.
    unresolved: Vec<(NodeId, String)>,
    component_keys: BTreeMap<String, String>,
    /// `.fig` instance overrides / derived data, keyed by instance.
    overrides: BTreeMap<NodeId, Overrides>,
    next_clone: u64,
    /// `.fig` override keys (`_overrideKey`): the id override paths use for a
    /// node. Clones inherit the key of the node they copy.
    override_keys: HashMap<NodeId, std::sync::Arc<str>>,
    /// `.fig` `uniformScaleFactor` of instances made with the scale tool.
    scales: BTreeMap<NodeId, f32>,
    /// Library components and sets of `.fig` internal canvases: node ->
    /// (`componentKey`, `sourceLibraryKey`).
    library_keys: BTreeMap<NodeId, (String, Option<String>)>,
    /// Instance-swap property values naming a component by REST id:
    /// (node, property name, REST component id, is a definition default).
    pending_swaps: Vec<(NodeId, String, String, bool)>,
    /// REST `overrides` entries: (instance, node id inside it, field names).
    rest_overrides: Vec<(NodeId, String, Vec<String>)>,
    /// Prototype data to resolve once every node id is known.
    proto: Vec<(NodeId, Value)>,
    /// Overlay presentation of potential overlay destinations, by REST id.
    overlay_props: BTreeMap<String, Value>,
    /// `flowStartingPoints` of each page.
    flow_pages: Vec<(NodeId, Value)>,
    /// Memoized SVG path parses across base nodes and overrides.
    path_cache: HashMap<String, Path>,
    /// Memoized direct binary blob path parses.
    blob_cache: HashMap<(usize, bool), Path>,
    /// Components whose nested instances have already been expanded.
    expanded_comps: HashSet<NodeId>,
    /// Whether any node carries an instance-swap property value (set once before instances are expanded).
    has_instance_swap_values: bool,
    /// Whether any `CONNECTOR` node was encountered during import.
    has_connectors: bool,
    connectors: Vec<(NodeId, Option<String>, Option<String>)>,
    /// Deferred multi-page hydrator state assembled during `finish`.
    deferred_pending: Option<DeferredHydration>,
}

/// Overrides carried on `.fig` instance JSON as `_overrides` / `_derived`:
/// `[{"path": ["1:6", ...], "set": {<partial REST node fields>}}]`.
#[derive(Default, Debug)]
struct Overrides {
    set: Vec<OverrideEntry>,
    derived: Vec<OverrideEntry>,
    fast_derived: Vec<crate::fig::FastDerivedEntry>,
}

#[derive(Debug)]
struct OverrideEntry {
    path: Vec<String>,
    set: Value,
}

fn parse_override_entries(v: Option<&Value>) -> Vec<OverrideEntry> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    let path = e.get("path")?.as_array()?.iter().filter_map(|s| s.as_str().map(String::from)).collect();
                    Some(OverrideEntry { path, set: e.get("set")?.clone() })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn take_override_entries(v: Option<&mut Value>) -> Vec<OverrideEntry> {
    v.and_then(Value::as_array_mut)
        .map(|a| {
            std::mem::take(a)
                .into_iter()
                .filter_map(|mut e| {
                    let path = e
                        .get_mut("path")?
                        .as_array_mut()?
                        .iter_mut()
                        .filter_map(|p| match std::mem::take(p) {
                            Value::String(s) => Some(s),
                            _ => None,
                        })
                        .collect();
                    let set = std::mem::take(e.get_mut("set")?);
                    Some(OverrideEntry { path, set })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
struct ParentInfo {
    /// Absolute bounding-box origin of the parent, for the no-transform fallback.
    origin: Vec2,
    layout: Option<LayoutDirection>,
}

/// Parses REST JSON text (rejecting pathologically deep nesting).
pub(crate) fn parse_json_text(json: &str) -> Result<Value, FigError> {
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    for &b in json.as_bytes() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > 600 {
                    return Err(FigError::Unsupported("JSON nesting too deep".into()));
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    let mut de = serde_json::Deserializer::from_str(json);
    de.disable_recursion_limit();
    let v = serde::Deserialize::deserialize(&mut de)?;
    de.end()?;
    Ok(v)
}

pub fn import_rest_json(json: &str) -> Result<Document, FigError> {
    let v = parse_json_text(json)?;
    import_rest_value(&v, false)
}

/// Maps an already-parsed REST-shaped value. `expand_instances` clones the
/// component subtree into childless instances (used by `.fig`, where instance
/// children are often not stored).
pub(crate) fn import_rest_value(v: &Value, expand_instances: bool) -> Result<Document, FigError> {
    import_rest_value_with_blobs(v, expand_instances, &[], None)
}

pub(crate) fn import_rest_value_owned<'a>(
    mut v: Value,
    expand_instances: bool,
    blobs: &'a [&'a [u8]],
    fast_derived_map: Option<&'a mut HashMap<String, Vec<crate::fig::FastDerivedEntry>>>,
) -> Result<Document, FigError> {
    if crate::board::is_figjam(&v) {
        return import_rest_value_with_blobs(&v, expand_instances, blobs, fast_derived_map);
    }
    let has_doc = v.get("document").is_some();
    if !has_doc && fstr(&v, "type").is_none() {
        return Err(FigError::Format("REST JSON has no `document`".into()));
    }
    let doc_name = fstr(&v, "name").unwrap_or("Untitled").to_string();
    let large_file = fast_derived_map.is_some();
    let mut cx = Ctx {
        doc: Document::new(&doc_name),
        blobs,
        fast_derived_map,
        large_file,
        ids: BTreeMap::new(),
        instances: Vec::new(),
        unresolved: Vec::new(),
        component_keys: BTreeMap::new(),
        overrides: BTreeMap::new(),
        next_clone: 0,
        override_keys: HashMap::new(),
        scales: BTreeMap::new(),
        library_keys: BTreeMap::new(),
        pending_swaps: Vec::new(),
        rest_overrides: Vec::new(),
        proto: Vec::new(),
        overlay_props: BTreeMap::new(),
        flow_pages: Vec::new(),
        path_cache: HashMap::new(),
        blob_cache: HashMap::with_capacity(blobs.len()),
        expanded_comps: HashSet::new(),
        has_instance_swap_values: false,
        has_connectors: false,
        connectors: Vec::new(),
        deferred_pending: None,
    };
    if let Some(comps) = v.get("components").and_then(Value::as_object) {
        for (id, c) in comps {
            if let Some(k) = fstr(c, "key") {
                cx.component_keys.insert(id.clone(), k.to_string());
            }
        }
    }
    let pages: Vec<Value> = if has_doc {
        let root = v.get_mut("document").unwrap();
        if fstr(root, "type") == Some("CANVAS") {
            vec![std::mem::take(root)]
        } else {
            root.get_mut("children")
                .and_then(Value::as_array_mut)
                .map(|arr| std::mem::take(arr).into_iter().filter(|c| fstr(c, "type") == Some("CANVAS")).collect())
                .unwrap_or_default()
        }
    } else if fstr(&v, "type") == Some("CANVAS") {
        vec![std::mem::take(&mut v)]
    } else {
        v.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(|arr| std::mem::take(arr).into_iter().filter(|c| fstr(c, "type") == Some("CANVAS")).collect())
            .unwrap_or_default()
    };
    for p in pages {
        cx.import_page_owned(p);
    }
    let library_pages: Vec<Value> = v.get_mut("libraryPages").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
    if !library_pages.is_empty() {
        let lib = ensure_library_page(&mut cx.doc);
        let info = ParentInfo { origin: Vec2::ZERO, layout: None };
        for mut p in library_pages {
            let children: Vec<Value> = p.get_mut("children").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
            drop(p);
            for c in children {
                cx.import_node_owned(c, lib, info, 0);
            }
        }
    }
    if cx.doc.nodes.len() > 10_000 {
        crate::fig::release_os_memory();
    }
    cx.finish(expand_instances);
    cx.register_library_assets();
    cx.doc.kind = DocumentKind::Design;
    if !cx.connectors.is_empty() {
        crate::board::link_connector_list(&mut cx.doc, &cx.ids, &cx.connectors);
    }
    cx.link_remote_components(&v);
    crate::variables::import_variables_value(&mut cx.doc, &v);
    apply_style_node_values(&mut cx.doc, v.get("styles"));
    cx.doc.style_meta = style_meta(v.get("styles"), &cx.doc);
    drop(v);
    cx.install_deferred_hydrator();
    let doc = cx.doc;
    drop(cx.override_keys);
    drop(cx.ids);
    drop(cx.instances);
    drop(cx.blob_cache);
    drop(cx.path_cache);
    if doc.nodes.len() > 10_000 {
        crate::fig::release_os_memory();
    }
    Ok(doc)
}

pub(crate) fn import_rest_value_with_blobs<'a>(
    v: &'a Value,
    expand_instances: bool,
    blobs: &'a [&'a [u8]],
    fast_derived_map: Option<&'a mut HashMap<String, Vec<crate::fig::FastDerivedEntry>>>,
) -> Result<Document, FigError> {
    let root = match v.get("document") {
        Some(d) => d,
        None if fstr(v, "type").is_some() => v,
        None => return Err(FigError::Format("REST JSON has no `document`".into())),
    };
    let large_file = fast_derived_map.is_some();
    let mut cx = Ctx {
        doc: Document::new(fstr(v, "name").unwrap_or("Untitled")),
        blobs,
        fast_derived_map,
        large_file,
        ids: BTreeMap::new(),
        instances: Vec::new(),
        unresolved: Vec::new(),
        component_keys: BTreeMap::new(),
        overrides: BTreeMap::new(),
        next_clone: 0,
        override_keys: HashMap::new(),
        scales: BTreeMap::new(),
        library_keys: BTreeMap::new(),
        pending_swaps: Vec::new(),
        rest_overrides: Vec::new(),
        proto: Vec::new(),
        overlay_props: BTreeMap::new(),
        flow_pages: Vec::new(),
        path_cache: HashMap::new(),
        blob_cache: HashMap::with_capacity(blobs.len()),
        expanded_comps: HashSet::new(),
        has_instance_swap_values: false,
        has_connectors: false,
        connectors: Vec::new(),
        deferred_pending: None,
    };
    if let Some(comps) = v.get("components").and_then(Value::as_object) {
        for (id, c) in comps {
            if let Some(k) = fstr(c, "key") {
                cx.component_keys.insert(id.clone(), k.to_string());
            }
        }
    }
    let pages: Vec<&Value> = match fstr(root, "type") {
        Some("CANVAS") => vec![root],
        _ => farr(root, "children").iter().filter(|c| fstr(c, "type") == Some("CANVAS")).collect(),
    };
    for p in pages {
        cx.import_page(p);
    }
    // `.fig` internal-only canvases: what the file uses from libraries.
    let library_pages = farr(v, "libraryPages");
    if !library_pages.is_empty() {
        let lib = ensure_library_page(&mut cx.doc);
        let info = ParentInfo { origin: Vec2::ZERO, layout: None };
        for p in library_pages {
            for c in farr(p, "children") {
                cx.import_node(c, lib, info, 0);
            }
        }
    }
    cx.finish(expand_instances);
    cx.register_library_assets();
    cx.doc.kind = if crate::board::is_figjam(v) { DocumentKind::Board } else { DocumentKind::Design };
    if cx.doc.kind == DocumentKind::Board {
        // a board draws its own dotted canvas: the pages are transparent over it
        for p in cx.doc.pages.clone() {
            if let Some(NodeKind::Page { background }) = cx.doc.get_mut(p).map(|n| &mut n.kind) {
                background.a = 0.0;
            }
        }
    }
    if cx.has_connectors {
        crate::board::link_connectors(&mut cx.doc, &cx.ids, root);
    }
    cx.link_remote_components(v);
    crate::variables::import_variables_value(&mut cx.doc, v);
    apply_style_node_values(&mut cx.doc, v.get("styles"));
    cx.doc.style_meta = style_meta(v.get("styles"), &cx.doc);
    cx.install_deferred_hydrator();
    let doc = cx.doc;
    drop(cx.override_keys);
    drop(cx.ids);
    drop(cx.instances);
    drop(cx.blob_cache);
    drop(cx.path_cache);
    if doc.nodes.len() > 10_000 {
        crate::fig::release_os_memory();
    }
    Ok(doc)
}

impl<'a> Ctx<'a> {
    fn import_page_owned(&mut self, mut v: Value) {
        let pid = self.doc.add_page(fstr(&v, "name").unwrap_or("Page"));
        if let Some(src) = fstr(&v, "id") {
            self.ids.insert(src.to_string(), pid);
        }
        let bg = v.get("backgroundColor").and_then(color).unwrap_or(Color::rgb(0xE5, 0xE5, 0xE5));
        if let Some(n) = self.doc.get_mut(pid) {
            n.kind = NodeKind::Page { background: bg.with_alpha(1.0) };
            n.source_id = fstr(&v, "id").map(String::from);
        }
        if let Some(fl) = v.get_mut("flowStartingPoints").map(std::mem::take) {
            self.flow_pages.push((pid, serde_json::json!({ "flowStartingPoints": fl })));
        }
        let guides = parse_guides(&v, None);
        if !guides.is_empty() {
            self.doc.guides.entry(pid).or_default().extend(guides);
        }
        let children: Vec<Value> = v.get_mut("children").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
        drop(v);
        let info = ParentInfo { origin: Vec2::ZERO, layout: None };
        for c in children {
            self.import_node_owned(c, pid, info, 0);
        }
    }

    fn import_node_owned(&mut self, mut v: Value, parent: NodeId, pinfo: ParentInfo, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let Some(ty_str) = fstr(&v, "type").map(String::from) else { return };
        let ty = ty_str.as_str();
        if ty == "CANVAS" || ty == "DOCUMENT" {
            return;
        }
        let children: Vec<Value> = v.get_mut("children").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
        let id = self.doc.alloc_id();
        let (mut node, my_layout) = self.build_node(id, ty, &v, pinfo);
        node.source_id = fstr(&v, "id").map(String::from);
        if let Some(src) = &node.source_id {
            self.ids.insert(src.clone(), id);
        }
        crate::proto::apply_scroll(&mut node, &v);
        if let Some(k) = fstr(&v, "_overrideKey") {
            self.override_keys.insert(id, std::sync::Arc::from(k));
        }
        if let (Some(k), true) = (fstr(&v, "componentKey"), matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))) {
            self.library_keys.insert(id, (k.to_string(), fstr(&v, "sourceLibraryKey").map(String::from)));
        }
        let is_instance = matches!(node.kind, NodeKind::Instance { .. });
        let origin = abs_origin(&v).unwrap_or(pinfo.origin);
        let container = node.kind.is_container();
        let is_frame = node.kind.frame_props().is_some();
        if self.doc.insert(parent, node).is_err() {
            return;
        }
        if is_frame {
            let guides = parse_guides(&v, Some(id));
            if let (false, Some(page)) = (guides.is_empty(), self.doc.page_of(id)) {
                self.doc.guides.entry(page).or_default().extend(guides);
            }
        }
        if let Some(raw) = crate::proto::collect(&v) {
            self.proto.push((id, raw));
        }
        if let Some(o) = crate::proto::collect_overlay(&v) {
            if let Some(src) = fstr(&v, "id") {
                self.overlay_props.insert(src.to_string(), o);
            }
        }
        if crate::proto::is_fixed(&v) {
            if let Some(p) = self.doc.get_mut(parent) {
                p.prototype.fixed_children.push(id);
            }
        }
        if is_instance {
            if let Some(cid) = fstr(&v, "componentId") {
                self.instances.push((id, cid.to_string()));
            }
            if let Some(s) = fnum(&v, "_uniformScale").filter(|s| *s > 0.0 && (*s - 1.0).abs() > 1e-4) {
                self.scales.insert(id, s);
            }
            for e in farr(&v, "overrides") {
                if let Some(nid) = fstr(e, "id") {
                    let fields: Vec<String> = farr(e, "overriddenFields").iter().filter_map(|f| f.as_str().map(String::from)).collect();
                    self.rest_overrides.push((id, nid.to_string(), fields));
                }
            }
            let fast_derived = fstr(&v, "id").and_then(|sid| self.fast_derived_map.as_deref_mut().and_then(|m| m.remove(sid))).unwrap_or_default();
            let ov = Overrides { set: take_override_entries(v.get_mut("_overrides")), derived: take_override_entries(v.get_mut("_derived")), fast_derived };
            if !ov.set.is_empty() || !ov.derived.is_empty() || !ov.fast_derived.is_empty() {
                self.overrides.insert(id, ov);
            }
        }
        drop(v);
        if container {
            let info = ParentInfo { origin, layout: my_layout };
            for c in children {
                self.import_node_owned(c, id, info, depth + 1);
            }
        }
    }

    fn import_page(&mut self, v: &'a Value) {
        let pid = self.doc.add_page(fstr(v, "name").unwrap_or("Page"));
        if let Some(src) = fstr(v, "id") {
            self.ids.insert(src.to_string(), pid);
        }
        let bg = v.get("backgroundColor").and_then(color).unwrap_or(Color::rgb(0xE5, 0xE5, 0xE5));
        if let Some(n) = self.doc.get_mut(pid) {
            n.kind = NodeKind::Page { background: bg.with_alpha(1.0) };
            n.source_id = fstr(v, "id").map(String::from);
        }
        if let Some(fl) = v.get("flowStartingPoints") {
            self.flow_pages.push((pid, serde_json::json!({ "flowStartingPoints": fl })));
        }
        let guides = parse_guides(v, None);
        if !guides.is_empty() {
            self.doc.guides.entry(pid).or_default().extend(guides);
        }
        let info = ParentInfo { origin: Vec2::ZERO, layout: None };
        for c in farr(v, "children") {
            self.import_node(c, pid, info, 0);
        }
    }

    fn import_node(&mut self, v: &'a Value, parent: NodeId, pinfo: ParentInfo, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let Some(ty) = fstr(v, "type") else { return };
        if ty == "CANVAS" || ty == "DOCUMENT" {
            return;
        }
        let id = self.doc.alloc_id();
        let (mut node, my_layout) = self.build_node(id, ty, v, pinfo);
        node.source_id = fstr(v, "id").map(String::from);
        if let Some(src) = &node.source_id {
            self.ids.insert(src.clone(), id);
        }
        crate::proto::apply_scroll(&mut node, v);
        if let Some(k) = fstr(v, "_overrideKey") {
            self.override_keys.insert(id, std::sync::Arc::from(k));
        }
        if let (Some(k), true) = (fstr(v, "componentKey"), matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))) {
            self.library_keys.insert(id, (k.to_string(), fstr(v, "sourceLibraryKey").map(String::from)));
        }
        let is_instance = matches!(node.kind, NodeKind::Instance { .. });
        let origin = abs_origin(v).unwrap_or(pinfo.origin);
        let container = node.kind.is_container();
        let is_frame = node.kind.frame_props().is_some();
        if self.doc.insert(parent, node).is_err() {
            return;
        }
        if is_frame {
            let guides = parse_guides(v, Some(id));
            if let (false, Some(page)) = (guides.is_empty(), self.doc.page_of(id)) {
                self.doc.guides.entry(page).or_default().extend(guides);
            }
        }
        if let Some(raw) = crate::proto::collect(v) {
            self.proto.push((id, raw));
        }
        if let Some(o) = crate::proto::collect_overlay(v) {
            if let Some(src) = fstr(v, "id") {
                self.overlay_props.insert(src.to_string(), o);
            }
        }
        if crate::proto::is_fixed(v) {
            if let Some(p) = self.doc.get_mut(parent) {
                p.prototype.fixed_children.push(id);
            }
        }
        if is_instance {
            if let Some(cid) = fstr(v, "componentId") {
                self.instances.push((id, cid.to_string()));
            }
            if let Some(s) = fnum(v, "_uniformScale").filter(|s| *s > 0.0 && (*s - 1.0).abs() > 1e-4) {
                self.scales.insert(id, s);
            }
            for e in farr(v, "overrides") {
                if let Some(nid) = fstr(e, "id") {
                    let fields: Vec<String> = farr(e, "overriddenFields").iter().filter_map(|f| f.as_str().map(String::from)).collect();
                    self.rest_overrides.push((id, nid.to_string(), fields));
                }
            }
            let fast_derived = fstr(v, "id").and_then(|sid| self.fast_derived_map.as_deref_mut().and_then(|m| m.remove(sid))).unwrap_or_default();
            let ov = Overrides { set: parse_override_entries(v.get("_overrides")), derived: parse_override_entries(v.get("_derived")), fast_derived };
            if !ov.set.is_empty() || !ov.derived.is_empty() || !ov.fast_derived.is_empty() {
                self.overrides.insert(id, ov);
            }
        }
        if container {
            let info = ParentInfo { origin, layout: my_layout };
            for c in farr(v, "children") {
                self.import_node(c, id, info, depth + 1);
            }
        }
    }

    /// Returns the node and, for auto-layout frames, its direction.
    fn build_node(&mut self, id: NodeId, ty: &str, v: &Value, pinfo: ParentInfo) -> (Node, Option<LayoutDirection>) {
        let frame = || frame_props(v);
        let skip_geo = ty == "TEXT" || (self.large_file && matches!(ty, "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" | "SECTION" | "RECTANGLE"));
        // Text outlines (glyph paths) are not stored; the renderer lays text out itself.
        let mut fill_geo = (!skip_geo)
            .then(|| parse_geometry_cached(farr(v, "fillGeometry"), &mut self.path_cache, &mut self.blob_cache, self.blobs))
            .filter(|g| !g.is_empty());
        let stroke_geo = (!skip_geo)
            .then(|| parse_geometry_cached(farr(v, "strokeGeometry"), &mut self.path_cache, &mut self.blob_cache, self.blobs))
            .filter(|g| !g.is_empty());
        let kind = match ty {
            "FRAME" => NodeKind::Frame(frame()),
            "GROUP" => NodeKind::Group,
            "SECTION" => NodeKind::Section,
            "STICKY" | "SHAPE_WITH_TEXT" | "STAMP" | "CONNECTOR" => {
                if ty == "CONNECTOR" {
                    self.has_connectors = true;
                    let start_ep = v.get("connectorStart").and_then(|e| fstr(e, "endpointNodeId")).map(String::from);
                    let end_ep = v.get("connectorEnd").and_then(|e| fstr(e, "endpointNodeId")).map(String::from);
                    self.connectors.push((id, start_ep, end_ep));
                }
                crate::board::kind_from_json(ty, v, pinfo.origin).unwrap_or(NodeKind::Unknown { type_name: ty.to_string() })
            }
            "COMPONENT" => NodeKind::Component(frame()),
            "COMPONENT_SET" => NodeKind::ComponentSet(frame()),
            "INSTANCE" => NodeKind::Instance {
                frame: frame(),
                component: None,
                component_key: fstr(v, "componentId").and_then(|c| self.component_keys.get(c).cloned()).or_else(|| fstr(v, "componentKey").map(String::from)),
            },
            "RECTANGLE" => NodeKind::Rectangle,
            "ELLIPSE" => {
                let a = v.get("arcData");
                let d = ArcData::default();
                NodeKind::Ellipse(ArcData {
                    start: a.and_then(|a| fnum(a, "startingAngle")).unwrap_or(d.start),
                    end: a.and_then(|a| fnum(a, "endingAngle")).unwrap_or(d.end),
                    inner_radius: a.and_then(|a| fnum(a, "innerRadius")).unwrap_or(0.0),
                })
            }
            "LINE" => NodeKind::Line,
            "REGULAR_POLYGON" => NodeKind::Polygon { count: v.get("pointCount").and_then(Value::as_u64).unwrap_or(3).clamp(3, 1000) as u32 },
            "STAR" => NodeKind::Star {
                count: v.get("pointCount").and_then(Value::as_u64).unwrap_or(5).clamp(3, 1000) as u32,
                inner_ratio: fnum(v, "starInnerRadius").unwrap_or(0.382),
            },
            "VECTOR" | "BRUSH" => {
                // an editable network when the file carries one; Figma's own outline stays the rendered geometry
                let network = v.get("vectorNetwork").and_then(crate::vecnet::network_from_json).filter(|n| !n.vertices.is_empty());
                let paths = if self.large_file && network.is_none() {
                    fill_geo.take().or_else(|| stroke_geo.clone()).unwrap_or_default()
                } else {
                    fill_geo.clone().or_else(|| stroke_geo.clone()).or_else(|| network.as_ref().map(|n| n.to_paths())).unwrap_or_default()
                };
                NodeKind::Vector { paths, network }
            }
            "TEXT" => NodeKind::Text(text_props(v)),
            "BOOLEAN_OPERATION" => NodeKind::BooleanOp(match fstr(v, "booleanOperation") {
                Some("SUBTRACT") => BooleanOp::Subtract,
                Some("INTERSECT") => BooleanOp::Intersect,
                Some("EXCLUDE") => BooleanOp::Exclude,
                _ => BooleanOp::Union,
            }),
            other => NodeKind::Unknown { type_name: other.to_string() },
        };
        let is_text = matches!(kind, NodeKind::Text(_));
        let my_layout = kind.frame_props().and_then(|f| f.auto_layout).map(|a| a.direction);
        let default_blend = if matches!(kind, NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Line | NodeKind::Vector { .. } | NodeKind::Text(_)) {
            BlendMode::Normal
        } else {
            BlendMode::PassThrough
        };
        let mut n = Node::new(id, kind);
        if let Some(s) = fstr(v, "name") {
            n.name = s.to_string();
        }
        n.visible = fbool(v, "visible").unwrap_or(true);
        n.locked = fbool(v, "locked").unwrap_or(false);
        n.lock_aspect = fbool(v, "preserveRatio").or_else(|| fbool(v, "lockAspect")).unwrap_or(false);
        n.opacity = fnum(v, "opacity").unwrap_or(1.0);
        n.variables = crate::variables::parse_node_variables(v);
        n.blend = fstr(v, "blendMode").map(blend).unwrap_or(default_blend);
        n.is_mask = fbool(v, "isMask").unwrap_or(false);
        if let Some(hl) = fstr(v, "hyperlink").filter(|s| !s.is_empty()) {
            n.hyperlink = Some(hl.to_string());
        }
        if let Some(pd) = v.get("pluginData").and_then(Value::as_object) {
            for (k, val) in pd {
                if let Some(s) = val.as_str() {
                    n.plugin_data.insert(k.clone(), s.to_string());
                }
            }
        }
        if let Some(desc) = fstr(v, "symbolDescription").or_else(|| fstr(v, "description")).filter(|s| !s.is_empty()) {
            n.component.description = desc.to_string();
        }
        if let Some(bub) = fbool(v, "propsAreBubbled") {
            n.component.expose_nested_instances = bub;
        }
        if let Some(lvl) = v.get("overrideLevel").and_then(Value::as_i64) {
            n.component.override_level = Some(lvl as i32);
        }
        if let Some(ds) = fstr(v, "detachedSymbolId").filter(|s| !s.is_empty()) {
            n.detached_symbol_id = Some(ds.to_string());
            n.component.detached_symbol_id = Some(ds.to_string());
        }
        if let Some(fm) = v.get("figMetadata").and_then(Value::as_object) {
            for (k, val) in fm {
                if let Some(s) = val.as_str() {
                    n.fig_metadata.insert(k.clone(), s.to_string());
                }
            }
        }
        if ty == "BRUSH" {
            let bt = fstr(v, "brushType").unwrap_or("STRETCH").to_string();
            n.fig_metadata.entry("brushType".into()).or_insert(bt);
        } else if let Some(bt) = fstr(v, "brushType").filter(|s| !s.is_empty()) {
            n.fig_metadata.insert("brushType".into(), bt.to_string());
        }
        if ty == "SHAPE_WITH_TEXT" {
            if let Some(swt) = fstr(v, "shapeWithTextType").or_else(|| fstr(v, "shapeType")).filter(|s| !s.is_empty()) {
                n.fig_metadata.entry("shapeWithTextType".into()).or_insert_with(|| swt.to_string());
            }
        }
        n.export_settings = parse_export_settings(v);
        n.dev = crate::dev::parse_dev(v);
        n.fill_geometry = fill_geo;
        n.stroke_geometry = stroke_geo;

        // Placement.
        let bbox = v.get("absoluteBoundingBox");
        n.transform = match v.get("relativeTransform").and_then(parse_transform) {
            Some(t) => t,
            None => match bbox {
                Some(b) => Affine::translate(fnum(b, "x").unwrap_or(0.0) - pinfo.origin.x, fnum(b, "y").unwrap_or(0.0) - pinfo.origin.y),
                None => Affine::IDENTITY,
            },
        };
        n.size = v.get("size").and_then(vec2).or_else(|| bbox.and_then(|b| Some(Vec2::new(fnum(b, "width")?, fnum(b, "height")?)))).unwrap_or(Vec2::ZERO);

        // Paint & stroke.
        let fills = parse_paints(v.get("fills"));
        // Text keeps its color in `TextStyle::fills` (set by `text_props`).
        if !is_text {
            n.fills = fills;
        }
        n.stroke.paints = parse_paints(v.get("strokes"));
        if let Some(w) = fnum(v, "strokeWeight") {
            n.stroke.weight = w;
        }
        if let Some(i) = v.get("individualStrokeWeights") {
            let g = |k: &str| fnum(i, k).unwrap_or(0.0);
            n.stroke.side_weights = Some([g("top"), g("right"), g("bottom"), g("left")]);
        }
        n.stroke.align = match fstr(v, "strokeAlign") {
            Some("CENTER") => StrokeAlign::Center,
            Some("OUTSIDE") => StrokeAlign::Outside,
            _ => StrokeAlign::Inside,
        };
        n.stroke.cap = match fstr(v, "strokeCap") {
            Some("ROUND") => StrokeCap::Round,
            Some("SQUARE") => StrokeCap::Square,
            _ => StrokeCap::None,
        };
        n.stroke.join = match fstr(v, "strokeJoin") {
            Some("BEVEL") => StrokeJoin::Bevel,
            Some("ROUND") => StrokeJoin::Round,
            _ => StrokeJoin::Miter,
        };
        if let Some(l) = fnum(v, "strokeMiterLimit") {
            n.stroke.miter_limit = l;
        } else if let Some(deg) = fnum(v, "strokeMiterAngle") {
            let s = (deg.to_radians() / 2.0).sin();
            if s > 1e-4 {
                n.stroke.miter_limit = 1.0 / s;
            }
        }
        n.stroke.dashes = farr(v, "strokeDashes").iter().filter_map(num).collect();
        n.effects = farr(v, "effects").iter().filter_map(parse_effect).collect();

        // Corners.
        if let Some(r) = v.get("rectangleCornerRadii").and_then(Value::as_array).filter(|a| a.len() == 4) {
            for (i, x) in r.iter().enumerate() {
                n.corner_radii[i] = num(x).unwrap_or(0.0);
            }
        } else if let Some(r) = fnum(v, "cornerRadius") {
            n.corner_radii = [r; 4];
        }
        n.corner_smoothing = fnum(v, "cornerSmoothing").unwrap_or(0.0);

        // Constraints.
        if let Some(c) = v.get("constraints") {
            n.constraints = Constraints {
                horizontal: match fstr(c, "horizontal") {
                    Some("RIGHT") => Constraint::Max,
                    Some("CENTER") => Constraint::Center,
                    Some("LEFT_RIGHT") => Constraint::Stretch,
                    Some("SCALE") => Constraint::Scale,
                    _ => Constraint::Min,
                },
                vertical: match fstr(c, "vertical") {
                    Some("BOTTOM") => Constraint::Max,
                    Some("CENTER") => Constraint::Center,
                    Some("TOP_BOTTOM") => Constraint::Stretch,
                    Some("SCALE") => Constraint::Scale,
                    _ => Constraint::Min,
                },
            };
        }

        n.layout_child = layout_child(v, pinfo.layout, &n.kind);

        // Style references and library style definitions.
        if let Some(styles) = v.get("styles").and_then(Value::as_object) {
            for (k, sid) in styles {
                let Some(sid) = sid.as_str() else { continue };
                let key = match k.as_str() {
                    "fills" | "fill" => "fill",
                    "strokes" | "stroke" => "stroke",
                    "text" => "text",
                    "effect" | "effects" => "effect",
                    "grid" | "grids" => "grid",
                    other => other,
                };
                n.style_refs.insert(key.to_string(), sid.to_string());
                if self.doc.styles.contains_key(sid) {
                    continue;
                }
                let def = match key {
                    "fill" => Some(Style::Fill(match &n.kind {
                        NodeKind::Text(t) => t.style.fills.clone(),
                        _ => n.fills.clone(),
                    })),
                    "stroke" => Some(Style::Fill(n.stroke.paints.clone())),
                    "text" => match &n.kind {
                        NodeKind::Text(t) => Some(Style::Text(t.style.clone())),
                        _ => None,
                    },
                    "effect" => Some(Style::Effect(n.effects.clone())),
                    "grid" => n.kind.frame_props().map(|f| Style::Grid(f.grids.clone())),
                    _ => None,
                };
                if let Some(d) = def {
                    self.doc.styles.insert(sid.to_string(), d);
                }
            }
        }
        self.parse_component_props(&mut n, v);
        crate::board::finish_node(&mut n);
        (n, my_layout)
    }

    /// `componentPropertyDefinitions` / `componentProperties` /
    /// `componentPropertyReferences` -> `props` / `values` / `bindings`.
    fn parse_component_props(&mut self, n: &mut Node, v: &Value) {
        let id = n.id;
        if let Some(defs) = v.get("componentPropertyDefinitions").and_then(Value::as_object) {
            for (key, d) in defs {
                let name = prop_name(key);
                let dv = d.get("defaultValue");
                let text = || dv.and_then(Value::as_str).unwrap_or("").to_string();
                let (kind, default) = match fstr(d, "type") {
                    Some("BOOLEAN") => (PropKind::Boolean, PropValue::Bool(dv.and_then(Value::as_bool).unwrap_or(true))),
                    Some("TEXT") => (PropKind::Text, PropValue::Text(text())),
                    Some("VARIANT") => (
                        PropKind::Variant { options: farr(d, "variantOptions").iter().filter_map(|o| o.as_str().map(String::from)).collect() },
                        PropValue::Variant(text()),
                    ),
                    Some("INSTANCE_SWAP") => {
                        self.pending_swaps.push((id, name.clone(), text(), true));
                        for p in farr(d, "preferredValues") {
                            if let Some(ps) = p.as_str().or_else(|| fstr(p, "key")) {
                                self.pending_swaps.push((id, name.clone(), format!("PREF:{ps}"), true));
                            }
                        }
                        (PropKind::InstanceSwap { preferred: Vec::new() }, PropValue::Instance(NodeId(0)))
                    }
                    Some("SLOT") => (PropKind::Slot, PropValue::Slot(Vec::new())),
                    _ => continue,
                };
                n.component.props.push(ComponentPropDef { name, kind, default });
            }
        }
        if let Some(vals) = v.get("componentProperties").and_then(Value::as_object) {
            for (key, d) in vals {
                let name = prop_name(key);
                let val = d.get("value");
                let text = || val.and_then(Value::as_str).unwrap_or("").to_string();
                let pv = match fstr(d, "type") {
                    Some("BOOLEAN") => PropValue::Bool(val.and_then(Value::as_bool).unwrap_or(true)),
                    Some("TEXT") => PropValue::Text(text()),
                    Some("VARIANT") => PropValue::Variant(text()),
                    Some("INSTANCE_SWAP") => {
                        self.pending_swaps.push((id, name.clone(), text(), false));
                        PropValue::Instance(NodeId(0))
                    }
                    Some("SLOT") => {
                        let s = text();
                        self.pending_swaps.push((id, name.clone(), format!("SLOT:{s}"), false));
                        PropValue::Slot(Vec::new())
                    }
                    _ => continue,
                };
                n.component.values.insert(name, pv);
            }
        }
        if let Some(refs) = v.get("componentPropertyReferences").and_then(Value::as_object) {
            for (k, p) in refs {
                let field = match k.as_str() {
                    "visible" => OverrideField::Visible,
                    "characters" => OverrideField::Characters,
                    "mainComponent" => OverrideField::MainComponent,
                    "slotContent" => OverrideField::SlotContent,
                    _ => continue,
                };
                if let Some(p) = p.as_str() {
                    n.component.bindings.insert(field, prop_name(p));
                }
            }
        }
        if let Some(vspecs) = v.get("variantPropSpecs") {
            if let Some(obj) = vspecs.as_object() {
                for (k, val) in obj {
                    if let Some(s) = val.as_str() {
                        n.component.variant_props.insert(k.clone(), s.to_string());
                    }
                }
            } else if let Some(arr) = vspecs.as_array() {
                for s in arr {
                    let Some(val) = fstr(s, "value") else { continue };
                    let Some(k) = fstr(s, "propertyName").or_else(|| fstr(s, "name")) else { continue };
                    n.component.variant_props.insert(k.to_string(), val.to_string());
                }
            }
        }
    }

    fn finish(&mut self, expand_instances: bool) {
        let links = std::mem::take(&mut self.instances);
        let mut targets: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        for (inst, cid) in links {
            let Some(&comp) = self.ids.get(&cid) else {
                self.unresolved.push((inst, cid));
                continue;
            };
            if let Some(NodeKind::Instance { component, .. }) = self.doc.get_mut(inst).map(|n| &mut n.kind) {
                *component = Some(comp);
            }
            targets.insert(inst, comp);
        }
        self.link_sources(&targets);
        self.resolve_swaps();
        if !expand_instances {
            self.apply_rest_overrides();
            self.resolve_proto();
            return;
        }
        // Clone ids are handed out sequentially above every imported id, in
        // instance order, so the same input always yields the same ids.
        self.next_clone = self.doc.nodes.keys().next_back().map(|k| k.0).unwrap_or(0) + 1;
        self.has_instance_swap_values = self.doc.nodes.values().any(|n| n.component.values.values().any(|v| matches!(v, PropValue::Instance(t) if t.0 != 0)));
        let mut done = BTreeSet::new();
        let defer_inactive = self.large_file && self.doc.nodes.len() > 25_000 && self.doc.pages.len() > 2;
        if defer_inactive {
            let mut node_page: HashMap<NodeId, NodeId> = HashMap::with_capacity(self.doc.nodes.len());
            let mut page_counts: HashMap<NodeId, usize> = HashMap::with_capacity(self.doc.pages.len() + 1);
            let mut stack = Vec::new();
            for &pid in self.doc.pages.iter().chain(self.doc.library_page.iter()) {
                stack.push(pid);
                let mut cnt = 0usize;
                while let Some(nid) = stack.pop() {
                    node_page.insert(nid, pid);
                    cnt += 1;
                    if let Some(n) = self.doc.get(nid) {
                        stack.extend(n.children.iter().copied());
                    }
                }
                page_counts.insert(pid, cnt);
            }
            let mut initial_pages: HashSet<NodeId> = HashSet::new();
            if let Some(&first_p) = self.doc.pages.first() {
                initial_pages.insert(first_p);
            }
            if let Some(&non_empty_p) = self.doc.pages.iter().find(|&&p| page_counts.get(&p).copied().unwrap_or(0) >= 50) {
                initial_pages.insert(non_empty_p);
            }
            if let Some(&largest_p) = self.doc.pages.iter().max_by_key(|&&p| page_counts.get(&p).copied().unwrap_or(0)) {
                initial_pages.insert(largest_p);
            }
            if let Some(lp) = self.doc.library_page {
                initial_pages.insert(lp);
            }
            let all_comps: BTreeSet<NodeId> = targets.values().copied().collect();
            for comp in all_comps {
                self.expand_nested(comp, &targets, &mut done);
            }
            let mut pending_by_page: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
            let order: Vec<NodeId> = targets.keys().copied().collect();
            for inst in order {
                if done.contains(&inst) {
                    continue;
                }
                match node_page.get(&inst).copied() {
                    Some(p) if !initial_pages.contains(&p) => {
                        pending_by_page.entry(p).or_default().push(inst);
                    }
                    _ => {
                        self.expand_rec(inst, &targets, &mut done);
                    }
                }
            }
            let rest: Vec<NodeId> = self.overrides.keys().copied().filter(|i| !done.contains(i)).collect();
            for inst in rest {
                match node_page.get(&inst).copied() {
                    Some(p) if !initial_pages.contains(&p) => {
                        let list = pending_by_page.entry(p).or_default();
                        if !list.contains(&inst) {
                            list.push(inst);
                        }
                    }
                    _ => {
                        self.apply_overrides(inst, &targets, &mut done);
                    }
                }
            }
            if !pending_by_page.is_empty() {
                self.deferred_pending = Some((pending_by_page, initial_pages, targets, done));
            }
        } else {
            let order: Vec<NodeId> = targets.keys().copied().collect();
            for inst in order {
                self.expand_rec(inst, &targets, &mut done);
            }
            // Instances whose component is unknown still honour their own overrides.
            let rest: Vec<NodeId> = self.overrides.keys().copied().filter(|i| !done.contains(i)).collect();
            for inst in rest {
                self.apply_overrides(inst, &targets, &mut done);
            }
        }
        if self.next_clone > 1 {
            self.doc.reserve_ids(NodeId(self.next_clone - 1));
        }
        self.apply_rest_overrides();
        self.resolve_proto();
    }

    fn install_deferred_hydrator(&mut self) {
        let Some((pending_by_page, hydrated_pages, targets, done)) = self.deferred_pending.take() else {
            return;
        };
        if pending_by_page.is_empty() {
            return;
        }
        for ov in self.overrides.values() {
            for d in &ov.fast_derived {
                for &(idx, odd) in d.fill_geometry.iter().chain(d.stroke_geometry.iter()).flatten() {
                    if !self.blob_cache.contains_key(&(idx, odd)) {
                        if let Some(blob) = self.blobs.get(idx) {
                            let parsed = crate::fig::commands_blob_to_path(blob, odd);
                            if !parsed.cmds.is_empty() {
                                self.blob_cache.insert((idx, odd), parsed);
                            }
                        }
                    }
                }
            }
            for e in ov.set.iter().chain(ov.derived.iter()) {
                for k in ["fillGeometry", "strokeGeometry"] {
                    let arr = farr(&e.set, k);
                    if !arr.is_empty() {
                        let _ = parse_geometry_cached(arr, &mut self.path_cache, &mut self.blob_cache, self.blobs);
                    }
                }
            }
        }
        let hydrator = DeferredFigHydrator {
            pending_by_page,
            hydrated_pages,
            targets,
            done,
            ids: std::mem::take(&mut self.ids),
            overrides: std::mem::take(&mut self.overrides),
            override_keys: std::mem::take(&mut self.override_keys),
            scales: std::mem::take(&mut self.scales),
            path_cache: std::mem::take(&mut self.path_cache),
            blob_cache: std::mem::take(&mut self.blob_cache),
            expanded_comps: std::mem::take(&mut self.expanded_comps),
            next_clone: self.next_clone,
        };
        self.doc.page_hydrator = Some(std::sync::Arc::new(std::sync::Mutex::new(hydrator)));
    }

    /// Instances of components defined in other files (`remote: true` in the
    /// REST `components` map, or simply absent from this file) get a stand-in
    /// main component on the hidden library page, grouped under a placeholder
    /// library named after the file key, so they keep working and are listed
    /// as coming from another file.
    fn link_remote_components(&mut self, v: &Value) {
        let unresolved = std::mem::take(&mut self.unresolved);
        if unresolved.is_empty() {
            return;
        }
        let file_key = fstr(v, "fileKey").or_else(|| fstr(v, "file_key")).unwrap_or("external");
        let library_id = format!("{}{file_key}", designos_doc::REMOTE_LIBRARY_PREFIX);
        let meta = v.get("components").and_then(Value::as_object);
        // Group by component key (falling back to the REST id), in first-use order.
        let mut order: Vec<String> = Vec::new();
        let mut groups: BTreeMap<String, (String, Vec<NodeId>)> = BTreeMap::new();
        for (inst, cid) in unresolved {
            let info = meta.and_then(|m| m.get(&cid));
            let key = info.and_then(|c| fstr(c, "key")).map(String::from).unwrap_or_else(|| cid.clone());
            let name = info.and_then(|c| fstr(c, "name")).map(String::from).or_else(|| self.doc.get(inst).map(|n| n.name.clone())).unwrap_or_default();
            if !groups.contains_key(&key) {
                order.push(key.clone());
            }
            groups.entry(key).or_insert_with(|| (name, Vec::new())).1.push(inst);
        }
        for key in order {
            let (name, insts) = &groups[&key];
            for i in insts {
                if let Some(NodeKind::Instance { component_key, .. }) = self.doc.get_mut(*i).map(|n| &mut n.kind) {
                    component_key.get_or_insert_with(|| key.clone());
                }
            }
            designos_doc::link_remote_component(&mut self.doc, &library_id, &key, name, insts);
        }
    }

    /// Maps collected prototype data (REST ids) onto the imported nodes.
    fn resolve_proto(&mut self) {
        let res = crate::proto::Resolver { ids: &self.ids, overlay: &self.overlay_props };
        let mut reactions = Vec::new();
        for (id, raw) in std::mem::take(&mut self.proto) {
            reactions.push((id, res.reactions(&raw)));
        }
        let mut flows = Vec::new();
        for (page, canvas) in std::mem::take(&mut self.flow_pages) {
            flows.extend(res.flows(page, &canvas));
        }
        for (id, r) in reactions {
            if let Some(n) = self.doc.get_mut(id) {
                n.prototype.reactions = r;
            }
        }
        self.doc.flows.extend(flows);
    }

    /// Sets `ComponentLink::source` on the children REST already inlined under
    /// instances (matching `I<instance>;<id>` ids to the main component's
    /// nodes) and on every instance root.
    fn link_sources(&mut self, targets: &BTreeMap<NodeId, NodeId>) {
        for inst in targets.keys() {
            let mut stack: Vec<NodeId> = self.doc.get(*inst).map(|n| n.children.clone()).unwrap_or_default();
            while let Some(d) = stack.pop() {
                let Some(n) = self.doc.get(d) else { continue };
                let kids = n.children.clone();
                let src = n
                    .source_id
                    .as_deref()
                    .and_then(counterpart_key)
                    .or_else(|| self.override_keys.get(&d).map(|s| s.to_string()))
                    .and_then(|k| self.ids.get(&k).copied())
                    .filter(|s| *s != d);
                if let (Some(s), Some(m)) = (src, self.doc.get_mut(d)) {
                    m.component.source = Some(s);
                }
                stack.extend(kids);
            }
        }
        for (inst, comp) in targets {
            if let Some(n) = self.doc.get_mut(*inst) {
                if n.component.source.is_none() {
                    n.component.source = Some(*comp);
                }
            }
        }
    }

    fn resolve_swaps(&mut self) {
        for (node, name, rest_id, is_def) in std::mem::take(&mut self.pending_swaps) {
            if let Some(pref_key) = rest_id.strip_prefix("PREF:") {
                let Some(&target) = self.ids.get(pref_key) else { continue };
                let Some(n) = self.doc.get_mut(node) else { continue };
                if let Some(d) = n.component.props.iter_mut().find(|d| d.name == name) {
                    if let PropKind::InstanceSwap { preferred } = &mut d.kind {
                        if !preferred.contains(&target) {
                            preferred.push(target);
                        }
                    }
                }
                continue;
            }
            if let Some(slot_csv) = rest_id.strip_prefix("SLOT:") {
                let ids: Vec<NodeId> = if slot_csv.is_empty() {
                    Vec::new()
                } else {
                    slot_csv
                        .split(',')
                        .filter_map(|part| {
                            let part = part.trim();
                            self.ids.get(part).copied().or_else(|| part.parse::<u64>().ok().map(NodeId))
                        })
                        .collect()
                };
                if let Some(n) = self.doc.get_mut(node) {
                    n.component.values.insert(name, PropValue::Slot(ids));
                }
                continue;
            }
            let target = self.ids.get(&rest_id).copied().unwrap_or(NodeId(0));
            let Some(n) = self.doc.get_mut(node) else { continue };
            if is_def {
                if let Some(d) = n.component.props.iter_mut().find(|d| d.name == name) {
                    d.default = PropValue::Instance(target);
                }
            } else if target.0 == 0 {
                n.component.values.remove(&name);
            } else {
                n.component.values.insert(name, PropValue::Instance(target));
            }
        }
    }

    /// REST `overrides[].overriddenFields` -> `ComponentLink::overrides`.
    fn apply_rest_overrides(&mut self) {
        for (_inst, nid, fields) in std::mem::take(&mut self.rest_overrides) {
            let Some(&id) = self.ids.get(&nid) else { continue };
            let Some(n) = self.doc.get_mut(id) else { continue };
            let is_text = matches!(n.kind, NodeKind::Text(_));
            for f in &fields {
                if let Some(of) = override_field(f, is_text) {
                    n.component.overrides.insert(of);
                }
            }
        }
    }

    fn fresh_id(&mut self) -> NodeId {
        let id = NodeId(self.next_clone);
        self.next_clone += 1;
        id
    }

    /// Expands `inst`, first expanding instances nested inside its component so
    /// their (overridden) children are copied too.
    ///
    /// Iterative post-order walk on an explicit stack, so a chain of any depth is expanded completely
    /// (inner components first, each before it is cloned) without growing the call stack. An instance is
    /// entered once (`done`) and a component's nested instances are listed once (`expanded_comps`), which
    /// keeps component cycles finite.
    ///
    /// An instance swap needs the new component's nested instances expanded before its contents are copied.
    /// Those components are known before the swap runs (see [`Self::swap_target_components`]), so their nested
    /// instances are scheduled on the same stack ahead of the step that applies the swap, instead of being
    /// expanded from inside it, which would grow the call stack with the length of a swap chain.
    fn expand_rec(&mut self, inst: NodeId, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) {
        enum Step {
            Enter(NodeId),
            Finish(NodeId, NodeId),
            Apply(NodeId),
        }
        let mut stack = vec![Step::Enter(inst)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(inst) => {
                    if !done.insert(inst) {
                        continue;
                    }
                    let Some(&comp) = targets.get(&inst) else { continue };
                    stack.push(Step::Finish(inst, comp));
                    // Reversed so the lowest id is expanded first, which keeps clone ids deterministic.
                    stack.extend(self.take_nested_instances(comp, targets).into_iter().rev().map(Step::Enter));
                }
                Step::Finish(inst, comp) => {
                    self.expand_instance(inst, comp);
                    if let Some(s) = self.scales.get(&inst).copied() {
                        self.scale_contents(inst, s);
                    }
                    stack.push(Step::Apply(inst));
                    let swap_nested_instances: Vec<NodeId> =
                        self.swap_target_components(inst).into_iter().flat_map(|comp| self.take_nested_instances(comp, targets)).collect();
                    stack.extend(swap_nested_instances.into_iter().rev().map(Step::Enter));
                }
                Step::Apply(inst) => {
                    // Property values first: an instance swap changes which nodes the overrides address.
                    self.apply_props(inst, targets, done);
                    self.apply_overrides(inst, targets, done);
                }
            }
        }
    }

    /// Scales the copied children of an instance made with the scale tool
    /// (`uniformScaleFactor`): positions, sizes, radii, strokes, text sizes,
    /// layout spacing and geometry. Overrides and derived data are applied after.
    fn scale_contents(&mut self, inst: NodeId, s: f32) {
        fn scale_path(p: &mut Path, s: f32) {
            let f = |v: &mut Vec2| *v = Vec2::new(v.x * s, v.y * s);
            for c in &mut p.cmds {
                match c {
                    PathCmd::MoveTo(a) | PathCmd::LineTo(a) => f(a),
                    PathCmd::QuadTo(a, b) => {
                        f(a);
                        f(b);
                    }
                    PathCmd::CubicTo(a, b, c) => {
                        f(a);
                        f(b);
                        f(c);
                    }
                    PathCmd::Close => {}
                }
            }
        }
        let mut stack: Vec<NodeId> = self.doc.get(inst).map(|n| n.children.clone()).unwrap_or_default();
        while let Some(d) = stack.pop() {
            let Some(n) = self.doc.get_mut(d) else { continue };
            stack.extend(n.children.iter().copied());
            n.transform.tx *= s;
            n.transform.ty *= s;
            n.size = Vec2::new(n.size.x * s, n.size.y * s);
            for r in &mut n.corner_radii {
                *r *= s;
            }
            n.stroke.weight *= s;
            for g in n.fill_geometry.iter_mut().chain(n.stroke_geometry.iter_mut()).flatten() {
                scale_path(g, s);
            }
            match &mut n.kind {
                NodeKind::Text(t) => {
                    t.style.font_size *= s;
                    for r in &mut t.runs {
                        r.style.font_size *= s;
                    }
                }
                NodeKind::Vector { paths, network } => {
                    paths.iter_mut().for_each(|p| scale_path(p, s));
                    if let Some(net) = network {
                        *net = net.transformed(&Affine::scale(s, s));
                    }
                }
                k => {
                    if let Some(al) = k.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                        al.item_spacing *= s;
                        al.counter_spacing *= s;
                        al.padding.iter_mut().for_each(|p| *p *= s);
                    }
                }
            }
        }
    }

    /// Components the instance's property values and overrides may swap a nested instance to (a superset is
    /// harmless: listing a component's nested instances earlier does not change what they expand to).
    fn swap_target_components(&self, inst: NodeId) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = Vec::new();
        if let Some(ov) = self.overrides.get(&inst) {
            for e in &ov.set {
                out.extend(fstr(&e.set, "componentId").and_then(|c| self.ids.get(c)).copied());
                if let Some(props) = e.set.get("componentProperties").and_then(Value::as_object) {
                    let swapped = props.values().filter(|p| fstr(p, "type") == Some("INSTANCE_SWAP")).filter_map(|p| p.get("value").and_then(Value::as_str));
                    out.extend(swapped.filter_map(|c| self.ids.get(c)).copied());
                }
            }
        }
        if self.has_instance_swap_values {
            let mut stack = vec![inst];
            while let Some(d) = stack.pop() {
                let Some(n) = self.doc.get(d) else { continue };
                stack.extend(n.children.iter().copied());
                out.extend(n.component.values.values().filter_map(|v| match v {
                    PropValue::Instance(t) if t.0 != 0 => Some(*t),
                    _ => None,
                }));
            }
        }
        out.retain(|t| matches!(self.doc.get(*t).map(|n| &n.kind), Some(NodeKind::Component(_))));
        out
    }

    /// Expands the instances inside `comp` (innermost first).
    fn expand_nested(&mut self, comp: NodeId, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) {
        for n in self.take_nested_instances(comp, targets) {
            self.expand_rec(n, targets, done);
        }
    }

    /// Instances inside `comp` that still have to be expanded, lowest id first; empty once `comp` was listed.
    fn take_nested_instances(&mut self, comp: NodeId, targets: &BTreeMap<NodeId, NodeId>) -> Vec<NodeId> {
        if !self.expanded_comps.insert(comp) {
            return Vec::new();
        }
        let mut stack = vec![comp];
        let mut nested = Vec::new();
        while let Some(n) = stack.pop() {
            if let Some(node) = self.doc.get(n) {
                if n != comp && targets.contains_key(&n) {
                    nested.push(n);
                }
                stack.extend(node.children.iter().copied());
            }
        }
        nested.sort();
        nested
    }

    /// Applies the instance's component property values to the nodes of its
    /// copy that are bound to them: visibility, text, and instance swaps.
    fn apply_props(&mut self, inst: NodeId, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) {
        let values = match self.doc.get(inst) {
            Some(n) if !n.component.values.is_empty() => n.component.values.clone(),
            _ => return,
        };
        let mut stack: Vec<NodeId> = self.doc.get(inst).map(|n| n.children.clone()).unwrap_or_default();
        while let Some(d) = stack.pop() {
            let Some(n) = self.doc.get(d) else { continue };
            stack.extend(n.children.iter().copied());
            let bindings: Vec<(OverrideField, String)> = n.component.bindings.iter().map(|(f, p)| (*f, p.clone())).collect();
            for (field, prop) in bindings {
                match (field, values.get(&prop)) {
                    (OverrideField::Visible, Some(PropValue::Bool(b))) => {
                        if let Some(n) = self.doc.get_mut(d) {
                            n.visible = *b;
                        }
                    }
                    (OverrideField::Characters, Some(PropValue::Text(s))) => {
                        if let Some(NodeKind::Text(t)) = self.doc.get_mut(d).map(|n| &mut n.kind) {
                            t.content = s.clone();
                            t.runs.clear();
                            t.lists.clear();
                        }
                    }
                    (OverrideField::MainComponent, Some(PropValue::Instance(target))) => self.swap_nested(d, *target, targets, done),
                    _ => {}
                }
            }
        }
    }

    /// Points the nested instance `d` at `target` and rebuilds its children.
    fn swap_nested(&mut self, d: NodeId, target: NodeId, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) {
        let current = match self.doc.get(d).map(|n| &n.kind) {
            Some(NodeKind::Instance { component, .. }) => *component,
            _ => return,
        };
        if current == Some(target) || !matches!(self.doc.get(target).map(|n| &n.kind), Some(NodeKind::Component(_))) {
            return;
        }
        self.expand_nested(target, targets, done);
        for k in self.doc.get(d).map(|n| n.children.clone()).unwrap_or_default() {
            let _ = self.doc.remove(k);
        }
        if let Some(NodeKind::Instance { component, component_key, .. }) = self.doc.get_mut(d).map(|n| &mut n.kind) {
            *component = Some(target);
            *component_key = None;
        }
        self.expand_instance(d, target);
    }

    /// `.fig` library components of internal-only canvases become external
    /// assets of placeholder libraries (one per `sourceLibraryKey`), and the
    /// instances using them learn their component key.
    fn register_library_assets(&mut self) {
        let Some(lp) = self.doc.library_page else { return };
        if self.library_keys.is_empty() {
            return;
        }
        let keys = std::mem::take(&mut self.library_keys);
        for (node, (key, lib)) in &keys {
            let Some(n) = self.doc.get(*node) else { continue };
            let in_set = n.parent.and_then(|p| self.doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
            if in_set || !self.doc.is_ancestor(lp, *node) {
                continue;
            }
            let kind = if matches!(n.kind, NodeKind::ComponentSet(_)) { LibraryAssetKind::ComponentSet } else { LibraryAssetKind::Component };
            let library_id = format!("{}{}", REMOTE_LIBRARY_PREFIX, lib.as_deref().unwrap_or("file"));
            self.doc.library.external.insert(local_key(&library_id, key), ExternalAsset { library_id, version: 0, kind, node: Some(*node), local_key: None });
        }
        for n in self.doc.nodes.values_mut() {
            if let NodeKind::Instance { component: Some(comp), component_key, .. } = &mut n.kind {
                if component_key.is_none() {
                    if let Some((key, _)) = keys.get(comp) {
                        *component_key = Some(key.clone());
                    }
                }
            }
        }
        self.library_keys = keys;
    }

    /// Deep-copies the component's children under an empty instance.
    fn expand_instance(&mut self, inst: NodeId, comp: NodeId) {
        if !self.doc.get(inst).map(|n| n.children.is_empty()).unwrap_or(false) || self.doc.is_ancestor(comp, inst) || comp == inst {
            return;
        }
        let fast_clone = self.rest_overrides.is_empty() && self.doc.nodes.len() >= 10_000;
        let prefix = if fast_clone { String::new() } else { self.doc.get(inst).and_then(|n| n.source_id.clone()).unwrap_or_default() };
        let kids = self.doc.get(comp).map(|n| n.children.clone()).unwrap_or_default();
        let mut budget = 50_000usize;
        // Pre-order on an explicit stack: a component of any depth is copied completely without recursion.
        let mut stack: Vec<(NodeId, NodeId)> = kids.into_iter().rev().map(|k| (k, inst)).collect();
        while let Some((src, parent)) = stack.pop() {
            if budget == 0 {
                return;
            }
            budget -= 1;
            let Some(mut n) = self.doc.get(src).cloned() else { continue };
            let orig_children = std::mem::take(&mut n.children);
            let id = self.fresh_id();
            n.id = id;
            n.parent = Some(parent);
            if fast_clone {
                n.fig_metadata.clear();
                if let Some(sid) = n.source_id.take() {
                    self.override_keys.entry(src).or_insert_with(|| std::sync::Arc::from(sid.rsplit(';').next().unwrap_or(&sid)));
                }
            } else {
                n.source_id = n.source_id.as_ref().map(|s| format!("I{prefix};{s}"));
                if let Some(k) = self.override_keys.get(&src).cloned() {
                    self.override_keys.insert(id, k);
                }
            }
            n.component.source = Some(src);
            n.component.overrides.clear();
            n.component.props.clear();
            let container = n.kind.is_container();
            self.doc.nodes.insert(id, n);
            if let Some(p) = self.doc.nodes.get_mut(&parent) {
                p.children.push(id);
            }
            if container {
                stack.extend(orig_children.into_iter().rev().map(|c| (c, id)));
            }
        }
    }

    fn override_key_of(&self, mut id: NodeId) -> Option<&std::sync::Arc<str>> {
        for _ in 0..8 {
            if let Some(k) = self.override_keys.get(&id) {
                return Some(k);
            }
            let src = self.doc.get(id).and_then(|n| n.component.source)?;
            if src == id {
                return None;
            }
            id = src;
        }
        None
    }

    /// Applies `.fig` `symbolOverrides` and then `derivedSymbolData` to the
    /// instance's copied subtree.
    fn apply_overrides(&mut self, inst: NodeId, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) {
        let Some(ov) = self.overrides.remove(&inst) else { return };
        let mut subtree_maps: HashMap<NodeId, HashMap<String, NodeId>> = HashMap::new();
        for e in &ov.set {
            if let Some(t) = self.resolve_path_cached(inst, &e.path, &mut subtree_maps) {
                if self.apply_override(t, &e.set, false, targets, done) {
                    subtree_maps.clear();
                }
            }
        }
        // Figma's computed results for resized instances win over the raw overrides.
        for e in &ov.derived {
            if let Some(t) = self.resolve_path_cached(inst, &e.path, &mut subtree_maps) {
                self.apply_override(t, &e.set, true, targets, done);
            }
        }
        for d in &ov.fast_derived {
            if let Some(t) = self.resolve_path_cached(inst, &d.path, &mut subtree_maps) {
                self.apply_fast_derived(t, d);
            }
        }
    }

    fn apply_fast_derived(&mut self, id: NodeId, d: &crate::fig::FastDerivedEntry) {
        let skip_geo = match self.doc.get(id).map(|n| &n.kind) {
            Some(
                NodeKind::Text(_)
                | NodeKind::Sticky(_)
                | NodeKind::ShapeWithText(_)
                | NodeKind::Connector(_)
                | NodeKind::Stamp { .. }
                | NodeKind::Frame(_)
                | NodeKind::Component(_)
                | NodeKind::ComponentSet(_)
                | NodeKind::Instance { .. }
                | NodeKind::Section
                | NodeKind::Rectangle,
            ) => true,
            Some(_) => false,
            None => return,
        };
        let mut fill_geo = (!skip_geo)
            .then(|| {
                d.fill_geometry.as_ref().map(|arr| {
                    arr.iter()
                        .filter_map(|&(idx, odd)| {
                            if let Some(cached) = self.blob_cache.get(&(idx, odd)) {
                                return Some(cached.clone());
                            }
                            let blob = self.blobs.get(idx)?;
                            let parsed = crate::fig::commands_blob_to_path(blob, odd);
                            if parsed.cmds.is_empty() {
                                None
                            } else {
                                self.blob_cache.insert((idx, odd), parsed.clone());
                                Some(parsed)
                            }
                        })
                        .collect::<Vec<Path>>()
                })
            })
            .flatten();
        let stroke_geo = (!skip_geo)
            .then(|| {
                d.stroke_geometry.as_ref().map(|arr| {
                    arr.iter()
                        .filter_map(|&(idx, odd)| {
                            if let Some(cached) = self.blob_cache.get(&(idx, odd)) {
                                return Some(cached.clone());
                            }
                            let blob = self.blobs.get(idx)?;
                            let parsed = crate::fig::commands_blob_to_path(blob, odd);
                            if parsed.cmds.is_empty() {
                                None
                            } else {
                                self.blob_cache.insert((idx, odd), parsed.clone());
                                Some(parsed)
                            }
                        })
                        .collect::<Vec<Path>>()
                })
            })
            .flatten();
        let Some(n) = self.doc.get_mut(id) else { return };
        let is_vec_or_bool = matches!(n.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_));
        if let Some(sz) = d.size {
            if n.size != sz && !is_vec_or_bool {
                if d.fill_geometry.is_none() || skip_geo {
                    n.fill_geometry = None;
                }
                if d.stroke_geometry.is_none() || skip_geo {
                    n.stroke_geometry = None;
                }
            }
            n.size = sz;
        }
        if let Some(t) = d.transform {
            n.transform = t;
        }
        if let Some(g) = fill_geo.take() {
            if let NodeKind::Vector { paths, network } = &mut n.kind {
                *paths = g;
                *network = None;
                n.fill_geometry = None;
            } else {
                n.fill_geometry = Some(g).filter(|g| !g.is_empty());
            }
        }
        if let Some(g) = stroke_geo {
            n.stroke_geometry = Some(g).filter(|g| !g.is_empty());
        }
    }

    fn node_matches(&self, c: NodeId, g: &str) -> bool {
        if let Some(k) = self.override_key_of(c) {
            if &**k == g {
                return true;
            }
        }
        self.doc.get(c).is_some_and(|n| {
            let s = n.source_id.as_deref().unwrap_or("");
            s.rsplit(';').next().unwrap_or(s) == g
        })
    }

    fn build_subtree_lookup(&self, root: NodeId) -> HashMap<String, NodeId> {
        let mut map = HashMap::new();
        let Some(node) = self.doc.get(root) else { return map };
        for &c in &node.children {
            let ok = self.override_key_of(c);
            if let Some(k) = ok {
                if !map.contains_key(&**k) {
                    map.insert(k.to_string(), c);
                }
            }
            if let Some(cn) = self.doc.get(c) {
                if let Some(s) = cn.source_id.as_deref() {
                    let tail = s.rsplit(';').next().unwrap_or(s);
                    if !tail.is_empty() && ok.map(|x| &**x) != Some(tail) && !map.contains_key(tail) {
                        map.insert(tail.to_string(), c);
                    }
                }
            }
        }
        let mut stack: Vec<NodeId> = Vec::new();
        for &c in node.children.iter().rev() {
            if let Some(cn) = self.doc.get(c) {
                stack.extend(cn.children.iter().rev().copied());
            }
        }
        while let Some(n) = stack.pop() {
            let Some(nn) = self.doc.get(n) else { continue };
            let ok = self.override_key_of(n);
            if let Some(k) = ok {
                if !map.contains_key(&**k) {
                    map.insert(k.to_string(), n);
                }
            }
            if let Some(s) = nn.source_id.as_deref() {
                let tail = s.rsplit(';').next().unwrap_or(s);
                if !tail.is_empty() && ok.map(|x| &**x) != Some(tail) && !map.contains_key(tail) {
                    map.insert(tail.to_string(), n);
                }
            }
            stack.extend(nn.children.iter().rev().copied());
        }
        map
    }

    /// Walks a `guidPath` (ids of the nodes inside the component, outermost
    /// first) down from the instance. Clone source ids look like
    /// `I<instance>;<original>`; only the part after the last `;` identifies the
    /// original node.
    fn resolve_path_cached<S: AsRef<str>>(&self, inst: NodeId, path: &[S], subtree_maps: &mut HashMap<NodeId, HashMap<String, NodeId>>) -> Option<NodeId> {
        let mut cur = inst;
        for g_ref in path {
            let g = g_ref.as_ref();
            let node = self.doc.get(cur)?;
            let found = if let Some(map) = subtree_maps.get(&cur) {
                map.get(g).copied()
            } else if let Some(c) = node.children.iter().copied().find(|c| self.node_matches(*c, g)) {
                Some(c)
            } else {
                let map = subtree_maps.entry(cur).or_insert_with(|| self.build_subtree_lookup(cur));
                map.get(g).copied()
            };
            cur = match found {
                Some(f) => f,
                None if cur == inst && (self.node_matches(inst, g) || node.component.source.is_some_and(|src| self.node_matches(src, g))) => inst,
                None => return None,
            };
        }
        Some(cur)
    }

    /// Applies partial REST node fields to an existing node. `derived_only`
    /// restricts to size, transform and geometry. Returns true if a nested
    /// instance swap took place.
    fn apply_override(&mut self, id: NodeId, set: &Value, derived_only: bool, targets: &BTreeMap<NodeId, NodeId>, done: &mut BTreeSet<NodeId>) -> bool {
        let mut swapped = false;
        if !derived_only {
            if let Some(cid) = fstr(set, "componentId") {
                if let Some(&target_comp) = self.ids.get(cid) {
                    self.swap_nested(id, target_comp, targets, done);
                    swapped = true;
                }
            }
        }
        let parent_dir = self
            .doc
            .get(id)
            .and_then(|n| n.parent)
            .and_then(|p| self.doc.get(p))
            .and_then(|p| p.kind.frame_props())
            .and_then(|f| f.auto_layout)
            .map(|a| a.direction);
        let Some(n) = self.doc.get_mut(id) else { return swapped };
        let keep = |k: &str| !derived_only || matches!(k, "size" | "relativeTransform" | "fillGeometry" | "strokeGeometry");
        let has = |k: &str| keep(k) && set.get(k).is_some();
        let is_vec_or_bool = matches!(n.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_));
        let skip_geo = matches!(n.kind, NodeKind::Text(_) | NodeKind::Sticky(_) | NodeKind::ShapeWithText(_) | NodeKind::Connector(_) | NodeKind::Stamp { .. })
            || (self.large_file
                && matches!(
                    n.kind,
                    NodeKind::Frame(_)
                        | NodeKind::Component(_)
                        | NodeKind::ComponentSet(_)
                        | NodeKind::Instance { .. }
                        | NodeKind::Section
                        | NodeKind::Rectangle
                ));
        if has("size") {
            if let Some(sz) = set.get("size").and_then(vec2) {
                if n.size != sz && !is_vec_or_bool {
                    if !has("fillGeometry") || skip_geo {
                        n.fill_geometry = None;
                    }
                    if !has("strokeGeometry") || skip_geo {
                        n.stroke_geometry = None;
                    }
                }
                n.size = sz;
            }
        }
        if has("relativeTransform") {
            if let Some(t) = set.get("relativeTransform").and_then(parse_transform) {
                n.transform = t;
            }
        }
        if has("fillGeometry") && !skip_geo {
            let g = parse_geometry_cached(farr(set, "fillGeometry"), &mut self.path_cache, &mut self.blob_cache, self.blobs);
            if let NodeKind::Vector { paths, network } = &mut n.kind {
                if self.large_file {
                    *paths = g;
                    *network = None;
                    n.fill_geometry = None;
                } else {
                    *paths = g.clone();
                    // the new outline replaces what the network described
                    *network = None;
                    n.fill_geometry = Some(g).filter(|g| !g.is_empty());
                }
            } else {
                n.fill_geometry = Some(g).filter(|g| !g.is_empty());
            }
        }
        if has("strokeGeometry") && !skip_geo {
            n.stroke_geometry =
                Some(parse_geometry_cached(farr(set, "strokeGeometry"), &mut self.path_cache, &mut self.blob_cache, self.blobs)).filter(|g| !g.is_empty());
        }
        if derived_only {
            return swapped;
        }
        let is_text = matches!(n.kind, NodeKind::Text(_));
        if let Some(obj) = set.as_object() {
            for k in obj.keys() {
                if let Some(of) = override_field(k, is_text) {
                    n.component.overrides.insert(of);
                }
            }
        }
        if let Some(s) = fstr(set, "name") {
            n.name = s.to_string();
        }
        if let Some(lvl) = set.get("overrideLevel").and_then(Value::as_i64) {
            n.component.override_level = Some(lvl as i32);
        }
        if let Some(ds) = fstr(set, "detachedSymbolId").filter(|s| !s.is_empty()) {
            n.detached_symbol_id = Some(ds.to_string());
            n.component.detached_symbol_id = Some(ds.to_string());
        }
        if let Some(b) = fbool(set, "visible") {
            n.visible = b;
        }
        if let Some(b) = fbool(set, "locked") {
            n.locked = b;
        }
        if let Some(o) = fnum(set, "opacity") {
            n.opacity = o;
        }
        if let Some(b) = fstr(set, "blendMode") {
            n.blend = blend(b);
        }
        if let Some(b) = fbool(set, "isMask") {
            n.is_mask = b;
        }
        if set.get("fills").is_some() {
            n.variables.bindings.retain(|k, _| !k.starts_with("fills/"));
            let fills = parse_paints(set.get("fills"));
            match &mut n.kind {
                NodeKind::Text(t) => t.style.fills = fills,
                _ => n.fills = fills,
            }
        }
        if set.get("strokes").is_some() {
            n.variables.bindings.retain(|k, _| !k.starts_with("strokes/"));
            n.stroke.paints = parse_paints(set.get("strokes"));
        }
        if set.get("boundVariables").is_some() || set.get("explicitVariableModes").is_some() {
            let ov_vars = crate::variables::parse_node_variables(set);
            n.variables.bindings.extend(ov_vars.bindings);
            n.variables.modes.extend(ov_vars.modes);
        }
        if let Some(w) = fnum(set, "strokeWeight") {
            n.stroke.weight = w;
            if !has("strokeGeometry") {
                n.stroke_geometry = None;
            }
        }
        if let Some(iw) = set.get("individualStrokeWeights") {
            let g = |k: &str| fnum(iw, k).unwrap_or(0.0);
            n.stroke.side_weights = Some([g("top"), g("right"), g("bottom"), g("left")]);
            if !has("strokeGeometry") {
                n.stroke_geometry = None;
            }
        }
        if let Some(a) = fstr(set, "strokeAlign") {
            n.stroke.align = match a {
                "CENTER" => StrokeAlign::Center,
                "OUTSIDE" => StrokeAlign::Outside,
                _ => StrokeAlign::Inside,
            };
        }
        if set.get("effects").is_some() {
            n.effects = farr(set, "effects").iter().filter_map(parse_effect).collect();
        }
        if let Some(r) = set.get("rectangleCornerRadii").and_then(Value::as_array).filter(|a| a.len() == 4) {
            for (i, x) in r.iter().enumerate() {
                n.corner_radii[i] = num(x).unwrap_or(0.0);
            }
            if !is_vec_or_bool && !has("fillGeometry") {
                n.fill_geometry = None;
            }
        } else if let Some(r) = fnum(set, "cornerRadius") {
            n.corner_radii = [r; 4];
            if !is_vec_or_bool && !has("fillGeometry") {
                n.fill_geometry = None;
            }
        }
        if let Some(fp) = n.kind.frame_props_mut() {
            if let Some(c) = fbool(set, "clipsContent") {
                fp.clips_content = c;
            }
            if set.get("layoutGrids").is_some() {
                fp.grids = farr(set, "layoutGrids").iter().filter_map(parse_grid).collect();
            }
            if set.get("layoutMode").is_some() {
                fp.auto_layout = frame_props(set).auto_layout;
            } else if let Some(al) = fp.auto_layout.as_mut() {
                let align = |s: &str| match s {
                    "CENTER" => LayoutAlign::Center,
                    "MAX" => LayoutAlign::End,
                    "SPACE_BETWEEN" => LayoutAlign::SpaceBetween,
                    "BASELINE" => LayoutAlign::Baseline,
                    _ => LayoutAlign::Start,
                };
                let sizing = |s: &str| if s == "AUTO" { Sizing::Hug } else { Sizing::Fixed };
                if let Some(x) = fnum(set, "itemSpacing") {
                    al.item_spacing = x;
                }
                if let Some(x) = fnum(set, "counterAxisSpacing") {
                    al.counter_spacing = x;
                }
                for (idx, k) in [(0, "paddingTop"), (1, "paddingRight"), (2, "paddingBottom"), (3, "paddingLeft")] {
                    if let Some(x) = fnum(set, k) {
                        al.padding[idx] = x;
                    }
                }
                if let Some(s) = fstr(set, "primaryAxisAlignItems") {
                    al.primary_align = align(s);
                }
                if let Some(s) = fstr(set, "counterAxisAlignItems") {
                    al.counter_align = align(s);
                }
                if let Some(s) = fstr(set, "primaryAxisSizingMode") {
                    al.primary_sizing = sizing(s);
                }
                if let Some(s) = fstr(set, "counterAxisSizingMode") {
                    al.counter_sizing = sizing(s);
                }
                if let Some(w) = fstr(set, "layoutWrap") {
                    al.wrap = w == "WRAP";
                }
                if let Some(b) = fbool(set, "strokesIncludedInLayout") {
                    al.stroke_included = b;
                }
                if let Some(b) = fbool(set, "itemReverseZIndex") {
                    al.reverse_z = b;
                }
                if let Some(s) = fstr(set, "gridColumnsSizing") {
                    let (cols, tracks) = parse_grid_sizing(s);
                    if cols > 0 {
                        al.grid_columns = cols;
                        al.grid_col_tracks = tracks;
                    }
                }
                if let Some(s) = fstr(set, "gridRowsSizing") {
                    let (rows, tracks) = parse_grid_sizing(s);
                    if rows > 0 {
                        al.grid_rows = rows;
                        al.grid_row_tracks = tracks;
                    }
                }
                if let Some(c) = set.get("gridColumns").and_then(Value::as_u64) {
                    al.grid_columns = c as u32;
                }
                if let Some(r) = set.get("gridRows").and_then(Value::as_u64) {
                    al.grid_rows = r as u32;
                }
            }
        }
        let non_fill_sizing = |kind: &NodeKind, dir_h: bool| -> Sizing {
            match kind {
                NodeKind::Text(t) => match t.auto_resize {
                    TextAutoResize::WidthAndHeight => Sizing::Hug,
                    TextAutoResize::Height if !dir_h => Sizing::Hug,
                    _ => Sizing::Fixed,
                },
                k => k.frame_props().and_then(|f| f.auto_layout).map_or(Sizing::Fixed, |al| al.sizing_along(dir_h)),
            }
        };
        if let Some(al) = n.kind.frame_props().and_then(|fp| fp.auto_layout) {
            let prim_h = al.primary_is_horizontal();
            if set.get("primaryAxisSizingMode").is_some() {
                let slot = if prim_h { &mut n.layout_child.horizontal } else { &mut n.layout_child.vertical };
                if *slot != Sizing::Fill {
                    *slot = al.primary_sizing;
                }
            }
            if set.get("counterAxisSizingMode").is_some() {
                let slot = if prim_h { &mut n.layout_child.vertical } else { &mut n.layout_child.horizontal };
                if *slot != Sizing::Fill {
                    *slot = al.counter_sizing;
                }
            }
        }
        if let Some(g) = fnum(set, "layoutGrow") {
            n.layout_child.grow = g;
            if g > 0.0 {
                match parent_dir {
                    Some(LayoutDirection::Horizontal | LayoutDirection::Grid) => n.layout_child.horizontal = Sizing::Fill,
                    Some(LayoutDirection::Vertical) => n.layout_child.vertical = Sizing::Fill,
                    None => {}
                }
            } else {
                match parent_dir {
                    Some(LayoutDirection::Horizontal | LayoutDirection::Grid) if n.layout_child.horizontal == Sizing::Fill => {
                        n.layout_child.horizontal = non_fill_sizing(&n.kind, true);
                    }
                    Some(LayoutDirection::Vertical) if n.layout_child.vertical == Sizing::Fill => {
                        n.layout_child.vertical = non_fill_sizing(&n.kind, false);
                    }
                    _ => {}
                }
            }
        }
        if let Some(a) = fstr(set, "layoutAlign") {
            if a == "STRETCH" {
                match parent_dir {
                    Some(LayoutDirection::Horizontal | LayoutDirection::Grid) => n.layout_child.vertical = Sizing::Fill,
                    Some(LayoutDirection::Vertical) => n.layout_child.horizontal = Sizing::Fill,
                    None => {}
                }
            } else {
                match parent_dir {
                    Some(LayoutDirection::Horizontal | LayoutDirection::Grid) if n.layout_child.vertical == Sizing::Fill => {
                        n.layout_child.vertical = non_fill_sizing(&n.kind, false);
                    }
                    Some(LayoutDirection::Vertical) if n.layout_child.horizontal == Sizing::Fill => {
                        n.layout_child.horizontal = non_fill_sizing(&n.kind, true);
                    }
                    _ => {}
                }
            }
        }
        if let Some(p) = fstr(set, "layoutPositioning") {
            n.layout_child.absolute = p == "ABSOLUTE";
        }
        if let Some(c) = set.get("gridColumnAnchorIndex").and_then(Value::as_u64) {
            n.layout_child.grid_column = Some(c as u16);
        }
        if let Some(r) = set.get("gridRowAnchorIndex").and_then(Value::as_u64) {
            n.layout_child.grid_row = Some(r as u16);
        }
        if let Some(c) = set.get("gridColumnSpan").and_then(Value::as_u64) {
            n.layout_child.col_span = (c as u16).max(1);
        }
        if let Some(r) = set.get("gridRowSpan").and_then(Value::as_u64) {
            n.layout_child.row_span = (r as u16).max(1);
        }
        if let Some(h) = fstr(set, "layoutSizingHorizontal") {
            n.layout_child.horizontal = match h {
                "FILL" => Sizing::Fill,
                "HUG" => Sizing::Hug,
                _ => Sizing::Fixed,
            };
        }
        if let Some(v) = fstr(set, "layoutSizingVertical") {
            n.layout_child.vertical = match v {
                "FILL" => Sizing::Fill,
                "HUG" => Sizing::Hug,
                _ => Sizing::Fixed,
            };
        }
        if let NodeKind::Text(t) = &mut n.kind {
            if let Some(ar) = fbool(set, "autoRename") {
                t.auto_rename = ar;
            }
            if let Some(si) = fbool(set, "textDecorationSkipInk") {
                t.decoration_skip_ink = si;
            }
            if let Some(st) = set.get("style") {
                apply_type_style(&mut t.style, st);
                crate::textfmt::apply_paragraph(t, st);
                if let Some(a) = fstr(st, "textAutoResize") {
                    t.auto_resize = auto_resize(a);
                }
                if fstr(st, "textTruncation") == Some("ENDING") {
                    t.auto_resize = TextAutoResize::Truncate;
                }
                if let Some(m) = st.get("maxLines").and_then(Value::as_u64) {
                    t.max_lines = Some(m.min(u32::MAX as u64) as u32).filter(|m| *m > 0);
                }
                if let Some(ar) = fbool(st, "autoRename") {
                    t.auto_rename = ar;
                }
                if let Some(si) = fbool(st, "textDecorationSkipInk") {
                    t.decoration_skip_ink = si;
                }
            }
            if let Some(c) = fstr(set, "characters") {
                if c != t.content {
                    t.runs.clear(); // byte ranges no longer apply
                }
                t.content = c.to_string();
                t.lists.clear();
            }
            if set.get("lineTypes").is_some() {
                crate::textfmt::apply_lines(t, set);
            }
            if set.get("characterStyleOverrides").is_some() {
                let ids: Vec<u64> = farr(set, "characterStyleOverrides").iter().map(|x| x.as_u64().unwrap_or(0)).collect();
                t.runs = build_runs(&t.content, &t.style, &ids, set.get("styleOverrideTable"));
            }
        }
        if set.get("componentProperties").is_some() || set.get("componentPropertyReferences").is_some() || set.get("variantPropSpecs").is_some() {
            if let Some(mut node_copy) = self.doc.get(id).cloned() {
                self.parse_component_props(&mut node_copy, set);
                if let Some(n) = self.doc.get_mut(id) {
                    n.component = node_copy.component;
                }
                self.resolve_swaps();
                self.apply_props(id, targets, done);
                swapped = true;
            }
        }
        swapped
    }
}

struct DeferredFigHydrator {
    pending_by_page: HashMap<NodeId, Vec<NodeId>>,
    hydrated_pages: HashSet<NodeId>,
    targets: BTreeMap<NodeId, NodeId>,
    done: BTreeSet<NodeId>,
    ids: BTreeMap<String, NodeId>,
    overrides: BTreeMap<NodeId, Overrides>,
    override_keys: HashMap<NodeId, std::sync::Arc<str>>,
    scales: BTreeMap<NodeId, f32>,
    path_cache: HashMap<String, Path>,
    blob_cache: HashMap<(usize, bool), Path>,
    expanded_comps: HashSet<NodeId>,
    next_clone: u64,
}

impl std::fmt::Debug for DeferredFigHydrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeferredFigHydrator").field("pending_pages", &self.pending_by_page.len()).field("hydrated_pages", &self.hydrated_pages.len()).finish()
    }
}

impl designos_doc::PageHydrator for DeferredFigHydrator {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
        if !self.hydrated_pages.insert(page) {
            return false;
        }
        let Some(insts) = self.pending_by_page.remove(&page) else {
            return false;
        };
        if insts.is_empty() {
            return false;
        }
        let next_clone = self.next_clone.max(doc.peek_next_id().0);
        let taken_doc = std::mem::take(doc);
        let has_instance_swap_values = taken_doc.nodes.values().any(|n| n.component.values.values().any(|v| matches!(v, PropValue::Instance(t) if t.0 != 0)));
        let mut cx = Ctx {
            doc: taken_doc,
            ids: std::mem::take(&mut self.ids),
            instances: Vec::new(),
            unresolved: Vec::new(),
            overrides: std::mem::take(&mut self.overrides),
            rest_overrides: Vec::new(),
            override_keys: std::mem::take(&mut self.override_keys),
            scales: std::mem::take(&mut self.scales),
            pending_swaps: Vec::new(),
            component_keys: BTreeMap::new(),
            library_keys: BTreeMap::new(),
            proto: Vec::new(),
            overlay_props: BTreeMap::new(),
            flow_pages: Vec::new(),
            path_cache: std::mem::take(&mut self.path_cache),
            blob_cache: std::mem::take(&mut self.blob_cache),
            blobs: &[],
            fast_derived_map: None,
            large_file: true,
            expanded_comps: std::mem::take(&mut self.expanded_comps),
            next_clone,
            has_instance_swap_values,
            has_connectors: false,
            connectors: Vec::new(),
            deferred_pending: None,
        };
        for inst in insts {
            if self.targets.contains_key(&inst) {
                cx.expand_rec(inst, &self.targets, &mut self.done);
            } else if !self.done.contains(&inst) {
                cx.apply_overrides(inst, &self.targets, &mut self.done);
            }
        }
        if cx.next_clone > 1 {
            cx.doc.reserve_ids(NodeId(cx.next_clone - 1));
        }
        self.next_clone = cx.next_clone;
        *doc = cx.doc;
        if !self.pending_by_page.is_empty() {
            self.ids = cx.ids;
            self.overrides = cx.overrides;
            self.override_keys = cx.override_keys;
            self.scales = cx.scales;
            self.path_cache = cx.path_cache;
            self.blob_cache = cx.blob_cache;
            self.expanded_comps = cx.expanded_comps;
        } else {
            self.targets.clear();
            self.done.clear();
        }
        true
    }

    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        let pages: Vec<NodeId> = self.pending_by_page.keys().copied().collect();
        let mut any = false;
        for page in pages {
            if self.hydrate_page(doc, page) {
                any = true;
            }
        }
        any
    }

    fn is_hydrated(&self, page: NodeId) -> bool {
        self.hydrated_pages.contains(&page) || !self.pending_by_page.contains_key(&page)
    }
}

/// Effect and grid styles whose values the file states on the style node (`.fig` imports put them
/// in the `styles` map): the node's values replace the copy seeded from the first consumer, and a
/// style no node uses is kept too. REST JSON carries no values there, so it is left as seeded.
fn apply_style_node_values(doc: &mut Document, styles: Option<&Value>) {
    let Some(map) = styles.and_then(Value::as_object) else { return };
    for (id, m) in map {
        let style = match fstr(m, "styleType") {
            Some("EFFECT") if m.get("effects").is_some() => Style::Effect(farr(m, "effects").iter().filter_map(parse_effect).collect()),
            Some("GRID") if m.get("layoutGrids").is_some() => Style::Grid(farr(m, "layoutGrids").iter().filter_map(parse_grid).collect()),
            _ => continue,
        };
        doc.styles.insert(id.clone(), style);
    }
}

/// Library style metadata from the top-level REST `styles` map. A FILL style
/// that is only ever used as a stroke gets [`StyleProperty::Stroke`].
fn style_meta(styles: Option<&Value>, doc: &Document) -> BTreeMap<String, StyleMeta> {
    let mut out = BTreeMap::new();
    let Some(map) = styles.and_then(Value::as_object) else { return out };
    if map.is_empty() {
        return out;
    }
    let mut used_as_fill: BTreeSet<&str> = BTreeSet::new();
    let mut used_as_stroke: BTreeSet<&str> = BTreeSet::new();
    for n in doc.nodes.values() {
        if n.component.source.is_some() {
            continue;
        }
        for (k, sid) in &n.style_refs {
            match k.as_str() {
                "stroke" => used_as_stroke.insert(sid),
                "fill" => used_as_fill.insert(sid),
                _ => continue,
            };
        }
    }
    for (id, m) in map {
        if !doc.styles.contains_key(id) {
            continue;
        }
        let property = match fstr(m, "styleType") {
            Some("TEXT") => StyleProperty::Text,
            Some("EFFECT") => StyleProperty::Effect,
            Some("GRID") => StyleProperty::Grid,
            Some("STROKE") => StyleProperty::Stroke,
            _ if used_as_stroke.contains(id.as_str()) && !used_as_fill.contains(id.as_str()) => StyleProperty::Stroke,
            _ => StyleProperty::Fill,
        };
        out.insert(
            id.clone(),
            StyleMeta {
                name: fstr(m, "name").unwrap_or("").to_string(),
                key: fstr(m, "key").map(String::from).filter(|k| !k.is_empty()),
                description: fstr(m, "description").unwrap_or("").to_string(),
                property,
            },
        );
    }
    out
}

/// Property name without Figma's `#<id>` suffix.
fn prop_name(key: &str) -> String {
    key.split('#').next().unwrap_or(key).to_string()
}

/// For an id inside an instance copy (`I<inst>;<a>;<b>`), the id of the node
/// of the main component it mirrors (`<a>`, or `I<a>;<b>` when nested).
fn counterpart_key(sid: &str) -> Option<String> {
    let rest = sid.strip_prefix('I')?;
    let mut segs = rest.split(';');
    segs.next()?;
    let tail: Vec<&str> = segs.collect();
    match tail.len() {
        0 => None,
        1 => Some(tail[0].to_string()),
        _ => Some(format!("I{}", tail.join(";"))),
    }
}

/// Maps a REST field name (as in `overriddenFields` or an override entry) to
/// the document-model override category.
pub(crate) fn override_field(key: &str, is_text: bool) -> Option<OverrideField> {
    Some(match key {
        "name" => OverrideField::Name,
        "visible" => OverrideField::Visible,
        "locked" => OverrideField::Locked,
        "opacity" => OverrideField::Opacity,
        "blendMode" => OverrideField::Blend,
        "size" => OverrideField::Size,
        "relativeTransform" | "transform" => OverrideField::Transform,
        "fills" if is_text => OverrideField::TextStyle,
        "fills" => OverrideField::Fills,
        "strokes" | "strokeWeight" | "strokeAlign" | "strokeCap" | "strokeJoin" | "strokeDashes" | "individualStrokeWeights" => OverrideField::Stroke,
        "effects" => OverrideField::Effects,
        "cornerRadius" | "rectangleCornerRadii" | "cornerSmoothing" => OverrideField::CornerRadii,
        "characters" => OverrideField::Characters,
        "style"
        | "characterStyleOverrides"
        | "styleOverrideTable"
        | "fontSize"
        | "fontName"
        | "fontFamily"
        | "textAutoResize"
        | "textTruncation"
        | "lineTypes"
        | "lineIndentations"
        | "opentypeFlags"
        | "fontVariations"
        | "maxLines" => OverrideField::TextStyle,
        "layoutMode"
        | "itemSpacing"
        | "counterAxisSpacing"
        | "paddingLeft"
        | "paddingRight"
        | "paddingTop"
        | "paddingBottom"
        | "primaryAxisAlignItems"
        | "counterAxisAlignItems"
        | "primaryAxisSizingMode"
        | "counterAxisSizingMode"
        | "layoutWrap"
        | "clipsContent"
        | "layoutGrids"
        | "layoutAlign"
        | "layoutGrow"
        | "layoutPositioning"
        | "layoutSizingHorizontal"
        | "layoutSizingVertical"
        | "gridColumns"
        | "gridRows"
        | "gridColumnsSizing"
        | "gridRowsSizing"
        | "gridColumnSpan"
        | "gridRowSpan"
        | "gridColumnAnchorIndex"
        | "gridRowAnchorIndex" => OverrideField::Layout,
        "componentId" | "mainComponent" | "overrides" => OverrideField::MainComponent,
        "slotContent" => OverrideField::SlotContent,
        _ => return None,
    })
}

fn abs_origin(v: &Value) -> Option<Vec2> {
    let b = v.get("absoluteBoundingBox")?;
    Some(Vec2::new(fnum(b, "x")?, fnum(b, "y")?))
}

fn parse_transform(v: &Value) -> Option<Affine> {
    let r = v.as_array()?;
    let row = |i: usize| -> Option<[f32; 3]> {
        let a = r.get(i)?.as_array()?;
        Some([num(a.first()?)?, num(a.get(1)?)?, num(a.get(2)?)?])
    };
    let (r0, r1) = (row(0)?, row(1)?);
    Some(Affine { a: r0[0], c: r0[1], tx: r0[2], b: r1[0], d: r1[1], ty: r1[2] })
}

fn parse_geometry_cached(items: &[Value], cache: &mut HashMap<String, Path>, blob_cache: &mut HashMap<(usize, bool), Path>, blobs: &[&[u8]]) -> Vec<Path> {
    let mut out = Vec::with_capacity(items.len());
    for g in items {
        let odd = matches!(fstr(g, "windingRule"), Some("EVENODD"));
        let rule = if odd { FillRule::EvenOdd } else { FillRule::NonZero };
        if let Some(idx) = g.get("_blob").and_then(Value::as_u64).map(|x| x as usize) {
            if let Some(p) = blob_cache.get(&(idx, odd)).cloned() {
                out.push(p);
            } else if let Some(blob) = blobs.get(idx) {
                let p = crate::fig::commands_blob_to_path(blob, odd);
                blob_cache.insert((idx, odd), p.clone());
                out.push(p);
            }
            continue;
        }
        let Some(d) = fstr(g, "path") else { continue };
        if let Some(mut p) = cache.get(d).cloned() {
            p.fill_rule = rule;
            out.push(p);
        } else {
            let p = parse_path(d, rule);
            cache.insert(d.to_string(), p.clone());
            out.push(p);
        }
    }
    out
}

/// Parses `.fig` v106 CSS-grid track strings (e.g. `"1fr 2fr 120px auto"`,
/// `"repeat(3, 1fr)"`, `"minmax(0, 1fr)"`) into track count + up to 8 [`GridTrack`]s.
fn parse_grid_sizing(s: &str) -> (u32, [GridTrack; 8]) {
    fn parse_one(tok: &str) -> GridTrack {
        let t = tok.trim();
        let lower = t.to_ascii_lowercase();
        if let Some(inner) = lower.strip_prefix("minmax(").and_then(|r| r.strip_suffix(')')) {
            if let Some((_, max_part)) = inner.rsplit_once(',') {
                return parse_one(max_part);
            }
        }
        if matches!(lower.as_str(), "auto" | "min-content" | "max-content" | "fit-content") {
            GridTrack::Hug
        } else if let Some(fr) = lower.strip_suffix("fr") {
            GridTrack::Flex(fr.trim().parse::<f32>().unwrap_or(1.0).max(0.01))
        } else if let Some(px) = lower.strip_suffix("px") {
            GridTrack::Fixed(px.trim().parse::<f32>().unwrap_or(0.0).max(0.0))
        } else if let Ok(v) = lower.parse::<f32>() {
            GridTrack::Fixed(v.max(0.0))
        } else {
            GridTrack::Flex(1.0)
        }
    }

    /// Tracks seen so far: the first [`KEPT_TRACKS`] are kept, the rest are only counted.
    #[derive(Default)]
    struct Tracks {
        kept: Vec<GridTrack>,
        count: u32,
    }

    impl Tracks {
        fn push(&mut self, t: GridTrack) {
            if self.kept.len() < KEPT_TRACKS {
                self.kept.push(t);
            }
            self.count = (self.count + 1).min(MAX_TRACK_COUNT);
        }

        /// Appends `times` copies of `sub` without materialising more than the kept prefix.
        fn push_repeated(&mut self, sub: &Tracks, times: u32) {
            if sub.count == 0 {
                return;
            }
            for _ in 0..times {
                if self.kept.len() >= KEPT_TRACKS {
                    break;
                }
                let room = KEPT_TRACKS - self.kept.len();
                self.kept.extend(sub.kept.iter().copied().take(room));
            }
            self.count = self.count.saturating_add(sub.count.saturating_mul(times)).min(MAX_TRACK_COUNT);
        }
    }

    const KEPT_TRACKS: usize = 8;
    const MAX_TRACK_COUNT: u32 = 1024;
    const MAX_REPEAT_DEPTH: usize = 8;

    fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
        s.as_bytes().get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
    }

    fn collect_tokens(input: &str, out: &mut Tracks, depth: usize) {
        let mut rest = input.trim();
        while !rest.is_empty() {
            if starts_with_ignore_case(rest, "repeat(") {
                if depth >= MAX_REPEAT_DEPTH {
                    return;
                }
                let mut paren_depth = 0usize;
                let mut end_idx = None;
                for (i, ch) in rest.char_indices() {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => {
                            paren_depth = paren_depth.saturating_sub(1);
                            if paren_depth == 0 {
                                end_idx = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                // An unclosed repeat() has no defined extent; stop rather than rescanning the tail per token.
                let Some(end) = end_idx else { return };
                let inner = &rest["repeat(".len()..end];
                if let Some((cnt_str, pat)) = inner.split_once(',') {
                    let cnt = cnt_str.trim().parse::<u32>().unwrap_or(1).min(64);
                    let mut sub = Tracks::default();
                    collect_tokens(pat, &mut sub, depth + 1);
                    out.push_repeated(&sub, cnt);
                }
                rest = rest[end + 1..].trim_start();
                continue;
            }
            if starts_with_ignore_case(rest, "minmax(") {
                // An unclosed minmax() has no defined extent; stop rather than rescanning the tail per token.
                let Some(end) = rest.find(')') else { return };
                out.push(parse_one(&rest[..=end]));
                rest = rest[end + 1..].trim_start();
                continue;
            }
            if let Some(ws) = rest.find(char::is_whitespace) {
                out.push(parse_one(&rest[..ws]));
                rest = rest[ws..].trim_start();
            } else {
                out.push(parse_one(rest));
                break;
            }
        }
    }

    let mut list = Tracks::default();
    collect_tokens(s, &mut list, 0);
    let mut tracks = [GridTrack::default(); KEPT_TRACKS];
    for (slot, t) in tracks.iter_mut().zip(list.kept.iter().copied()) {
        *slot = t;
    }
    (list.count, tracks)
}

fn frame_props(v: &Value) -> FrameProps {
    let dir = match fstr(v, "layoutMode") {
        Some("HORIZONTAL") => Some(LayoutDirection::Horizontal),
        Some("VERTICAL") => Some(LayoutDirection::Vertical),
        Some("GRID") => Some(LayoutDirection::Grid),
        _ => None,
    };
    let auto_layout = dir.map(|direction| {
        let align = |s: Option<&str>| match s {
            Some("CENTER") => LayoutAlign::Center,
            Some("MAX") => LayoutAlign::End,
            Some("SPACE_BETWEEN") => LayoutAlign::SpaceBetween,
            Some("SPACE_AROUND") => LayoutAlign::SpaceAround,
            Some("SPACE_EVENLY") => LayoutAlign::SpaceEvenly,
            Some("BASELINE") => LayoutAlign::Baseline,
            _ => LayoutAlign::Start,
        };
        let sizing = |s: Option<&str>| if s == Some("AUTO") { Sizing::Hug } else { Sizing::Fixed };
        let p = |k: &str| fnum(v, k).unwrap_or(0.0);
        let (sizing_cols, grid_col_tracks) = fstr(v, "gridColumnsSizing").map(parse_grid_sizing).unwrap_or_default();
        let (sizing_rows, grid_row_tracks) = fstr(v, "gridRowsSizing").map(parse_grid_sizing).unwrap_or_default();
        let grid_columns = v.get("gridColumns").and_then(Value::as_u64).map(|x| x as u32).filter(|&x| x > 0).unwrap_or(sizing_cols);
        let grid_rows = v.get("gridRows").and_then(Value::as_u64).map(|x| x as u32).filter(|&x| x > 0).unwrap_or(sizing_rows);
        AutoLayout {
            direction,
            wrap: fstr(v, "layoutWrap") == Some("WRAP"),
            item_spacing: p("itemSpacing"),
            counter_spacing: p("counterAxisSpacing"),
            padding: [p("paddingTop"), p("paddingRight"), p("paddingBottom"), p("paddingLeft")],
            primary_align: align(fstr(v, "primaryAxisAlignItems")),
            counter_align: align(fstr(v, "counterAxisAlignItems")),
            counter_align_content: if fstr(v, "counterAxisAlignContent") == Some("SPACE_BETWEEN") {
                CounterAlignContent::SpaceBetween
            } else {
                CounterAlignContent::Auto
            },
            layout_version: fnum(v, "stackLayoutVersion").map_or(0, |n| n.clamp(0.0, u8::MAX as f32) as u8),
            primary_sizing: sizing(fstr(v, "primaryAxisSizingMode")),
            counter_sizing: sizing(fstr(v, "counterAxisSizingMode")),
            stroke_included: fbool(v, "strokesIncludedInLayout").unwrap_or(false),
            reverse_z: fbool(v, "itemReverseZIndex").unwrap_or(false),
            grid_columns,
            grid_rows,
            grid_col_tracks,
            grid_row_tracks,
        }
    });
    FrameProps { clips_content: fbool(v, "clipsContent").unwrap_or(false), auto_layout, grids: farr(v, "layoutGrids").iter().filter_map(parse_grid).collect() }
}

fn text_props(v: &Value) -> TextProps {
    let content = fstr(v, "characters").unwrap_or("").to_string();
    let mut style = TextStyle::default();
    let st = v.get("style");
    if let Some(st) = st {
        apply_type_style(&mut style, st);
    }
    if v.get("fills").is_some() {
        style.fills = parse_paints(v.get("fills"));
    }
    let mut auto = fstr(v, "textAutoResize").or_else(|| st.and_then(|s| fstr(s, "textAutoResize"))).map(auto_resize).unwrap_or(TextAutoResize::None);
    if fstr(v, "textTruncation").or_else(|| st.and_then(|s| fstr(s, "textTruncation"))) == Some("ENDING") {
        auto = TextAutoResize::Truncate;
    }
    let max_lines =
        v.get("maxLines").or_else(|| st.and_then(|s| s.get("maxLines"))).and_then(Value::as_u64).filter(|n| *n > 0).map(|n| n.min(u32::MAX as u64) as u32);
    let auto_rename = fbool(v, "autoRename").or_else(|| st.and_then(|s| fbool(s, "autoRename"))).unwrap_or(true);
    let decoration_skip_ink = fbool(v, "textDecorationSkipInk").or_else(|| st.and_then(|s| fbool(s, "textDecorationSkipInk"))).unwrap_or(true);

    let overrides: Vec<u64> = farr(v, "characterStyleOverrides").iter().map(|x| x.as_u64().unwrap_or(0)).collect();
    let runs = build_runs(&content, &style, &overrides, v.get("styleOverrideTable"));
    let text_wrap = match fstr(v, "textWrapStyle").or_else(|| st.and_then(|s| fstr(s, "textWrapStyle"))) {
        Some("BALANCE") => TextWrap::Balance,
        Some("PRETTY") => TextWrap::Pretty,
        _ => TextWrap::Auto,
    };
    let mut tp = TextProps { content, style, runs, auto_resize: auto, max_lines, auto_rename, decoration_skip_ink, text_wrap, ..Default::default() };
    if let Some(st) = st {
        crate::textfmt::apply_paragraph(&mut tp, st);
    }
    crate::textfmt::apply_lines(&mut tp, v);
    tp
}

/// Style runs over byte ranges from per-UTF-16-unit override ids.
fn build_runs(content: &str, base: &TextStyle, overrides: &[u64], table: Option<&Value>) -> Vec<TextRun> {
    let mut runs = Vec::new();
    if overrides.iter().any(|&x| x != 0) {
        let offs = utf16_to_byte_offsets(content);
        let units = offs.len() - 1;
        let mut i = 0;
        while i < overrides.len().min(units) {
            let sid = overrides[i];
            let mut j = i + 1;
            while j < overrides.len().min(units) && overrides[j] == sid {
                j += 1;
            }
            if sid != 0 {
                if let Some(o) = table.and_then(|t| t.get(sid.to_string())) {
                    let mut rs = base.clone();
                    apply_type_style(&mut rs, o);
                    let (s, e) = (offs[i], offs[j]);
                    if e > s {
                        runs.push(TextRun { start: s, end: e, style: rs });
                    }
                }
            }
            i = j;
        }
    }
    runs
}

fn layout_child(v: &Value, parent: Option<LayoutDirection>, kind: &NodeKind) -> LayoutChild {
    let sizing = |s: Option<&str>| match s {
        Some("HUG") => Some(Sizing::Hug),
        Some("FILL") => Some(Sizing::Fill),
        Some("FIXED") => Some(Sizing::Fixed),
        _ => None,
    };
    let grow = fnum(v, "layoutGrow").unwrap_or(0.0);
    let u = |k: &str| v.get(k).and_then(Value::as_u64).map(|x| x as u16);
    let mut lc = LayoutChild {
        absolute: fstr(v, "layoutPositioning") == Some("ABSOLUTE"),
        grow,
        min_w: fnum(v, "minWidth"),
        max_w: fnum(v, "maxWidth"),
        min_h: fnum(v, "minHeight"),
        max_h: fnum(v, "maxHeight"),
        grid_column: u("gridColumnAnchorIndex"),
        grid_row: u("gridRowAnchorIndex"),
        col_span: u("gridColumnSpan").unwrap_or(1).max(1),
        row_span: u("gridRowSpan").unwrap_or(1).max(1),
        ..Default::default()
    };
    let (sh, sv) = (sizing(fstr(v, "layoutSizingHorizontal")), sizing(fstr(v, "layoutSizingVertical")));
    // Derive from the legacy fields when the explicit ones are absent.
    let mut h = Sizing::Fixed;
    let mut vt = Sizing::Fixed;
    let own_hug = |dir_h: bool| -> bool {
        match kind {
            NodeKind::Text(t) => match t.auto_resize {
                TextAutoResize::WidthAndHeight => true,
                TextAutoResize::Height => !dir_h,
                _ => false,
            },
            k => k.frame_props().and_then(|f| f.auto_layout).is_some_and(|al| al.sizing_along(dir_h) == Sizing::Hug),
        }
    };
    if own_hug(true) {
        h = Sizing::Hug;
    }
    if own_hug(false) {
        vt = Sizing::Hug;
    }
    if let Some(dir) = parent {
        let stretch = fstr(v, "layoutAlign") == Some("STRETCH");
        let (prim_fill, counter_fill) = (grow > 0.0, stretch);
        match dir {
            LayoutDirection::Horizontal | LayoutDirection::Grid => {
                if prim_fill {
                    h = Sizing::Fill;
                }
                if counter_fill {
                    vt = Sizing::Fill;
                }
            }
            LayoutDirection::Vertical => {
                if prim_fill {
                    vt = Sizing::Fill;
                }
                if counter_fill {
                    h = Sizing::Fill;
                }
            }
        }
    }
    lc.horizontal = sh.unwrap_or(h);
    lc.vertical = sv.unwrap_or(vt);
    lc
}

/// Every `imageRef` used by fills, strokes and text styles, sorted and deduplicated.
pub fn image_refs(doc: &Document) -> Vec<String> {
    let mut set = BTreeSet::new();
    let mut add = |ps: &[Paint]| {
        for p in ps {
            if let PaintKind::Image { image_ref, .. } = &p.kind {
                set.insert(image_ref.clone());
            }
        }
    };
    for n in doc.nodes.values() {
        add(&n.fills);
        add(&n.stroke.paints);
        if let NodeKind::Text(t) = &n.kind {
            add(&t.style.fills);
            for r in &t.runs {
                add(&r.style.fills);
            }
        }
    }
    for s in doc.styles.values() {
        match s {
            Style::Fill(p) => add(p),
            Style::Text(t) => add(&t.fills),
            _ => {}
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod expand_depth_tests {
    use super::*;
    use serde_json::json;

    /// Component `C{i}` holds an instance of `C{i+1}`; an instance of `C0` sits at the end.
    fn component_chain(len: usize) -> Value {
        let mut kids = Vec::new();
        for i in 0..len {
            let inner = if i + 1 < len {
                json!({"id": format!("{}:2", i + 10), "name": "N", "type": "INSTANCE", "componentId": format!("{}:1", i + 11), "size": {"x": 4, "y": 4}})
            } else {
                json!({"id": format!("{}:2", i + 10), "name": "Dot", "type": "RECTANGLE", "size": {"x": 4, "y": 4}})
            };
            kids.push(json!({"id": format!("{}:1", i + 10), "name": format!("C{i}"), "type": "COMPONENT", "size": {"x": 4, "y": 4}, "children": [inner]}));
        }
        kids.push(json!({"id": "1:1", "name": "Use", "type": "INSTANCE", "componentId": "10:1", "size": {"x": 4, "y": 4}}));
        json!({"document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "children": kids}]}})
    }

    fn subtree_of_named(doc: &Document, name: &str) -> Vec<String> {
        let root = doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id;
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let n = doc.get(id).unwrap();
            out.push(n.name.clone());
            stack.extend(n.children.iter().copied());
        }
        out
    }

    #[test]
    fn deep_component_chain_expands_completely() {
        // Use -> C0 -> C1 -> ... -> C(len-1) -> Dot: every link must be expanded before it is cloned, however deep.
        for len in [70usize, 100, 200, 400] {
            let doc = import_rest_value(&component_chain(len), true).unwrap();
            let names = subtree_of_named(&doc, "Use");
            assert_eq!(names.len(), len + 1, "chain of {len}");
            assert!(names.iter().any(|n| n == "Dot"), "chain of {len} lost its leaf");
        }
    }

    #[test]
    fn component_cycles_still_terminate() {
        // C0 holds an instance of C1 and C1 an instance of C0.
        let doc = json!({"document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "children": [
            {"id": "10:1", "name": "C0", "type": "COMPONENT", "size": {"x": 4, "y": 4}, "children": [
                {"id": "10:2", "name": "toC1", "type": "INSTANCE", "componentId": "11:1", "size": {"x": 4, "y": 4}}]},
            {"id": "11:1", "name": "C1", "type": "COMPONENT", "size": {"x": 4, "y": 4}, "children": [
                {"id": "11:2", "name": "toC0", "type": "INSTANCE", "componentId": "10:1", "size": {"x": 4, "y": 4}}]},
            {"id": "1:1", "name": "Use", "type": "INSTANCE", "componentId": "10:1", "size": {"x": 4, "y": 4}}]}]}});
        let imported = import_rest_value(&doc, true).unwrap();
        assert!(imported.nodes.len() < 50, "{}", imported.nodes.len());
    }

    #[test]
    fn long_component_chain_expands_without_exhausting_the_stack() {
        // A small stack stands in for a very long chain; the old recursion needed a frame pair per link.
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let doc = import_rest_value(&component_chain(700), true).unwrap();
                assert!(doc.nodes.len() > 700);
            })
            .unwrap();
        handle.join().expect("import must not overflow the stack");
    }

    /// `C{i}` holds an instance `X` of the holder `H`, whose own instance `S` is bound to the instance-swap property
    /// `p`; `X` sets `p` to `C{i+1}` (the last one to the plain component `B`). Expanding `X` therefore swaps `S`
    /// for `C{i+1}`, which first needs `C{i+1}`'s own `X` expanded, and so on down the whole chain.
    fn instance_swap_chain(len: usize) -> Value {
        let size = json!({"x": 4, "y": 4});
        let mut kids = vec![
            json!({"id": "4:1", "name": "B", "type": "COMPONENT", "size": size}),
            json!({"id": "5:1", "name": "H", "type": "COMPONENT", "size": size, "children": [
                {"id": "5:2", "name": "S", "type": "INSTANCE", "componentId": "4:1", "size": size,
                 "componentPropertyReferences": {"mainComponent": "p"}}]}),
        ];
        for i in 0..len {
            let next = if i + 1 < len { format!("{}:1", i + 11) } else { "4:1".to_string() };
            kids.push(json!({"id": format!("{}:1", i + 10), "name": format!("C{i}"), "type": "COMPONENT", "size": size, "children": [
                {"id": format!("{}:2", i + 10), "name": "X", "type": "INSTANCE", "componentId": "5:1", "size": size,
                 "componentProperties": {"p": {"type": "INSTANCE_SWAP", "value": next}}}]}));
        }
        kids.push(json!({"id": "1:1", "name": "Use", "type": "INSTANCE", "componentId": "10:1", "size": size}));
        json!({"document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "children": kids}]}})
    }

    /// Same chain, but each `X` swaps `S` through an instance override (`_overrides` with a `componentId`) rather
    /// than through a component property.
    fn override_swap_chain(len: usize) -> Value {
        let size = json!({"x": 4, "y": 4});
        let mut kids = vec![
            json!({"id": "4:1", "name": "B", "type": "COMPONENT", "size": size}),
            json!({"id": "5:1", "name": "H", "type": "COMPONENT", "size": size, "children": [
                {"id": "5:2", "name": "S", "type": "INSTANCE", "componentId": "4:1", "size": size}]}),
        ];
        for i in 0..len {
            let next = if i + 1 < len { format!("{}:1", i + 11) } else { "4:1".to_string() };
            kids.push(json!({"id": format!("{}:1", i + 10), "name": format!("C{i}"), "type": "COMPONENT", "size": size, "children": [
                {"id": format!("{}:2", i + 10), "name": "X", "type": "INSTANCE", "componentId": "5:1", "size": size,
                 "_overrides": [{"path": ["5:2"], "set": {"componentId": next}}]}]}));
        }
        kids.push(json!({"id": "1:1", "name": "Use", "type": "INSTANCE", "componentId": "10:1", "size": size}));
        json!({"document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "children": kids}]}})
    }

    /// Nesting depth of the deepest node below the node named `name` (iterative: the document's own `walk` recurses).
    fn depth_below_named(doc: &Document, name: &str) -> usize {
        let root = doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id;
        let (mut deepest, mut stack) = (0, vec![(root, 0usize)]);
        while let Some((id, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            stack.extend(doc.get(id).unwrap().children.iter().map(|&c| (c, depth + 1)));
        }
        deepest
    }

    #[test]
    fn instance_swap_chain_expands_completely() {
        // Use -> X -> S(swapped to C1) -> X -> S(swapped to C2) -> ... -> S(swapped to B): two nodes per link.
        for len in [3usize, 20, 80] {
            let doc = import_rest_value(&instance_swap_chain(len), true).unwrap();
            assert_eq!(subtree_of_named(&doc, "Use").len(), 2 * len + 1, "chain of {len}");
            assert_eq!(depth_below_named(&doc, "Use"), 2 * len, "chain of {len}");
        }
    }

    #[test]
    fn override_swap_chain_expands_completely_and_on_a_small_stack() {
        for len in [3usize, 20, 80] {
            let doc = import_rest_value(&override_swap_chain(len), true).unwrap();
            assert_eq!(subtree_of_named(&doc, "Use").len(), 2 * len + 1, "chain of {len}");
        }
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let doc = import_rest_value(&override_swap_chain(300), true).unwrap();
                assert_eq!(subtree_of_named(&doc, "Use").len(), 2 * 300 + 1);
            })
            .unwrap();
        handle.join().expect("import must not overflow the stack");
    }

    #[test]
    fn long_instance_swap_chain_expands_without_exhausting_the_stack() {
        // Swapping an instance used to expand the new component's contents from inside the swap itself, one
        // frame group per link, so a long chain of swaps overflowed even though plain nesting no longer did.
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let doc = import_rest_value(&instance_swap_chain(300), true).unwrap();
                assert_eq!(subtree_of_named(&doc, "Use").len(), 2 * 300 + 1);
            })
            .unwrap();
        handle.join().expect("import must not overflow the stack");
    }
}

#[cfg(test)]
mod grid_sizing_tests {
    use super::*;

    #[test]
    fn plain_and_repeat_tracks_keep_their_meaning() {
        let (n, t) = parse_grid_sizing("1fr 2fr 120px auto");
        assert_eq!(n, 4);
        assert_eq!(t[..4], [GridTrack::Flex(1.0), GridTrack::Flex(2.0), GridTrack::Fixed(120.0), GridTrack::Hug]);
        let (n, t) = parse_grid_sizing("REPEAT(3, 1fr 50px) auto");
        assert_eq!(n, 7);
        assert_eq!(
            t[..7],
            [
                GridTrack::Flex(1.0),
                GridTrack::Fixed(50.0),
                GridTrack::Flex(1.0),
                GridTrack::Fixed(50.0),
                GridTrack::Flex(1.0),
                GridTrack::Fixed(50.0),
                GridTrack::Hug
            ]
        );
        assert_eq!(parse_grid_sizing("minmax(0, 1fr)").0, 1);
    }

    #[test]
    fn nested_repeat_bomb_is_counted_not_expanded() {
        // 64^6 tracks if expanded.
        let s = format!("{}1fr{}", "repeat(64, ".repeat(6), ")".repeat(6));
        let (n, t) = parse_grid_sizing(&s);
        assert_eq!(n, 1024);
        assert!(t.iter().all(|x| *x == GridTrack::Flex(1.0)));
    }

    #[test]
    fn deeply_nested_repeat_stops_at_depth_cap() {
        let s = format!("{}1fr{}", "repeat(2, ".repeat(20_000), ")".repeat(20_000));
        let (n, _) = parse_grid_sizing(&s);
        assert!(n <= 1024);
    }

    #[test]
    fn long_track_list_parses_in_linear_time() {
        let s = "1fr ".repeat(200_000);
        let started = std::time::Instant::now();
        let (n, _) = parse_grid_sizing(&s);
        assert_eq!(n, 1024);
        assert!(started.elapsed().as_millis() < 900, "took {:?}", started.elapsed());
        let unclosed = "repeat( ".repeat(100_000);
        let started = std::time::Instant::now();
        let _ = parse_grid_sizing(&unclosed);
        assert!(started.elapsed().as_millis() < 900, "took {:?}", started.elapsed());
    }

    #[test]
    fn unclosed_minmax_stops_the_scan_in_linear_time() {
        // Everything after an unclosed minmax( has no defined extent, like an unclosed repeat(.
        let (n, t) = parse_grid_sizing("1fr 50px minmax( 2fr 3fr");
        assert_eq!(n, 2);
        assert_eq!(t[..2], [GridTrack::Flex(1.0), GridTrack::Fixed(50.0)]);
        // 80k such tokens rescanned the tail each time (846 ms measured quadratic); linear parsing takes a few ms.
        let s = "minmax( ".repeat(80_000);
        let started = std::time::Instant::now();
        let (n, _) = parse_grid_sizing(&s);
        assert_eq!(n, 0);
        assert!(started.elapsed().as_millis() < 250, "took {:?}", started.elapsed());
    }
}
