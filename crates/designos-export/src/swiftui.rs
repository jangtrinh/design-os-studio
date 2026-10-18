//! SwiftUI output.

use crate::codegen::*;
use crate::slots::*;
use crate::util::num;
use designos_doc::*;

/// A SwiftUI view for `node` and its descendants. Auto layout becomes `HStack`/`VStack` with
/// spacing and padding, text becomes `Text` with font, weight and color, shapes become
/// `RoundedRectangle`/`Circle` with fill and stroke; `.shadow`, `.opacity` and `.frame` are
/// added as modifiers. Instances use their component name as view name and pass changed
/// component properties as arguments; bound variables become `Color("name")` / `Tokens.name`.
pub fn swiftui_for(doc: &Document, node: NodeId) -> String {
    let mut o = Out::new("    ");
    let slots = slot_infos(doc, node);
    match doc.get(node) {
        Some(n) if !slots.is_empty() => slot_view(doc, n, &slots, &mut o),
        _ => view(doc, node, Parent::root(), 0, &mut o),
    }
    o.finish()
}

/// Generic type parameter that carries the content of slot `name`.
fn slot_type(name: &str) -> String {
    format!("{}Content", pascal(name))
}

/// `name: Type = default` initializer parameters for the non-slot properties of a component.
fn prop_init_params(props: &[PropParam]) -> Vec<String> {
    props
        .iter()
        .map(|p| match &p.default {
            PropDefault::Bool(b) => format!("{}: Bool = {b}", p.name),
            PropDefault::Text(t) => format!("{}: String = {}", p.name, quote(t)),
            PropDefault::Instance(v) => format!("{}: AnyView = AnyView({})", p.name, v.as_ref().map_or("EmptyView()".to_string(), |v| format!("{v}()"))),
        })
        .collect()
}

/// A component with slots as a generic `View` struct: its properties and one `@ViewBuilder` init
/// parameter per slot, and an `init` for callers that keep every slot's default content.
fn slot_view(doc: &Document, n: &Node, slots: &[SlotInfo], o: &mut Out) {
    let name = view_name(doc, n);
    let props = prop_params(doc, n.id);
    let prop_params = prop_init_params(&props);
    let generics: Vec<String> = slots.iter().map(|s| format!("{}: View", slot_type(&s.name))).collect();
    o.line(0, format!("struct {name}<{}>: View {{", generics.join(", ")));
    for (p, decl) in props.iter().zip(&prop_params) {
        let ty = decl.split_once(": ").and_then(|(_, r)| r.split(" = ").next()).unwrap_or("String");
        o.line(1, format!("private let {}: {ty}", p.name));
    }
    for s in slots {
        o.line(1, format!("private let {}: {}?", slot_ident(&s.name, Lang::Swift), slot_type(&s.name)));
    }
    o.line(0, "");
    let closures: Vec<String> = slots.iter().map(|s| format!("@ViewBuilder {}: () -> {}", slot_ident(&s.name, Lang::Swift), slot_type(&s.name))).collect();
    o.line(1, format!("init({}) {{", prop_params.iter().chain(&closures).cloned().collect::<Vec<_>>().join(", ")));
    for p in &props {
        o.line(2, format!("self.{0} = {0}", p.name));
    }
    for s in slots {
        let id = slot_ident(&s.name, Lang::Swift);
        o.line(2, format!("self.{id} = {id}()"));
    }
    o.line(1, "}");
    o.line(0, "");
    o.line(1, "var body: some View {");
    view(doc, n.id, Parent::root(), 2, o);
    o.line(1, "}");
    o.line(0, "}");
    o.line(0, "");
    let empties: Vec<String> = slots.iter().map(|s| format!("{} == EmptyView", slot_type(&s.name))).collect();
    o.line(0, format!("extension {name} where {} {{", empties.join(", ")));
    o.line(1, format!("init({}) {{", prop_params.join(", ")));
    for p in &props {
        o.line(2, format!("self.{0} = {0}", p.name));
    }
    for s in slots {
        o.line(2, format!("self.{} = nil", slot_ident(&s.name, Lang::Swift)));
    }
    o.line(1, "}");
    o.line(0, "}");
}

