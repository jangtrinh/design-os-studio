//! Tailwind CSS output: markup with utility classes.

use crate::codegen::*;
use crate::css::paint_css;
use crate::slots::*;
use crate::util::{css_color, num, num2};
use designos_doc::*;

/// HTML with Tailwind utility classes for `node` and its descendants. Values on the Tailwind
/// scale use tokens (`p-4`, `rounded-lg`), everything else exact arbitrary values (`w-[312px]`);
/// bound variables become CSS variable references (`bg-[var(--Color-Primary)]`). Instances
/// are written as `<Component prop="value" />`.
pub fn tailwind_for(doc: &Document, node: NodeId) -> String {
    let mut o = Out::new("  ");
    markup(doc, node, Parent::root(), 0, &mut o, false);
    o.finish()
}

/// Same markup for use inside JSX (`className`, escaped text).
pub(crate) fn jsx_markup(doc: &Document, node: NodeId, depth: usize, o: &mut Out) {
    markup(doc, node, Parent::root(), depth, o, true);
}

const SPACING: [(f32, &str); 35] = [
    (0.0, "0"),
    (1.0, "px"),
    (2.0, "0.5"),
    (4.0, "1"),
    (6.0, "1.5"),
    (8.0, "2"),
    (10.0, "2.5"),
    (12.0, "3"),
    (14.0, "3.5"),
    (16.0, "4"),
    (20.0, "5"),
    (24.0, "6"),
    (28.0, "7"),
    (32.0, "8"),
    (36.0, "9"),
    (40.0, "10"),
    (44.0, "11"),
    (48.0, "12"),
    (52.0, "13"),
    (56.0, "14"),
    (60.0, "15"),
    (64.0, "16"),
    (72.0, "18"),
    (80.0, "20"),
    (96.0, "24"),
    (112.0, "28"),
    (128.0, "32"),
    (144.0, "36"),
    (160.0, "40"),
    (176.0, "44"),
    (192.0, "48"),
    (208.0, "52"),
    (224.0, "56"),
    (240.0, "60"),
    (256.0, "64"),
];

fn space_token(px: f32) -> Option<&'static str> {
    SPACING.iter().find(|(v, _)| (*v - px).abs() < 1e-3).map(|(_, t)| *t)
}

/// `prefix-token` or `prefix-[Npx]` / `prefix-[var(--x)]`.
fn spaced(prefix: &str, px: f32, var: Option<&Variable>) -> String {
    if let Some(v) = var {
        return format!("{prefix}-[{}]", css_var(v));
    }
    match space_token(px) {
        Some(t) => format!("{prefix}-{t}"),
        None => format!("{prefix}-[{}px]", num(px)),
    }
}

// Largest value each stock grid utility ships with (Tailwind's default theme).
const STOCK_GRID_COLS: usize = 12;
const STOCK_GRID_ROWS: usize = 6;
const STOCK_COL_START: u32 = 13;
const STOCK_ROW_START: u32 = 7;
const STOCK_COL_SPAN: u16 = 12;
const STOCK_ROW_SPAN: u16 = 6;

/// `grid-cols-3` for `count` tracks that are all `1fr`, while the stock class exists. Anything else,
/// including a count past the stock scale, is an arbitrary value of the same CSS track list.
fn template_class(prefix: &str, tracks: &[GridTrack], count: usize, stock_max: usize) -> String {
    if grid_tracks_are_default(tracks, count) && count <= stock_max {
        return format!("{prefix}-{count}");
    }
    format!("{prefix}-[{}]", tailwind_tracks_value(&grid_tracks_css(tracks, count)))
}

/// `col-start-3` for grid line `line` while the stock class exists, else `col-start-[14]`.
fn line_class(prefix: &str, line: u32, stock_max: u32) -> String {
    if line <= stock_max {
        format!("{prefix}-{line}")
    } else {
        format!("{prefix}-[{line}]")
    }
}

/// `col-span-2` for a span while the stock class exists, else `col-[span_14_/_span_14]`.
fn span_class(prefix: &str, span: u16, stock_max: u16) -> String {
    if span <= stock_max {
        format!("{prefix}-span-{span}")
    } else {
        format!("{prefix}-[span_{span}_/_span_{span}]")
    }
}

