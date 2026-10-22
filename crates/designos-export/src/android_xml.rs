//! Android View XML output.

use crate::codegen::*;
use crate::util::{esc, num};
use designos_doc::*;
use std::collections::HashSet;

/// An Android View layout for `node` and its descendants, followed by the `<shape>` drawables the
/// layout refers to, each under a `<!-- res/... -->` file header. Auto layout becomes a
/// `LinearLayout` (orientation, gravity, padding, item spacing as child margins), text becomes a
/// `TextView`, other frames a `FrameLayout` with margin offsets, and shapes `View`s. A fill alone
/// is an inline `android:background` colour; corners, a border, an oval or a gradient go into a
/// shape drawable. Instances become `<include>` of the component's layout; bound variables become
/// `@color/name` / `@dimen/name`. Text is escaped for an XML attribute and for Android resources.
pub fn android_xml_for(doc: &Document, node: NodeId) -> String {
    let mut cx = Cx { doc, out: Out::new("    "), drawables: Vec::new(), used: HashSet::new() };
    cx.emit(node, Parent::root(), 0, None);
    let mut text = format!("<!-- res/layout/{}.xml -->\n{}", snake(&doc.get(node).map(|n| n.name.clone()).unwrap_or_default()), cx.out.finish());
    for (name, body) in &cx.drawables {
        text.push_str(&format!("\n\n<!-- res/drawable/{name}.xml -->\n{}", body.join("\n")));
    }
    text
}

/// Where a view sits: how its parent arranges it.
#[derive(Clone)]
struct Slot {
    horizontal: bool,
    /// Margin before this child along the main axis (item spacing); `None` for the first child
    /// or when there is no spacing.
    gap: Option<String>,
    linear: bool,
}

struct Cx<'a> {
    doc: &'a Document,
    out: Out,
    /// (file name, lines) of generated shape drawables.
    drawables: Vec<(String, Vec<String>)>,
    used: HashSet<String>,
}

/// An attribute value for a literal Android string: Android resource escapes (`\n`, `\'`, a leading
/// `@` or `?`), then XML attribute escapes.
fn android_text(s: &str) -> String {
    let mut o = String::new();
    for (i, c) in s.chars().enumerate() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\'' => o.push_str("\\'"),
            '@' | '?' if i == 0 => {
                o.push('\\');
                o.push(c);
            }
            c => o.push(c),
        }
    }
    esc(&o)
}

/// A user-supplied code-syntax string as an XML attribute value: the shared XML escapes, plus line
/// breaks and tabs as character references so the attribute stays on one line.
fn attr_text(s: &str) -> String {
    esc(s).replace('\n', "&#10;").replace('\t', "&#9;")
}

fn argb(c: &Color, opacity: f32) -> String {
    let a = ((c.a * opacity).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{a:02X}{}", c.to_hex())
}

fn token(v: &Variable) -> String {
    snake(&v.name)
}

fn color(doc: &Document, n: &Node, key: &str, c: &Color, opacity: f32) -> String {
    match bound(doc, n, key) {
        Some(v) => attr_text(&token_text(v, CodeSyntaxPlatform::Android, || format!("@color/{}", token(v)))),
        None => argb(c, opacity),
    }
}

fn dp(doc: &Document, n: &Node, key: &str, v: f32) -> String {
    match bound(doc, n, key) {
        Some(var) => attr_text(&token_text(var, CodeSyntaxPlatform::Android, || format!("@dimen/{}", token(var)))),
        None => format!("{}dp", num(v)),
    }
}

fn sp(doc: &Document, n: &Node, key: &str, v: f32) -> String {
    match bound(doc, n, key) {
        Some(var) => attr_text(&token_text(var, CodeSyntaxPlatform::Android, || format!("@dimen/{}", token(var)))),
        None => format!("{}sp", num(v)),
    }
}

fn gravity_word(a: LayoutAlign, horizontal: bool) -> &'static str {
    match (a, horizontal) {
        (LayoutAlign::Center, true) => "center_horizontal",
        (LayoutAlign::Center, false) => "center_vertical",
        (LayoutAlign::End, true) => "end",
        (LayoutAlign::End, false) => "bottom",
        (_, true) => "start",
        (_, false) => "top",
    }
}

type Attrs = Vec<(String, String)>;

fn attr(a: &mut Attrs, name: &str, value: impl Into<String>) {
    a.push((format!("android:{name}"), value.into()));
}