fn color(doc: &Document, n: &Node, key: &str, c: &Color, opacity: f32) -> String {
    if let Some(v) = bound(doc, n, key) {
        return token_text(v, CodeSyntaxPlatform::Ios, || format!("Color(\"{}\")", token_camel(v, true)));
    }
    let a = (c.a * opacity).clamp(0.0, 1.0);
    let base = if c.r == 1.0 && c.g == 1.0 && c.b == 1.0 {
        "Color.white".to_string()
    } else if c.r == 0.0 && c.g == 0.0 && c.b == 0.0 {
        "Color.black".to_string()
    } else {
        let f = |v: f32| num((v * 1000.0).round() / 1000.0);
        format!("Color(red: {}, green: {}, blue: {})", f(c.r), f(c.g), f(c.b))
    };
    if a < 0.999 {
        format!("{base}.opacity({})", num((a * 100.0).round() / 100.0))
    } else {
        base
    }
}

fn unit_point(p: Vec2) -> String {
    format!("UnitPoint(x: {}, y: {})", num((p.x * 1000.0).round() / 1000.0), num((p.y * 1000.0).round() / 1000.0))
}

fn gradient_stops(doc: &Document, n: &Node, stops: &[GradientStop], opacity: f32) -> String {
    let items: Vec<String> =
        stops.iter().map(|s| format!("Gradient.Stop(color: {}, location: {})", color(doc, n, "", &s.color, opacity), num(s.pos))).collect();
    format!("[{}]", items.join(", "))
}

/// A `ShapeStyle` expression for a paint.
fn fill_expr(doc: &Document, n: &Node, key: &str, p: &Paint) -> String {
    match &p.kind {
        PaintKind::Solid(c) => color(doc, n, key, c, p.opacity),
        PaintKind::LinearGradient { handles, stops } => {
            format!(
                "LinearGradient(stops: {}, startPoint: {}, endPoint: {})",
                gradient_stops(doc, n, stops, p.opacity),
                unit_point(handles[0]),
                unit_point(handles[1])
            )
        }
        PaintKind::RadialGradient { handles, stops } | PaintKind::DiamondGradient { handles, stops } => {
            let r = ((handles[1].x - handles[0].x) * n.size.x).hypot((handles[1].y - handles[0].y) * n.size.y);
            format!(
                "RadialGradient(stops: {}, center: {}, startRadius: 0, endRadius: {})",
                gradient_stops(doc, n, stops, p.opacity),
                unit_point(handles[0]),
                num(r)
            )
        }
        PaintKind::AngularGradient { handles, stops } => {
            format!("AngularGradient(stops: {}, center: {})", gradient_stops(doc, n, stops, p.opacity), unit_point(handles[0]))
        }
        PaintKind::Image { image_ref, .. } => format!("Color.gray /* image fill {} */", &image_ref.chars().take(8).collect::<String>()),
    }
}

fn dimension(doc: &Document, n: &Node, key: &str, v: f32) -> String {
    match bound(doc, n, key) {
        Some(var) => token_text(var, CodeSyntaxPlatform::Ios, || format!("Tokens.{}", token_camel(var, false))),
        None => num(v),
    }
}

