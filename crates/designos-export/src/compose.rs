//! Jetpack Compose output.

use crate::codegen::*;
use crate::slots::*;
use crate::util::num;
use designos_doc::*;

/// A Jetpack Compose composable for `node` and its descendants: `Row`/`Column` with
/// `Arrangement` and `Alignment` for auto layout, `Text`, and a `Modifier` chain of
/// `size`, `shadow`, `clip(RoundedCornerShape)`, `background`, `border` and `padding`.
/// Instances call the component by name with changed properties as named arguments; bound
/// variables become `MaterialTheme.colorScheme.name` / `Tokens.name` placeholders.
pub fn compose_for(doc: &Document, node: NodeId) -> String {
    let mut o = Out::new("    ");
    let slots = slot_infos(doc, node);
    match doc.get(node) {
        Some(n) if !slots.is_empty() => slot_function(doc, n, &slots, &mut o),
        _ => emit(doc, node, Parent::root(), 0, &mut o),
    }
    o.finish()
}

/// A component with slots as a `@Composable` function: one lambda parameter per slot, whose
/// default is the slot's own content.
fn slot_function(doc: &Document, n: &Node, slots: &[SlotInfo], o: &mut Out) {
    o.line(0, "@Composable");
    o.line(0, format!("fun {}(", view_name(doc, n)));
    o.line(1, "modifier: Modifier = Modifier,");
    let mut taken: Vec<String> = Vec::new();
    for p in prop_params(doc, n.id) {
        let name = prop_ident(&p.name, Lang::Kotlin, &mut taken);
        o.line(
            1,
            match &p.default {
                PropDefault::Bool(b) => format!("{name}: Boolean = {b},"),
                PropDefault::Text(t) | PropDefault::Variant { value: t, .. } => format!("{name}: String = {},", quote(t)),
                PropDefault::Instance(Some(v)) => format!("{name}: @Composable () -> Unit = {{ {v}() }},"),
                PropDefault::Instance(None) => format!("{name}: @Composable () -> Unit = {{}},"),
            },
        );
    }
    let single = slots.len() == 1;
    for s in slots {
        let s_id = slot_ident_in(&s.name, single, Lang::Kotlin, &mut taken);
        let head = format!("{s_id}: @Composable () -> Unit = {{");
        let defaults: Vec<&Node> = s.container.and_then(|c| doc.get(c)).map(|c| visible_children(doc, c)).unwrap_or_default();
        if defaults.is_empty() {
            o.line(1, format!("{head}}},"));
            continue;
        }
        o.line(1, head);
        let me = s.container.and_then(|c| doc.get(c)).map(Parent::of).unwrap_or_else(Parent::root);
        for k in defaults {
            emit(doc, k.id, me, 2, o);
        }
        o.line(1, "},");
    }
    o.line(0, ") {");
    o.root_takes_modifier = true;
    emit(doc, n.id, Parent::root(), 1, o);
    o.root_takes_modifier = false;
    o.line(0, "}");
}

fn dp(doc: &Document, n: &Node, key: &str, v: f32) -> String {
    match bound(doc, n, key) {
        Some(var) => token_text(var, CodeSyntaxPlatform::Android, || format!("Tokens.{}", token_camel(var, false))),
        None => format!("{}.dp", num(v)),
    }
}