impl Cx<'_> {
    /// Writes `<tag` with one attribute per line, closed by `>` or, for `self_closing`, ` />`.
    fn open(&mut self, depth: usize, tag: &str, attrs: &Attrs, self_closing: bool) {
        let tail = if self_closing { " />" } else { ">" };
        if attrs.is_empty() {
            self.out.line(depth, format!("<{tag}{tail}"));
            return;
        }
        self.out.line(depth, format!("<{tag}"));
        let last = attrs.len() - 1;
        for (i, (k, v)) in attrs.iter().enumerate() {
            self.out.line(depth + 1, format!("{k}=\"{v}\"{}", if i == last { tail } else { "" }));
        }
    }

    fn drawable_name(&mut self, node_name: &str) -> String {
        let base = format!("{}_background", snake(node_name));
        let mut name = base.clone();
        let mut k = 2;
        while !self.used.insert(name.clone()) {
            name = format!("{base}_{k}");
            k += 1;
        }
        name
    }

    /// `android:background` (inline colour or a new shape drawable), `elevation`, `alpha`, `rotation`.
    fn style(&mut self, n: &Node, attrs: &mut Attrs) {
        let doc = self.doc;
        let fill = if text_of(n).is_some() { None } else { n.fills.iter().enumerate().rev().find(|(_, p)| p.visible && p.opacity > 0.0) };
        let stroke = n.stroke.paints.iter().find(|p| p.visible && p.opacity > 0.0).filter(|_| n.stroke.weight > 0.0 && !matches!(n.kind, NodeKind::Line));
        let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let oval = matches!(n.kind, NodeKind::Ellipse(_));
        let rounded = n.corner_radii.iter().any(|r| *r > 0.0) || keys.iter().any(|k| bound(doc, n, k).is_some());
        let gradient = fill.is_some_and(|(_, p)| !matches!(p.kind, PaintKind::Solid(_)));
        if matches!(n.kind, NodeKind::Line) {
            if let Some(p) = first_paint(&n.stroke.paints) {
                if let PaintKind::Solid(c) = &p.kind {
                    attr(attrs, "background", color(doc, n, "strokes/0", c, p.opacity));
                }
            }
        } else if stroke.is_some() || oval || rounded || gradient {
            let name = self.drawable_name(&n.name);
            let body = self.shape(n, fill, stroke, oval);
            self.drawables.push((name.clone(), body));
            attr(attrs, "background", format!("@drawable/{name}"));
        } else if let Some((i, p)) = fill {
            if let PaintKind::Solid(c) = &p.kind {
                attr(attrs, "background", color(doc, n, &format!("fills/{i}"), c, p.opacity));
            }
        }
        if n.kind.frame_props().is_some_and(|f| f.clips_content) && !n.children.is_empty() {
            attr(attrs, "clipChildren", "true");
            if rounded || oval {
                attr(attrs, "clipToOutline", "true");
            }
        }
        if let Some((_, _, radius, _)) = drop_shadows(n).first() {
            attr(attrs, "elevation", format!("{}dp", num(radius / 2.0)));
        }
        match bound(doc, n, "opacity") {
            Some(v) => attr(attrs, "alpha", attr_text(&token_text(v, CodeSyntaxPlatform::Android, || format!("@dimen/{}", token(v))))),
            None if n.opacity < 0.999 => attr(attrs, "alpha", num((n.opacity * 100.0).round() / 100.0)),
            None => {}
        }
        let rot = rotation_deg(n);
        if rot != 0.0 {
            attr(attrs, "rotation", num(rot));
        }
    }

    fn shape(&self, n: &Node, fill: Option<(usize, &Paint)>, stroke: Option<&Paint>, oval: bool) -> Vec<String> {
        let doc = self.doc;
        let mut l = vec![
            "<shape xmlns:android=\"http://schemas.android.com/apk/res/android\"".to_string(),
            format!("    android:shape=\"{}\">", if oval { "oval" } else { "rectangle" }),
        ];
        if let Some((i, p)) = fill {
            match &p.kind {
                PaintKind::Solid(c) => l.push(format!("    <solid android:color=\"{}\" />", color(doc, n, &format!("fills/{i}"), c, p.opacity))),
                PaintKind::LinearGradient { handles, stops } => {
                    let (dx, dy) = ((handles[1].x - handles[0].x) * n.size.x, (handles[1].y - handles[0].y) * n.size.y);
                    // Android angles run counter-clockwise from "left to right" in steps of 45 degrees
                    let angle = ((-dy.atan2(dx).to_degrees() / 45.0).round() as i32 * 45).rem_euclid(360);
                    l.extend(gradient_lines(doc, n, p, stops, &format!("android:type=\"linear\" android:angle=\"{angle}\"")));
                }
                PaintKind::RadialGradient { handles, stops } | PaintKind::DiamondGradient { handles, stops } => {
                    let r = ((handles[1].x - handles[0].x) * n.size.x).hypot((handles[1].y - handles[0].y) * n.size.y);
                    l.extend(gradient_lines(doc, n, p, stops, &format!("android:type=\"radial\" android:gradientRadius=\"{}dp\"", num(r))));
                }
                PaintKind::AngularGradient { stops, .. } => l.extend(gradient_lines(doc, n, p, stops, "android:type=\"sweep\"")),
                PaintKind::Image { .. } => l.push("    <solid android:color=\"#FF808080\" /> <!-- image fill -->".to_string()),
            }
        }
        if let Some(p) = stroke {
            let c = match &p.kind {
                PaintKind::Solid(c) => color(doc, n, "strokes/0", c, p.opacity),
                _ => "#FF808080".to_string(),
            };
            let dash = n.stroke.dashes.iter().copied().find(|d| *d > 0.0);
            let dashes = dash
                .map(|d| format!(" android:dashWidth=\"{}dp\" android:dashGap=\"{}dp\"", num(d), num(n.stroke.dashes.get(1).copied().unwrap_or(d))))
                .unwrap_or_default();
            l.push(format!("    <stroke android:width=\"{}\" android:color=\"{c}\"{dashes} />", dp(doc, n, "strokeWeight", n.stroke.weight)));
        }
        let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let r = n.corner_radii;
        let any = r.iter().any(|x| *x > 0.0) || keys.iter().any(|k| bound(doc, n, k).is_some());
        if any && !oval {
            let uniform = r.iter().all(|x| *x == r[0]) && keys.iter().all(|k| bound(doc, n, k).map(|v| &v.id) == bound(doc, n, keys[0]).map(|v| &v.id));
            if uniform {
                l.push(format!("    <corners android:radius=\"{}\" />", dp(doc, n, keys[0], r[0])));
            } else {
                l.push(format!(
                    "    <corners android:topLeftRadius=\"{}\" android:topRightRadius=\"{}\" android:bottomRightRadius=\"{}\" android:bottomLeftRadius=\"{}\" />",
                    dp(doc, n, keys[0], r[0]),
                    dp(doc, n, keys[1], r[1]),
                    dp(doc, n, keys[2], r[2]),
                    dp(doc, n, keys[3], r[3])
                ));
            }
        }
        l.push("</shape>".into());
        l
    }
}

