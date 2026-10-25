//! [`Document`] -> Figma REST-shaped JSON (the inverse of `import_rest_json`).

use crate::kiwi::clean_f32;
use crate::rest_import::{grid_child_align_name, utf16_to_byte_offsets, writes_grid_child_align};
use crate::svgpath::write_path;
use designos_doc::*;
use serde_json::{json, Map, Value};

pub(crate) fn n(f: f32) -> Value {
    clean_f32(f)
}

fn col(c: Color) -> Value {
    json!({"r": n(c.r), "g": n(c.g), "b": n(c.b), "a": n(c.a)})
}

pub(crate) fn v2(v: Vec2) -> Value {
    json!({"x": n(v.x), "y": n(v.y)})
}

fn blend_name(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal => "NORMAL",
        BlendMode::PassThrough => "PASS_THROUGH",
        BlendMode::Darken => "DARKEN",
        BlendMode::Multiply => "MULTIPLY",
        BlendMode::ColorBurn => "COLOR_BURN",
        BlendMode::LinearBurn => "LINEAR_BURN",
        BlendMode::Lighten => "LIGHTEN",
        BlendMode::Screen => "SCREEN",
        BlendMode::ColorDodge => "COLOR_DODGE",
        BlendMode::LinearDodge => "LINEAR_DODGE",
        BlendMode::Overlay => "OVERLAY",
        BlendMode::SoftLight => "SOFT_LIGHT",
        BlendMode::HardLight => "HARD_LIGHT",
        BlendMode::Difference => "DIFFERENCE",
        BlendMode::Exclusion => "EXCLUSION",
        BlendMode::Hue => "HUE",
        BlendMode::Saturation => "SATURATION",
        BlendMode::Color => "COLOR",
        BlendMode::Luminosity => "LUMINOSITY",
    }
}

fn stops(s: &[GradientStop]) -> Value {
    Value::Array(s.iter().map(|g| json!({"position": n(g.pos), "color": col(g.color)})).collect())
}

fn paint(p: &Paint) -> Value {
    let mut m = Map::new();
    let grad = |m: &mut Map<String, Value>, t: &str, h: &[Vec2; 3], s: &[GradientStop]| {
        m.insert("type".into(), t.into());
        m.insert("gradientHandlePositions".into(), Value::Array(h.iter().map(|p| v2(*p)).collect()));
        m.insert("gradientStops".into(), stops(s));
    };
    match &p.kind {
        PaintKind::Solid(c) => {
            m.insert("type".into(), "SOLID".into());
            m.insert("color".into(), col(*c));
        }
        PaintKind::LinearGradient { handles, stops } => grad(&mut m, "GRADIENT_LINEAR", handles, stops),
        PaintKind::RadialGradient { handles, stops } => grad(&mut m, "GRADIENT_RADIAL", handles, stops),
        PaintKind::AngularGradient { handles, stops } => grad(&mut m, "GRADIENT_ANGULAR", handles, stops),
        PaintKind::DiamondGradient { handles, stops } => grad(&mut m, "GRADIENT_DIAMOND", handles, stops),
        PaintKind::Image { image_ref, scale_mode } => {
            m.insert("type".into(), "IMAGE".into());
            m.insert("imageRef".into(), image_ref.clone().into());
            m.insert(
                "scaleMode".into(),
                match scale_mode {
                    ImageScaleMode::Fill => "FILL",
                    ImageScaleMode::Fit => "FIT",
                    ImageScaleMode::Crop => "STRETCH",
                    ImageScaleMode::Tile => "TILE",
                }
                .into(),
            );
        }
    }
    m.insert("opacity".into(), n(p.opacity));
    m.insert("visible".into(), p.visible.into());
    m.insert("blendMode".into(), blend_name(p.blend).into());
    Value::Object(m)
}

pub(crate) fn paints(ps: &[Paint]) -> Value {
    Value::Array(ps.iter().map(paint).collect())
}