fn hex8(c: &Color, opacity: f32) -> String {
    let a = (c.a * opacity).clamp(0.0, 1.0);
    let base = format!("#{}", c.to_hex());
    if (a - 1.0).abs() < 0.002 {
        base
    } else {
        format!("{base}{:02X}", (a * 255.0).round() as u8)
    }
}

fn color_class(doc: &Document, n: &Node, key: &str, prefix: &str, p: &Paint, text: bool) -> String {
    if let Some(v) = bound(doc, n, key) {
        return if text { format!("{prefix}-[color:{}]", css_var(v)) } else { format!("{prefix}-[{}]", css_var(v)) };
    }
    match &p.kind {
        PaintKind::Solid(c) => format!("{prefix}-[{}]", hex8(c, p.opacity)),
        _ => format!("{prefix}-[{}]", paint_css(p, Vec2::new(1.0, 1.0)).replace(", ", ",").replace(' ', "_")),
    }
}

fn radius_class(r: f32, size: Vec2) -> String {
    if r >= size.x.min(size.y) / 2.0 && r > 0.0 && size.x.min(size.y) > 0.0 && r >= 9999.0 {
        return "rounded-full".into();
    }
    match r as i32 {
        0 => "rounded-none".into(),
        2 => "rounded-sm".into(),
        4 => "rounded".into(),
        6 => "rounded-md".into(),
        8 => "rounded-lg".into(),
        12 => "rounded-xl".into(),
        16 => "rounded-2xl".into(),
        24 => "rounded-3xl".into(),
        _ => format!("rounded-[{}px]", num(r)),
    }
}

fn corner_classes(doc: &Document, n: &Node) -> Vec<String> {
    let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
    let r = n.corner_radii;
    let any_bound = keys.iter().any(|k| bound(doc, n, k).is_some());
    if !any_bound {
        if r.iter().all(|v| *v == 0.0) {
            return vec![];
        }
        if r.iter().all(|v| *v == r[0]) {
            return vec![radius_class(r[0], n.size)];
        }
    }
    if any_bound && keys.iter().all(|k| bound(doc, n, k).map(|v| v.id.as_str()) == bound(doc, n, keys[0]).map(|v| v.id.as_str())) {
        return vec![format!("rounded-[{}]", css_var(bound(doc, n, keys[0]).unwrap()))];
    }
    ["tl", "tr", "br", "bl"]
        .iter()
        .enumerate()
        .filter(|(i, _)| r[*i] > 0.0 || bound(doc, n, keys[*i]).is_some())
        .map(|(i, side)| match bound(doc, n, keys[i]) {
            Some(v) => format!("rounded-{side}-[{}]", css_var(v)),
            None => format!("rounded-{side}-[{}px]", num(r[i])),
        })
        .collect()
}

fn opacity_class(o: f32) -> Option<String> {
    if o >= 0.999 {
        return None;
    }
    let pct = (o * 100.0).round();
    if [0.0, 5.0, 10.0, 20.0, 25.0, 30.0, 40.0, 50.0, 60.0, 70.0, 75.0, 80.0, 90.0, 95.0].contains(&pct) && ((o * 100.0) - pct).abs() < 0.01 {
        Some(format!("opacity-{}", pct as i32))
    } else {
        Some(format!("opacity-[{}]", num2(o.max(0.0))))
    }
}

fn shadow_value(n: &Node) -> Option<String> {
    let mut parts = Vec::new();
    for e in &n.effects {
        match e {
            Effect::DropShadow { color, offset, radius, spread, visible: true, .. } => {
                parts.push(format!(
                    "{}px_{}px_{}px_{}px_{}",
                    num(offset.x),
                    num(offset.y),
                    num(*radius),
                    num(*spread),
                    css_color(color, 1.0).replace(", ", ",")
                ));
            }
            Effect::InnerShadow { color, offset, radius, spread, visible: true } => {
                parts.push(format!(
                    "inset_{}px_{}px_{}px_{}px_{}",
                    num(offset.x),
                    num(offset.y),
                    num(*radius),
                    num(*spread),
                    css_color(color, 1.0).replace(", ", ",")
                ));
            }
            _ => {}
        }
    }
    (!parts.is_empty()).then(|| format!("shadow-[{}]", parts.join(",")))
}

fn size_class(prefix: &str, d: Dim, var: Option<&Variable>) -> Option<String> {
    match d {
        Dim::Fixed(px) => Some(spaced(prefix, px, var)),
        Dim::Hug | Dim::Fill => None,
    }
}

