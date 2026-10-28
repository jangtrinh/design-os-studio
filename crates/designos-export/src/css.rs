//! Figma Dev Mode style CSS for a node.

use crate::codegen::{web_sizes, Dim, Parent};
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
    let parent_al = parent.and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref());
    let in_flow = parent_al.is_some() && !n.layout_child.absolute;
    let is_text = matches!(n.kind, NodeKind::Text(_));
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());

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
    // Width and height follow the node's sizing: Fixed prints pixels, Fill is left to the parent, Hug
    // (self-sizing text, a flex frame hugging its children) is left to the content. A grid frame is
    // never Hug here: it prints the size the solver gave it.
    let sz = web_sizes(n, Parent { al: parent_al, root: false });
    let row_parent = sz.row_parent;
    let fill_w = in_flow && sz.w == Dim::Fill;
    let fill_h = in_flow && sz.h == Dim::Fill;
    let grows = in_flow && n.layout_child.grow > 0.0;
    let mut flex_done = false;
    let mut stretch = false;
    if in_flow && grid_parent {
        if let Some(al) = parent_al {
            let placement = crate::codegen::normalise_grid_placement(&n.layout_child, al);
            if let Some(col) = crate::codegen::grid_placement_css(placement.column, placement.col_span) {
                push("grid-column", col);
            }
            if let Some(row) = crate::codegen::grid_placement_css(placement.row, placement.row_span) {
                push("grid-row", row);
            }
        }
        // An item sized by its own content keeps that size instead of stretching across its cell.
        if sz.w == Dim::Hug {
            push("justify-self", "start".into());
        }
        if n.layout_child.align_self == AlignSelf::Stretch {
            push("align-self", "stretch".into());
        } else if sz.h == Dim::Hug {
            push("align-self", "start".into());
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
    if sz.cross_stretch {
        stretch = true;
    }
    let width_skipped = !matches!(sz.w, Dim::Fixed(_));
    let height_skipped = !matches!(sz.h, Dim::Fixed(_));
    if !width_skipped {
        push("width", format!("{}px", num(n.size.x)));
    }
    if !height_skipped || matches!(n.kind, NodeKind::Line) {
        push("height", format!("{}px", num(if matches!(n.kind, NodeKind::Line) { 0.0 } else { n.size.y })));
    }
    if in_flow && !grid_parent {
        let align_self_css = match n.layout_child.align_self {
            AlignSelf::Start => Some("flex-start"),
            AlignSelf::Center => Some("center"),
            AlignSelf::End => Some("flex-end"),
            AlignSelf::Baseline => Some("baseline"),
            AlignSelf::Stretch | AlignSelf::Auto if stretch => Some("stretch"),
            AlignSelf::Stretch | AlignSelf::Auto => None,
        };
        if let Some(a) = align_self_css {
            push("align-self", a.into());
        }
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
            let cols = crate::codegen::grid_column_count(al);
            push("grid-template-columns", crate::codegen::grid_tracks_css(&al.grid_col_tracks, cols));
            if al.grid_rows > 0 {
                push("grid-template-rows", crate::codegen::grid_tracks_css(&al.grid_row_tracks, al.grid_rows as usize));
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
            Effect::InnerShadow { color, offset, radius, spread, visible: true, .. } => {
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
                    blend: Default::default(),
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

    #[test]
    fn grid_tracks_placement_and_gaps_css() {
        let mut pinned_id = NodeId(0);
        let mut span_only_id = NodeId(0);
        let (d, grid_id) = doc_with(|d, page| {
            let gid = d.alloc_id();
            let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 480.0, 200.0));
            g.fills.clear();
            let mut al = AutoLayout {
                direction: LayoutDirection::Grid,
                item_spacing: 16.0,
                counter_spacing: 24.0,
                padding: [12.0, 16.0, 12.0, 16.0],
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                grid_columns: 3,
                grid_rows: 2,
                ..Default::default()
            };
            al.grid_col_tracks[0] = GridTrack::Flex(1.0);
            al.grid_col_tracks[1] = GridTrack::Fixed(120.0);
            al.grid_col_tracks[2] = GridTrack::Hug;
            al.grid_row_tracks[0] = GridTrack::Fixed(80.0);
            al.grid_row_tracks[1] = GridTrack::Hug;
            g.kind = NodeKind::Frame(FrameProps { clips_content: true, auto_layout: Some(al), ..Default::default() });
            let g = d.insert(page, g).unwrap();

            let c1 = d.alloc_id();
            let mut n1 = Node::rectangle(c1, Rect::new(0.0, 0.0, 200.0, 80.0));
            n1.fills.clear();
            n1.layout_child.grid_column = Some(1);
            n1.layout_child.grid_row = Some(0);
            n1.layout_child.col_span = 2;
            n1.layout_child.row_span = 1;
            pinned_id = d.insert(g, n1).unwrap();

            let c2 = d.alloc_id();
            let mut n2 = Node::rectangle(c2, Rect::new(0.0, 0.0, 180.0, 60.0));
            n2.fills.clear();
            n2.layout_child.col_span = 2;
            n2.layout_child.row_span = 1;
            span_only_id = d.insert(g, n2).unwrap();
        });

        assert_eq!(
            css_for(&d, grid_id),
            "display: grid;\nwidth: 480px;\nheight: 200px;\npadding: 12px 16px;\ngrid-template-columns: minmax(0, 1fr) 120px auto;\ngrid-template-rows: 80px auto;\ngap: 24px 16px;\noverflow: hidden;"
        );
        assert_eq!(css_for(&d, pinned_id), "grid-column: 2 / span 2;\ngrid-row: 1 / span 1;\nwidth: 200px;\nheight: 80px;");
        assert_eq!(css_for(&d, span_only_id), "grid-column: span 2;\nwidth: 180px;\nheight: 60px;");
    }

    #[test]
    fn grid_frame_and_children_sizing_in_css_and_codegen() {
        let (d, grid_id) = doc_with(|d, page| {
            let gid = d.alloc_id();
            let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 200.0, 300.0));
            g.fills.clear();
            if let NodeKind::Frame(fp) = &mut g.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Grid,
                    grid_columns: 3,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Fixed,
                    ..Default::default()
                });
            }
            let gid = d.insert(page, g).unwrap();

            // 1. Fixed child spanning 2 columns and 2 rows keeps its explicit width and height
            let c1 = d.alloc_id();
            let mut fixed_span = Node::rectangle(c1, Rect::new(0.0, 0.0, 80.0, 40.0));
            fixed_span.layout_child.col_span = 2;
            fixed_span.layout_child.row_span = 2;
            d.insert(gid, fixed_span).unwrap();

            // 2. Fill child skips width so the grid stretches it horizontally and exports solved height
            let c2 = d.alloc_id();
            let mut fill_child = Node::rectangle(c2, Rect::new(0.0, 0.0, 80.0, 40.0));
            fill_child.layout_child.horizontal = Sizing::Fill;
            fill_child.layout_child.vertical = Sizing::Fill;
            d.insert(gid, fill_child).unwrap();

            // 3. Hug child omits width and height and emits justify-self: start; align-self: start;
            let c3 = d.alloc_id();
            let mut hug_child = Node::frame(c3, Rect::new(0.0, 0.0, 60.0, 24.0));
            hug_child.fills.clear();
            if let NodeKind::Frame(fp) = &mut hug_child.kind {
                fp.auto_layout =
                    Some(AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() });
            }
            d.insert(gid, hug_child).unwrap();
        });

        let grid_css = css_for(&d, grid_id);
        assert!(grid_css.contains("width: 200px;"), "a hugging grid exports the size it holds, its tracks are fractions of it: {grid_css}");
        assert!(grid_css.contains("height: 300px;"), "grid with fixed height must emit height: {grid_css}");
        assert!(grid_css.contains("grid-template-columns: repeat(3, minmax(0, 1fr));"), "{grid_css}");
        let tw = crate::tailwind_for(&d, grid_id);
        assert!(tw.contains("h-[300px]") && tw.contains("w-[200px]") && !tw.contains("w-max"), "codegen sizes must treat grid primary as width: {tw}");

        let children = &d.get(grid_id).unwrap().children;
        let fixed_css = css_for(&d, children[0]);
        assert!(fixed_css.contains("grid-column: span 2;") && fixed_css.contains("grid-row: span 2;"), "{fixed_css}");
        assert!(fixed_css.contains("width: 80px;") && fixed_css.contains("height: 40px;"), "spanning Fixed child in grid must keep its size: {fixed_css}");

        let fill_css = css_for(&d, children[1]);
        assert!(!fill_css.contains("width:") && fill_css.contains("height: 40px;"), "Fill child in grid skips width and exports solved height: {fill_css}");

        let hug_css = css_for(&d, children[2]);
        assert!(!hug_css.contains("width:") && !hug_css.contains("height:"), "Hug child in grid must skip width/height: {hug_css}");
        assert!(
            hug_css.contains("justify-self: start;") && hug_css.contains("align-self: start;"),
            "Hug child in grid must emit justify-self: start and align-self: start: {hug_css}"
        );
    }

    /// The grid's exported width and height must be the frame the solver computed, and its tracks
    /// stay fractions of that size.
    fn solved_grid_css(d: &Document, grid: NodeId, cols: u32, rows: u32) -> String {
        let uniform = |n: u32| format!("repeat({n}, minmax(0, 1fr))");
        solved_grid_template(d, grid, &uniform(cols), (rows > 0).then(|| uniform(rows)).as_deref())
    }

    /// [`solved_grid_css`] for a grid whose column and row templates are the given CSS.
    fn solved_grid_template(d: &Document, grid: NodeId, columns: &str, rows: Option<&str>) -> String {
        let size = d.get(grid).unwrap().size;
        let css = css_for(d, grid);
        assert!(css.contains(&format!("width: {}px;", num(size.x))), "the exported width must be the solver's {}: {css}", size.x);
        assert!(css.contains(&format!("height: {}px;", num(size.y))), "the exported height must be the solver's {}: {css}", size.y);
        assert!(css.contains(&format!("grid-template-columns: {columns};")), "{css}");
        if let Some(rows) = rows {
            assert!(css.contains(&format!("grid-template-rows: {rows};")), "{css}");
        }
        assert!(
            !css.contains("max-content") && !css.contains("minmax(40px") && !css.contains(", auto)"),
            "tracks must not be re-derived by the CSS track algorithm: {css}"
        );
        css
    }

    #[test]
    fn hugging_grid_exports_the_solved_width_for_a_small_child() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 3, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(1000.0, 100.0));
        fixture::add_rect(&mut d, grid, Vec2::new(10.0, 10.0));
        d.relayout_subtree(grid);
        assert_eq!(d.get(grid).unwrap().size, Vec2::new(120.0, 100.0), "the solver sizes three default columns at their 40px minimum");
        solved_grid_css(&d, grid, 3, 0);
    }

    #[test]
    fn hugging_grid_exports_the_solved_width_when_the_widest_child_fills() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 1, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(500.0, 100.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(100.0, 20.0));
        d.get_mut(rect).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(grid);
        assert_eq!(d.get(grid).unwrap().size.x, 100.0, "a Fill child still counts at its own size when the column is sized");
        solved_grid_css(&d, grid, 1, 0);
    }

    #[test]
    fn hugging_grid_exports_the_solved_width_when_a_child_spans_columns() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(500.0, 100.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(200.0, 20.0));
        d.get_mut(rect).unwrap().layout_child.col_span = 2;
        d.relayout_subtree(grid);
        assert_eq!(d.get(grid).unwrap().size.x, 80.0, "a spanning child does not widen the hugging columns");
        solved_grid_css(&d, grid, 2, 0);
    }

    #[test]
    fn hugging_grid_exports_the_solved_width_with_a_fixed_track() {
        use crate::codegen::fixture;
        let mut al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() };
        al.grid_col_tracks[0] = GridTrack::Fixed(200.0);
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(500.0, 100.0));
        fixture::add_rect(&mut d, grid, Vec2::new(10.0, 10.0));
        fixture::add_rect(&mut d, grid, Vec2::new(60.0, 10.0));
        d.relayout_subtree(grid);
        assert_eq!(d.get(grid).unwrap().size.x, 260.0, "a fixed 200px column plus a 60px hugging one");
        solved_grid_template(&d, grid, "200px minmax(0, 1fr)", None);
    }

    #[test]
    fn hugging_grid_exports_the_solved_height_with_template_rows() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, grid_rows: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Hug, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(300.0, 500.0));
        for h in [20.0, 20.0, 50.0, 50.0] {
            fixture::add_rect(&mut d, grid, Vec2::new(30.0, h));
        }
        d.relayout_subtree(grid);
        assert_eq!(d.get(grid).unwrap().size, Vec2::new(300.0, 70.0), "rows take their tallest child");
        solved_grid_css(&d, grid, 2, 2);
    }

    #[test]
    fn grid_item_with_grow_stretches_across_its_cell_and_keeps_its_height() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(400.0, 300.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(rect).unwrap().layout_child.grow = 1.0;
        d.relayout_subtree(grid);
        assert_eq!(d.get(rect).unwrap().size, Vec2::new(200.0, 40.0), "the solver stretches a grow item across its column and leaves its height");
        let css = css_for(&d, rect);
        assert!(!css.contains("width:"), "the grid stretches the item across its cell: {css}");
        assert!(css.contains("height: 40px;"), "{css}");
    }

    #[test]
    fn fixed_grid_keeps_fractional_tracks_and_pixel_size() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, grid_rows: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (d, grid) = fixture::grid_frame(al, Vec2::new(300.0, 200.0));
        let css = css_for(&d, grid);
        assert_eq!(
            css,
            "display: grid;\nwidth: 300px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: repeat(2, minmax(0, 1fr));"
        );
    }

    #[test]
    fn grid_item_with_hug_sizing_mode_keeps_its_pixel_size() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(400.0, 200.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(rect).unwrap().layout_child.horizontal = Sizing::Hug;
        d.get_mut(rect).unwrap().layout_child.vertical = Sizing::Hug;
        d.relayout_subtree(grid);
        assert_eq!(d.get(rect).unwrap().size, Vec2::new(80.0, 40.0), "the solver keeps a non-Fill grid child at its own size");
        let css = css_for(&d, rect);
        assert!(css.contains("width: 80px;") && css.contains("height: 40px;"), "a rectangle has no intrinsic size to fall back on: {css}");
        assert!(!css.contains("justify-self") && !css.contains("align-self"), "only content-sized items align to the start: {css}");
    }

    #[test]
    fn flex_child_align_self_exports_css_align_self_for_all_variants() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        row.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                counter_align: LayoutAlign::Start,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            ..Default::default()
        });
        let fid = d.insert(page, row).unwrap();

        for (align, expected) in [
            (AlignSelf::Start, Some("align-self: flex-start;")),
            (AlignSelf::Center, Some("align-self: center;")),
            (AlignSelf::End, Some("align-self: flex-end;")),
            (AlignSelf::Stretch, Some("align-self: stretch;")),
            (AlignSelf::Baseline, Some("align-self: baseline;")),
            (AlignSelf::Auto, None),
        ] {
            let cid = d.alloc_id();
            let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 30.0, 20.0));
            child.layout_child.align_self = align;
            let cid = d.insert(fid, child).unwrap();
            d.relayout_subtree(fid);
            let css = css_for(&d, cid);
            match expected {
                Some(decl) => assert!(css.contains(decl), "{align:?} missing {decl:?} in:\n{css}"),
                None => assert!(!css.contains("align-self:"), "Auto should not emit align-self in:\n{css}"),
            }
            if align == AlignSelf::Stretch {
                assert!(!css.contains("height:"), "Stretch child should omit fixed height in:\n{css}");
            } else {
                assert!(css.contains("height: 20px;"), "{align:?} child should keep height: 20px in:\n{css}");
            }
        }
    }

    #[test]
    fn stretch_child_keeps_its_cross_size_when_the_parent_hugs() {
        use crate::codegen::fixture::row_with_stretch_children;
        let (d, row, kids) = row_with_stretch_children(Sizing::Hug, &[40.0]);
        assert_eq!(d.get(row).unwrap().size.y, 40.0, "the solver hugs the row around the stretch child");
        let css = css_for(&d, kids[0]);
        assert!(css.contains("height: 40px;") && css.contains("align-self: stretch;"), "{css}");

        let (d, row, kids) = row_with_stretch_children(Sizing::Hug, &[20.0, 100.0]);
        assert_eq!(d.get(row).unwrap().size.y, 100.0);
        for k in kids {
            let css = css_for(&d, k);
            assert!(css.contains("height: 100px;") && css.contains("align-self: stretch;"), "all-stretch row must not collapse: {css}");
        }
    }

    #[test]
    fn css_hugging_row_all_fill_children_print_the_solved_height() {
        use crate::codegen::fixture::row_with_counter_children;
        let (d, row, kids) = row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, false);
        assert_eq!(d.get(row).unwrap().size.y, 100.0);
        for k in &kids {
            let css = css_for(&d, *k);
            assert!(css.contains("height: 100px;") && css.contains("align-self: stretch;"), "all-fill row must not collapse: {css}");
        }
    }

    #[test]
    fn imported_fill_plus_stretch_same_output() {
        use crate::codegen::fixture::row_with_counter_children;
        let (d_fill, _, kids_fill) = row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, false);
        let (d_both, _, kids_both) = row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, true);
        for (kf, kb) in kids_fill.into_iter().zip(kids_both) {
            assert_eq!(css_for(&d_fill, kf), css_for(&d_both, kb));
        }
    }

    #[test]
    fn fixed_parent_stays_fill_only() {
        use crate::codegen::fixture::row_with_counter_children;
        let (d, _, kids) = row_with_counter_children(Sizing::Fixed, &[40.0], true, false);
        let css = css_for(&d, kids[0]);
        assert!(css.contains("align-self: stretch;") && !css.contains("height:"), "{css}");
    }

    #[test]
    fn plain_sibling_sets_the_line() {
        use crate::codegen::fixture::add_rect;
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        row.fills.clear();
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                counter_align: LayoutAlign::Start,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            });
        }
        let fid = d.insert(page, row).unwrap();
        let _plain = add_rect(&mut d, fid, Vec2::new(30.0, 60.0));
        let fill_kid = add_rect(&mut d, fid, Vec2::new(30.0, 20.0));
        d.get_mut(fill_kid).unwrap().layout_child.vertical = Sizing::Fill;
        d.relayout_subtree(fid);
        let css = css_for(&d, fill_kid);
        assert!(css.contains("height: 60px;") && css.contains("align-self: stretch;"), "{css}");
    }

    #[test]
    fn vertical_stack_symmetric() {
        use crate::codegen::fixture::add_rect;
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut col = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0));
        col.fills.clear();
        if let NodeKind::Frame(fp) = &mut col.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                counter_align: LayoutAlign::Start,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            });
        }
        let fid = d.insert(page, col).unwrap();
        let k1 = add_rect(&mut d, fid, Vec2::new(20.0, 30.0));
        let k2 = add_rect(&mut d, fid, Vec2::new(100.0, 30.0));
        d.get_mut(k1).unwrap().layout_child.horizontal = Sizing::Fill;
        d.get_mut(k2).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(fid);
        for k in [k1, k2] {
            let css = css_for(&d, k);
            assert!(css.contains("width: 100px;") && css.contains("align-self: stretch;"), "{css}");
        }
    }

    #[test]
    fn stretch_child_leaves_the_cross_size_to_a_fixed_parent() {
        use crate::codegen::fixture::row_with_stretch_children;
        let (d, _, kids) = row_with_stretch_children(Sizing::Fixed, &[40.0]);
        let css = css_for(&d, kids[0]);
        assert!(css.contains("align-self: stretch;") && !css.contains("height:"), "{css}");
    }

    #[test]
    fn grid_stretch_child_exports_its_solved_size() {
        use crate::codegen::fixture;
        // (row track, solved height): an auto row is sized by the child, a taller row is filled.
        for (track, height) in [(None, "20px"), (Some(GridTrack::Fixed(100.0)), "100px")] {
            let (d, _, rect) = fixture::grid_stretch_child(track, false);
            let css = css_for(&d, rect);
            assert!(css.contains("width: 30px;") && css.contains(&format!("height: {height};")), "{track:?}: {css}");
            assert!(css.contains("align-self: stretch;"), "{track:?}: {css}");
            assert!(!css.contains("flex"), "{track:?}: {css}");
        }
    }

    #[test]
    fn css_grid_fill_child_exports_solved_height() {
        use crate::codegen::fixture;
        for (track, height) in [(None, "20px"), (Some(GridTrack::Fixed(100.0)), "100px")] {
            for (d, _, kid) in [fixture::grid_fill_child(track), fixture::grid_text_fill_child(track)] {
                let css = css_for(&d, kid);
                assert!(css.contains("width: 30px;") && css.contains(&format!("height: {height};")), "{track:?}: {css}");
                assert!(!css.contains("align-self:"), "{track:?}: {css}");
                assert!(!css.contains("flex"), "{track:?}: {css}");
            }
        }
        // A real Hug text child (vertical != Fill, align_self != Stretch) stays on the Hug path.
        let (mut d, grid, tid) = fixture::grid_text_fill_child(Some(GridTrack::Fixed(100.0)));
        d.get_mut(tid).unwrap().layout_child.vertical = Sizing::Fixed;
        d.relayout_subtree(grid);
        let css = css_for(&d, tid);
        assert!(css.contains("width: 30px;") && !css.lines().any(|l| l.starts_with("height:")), "real Hug text omits height: {css}");
        assert!(css.contains("align-self: start;"), "real Hug text emits align-self: start: {css}");
    }

    #[test]
    fn css_hugging_row_fill_text_children_print_the_solved_height() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        row.fills.clear();
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                counter_align: LayoutAlign::Start,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            });
        }
        let fid = d.insert(page, row).unwrap();
        let mut kids = Vec::new();
        for h in [20.0, 100.0] {
            let tid = d.alloc_id();
            let mut txt = Node::text(tid, Rect::new(0.0, 0.0, 30.0, h), "Label", TextStyle::default());
            if let NodeKind::Text(t) = &mut txt.kind {
                t.auto_resize = TextAutoResize::Height;
            }
            txt.layout_child.vertical = Sizing::Fill;
            kids.push(d.insert(fid, txt).unwrap());
        }
        d.relayout_subtree(fid);
        for k in kids {
            let css = css_for(&d, k);
            assert!(css.contains("height: 100px;") && css.contains("align-self: stretch;"), "fill text in hugging row must keep solved height: {css}");
        }
    }

    #[test]
    fn css_grid_imported_fill_plus_stretch() {
        use crate::codegen::fixture;
        for (track, height) in [(None, "20px"), (Some(GridTrack::Fixed(100.0)), "100px")] {
            let (d, _, rect) = fixture::grid_stretch_child(track, true);
            let css = css_for(&d, rect);
            assert!(css.contains("width: 30px;") && css.contains(&format!("height: {height};")), "{track:?}: {css}");
            assert!(css.contains("align-self: stretch;"), "{track:?}: {css}");
            assert!(!css.contains("flex"), "{track:?}: {css}");
        }
    }

    #[test]
    fn grid_child_horizontal_fill_unchanged() {
        use crate::codegen::fixture;
        let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(400.0, 300.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(30.0, 20.0));
        d.get_mut(rect).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(grid);
        let css = css_for(&d, rect);
        assert!(!css.contains("width:"), "horizontal Fill in grid omits width: {css}");
        assert!(css.contains("height: 20px;"), "{css}");
    }
}
