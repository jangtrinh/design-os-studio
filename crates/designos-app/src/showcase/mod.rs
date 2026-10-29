//! The built-in showcase document (`--fixture showcase`): a small, invented mobile-banking product
#![cfg(any(test, feature = "fixtures"))]
//! ("Marlow") drawn entirely with designos's own nodes, so the README screenshots show real design
//! work instead of a test fixture.
//!
//! * **Mobile**: four 390 x 844 screens (welcome, home, spending, settings) in real auto layout,
//!   built from the components below, with prototype links and two flow starting points.
//! * **Web**: one 1440 px dashboard (sidebar, KPI cards, charts, table).
//! * **Components**: Button, Tag, Avatar, Toggle, List row and Tab bar as component sets with
//!   variants, text, boolean and instance-swap properties, plus the icon components.
//! * A "Theme" variable collection (Light and Dark) bound to the main surfaces, shared color, text
//!   and effect styles.
//!
//! Everything is vectors, gradients and Inter (bundled), so rendering never needs the network. The
//! icons are drawn here from simple geometry; no icon set, logo, brand or stock image is used.

mod icons;
mod kit;
mod mobile;
mod proto;
mod web;

#[cfg(test)]
mod tests;

use crate::edit::Edit;
use crate::layout;
use crate::textmeasure;
use designos_doc::*;
use icons::Ic;
use std::collections::BTreeMap;

/// Name of the fixture and of the document.
pub const FIXTURE: &str = "showcase";
pub const NAME: &str = "Marlow - Mobile banking";

/// Theme tokens: a name in the "Theme" variable collection with a Light and a Dark value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tok {
    Bg,
    Card,
    Raised,
    Ink,
    Muted,
    Faint,
    Border,
    Brand,
    BrandSoft,
    Pos,
    PosSoft,
    Neg,
    NegSoft,
}

const TOKENS: [(Tok, &str, u32, u32); 13] = [
    (Tok::Bg, "Surface/Background", 0xF3F4FA, 0x0D0E16),
    (Tok::Card, "Surface/Card", 0xFFFFFF, 0x191B28),
    (Tok::Raised, "Surface/Raised", 0xEEF0F8, 0x242739),
    (Tok::Ink, "Text/Primary", 0x12131C, 0xF3F4FB),
    (Tok::Muted, "Text/Secondary", 0x6A6E85, 0xA1A6BF),
    (Tok::Faint, "Text/Tertiary", 0xA3A7BC, 0x6B7089),
    (Tok::Border, "Border/Subtle", 0xE4E6F1, 0x2A2D40),
    (Tok::Brand, "Brand/Primary", 0x4C5BF0, 0x7C88FF),
    (Tok::BrandSoft, "Brand/Soft", 0xE9ECFF, 0x262B5E),
    (Tok::Pos, "Status/Positive", 0x12A67F, 0x36D3A8),
    (Tok::PosSoft, "Status/Positive soft", 0xDDF6EE, 0x143C33),
    (Tok::Neg, "Status/Negative", 0xE5436B, 0xFF7A99),
    (Tok::NegSoft, "Status/Negative soft", 0xFDE6EC, 0x41202D),
];