fn pad_classes(doc: &Document, n: &Node, al: &AutoLayout) -> Vec<String> {
    let keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    let var = |i: usize| bound(doc, n, keys[i]);
    let p = al.padding;
    if p.iter().all(|v| *v == 0.0) && keys.iter().all(|k| bound(doc, n, k).is_none()) {
        return vec![];
    }
    let same = |a: usize, b: usize| p[a] == p[b] && var(a).map(|v| &v.id) == var(b).map(|v| &v.id);
    if same(0, 1) && same(1, 2) && same(2, 3) {
        return vec![spaced("p", p[0], var(0))];
    }
    let mut out = Vec::new();
    if same(1, 3) {
        out.push(spaced("px", p[1], var(1)));
    } else {
        out.push(spaced("pr", p[1], var(1)));
        out.push(spaced("pl", p[3], var(3)));
    }
    if same(0, 2) {
        out.push(spaced("py", p[0], var(0)));
    } else {
        out.push(spaced("pt", p[0], var(0)));
        out.push(spaced("pb", p[2], var(2)));
    }
    // order: horizontal then vertical, zero sides dropped
    out.retain(|c| !c.ends_with("-0"));
    out
}

fn text_classes(doc: &Document, n: &Node, t: &TextProps, c: &mut Vec<String>) {
    let s = &t.style;
    c.push(format!("font-['{}']", s.font_family.replace(' ', "_")));
    let fs = match s.font_size as i32 {
        12 if s.font_size == 12.0 => Some("text-xs"),
        14 if s.font_size == 14.0 => Some("text-sm"),
        16 if s.font_size == 16.0 => Some("text-base"),
        18 if s.font_size == 18.0 => Some("text-lg"),
        20 if s.font_size == 20.0 => Some("text-xl"),
        24 if s.font_size == 24.0 => Some("text-2xl"),
        30 if s.font_size == 30.0 => Some("text-3xl"),
        36 if s.font_size == 36.0 => Some("text-4xl"),
        48 if s.font_size == 48.0 => Some("text-5xl"),
        60 if s.font_size == 60.0 => Some("text-6xl"),
        72 if s.font_size == 72.0 => Some("text-7xl"),
        _ => None,
    };
    match (bound(doc, n, "fontSize"), fs) {
        (Some(v), _) => c.push(format!("text-[length:{}]", css_var(v))),
        (None, Some(t)) => c.push(t.into()),
        (None, None) => c.push(format!("text-[{}px]", num(s.font_size))),
    }
    let w = match s.font_weight {
        100 => "font-thin",
        200 => "font-extralight",
        300 => "font-light",
        400 => "font-normal",
        500 => "font-medium",
        600 => "font-semibold",
        700 => "font-bold",
        800 => "font-extrabold",
        900 => "font-black",
        other => return c.push(format!("font-[{other}]")),
    };
    match bound(doc, n, "fontWeight") {
        Some(v) => c.push(format!("font-[{}]", css_var(v))),
        None => c.push(w.into()),
    }
    if s.italic {
        c.push("italic".into());
    }
    match (bound(doc, n, "lineHeight"), s.line_height) {
        (Some(v), _) => c.push(format!("leading-[{}]", css_var(v))),
        (None, LineHeight::Auto) => {}
        (None, LineHeight::Pixels(p)) => c.push(spaced("leading", p, None)),
        (None, LineHeight::Percent(p)) => c.push(format!("leading-[{}]", num2(p / 100.0))),
    }
    match (bound(doc, n, "letterSpacing"), s.letter_spacing) {
        (Some(v), _) => c.push(format!("tracking-[{}]", css_var(v))),
        (None, l) if l != 0.0 => c.push(format!("tracking-[{}px]", num(l))),
        _ => {}
    }
    match s.align_h {
        TextAlignH::Left => {}
        TextAlignH::Center => c.push("text-center".into()),
        TextAlignH::Right => c.push("text-right".into()),
        TextAlignH::Justified => c.push("text-justify".into()),
    }
    match s.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => c.push("underline".into()),
        TextDecoration::Strikethrough => c.push("line-through".into()),
    }
    match s.case {
        TextCase::Original => {}
        TextCase::Upper => c.push("uppercase".into()),
        TextCase::Lower => c.push("lowercase".into()),
        TextCase::Title => c.push("capitalize".into()),
        TextCase::SmallCaps => c.push("[font-variant:small-caps]".into()),
    }
    if let Some(p) = first_paint(&s.fills) {
        c.push(color_class(doc, n, "fills/0", "text", p, true));
    }
}