fn shape(doc: &Document, n: &Node) -> String {
    match &n.kind {
        NodeKind::Ellipse(_) => {
            if (n.size.x - n.size.y).abs() < 0.01 {
                "Circle()".into()
            } else {
                "Ellipse()".into()
            }
        }
        _ => {
            let r = n.corner_radii;
            let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
            let bound_any = keys.iter().any(|k| bound(doc, n, k).is_some());
            if r.iter().all(|v| *v == r[0]) && !bound_any
                || (bound_any && keys.iter().all(|k| bound(doc, n, k).map(|v| &v.id) == bound(doc, n, keys[0]).map(|v| &v.id)))
            {
                if r[0] == 0.0 && !bound_any {
                    "Rectangle()".into()
                } else {
                    format!("RoundedRectangle(cornerRadius: {})", dimension(doc, n, keys[0], r[0]))
                }
            } else {
                format!(
                    "UnevenRoundedRectangle(topLeadingRadius: {}, bottomLeadingRadius: {}, bottomTrailingRadius: {}, topTrailingRadius: {})",
                    dimension(doc, n, keys[0], r[0]),
                    dimension(doc, n, keys[3], r[3]),
                    dimension(doc, n, keys[2], r[2]),
                    dimension(doc, n, keys[1], r[1])
                )
            }
        }
    }
}

fn stroke_expr(doc: &Document, n: &Node) -> Option<String> {
    let p = n.stroke.paints.iter().find(|p| p.visible && p.opacity > 0.0)?;
    if n.stroke.weight <= 0.0 {
        return None;
    }
    let dash = if n.stroke.dashes.iter().any(|d| *d > 0.0) {
        let d: Vec<String> = n.stroke.dashes.iter().map(|d| num(*d)).collect();
        format!(", style: StrokeStyle(lineWidth: {}, dash: [{}])", num(n.stroke.weight), d.join(", "))
    } else {
        String::new()
    };
    let fill = fill_expr(doc, n, "strokes/0", p);
    if dash.is_empty() {
        Some(format!("{fill}, lineWidth: {}", dimension(doc, n, "strokeWeight", n.stroke.weight)))
    } else {
        Some(format!("{fill}{dash}"))
    }
}

fn frame_args(w: Dim, h: Dim, align: Option<&str>, n: &Node, doc: &Document) -> Option<String> {
    let mut a: Vec<String> = Vec::new();
    match w {
        Dim::Fixed(px) => a.push(format!("width: {}", dimension(doc, n, "width", px))),
        Dim::Fill => a.push("maxWidth: .infinity".into()),
        Dim::Hug => {}
    }
    match h {
        Dim::Fixed(px) => a.push(format!("height: {}", dimension(doc, n, "height", px))),
        Dim::Fill => a.push("maxHeight: .infinity".into()),
        Dim::Hug => {}
    }
    if a.is_empty() {
        return None;
    }
    if let Some(al) = align {
        a.push(format!("alignment: {al}"));
    }
    Some(format!(".frame({})", a.join(", ")))
}

pub(crate) fn weight_name(w: u16) -> &'static str {
    match w {
        0..=149 => ".ultraLight",
        150..=249 => ".thin",
        250..=349 => ".light",
        350..=449 => ".regular",
        450..=549 => ".medium",
        550..=649 => ".semibold",
        650..=749 => ".bold",
        750..=849 => ".heavy",
        _ => ".black",
    }
}

fn text_mods(doc: &Document, n: &Node, t: &TextProps, m: &mut Vec<String>) {
    let s = &t.style;
    m.push(format!(".font(.custom(\"{}\", size: {}))", s.font_family, dimension(doc, n, "fontSize", s.font_size)));
    if s.font_weight != 400 {
        m.push(format!(".fontWeight({})", weight_name(s.font_weight)));
    }
    if s.italic {
        m.push(".italic()".into());
    }
    if let Some(p) = first_paint(&s.fills) {
        m.push(format!(".foregroundColor({})", fill_expr(doc, n, "fills/0", p)));
    }
    match s.align_h {
        TextAlignH::Left => {}
        TextAlignH::Center => m.push(".multilineTextAlignment(.center)".into()),
        TextAlignH::Right => m.push(".multilineTextAlignment(.trailing)".into()),
        TextAlignH::Justified => m.push(".multilineTextAlignment(.leading)".into()),
    }
    match s.line_height {
        LineHeight::Auto => {}
        LineHeight::Pixels(p) => m.push(format!(".lineSpacing({})", num((p - s.font_size).max(0.0)))),
        LineHeight::Percent(p) => m.push(format!(".lineSpacing({})", num((s.font_size * p / 100.0 - s.font_size).max(0.0)))),
    }
    if s.letter_spacing != 0.0 {
        m.push(format!(".tracking({})", dimension(doc, n, "letterSpacing", s.letter_spacing)));
    }
    match s.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => m.push(".underline()".into()),
        TextDecoration::Strikethrough => m.push(".strikethrough()".into()),
    }
    match s.case {
        TextCase::Upper => m.push(".textCase(.uppercase)".into()),
        TextCase::Lower => m.push(".textCase(.lowercase)".into()),
        _ => {}
    }
}