fn effect(e: &Effect) -> Value {
    match e {
        Effect::DropShadow { color, offset, radius, spread, visible, show_behind } => json!({
            "type": "DROP_SHADOW", "color": col(*color), "offset": v2(*offset), "radius": n(*radius),
            "spread": n(*spread), "visible": visible, "showShadowBehindNode": show_behind, "blendMode": "NORMAL"
        }),
        Effect::InnerShadow { color, offset, radius, spread, visible } => json!({
            "type": "INNER_SHADOW", "color": col(*color), "offset": v2(*offset), "radius": n(*radius),
            "spread": n(*spread), "visible": visible, "blendMode": "NORMAL"
        }),
        Effect::LayerBlur { radius, visible } => json!({"type": "LAYER_BLUR", "radius": n(*radius), "visible": visible}),
        Effect::BackgroundBlur { radius, visible } => json!({"type": "BACKGROUND_BLUR", "radius": n(*radius), "visible": visible}),
        // Field names for the newer effect types are best guesses at Figma's
        // REST shape; import reads the same names back.
        Effect::Noise(e) => {
            let (mode, color, secondary) = match &e.mode {
                NoiseMode::Mono { color } => ("MONOTONE", Some(*color), None),
                NoiseMode::Duo { color, secondary } => ("DUOTONE", Some(*color), Some(*secondary)),
                NoiseMode::Multi => ("MULTITONE", None, None),
            };
            json!({
                "type": "NOISE", "noiseType": mode, "color": color.map(col), "secondaryColor": secondary.map(col),
                "noiseSize": n(e.size), "density": n(e.density), "opacity": n(e.opacity),
                "blendMode": blend_name(e.blend), "seed": e.seed, "visible": e.visible
            })
        }
        Effect::Texture(e) => json!({
            "type": "TEXTURE", "noiseSize": n(e.size), "radius": n(e.radius),
            "clipToShape": e.clip_to_shape, "seed": e.seed, "visible": e.visible
        }),
        Effect::Glass(e) => json!({
            "type": "GLASS", "lightAngle": n(e.light_angle), "lightIntensity": n(e.light_intensity),
            "refraction": n(e.refraction), "depth": n(e.depth), "dispersion": n(e.dispersion),
            "radius": n(e.frost), "visible": e.visible
        }),
    }
}

fn grid(g: &LayoutGrid) -> Value {
    match g {
        LayoutGrid::Grid { size, color, visible } => json!({"pattern": "GRID", "sectionSize": n(*size), "visible": *visible, "color": col(*color)}),
        LayoutGrid::Columns { count, gutter, offset, section_size, alignment, color, visible }
        | LayoutGrid::Rows { count, gutter, offset, section_size, alignment, color, visible } => {
            let pat = if matches!(g, LayoutGrid::Columns { .. }) { "COLUMNS" } else { "ROWS" };
            let align = match alignment {
                LayoutGridAlign::Min => "MIN",
                LayoutGridAlign::Center => "CENTER",
                LayoutGridAlign::Stretch => "STRETCH",
                LayoutGridAlign::Max => "MAX",
            };
            let cnt: i64 = if *count == 0 { -1 } else { i64::from(*count) };
            json!({
                "pattern": pat,
                "count": cnt,
                "gutterSize": n(*gutter),
                "offset": n(*offset),
                "visible": *visible,
                "color": col(*color),
                "alignment": align,
                // 0 is a real mode (equal-width stripes), so it is written like any other width
                "sectionSize": n(*section_size)
            })
        }
    }
}