fn color(doc: &Document, n: &Node, key: &str, c: &Color, opacity: f32) -> String {
    if let Some(v) = bound(doc, n, key) {
        return token_text(v, CodeSyntaxPlatform::Android, || format!("MaterialTheme.colorScheme.{}", token_camel(v, true)));
    }
    let a = ((c.a * opacity).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("Color(0x{a:02X}{})", c.to_hex())
}

fn brush(doc: &Document, n: &Node, p: &Paint) -> Option<String> {
    let cols = |stops: &[GradientStop]| stops.iter().map(|s| color(doc, n, "", &s.color, p.opacity)).collect::<Vec<_>>().join(", ");
    let px = |v: Vec2| format!("Offset({}f, {}f)", num(v.x * n.size.x), num(v.y * n.size.y));
    Some(match &p.kind {
        PaintKind::Solid(_) => return None,
        PaintKind::LinearGradient { handles, stops } => {
            format!("Brush.linearGradient(colors = listOf({}), start = {}, end = {})", cols(stops), px(handles[0]), px(handles[1]))
        }
        PaintKind::RadialGradient { handles, stops } | PaintKind::DiamondGradient { handles, stops } => {
            let r = ((handles[1].x - handles[0].x) * n.size.x).hypot((handles[1].y - handles[0].y) * n.size.y);
            format!("Brush.radialGradient(colors = listOf({}), center = {}, radius = {}f)", cols(stops), px(handles[0]), num(r))
        }
        PaintKind::AngularGradient { handles, stops } => format!("Brush.sweepGradient(colors = listOf({}), center = {})", cols(stops), px(handles[0])),
        PaintKind::Image { .. } => "SolidColor(Color.Gray) /* image fill */".to_string(),
    })
}

fn shape(doc: &Document, n: &Node) -> String {
    if matches!(n.kind, NodeKind::Ellipse(_)) {
        return if (n.size.x - n.size.y).abs() < 0.01 { "CircleShape".into() } else { "RoundedCornerShape(50)".into() };
    }
    let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
    let r = n.corner_radii;
    let any_bound = keys.iter().any(|k| bound(doc, n, k).is_some());
    let uniform = r.iter().all(|v| *v == r[0]) && keys.iter().all(|k| bound(doc, n, k).map(|v| &v.id) == bound(doc, n, keys[0]).map(|v| &v.id));
    if uniform {
        if r[0] == 0.0 && !any_bound {
            "RectangleShape".into()
        } else {
            format!("RoundedCornerShape({})", dp(doc, n, keys[0], r[0]))
        }
    } else {
        format!(
            "RoundedCornerShape(topStart = {}, topEnd = {}, bottomEnd = {}, bottomStart = {})",
            dp(doc, n, keys[0], r[0]),
            dp(doc, n, keys[1], r[1]),
            dp(doc, n, keys[2], r[2]),
            dp(doc, n, keys[3], r[3])
        )
    }
}

fn weight(w: u16) -> &'static str {
    match w {
        0..=149 => "FontWeight.Thin",
        150..=249 => "FontWeight.ExtraLight",
        250..=349 => "FontWeight.Light",
        350..=449 => "FontWeight.Normal",
        450..=549 => "FontWeight.Medium",
        550..=649 => "FontWeight.SemiBold",
        650..=749 => "FontWeight.Bold",
        750..=849 => "FontWeight.ExtraBold",
        _ => "FontWeight.Black",
    }
}

type Arg = Vec<String>;

fn arg(s: impl Into<String>) -> Arg {
    vec![s.into()]
}

/// The `modifier` argument of a call. The exported root of a function with a `modifier`
/// parameter chains onto that parameter, so a caller's modifier styles the root layout.
fn modifier_arg(o: &Out, parent: Parent<'_>, mods: &[String]) -> Option<Arg> {
    let base = if o.root_takes_modifier && parent.root { "modifier" } else { "Modifier" };
    if mods.is_empty() {
        return (base == "modifier").then(|| arg("modifier = modifier"));
    }
    if mods.len() == 1 {
        return Some(arg(format!("modifier = {base}{}", mods[0])));
    }
    let mut v = vec![format!("modifier = {base}")];
    v.extend(mods.iter().map(|m| format!("    {m}")));
    Some(v)
}

/// Writes `Name(args) {` ... or a complete call; returns whether a block was opened.
fn open_call(o: &mut Out, depth: usize, name: &str, args: Vec<Arg>, block: bool) {
    let tail = if block { " {" } else { "" };
    if args.is_empty() {
        o.line(depth, format!("{name}{tail}"));
        return;
    }
    if args.len() == 1 && args[0].len() == 1 {
        o.line(depth, format!("{name}({}){tail}", args[0][0]));
        return;
    }
    o.line(depth, format!("{name}("));
    let last = args.len() - 1;
    for (i, a) in args.iter().enumerate() {
        for (j, l) in a.iter().enumerate() {
            let comma = if i != last && j == a.len() - 1 { "," } else { "" };
            o.line(depth + 1, format!("{l}{comma}"));
        }
    }
    o.line(depth, format!("){tail}"));
}