fn alignment(h: &str, v: &str) -> String {
    match (v, h) {
        ("center", "center") => ".center".into(),
        ("center", h) => format!(".{h}"),
        (v, "center") => format!(".{v}"),
        (v, h) => format!(".{v}{}{}", h[..1].to_uppercase(), &h[1..]),
    }
}

fn padding_mods(doc: &Document, n: &Node, al: &AutoLayout, m: &mut Vec<String>) {
    let keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    let p = al.padding;
    let val = |i: usize| dimension(doc, n, keys[i], p[i]);
    let same = |a: usize, b: usize| p[a] == p[b] && bound(doc, n, keys[a]).map(|v| &v.id) == bound(doc, n, keys[b]).map(|v| &v.id);
    let any = |i: usize| p[i] > 0.0 || bound(doc, n, keys[i]).is_some();
    if !(0..4).any(any) {
        return;
    }
    if same(0, 1) && same(1, 2) && same(2, 3) {
        m.push(format!(".padding({})", val(0)));
        return;
    }
    if same(1, 3) && any(1) {
        m.push(format!(".padding(.horizontal, {})", val(1)));
    } else {
        if any(3) {
            m.push(format!(".padding(.leading, {})", val(3)));
        }
        if any(1) {
            m.push(format!(".padding(.trailing, {})", val(1)));
        }
    }
    if same(0, 2) && any(0) {
        m.push(format!(".padding(.vertical, {})", val(0)));
    } else {
        if any(0) {
            m.push(format!(".padding(.top, {})", val(0)));
        }
        if any(2) {
            m.push(format!(".padding(.bottom, {})", val(2)));
        }
    }
}

fn call(name: &str, args: &[String]) -> String {
    format!("{name}({})", args.join(", "))
}