/// Full REST `TypeStyle` for `ts` (includes `fills`).
fn type_style(ts: &TextStyle, auto: Option<TextAutoResize>, max_lines: Option<u32>) -> Value {
    let mut m = Map::new();
    m.insert("fontFamily".into(), ts.font_family.clone().into());
    m.insert("fontWeight".into(), ts.font_weight.into());
    m.insert("italic".into(), ts.italic.into());
    m.insert("fontSize".into(), n(ts.font_size));
    m.insert("letterSpacing".into(), n(ts.letter_spacing));
    m.insert("paragraphSpacing".into(), n(ts.paragraph_spacing));
    m.insert(
        "textAlignHorizontal".into(),
        match ts.align_h {
            TextAlignH::Left => "LEFT",
            TextAlignH::Center => "CENTER",
            TextAlignH::Right => "RIGHT",
            TextAlignH::Justified => "JUSTIFIED",
        }
        .into(),
    );
    m.insert(
        "textAlignVertical".into(),
        match ts.align_v {
            TextAlignV::Top => "TOP",
            TextAlignV::Center => "CENTER",
            TextAlignV::Bottom => "BOTTOM",
        }
        .into(),
    );
    match ts.line_height {
        LineHeight::Auto => {
            m.insert("lineHeightUnit".into(), "INTRINSIC_%".into());
        }
        LineHeight::Pixels(px) => {
            m.insert("lineHeightUnit".into(), "PIXELS".into());
            m.insert("lineHeightPx".into(), n(px));
        }
        LineHeight::Percent(p) => {
            m.insert("lineHeightUnit".into(), "FONT_SIZE_%".into());
            m.insert("lineHeightPercentFontSize".into(), n(p));
            m.insert("lineHeightPx".into(), n(p / 100.0 * ts.font_size));
        }
    }
    m.insert(
        "textCase".into(),
        match ts.case {
            TextCase::Original => "ORIGINAL",
            TextCase::Upper => "UPPER",
            TextCase::Lower => "LOWER",
            TextCase::Title => "TITLE",
            TextCase::SmallCaps => "SMALL_CAPS",
        }
        .into(),
    );
    m.insert(
        "textDecoration".into(),
        match ts.decoration {
            TextDecoration::None => "NONE",
            TextDecoration::Underline => "UNDERLINE",
            TextDecoration::Strikethrough => "STRIKETHROUGH",
        }
        .into(),
    );
    if let Some(a) = auto {
        m.insert(
            "textAutoResize".into(),
            match a {
                TextAutoResize::None => "NONE",
                TextAutoResize::WidthAndHeight => "WIDTH_AND_HEIGHT",
                TextAutoResize::Height => "HEIGHT",
                TextAutoResize::Truncate => "TRUNCATE",
            }
            .into(),
        );
    }
    if auto == Some(TextAutoResize::Truncate) {
        m.insert("textTruncation".into(), "ENDING".into());
    }
    if let Some(n) = max_lines {
        m.insert("maxLines".into(), n.into());
    }
    m.insert("fills".into(), paints(&ts.fills));
    if let Some(flags) = crate::textfmt::flags_value(&ts.features) {
        m.insert("opentypeFlags".into(), flags);
    }
    if !ts.variation_axes.is_empty() {
        let fvs: Vec<Value> = ts
            .variation_axes
            .iter()
            .map(|(tag, val)| {
                let mut bytes = [b' '; 4];
                for (i, b) in tag.as_bytes().iter().take(4).enumerate() {
                    bytes[i] = *b;
                }
                json!({"axisTag": u32::from_be_bytes(bytes), "axisName": tag, "value": n(*val)})
            })
            .collect();
        m.insert("fontVariations".into(), Value::Array(fvs));
    }
    Value::Object(m)
}

/// REST geometry list: SVG path data plus winding rule.
fn geometry(paths: &[Path]) -> Value {
    Value::Array(
        paths.iter().map(|p| json!({"path": write_path(p), "windingRule": if p.fill_rule == FillRule::EvenOdd { "EVENODD" } else { "NONZERO" }})).collect(),
    )
}

pub(crate) fn figma_id(doc: &Document, id: NodeId) -> String {
    doc.get(id).and_then(|n| n.source_id.clone()).unwrap_or_else(|| format!("9000:{}", id.0))
}

fn align_name(a: LayoutAlign) -> &'static str {
    match a {
        LayoutAlign::Start => "MIN",
        LayoutAlign::Center => "CENTER",
        LayoutAlign::End => "MAX",
        LayoutAlign::SpaceBetween => "SPACE_BETWEEN",
        LayoutAlign::SpaceAround => "SPACE_AROUND",
        LayoutAlign::SpaceEvenly => "SPACE_EVENLY",
        LayoutAlign::Baseline => "BASELINE",
    }
}