fn size_mods(doc: &Document, n: &Node, sz: &Sizes, parent: Parent, m: &mut Vec<String>) {
    let parent_row = parent.al.map(|a| a.direction == LayoutDirection::Horizontal).unwrap_or(true);
    let mut weighted = false;
    let mut fills = Vec::new();
    match (sz.w, sz.h) {
        (Dim::Fixed(w), Dim::Fixed(h)) if (w - h).abs() < 1e-3 && bound(doc, n, "width").is_none() && bound(doc, n, "height").is_none() => {
            m.push(format!(".size({})", dp(doc, n, "width", w)))
        }
        (Dim::Fixed(w), Dim::Fixed(h)) => m.push(format!(".size(width = {}, height = {})", dp(doc, n, "width", w), dp(doc, n, "height", h))),
        (w, h) => {
            if let Dim::Fixed(w) = w {
                m.push(format!(".width({})", dp(doc, n, "width", w)));
            }
            if let Dim::Fixed(h) = h {
                m.push(format!(".height({})", dp(doc, n, "height", h)));
            }
        }
    }
    for (is_w, d) in [(true, sz.w), (false, sz.h)] {
        if d == Dim::Fill {
            if sz.in_flow && is_w == parent_row && !weighted {
                weighted = true;
                fills.push(".weight(1f)".to_string());
            } else if is_w {
                fills.push(".fillMaxWidth()".to_string());
            } else {
                fills.push(".fillMaxHeight()".to_string());
            }
        }
    }
    m.extend(fills);
}

fn style_mods(doc: &Document, n: &Node, m: &mut Vec<String>) {
    let shp = shape(doc, n);
    let shadows = drop_shadows(n);
    for (_, _, radius, _) in &shadows {
        m.push(format!(".shadow(elevation = {}.dp, shape = {shp})", num(radius / 2.0)));
    }
    let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
    if clips {
        m.push(format!(".clip({shp})"));
    }
    if let Some((i, p)) = n.fills.iter().enumerate().rev().find(|(_, p)| p.visible && p.opacity > 0.0) {
        let shape_arg = if shp == "RectangleShape" { String::new() } else { format!(", {shp}") };
        match (brush(doc, n, p), &p.kind) {
            (Some(b), _) => m.push(format!(".background(brush = {b}{})", if shape_arg.is_empty() { String::new() } else { format!(", shape = {shp}") })),
            (None, PaintKind::Solid(c)) => m.push(format!(".background({}{shape_arg})", color(doc, n, &format!("fills/{i}"), c, p.opacity))),
            _ => {}
        }
    }
    if let Some(p) = n.stroke.paints.iter().find(|p| p.visible && p.opacity > 0.0).filter(|_| n.stroke.weight > 0.0) {
        let shape_arg = if shp == "RectangleShape" { String::new() } else { format!(", {shp}") };
        match &p.kind {
            PaintKind::Solid(c) => {
                m.push(format!(".border({}, {}{shape_arg})", dp(doc, n, "strokeWeight", n.stroke.weight), color(doc, n, "strokes/0", c, p.opacity)))
            }
            _ => {
                if let Some(b) = brush(doc, n, p) {
                    m.push(format!(".border({}, {b}{shape_arg})", dp(doc, n, "strokeWeight", n.stroke.weight)));
                }
            }
        }
    }
}

fn padding_mods(doc: &Document, n: &Node, al: &AutoLayout, m: &mut Vec<String>) {
    let keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    let p = al.padding;
    let any = |i: usize| p[i] > 0.0 || bound(doc, n, keys[i]).is_some();
    if !(0..4).any(any) {
        return;
    }
    let v = |i: usize| dp(doc, n, keys[i], p[i]);
    let same = |a: usize, b: usize| p[a] == p[b] && bound(doc, n, keys[a]).map(|x| &x.id) == bound(doc, n, keys[b]).map(|x| &x.id);
    if same(0, 1) && same(1, 2) && same(2, 3) {
        m.push(format!(".padding({})", v(0)));
    } else if same(1, 3) && same(0, 2) {
        m.push(format!(".padding(horizontal = {}, vertical = {})", v(1), v(0)));
    } else {
        m.push(format!(".padding(start = {}, top = {}, end = {}, bottom = {})", v(3), v(0), v(1), v(2)));
    }
}

