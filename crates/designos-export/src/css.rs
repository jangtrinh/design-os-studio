//! Figma Dev Mode style CSS for a node.

use crate::svg::blend_css;
use crate::util::{css_color, num, num2};
use designos_doc::*;

/// CSS declarations (`prop: value;`, one per line) describing `node` the way
/// Figma's Dev Mode inspect panel prints them.
pub fn css_for(doc: &Document, node: NodeId) -> String {
    let Some(n) = doc.get(node) else { return String::new() };
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: &str, v: String| out.push(format!("{p}: {v};"));

    let parent = n.parent.and_then(|p| doc.get(p));
    let parent_al = parent.and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout);
    let in_flow = parent_al.is_some() && !n.layout_child.absolute;
    let is_text = matches!(n.kind, NodeKind::Text(_));
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout);

    // position
    if let Some(p) = parent {
        if !in_flow && !matches!(p.kind, NodeKind::Page { .. }) {
            push("position", "absolute".into());
            push("left", format!("{}px", num(n.transform.tx)));
            push("top", format!("{}px", num(n.transform.ty)));
        }
    }
    let rot = n.transform.rotation_rad().to_degrees();
    if rot.abs() > 0.01 {
        push("transform", format!("rotate({}deg)", num2(rot)));
    }

    // display + size
    if let Some(al) = own_al {
        if al.direction == LayoutDirection::Grid {
            push("display", "grid".into());
        } else {
            push("display", "flex".into());
        }
    }
    let grid_parent = parent_al.is_some_and(|a| a.direction == LayoutDirection::Grid);
    let row_parent = parent_al.map(|a| a.direction == LayoutDirection::Horizontal).unwrap_or(true);
    let hug_w = own_al.map(|a| if a.direction == LayoutDirection::Horizontal { a.primary_sizing } else { a.counter_sizing } == Sizing::Hug).unwrap_or(false);
    let hug_h = own_al.map(|a| if a.direction == LayoutDirection::Horizontal { a.counter_sizing } else { a.primary_sizing } == Sizing::Hug).unwrap_or(false);
    let text_auto_w = matches!(&n.kind, NodeKind::Text(t) if t.auto_resize == TextAutoResize::WidthAndHeight);
    let text_auto_h = matches!(&n.kind, NodeKind::Text(t) if matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height));
    let fill_w = in_flow && (n.layout_child.horizontal == Sizing::Fill);
    let fill_h = in_flow && (n.layout_child.vertical == Sizing::Fill);
    let grows = in_flow && n.layout_child.grow > 0.0;
    let mut flex_done = false;
    let mut stretch = false;
    if in_flow && grid_parent {
        if n.layout_child.col_span > 1 {
            push("grid-column", format!("span {}", n.layout_child.col_span));
        }
        if n.layout_child.row_span > 1 {
            push("grid-row", format!("span {}", n.layout_child.row_span));
        }
    } else {
        for (is_w, fill) in [(true, fill_w), (false, fill_h)] {
            let main = is_w == row_parent;
            if fill || (grows && main) {
                if main {
                    if !flex_done {
                        push("flex", "1 0 0".into());
                        flex_done = true;
                    }
                } else {
                    stretch = true;
                }
            }
        }
    }
    let width_skipped = fill_w || (grows && row_parent) || hug_w || text_auto_w || (in_flow && grid_parent && n.layout_child.col_span > 1);
    let height_skipped = fill_h || (grows && !row_parent) || hug_h || text_auto_h || (in_flow && grid_parent && n.layout_child.row_span > 1);
    if !width_skipped {
        push("width", format!("{}px", num(n.size.x)));
    }
    if !height_skipped || matches!(n.kind, NodeKind::Line) {
        push("height", format!("{}px", num(if matches!(n.kind, NodeKind::Line) { 0.0 } else { n.size.y })));
    }
    if stretch {
        push("align-self", "stretch".into());
    }
    if in_flow && !grid_parent && !flex_done && (!width_skipped || !height_skipped) {
        push("flex-shrink", "0".into());
    }

    // auto layout container
    if let Some(al) = own_al {
        let [t, r, b, l] = al.padding;
        if t + r + b + l > 0.0 {
            push("padding", box_shorthand(t, r, b, l));
        }
        if al.direction == LayoutDirection::Grid {
            let cols = (if al.grid_columns == 0 { 3 } else { al.grid_columns }).max(1);
            push("grid-template-columns", format!("repeat({cols}, minmax(0, 1fr))"));
            if al.grid_rows > 0 {
                push("grid-template-rows", format!("repeat({}, minmax(0, 1fr))", al.grid_rows));
            }
            let col = al.item_spacing;
            let row = if al.counter_spacing > 0.0 { al.counter_spacing } else { al.item_spacing };
            if row > 0.0 || col > 0.0 {
                push("gap", if (row - col).abs() < 1e-4 { format!("{}px", num(row)) } else { format!("{}px {}px", num(row), num(col)) });
            }
        } else {
            if al.direction == LayoutDirection::Vertical {
                push("flex-direction", "column".into());
            }
            let justify = match al.primary_align {
                LayoutAlign::Start | LayoutAlign::Baseline => None,
                LayoutAlign::Center => Some("center"),
                LayoutAlign::End => Some("flex-end"),
                LayoutAlign::SpaceBetween => Some("space-between"),
                LayoutAlign::SpaceAround => Some("space-around"),
                LayoutAlign::SpaceEvenly => Some("space-evenly"),
            };
            if let Some(j) = justify {
                push("justify-content", j.into());
            }
            push(
                "align-items",
                match al.counter_align {
                    LayoutAlign::Start | LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => "flex-start",
                    LayoutAlign::Center => "center",
                    LayoutAlign::End => "flex-end",
                    LayoutAlign::Baseline => "baseline",
                }
                .into(),
            );
            if al.wrap {
                push("flex-wrap", "wrap".into());
                if al.counter_align_content == CounterAlignContent::SpaceBetween {
                    push("align-content", "space-between".into());
                }
                if !distributes_space(al.primary_align) || al.counter_spacing > 0.0 {
                    let (row, col) =
                        if al.direction == LayoutDirection::Horizontal { (al.counter_spacing, al.item_spacing) } else { (al.item_spacing, al.counter_spacing) };
                    if row > 0.0 || col > 0.0 {
                        push("gap", if (row - col).abs() < 1e-4 { format!("{}px", num(row)) } else { format!("{}px {}px", num(row), num(col)) });
                    }
                }
            } else if al.item_spacing > 0.0 && !distributes_space(al.primary_align) {
                push("gap", format!("{}px", num(al.item_spacing)));
            }
        }
    }

    // text block
    if let NodeKind::Text(t) = &n.kind {
        text_block(t, &mut out);
    }

    // background
    let fills: Vec<&Paint> = match &n.kind {
        NodeKind::Text(_) => Vec::new(),
        _ => n.fills.iter().filter(|p| p.visible && p.opacity > 0.0).collect(),
    };
    let mut push = |p: &str, v: String| out.push(format!("{p}: {v};"));
    if !fills.is_empty() && !matches!(n.kind, NodeKind::Line) {
        // CSS lists the top layer first; fills are stored bottom to top.
        let layers: Vec<String> = fills.iter().rev().map(|p| paint_css(p, n.size)).collect();
        push("background", layers.join(", "));
    }

    // border / outline
    border(n, &mut out);
    let mut push = |p: &str, v: String| out.push(format!("{p}: {v};"));

    // radius
    let rounded = matches!(n.kind, NodeKind::Frame(_) | NodeKind::Rectangle | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. });
    if rounded {
        let r = n.corner_radii;
        if r.iter().any(|v| *v > 0.0) {
            push("border-radius", corners_shorthand(r));
        }
    } else if let NodeKind::Ellipse(arc) = &n.kind {
        if (arc.end - arc.start).abs() >= std::f32::consts::TAU - 1e-4 && arc.inner_radius <= 0.0 {
            push("border-radius", "50%".into());
        }
    }
    if n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) && !n.children.is_empty() {
        push("overflow", "hidden".into());
    }

    // effects
    let mut shadows = Vec::new();
    let mut text_shadows = Vec::new();
    for e in &n.effects {
        match e {
            Effect::DropShadow { color, offset, radius, spread, visible: true, .. } => {
                let c = css_color(color, 1.0);
                shadows.push(format!("{}px {}px {}px {}px {c}", num(offset.x), num(offset.y), num(*radius), num(*spread)));
                text_shadows.push(format!("{}px {}px {}px {c}", num(offset.x), num(offset.y), num(*radius)));
            }
            Effect::InnerShadow { color, offset, radius, spread, visible: true } => {
                shadows.push(format!("inset {}px {}px {}px {}px {}", num(offset.x), num(offset.y), num(*radius), num(*spread), css_color(color, 1.0)));
            }
            _ => {}
        }
    }
    if is_text {
        if !text_shadows.is_empty() {
            push("text-shadow", text_shadows.join(", "));
        }
    } else if !shadows.is_empty() {
        push("box-shadow", shadows.join(", "));
    }
    for e in &n.effects {
        match e {
            Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => push("filter", format!("blur({}px)", num(radius / 2.0))),
            Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => push("backdrop-filter", format!("blur({}px)", num(radius / 2.0))),
            // Glass: the frosted backdrop maps to backdrop-filter; refraction, dispersion and the
            // light highlight have no CSS equivalent. Noise and texture are skipped as well.
            Effect::Glass(g) if g.visible && g.frost > 0.0 => push("backdrop-filter", format!("blur({}px)", num(g.frost / 2.0))),
            _ => {}
        }
    }
    if n.opacity < 0.999 {
        push("opacity", num2(n.opacity.max(0.0)));
    }
    if let Some(b) = blend_css(n.blend) {
        push("mix-blend-mode", b.into());
    }
    crate::codegen::apply_css_vars(doc, n, &mut out);
    out.join("\n")
}