/// `<gradient>` with start, optional centre and end colours (Android shapes take at most three stops).
fn gradient_lines(doc: &Document, n: &Node, p: &Paint, stops: &[GradientStop], kind: &str) -> Vec<String> {
    let col = |s: &GradientStop| color(doc, n, "", &s.color, p.opacity);
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Vec::new() };
    let mut l = vec![format!("    <gradient {kind}")];
    l.push(format!("        android:startColor=\"{}\"", col(first)));
    if stops.len() > 2 {
        l.push(format!("        android:centerColor=\"{}\"", col(&stops[stops.len() / 2])));
    }
    l.push(format!("        android:endColor=\"{}\" />", col(last)));
    if stops.len() > 3 {
        l.push("    <!-- more than three stops: start, middle and end are kept -->".into());
    }
    l
}

impl Cx<'_> {
    /// Width and height, plus weight / margins for a child of a `LinearLayout`.
    fn size_attrs(&self, n: &Node, sz: &Sizes, slot: Option<Slot>, attrs: &mut Attrs) {
        let doc = self.doc;
        let linear = slot.as_ref().is_some_and(|s| s.linear);
        let main_horizontal = slot.as_ref().is_some_and(|s| s.horizontal);
        let mut weighted = false;
        let mut dim = |axis_horizontal: bool, d: Dim| -> String {
            match d {
                Dim::Fixed(px) => dp(doc, n, if axis_horizontal { "width" } else { "height" }, px),
                Dim::Hug => "wrap_content".into(),
                Dim::Fill if linear && axis_horizontal == main_horizontal => {
                    weighted = true;
                    "0dp".into()
                }
                Dim::Fill => "match_parent".into(),
            }
        };
        let (w, h) = (dim(true, sz.w), dim(false, sz.h));
        attr(attrs, "layout_width", w);
        attr(attrs, "layout_height", h);
        if weighted {
            attr(attrs, "layout_weight", "1");
        }
        if let Some(Slot { gap: Some(gap), horizontal, linear: true }) = slot {
            attr(attrs, if horizontal { "layout_marginStart" } else { "layout_marginTop" }, gap);
        }
    }

    fn emit(&mut self, id: NodeId, parent: Parent<'_>, depth: usize, slot: Option<Slot>) {
        let doc = self.doc;
        let Some(n) = doc.get(id) else { return };
        if !n.visible {
            return;
        }
        let sz = sizes(n, parent);
        let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
        let kids = visible_children(doc, n);
        let mut attrs: Attrs = Vec::new();
        if depth == 0 {
            attrs.push(("xmlns:android".into(), "http://schemas.android.com/apk/res/android".into()));
        }
        self.size_attrs(n, &sz, slot.clone(), &mut attrs);
        // FrameLayout children placed by position
        if let (Some(s), Some((x, y))) = (&slot, offset(n, parent, &sz)) {
            if !s.linear {
                if x != 0.0 {
                    attr(&mut attrs, "layout_marginStart", format!("{}dp", num(x)));
                }
                if y != 0.0 {
                    attr(&mut attrs, "layout_marginTop", format!("{}dp", num(y)));
                }
            }
        }

        if let NodeKind::Instance { .. } = n.kind {
            let args: Vec<String> = instance_args(doc, n).into_iter().map(|(name, _)| name).collect();
            if !args.is_empty() {
                self.out.line(depth, format!("<!-- component properties set in code: {} -->", args.join(", ")));
            }
            let mut a: Attrs = vec![("layout".into(), format!("@layout/{}", snake(&view_name(doc, n))))];
            a.extend(attrs);
            self.open(depth, "include", &a, true);
        } else if is_vector_asset(doc, id) {
            attr(&mut attrs, "src", format!("@drawable/{}", snake(&n.name)));
            attr(&mut attrs, "importantForAccessibility", "no");
            self.style(n, &mut attrs);
            self.open(depth, "ImageView", &attrs, true);
        } else if let Some(t) = text_of(n) {
            self.text_attrs(n, t, &mut attrs);
            self.style(n, &mut attrs);
            self.open(depth, "TextView", &attrs, true);
        } else if let Some(al) = own_al.filter(|_| !kids.is_empty()) {
            let horizontal = al.direction == LayoutDirection::Horizontal;
            attr(&mut attrs, "orientation", if horizontal { "horizontal" } else { "vertical" });
            let (primary, counter) = (gravity_word(al.primary_align, horizontal), gravity_word(al.counter_align, !horizontal));
            let defaults = if horizontal { ("start", "top") } else { ("top", "start") };
            let (hp, vp) = if horizontal { (primary, counter) } else { (counter, primary) };
            let (dh, dv) = (if horizontal { defaults.0 } else { defaults.1 }, if horizontal { defaults.1 } else { defaults.0 });
            let mut gravity: Vec<&str> = Vec::new();
            if vp != dv {
                gravity.push(vp);
            }
            if hp != dh {
                gravity.push(hp);
            }
            if gravity == ["center_vertical", "center_horizontal"] {
                attr(&mut attrs, "gravity", "center");
            } else if !gravity.is_empty() {
                attr(&mut attrs, "gravity", gravity.join("|"));
            }
            self.padding(n, al, &mut attrs);
            self.style(n, &mut attrs);
            if al.wrap {
                self.out.line(depth, "<!-- auto layout wraps: use a FlexboxLayout -->");
            }
            self.open(depth, "LinearLayout", &attrs, false);
            let between = al.primary_align == LayoutAlign::SpaceBetween;
            let spaced = !between && (al.item_spacing > 0.0 || bound(doc, n, "itemSpacing").is_some());
            let gap = spaced.then(|| dp(doc, n, "itemSpacing", al.item_spacing));
            let me = Parent::of(n);
            for (i, k) in kids.iter().enumerate() {
                if i > 0 && between {
                    let mut s: Attrs = Vec::new();
                    attr(&mut s, "layout_width", if horizontal { "0dp" } else { "wrap_content" });
                    attr(&mut s, "layout_height", if horizontal { "wrap_content" } else { "0dp" });
                    attr(&mut s, "layout_weight", "1");
                    self.open(depth + 1, "Space", &s, true);
                }
                let slot = Slot { horizontal, gap: gap.clone().filter(|_| i > 0), linear: true };
                self.emit(k.id, me, depth + 1, Some(slot));
            }
            self.out.line(depth, "</LinearLayout>");
        } else if n.kind.is_container() && !kids.is_empty() {
            self.style(n, &mut attrs);
            self.open(depth, "FrameLayout", &attrs, false);
            let me = Parent::of(n);
            for k in &kids {
                self.emit(k.id, me, depth + 1, Some(Slot { horizontal: true, gap: None, linear: false }));
            }
            self.out.line(depth, "</FrameLayout>");
        } else {
            self.style(n, &mut attrs);
            self.open(depth, "View", &attrs, true);
        }
    }

    fn padding(&self, n: &Node, al: &AutoLayout, attrs: &mut Attrs) {
        let doc = self.doc;
        let keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
        let p = al.padding;
        let bound_any = keys.iter().any(|k| bound(doc, n, k).is_some());
        if !bound_any && p.iter().all(|v| *v == p[0]) {
            if p[0] > 0.0 {
                attr(attrs, "padding", format!("{}dp", num(p[0])));
            }
            return;
        }
        for (i, name) in [(0, "paddingTop"), (1, "paddingEnd"), (2, "paddingBottom"), (3, "paddingStart")] {
            if p[i] > 0.0 || bound(doc, n, keys[i]).is_some() {
                attr(attrs, name, dp(doc, n, keys[i], p[i]));
            }
        }
    }

    fn text_attrs(&mut self, n: &Node, t: &TextProps, attrs: &mut Attrs) {
        let doc = self.doc;
        let s = &t.style;
        attr(attrs, "text", android_text(&t.content));
        attr(attrs, "textSize", sp(doc, n, "fontSize", s.font_size));
        if let Some(p) = first_paint(&s.fills) {
            if let PaintKind::Solid(c) = &p.kind {
                attr(attrs, "textColor", color(doc, n, "fills/0", c, p.opacity));
            }
        }
        attr(attrs, "fontFamily", format!("@font/{}", snake(&s.font_family)));
        if s.font_weight != 400 {
            attr(attrs, "fontWeight", s.font_weight.to_string());
        }
        if s.italic {
            attr(attrs, "textStyle", "italic");
        }
        match s.align_h {
            TextAlignH::Left | TextAlignH::Justified => {}
            TextAlignH::Center => attr(attrs, "textAlignment", "center"),
            TextAlignH::Right => attr(attrs, "textAlignment", "viewEnd"),
        }
        if s.case == TextCase::Upper {
            attr(attrs, "textAllCaps", "true");
        }
        if s.letter_spacing != 0.0 && s.font_size > 0.0 {
            attr(attrs, "letterSpacing", num(s.letter_spacing / s.font_size));
        }
        let line_height = match s.line_height {
            LineHeight::Auto => None,
            LineHeight::Pixels(p) => Some(p),
            LineHeight::Percent(p) => Some(s.font_size * p / 100.0),
        };
        if let Some(h) = line_height.filter(|h| *h > s.font_size) {
            attr(attrs, "lineSpacingExtra", format!("{}sp", num(h - s.font_size)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fixture;

    /// A vertical auto-layout card with padding, a fill, a stroke and a radius, holding a title
    /// text and a red rectangle.
    fn card_fixture() -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 200.0, 120.0));
        card.name = "Card".into();
        card.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                item_spacing: 8.0,
                padding: [16.0, 16.0, 16.0, 16.0],
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            clips_content: true,
            ..Default::default()
        });
        card.fills = vec![Paint::solid(Color::WHITE)];
        card.corner_radii = [8.0; 4];
        card.stroke.weight = 1.0;
        card.stroke.paints = vec![Paint::solid(Color::rgb(0xE5, 0xE5, 0xE5))];
        let card_id = d.insert(page, card).unwrap();

        let title_id = d.alloc_id();
        let style = TextStyle {
            font_family: "Inter".into(),
            font_size: 20.0,
            font_weight: 600,
            fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))],
            ..Default::default()
        };
        let mut title = Node::text(title_id, Rect::new(0.0, 0.0, 120.0, 24.0), "Hello world", style);
        title.name = "Title".into();
        d.insert(card_id, title).unwrap();

        let chip_id = d.alloc_id();
        let mut chip = Node::rectangle(chip_id, Rect::new(0.0, 0.0, 40.0, 24.0));
        chip.name = "Chip".into();
        chip.fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
        d.insert(card_id, chip).unwrap();
        (d, card_id)
    }

    #[test]
    fn golden_card() {
        let (d, id) = card_fixture();
        let expected = r####"<!-- res/layout/card.xml -->
<LinearLayout
    xmlns:android="http://schemas.android.com/apk/res/android"
    android:layout_width="200dp"
    android:layout_height="120dp"
    android:orientation="vertical"
    android:padding="16dp"
    android:background="@drawable/card_background"
    android:clipChildren="true"
    android:clipToOutline="true">
    <TextView
        android:layout_width="wrap_content"
        android:layout_height="wrap_content"
        android:text="Hello world"
        android:textSize="20sp"
        android:textColor="#FF111111"
        android:fontFamily="@font/inter"
        android:fontWeight="600" />
    <View
        android:layout_width="40dp"
        android:layout_height="24dp"
        android:layout_marginTop="8dp"
        android:background="#FFFF0000" />
</LinearLayout>

<!-- res/drawable/card_background.xml -->
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="#FFFFFFFF" />
    <stroke android:width="1dp" android:color="#FFE5E5E5" />
    <corners android:radius="8dp" />
</shape>"####;
        assert_eq!(android_xml_for(&d, id), expected);
    }

    #[test]
    fn text_is_escaped_for_xml_and_android_resources() {
        let (mut d, card) = card_fixture();
        let title = d.get(card).unwrap().children[0];
        if let NodeKind::Text(t) = &mut d.get_mut(title).unwrap().kind {
            t.content = "@home <b>\"Tom\" & 'Jerry'</b>\\ line\nnext".into();
        }
        let xml = android_xml_for(&d, card);
        assert!(xml.contains(r#"android:text="\@home &lt;b&gt;&quot;Tom&quot; &amp; \&apos;Jerry\&apos;&lt;/b&gt;\\ line\nnext""#), "{xml}");
        assert_eq!(xml.matches("<TextView").count(), 1);
    }

    #[test]
    fn variables_spacing_and_shared_fixture_render() {
        let (d, id) = fixture::card();
        let xml = android_xml_for(&d, id);
        assert!(xml.contains("android:textColor=\"@color/color_text_muted\""), "{xml}");
        assert!(xml.contains("android:background=\"@drawable/actions_background\""), "{xml}");
        assert!(xml.contains("android:layout_marginStart=\"@dimen/spacing_md\""), "{xml}");
        assert!(xml.contains("android:gravity=\"center\""), "{xml}");
        assert!(xml.contains("<include"), "{xml}");
        assert!(xml.contains("android:layout=\"@layout/button\"") || xml.contains("layout=\"@layout/button\""), "{xml}");
        assert!(xml.contains("<!-- component properties set in code: label, disabled -->"), "{xml}");
        assert!(xml.contains("<!-- res/drawable/actions_background.xml -->"), "{xml}");
        // well-formed nesting: every opened container is closed
        assert_eq!(xml.matches("<LinearLayout").count(), xml.matches("</LinearLayout>").count(), "{xml}");
    }

    #[test]
    fn android_xml_hugging_row_all_fill_children() {
        let (d, row, _) = fixture::row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, false);
        let xml = android_xml_for(&d, row);
        assert_eq!(xml.matches("android:layout_height=\"100dp\"").count(), 2, "{xml}");
        assert!(!xml.contains("android:layout_height=\"match_parent\""), "{xml}");
    }

    #[test]
    fn android_xml_grid_fill_child_exports_solved_height() {
        for (track, height) in [(None, 20), (Some(GridTrack::Fixed(100.0)), 100)] {
            let (d, grid, _) = fixture::grid_fill_child(track);
            let xml = android_xml_for(&d, grid);
            assert!(xml.contains(&format!("android:layout_height=\"{height}dp\"")), "{track:?}: {xml}");
            assert!(!xml.contains("android:layout_height=\"match_parent\""), "{track:?}: {xml}");
        }
    }
}