fn text_args(doc: &Document, n: &Node, t: &TextProps, args: &mut Vec<Arg>) {
    let s = &t.style;
    args.push(arg(format!("text = {}", quote(&t.content))));
    if let Some(p) = first_paint(&s.fills) {
        if let PaintKind::Solid(c) = &p.kind {
            args.push(arg(format!("color = {}", color(doc, n, "fills/0", c, p.opacity))));
        }
    }
    args.push(arg(format!(
        "fontSize = {}",
        match bound(doc, n, "fontSize") {
            Some(v) => token_text(v, CodeSyntaxPlatform::Android, || format!("Tokens.{}", token_camel(v, false))),
            None => format!("{}.sp", num(s.font_size)),
        }
    )));
    if s.font_weight != 400 {
        args.push(arg(format!("fontWeight = {}", weight(s.font_weight))));
    }
    if s.italic {
        args.push(arg("fontStyle = FontStyle.Italic"));
    }
    args.push(arg(format!("fontFamily = FontFamily(Font(R.font.{}))", snake(&s.font_family))));
    if s.letter_spacing != 0.0 {
        args.push(arg(format!("letterSpacing = {}.sp", num(s.letter_spacing))));
    }
    match s.line_height {
        LineHeight::Auto => {}
        LineHeight::Pixels(p) => args.push(arg(format!("lineHeight = {}.sp", num(p)))),
        LineHeight::Percent(p) => args.push(arg(format!("lineHeight = {}.sp", num(s.font_size * p / 100.0)))),
    }
    match s.align_h {
        TextAlignH::Left => {}
        TextAlignH::Center => args.push(arg("textAlign = TextAlign.Center")),
        TextAlignH::Right => args.push(arg("textAlign = TextAlign.End")),
        TextAlignH::Justified => args.push(arg("textAlign = TextAlign.Justify")),
    }
    match s.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => args.push(arg("textDecoration = TextDecoration.Underline")),
        TextDecoration::Strikethrough => args.push(arg("textDecoration = TextDecoration.LineThrough")),
    }
}

fn compose_cell_align(ah: GridChildAlign, av: GridChildAlign) -> &'static str {
    match (av, ah) {
        (GridChildAlign::Center, GridChildAlign::Center) => "Alignment.Center",
        (GridChildAlign::Center, GridChildAlign::End) => "Alignment.CenterEnd",
        (GridChildAlign::Center, _) => "Alignment.CenterStart",
        (GridChildAlign::End, GridChildAlign::Center) => "Alignment.BottomCenter",
        (GridChildAlign::End, GridChildAlign::End) => "Alignment.BottomEnd",
        (GridChildAlign::End, _) => "Alignment.BottomStart",
        (_, GridChildAlign::Center) => "Alignment.TopCenter",
        (_, GridChildAlign::End) => "Alignment.TopEnd",
        _ => "Alignment.TopStart",
    }
}

fn emit(doc: &Document, id: NodeId, parent: Parent<'_>, depth: usize, o: &mut Out) {
    emit_with_offset(doc, id, parent, depth, None, o);
}