fn box_shorthand(t: f32, r: f32, b: f32, l: f32) -> String {
    let f = |v: f32| format!("{}px", num(v));
    if t == r && r == b && b == l {
        f(t)
    } else if t == b && r == l {
        format!("{} {}", f(t), f(r))
    } else if r == l {
        format!("{} {} {}", f(t), f(r), f(b))
    } else {
        format!("{} {} {} {}", f(t), f(r), f(b), f(l))
    }
}

fn corners_shorthand(r: [f32; 4]) -> String {
    box_shorthand(r[0], r[1], r[2], r[3])
}

fn stop_list(stops: &[GradientStop], opacity: f32, pos: impl Fn(f32) -> String) -> String {
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    s.iter().map(|g| format!("{} {}", css_color(&g.color, opacity), pos(g.pos))).collect::<Vec<_>>().join(", ")
}

/// A fill as a CSS `<background>` layer.
pub fn paint_css(p: &Paint, size: Vec2) -> String {
    match &p.kind {
        PaintKind::Solid(c) => css_color(c, p.opacity),
        PaintKind::LinearGradient { handles, stops } => linear_css(handles, stops, size, p.opacity),
        PaintKind::RadialGradient { handles, stops } | PaintKind::DiamondGradient { handles, stops } => {
            let [h0, h1, h2] = *handles;
            let (w, h) = (size.x.max(1e-3), size.y.max(1e-3));
            let rx = ((h1.x - h0.x) * w).hypot((h1.y - h0.y) * h) / w * 100.0;
            let ry = ((h2.x - h0.x) * w).hypot((h2.y - h0.y) * h) / h * 100.0;
            format!(
                "radial-gradient({}% {}% at {}% {}%, {})",
                num2(rx),
                num2(ry),
                num2(h0.x * 100.0),
                num2(h0.y * 100.0),
                stop_list(stops, p.opacity, |t| format!("{}%", num2(t * 100.0)))
            )
        }
        PaintKind::AngularGradient { handles, stops } => {
            let [h0, h1, _] = *handles;
            let (dx, dy) = ((h1.x - h0.x) * size.x, (h1.y - h0.y) * size.y);
            let from = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
            format!(
                "conic-gradient(from {}deg at {}% {}%, {})",
                num2(from),
                num2(h0.x * 100.0),
                num2(h0.y * 100.0),
                stop_list(stops, p.opacity, |t| format!("{}deg", num2(t * 360.0)))
            )
        }
        PaintKind::Image { image_ref, scale_mode } => {
            let fit = match scale_mode {
                ImageScaleMode::Fill | ImageScaleMode::Crop => "50% / cover no-repeat",
                ImageScaleMode::Fit => "50% / contain no-repeat",
                ImageScaleMode::Tile => "0 0 / auto repeat",
            };
            format!("url(<{image_ref}>) lightgray {fit}")
        }
    }
}

