//! Swift UIKit output.

use crate::codegen::*;
use crate::swiftui::weight_name;
use crate::util::num;
use designos_doc::*;
use std::collections::HashSet;

/// UIKit code that builds the view tree for `node` and its descendants, one statement per line.
/// Auto layout becomes a `UIStackView` (axis, spacing, alignment, layout margins), text becomes a
/// `UILabel`, shapes become `UIView`s with `backgroundColor`, `layer.cornerRadius` and
/// `layer.borderWidth`; shadows, opacity and rotation are set on the layer or view. Size is
/// expressed as Auto Layout constraints inside stacks and as `frame` elsewhere. Instances call the
/// component class by name and assign changed component properties; bound variables become
/// `UIColor(named:)` / `Tokens.name`.
pub fn uikit_for(doc: &Document, node: NodeId) -> String {
    let mut cx = Cx { doc, out: Out::new("    "), used: HashSet::new() };
    cx.emit(node, Parent::root(), None);
    cx.out.finish()
}

/// How a view is attached to its parent.
#[derive(Clone, Copy)]
enum Host<'a> {
    Stack(&'a str),
    Plain(&'a str),
}

struct Cx<'a> {
    doc: &'a Document,
    out: Out,
    used: HashSet<String>,
}

const SWIFT_KEYWORDS: &[&str] = &[
    "class",
    "struct",
    "enum",
    "func",
    "let",
    "var",
    "if",
    "else",
    "for",
    "while",
    "switch",
    "case",
    "default",
    "return",
    "import",
    "in",
    "is",
    "as",
    "self",
    "super",
    "init",
    "nil",
    "true",
    "false",
    "extension",
    "protocol",
    "static",
    "subscript",
    "where",
    "do",
    "try",
    "throw",
    "repeat",
    "break",
    "continue",
    "guard",
    "defer",
    "operator",
    "private",
    "public",
    "internal",
    "open",
    "typealias",
    "associatedtype",
    "inout",
    "some",
    "any",
];

fn ui_color(doc: &Document, n: &Node, key: &str, c: &Color, opacity: f32) -> String {
    let a = (c.a * opacity).clamp(0.0, 1.0);
    let alpha = |base: String| if a < 0.999 { format!("{base}.withAlphaComponent({})", num((a * 100.0).round() / 100.0)) } else { base };
    if let Some(v) = bound(doc, n, key) {
        return alpha(token_text(v, CodeSyntaxPlatform::Ios, || format!("(UIColor(named: {}) ?? UIColor.clear)", swift_quote(&token_camel(v, true)))));
    }
    let base = if c.r == 1.0 && c.g == 1.0 && c.b == 1.0 {
        "UIColor.white".to_string()
    } else if c.r == 0.0 && c.g == 0.0 && c.b == 0.0 {
        "UIColor.black".to_string()
    } else {
        let f = |v: f32| num((v * 1000.0).round() / 1000.0);
        format!("UIColor(red: {}, green: {}, blue: {}, alpha: 1)", f(c.r), f(c.g), f(c.b))
    };
    alpha(base)
}

fn dimension(doc: &Document, n: &Node, key: &str, v: f32) -> String {
    match bound(doc, n, key) {
        Some(var) => token_text(var, CodeSyntaxPlatform::Ios, || format!("Tokens.{}", token_camel(var, false))),
        None => num(v),
    }
}