fn emit_with_offset(doc: &Document, id: NodeId, parent: Parent<'_>, depth: usize, extra_offset: Option<(f32, f32)>, o: &mut Out) {
    let Some(n) = doc.get(id) else { return };
    if !n.visible {
        return;
    }
    let mut sz = sizes(n, parent);
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let site = slot_site(doc, id);
    let grid_opt = own_al.filter(|_| site.is_none()).and_then(|al| solved_grid(doc, n, al).map(|g| (al, g)));
    let kids = if site.is_some() { Vec::new() } else { visible_children(doc, n) };
    let mut mods: Vec<String> = Vec::new();
    if let Some((x, y)) = extra_offset.or_else(|| offset(n, parent, &sz)) {
        if x != 0.0 || y != 0.0 {
            mods.push(format!(".offset(x = {}.dp, y = {}.dp)", num(x), num(y)));
        }
    }
    let rot = rotation_deg(n);

    if let NodeKind::Instance { .. } = n.kind {
        let comp = instance_component(n);
        let slots = instance_slots(doc, n);
        let single = slots.single();
        let mut args: Vec<Arg> = instance_args_for(doc, n, Lang::Kotlin)
            .into_iter()
            .map(|(name, lit)| match lit {
                PropLit::Bool(b) => arg(format!("{name} = {b}")),
                PropLit::Text(t) => arg(format!("{name} = {}", quote(&t))),
                PropLit::Instance(v) => arg(format!("{name} = {{ {v}() }}")),
            })
            .collect();
        size_mods(doc, n, &sz, parent, &mut mods);
        finish_mods(doc, n, rot, &mut mods);
        args.extend(modifier_arg(o, parent, &mods));
        let me = Parent::of(n);
        if single && !slots.fills.is_empty() {
            // The only slot is the trailing content lambda.
            open_call(o, depth, &view_name(doc, n), args, true);
            for k in slots.fills.iter().flat_map(|f| &f.ids) {
                emit(doc, *k, me, depth + 1, o);
            }
            o.line(depth, "}");
        } else {
            for f in &slots.fills {
                let name = slot_ident_for(doc, comp, &f.name, single, Lang::Kotlin);
                if f.ids.is_empty() {
                    args.push(arg(format!("{name} = {{}}")));
                    continue;
                }
                let mut body = o.sub();
                for k in &f.ids {
                    emit(doc, *k, me, 1, &mut body);
                }
                let mut lines = vec![format!("{name} = {{")];
                lines.extend(body.into_lines());
                lines.push("}".to_string());
                args.push(lines);
            }
            open_call(o, depth, &view_name(doc, n), args, false);
        }
        return;
    }
    if is_vector_asset(doc, id) {
        size_mods(doc, n, &sz, parent, &mut mods);
        finish_mods(doc, n, rot, &mut mods);
        let mut args = vec![arg(format!("painter = painterResource(R.drawable.{})", snake(&n.name))), arg("contentDescription = null")];
        args.extend(modifier_arg(o, parent, &mods));
        open_call(o, depth, "Icon", args, false);
        return;
    }
    if let Some(t) = text_of(n) {
        let mut args = Vec::new();
        text_args(doc, n, t, &mut args);
        size_mods(doc, n, &sz, parent, &mut mods);
        finish_mods(doc, n, rot, &mut mods);
        args.extend(modifier_arg(o, parent, &mods));
        open_call(o, depth, "Text", args, false);
        return;
    }
    let mut args: Vec<Arg> = Vec::new();
    let name;
    let block;
    if let Some((al, ref grid)) = grid_opt {
        let mut grid_h = grid.rows.iter().sum::<f32>() + grid.row_gap * (grid.rows.len().saturating_sub(1) as f32) + al.padding[0] + al.padding[2];
        if al.counter_sizing == Sizing::Fixed {
            grid_h = n.size.y;
        }
        sz.h = Dim::Fixed(grid_h);
    }
    size_mods(doc, n, &sz, parent, &mut mods);
    style_mods(doc, n, &mut mods);
    if let Some((al, grid)) = grid_opt {
        let out_of_flow: Vec<&Node> = kids.iter().copied().filter(|k| !grid.children.iter().any(|(cid, _)| *cid == k.id)).collect();
        let has_oof = !out_of_flow.is_empty();
        let g_depth = if has_oof { depth + 1 } else { depth };
        let widths_dp = grid.widths.iter().map(|w| format!("{}.dp", num(*w))).collect::<Vec<_>>().join(", ");
        args.push(arg(format!(
            "columns = object : GridCells {{ override fun Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int): List<Int> = listOf({widths_dp}).map {{ it.roundToPx() }} }}"
        )));
        if has_oof {
            finish_mods(doc, n, rot, &mut mods);
            let box_args: Vec<Arg> = modifier_arg(o, parent, &mods).into_iter().collect();
            open_call(o, depth, "Box", box_args, true);
            let mut grid_mods = vec![".fillMaxSize()".to_string()];
            padding_mods(doc, n, al, &mut grid_mods);
            args.extend(modifier_arg(o, Parent { al: None, root: false }, &grid_mods));
        } else {
            padding_mods(doc, n, al, &mut mods);
            finish_mods(doc, n, rot, &mut mods);
            args.extend(modifier_arg(o, parent, &mods));
        }
        let col_gap_s = dp(doc, n, "itemSpacing", grid.col_gap);
        let row_gap_s = dp(doc, n, grid_row_gap_key(doc, n, al), grid.row_gap);
        args.push(arg(format!("horizontalArrangement = Arrangement.spacedBy({col_gap_s})")));
        args.push(arg(format!("verticalArrangement = Arrangement.spacedBy({row_gap_s})")));
        args.push(arg("userScrollEnabled = false"));
        open_call(o, g_depth, "LazyVerticalGrid", args, true);
        let me = Parent::of(n);
        let cols = grid.widths.len();
        for r in 0..grid.rows.len() {
            let groups = grid.row_cells(r);
            let mut c = 0usize;
            while c < cols {
                if let Some(group) = groups.iter().find(|g| g.placed.col == c) {
                    let cid = group.id;
                    let p = &group.placed;
                    if p.row_span > 1 {
                        let cname = doc.get(cid).map(|cn| cn.name.as_str()).unwrap_or("");
                        o.line(
                            g_depth + 1,
                            format!("// \"{}\": row span {} is not supported by Compose LazyVerticalGrid; placed in row {}", cname, p.row_span, p.row + 1),
                        );
                    }
                    for s in &group.stacked {
                        if s.placed.row_span > 1 {
                            let sname = doc.get(s.id).map(|cn| cn.name.as_str()).unwrap_or("");
                            o.line(
                                g_depth + 1,
                                format!(
                                    "// \"{}\": row span {} is not supported by Compose LazyVerticalGrid; placed in row {}",
                                    sname,
                                    s.placed.row_span,
                                    s.placed.row + 1
                                ),
                            );
                        }
                    }
                    if let Some(comment) = SolvedGrid::overlap_comment(doc, group) {
                        o.line(g_depth + 1, comment);
                    }
                    let item_head = if p.col_span > 1 { format!("item(span = {{ GridItemSpan({}) }}) {{", p.col_span) } else { "item {".to_string() };
                    o.line(g_depth + 1, item_head);
                    let cell_h = grid.cell_height(p, false);
                    let (ah, av) = grid_cell_align(doc, cid, al);
                    let align = compose_cell_align(ah, av);
                    o.line(g_depth + 2, format!("Box(Modifier.height({}.dp), contentAlignment = {align}) {{", num(cell_h)));
                    emit(doc, cid, me, g_depth + 3, o);
                    for s in &group.stacked {
                        emit_with_offset(doc, s.id, me, g_depth + 3, Some(s.offset), o);
                    }
                    o.line(g_depth + 2, "}");
                    o.line(g_depth + 1, "}");
                    c += p.col_span;
                } else {
                    o.line(g_depth + 1, format!("item {{ Spacer(Modifier.height({}.dp)) }}", num(grid.rows[r])));
                    c += 1;
                }
            }
        }
        o.line(g_depth, "}");
        if has_oof {
            for k in out_of_flow {
                emit(doc, k.id, me, depth + 1, o);
            }
            o.line(depth, "}");
        }
        return;
    } else if let Some(al) = own_al.filter(|_| !kids.is_empty() || site.is_some()) {
        let horizontal = al.direction == LayoutDirection::Horizontal;
        name = if horizontal { "Row" } else { "Column" };
        block = true;
        padding_mods(doc, n, al, &mut mods);
        finish_mods(doc, n, rot, &mut mods);
        args.extend(modifier_arg(o, parent, &mods));
        let spacing = (al.item_spacing > 0.0 || bound(doc, n, "itemSpacing").is_some()).then(|| dp(doc, n, "itemSpacing", al.item_spacing));
        let (start, center, end) = if horizontal {
            ("Alignment.Start", "Alignment.CenterHorizontally", "Alignment.End")
        } else {
            ("Alignment.Top", "Alignment.CenterVertically", "Alignment.Bottom")
        };
        let arrangement = match (al.primary_align, spacing) {
            (LayoutAlign::SpaceBetween, _) => Some("Arrangement.SpaceBetween".to_string()),
            (LayoutAlign::Center, Some(s)) => Some(format!("Arrangement.spacedBy({s}, {center})")),
            (LayoutAlign::Center, None) => Some("Arrangement.Center".into()),
            (LayoutAlign::End, Some(s)) => Some(format!("Arrangement.spacedBy({s}, {end})")),
            (LayoutAlign::End, None) => Some("Arrangement.End".into()),
            (_, Some(s)) => Some(format!("Arrangement.spacedBy({s})")),
            (_, None) => None,
        };
        let _ = start;
        if let Some(a) = arrangement {
            args.push(arg(format!("{} = {a}", if horizontal { "horizontalArrangement" } else { "verticalArrangement" })));
        }
        let cross = match (al.counter_align, horizontal) {
            (LayoutAlign::Center, true) => "Alignment.CenterVertically",
            (LayoutAlign::Center, false) => "Alignment.CenterHorizontally",
            (LayoutAlign::End, true) => "Alignment.Bottom",
            (LayoutAlign::End, false) => "Alignment.End",
            (_, true) => "Alignment.Top",
            (_, false) => "Alignment.Start",
        };
        args.push(arg(format!("{} = {cross}", if horizontal { "verticalAlignment" } else { "horizontalAlignment" })));
    } else {
        name = "Box";
        block = !kids.is_empty() || site.is_some();
        finish_mods(doc, n, rot, &mut mods);
        args.extend(modifier_arg(o, parent, &mods));
    }
    open_call(o, depth, name, args, block);
    if block {
        if let Some(site) = &site {
            o.line(depth + 1, format!("{}()", slot_ident_for(doc, site.comp, &site.name, site.single, Lang::Kotlin)));
        }
        let me = Parent::of(n);
        for k in kids {
            emit(doc, k.id, me, depth + 1, o);
        }
        o.line(depth, "}");
    }
}