fn classes(doc: &Document, n: &Node, parent: Parent<'_>) -> Vec<String> {
    let mut c: Vec<String> = Vec::new();
    let sz = web_sizes(n, parent);
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let has_children = !visible_children(doc, n).is_empty();
    let abs_children = visible_children(doc, n).iter().any(|k| own_al.is_none() || k.layout_child.absolute);

    if let Some((x, y)) = offset(n, parent, &sz) {
        c.push("absolute".into());
        c.push(format!("left-[{}px]", num(x)));
        c.push(format!("top-[{}px]", num(y)));
    } else if has_children && abs_children && n.kind.is_container() {
        c.push("relative".into());
    }
    let rot = rotation_deg(n);
    if rot != 0.0 {
        c.push(format!("rotate-[{}deg]", num2(rot)));
    }
    if let Some(al) = own_al {
        if al.direction == LayoutDirection::Grid {
            c.push("grid".into());
            c.push(template_class("grid-cols", &al.grid_col_tracks, grid_column_count(al), STOCK_GRID_COLS));
            if al.grid_rows > 0 {
                c.push(template_class("grid-rows", &al.grid_row_tracks, al.grid_rows as usize, STOCK_GRID_ROWS));
            }
            let col_gap = al.item_spacing;
            let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { al.item_spacing };
            let col_var = bound(doc, n, "itemSpacing");
            let row_var = bound(doc, n, "counterAxisSpacing");
            // One `gap` only when a single value drives both axes: a bound item spacing reaches the row only
            // when the row has no spacing of its own (`counterAxisSpacing` unset, so the solver reuses it).
            let one_gap = (row_gap - col_gap).abs() < 1e-4 && row_var.is_none() && (col_var.is_none() || al.counter_spacing <= 0.0);
            if one_gap {
                if col_gap > 0.0 || col_var.is_some() {
                    c.push(spaced("gap", col_gap, col_var));
                }
            } else {
                if col_gap > 0.0 || col_var.is_some() {
                    c.push(spaced("gap-x", col_gap, col_var));
                }
                if row_gap > 0.0 || row_var.is_some() {
                    c.push(spaced("gap-y", row_gap, row_var));
                }
            }
            c.extend(pad_classes(doc, n, al));
        } else {
            c.push("flex".into());
            if al.direction == LayoutDirection::Vertical {
                c.push("flex-col".into());
            }
            if al.wrap {
                c.push("flex-wrap".into());
            }
            match al.primary_align {
                LayoutAlign::Center => c.push("justify-center".into()),
                LayoutAlign::End => c.push("justify-end".into()),
                LayoutAlign::SpaceBetween => c.push("justify-between".into()),
                _ => {}
            }
            match al.counter_align {
                LayoutAlign::Center => c.push("items-center".into()),
                LayoutAlign::End => c.push("items-end".into()),
                LayoutAlign::Baseline => c.push("items-baseline".into()),
                _ => c.push("items-start".into()),
            }
            if al.item_spacing > 0.0 && al.primary_align != LayoutAlign::SpaceBetween {
                c.push(spaced("gap", al.item_spacing, bound(doc, n, "itemSpacing")));
            }
            c.extend(pad_classes(doc, n, al));
        }
    }
    if sz.in_flow {
        if let Some(al) = parent.al.filter(|a| a.direction == LayoutDirection::Grid) {
            let placement = normalise_grid_placement(&n.layout_child, al);
            if let Some(col) = placement.column {
                c.push(line_class("col-start", u32::from(col) + 1, STOCK_COL_START));
            }
            if placement.col_span > 1 {
                c.push(span_class("col", placement.col_span, STOCK_COL_SPAN));
            }
            if let Some(row) = placement.row {
                c.push(line_class("row-start", u32::from(row) + 1, STOCK_ROW_START));
            }
            if placement.row_span > 1 {
                c.push(span_class("row", placement.row_span, STOCK_ROW_SPAN));
            }
            // An item sized by its own content keeps that size instead of stretching across its cell.
            if sz.w == Dim::Hug {
                c.push("justify-self-start".into());
            }
            if sz.h == Dim::Hug {
                c.push("self-start".into());
            }
        }
    }
    // size
    let grid_parent = parent.al.is_some_and(|a| a.direction == LayoutDirection::Grid);
    let main_fill = |is_w: bool| sz.in_flow && !grid_parent && (is_w == sz.row_parent);
    for (is_w, d) in [(true, sz.w), (false, sz.h)] {
        match d {
            Dim::Fill if main_fill(is_w) => {
                if !c.contains(&"flex-1".to_string()) {
                    c.push("flex-1".into());
                }
            }
            Dim::Fill if !grid_parent => c.push("self-stretch".into()),
            _ => {}
        }
    }
    if sz.cross_stretch {
        c.push("self-stretch".into());
    }
    if let Some(w) = size_class("w", sz.w, bound(doc, n, "width")) {
        c.push(w);
    }
    if let Some(h) = size_class("h", sz.h, bound(doc, n, "height")) {
        c.push(h);
    }
    if sz.in_flow && !grid_parent && !c.contains(&"flex-1".to_string()) && (matches!(sz.w, Dim::Fixed(_)) || matches!(sz.h, Dim::Fixed(_))) {
        c.push("shrink-0".into());
    }
    // fills
    if let Some(t) = text_of(n) {
        text_classes(doc, n, t, &mut c);
    } else if !matches!(n.kind, NodeKind::Line) {
        let visible: Vec<(usize, &Paint)> = n.fills.iter().enumerate().filter(|(_, p)| p.visible && p.opacity > 0.0).collect();
        if let Some((i, p)) = visible.last() {
            c.push(color_class(doc, n, &format!("fills/{i}"), "bg", p, false));
        }
    }
    // stroke
    if let Some(p) = n.stroke.paints.iter().find(|p| p.visible && p.opacity > 0.0).filter(|_| n.stroke.weight > 0.0 && text_of(n).is_none()) {
        let w = n.stroke.weight;
        let side = if matches!(n.kind, NodeKind::Line) { "border-t" } else { "border" };
        c.push(match w as i32 {
            1 if w == 1.0 => side.to_string(),
            2 | 4 | 8 if w.fract() == 0.0 => format!("{side}-{}", w as i32),
            _ => format!("{side}-[{}px]", num(w)),
        });
        if n.stroke.dashes.iter().any(|d| *d > 0.0) {
            c.push("border-dashed".into());
        }
        c.push(color_class(doc, n, "strokes/0", "border", p, false));
        if n.stroke.align != StrokeAlign::Inside {
            c.push(format!("outline-offset-[{}px]", if n.stroke.align == StrokeAlign::Center { num(-w / 2.0) } else { "0".into() }));
        }
    }
    // shape
    match &n.kind {
        NodeKind::Ellipse(a) if (a.end - a.start).abs() >= std::f32::consts::TAU - 1e-4 && a.inner_radius <= 0.0 => c.push("rounded-full".into()),
        NodeKind::Rectangle | NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } => {
            c.extend(corner_classes(doc, n))
        }
        _ => {}
    }
    if n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) && has_children {
        c.push("overflow-hidden".into());
    }
    // effects
    if let Some(s) = shadow_value(n) {
        c.push(s);
    }
    for e in &n.effects {
        match e {
            Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => c.push(format!("blur-[{}px]", num(radius / 2.0))),
            Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => c.push(format!("backdrop-blur-[{}px]", num(radius / 2.0))),
            _ => {}
        }
    }
    match bound(doc, n, "opacity") {
        Some(v) => c.push(format!("opacity-[calc({}/100)]", css_var(v))),
        None => c.extend(opacity_class(n.opacity)),
    }
    c
}