pub fn hex(h: u32) -> Color {
    Color::rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

fn light(t: Tok) -> Color {
    hex(TOKENS.iter().find(|x| x.0 == t).map(|x| x.2).unwrap_or(0))
}

/// A color that is either a theme token (bound to its variable) or a literal.
#[derive(Clone, Copy, Debug)]
pub enum Cl {
    T(Tok),
    C(Color),
}

impl Cl {
    fn color(self) -> Color {
        match self {
            Cl::T(t) => light(t),
            Cl::C(c) => c,
        }
    }
}

pub const WHITE: Cl = Cl::C(Color::WHITE);

pub fn white(a: f32) -> Cl {
    Cl::C(Color::WHITE.with_alpha(a))
}

use LayoutAlign::{Center, End, SpaceBetween, Start};
use LayoutDirection::{Horizontal as H, Vertical as V};

pub struct S {
    pub d: Document,
    styles: BTreeMap<&'static str, TextStyle>,
    vars: BTreeMap<Tok, String>,
    pub theme: String,
    pub dark: String,
    pub v_radius_card: String,
    pub v_radius_ctl: String,
    pub v_space: String,
    /// Component set name -> set node.
    pub sets: BTreeMap<&'static str, NodeId>,
    /// "Set/Variant/Variant" -> the variant's main component.
    pub variants: BTreeMap<String, NodeId>,
    pub icon_comps: BTreeMap<Ic, NodeId>,
}

const TEXT_STYLES: [(&str, f32, u16, f32, f32); 11] = [
    ("Text/Display", 34.0, 700, 40.0, -0.7),
    ("Text/Headline", 28.0, 700, 34.0, -0.5),
    ("Text/Title", 24.0, 700, 30.0, -0.4),
    ("Text/Heading", 17.0, 600, 24.0, -0.1),
    ("Text/Body", 15.0, 400, 22.0, 0.0),
    ("Text/Body strong", 15.0, 600, 22.0, 0.0),
    ("Text/Caption", 13.0, 500, 18.0, 0.0),
    ("Text/Label", 12.0, 600, 16.0, 0.1),
    ("Text/Micro", 11.0, 600, 14.0, 0.6),
    ("Text/Amount", 15.0, 600, 22.0, 0.0),
    ("Text/Button", 16.0, 600, 22.0, 0.0),
];

impl S {
    fn new() -> Self {
        let mut d = Document::new(NAME);
        for (name, bg) in [("Mobile", 0xE4E6F0), ("Web", 0xE4E6F0), ("Components", 0xF1F2F8)] {
            let p = d.add_page(name);
            if let Some(n) = d.get_mut(p) {
                n.kind = NodeKind::Page { background: hex(bg) };
            }
        }
        let mut s = S {
            d,
            styles: BTreeMap::new(),
            vars: BTreeMap::new(),
            theme: String::new(),
            dark: String::new(),
            v_radius_card: String::new(),
            v_radius_ctl: String::new(),
            v_space: String::new(),
            sets: BTreeMap::new(),
            variants: BTreeMap::new(),
            icon_comps: BTreeMap::new(),
        };
        s.make_styles();
        s.make_variables();
        s
    }

    pub fn page(&self, i: usize) -> NodeId {
        self.d.pages[i]
    }

    // ---------------------------------------------------------------- styles and variables

    fn make_styles(&mut self) {
        for (name, size, weight, lh, ls) in TEXT_STYLES {
            let mut ts = TextStyle { font_size: size, font_weight: weight, line_height: LineHeight::Pixels(lh), letter_spacing: ls, ..Default::default() };
            if name == "Text/Amount" || name == "Text/Display" || name == "Text/Headline" {
                ts.features = vec![("tnum".into(), 1)];
            }
            if name == "Text/Micro" {
                ts.case = TextCase::Upper;
            }
            self.d.styles.insert(name.into(), Style::Text(ts.clone()));
            self.styles.insert(name, ts);
        }
        let solid = |c: Color| Style::Fill(vec![Paint::solid(c)]);
        for (name, c) in [
            ("Brand/Primary", 0x4C5BF0),
            ("Brand/Soft", 0xE9ECFF),
            ("Neutral/Ink", 0x12131C),
            ("Neutral/Muted", 0x6A6E85),
            ("Surface/Card", 0xFFFFFF),
            ("Status/Positive", 0x12A67F),
            ("Status/Negative", 0xE5436B),
            ("Accent/Amber", 0xF5B544),
        ] {
            self.d.styles.insert(name.into(), solid(hex(c)));
        }
        self.d.styles.insert("Gradient/Brand".into(), Style::Fill(vec![grad_d(&[(0.0, hex(0x4C5BF0)), (1.0, hex(0x8D5CF6))])]));
        self.d.styles.insert("Gradient/Night".into(), Style::Fill(vec![grad_d(&[(0.0, hex(0x23265A)), (1.0, hex(0x0F1030))])]));
        self.d.styles.insert("Shadow/Card".into(), Style::Effect(vec![card_shadow()]));
        self.d.styles.insert("Shadow/Floating".into(), Style::Effect(vec![floating_shadow()]));
    }

    fn make_variables(&mut self) {
        let mut s = VariableStore::default();
        // Theme first so it is the collection the Variables view opens on.
        let theme = s.add_collection("Theme");
        let prim = s.add_collection("Primitives");
        let pm = s.collections[&prim].default_mode.clone();
        s.rename_mode(&prim, &pm, "Value");
        let color = |s: &mut VariableStore, coll: &str, name: &str, vals: &[(&str, u32)]| {
            let id = s.add_variable(coll, name, VariableKind::Color).expect("collection exists");
            for (m, h) in vals {
                s.set_value(&id, m, VariableValue::Color(hex(*h)));
            }
            id
        };
        let indigo500 = color(&mut s, &prim, "Indigo/500", &[(&pm, 0x4C5BF0)]);
        let indigo400 = color(&mut s, &prim, "Indigo/400", &[(&pm, 0x7C88FF)]);
        let indigo50 = color(&mut s, &prim, "Indigo/50", &[(&pm, 0xE9ECFF)]);
        let indigo900 = color(&mut s, &prim, "Indigo/900", &[(&pm, 0x262B5E)]);
        let mint600 = color(&mut s, &prim, "Mint/600", &[(&pm, 0x12A67F)]);
        let mint400 = color(&mut s, &prim, "Mint/400", &[(&pm, 0x36D3A8)]);
        let coral500 = color(&mut s, &prim, "Coral/500", &[(&pm, 0xE5436B)]);
        let coral300 = color(&mut s, &prim, "Coral/300", &[(&pm, 0xFF7A99)]);
        color(&mut s, &prim, "Ink/900", &[(&pm, 0x12131C)]);
        color(&mut s, &prim, "Ink/50", &[(&pm, 0xF3F4FB)]);

        let light_m = s.collections[&theme].default_mode.clone();
        s.rename_mode(&theme, &light_m, "Light");
        let dark_m = s.add_mode(&theme, "Dark").expect("mode");
        for (tok, name, l, dk) in TOKENS {
            let id = color(&mut s, &theme, name, &[(&light_m, l), (&dark_m, dk)]);
            let alias = match tok {
                Tok::Brand => Some((&indigo500, &indigo400)),
                Tok::BrandSoft => Some((&indigo50, &indigo900)),
                Tok::Pos => Some((&mint600, &mint400)),
                Tok::Neg => Some((&coral500, &coral300)),
                _ => None,
            };
            if let Some((a, b)) = alias {
                s.set_alias(&id, &light_m, a);
                s.set_alias(&id, &dark_m, b);
            }
            self.vars.insert(tok, id);
        }
        let number = |s: &mut VariableStore, name: &str, v: f32| {
            let id = s.add_variable(&theme, name, VariableKind::Number).expect("collection exists");
            for m in [&light_m, &dark_m] {
                s.set_value(&id, m, VariableValue::Number(v));
            }
            id
        };
        self.v_radius_card = number(&mut s, "Radius/Card", 24.0);
        self.v_radius_ctl = number(&mut s, "Radius/Control", 16.0);
        self.v_space = number(&mut s, "Space/Screen", 20.0);
        self.d.variables = s.variables;
        self.d.variable_collections = s.collections;
        self.theme = theme;
        self.dark = dark_m;
    }

    pub fn var(&self, t: Tok) -> &str {
        &self.vars[&t]
    }

    // ---------------------------------------------------------------- nodes

    pub fn n(&mut self, id: NodeId) -> &mut Node {
        self.d.get_mut(id).expect("showcase node")
    }

    fn add(&mut self, parent: NodeId, mut node: Node, name: &str) -> NodeId {
        node.name = name.into();
        self.d.insert(parent, node).expect("showcase parent is a container")
    }

    /// A transparent frame at the origin; size it with [`S::size`] or let auto layout do it.
    pub fn frame(&mut self, parent: NodeId, name: &str) -> NodeId {
        let id = self.d.alloc_id();
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        n.fills = Vec::new();
        self.add(parent, n, name)
    }

    pub fn rect(&mut self, parent: NodeId, name: &str, w: f32, h: f32, cl: Cl) -> NodeId {
        let id = self.d.alloc_id();
        let n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
        let id = self.add(parent, n, name);
        self.fill(id, cl);
        id
    }

    pub fn ellipse(&mut self, parent: NodeId, name: &str, w: f32, h: f32, cl: Cl) -> NodeId {
        let id = self.d.alloc_id();
        let n = Node::ellipse(id, Rect::new(0.0, 0.0, w, h));
        let id = self.add(parent, n, name);
        self.fill(id, cl);
        id
    }

    pub fn vector(&mut self, parent: NodeId, name: &str, w: f32, h: f32, paths: Vec<Path>) -> NodeId {
        let id = self.d.alloc_id();
        let mut n = Node::new(id, NodeKind::Vector { paths, network: None });
        n.size = Vec2::new(w, h);
        let id = self.add(parent, n, name);
        self.n(id).stroke.cap = StrokeCap::Round;
        self.n(id).stroke.join = StrokeJoin::Round;
        id
    }

    /// A stroke icon from the showcase set at `size` px, stroked in `cl`.
    pub fn vicon(&mut self, parent: NodeId, ic: Ic, size: f32, cl: Cl) -> NodeId {
        let k = size / 24.0;
        let id = self.vector(parent, ic.name(), size, size, icons::paths(ic, k));
        self.stroke(id, cl, ic.weight() * k.max(0.85));
        id
    }

    /// Text in a shared text style; `cl` is bound to its variable when it is a theme token.
    pub fn text(&mut self, parent: NodeId, s: &str, style: &'static str, cl: Cl) -> NodeId {
        let id = self.d.alloc_id();
        let mut ts = self.styles[style].clone();
        ts.fills = vec![Paint::solid(cl.color())];
        let mut n = Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), s, ts);
        n.style_refs.insert("text".into(), style.into());
        let name: String = s.chars().take(36).collect();
        let id = self.add(parent, n, &name);
        if let Cl::T(t) = cl {
            let v = self.vars[&t].clone();
            self.n(id).variables.bindings.insert("fills/0".into(), v);
        }
        self.fit(id);
        id
    }

    /// A text that wraps at a fixed width.
    pub fn text_w(&mut self, parent: NodeId, s: &str, style: &'static str, cl: Cl, w: f32) -> NodeId {
        let id = self.text(parent, s, style, cl);
        if let NodeKind::Text(t) = &mut self.n(id).kind {
            t.auto_resize = TextAutoResize::Height;
        }
        self.n(id).size.x = w;
        self.fit(id);
        id
    }

    pub fn set_text_color(&mut self, id: NodeId, cl: Cl) {
        let v = if let Cl::T(t) = cl { Some(self.vars[&t].clone()) } else { None };
        let n = self.n(id);
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.fills = vec![Paint::solid(cl.color())];
        }
        match v {
            Some(v) => {
                n.variables.bindings.insert("fills/0".into(), v);
            }
            None => {
                n.variables.bindings.remove("fills/0");
            }
        }
    }

    /// Re-measures a text layer after its content or style changed.
    pub fn fit(&mut self, id: NodeId) {
        let Some(n) = self.d.get(id) else { return };
        let NodeKind::Text(t) = &n.kind else { return };
        let size = match t.auto_resize {
            TextAutoResize::WidthAndHeight => textmeasure::measure(t, None),
            TextAutoResize::Height => Vec2::new(n.size.x, textmeasure::measure(t, Some(n.size.x.max(1.0))).y),
            _ => n.size,
        };
        self.n(id).size = size;
    }

    // ---------------------------------------------------------------- appearance

    pub fn at(&mut self, id: NodeId, x: f32, y: f32) {
        self.n(id).transform = Affine::translate(x, y);
    }

    pub fn size(&mut self, id: NodeId, w: f32, h: f32) {
        self.n(id).size = Vec2::new(w, h);
    }

    /// Rotates a node of known size about its own center, which lands on (cx, cy).
    pub fn place_rotated(&mut self, id: NodeId, cx: f32, cy: f32, degrees: f32) {
        let sz = self.n(id).size;
        self.n(id).transform = Affine::translate(cx, cy) * Affine::rotate(degrees.to_radians()) * Affine::translate(-sz.x / 2.0, -sz.y / 2.0);
    }

    pub fn fill(&mut self, id: NodeId, cl: Cl) {
        self.fill_paint(id, Paint::solid(cl.color()));
        let v = if let Cl::T(t) = cl { Some(self.vars[&t].clone()) } else { None };
        let n = self.n(id);
        match v {
            Some(v) => {
                n.variables.bindings.insert("fills/0".into(), v);
            }
            None => {
                n.variables.bindings.remove("fills/0");
            }
        }
    }

    pub fn fill_paint(&mut self, id: NodeId, p: Paint) {
        self.n(id).fills = vec![p];
    }

    pub fn stroke(&mut self, id: NodeId, cl: Cl, weight: f32) {
        let v = if let Cl::T(t) = cl { Some(self.vars[&t].clone()) } else { None };
        let n = self.n(id);
        n.stroke.paints = vec![Paint::solid(cl.color())];
        n.stroke.weight = weight;
        if let Some(v) = v {
            n.variables.bindings.insert("strokes/0".into(), v);
        }
    }

    /// A 1 px inside border in the subtle border color.
    pub fn border(&mut self, id: NodeId) {
        self.stroke(id, Cl::T(Tok::Border), 1.0);
    }

    pub fn radius(&mut self, id: NodeId, r: f32) {
        self.n(id).corner_radii = [r; 4];
    }

    /// Corner radius bound to the "Radius/Card" or "Radius/Control" variable.
    pub fn radius_var(&mut self, id: NodeId, card: bool) {
        let (v, r) = if card { (self.v_radius_card.clone(), 24.0) } else { (self.v_radius_ctl.clone(), 16.0) };
        let n = self.n(id);
        n.corner_radii = [r; 4];
        for k in ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"] {
            n.variables.bindings.insert(k.into(), v.clone());
        }
    }

    pub fn clip(&mut self, id: NodeId) {
        if let Some(f) = self.n(id).kind.frame_props_mut() {
            f.clips_content = true;
        }
    }

    pub fn effect(&mut self, id: NodeId, e: Effect, style: Option<&str>) {
        self.n(id).effects.push(e);
        if let Some(s) = style {
            self.n(id).style_refs.insert("effect".into(), s.into());
        }
    }

    pub fn style_fill(&mut self, id: NodeId, key: &str) {
        self.n(id).style_refs.insert("fill".into(), key.into());
    }

    pub fn opacity(&mut self, id: NodeId, o: f32) {
        self.n(id).opacity = o;
    }

    // ---------------------------------------------------------------- auto layout

    /// An auto-layout frame that hugs its content on both axes.
    pub fn stack(&mut self, parent: NodeId, name: &str, dir: LayoutDirection, gap: f32, pad: [f32; 4]) -> NodeId {
        let id = self.frame(parent, name);
        self.make_stack(id, dir, gap, pad);
        id
    }

    pub fn make_stack(&mut self, id: NodeId, dir: LayoutDirection, gap: f32, pad: [f32; 4]) {
        let al = AutoLayout {
            direction: dir,
            item_spacing: gap,
            padding: pad,
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            primary_align: Start,
            counter_align: Start,
            ..Default::default()
        };
        if let Some(f) = self.n(id).kind.frame_props_mut() {
            f.auto_layout = Some(al);
        }
    }

    pub fn row(&mut self, parent: NodeId, name: &str, gap: f32, pad: [f32; 4]) -> NodeId {
        self.stack(parent, name, H, gap, pad)
    }

    pub fn col(&mut self, parent: NodeId, name: &str, gap: f32, pad: [f32; 4]) -> NodeId {
        self.stack(parent, name, V, gap, pad)
    }

    pub fn align(&mut self, id: NodeId, primary: LayoutAlign, counter: LayoutAlign) {
        if let Some(al) = self.n(id).kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            al.primary_align = primary;
            al.counter_align = counter;
        }
    }

    pub fn centered(&mut self, id: NodeId) {
        self.align(id, Center, Center);
    }

    pub fn spread(&mut self, id: NodeId, counter: LayoutAlign) {
        self.align(id, SpaceBetween, counter);
    }

    pub fn end_aligned(&mut self, id: NodeId) {
        self.align(id, End, Center);
    }

    fn axis_fixed(&mut self, id: NodeId, horizontal: bool) {
        if let Some(al) = self.n(id).kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            if (al.direction == H) == horizontal {
                al.primary_sizing = Sizing::Fixed;
            } else {
                al.counter_sizing = Sizing::Fixed;
            }
        }
    }

    pub fn fixed_w(&mut self, id: NodeId, w: f32) {
        self.n(id).size.x = w;
        self.axis_fixed(id, true);
    }

    pub fn fixed_h(&mut self, id: NodeId, h: f32) {
        self.n(id).size.y = h;
        self.axis_fixed(id, false);
    }

    pub fn fixed(&mut self, id: NodeId, w: f32, h: f32) {
        self.fixed_w(id, w);
        self.fixed_h(id, h);
    }

    /// Fill the parent's width (the node stops hugging horizontally).
    pub fn fill_w(&mut self, id: NodeId) {
        self.n(id).layout_child.horizontal = Sizing::Fill;
        self.axis_fixed(id, true);
    }

    pub fn fill_h(&mut self, id: NodeId) {
        self.n(id).layout_child.vertical = Sizing::Fill;
        self.axis_fixed(id, false);
    }

    /// Takes the free space along the parent's main axis.
    pub fn grow(&mut self, id: NodeId) {
        self.n(id).layout_child.grow = 1.0;
    }

    pub fn bind(&mut self, id: NodeId, key: &str, var: &str) {
        self.n(id).variables.bindings.insert(key.into(), var.into());
    }

    /// Screen side padding bound to the "Space/Screen" variable.
    pub fn bind_screen_padding(&mut self, id: NodeId) {
        let v = self.v_space.clone();
        self.bind(id, "paddingLeft", &v);
        self.bind(id, "paddingRight", &v);
    }

    /// A hairline divider inset by 16 px on both sides.
    pub fn divider(&mut self, parent: NodeId) -> NodeId {
        let f = self.row(parent, "Divider", 0.0, [0.0, 16.0, 0.0, 16.0]);
        self.fill_w(f);
        let r = self.rect(f, "Line", 1.0, 1.0, Cl::T(Tok::Border));
        self.fill_w(r);
        f
    }

    // ---------------------------------------------------------------- components

    /// Instantiates a main component under `parent`.
    pub fn instance(&mut self, parent: NodeId, comp: NodeId) -> NodeId {
        let cmd = create_instance(&mut self.d, comp, parent, Vec2::ZERO);
        let Command::Insert { nodes, .. } = &cmd else { panic!("showcase instance of a non-component") };
        let id = nodes[0].id;
        cmd.apply(&mut self.d).expect("showcase instance");
        id
    }

    /// The main component of variant `key` ("Button/Primary/Large").
    pub fn variant(&self, key: &str) -> NodeId {
        *self.variants.get(key).unwrap_or_else(|| panic!("missing variant {key}"))
    }

    pub fn inst(&mut self, parent: NodeId, key: &str) -> NodeId {
        let c = self.variant(key);
        self.instance(parent, c)
    }

    pub fn set_prop(&mut self, inst: NodeId, name: &str, v: PropValue) {
        // Syncing copies the main component's name onto the instance; keep the set's name instead.
        let keep = self.d.get(inst).map(|n| n.name.clone()).unwrap_or_default();
        let c = set_instance_prop(&mut self.d, inst, name, Some(v));
        c.apply(&mut self.d).expect("showcase property");
        self.n(inst).name = keep;
        self.ov(inst, OverrideField::Name);
    }

    pub fn set_text(&mut self, inst: NodeId, name: &str, v: &str) {
        self.set_prop(inst, name, PropValue::Text(v.into()));
    }

    pub fn set_icon(&mut self, inst: NodeId, ic: Ic) {
        let c = self.icon_comps[&ic];
        self.set_prop(inst, "Icon", PropValue::Instance(c));
    }

    /// First descendant of `root` with this name.
    pub fn find(&self, root: NodeId, name: &str) -> Option<NodeId> {
        let mut found = None;
        self.d.walk(root, |n, _| {
            if found.is_none() && n.name == name {
                found = Some(n.id);
            }
        });
        found
    }

    pub fn ov(&mut self, id: NodeId, f: OverrideField) {
        self.n(id).component.overrides.insert(f);
    }

    /// Makes an instance fill the parent's width: a layout override on the instance.
    pub fn inst_fill_w(&mut self, inst: NodeId) {
        self.fill_w(inst);
        self.ov(inst, OverrideField::Layout);
    }

    /// Overrides the fill of a layer inside an instance.
    pub fn ov_fill(&mut self, id: NodeId, p: Paint) {
        self.fill_paint(id, p);
        self.n(id).variables.bindings.remove("fills/0");
        self.ov(id, OverrideField::Fills);
    }

    /// Overrides the color of a text layer inside an instance (rebinding it to a theme token).
    pub fn ov_text_color(&mut self, id: NodeId, cl: Cl) {
        self.set_text_color(id, cl);
        self.ov(id, OverrideField::TextStyle);
    }

    // ---------------------------------------------------------------- layout

    /// Re-measures every text under `root` and lays the subtree out.
    pub fn relayout(&mut self, root: NodeId) {
        let mut texts = Vec::new();
        self.d.walk(root, |n, _| {
            if matches!(n.kind, NodeKind::Text(_)) {
                texts.push(n.id);
            }
        });
        for t in texts {
            self.fit(t);
        }
        let mut ed = Edit::new();
        layout::layout_node(&mut self.d, &mut ed, root);
    }

    /// Flags what auto layout changed on instance roots as overrides, so a later sync keeps it.
    pub fn flag_instance_sizes(&mut self, root: NodeId) {
        let mut roots = Vec::new();
        self.d.walk(root, |n, _| {
            if matches!(n.kind, NodeKind::Instance { .. }) && n.component.source.is_some() && n.component.source == instance_component(n) {
                roots.push(n.id);
            }
        });
        for r in roots {
            let Some(comp) = self.d.get(r).and_then(instance_component).and_then(|c| self.d.get(c)).map(|c| c.size) else { continue };
            if self.d.get(r).is_some_and(|n| (n.size.x - comp.x).abs() > 0.5 || (n.size.y - comp.y).abs() > 0.5) {
                self.ov(r, OverrideField::Size);
            }
        }
    }

    /// Top-level frame on a page at (x, y).
    #[allow(clippy::too_many_arguments)]
    pub fn screen_frame(&mut self, page: NodeId, name: &str, x: f32, y: f32, w: f32, h: f32, bg: Tok) -> NodeId {
        let f = self.frame(page, name);
        self.at(f, x, y);
        self.size(f, w, h);
        self.fill(f, Cl::T(bg));
        self.clip(f);
        f
    }
}