fn view(doc: &Document, id: NodeId, parent: Parent<'_>, depth: usize, o: &mut Out) {
    let Some(n) = doc.get(id) else { return };
    if !n.visible {
        return;
    }
    let sz = sizes(n, parent);
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let site = slot_site(doc, id);
    let kids = if site.is_some() { Vec::new() } else { visible_children(doc, n) };
    let mut mods: Vec<String> = Vec::new();
    let mut head: Vec<String> = Vec::new(); // lines of the view expression before modifiers
    let mut body: Option<Vec<NodeId>> = None; // children inside braces
    let mut stack: Option<String> = None; // "HStack(...) {"
    let mut spacers = (false, false, false); // before, between, after
    let radius_shape = shape(doc, n);

    if let NodeKind::Instance { .. } = n.kind {
        let args: Vec<String> = instance_args(doc, n)
            .into_iter()
            .map(|(name, lit)| match lit {
                PropLit::Bool(b) => format!("{name}: {b}"),
                PropLit::Text(t) => format!("{name}: {}", quote(&t)),
                PropLit::Instance(v) => format!("{name}: AnyView({v}())"),
            })
            .collect();
        let slots = instance_slots(doc, n);
        let vname = view_name(doc, n);
        if slots.fills.is_empty() {
            head.push(call(&vname, &args));
        } else if slots.single() {
            stack = Some(if args.is_empty() { format!("{vname} {{") } else { format!("{} {{", call(&vname, &args)) });
            body = Some(slots.fills.into_iter().flat_map(|f| f.ids).collect());
        } else {
            // The generic view needs a closure for every slot; one the instance leaves out stays empty.
            let me = Parent::of(n);
            o.line(depth, format!("{vname}("));
            let total = args.len() + slots.component.len();
            let mut written = 0;
            for a in &args {
                written += 1;
                o.line(depth + 1, format!("{a}{}", if written < total { "," } else { "" }));
            }
            for info in &slots.component {
                written += 1;
                o.line(depth + 1, format!("{}: {{", slot_ident(&info.name, Lang::Swift)));
                match slots.fills.iter().find(|f| f.name == info.name).filter(|f| !f.ids.is_empty()) {
                    Some(f) => f.ids.iter().for_each(|c| view(doc, *c, me, depth + 2, o)),
                    None => o.line(depth + 2, "EmptyView()"),
                }
                o.line(depth + 1, format!("}}{}", if written < total { "," } else { "" }));
            }
            o.line(depth, ")");
        }
        if let Some(f) = frame_args(sz.w, sz.h, None, n, doc) {
            mods.push(f);
        }
    } else if is_vector_asset(doc, id) {
        head.push(format!("Image(\"{}\")", kebab(&n.name)));
        mods.push(".resizable()".into());
        mods.push(format!(".frame(width: {}, height: {})", num(n.size.x), num(n.size.y)));
    } else if let Some(t) = text_of(n) {
        head.push(format!("Text({})", quote(&t.content)));
        text_mods(doc, n, t, &mut mods);
        if let Some(f) = frame_args(sz.w, sz.h, None, n, doc) {
            mods.push(f);
        }
    } else if let Some(al) = own_al.filter(|_| !kids.is_empty() || site.is_some()) {
        let horizontal = al.direction == LayoutDirection::Horizontal;
        let counter = match (al.counter_align, horizontal) {
            (LayoutAlign::Center, _) => "center",
            (LayoutAlign::End, true) => "bottom",
            (LayoutAlign::End, false) => "trailing",
            (LayoutAlign::Baseline, true) => "firstTextBaseline",
            (_, true) => "top",
            (_, false) => "leading",
        };
        let mut args = Vec::new();
        if counter != "center" {
            args.push(format!("alignment: .{counter}"));
        }
        if al.item_spacing > 0.0 && al.primary_align != LayoutAlign::SpaceBetween {
            args.push(format!("spacing: {}", dimension(doc, n, "itemSpacing", al.item_spacing)));
        } else if al.item_spacing == 0.0 {
            args.push("spacing: 0".into());
        }
        let name = if horizontal { "HStack" } else { "VStack" };
        if al.wrap {
            head.push("// auto layout wraps: use a flow layout or LazyVGrid".into());
        }
        stack = Some(if args.is_empty() { format!("{name} {{") } else { format!("{} {{", call(name, &args)) });
        spacers = match al.primary_align {
            LayoutAlign::Center => (true, false, true),
            LayoutAlign::End => (true, false, false),
            LayoutAlign::SpaceBetween => (false, true, false),
            _ => (false, false, false),
        };
        body = Some(kids.iter().map(|k| k.id).collect());
        padding_mods(doc, n, al, &mut mods);
        let pos = |a: LayoutAlign| match a {
            LayoutAlign::Center => 1,
            LayoutAlign::End => 2,
            _ => 0,
        };
        let (hp, vp) = if horizontal { (pos(al.primary_align), pos(al.counter_align)) } else { (pos(al.counter_align), pos(al.primary_align)) };
        let (h, v) = (["leading", "center", "trailing"][hp], ["top", "center", "bottom"][vp]);
        let al_s = alignment(h, v);
        let align = (al_s != ".center").then_some(al_s);
        if let Some(f) = frame_args(sz.w, sz.h, align.as_deref(), n, doc) {
            mods.push(f);
        }
        container_style(doc, n, &radius_shape, &mut mods);
    } else if n.kind.is_container() && (!kids.is_empty() || site.is_some()) {
        stack = Some("ZStack(alignment: .topLeading) {".into());
        body = Some(kids.iter().map(|k| k.id).collect());
        if let Some(f) = frame_args(sz.w, sz.h, Some(".topLeading"), n, doc) {
            mods.push(f);
        }
        container_style(doc, n, &radius_shape, &mut mods);
    } else if matches!(n.kind, NodeKind::Line) {
        head.push("Rectangle()".into());
        if let Some(p) = first_paint(&n.stroke.paints) {
            mods.push(format!(".fill({})", fill_expr(doc, n, "strokes/0", p)));
        }
        mods.push(format!(".frame(width: {}, height: {})", num(n.size.x), dimension(doc, n, "strokeWeight", n.stroke.weight.max(1.0))));
    } else {
        // leaf shape
        head.push(radius_shape.clone());
        let fill = n.fills.iter().enumerate().rev().find(|(_, p)| p.visible && p.opacity > 0.0);
        let stroke = stroke_expr(doc, n);
        match (fill, &stroke) {
            (Some((i, p)), _) => mods.push(format!(".fill({})", fill_expr(doc, n, &format!("fills/{i}"), p))),
            (None, Some(s)) => mods.push(format!(".stroke({s})")),
            (None, None) => {}
        }
        if let (Some(_), Some(s)) = (fill, &stroke) {
            mods.push(format!(".overlay({}.stroke({s}))", radius_shape));
        }
        if let Some(f) =
            frame_args(if sz.w == Dim::Hug { Dim::Fixed(n.size.x) } else { sz.w }, if sz.h == Dim::Hug { Dim::Fixed(n.size.y) } else { sz.h }, None, n, doc)
        {
            mods.push(f);
        }
    }

    // effects common to every view
    for (c, off, radius, _) in drop_shadows(n) {
        mods.push(format!(".shadow(color: {}, radius: {}, x: {}, y: {})", color(doc, n, "", c, 1.0), num(radius / 2.0), num(off.x), num(off.y)));
    }
    for e in &n.effects {
        match e {
            Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => mods.push(format!(".blur(radius: {})", num(radius / 2.0))),
            Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => mods.push(".background(.ultraThinMaterial)".into()),
            _ => {}
        }
    }
    match bound(doc, n, "opacity") {
        Some(v) => mods.push(format!(".opacity({} / 100)", token_text(v, CodeSyntaxPlatform::Ios, || format!("Tokens.{}", token_camel(v, false))))),
        None if n.opacity < 0.999 => mods.push(format!(".opacity({})", num((n.opacity * 100.0).round() / 100.0))),
        None => {}
    }
    let rot = rotation_deg(n);
    if rot != 0.0 {
        mods.push(format!(".rotationEffect(.degrees({}))", num(rot)));
    }
    if let Some((x, y)) = offset(n, parent, &sz) {
        if x != 0.0 || y != 0.0 {
            mods.push(format!(".offset(x: {}, y: {})", num(x), num(y)));
        }
    }

    for h in &head {
        o.line(depth, h);
    }
    if let (Some(open), Some(children)) = (&stack, &body) {
        o.line(depth, open);
        let me = Parent::of(n);
        if spacers.0 {
            o.line(depth + 1, "Spacer()");
        }
        if let Some(site) = &site {
            let name = slot_ident(&site.name, Lang::Swift);
            o.line(depth + 1, format!("if let {name} {{"));
            o.line(depth + 2, &name);
            if site.defaults.is_empty() {
                o.line(depth + 1, "}");
            } else {
                o.line(depth + 1, "} else {");
                for c in &site.defaults {
                    view(doc, *c, me, depth + 2, o);
                }
                o.line(depth + 1, "}");
            }
        }
        for (i, c) in children.iter().enumerate() {
            if i > 0 && spacers.1 {
                o.line(depth + 1, "Spacer()");
            }
            view(doc, *c, me, depth + 1, o);
        }
        if spacers.2 {
            o.line(depth + 1, "Spacer()");
        }
        o.line(depth, "}");
    }
    for m in mods {
        o.line(depth + 1, m);
    }
}