fn linear_css(handles: &[Vec2; 3], stops: &[GradientStop], size: Vec2, opacity: f32) -> String {
    let (w, h) = (size.x, size.y);
    let p0 = (handles[0].x * w, handles[0].y * h);
    let p1 = (handles[1].x * w, handles[1].y * h);
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = dx.hypot(dy);
    if len < 1e-4 {
        return stops.first().map(|s| css_color(&s.color, opacity)).unwrap_or_else(|| "transparent".into());
    }
    let ang = dx.atan2(-dy);
    let (sx, sy) = (ang.sin(), -ang.cos());
    let line_len = (w * ang.sin()).abs() + (h * ang.cos()).abs();
    let deg = ang.to_degrees().rem_euclid(360.0);
    // Figma places stops along its own start->end segment; CSS measures them
    // along the gradient line through the box centre.
    let pos = |t: f32| {
        let (px, py) = (p0.0 + dx * t, p0.1 + dy * t);
        let along = (px - w / 2.0) * sx + (py - h / 2.0) * sy;
        let v = if line_len > 1e-4 { along / line_len + 0.5 } else { t };
        format!("{}%", num2(v * 100.0))
    };
    format!("linear-gradient({}deg, {})", num2(deg), stop_list(stops, opacity, pos))
}

fn border(n: &Node, out: &mut Vec<String>) {
    let st = &n.stroke;
    let Some(p) = st.paints.iter().find(|p| p.visible && p.opacity > 0.0) else { return };
    if st.weight <= 0.0 && st.side_weights.is_none() {
        return;
    }
    let mut push = |p: &str, v: String| out.push(format!("{p}: {v};"));
    let style = if st.dashes.iter().any(|d| *d > 0.0) { "dashed" } else { "solid" };
    if matches!(n.kind, NodeKind::Text(_)) {
        push("-webkit-text-stroke-width", format!("{}px", num(st.weight)));
        push("-webkit-text-stroke-color", solid_or_first(p));
        return;
    }
    let gradient = !matches!(p.kind, PaintKind::Solid(_));
    let color = if gradient { "transparent".to_string() } else { solid_or_first(p) };
    if matches!(n.kind, NodeKind::Line) {
        push("border-top", format!("{}px {style} {}", num(st.weight), solid_or_first(p)));
        return;
    }
    match (st.align, st.side_weights) {
        (StrokeAlign::Inside, Some([t, r, b, l])) => {
            push("border-width", box_shorthand(t, r, b, l));
            push("border-style", style.into());
            push("border-color", color);
        }
        (StrokeAlign::Inside, None) => push("border", format!("{}px {style} {color}", num(st.weight))),
        (align, _) => {
            // The box keeps its size; outline draws outside it, shifted in for centered strokes.
            push("outline", format!("{}px {style} {color}", num(st.weight)));
            let off = if align == StrokeAlign::Center { -st.weight / 2.0 } else { 0.0 };
            if off != 0.0 {
                push("outline-offset", format!("{}px", num(off)));
            }
        }
    }
    if gradient && st.align == StrokeAlign::Inside {
        push("border-image-source", paint_css(p, n.size));
        push("border-image-slice", "1".into());
    }
}