/// Soft shadow for cards on the canvas.
pub fn card_shadow() -> Effect {
    Effect::DropShadow {
        color: Color::rgba(24, 28, 70, 28),
        offset: Vec2::new(0.0, 6.0),
        radius: 20.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

pub fn floating_shadow() -> Effect {
    Effect::DropShadow {
        color: Color::rgba(20, 24, 80, 70),
        offset: Vec2::new(0.0, 14.0),
        radius: 32.0,
        spread: -4.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

/// A linear gradient from the top-left to the bottom-right corner.
pub fn grad_d(stops: &[(f32, Color)]) -> Paint {
    grad([Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0)], stops)
}

/// A linear gradient from the top edge to the bottom edge.
pub fn grad_v(stops: &[(f32, Color)]) -> Paint {
    grad([Vec2::new(0.5, 0.0), Vec2::new(0.5, 1.0), Vec2::new(1.0, 0.0)], stops)
}

fn grad(handles: [Vec2; 3], stops: &[(f32, Color)]) -> Paint {
    let stops = stops.iter().map(|(pos, color)| GradientStop { pos: *pos, color: *color }).collect();
    Paint { kind: PaintKind::LinearGradient { handles, stops }, opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

/// The showcase document.
pub fn document() -> Document {
    let mut s = S::new();
    kit::build(&mut s);
    let screens = mobile::build(&mut s);
    let dash = web::build(&mut s);
    proto::wire(&mut s, &screens);
    for id in screens.all().into_iter().chain([dash]) {
        s.relayout(id);
        s.flag_instance_sizes(id);
    }
    // Home first in the z-order so it is the file's thumbnail.
    let page = s.page(0);
    s.d.reparent(screens.home, page, 0).expect("home first");
    s.d
}

/// Files for a populated home screen: the full showcase, two smaller ones cut from it and a board.
pub fn library_samples() -> Vec<Document> {
    let full = document();
    let mut web = document();
    web.name = "Marlow - Web dashboard".into();
    web.pages.rotate_left(1);
    let mut kit = document();
    kit.name = "Marlow - Design system".into();
    kit.pages.rotate_left(2);
    let mut full = full;
    full.name = "Marlow - Mobile banking".into();
    let mut board = crate::board::demo::document();
    board.name = "Marlow - Launch brainstorm".into();
    vec![full, web, kit, board]
}

/// Writes the sample files into a projects library at `root` (screenshot aid: `--seed-library DIR`).
pub fn seed_library(root: &std::path::Path) -> Result<(), String> {
    let lib = designos_library::Library::open(root).map_err(|e| e.to_string())?;
    let project = lib.create_project("Marlow").map_err(|e| e.to_string())?;
    let names = ["Mobile banking app", "Web dashboard", "Design system", "Launch brainstorm"];
    for (doc, name) in library_samples().into_iter().zip(names) {
        let entry = lib.create_file(&project.id, None, name, &doc).map_err(|e| e.to_string())?;
        crate::home::thumb::refresh(&lib, &entry.id, &doc);
    }
    Ok(())
}

/// Test-only (`#1126` §3 rule 7 & rule 14): seeds `root` with the shipped starter stamped with a fixed `modified` timestamp.
#[cfg(test)]
pub fn seed_library_at(root: &std::path::Path, modified: u64) -> Result<(), String> {
    crate::layout_snapshot::seed_library_at(root, modified)
}