fn escape_text(s: &str, jsx: bool) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '{' if jsx => o.push_str("&#123;"),
            '}' if jsx => o.push_str("&#125;"),
            '\n' => o.push_str("<br />"),
            c => o.push(c),
        }
    }
    o
}

/// ` name="value"` attributes of an instance's component properties.
pub(crate) fn instance_attrs(doc: &Document, n: &Node) -> String {
    let mut o = String::new();
    for (name, lit) in instance_args(doc, n) {
        match lit {
            PropLit::Bool(true) => o.push_str(&format!(" {name}")),
            PropLit::Bool(false) => o.push_str(&format!(" {name}={{false}}")),
            PropLit::Text(t) => o.push_str(&format!(" {name}=\"{}\"", t.replace('"', "&quot;"))),
            PropLit::Instance(v) => o.push_str(&format!(" {name}={{<{v} />}}")),
        }
    }
    o
}

fn markup(doc: &Document, id: NodeId, parent: Parent, depth: usize, o: &mut Out, jsx: bool) {
    let Some(n) = doc.get(id) else { return };
    if !n.visible {
        return;
    }
    let attr = if jsx { "className" } else { "class" };
    let cls = classes(doc, n, parent).join(" ");
    let class_attr = if cls.is_empty() { String::new() } else { format!(" {attr}=\"{cls}\"") };

    if matches!(n.kind, NodeKind::Instance { .. }) {
        let vname = view_name(doc, n);
        let me = Parent::of(n);
        let fills = react_fills(doc, n);
        if jsx {
            let gap = if fills.is_empty() { " " } else { "" }; // `<X a="b" />`
            let head = [(0, format!("<{vname}{}{class_attr}{gap}", instance_attrs(doc, n)))];
            emit_jsx_instance(o, depth, &vname, &head, &fills, &|o, k, d| markup(doc, k, me, d, o, true));
        } else if fills.is_empty() {
            o.line(depth, format!("<{vname}{}{class_attr} />", instance_attrs(doc, n)));
        } else {
            // Plain HTML has no props: named slots become `<template slot="name">` children.
            o.line(depth, format!("<{vname}{}{class_attr}>", instance_attrs(doc, n)));
            for id in fills.children.iter().flatten() {
                markup(doc, *id, me, depth + 1, o, false);
            }
            for (prop, ids) in &fills.props {
                o.line(depth + 1, format!("<template slot=\"{prop}\">"));
                for id in ids {
                    markup(doc, *id, me, depth + 2, o, false);
                }
                o.line(depth + 1, "</template>");
            }
            o.line(depth, format!("</{vname}>"));
        }
        return;
    }
    if is_vector_asset(doc, id) {
        let src = format!("/assets/{}.svg", kind_slug(&n.name));
        let size = if jsx {
            format!(" width={{{}}} height={{{}}}", num(n.size.x), num(n.size.y))
        } else {
            format!(" width=\"{}\" height=\"{}\"", num(n.size.x), num(n.size.y))
        };
        let no_size: String = cls.split(' ').filter(|c| !c.starts_with("w-") && !c.starts_with("h-")).collect::<Vec<_>>().join(" ");
        let class_attr = if no_size.is_empty() { String::new() } else { format!(" {attr}=\"{no_size}\"") };
        o.line(depth, format!("<img src=\"{src}\" alt=\"{}\"{size}{class_attr} />", n.name.replace('"', "&quot;")));
        return;
    }
    if let Some(t) = text_of(n) {
        o.line(depth, format!("<p{class_attr}>{}</p>", escape_text(&t.content, jsx)));
        return;
    }
    let tag = if matches!(n.kind, NodeKind::Section) { "section" } else { "div" };
    let site = slot_site(doc, id);
    // A JSX slot renders through its parameter; HTML has no parameters, so it keeps the default content.
    let kids = if jsx && site.is_some() { Vec::new() } else { visible_children(doc, n) };
    if kids.is_empty() && site.is_none() {
        if jsx {
            o.line(depth, format!("<{tag}{class_attr} />"));
        } else {
            o.line(depth, format!("<{tag}{class_attr}></{tag}>"));
        }
        return;
    }
    o.line(depth, format!("<{tag}{class_attr}>"));
    let me = Parent::of(n);
    if let Some(site) = &site {
        if jsx {
            emit_react_slot(o, depth + 1, site, &|o, k, d| markup(doc, k, me, d, o, true));
        } else {
            o.line(depth + 1, format!("<!-- slot: {} -->", site.name));
        }
    }
    for k in kids {
        markup(doc, k.id, me, depth + 1, o, jsx);
    }
    o.line(depth, format!("</{tag}>"));
}