fn sizing_name(s: Sizing) -> &'static str {
    match s {
        Sizing::Fixed => "FIXED",
        Sizing::Hug => "HUG",
        Sizing::Fill => "FILL",
    }
}

fn frame_fields(m: &mut Map<String, Value>, f: &FrameProps, doc: &Document, node: &Node) {
    m.insert("clipsContent".into(), f.clips_content.into());
    if !f.grids.is_empty() {
        m.insert("layoutGrids".into(), Value::Array(f.grids.iter().map(grid).collect()));
    }
    let Some(al) = &f.auto_layout else {
        m.insert("layoutMode".into(), "NONE".into());
        return;
    };
    let mode = match al.direction {
        LayoutDirection::Horizontal => "HORIZONTAL",
        LayoutDirection::Vertical => "VERTICAL",
        LayoutDirection::Grid => "GRID",
    };
    m.insert("layoutMode".into(), mode.into());
    if al.grid_columns > 0 {
        m.insert("gridColumns".into(), al.grid_columns.into());
        m.insert("gridColumnsSizing".into(), crate::fig_export::format_grid_tracks(al.grid_columns, &al.grid_col_tracks).into());
    }
    if let Some((rows, sizing)) = crate::fig_export::grid_rows_for_export(doc, node, al) {
        m.insert("gridRows".into(), rows.into());
        m.insert("gridRowsSizing".into(), sizing.into());
    }
    if let Some(auto_tracks) = crate::fig_export::format_grid_auto_tracks(al.grid_auto_tracks) {
        m.insert("gridAutoTracks".into(), auto_tracks.into());
    }
    m.insert("layoutWrap".into(), if al.wrap { "WRAP" } else { "NO_WRAP" }.into());
    m.insert("itemSpacing".into(), n(al.item_spacing));
    m.insert("counterAxisSpacing".into(), n(al.counter_spacing));
    m.insert("paddingTop".into(), n(al.padding[0]));
    m.insert("paddingRight".into(), n(al.padding[1]));
    m.insert("paddingBottom".into(), n(al.padding[2]));
    m.insert("paddingLeft".into(), n(al.padding[3]));
    m.insert("primaryAxisAlignItems".into(), align_name(al.primary_align).into());
    m.insert("counterAxisAlignItems".into(), align_name(al.counter_align).into());
    m.insert("primaryAxisSizingMode".into(), if al.primary_sizing == Sizing::Hug { "AUTO" } else { "FIXED" }.into());
    m.insert("counterAxisSizingMode".into(), if al.counter_sizing == Sizing::Hug { "AUTO" } else { "FIXED" }.into());
    if al.counter_align_content == CounterAlignContent::SpaceBetween {
        m.insert("counterAxisAlignContent".into(), "SPACE_BETWEEN".into());
    }
    if al.layout_version > 0 {
        m.insert("stackLayoutVersion".into(), al.layout_version.into());
    }
    m.insert("strokesIncludedInLayout".into(), al.stroke_included.into());
    m.insert("itemReverseZIndex".into(), al.reverse_z.into());
}

fn export_preset(p: &ExportPreset) -> Value {
    let format = match p.format {
        ExportFileFormat::Png => "PNG",
        ExportFileFormat::Jpg => "JPG",
        ExportFileFormat::Svg => "SVG",
        ExportFileFormat::Pdf => "PDF",
    };
    let (ty, value) = match p.constraint {
        ExportConstraint::Scale(v) => ("SCALE", v),
        ExportConstraint::Width(v) => ("WIDTH", v),
        ExportConstraint::Height(v) => ("HEIGHT", v),
    };
    json!({"suffix": p.suffix, "format": format, "constraint": {"type": ty, "value": n(value)}})
}