fn container_style(doc: &Document, n: &Node, shape: &str, m: &mut Vec<String>) {
    let fills: Vec<(usize, &Paint)> = n.fills.iter().enumerate().filter(|(_, p)| p.visible && p.opacity > 0.0).collect();
    if let Some((i, p)) = fills.last() {
        let f = fill_expr(doc, n, &format!("fills/{i}"), p);
        if shape == "Rectangle()" {
            m.push(format!(".background({f})"));
        } else {
            m.push(format!(".background({shape}.fill({f}))"));
        }
    }
    if let Some(s) = stroke_expr(doc, n) {
        m.push(format!(".overlay({shape}.stroke({s}))"));
    }
    if n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) {
        if shape == "Rectangle()" {
            m.push(".clipped()".into());
        } else {
            m.push(format!(".clipShape({shape})"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fixture;

    #[test]
    fn golden_card() {
        let (d, id) = fixture::card();
        let expected = r####"VStack(alignment: .leading, spacing: 8) {
    Text("Hello world")
        .font(.custom("Inter", size: 20))
        .fontWeight(.semibold)
        .foregroundColor(Color(red: 0.067, green: 0.067, blue: 0.067))
    Text("Body copy")
        .font(.custom("Inter", size: 14))
        .foregroundColor(Color("textMuted"))
        .multilineTextAlignment(.center)
        .lineSpacing(6)
        .frame(maxWidth: .infinity)
    HStack(spacing: Tokens.spacingMd) {
        Spacer()
        Button(label: "Save", disabled: true)
            .frame(width: 80, height: 32)
        Circle()
            .fill(Color(red: 1, green: 0, blue: 0))
            .frame(width: 24, height: 24)
        Spacer()
    }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity)
        .background(RoundedRectangle(cornerRadius: 4).fill(Color("primary")))
}
    .padding(16)
    .frame(width: 312, height: 200, alignment: .topLeading)
    .background(RoundedRectangle(cornerRadius: 8).fill(Color.white))
    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(red: 0.898, green: 0.898, blue: 0.898), lineWidth: 1))
    .clipShape(RoundedRectangle(cornerRadius: 8))
    .shadow(color: Color.black.opacity(0.25), radius: 4, x: 0, y: 4)"####;
        assert_eq!(swiftui_for(&d, id), expected);
    }

    #[test]
    fn stretch_child_keeps_its_cross_size_when_the_parent_hugs() {
        let (d, row, _) = fixture::row_with_stretch_children(Sizing::Hug, &[40.0]);
        let out = swiftui_for(&d, row);
        assert!(out.contains("height: 40") && !out.contains("maxHeight"), "{out}");

        let (d, row, _) = fixture::row_with_stretch_children(Sizing::Fixed, &[40.0]);
        let out = swiftui_for(&d, row);
        assert!(out.contains("maxHeight"), "a fixed parent still stretches the child: {out}");
    }

    #[test]
    fn grid_stretch_child_exports_its_solved_size() {
        // (row track, solved height): an auto row is sized by the child, a taller row is filled.
        for (track, height) in [(None, 20), (Some(GridTrack::Fixed(100.0)), 100)] {
            // Export the grid: a node exported on its own has no flow parent.
            let (d, grid, _) = fixture::grid_stretch_child(track, false);
            let out = swiftui_for(&d, grid);
            assert!(out.contains(&format!("width: 30, height: {height}")), "{track:?}: {out}");
            for banned in ["maxWidth", "maxHeight"] {
                assert!(!out.contains(banned), "{track:?} has {banned}: {out}");
            }
        }
    }
}