fn kind_slug(name: &str) -> String {
    crate::codegen::kebab(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fixture;

    #[test]
    fn golden_card() {
        let (d, id) = fixture::card();
        let expected = r####"<div class="flex flex-col items-start gap-2 p-4 w-[312px] h-[200px] bg-[#FFFFFF] border border-[#E5E5E5] rounded-lg overflow-hidden shadow-[0px_4px_8px_0px_rgba(0,0,0,0.25)]">
  <p class="font-['Inter'] text-xl font-semibold text-[#111111]">Hello world</p>
  <p class="self-stretch font-['Inter'] text-sm font-normal leading-5 text-center text-[color:var(--Color-Text-Muted)]">Body copy</p>
  <div class="flex justify-center items-center gap-[var(--Spacing-Md)] px-3 py-2 self-stretch bg-[var(--Color-Primary)] rounded">
    <Button label="Save" disabled class="w-20 h-8 shrink-0" />
    <div class="w-6 h-6 shrink-0 bg-[#FF0000] rounded-full"></div>
  </div>
</div>"####;
        assert_eq!(tailwind_for(&d, id), expected);
    }

    #[test]
    fn exact_values_when_off_scale() {
        let (mut d, id) = fixture::card();
        d.get_mut(id).unwrap().size = Vec2::new(313.0, 200.0);
        d.get_mut(id).unwrap().corner_radii = [10.0, 10.0, 0.0, 0.0];
        d.get_mut(id).unwrap().opacity = 0.37;
        let out = tailwind_for(&d, id);
        assert!(out.contains("w-[313px]"), "{out}");
        assert!(out.contains("rounded-tl-[10px] rounded-tr-[10px]"), "{out}");
        assert!(out.contains("opacity-[0.37]"), "{out}");
    }

    #[test]
    fn golden_grid_tracks_placement_and_gaps() {
        let mut d = Document::new("grid");
        let page = d.add_page("p");
        let gid = d.alloc_id();
        let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 480.0, 200.0));
        g.name = "DashboardGrid".into();
        g.fills = vec![];
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
        g.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), ..Default::default() });
        let g = d.insert(page, g).unwrap();

        let c1 = d.alloc_id();
        let mut n1 = Node::rectangle(c1, Rect::new(0.0, 0.0, 200.0, 80.0));
        n1.fills = vec![];
        n1.layout_child.grid_column = Some(1);
        n1.layout_child.grid_row = Some(0);
        n1.layout_child.col_span = 2;
        n1.layout_child.row_span = 1;
        d.insert(g, n1).unwrap();

        let expected = "<div class=\"grid grid-cols-[minmax(0,1fr)_120px_auto] grid-rows-[80px_auto] gap-x-4 gap-y-6 px-4 py-3 w-[480px] h-[200px]\">\n  <div class=\"col-start-2 col-span-2 row-start-1 w-[200px] h-20\"></div>\n</div>";
        assert_eq!(tailwind_for(&d, g), expected);
    }

    /// The utility classes on the `n`th line of `html`, split into tokens.
    fn tokens(html: &str, n: usize) -> Vec<String> {
        let line = html.lines().nth(n).unwrap_or_default();
        let start = line.find("class=\"").map(|i| i + 7).unwrap_or(0);
        let end = line[start..].find('"').map(|i| start + i).unwrap_or(start);
        line[start..end].split_whitespace().map(String::from).collect()
    }

    #[test]
    fn grid_items_keep_their_size_and_only_content_sized_items_align_to_start() {
        let al = AutoLayout { grid_columns: 3, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(400.0, 300.0));
        // line 1: fixed item spanning 2 columns and 2 rows
        let span = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(span).unwrap().layout_child.col_span = 2;
        d.get_mut(span).unwrap().layout_child.row_span = 2;
        // line 2: a rectangle whose sizing mode says Hug keeps its pixel size
        let hug_rect = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(hug_rect).unwrap().layout_child.horizontal = Sizing::Hug;
        d.get_mut(hug_rect).unwrap().layout_child.vertical = Sizing::Hug;
        // line 3: a frame that hugs its own content is the content-sized case
        let hug_frame = d.alloc_id();
        let mut f = Node::frame(hug_frame, Rect::new(0.0, 0.0, 60.0, 24.0));
        f.fills.clear();
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.auto_layout = Some(AutoLayout { primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() });
        }
        d.insert(grid, f).unwrap();
        // line 4: a Fill item is stretched by the grid
        let fill = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(fill).unwrap().layout_child.horizontal = Sizing::Fill;
        d.get_mut(fill).unwrap().layout_child.vertical = Sizing::Fill;
        d.relayout_subtree(grid);

        let html = tailwind_for(&d, grid);
        let has = |line: usize, want: &str| tokens(&html, line).iter().any(|t| t == want);
        for want in ["col-span-2", "row-span-2", "w-20", "h-10"] {
            assert!(has(1, want), "fixed spanning item must keep {want}:\n{html}");
        }
        assert!(!has(1, "justify-self-start") && !has(1, "self-start"), "{html}");
        for want in ["w-20", "h-10"] {
            assert!(has(2, want), "a rectangle with Hug sizing mode must keep {want}:\n{html}");
        }
        assert!(!has(2, "justify-self-start") && !has(2, "self-start"), "{html}");
        assert!(has(3, "justify-self-start") && has(3, "self-start"), "a content-sized item must align to the start:\n{html}");
        assert!(!tokens(&html, 3).iter().any(|t| t.starts_with("w-") || t.starts_with("h-")), "{html}");
        assert!(!tokens(&html, 4).iter().any(|t| t.starts_with("w-") || t.starts_with("h-") || t.contains("self-start")), "a Fill item is stretched: {html}");
    }

    #[test]
    fn hugging_grid_exports_its_solved_pixel_size_with_fractional_tracks() {
        let al = AutoLayout { grid_columns: 3, grid_rows: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(500.0, 500.0));
        fixture::add_rect(&mut d, grid, Vec2::new(100.0, 10.0));
        d.relayout_subtree(grid);
        let size = d.get(grid).unwrap().size;
        assert_eq!(size, Vec2::new(180.0, 10.0), "the solver sizes the columns 100, 40 and 40 and the rows at their tallest child");
        let html = tailwind_for(&d, grid);
        let t = tokens(&html, 0);
        for want in ["grid", "grid-cols-3", "grid-rows-2", "w-[180px]", "h-2.5"] {
            assert!(t.iter().any(|c| c == want), "{want} missing in {html}");
        }
        assert!(!t.iter().any(|c| c.contains("max") || c.contains("auto")), "{html}");

        let al = AutoLayout { grid_columns: 3, grid_rows: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (d, grid) = fixture::grid_frame(al, Vec2::new(300.0, 200.0));
        let t = tokens(&tailwind_for(&d, grid), 0);
        for want in ["grid-cols-3", "grid-rows-2", "w-[300px]", "h-[200px]"] {
            assert!(t.iter().any(|c| c == want), "{want} missing in {t:?}");
        }
    }

    #[test]
    fn grid_item_with_grow_stretches_across_its_cell_and_keeps_its_height() {
        let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(400.0, 300.0));
        let rect = fixture::add_rect(&mut d, grid, Vec2::new(80.0, 40.0));
        d.get_mut(rect).unwrap().layout_child.grow = 1.0;
        d.relayout_subtree(grid);
        assert_eq!(d.get(rect).unwrap().size, Vec2::new(200.0, 40.0), "the solver stretches a grow item across its column and leaves its height");
        let html = tailwind_for(&d, grid);
        let t = tokens(&html, 1);
        assert!(t.iter().any(|c| c == "h-10"), "the height stays: {html}");
        assert!(!t.iter().any(|c| c.starts_with("w-")), "the grid stretches the item across its cell: {html}");
    }

    #[test]
    fn stretch_child_keeps_its_cross_size_when_the_parent_hugs() {
        use crate::codegen::fixture::row_with_stretch_children;
        let (d, row, _) = row_with_stretch_children(Sizing::Hug, &[40.0]);
        let out = tailwind_for(&d, row);
        let child = out.lines().find(|l| l.contains("w-[30px]") || l.contains("w-7")).unwrap_or_else(|| panic!("no child line in {out}"));
        assert!(child.contains("h-10") && child.contains("self-stretch"), "{child}");

        let (d, row, _) = row_with_stretch_children(Sizing::Hug, &[20.0, 100.0]);
        let out = tailwind_for(&d, row);
        let children: Vec<&str> = out.lines().filter(|l| l.contains("self-stretch")).collect();
        assert_eq!(children.len(), 2, "{out}");
        assert!(children.iter().all(|l| l.contains("h-[100px]")), "all-stretch row must not collapse: {out}");
    }

    #[test]
    fn stretch_child_leaves_the_cross_size_to_a_fixed_parent() {
        use crate::codegen::fixture::row_with_stretch_children;
        let (d, row, _) = row_with_stretch_children(Sizing::Fixed, &[40.0]);
        let out = tailwind_for(&d, row);
        let child = out.lines().find(|l| l.contains("self-stretch")).unwrap_or_else(|| panic!("no stretch child in {out}"));
        assert!(!child.contains(" h-"), "{child}");
    }
}