impl Cx<'_> {
    fn line(&mut self, s: impl AsRef<str>) {
        self.out.line(0, s);
    }

    /// A unique Swift identifier from a layer name.
    fn ident(&mut self, name: &str) -> String {
        let mut base = camel(name);
        if SWIFT_KEYWORDS.contains(&base.as_str()) {
            base.push_str("View");
        }
        let mut id = base.clone();
        let mut k = 2;
        while !self.used.insert(id.clone()) {
            id = format!("{base}{k}");
            k += 1;
        }
        id
    }

    fn emit(&mut self, id: NodeId, parent: Parent<'_>, host: Option<Host>) {
        let doc = self.doc;
        let Some(n) = doc.get(id) else { return };
        if !n.visible {
            return;
        }
        let sz = sizes(n, parent);
        let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
        let kids = visible_children(doc, n);
        let v = self.ident(&n.name);
        let mut spacers = (false, false, false);
        let mut children: Vec<NodeId> = Vec::new();
        let mut stack_al = None;

        if let NodeKind::Instance { .. } = n.kind {
            self.line(format!("let {v} = {}()", view_name(doc, n)));
            for (name, lit) in instance_args_for(doc, n, crate::slots::Lang::Swift) {
                let value = match lit {
                    PropLit::Bool(b) => b.to_string(),
                    PropLit::Text(t) => swift_quote(&t),
                    PropLit::Instance(c) => format!("{c}()"),
                };
                self.line(format!("{v}.{name} = {value}"));
            }
        } else if is_vector_asset(doc, id) {
            self.line(format!("let {v} = UIImageView(image: UIImage(named: {}))", swift_quote(&kebab(&n.name))));
            self.line(format!("{v}.contentMode = .scaleAspectFit"));
        } else if let Some(t) = text_of(n) {
            self.line(format!("let {v} = UILabel()"));
            self.label(&v, n, t);
        } else if let Some(al) = own_al.filter(|_| !kids.is_empty()) {
            let horizontal = al.direction == LayoutDirection::Horizontal;
            self.line(format!("let {v} = UIStackView()"));
            self.line(format!("{v}.axis = {}", if horizontal { ".horizontal" } else { ".vertical" }));
            let spacing = if al.primary_align == LayoutAlign::SpaceBetween { 0.0 } else { al.item_spacing };
            self.line(format!("{v}.spacing = {}", dimension(doc, n, "itemSpacing", spacing)));
            let counter = match (al.counter_align, horizontal) {
                (LayoutAlign::Center, _) => ".center",
                (LayoutAlign::End, true) => ".bottom",
                (LayoutAlign::End, false) => ".trailing",
                (LayoutAlign::Baseline, true) => ".firstBaseline",
                (_, true) => ".top",
                (_, false) => ".leading",
            };
            self.line(format!("{v}.alignment = {counter}"));
            let p = al.padding;
            if p.iter().any(|x| *x > 0.0) || ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"].iter().any(|k| bound(doc, n, k).is_some()) {
                let side = |i: usize, key: &str| dimension(doc, n, key, p[i]);
                self.line(format!("{v}.isLayoutMarginsRelativeArrangement = true"));
                self.line(format!(
                    "{v}.layoutMargins = UIEdgeInsets(top: {}, left: {}, bottom: {}, right: {})",
                    side(0, "paddingTop"),
                    side(3, "paddingLeft"),
                    side(2, "paddingBottom"),
                    side(1, "paddingRight")
                ));
            }
            if al.wrap {
                self.line("// auto layout wraps: use a UICollectionView with a flow layout");
            }
            spacers = match al.primary_align {
                LayoutAlign::Center => (true, false, true),
                LayoutAlign::End => (true, false, false),
                LayoutAlign::SpaceBetween => (false, true, false),
                _ => (false, false, false),
            };
            children = kids.iter().map(|k| k.id).collect();
            stack_al = Some(al);
        } else if n.kind.is_container() && !kids.is_empty() {
            self.line(format!("let {v} = UIView()"));
            children = kids.iter().map(|k| k.id).collect();
        } else if matches!(n.kind, NodeKind::Line) {
            self.line(format!("let {v} = UIView()"));
            if let Some(p) = first_paint(&n.stroke.paints) {
                self.line(format!("{v}.backgroundColor = {}", self.paint_color(n, "strokes/0", p)));
            }
        } else {
            self.line(format!("let {v} = UIView()"));
        }

        self.style(&v, n);
        self.layout(&v, n, &sz, host, parent);

        if let Some(host) = host {
            match host {
                Host::Stack(p) => self.line(format!("{p}.addArrangedSubview({v})")),
                Host::Plain(p) => self.line(format!("{p}.addSubview({v})")),
            }
        }
        let me = Parent::of(n);
        let child_host = if stack_al.is_some() { Host::Stack(&v) } else { Host::Plain(&v) };
        let horizontal = stack_al.is_some_and(|a| a.direction == LayoutDirection::Horizontal);
        let spacer = |this: &mut Self| {
            let s = this.ident("spacer");
            let axis = if horizontal { ".horizontal" } else { ".vertical" };
            this.line(format!("let {s} = UIView()"));
            this.line(format!("{s}.setContentHuggingPriority(.defaultLow, for: {axis})"));
            this.line(format!("{v}.addArrangedSubview({s})"));
        };
        if spacers.0 {
            spacer(self);
        }
        for (i, c) in children.iter().enumerate() {
            if i > 0 && spacers.1 {
                spacer(self);
            }
            self.emit(*c, me, Some(child_host));
        }
        if spacers.2 {
            spacer(self);
        }
    }

    fn paint_color(&self, n: &Node, key: &str, p: &Paint) -> String {
        match &p.kind {
            PaintKind::Solid(c) => ui_color(self.doc, n, key, c, p.opacity),
            PaintKind::LinearGradient { stops, .. }
            | PaintKind::RadialGradient { stops, .. }
            | PaintKind::AngularGradient { stops, .. }
            | PaintKind::DiamondGradient { stops, .. } => match stops.first() {
                Some(s) => ui_color(self.doc, n, "", &s.color, p.opacity),
                None => "UIColor.clear".into(),
            },
            PaintKind::Image { .. } => "UIColor.gray".into(),
        }
    }

    fn label(&mut self, v: &str, n: &Node, t: &TextProps) {
        let doc = self.doc;
        let s = &t.style;
        let size = dimension(doc, n, "fontSize", s.font_size);
        let weight = weight_name(s.font_weight);
        let font = if s.italic {
            // the system italic font has no family name to look up
            format!("UIFont(name: {}, size: {size}) ?? UIFont.italicSystemFont(ofSize: {size})", swift_quote(&s.font_family))
        } else {
            format!("UIFont(name: {}, size: {size}) ?? UIFont.systemFont(ofSize: {size}, weight: {weight})", swift_quote(&s.font_family))
        };
        let content = match s.case {
            TextCase::Upper => t.content.to_uppercase(),
            TextCase::Lower => t.content.to_lowercase(),
            _ => t.content.clone(),
        };
        let alignment = match s.align_h {
            TextAlignH::Left => ".left",
            TextAlignH::Center => ".center",
            TextAlignH::Right => ".right",
            TextAlignH::Justified => ".justified",
        };
        let color = first_paint(&s.fills).map(|p| self.paint_color(n, "fills/0", p));
        let line_height = match s.line_height {
            LineHeight::Auto => None,
            LineHeight::Pixels(p) => Some(p),
            LineHeight::Percent(p) => Some(s.font_size * p / 100.0),
        };
        let decorated = s.decoration != TextDecoration::None;
        if line_height.is_none() && s.letter_spacing == 0.0 && !decorated {
            self.line(format!("{v}.text = {}", swift_quote(&content)));
            self.line(format!("{v}.font = {font}"));
            if let Some(c) = &color {
                self.line(format!("{v}.textColor = {c}"));
            }
            self.line(format!("{v}.textAlignment = {alignment}"));
        } else {
            self.line(format!("{v}.font = {font}"));
            self.line(format!("let {v}Paragraph = NSMutableParagraphStyle()"));
            if let Some(h) = line_height {
                self.line(format!("{v}Paragraph.minimumLineHeight = {}", num(h)));
                self.line(format!("{v}Paragraph.maximumLineHeight = {}", num(h)));
            }
            self.line(format!("{v}Paragraph.alignment = {alignment}"));
            let mut attrs = vec![format!(".font: {v}.font as Any"), format!(".paragraphStyle: {v}Paragraph")];
            if let Some(c) = &color {
                attrs.push(format!(".foregroundColor: {c}"));
            }
            if s.letter_spacing != 0.0 {
                attrs.push(format!(".kern: {}", dimension(doc, n, "letterSpacing", s.letter_spacing)));
            }
            match s.decoration {
                TextDecoration::None => {}
                TextDecoration::Underline => attrs.push(".underlineStyle: NSUnderlineStyle.single.rawValue".into()),
                TextDecoration::Strikethrough => attrs.push(".strikethroughStyle: NSUnderlineStyle.single.rawValue".into()),
            }
            self.line(format!("{v}.attributedText = NSAttributedString(string: {}, attributes: [{}])", swift_quote(&content), attrs.join(", ")));
        }
        self.line(format!("{v}.numberOfLines = 0"));
    }

    /// Fill, border, corner radius, clipping, shadows, opacity and rotation.
    fn style(&mut self, v: &str, n: &Node) {
        let doc = self.doc;
        if text_of(n).is_none() && !matches!(n.kind, NodeKind::Line) {
            let fill = n.fills.iter().enumerate().rev().find(|(_, p)| p.visible && p.opacity > 0.0);
            if let Some((i, p)) = fill {
                self.fill(v, n, &format!("fills/{i}"), p);
            }
            if let Some(p) = n.stroke.paints.iter().find(|p| p.visible && p.opacity > 0.0).filter(|_| n.stroke.weight > 0.0) {
                self.line(format!("{v}.layer.borderWidth = {}", dimension(doc, n, "strokeWeight", n.stroke.weight)));
                self.line(format!("{v}.layer.borderColor = {}.cgColor", self.paint_color(n, "strokes/0", p)));
            }
        }
        let keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let r = n.corner_radii;
        if matches!(n.kind, NodeKind::Ellipse(_)) {
            let radius = if (n.size.x - n.size.y).abs() < 0.01 { num(n.size.x / 2.0) } else { num(n.size.x.min(n.size.y) / 2.0) };
            self.line(format!("{v}.layer.cornerRadius = {radius}"));
        } else if r.iter().any(|x| *x > 0.0) || keys.iter().any(|k| bound(doc, n, k).is_some()) {
            let uniform = r.iter().all(|x| *x == r[0]);
            if uniform {
                self.line(format!("{v}.layer.cornerRadius = {}", dimension(doc, n, keys[0], r[0])));
            } else {
                self.line(format!("{v}.layer.cornerRadius = {}", dimension(doc, n, keys[0], r[0].max(r[1]).max(r[2]).max(r[3]))));
                // CACornerMask: minX = left, maxX = right, minY = top (UIKit's y axis points down)
                let mut mask = Vec::new();
                for (i, name) in [".layerMinXMinYCorner", ".layerMaxXMinYCorner", ".layerMaxXMaxYCorner", ".layerMinXMaxYCorner"].into_iter().enumerate() {
                    if r[i] > 0.0 {
                        mask.push(name);
                    }
                }
                self.line(format!("{v}.layer.maskedCorners = [{}] // per-corner radii differ: only rounded corners are masked", mask.join(", ")));
            }
        }
        if n.kind.frame_props().is_some_and(|f| f.clips_content) {
            self.line(format!("{v}.clipsToBounds = true"));
        }
        if n.kind.frame_props().is_some_and(|f| f.clips_content) && !drop_shadows(n).is_empty() {
            self.line("// clipsToBounds also clips the shadow: put the shadow on a wrapper view");
        }
        for (c, off, radius, _) in drop_shadows(n) {
            self.line(format!("{v}.layer.shadowColor = {}.cgColor", ui_color(doc, n, "", &Color { a: 1.0, ..*c }, 1.0)));
            self.line(format!("{v}.layer.shadowOpacity = {}", num((c.a * 100.0).round() / 100.0)));
            self.line(format!("{v}.layer.shadowOffset = CGSize(width: {}, height: {})", num(off.x), num(off.y)));
            self.line(format!("{v}.layer.shadowRadius = {}", num(radius / 2.0)));
        }
        match bound(doc, n, "opacity") {
            Some(var) => {
                let token = token_text(var, CodeSyntaxPlatform::Ios, || format!("Tokens.{}", token_camel(var, false)));
                self.line(format!("{v}.alpha = {token} / 100"));
            }
            None if n.opacity < 0.999 => self.line(format!("{v}.alpha = {}", num((n.opacity * 100.0).round() / 100.0))),
            None => {}
        }
        let rot = rotation_deg(n);
        if rot != 0.0 {
            self.line(format!("{v}.transform = CGAffineTransform(rotationAngle: {} * .pi / 180)", num(rot)));
        }
    }

    fn fill(&mut self, v: &str, n: &Node, key: &str, p: &Paint) {
        let doc = self.doc;
        match &p.kind {
            PaintKind::Solid(c) => self.line(format!("{v}.backgroundColor = {}", ui_color(doc, n, key, c, p.opacity))),
            PaintKind::LinearGradient { handles, stops } => {
                let g = format!("{v}Gradient");
                let colors: Vec<String> = stops.iter().map(|s| format!("{}.cgColor", ui_color(doc, n, "", &s.color, p.opacity))).collect();
                let locations: Vec<String> = stops.iter().map(|s| num(s.pos)).collect();
                self.line(format!("let {g} = CAGradientLayer()"));
                self.line(format!("{g}.colors = [{}]", colors.join(", ")));
                self.line(format!("{g}.locations = [{}]", locations.join(", ")));
                self.line(format!("{g}.startPoint = CGPoint(x: {}, y: {})", num(handles[0].x), num(handles[0].y)));
                self.line(format!("{g}.endPoint = CGPoint(x: {}, y: {})", num(handles[1].x), num(handles[1].y)));
                self.line(format!("{g}.frame = CGRect(x: 0, y: 0, width: {}, height: {})", num(n.size.x), num(n.size.y)));
                self.line(format!("{v}.layer.insertSublayer({g}, at: 0)"));
            }
            _ => self.line(format!("{v}.backgroundColor = {} // gradient or image fill: first stop shown", self.paint_color(n, key, p))),
        }
    }

    /// Size and position: constraints inside stacks, `frame` elsewhere.
    fn layout(&mut self, v: &str, n: &Node, sz: &Sizes, host: Option<Host>, parent: Parent) {
        let doc = self.doc;
        let in_stack = matches!(host, Some(Host::Stack(_))) || host.is_none();
        if !in_stack {
            let (x, y) = offset(n, parent, sz).unwrap_or((n.transform.tx, n.transform.ty));
            self.line(format!(
                "{v}.frame = CGRect(x: {}, y: {}, width: {}, height: {})",
                num(x),
                num(y),
                dimension(doc, n, "width", n.size.x),
                dimension(doc, n, "height", n.size.y)
            ));
            return;
        }
        let mut cons: Vec<String> = Vec::new();
        match sz.w {
            Dim::Fixed(px) => cons.push(format!("{v}.widthAnchor.constraint(equalToConstant: {})", dimension(doc, n, "width", px))),
            Dim::Fill if !sz.row_parent => {
                if let (Some(Host::Stack(p)), Some(al)) = (host, parent.al) {
                    let inset = al.padding[1] + al.padding[3];
                    cons.push(format!("{v}.widthAnchor.constraint(equalTo: {p}.widthAnchor, constant: {})", num(-inset)));
                }
            }
            Dim::Fill => self.line(format!("{v}.setContentHuggingPriority(.defaultLow, for: .horizontal)")),
            Dim::Hug => {}
        }
        match sz.h {
            Dim::Fixed(px) => cons.push(format!("{v}.heightAnchor.constraint(equalToConstant: {})", dimension(doc, n, "height", px))),
            Dim::Fill if sz.row_parent => {
                if let (Some(Host::Stack(p)), Some(al)) = (host, parent.al) {
                    let inset = al.padding[0] + al.padding[2];
                    cons.push(format!("{v}.heightAnchor.constraint(equalTo: {p}.heightAnchor, constant: {})", num(-inset)));
                }
            }
            Dim::Fill => self.line(format!("{v}.setContentHuggingPriority(.defaultLow, for: .vertical)")),
            Dim::Hug => {}
        }
        if host.is_none() && !cons.is_empty() {
            self.line(format!("{v}.translatesAutoresizingMaskIntoConstraints = false"));
        }
        if !cons.is_empty() {
            self.line("NSLayoutConstraint.activate([");
            for c in cons {
                self.out.line(1, format!("{c},"));
            }
            self.line("])");
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
        chip.corner_radii = [4.0; 4];
        chip.fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
        d.insert(card_id, chip).unwrap();
        (d, card_id)
    }

    #[test]
    fn golden_card() {
        let (d, id) = card_fixture();
        let expected = r####"let card = UIStackView()
card.axis = .vertical
card.spacing = 8
card.alignment = .leading
card.isLayoutMarginsRelativeArrangement = true
card.layoutMargins = UIEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
card.backgroundColor = UIColor.white
card.layer.borderWidth = 1
card.layer.borderColor = UIColor(red: 0.898, green: 0.898, blue: 0.898, alpha: 1).cgColor
card.layer.cornerRadius = 8
card.clipsToBounds = true
card.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    card.widthAnchor.constraint(equalToConstant: 200),
    card.heightAnchor.constraint(equalToConstant: 120),
])
let title = UILabel()
title.text = "Hello world"
title.font = UIFont(name: "Inter", size: 20) ?? UIFont.systemFont(ofSize: 20, weight: .semibold)
title.textColor = UIColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
title.textAlignment = .left
title.numberOfLines = 0
card.addArrangedSubview(title)
let chip = UIView()
chip.backgroundColor = UIColor(red: 1, green: 0, blue: 0, alpha: 1)
chip.layer.cornerRadius = 4
NSLayoutConstraint.activate([
    chip.widthAnchor.constraint(equalToConstant: 40),
    chip.heightAnchor.constraint(equalToConstant: 24),
])
card.addArrangedSubview(chip)"####;
        assert_eq!(uikit_for(&d, id), expected);
    }

    #[test]
    fn strings_and_font_names_are_escaped_as_swift_literals() {
        let (mut d, card) = card_fixture();
        let title = d.get(card).unwrap().children[0];
        let n = d.get_mut(title).unwrap();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "Say \"hi\" \\ $100\nnext\u{1}".into();
            t.style.font_family = "Evil\") ?? UIFont(name: \"x".into();
        }
        let code = uikit_for(&d, card);
        assert!(code.contains(r#"title.text = "Say \"hi\" \\ $100\nnext\u{1}""#), "{code}");
        assert!(code.contains(r#"UIFont(name: "Evil\") ?? UIFont(name: \"x", size: 20)"#), "{code}");
        // the injected text never closes the literal early
        assert!(!code.contains(r#"name: "Evil") ?? UIFont(name: "x""#));
    }

    #[test]
    fn bound_variables_and_shared_fixture_render() {
        let (d, id) = fixture::card();
        let code = uikit_for(&d, id);
        assert!(code.contains("UIColor(named: \"textMuted\")"), "{code}");
        assert!(code.contains("actions.spacing = Tokens.spacingMd"), "{code}");
        assert!(code.contains("let button = Button()"), "{code}");
        assert!(code.contains("button.label = \"Save\""), "{code}");
        assert!(code.contains("button.disabled = true"), "{code}");
        // every added subview was declared first
        for line in code.lines().filter(|l| l.contains("addArrangedSubview(") || l.contains("addSubview(")) {
            let name = line.split('(').nth(1).unwrap().trim_end_matches(')');
            assert!(code.contains(&format!("let {name} = ")), "{name} is never declared:\n{code}");
        }
    }

    #[test]
    fn uikit_hugging_row_all_fill_children() {
        let (d, row, _) = fixture::row_with_counter_children(Sizing::Hug, &[20.0, 100.0], true, false);
        let out = uikit_for(&d, row);
        assert_eq!(out.matches("heightAnchor.constraint(equalToConstant: 100)").count(), 2, "{out}");
    }

    #[test]
    fn uikit_grid_fill_child_exports_solved_height() {
        for (track, height) in [(None, 20), (Some(GridTrack::Fixed(100.0)), 100)] {
            let (d, grid, _) = fixture::grid_fill_child(track);
            let out = uikit_for(&d, grid);
            assert!(out.contains(&format!("heightAnchor.constraint(equalToConstant: {height})")), "{track:?}: {out}");
        }
    }
}