fn solid_or_first(p: &Paint) -> String {
    match &p.kind {
        PaintKind::Solid(c) => css_color(c, p.opacity),
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => stops.first().map(|s| css_color(&s.color, p.opacity)).unwrap_or_else(|| "transparent".into()),
        PaintKind::Image { .. } => "transparent".into(),
    }
}

/// CSS font-family value (quoted when it needs to be).
pub fn font_family_css(f: &str) -> String {
    if f.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        f.to_string()
    } else {
        format!("\"{f}\"")
    }
}

/// Text-only declarations for one text style.
pub fn text_style_css(st: &TextStyle) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |p: &str, v: String| out.push(format!("{p}: {v};"));
    push("font-family", font_family_css(&st.font_family));
    push("font-size", format!("{}px", num(st.font_size)));
    push("font-style", if st.italic { "italic" } else { "normal" }.into());
    push("font-weight", st.font_weight.to_string());
    push(
        "line-height",
        match st.line_height {
            LineHeight::Auto => "normal".into(),
            LineHeight::Pixels(p) => format!("{}px", num(p)),
            LineHeight::Percent(p) => format!("{}%", num2(p)),
        },
    );
    if st.letter_spacing != 0.0 {
        push("letter-spacing", format!("{}px", num(st.letter_spacing)));
    }
    match st.align_h {
        TextAlignH::Left => {}
        TextAlignH::Center => push("text-align", "center".into()),
        TextAlignH::Right => push("text-align", "right".into()),
        TextAlignH::Justified => push("text-align", "justify".into()),
    }
    match st.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => push("text-decoration-line", "underline".into()),
        TextDecoration::Strikethrough => push("text-decoration-line", "line-through".into()),
    }
    match st.case {
        TextCase::Original => {}
        TextCase::Upper => push("text-transform", "uppercase".into()),
        TextCase::Lower => push("text-transform", "lowercase".into()),
        TextCase::Title => push("text-transform", "capitalize".into()),
        TextCase::SmallCaps => push("font-variant", "small-caps".into()),
    }
    if !st.features.is_empty() {
        let list: Vec<String> = st.features.iter().map(|(t, v)| format!("\"{t}\" {v}")).collect();
        push("font-feature-settings", list.join(", "));
    }
    if !st.variation_axes.is_empty() {
        let list: Vec<String> = st.variation_axes.iter().map(|(t, v)| format!("\"{t}\" {}", num2(*v))).collect();
        push("font-variation-settings", list.join(", "));
    }
    out
}