/// Ruler guides that belong to `id`: a page's own guides, or the guides of a frame.
fn guides_of(doc: &Document, id: NodeId) -> Vec<Value> {
    if doc.guides.is_empty() {
        return vec![];
    }
    let guide = |g: &Guide| json!({"axis": if g.axis == GuideAxis::X { "X" } else { "Y" }, "offset": n(g.offset)});
    if matches!(doc.get(id).map(|n| &n.kind), Some(NodeKind::Page { .. })) {
        return doc.guides.get(&id).map(|l| l.iter().filter(|g| g.frame.is_none()).map(guide).collect()).unwrap_or_default();
    }
    match doc.page_of(id).and_then(|p| doc.guides.get(&p)) {
        Some(l) => l.iter().filter(|g| g.frame == Some(id)).map(guide).collect(),
        None => vec![],
    }
}

fn export_node(doc: &Document, id: NodeId, parent_abs: Option<&Affine>, depth: usize) -> Option<Value> {
    if depth > 400 {
        return None;
    }
    let node = doc.get(id)?;
    let mut m = Map::new();
    m.insert("id".into(), figma_id(doc, id).into());
    m.insert("name".into(), node.name.clone().into());
    m.insert("visible".into(), node.visible.into());
    if node.locked {
        m.insert("locked".into(), true.into());
    }
    let ty = match &node.kind {
        NodeKind::Page { .. } => "CANVAS",
        NodeKind::Frame(_) => "FRAME",
        NodeKind::Group => "GROUP",
        NodeKind::Section => "SECTION",
        NodeKind::Rectangle => "RECTANGLE",
        NodeKind::Ellipse(_) => "ELLIPSE",
        NodeKind::Line => "LINE",
        NodeKind::Polygon { .. } => "REGULAR_POLYGON",
        NodeKind::Star { .. } => "STAR",
        NodeKind::Vector { .. } => "VECTOR",
        NodeKind::Text(_) => "TEXT",
        NodeKind::BooleanOp(_) => "BOOLEAN_OPERATION",
        NodeKind::Component(_) => "COMPONENT",
        NodeKind::ComponentSet(_) => "COMPONENT_SET",
        NodeKind::Instance { .. } => "INSTANCE",
        NodeKind::Unknown { type_name } => type_name.as_str(),
        NodeKind::Sticky(_) | NodeKind::Connector(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. } => {
            crate::board::type_name(&node.kind).unwrap_or("SECTION")
        }
    };
    m.insert("type".into(), ty.into());
    crate::variables::export_node_variables(node, &mut m);
    if !node.export_settings.is_empty() {
        m.insert("exportSettings".into(), Value::Array(node.export_settings.iter().map(export_preset).collect()));
    }
    crate::dev::export_dev(node, &mut m);
    let guides = guides_of(doc, id);
    if !guides.is_empty() {
        m.insert("guides".into(), Value::Array(guides));
    }
    if let Some(ip) = doc.in_progress.get(&id) {
        if let Ok(val) = serde_json::to_string(ip) {
            let mut pd = Map::new();
            pd.insert("designos:in_progress".into(), Value::String(val));
            m.insert("pluginData".into(), Value::Object(pd));
        }
    }

    let abs = parent_abs.map(|p| *p * node.transform).unwrap_or(node.transform);
    match &node.kind {
        NodeKind::Page { background } => {
            m.insert("backgroundColor".into(), col(*background));
        }
        _ => {
            m.insert("opacity".into(), n(node.opacity));
            m.insert("blendMode".into(), blend_name(node.blend).into());
            m.insert(
                "relativeTransform".into(),
                json!([[n(node.transform.a), n(node.transform.c), n(node.transform.tx)], [n(node.transform.b), n(node.transform.d), n(node.transform.ty)]]),
            );
            m.insert("size".into(), v2(node.size));
            // Bounding box of the transformed rect in page space.
            let corners = [Vec2::ZERO, Vec2::new(node.size.x, 0.0), Vec2::new(0.0, node.size.y), node.size].map(|c| abs.apply(c));
            let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.x), a.1.max(c.x)));
            let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.y), a.1.max(c.y)));
            m.insert("absoluteBoundingBox".into(), json!({"x": n(x0), "y": n(y0), "width": n(x1 - x0), "height": n(y1 - y0)}));
            m.insert("constraints".into(), {
                let h = match node.constraints.horizontal {
                    Constraint::Min => "LEFT",
                    Constraint::Max => "RIGHT",
                    Constraint::Center => "CENTER",
                    Constraint::Stretch => "LEFT_RIGHT",
                    Constraint::Scale => "SCALE",
                };
                let v = match node.constraints.vertical {
                    Constraint::Min => "TOP",
                    Constraint::Max => "BOTTOM",
                    Constraint::Center => "CENTER",
                    Constraint::Stretch => "TOP_BOTTOM",
                    Constraint::Scale => "SCALE",
                };
                json!({"horizontal": h, "vertical": v})
            });
            if node.is_mask {
                m.insert("isMask".into(), true.into());
            }
            if !matches!(node.kind, NodeKind::Text(_)) {
                m.insert("fills".into(), paints(&node.fills));
            }
            let s = &node.stroke;
            m.insert("strokes".into(), paints(&s.paints));
            m.insert("strokeWeight".into(), n(s.weight));
            if let Some(w) = s.side_weights {
                m.insert("individualStrokeWeights".into(), json!({"top": n(w[0]), "right": n(w[1]), "bottom": n(w[2]), "left": n(w[3])}));
            }
            m.insert(
                "strokeAlign".into(),
                match s.align {
                    StrokeAlign::Inside => "INSIDE",
                    StrokeAlign::Center => "CENTER",
                    StrokeAlign::Outside => "OUTSIDE",
                }
                .into(),
            );
            m.insert(
                "strokeCap".into(),
                match s.cap {
                    StrokeCap::None => "NONE",
                    StrokeCap::Round => "ROUND",
                    StrokeCap::Square => "SQUARE",
                }
                .into(),
            );
            m.insert(
                "strokeJoin".into(),
                match s.join {
                    StrokeJoin::Miter => "MITER",
                    StrokeJoin::Bevel => "BEVEL",
                    StrokeJoin::Round => "ROUND",
                }
                .into(),
            );
            if s.miter_limit >= 1.0 {
                m.insert("strokeMiterLimit".into(), n(s.miter_limit));
                m.insert("strokeMiterAngle".into(), n((2.0 * (1.0 / s.miter_limit).asin()).to_degrees()));
            }
            if !s.dashes.is_empty() {
                m.insert("strokeDashes".into(), Value::Array(s.dashes.iter().map(|d| n(*d)).collect()));
            }
            m.insert("effects".into(), Value::Array(node.effects.iter().map(effect).collect()));
            if node.corner_radii.iter().all(|r| *r == node.corner_radii[0]) {
                if node.corner_radii[0] != 0.0 {
                    m.insert("cornerRadius".into(), n(node.corner_radii[0]));
                }
            } else {
                m.insert("rectangleCornerRadii".into(), Value::Array(node.corner_radii.iter().map(|r| n(*r)).collect()));
            }
            if node.corner_smoothing != 0.0 {
                m.insert("cornerSmoothing".into(), n(node.corner_smoothing));
            }
            // Child layout.
            let lc = &node.layout_child;
            m.insert("layoutSizingHorizontal".into(), sizing_name(lc.horizontal).into());
            m.insert("layoutSizingVertical".into(), sizing_name(lc.vertical).into());
            if lc.grow != 0.0 {
                m.insert("layoutGrow".into(), n(lc.grow));
            }
            let align_self_str = match lc.align_self {
                AlignSelf::Start => Some("MIN"),
                AlignSelf::Center => Some("CENTER"),
                AlignSelf::End => Some("MAX"),
                AlignSelf::Stretch => Some("STRETCH"),
                AlignSelf::Baseline => Some("BASELINE"),
                AlignSelf::Auto => None,
            };
            if let Some(a) = align_self_str {
                m.insert("layoutAlign".into(), a.into());
            }
            if lc.absolute {
                m.insert("layoutPositioning".into(), "ABSOLUTE".into());
            }
            if let Some(c) = lc.grid_column {
                m.insert("gridColumnAnchorIndex".into(), c.into());
            }
            if let Some(r) = lc.grid_row {
                m.insert("gridRowAnchorIndex".into(), r.into());
            }
            if lc.col_span > 1 {
                m.insert("gridColumnSpan".into(), lc.col_span.into());
            }
            if lc.row_span > 1 {
                m.insert("gridRowSpan".into(), lc.row_span.into());
            }
            let parent_dir = node.parent.and_then(|p| doc.get(p)).and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).map(|a| a.direction);
            for (k, a) in [("gridChildHorizontalAlign", lc.grid_align_h), ("gridChildVerticalAlign", lc.grid_align_v)] {
                if writes_grid_child_align(a, parent_dir) {
                    m.insert(k.into(), grid_child_align_name(a).into());
                }
            }
            for (k, v) in [("minWidth", lc.min_w), ("maxWidth", lc.max_w), ("minHeight", lc.min_h), ("maxHeight", lc.max_h)] {
                if let Some(v) = v {
                    m.insert(k.into(), n(v));
                }
            }
            for (k, v) in [
                ("stackChildMarginTop", lc.margin[0]),
                ("stackChildMarginRight", lc.margin[1]),
                ("stackChildMarginBottom", lc.margin[2]),
                ("stackChildMarginLeft", lc.margin[3]),
            ] {
                if v != 0.0 {
                    m.insert(k.into(), n(v));
                }
            }
            if !node.style_refs.is_empty() {
                let mut sm = Map::new();
                for (k, v) in &node.style_refs {
                    sm.insert(k.clone(), v.clone().into());
                }
                m.insert("styles".into(), Value::Object(sm));
            }
        }
    }

    if let Some(g) = &node.fill_geometry {
        m.insert("fillGeometry".into(), geometry(g));
    }
    if let Some(g) = &node.stroke_geometry {
        m.insert("strokeGeometry".into(), geometry(g));
    }

    match &node.kind {
        NodeKind::Frame(f) | NodeKind::Component(f) | NodeKind::ComponentSet(f) => {
            frame_fields(&mut m, f, doc, node);
        }
        NodeKind::Instance { frame, component, component_key } => {
            frame_fields(&mut m, frame, doc, node);
            if let Some(c) = component {
                m.insert("componentId".into(), figma_id(doc, *c).into());
            } else if let Some(k) = component_key {
                m.insert("componentKey".into(), k.clone().into());
            }
        }
        NodeKind::Ellipse(a) => {
            m.insert("arcData".into(), json!({"startingAngle": n(a.start), "endingAngle": n(a.end), "innerRadius": n(a.inner_radius)}));
        }
        NodeKind::Polygon { count } => {
            m.insert("pointCount".into(), (*count).into());
        }
        NodeKind::Star { count, inner_ratio } => {
            m.insert("pointCount".into(), (*count).into());
            m.insert("starInnerRadius".into(), n(*inner_ratio));
        }
        NodeKind::Vector { paths, network } => {
            if node.fill_geometry.is_none() {
                m.insert("fillGeometry".into(), geometry(paths));
            }
            if let Some(net) = network {
                m.insert("vectorNetwork".into(), crate::vecnet::network_to_json(net));
            }
        }
        NodeKind::Sticky(_) | NodeKind::Connector(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. } => {
            crate::board::export_fields(node, parent_abs.map(|p| p.translation()).unwrap_or(Vec2::ZERO), &mut m, &|t| figma_id(doc, t));
        }
        NodeKind::BooleanOp(op) => {
            m.insert(
                "booleanOperation".into(),
                match op {
                    BooleanOp::Union => "UNION",
                    BooleanOp::Subtract => "SUBTRACT",
                    BooleanOp::Intersect => "INTERSECT",
                    BooleanOp::Exclude => "EXCLUDE",
                }
                .into(),
            );
        }
        NodeKind::Text(t) => {
            m.insert("characters".into(), t.content.clone().into());
            m.insert("fills".into(), paints(&t.style.fills));
            let mut style = type_style(&t.style, Some(t.auto_resize), t.max_lines);
            if let Some(sm) = style.as_object_mut() {
                crate::textfmt::export_paragraph(t, sm);
            }
            m.insert("style".into(), style);
            match t.text_wrap {
                TextWrap::Auto => {}
                TextWrap::Balance => {
                    m.insert("textWrapStyle".into(), "BALANCE".into());
                }
                TextWrap::Pretty => {
                    m.insert("textWrapStyle".into(), "PRETTY".into());
                }
            }
            crate::textfmt::export_lines(t, &mut m);
            if !t.runs.is_empty() {
                let offs = utf16_to_byte_offsets(&t.content);
                let mut overrides = vec![0u32; offs.len() - 1];
                let mut table = Map::new();
                for (i, r) in t.runs.iter().enumerate() {
                    let id = (i + 1) as u32;
                    let s = offs.partition_point(|&o| o < r.start);
                    let e = offs.partition_point(|&o| o < r.end);
                    let cap = e.min(overrides.len());
                    for slot in overrides.iter_mut().take(cap).skip(s) {
                        *slot = id;
                    }
                    table.insert(id.to_string(), type_style(&r.style, None, None));
                }
                m.insert("characterStyleOverrides".into(), json!(overrides));
                m.insert("styleOverrideTable".into(), Value::Object(table));
            }
        }
        _ => {}
    }

    if node.kind.is_container() {
        let mut kids = Vec::new();
        for c in &node.children {
            let pa = if matches!(node.kind, NodeKind::Page { .. }) { None } else { Some(&abs) };
            if let Some(k) = export_node(doc, *c, pa, depth + 1) {
                kids.push(k);
            }
        }
        m.insert("children".into(), Value::Array(kids));
    }
    Some(Value::Object(m))
}