fn finish_mods(doc: &Document, n: &Node, rot: f32, mods: &mut Vec<String>) {
    match bound(doc, n, "opacity") {
        Some(v) => mods.push(format!(".alpha({} / 100f)", token_text(v, CodeSyntaxPlatform::Android, || format!("Tokens.{}", token_camel(v, false))))),
        None if n.opacity < 0.999 => mods.push(format!(".alpha({}f)", num((n.opacity * 100.0).round() / 100.0))),
        None => {}
    }
    if rot != 0.0 {
        mods.push(format!(".rotate({}f)", num(rot)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fixture;

    #[test]
    fn golden_card() {
        let (d, id) = fixture::card();
        let expected = r####"Column(
    modifier = Modifier
        .size(width = 312.dp, height = 200.dp)
        .shadow(elevation = 4.dp, shape = RoundedCornerShape(8.dp))
        .clip(RoundedCornerShape(8.dp))
        .background(Color(0xFFFFFFFF), RoundedCornerShape(8.dp))
        .border(1.dp, Color(0xFFE5E5E5), RoundedCornerShape(8.dp))
        .padding(16.dp),
    verticalArrangement = Arrangement.spacedBy(8.dp),
    horizontalAlignment = Alignment.Start
) {
    Text(
        text = "Hello world",
        color = Color(0xFF111111),
        fontSize = 20.sp,
        fontWeight = FontWeight.SemiBold,
        fontFamily = FontFamily(Font(R.font.inter))
    )
    Text(
        text = "Body copy",
        color = MaterialTheme.colorScheme.textMuted,
        fontSize = 14.sp,
        fontFamily = FontFamily(Font(R.font.inter)),
        lineHeight = 20.sp,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth()
    )
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(4.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(Tokens.spacingMd, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Button(
            label = "Save",
            disabled = true,
            modifier = Modifier.size(width = 80.dp, height = 32.dp)
        )
        Box(
            modifier = Modifier
                .size(24.dp)
                .background(Color(0xFFFF0000), CircleShape)
        )
    }
}"####;
        assert_eq!(compose_for(&d, id), expected);
    }

    #[test]
    fn stretch_child_keeps_its_cross_size_when_the_parent_hugs() {
        let (d, row, _) = fixture::row_with_stretch_children(Sizing::Hug, &[40.0]);
        let out = compose_for(&d, row);
        assert!(out.contains("height = 40.dp") && !out.contains("fillMaxHeight"), "{out}");

        let (d, row, _) = fixture::row_with_stretch_children(Sizing::Fixed, &[40.0]);
        let out = compose_for(&d, row);
        assert!(out.contains("fillMaxHeight"), "a fixed parent still stretches the child: {out}");
    }

    #[test]
    fn compose_hugging_row_all_fill_children() {
        let (d, row, _) = fixture::row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, false);
        let out = compose_for(&d, row);
        assert_eq!(out.matches("height = 100.dp").count(), 2, "{out}");
        assert!(!out.contains("fillMaxHeight"), "{out}");
    }

    #[test]
    fn grid_stretch_child_exports_its_solved_size() {
        // (row track, solved height): an auto row is sized by the child, a taller row is filled.
        for (track, height) in [(None, 20), (Some(GridTrack::Fixed(100.0)), 100)] {
            // Export the grid: a node exported on its own has no flow parent.
            let (d, grid, _) = fixture::grid_stretch_child(track, false);
            let out = compose_for(&d, grid);
            assert!(out.contains(&format!("width = 30.dp, height = {height}.dp")), "{track:?}: {out}");
            for banned in ["fillMax", "weight"] {
                assert!(!out.contains(banned), "{track:?} has {banned}: {out}");
            }
        }
    }

    #[test]
    fn compose_grid_fill_child_exports_solved_height() {
        for (track, height) in [(None, 20), (Some(GridTrack::Fixed(100.0)), 100)] {
            for (d, grid, _) in [fixture::grid_fill_child(track), fixture::grid_stretch_child(track, true), fixture::grid_text_fill_child(track)] {
                let out = compose_for(&d, grid);
                assert!(out.contains(&format!("width = 30.dp, height = {height}.dp")), "{track:?}: {out}");
                for banned in ["fillMax", "weight"] {
                    assert!(!out.contains(banned), "{track:?} has {banned}: {out}");
                }
            }
        }
        // A real Hug text child (vertical != Fill, align_self != Stretch) stays on the Hug path.
        let (mut d, grid, tid) = fixture::grid_text_fill_child(Some(GridTrack::Fixed(100.0)));
        d.get_mut(tid).unwrap().layout_child.vertical = Sizing::Fixed;
        d.relayout_subtree(grid);
        let out = compose_for(&d, grid);
        assert!(out.contains(".width(30.dp)"), "real Hug text uses width only: {out}");
    }

    #[test]
    fn grid_emits_lazy_vertical_grid_tracks_explicit_height_and_holes() {
        let al = AutoLayout {
            grid_columns: 2,
            item_spacing: 10.0,
            counter_spacing: 8.0,
            padding: [12.0, 12.0, 12.0, 12.0],
            grid_col_tracks: vec![GridTrack::Fixed(100.0), GridTrack::Fixed(120.0)].into(),
            grid_row_tracks: vec![GridTrack::Fixed(40.0), GridTrack::Fixed(50.0)].into(),
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            ..Default::default()
        };
        let (mut d, grid) = fixture::grid_frame(al, Vec2::new(254.0, 122.0));
        let a = fixture::add_rect(&mut d, grid, Vec2::new(100.0, 40.0));
        d.get_mut(a).unwrap().name = "Hero".into();
        d.get_mut(a).unwrap().layout_child.row_span = 2;
        let b = fixture::add_rect(&mut d, grid, Vec2::new(120.0, 40.0));
        d.get_mut(b).unwrap().name = "Side".into();
        d.relayout_subtree(grid);

        let out = compose_for(&d, grid);
        assert!(out.contains("LazyVerticalGrid("), "{out}");
        assert!(out.contains("listOf(100.dp, 120.dp).map { it.roundToPx() }"), "{out}");
        assert!(out.contains(".height(122.dp)") || out.contains("height = 122.dp"), "{out}");
        assert!(out.contains("// \"Hero\": row span 2 is not supported by Compose LazyVerticalGrid; placed in row 1"), "{out}");
        assert!(out.contains("item { Spacer(Modifier.height(50.dp)) }"), "{out}");
    }
}