/// Alignments that spread the free space, so the gap is not a fixed `item_spacing`.
fn distributes_space(a: LayoutAlign) -> bool {
    matches!(a, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly)
}

fn text_block(t: &TextProps, out: &mut Vec<String>) {
    let st = &t.style;
    if let Some(p) = st.fills.iter().find(|p| p.visible && p.opacity > 0.0) {
        match &p.kind {
            PaintKind::Solid(c) => out.push(format!("color: {};", css_color(c, p.opacity))),
            _ => {
                out.push(format!("background: {};", paint_css(p, Vec2::new(1.0, 1.0))));
                out.push("background-clip: text;".into());
                out.push("-webkit-background-clip: text;".into());
                out.push("-webkit-text-fill-color: transparent;".into());
            }
        }
    }
    out.extend(text_style_css(st));
    match t.direction {
        TextDirection::Auto => {}
        TextDirection::Ltr => out.push("direction: ltr;".into()),
        TextDirection::Rtl => out.push("direction: rtl;".into()),
    }
    match t.text_wrap {
        TextWrap::Auto => {}
        TextWrap::Balance => out.push("text-wrap: balance;".into()),
        TextWrap::Pretty => out.push("text-wrap: pretty;".into()),
    }
    if t.paragraph.indent > 0.0 {
        out.push(format!("text-indent: {}px;", num(t.paragraph.indent)));
    }
    if t.paragraph.hanging_punctuation {
        out.push("hanging-punctuation: first last;".into());
    }
    if t.paragraph.leading_trim == LeadingTrim::CapHeight {
        out.push("text-box: trim-both cap alphabetic;".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(f: impl FnOnce(&mut Document, NodeId)) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        f(&mut d, page);
        let id = *d.get(page).unwrap().children.last().unwrap();
        (d, id)
    }

    #[test]
    fn golden_card_css() {
        let (d, id) = doc_with(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(10.0, 20.0, 200.0, 100.0));
            n.fills = vec![Paint::solid(Color::WHITE)];
            n.corner_radii = [8.0; 4];
            n.opacity = 0.5;
            n.stroke.paints = vec![Paint::solid(Color::rgb(0xE5, 0xE5, 0xE5))];
            n.stroke.weight = 1.0;
            n.effects = vec![
                Effect::DropShadow {
                    color: Color::rgba(0, 0, 0, 64),
                    offset: Vec2::new(0.0, 4.0),
                    radius: 4.0,
                    spread: 0.0,
                    visible: true,
                    show_behind: false,
                },
                Effect::LayerBlur { radius: 10.0, visible: true },
            ];
            d.insert(page, n).unwrap();
        });
        assert_eq!(
            css_for(&d, id),
            "width: 200px;\nheight: 100px;\nbackground: #FFF;\nborder: 1px solid #E5E5E5;\nborder-radius: 8px;\nbox-shadow: 0px 4px 4px 0px rgba(0, 0, 0, 0.25);\nfilter: blur(5px);\nopacity: 0.5;"
        );
    }

    #[test]
    fn golden_auto_layout_and_children() {
        let (d, id) = doc_with(|d, page| {
            let f = d.alloc_id();
            let mut fr = Node::frame(f, Rect::new(0.0, 0.0, 300.0, 80.0));
            fr.fills.clear();
            if let NodeKind::Frame(fp) = &mut fr.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 12.0,
                    padding: [8.0, 16.0, 8.0, 16.0],
                    primary_align: LayoutAlign::SpaceBetween,
                    counter_align: LayoutAlign::Center,
                    ..Default::default()
                });
            }
            let fr = d.insert(page, fr).unwrap();
            let c = d.alloc_id();
            let mut child = Node::rectangle(c, Rect::new(0.0, 0.0, 40.0, 40.0));
            child.layout_child.horizontal = Sizing::Fill;
            d.insert(fr, child).unwrap();
        });
        assert_eq!(css_for(&d, id), "display: flex;\nwidth: 300px;\nheight: 80px;\npadding: 8px 16px;\njustify-content: space-between;\nalign-items: center;");
        let child = d.get(id).unwrap().children[0];
        assert_eq!(css_for(&d, child), "flex: 1 0 0;\nheight: 40px;\nbackground: #D9D9D9;");
    }

    #[test]
    fn auto_spacing_modes_and_wrap_content_map_to_css() {
        for (align, want) in [(LayoutAlign::SpaceAround, "space-around"), (LayoutAlign::SpaceEvenly, "space-evenly")] {
            let (d, id) = doc_with(|d, page| {
                let f = d.alloc_id();
                let mut fr = Node::frame(f, Rect::new(0.0, 0.0, 300.0, 80.0));
                fr.fills.clear();
                if let NodeKind::Frame(fp) = &mut fr.kind {
                    fp.auto_layout = Some(AutoLayout {
                        direction: LayoutDirection::Vertical,
                        wrap: true,
                        item_spacing: 12.0,
                        primary_align: align,
                        counter_align_content: CounterAlignContent::SpaceBetween,
                        ..Default::default()
                    });
                }
                d.insert(page, fr).unwrap();
            });
            let css = css_for(&d, id);
            assert!(css.contains(&format!("justify-content: {want};")), "{css}");
            assert!(css.contains("flex-wrap: wrap;") && css.contains("align-content: space-between;"), "{css}");
            assert!(!css.contains("gap:"), "the fixed gap does not apply with auto spacing: {css}");
        }
    }

    #[test]
    fn text_wrap_maps_to_css() {
        let (d, id) = doc_with(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::text(i, Rect::new(0.0, 0.0, 120.0, 21.0), "Hi", TextStyle::default());
            if let NodeKind::Text(t) = &mut n.kind {
                t.text_wrap = TextWrap::Balance;
            }
            d.insert(page, n).unwrap();
        });
        assert!(css_for(&d, id).contains("text-wrap: balance;"));
    }

    #[test]
    fn absolute_child_and_gradient() {
        let (d, id) = doc_with(|d, page| {
            let f = d.alloc_id();
            let fr = d.insert(page, Node::frame(f, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
            let c = d.alloc_id();
            let mut n = Node::rectangle(c, Rect::new(10.0, 12.0, 50.0, 50.0));
            n.fills = vec![Paint {
                kind: PaintKind::LinearGradient {
                    handles: [Vec2::new(0.5, 0.0), Vec2::new(0.5, 1.0), Vec2::new(1.0, 0.0)],
                    stops: vec![GradientStop { pos: 0.0, color: Color::rgb(255, 0, 0) }, GradientStop { pos: 1.0, color: Color::rgb(0, 0, 255) }],
                },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            d.insert(fr, n).unwrap();
        });
        let child = d.get(id).unwrap().children[0];
        assert_eq!(
            css_for(&d, child),
            "position: absolute;\nleft: 10px;\ntop: 12px;\nwidth: 50px;\nheight: 50px;\nbackground: linear-gradient(180deg, #F00 0%, #00F 100%);"
        );
    }

    #[test]
    fn text_block_css() {
        let (d, id) = doc_with(|d, page| {
            let i = d.alloc_id();
            let st = TextStyle {
                font_family: "Roboto Mono".into(),
                font_size: 14.0,
                font_weight: 500,
                line_height: LineHeight::Percent(150.0),
                letter_spacing: -0.28,
                align_h: TextAlignH::Center,
                fills: vec![Paint::solid(Color::rgb(0x11, 0x22, 0x33))],
                ..Default::default()
            };
            d.insert(page, Node::text(i, Rect::new(0.0, 0.0, 120.0, 21.0), "Hi", st)).unwrap();
        });
        assert_eq!(
            css_for(&d, id),
            "color: #123;\nfont-family: \"Roboto Mono\";\nfont-size: 14px;\nfont-style: normal;\nfont-weight: 500;\nline-height: 150%;\nletter-spacing: -0.28px;\ntext-align: center;"
        );
    }

    #[test]
    fn text_features_direction_and_paragraph_settings_css() {
        let (d, id) = doc_with(|d, page| {
            let i = d.alloc_id();
            let st = TextStyle { features: vec![("ss01".into(), 1), ("tnum".into(), 1)], fills: vec![], ..Default::default() };
            let mut n = Node::text(i, Rect::new(0.0, 0.0, 120.0, 21.0), "Hi", st);
            if let NodeKind::Text(t) = &mut n.kind {
                t.direction = TextDirection::Rtl;
                t.paragraph.indent = 12.0;
                t.paragraph.hanging_punctuation = true;
                t.paragraph.leading_trim = LeadingTrim::CapHeight;
            }
            d.insert(page, n).unwrap();
        });
        let css = css_for(&d, id);
        for want in [
            "font-feature-settings: \"ss01\" 1, \"tnum\" 1;",
            "direction: rtl;",
            "text-indent: 12px;",
            "hanging-punctuation: first last;",
            "text-box: trim-both cap alphabetic;",
        ] {
            assert!(css.contains(want), "{want} missing in:\n{css}");
        }
    }

    #[test]
    fn ellipse_and_radial() {
        let (d, id) = doc_with(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::ellipse(i, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.fills = vec![Paint {
                kind: PaintKind::RadialGradient {
                    handles: [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)],
                    stops: vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: Color::BLACK }],
                },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            d.insert(page, n).unwrap();
        });
        let css = css_for(&d, id);
        assert!(css.contains("background: radial-gradient(50% 50% at 50% 50%, #FFF 0%, #000 100%);"), "{css}");
        assert!(css.contains("border-radius: 50%;"), "{css}");
    }
}