/// Produces JSON shaped like Figma's `GET /v1/files/:key` response.
pub fn export_rest_json(doc: &Document) -> Value {
    let mut pages: Vec<Value> = doc.pages.iter().filter_map(|p| export_node(doc, *p, None, 0)).collect();
    crate::proto::export_into(doc, &mut pages);
    let mut styles = Map::new();
    for (id, s) in &doc.styles {
        let ty = match s {
            Style::Fill(_) => "FILL",
            Style::Text(_) => "TEXT",
            Style::Effect(_) => "EFFECT",
            Style::Grid(_) => "GRID",
        };
        let mut e = Map::new();
        let meta = doc.style_meta.get(id);
        if let Some(k) = meta.and_then(|m| m.key.as_ref()) {
            e.insert("key".into(), k.clone().into());
        }
        e.insert("name".into(), doc.style_name(id).into());
        e.insert("styleType".into(), ty.into());
        e.insert("description".into(), meta.map(|m| m.description.clone()).unwrap_or_default().into());
        styles.insert(id.clone(), Value::Object(e));
    }
    // Components referenced by instances.
    let mut components = Map::new();
    for node in doc.nodes.values() {
        if let NodeKind::Instance { component: Some(c), component_key: Some(k), .. } = &node.kind {
            components.insert(figma_id(doc, *c), json!({"key": k, "name": doc.get(*c).map(|n| n.name.as_str()).unwrap_or("")}));
        }
    }
    let variables = (!doc.variable_collections.is_empty()).then(|| crate::variables::export_variables_meta(doc));
    let mut out = json!({
        "name": doc.name,
        "role": "owner",
        "lastModified": "",
        "version": "",
        "schemaVersion": 0,
        "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": pages},
        "components": components,
        "styles": styles,
    });
    if let (Some(v), Some(o)) = (variables, out.as_object_mut()) {
        o.insert("variables".into(), v);
    }
    if doc.kind == DocumentKind::Board {
        out["editorType"] = "figjam".into();
    }
    out
}
