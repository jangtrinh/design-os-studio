//! Demo document shown when `--fixture demo` is given.
#![cfg(any(test, feature = "fixtures"))]

use crate::edit::Edit;
use crate::layout;
use crate::theme;
use designos_doc::*;

// Demo-document builder: a flat positional signature is the clearest here.
#[allow(clippy::too_many_arguments)]
fn txt(doc: &mut Document, parent: NodeId, x: f32, y: f32, s: &str, size: f32, weight: u16, color: Color) -> NodeId {
    let id = doc.alloc_id();
    let style = TextStyle { font_size: size, font_weight: weight, fills: vec![Paint::solid(color)], ..Default::default() };
    let mut n = Node::text(id, Rect::new(x, y, 10.0, 10.0), s, style);
    if let NodeKind::Text(t) = &n.kind {
        n.size = layout::estimate_text_size(t);
    }
    n.name = s.to_string();
    doc.insert(parent, n).unwrap()
}

fn shape(doc: &mut Document, parent: NodeId, n: Node, name: &str, radius: f32) -> NodeId {
    let mut n = n;
    n.name = name.into();
    n.corner_radii = [radius; 4];
    doc.insert(parent, n).unwrap()
}

/// Blank document matching the Figma "nothing selected" reference capture: a near-black
/// page and a single gradient colour style (used by `--fixture no-selection`).
pub fn reference_document() -> Document {
    let mut d = Document::new("Product UI Kit");
    let page = d.add_page("UI Kit");
    if let Some(n) = d.nodes.get_mut(&page) {
        n.kind = NodeKind::Page { background: Color::rgb(0x1E, 0x1E, 0x1E) };
    }
    let stops = vec![GradientStop { pos: 0.0, color: Color::rgb(0x0E, 0x6F, 0xC4) }, GradientStop { pos: 1.0, color: Color::rgb(0x03, 0x14, 0x30) }];
    let handles = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0)];
    let kind = PaintKind::LinearGradient { handles, stops };
    d.styles.insert("Waterfront Gradient".into(), Style::Fill(vec![Paint { kind, opacity: 1.0, visible: true, blend: BlendMode::Normal }]));
    d
}

/// Document of the Figma local-variables reference capture (`--fixture variables`): one
/// collection "Variable collection" with six colour variables.
pub fn variables_document() -> Document {
    let mut d = Document::new("Product UI Kit - Design Project");
    d.add_page("Page 1");
    let mut store = VariableStore::default();
    let coll = store.add_collection("Variable collection");
    let mode = store.collections[&coll].default_mode.clone();
    for (name, hex) in [
        ("Color aqcuian", "435391"),
        ("Color A1", "66FFD9"),
        ("Color AI blue", "0C111D"),
        ("Secondary Blue", "00B0FF"),
        ("Primary Color", "00EBE2"),
        ("Color Bue", "002552"),
    ] {
        if let Some(id) = store.add_variable(&coll, name, VariableKind::Color) {
            if let Some(c) = Color::from_hex(hex) {
                store.set_value(&id, &mode, VariableValue::Color(c));
            }
        }
    }
    let _ = store.command().apply(&mut d);
    d
}

/// Document of the library-collections capture (`--fixture variables-library`): the file's own
/// "Brand colors" collection next to a subscribed copy of a shared library's collection of the same name.
pub fn variables_library_document() -> Document {
    let mut d = Document::new("Consumer file");
    d.add_page("Page 1");
    let mut store = VariableStore::default();
    let fill = |store: &mut VariableStore, coll: &str, vars: &[(&str, &str)]| {
        let mode = store.collections[coll].default_mode.clone();
        for (name, hex) in vars {
            let id = store.add_variable(coll, name, VariableKind::Color).expect("collection exists");
            store.set_value(&id, &mode, VariableValue::Color(Color::from_hex(hex).expect("demo colour")));
        }
    };
    let local = store.add_collection("Brand colors");
    fill(&mut store, &local, &[("Brand/Ink", "1E1E1E"), ("Brand/Sea", "0C8CE9"), ("Brand/Sand", "F2E8CF")]);
    let library = store.add_collection("Brand colors");
    fill(&mut store, &library, &[("Brand/Ink", "101828"), ("Brand/Sea", "1570EF")]);
    if let Some(c) = store.collections.get_mut(&library) {
        c.source_library_key = Some("lk-shared-brand".into());
    }
    let _ = store.command().apply(&mut d);
    d
}

pub fn demo_document() -> Document {
    let mut d = Document::new("Untitled");
    let page = d.add_page("Page 1");
    d.add_page("Components");
    d.add_page("Archive");
    let ink = Color::rgb(0x1E, 0x1E, 0x1E);
    let brand = Color::rgb(0x0C, 0x8C, 0xE9);
    let grey = Color::rgb(0x8A, 0x8A, 0x8A);

    // Mobile screen
    let id = d.alloc_id();
    let mut f = Node::frame(id, Rect::new(120.0, 120.0, 360.0, 640.0));
    f.name = "Welcome".into();
    f.corner_radii = [24.0; 4];
    if let Some(p) = f.kind.frame_props_mut() {
        p.clips_content = true;
    }
    let welcome = d.insert(page, f).unwrap();
    let id = d.alloc_id();
    shape(&mut d, welcome, Node::rectangle(id, Rect::new(0.0, 0.0, 360.0, 240.0)), "Hero", 0.0);
    if let Some(n) = d.get_mut(id) {
        n.fills = vec![Paint::solid(brand)];
    }
    let id = d.alloc_id();
    let av = shape(&mut d, welcome, Node::ellipse(id, Rect::new(24.0, 200.0, 80.0, 80.0)), "Avatar", 0.0);
    if let Some(n) = d.get_mut(av) {
        n.fills = vec![Paint::solid(Color::rgb(0xF2, 0xC9, 0x4C))];
        n.stroke.paints = vec![Paint::solid(Color::WHITE)];
        n.stroke.weight = 4.0;
    }
    txt(&mut d, welcome, 24.0, 304.0, "Welcome back", 32.0, 700, ink);
    txt(&mut d, welcome, 24.0, 352.0, "Pick up where you left off.", 16.0, 400, grey);
    let id = d.alloc_id();
    let btn = shape(&mut d, welcome, Node::rectangle(id, Rect::new(24.0, 560.0, 312.0, 48.0)), "Button", 8.0);
    if let Some(n) = d.get_mut(btn) {
        n.fills = vec![Paint::solid(ink)];
    }
    txt(&mut d, welcome, 132.0, 574.0, "Continue", 16.0, 500, Color::WHITE);

    // Auto layout card
    let id = d.alloc_id();
    let mut card = Node::frame(id, Rect::new(560.0, 120.0, 10.0, 10.0));
    card.name = "Card".into();
    card.corner_radii = [12.0; 4];
    card.effects = vec![Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 40),
        offset: Vec2::new(0.0, 4.0),
        radius: 12.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }];
    card.kind = NodeKind::Frame(FrameProps {
        clips_content: false,
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 8.0,
            padding: [20.0; 4],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Fixed,
            ..AutoLayout::v1()
        }),
        grids: vec![],
    });
    card.size = Vec2::new(280.0, 10.0);
    let card = d.insert(page, card).unwrap();
    let card_title = txt(&mut d, card, 0.0, 0.0, "Quarterly report", 20.0, 600, ink);
    let card_text = txt(&mut d, card, 0.0, 0.0, "Revenue grew 18% over the last quarter.", 12.0, 400, grey);
    let id = d.alloc_id();
    let bar = shape(&mut d, card, Node::rectangle(id, Rect::new(0.0, 0.0, 240.0, 8.0)), "Progress", 4.0);
    if let Some(n) = d.get_mut(bar) {
        n.fills = vec![Paint::solid(brand)];
        n.layout_child.horizontal = Sizing::Fill;
    }
    let card_bar = bar;

    // Desktop board
    let id = d.alloc_id();
    let mut board = Node::frame(id, Rect::new(560.0, 360.0, 640.0, 400.0));
    board.name = "Dashboard".into();
    board.fills = vec![Paint::solid(Color::rgb(0xFA, 0xFA, 0xFA))];
    let board = d.insert(page, board).unwrap();
    for (i, (name, c)) in [("Tile A", 0xFF6B6B), ("Tile B", 0x4ECDC4), ("Tile C", 0xFFD166)].iter().enumerate() {
        let id = d.alloc_id();
        let t = shape(&mut d, board, Node::rectangle(id, Rect::new(24.0 + i as f32 * 200.0, 24.0, 184.0, 120.0)), name, 12.0);
        d.get_mut(t).unwrap().fills = vec![Paint::solid(Color::from_hex(&format!("{c:06X}")).unwrap())];
    }
    let id = d.alloc_id();
    let st = d.insert(board, Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.5 })).unwrap();
    {
        let n = d.get_mut(st).unwrap();
        n.transform = Affine::translate(60.0, 200.0);
        n.size = Vec2::new(120.0, 120.0);
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0xB7, 0x03))];
    }
    let id = d.alloc_id();
    let pg = d.insert(board, Node::new(id, NodeKind::Polygon { count: 3 })).unwrap();
    {
        let n = d.get_mut(pg).unwrap();
        n.transform = Affine::translate(240.0, 210.0);
        n.size = Vec2::new(100.0, 100.0);
        n.fills = vec![Paint::solid(Color::rgb(0x8E, 0x7C, 0xC3))];
    }
    txt(&mut d, board, 400.0, 220.0, "Dashboard", 28.0, 700, ink);

    // Library styles
    d.styles.insert("Brand/Primary".into(), Style::Fill(vec![Paint::solid(brand)]));
    d.styles.insert("Neutral/Ink".into(), Style::Fill(vec![Paint::solid(ink)]));
    d.styles.insert("Accent/Sun".into(), Style::Fill(vec![Paint::solid(Color::rgb(0xFF, 0xB7, 0x03))]));
    d.styles.insert(
        "Heading/Large".into(),
        Style::Text(TextStyle { font_size: 32.0, font_weight: 700, line_height: LineHeight::Pixels(40.0), ..Default::default() }),
    );
    d.styles.insert("Body/Regular".into(), Style::Text(TextStyle { font_size: 16.0, line_height: LineHeight::Pixels(24.0), ..Default::default() }));
    if let Some(p) = d.get_mut(page) {
        p.kind = NodeKind::Page { background: Color::from_hex(&format!("{:06X}", theme_page_bg())).unwrap() };
    }

    components_demo(&mut d, page, brand);
    prototype_demo(&mut d, page, brand, ink);

    let mut ed = Edit::new();
    layout::layout_node(&mut d, &mut ed, card);
    variables_demo(&mut d, card, card_title, card_text, card_bar);
    d
}

/// A "Theme" collection (Light / Dark) over a "Primitives" collection, bound to
/// the Card's fill, texts, progress bar and corner radius.
fn variables_demo(d: &mut Document, card: NodeId, title: NodeId, text: NodeId, bar: NodeId) {
    let mut s = VariableStore::default();
    // Theme first so it is the collection the panel opens on.
    let theme = s.add_collection("Theme");
    let prim = s.add_collection("Primitives");
    let pm = s.collections[&prim].default_mode.clone();
    s.rename_mode(&prim, &pm, "Value");
    let color = |s: &mut VariableStore, coll: &str, name: &str, vals: &[(&str, VariableValue)]| {
        let id = s.add_variable(coll, name, VariableKind::Color).expect("collection exists");
        for (mode, v) in vals {
            s.set_value(&id, mode, v.clone());
        }
        id
    };
    let c = |hex: &str| VariableValue::Color(Color::from_hex(hex).expect("demo colour"));
    let white = color(&mut s, &prim, "Gray/50", &[(&pm, c("FFFFFF"))]);
    let panel = color(&mut s, &prim, "Gray/900", &[(&pm, c("2C2C2C"))]);
    let blue = color(&mut s, &prim, "Blue/500", &[(&pm, c("0C8CE9"))]);

    let light = s.collections[&theme].default_mode.clone();
    s.rename_mode(&theme, &light, "Light");
    let dark = s.add_mode(&theme, "Dark").expect("mode");
    let surface = color(&mut s, &theme, "Surface/Card", &[(&light, c("FFFFFF")), (&dark, c("2C2C2C"))]);
    s.set_alias(&surface, &light, &white);
    s.set_alias(&surface, &dark, &panel);
    let ink = color(&mut s, &theme, "Text/Primary", &[(&light, c("1E1E1E")), (&dark, c("FFFFFF"))]);
    let muted = color(&mut s, &theme, "Text/Secondary", &[(&light, c("8A8A8A")), (&dark, c("B8B8B8"))]);
    let brand = color(&mut s, &theme, "Brand/Primary", &[(&light, c("0C8CE9")), (&dark, c("0C8CE9"))]);
    s.set_alias(&brand, &light, &blue);
    s.set_alias(&brand, &dark, &blue);
    let radius = s.add_variable(&theme, "Radius/Card", VariableKind::Number).expect("collection exists");
    for m in [&light, &dark] {
        s.set_value(&radius, m, VariableValue::Number(12.0));
    }
    let heading = s.add_variable(&theme, "Copy/Heading", VariableKind::String).expect("collection exists");
    s.set_value(&heading, &light, VariableValue::String("Quarterly report".into()));
    s.set_value(&heading, &dark, VariableValue::String("Quarterly report".into()));
    let compact = s.add_variable(&theme, "Flags/Compact", VariableKind::Boolean).expect("collection exists");
    s.set_value(&compact, &dark, VariableValue::Boolean(true));
    s.rename_collection(&theme, "Theme");
    d.variables = s.variables;
    d.variable_collections = s.collections;

    let bind = |d: &mut Document, id: NodeId, pairs: &[(&str, &str)]| {
        if let Some(n) = d.get_mut(id) {
            for (k, v) in pairs {
                n.variables.bindings.insert(k.to_string(), v.to_string());
            }
        }
    };
    bind(
        d,
        card,
        &[("fills/0", &surface), ("topLeftRadius", &radius), ("topRightRadius", &radius), ("bottomRightRadius", &radius), ("bottomLeftRadius", &radius)],
    );
    bind(d, title, &[("fills/0", &ink), ("characters", &heading)]);
    bind(d, text, &[("fills/0", &muted)]);
    bind(d, bar, &[("fills/0", &brand)]);
}

/// A button component (with Show icon / Label properties) on the "Components"
/// page and two instances of it on `page`.
fn components_demo(d: &mut Document, page: NodeId, brand: Color) {
    let comps = d.pages[1];
    fn run(d: &mut Document, f: impl FnOnce(&mut Document) -> Command) {
        let c = f(d);
        c.apply(d).expect("demo component command");
    }
    let id = d.alloc_id();
    let mut f = Node::frame(id, Rect::new(120.0, 120.0, 168.0, 44.0));
    f.name = "Primary button".into();
    f.fills = Vec::new();
    let main = d.insert(comps, f).unwrap();
    let id = d.alloc_id();
    let bg = shape(d, main, Node::rectangle(id, Rect::new(0.0, 0.0, 168.0, 44.0)), "Bg", 10.0);
    d.get_mut(bg).unwrap().fills = vec![Paint::solid(brand)];
    let id = d.alloc_id();
    let icon = shape(d, main, Node::ellipse(id, Rect::new(16.0, 14.0, 16.0, 16.0)), "Icon", 0.0);
    d.get_mut(icon).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let label = txt(d, main, 40.0, 12.0, "Button", 16.0, 600, Color::WHITE);
    d.get_mut(label).unwrap().name = "Label".into();
    run(d, |d| create_component(d, main));
    run(d, |d| add_prop(d, main, ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) }));
    run(d, |d| add_prop(d, main, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Button".into()) }));
    run(d, |d| bind_prop(d, icon, OverrideField::Visible, Some("Show icon")));
    run(d, |d| bind_prop(d, label, OverrideField::Characters, Some("Label")));

    for (i, (name, text, show_icon)) in [("Get started", "Get started", true), ("Learn more", "Learn more", false)].into_iter().enumerate() {
        let at = Vec2::new(880.0, 120.0 + i as f32 * 64.0);
        let cmd = create_instance(d, main, page, at);
        let Command::Insert { nodes, .. } = &cmd else { continue };
        let inst = nodes[0].id;
        cmd.apply(d).expect("demo instance");
        // Keep the original layers on top of the page's z-order.
        d.reparent(inst, page, 0).expect("demo instance order");
        let n = d.get_mut(inst).unwrap();
        n.name = name.into();
        n.component.overrides.insert(OverrideField::Name);
        run(d, |d| set_instance_prop(d, inst, "Label", Some(PropValue::Text(text.into()))));
        run(d, |d| set_instance_prop(d, inst, "Show icon", Some(PropValue::Bool(show_icon))));
        // Fit the relabelled text and restyle the second button.
        let kids = d.get(inst).unwrap().children.clone();
        for k in kids {
            let Some(kname) = d.get(k).map(|n| n.name.clone()) else { continue };
            let Some(n) = d.get_mut(k) else { continue };
            match kname.as_str() {
                "Label" => {
                    if let NodeKind::Text(t) = &n.kind {
                        n.size = layout::estimate_text_size(t);
                    }
                    if !show_icon {
                        n.transform = Affine::translate(16.0, n.transform.ty);
                        n.component.overrides.insert(OverrideField::Transform);
                    }
                }
                "Bg" if !show_icon => {
                    n.size = Vec2::new(144.0, 44.0);
                    n.fills = vec![Paint::solid(Color::rgb(0x3A, 0x3A, 0x3A))];
                    n.component.overrides.extend([OverrideField::Size, OverrideField::Fills]);
                }
                _ => {}
            }
        }
        if !show_icon {
            let n = d.get_mut(inst).unwrap();
            n.size = Vec2::new(144.0, 44.0);
            n.component.overrides.insert(OverrideField::Size);
        }
    }
}

/// Two-screen prototype: the Welcome button navigates to Dashboard with Smart
/// Animate (layers shared by name morph), Dashboard's button goes back, and a
/// flow starts at Welcome.
fn prototype_demo(d: &mut Document, page: NodeId, brand: Color, ink: Color) {
    let find = |d: &Document, name: &str| d.nodes.values().find(|n| n.name == name && n.parent == Some(page)).map(|n| n.id);
    let (Some(welcome), Some(dash)) = (find(d, "Welcome"), find(d, "Dashboard")) else { return };
    let welcome_btn = d.get(welcome).and_then(|w| w.children.iter().copied().find(|c| d.get(*c).is_some_and(|n| n.name == "Button")));
    let Some(welcome_btn) = welcome_btn else { return };

    // The Welcome button owns its label so a click on the text counts as a click on the button.
    if let Some(label) = d.get(welcome).and_then(|w| w.children.iter().copied().find(|c| d.get(*c).is_some_and(|n| n.name == "Continue"))) {
        let origin = d.get(welcome_btn).map(|b| b.transform.translation()).unwrap_or(Vec2::ZERO);
        if let Some(b) = d.get_mut(welcome_btn) {
            b.kind = NodeKind::Frame(FrameProps::default());
        }
        d.reparent(label, welcome_btn, 0).expect("label into button");
        if let Some(l) = d.get_mut(label) {
            l.transform = Affine::translate(l.transform.tx - origin.x, l.transform.ty - origin.y);
        }
    }

    // Layers with the same names as on Welcome so Smart Animate has something to morph.
    let id = d.alloc_id();
    let hero = shape(d, dash, Node::rectangle(id, Rect::new(0.0, 0.0, 640.0, 120.0)), "Hero", 0.0);
    d.get_mut(hero).unwrap().fills = vec![Paint::solid(brand)];
    d.reparent(hero, dash, 0).expect("hero behind the tiles");
    let id = d.alloc_id();
    let btn = shape(d, dash, Node::frame(id, Rect::new(464.0, 336.0, 152.0, 44.0)), "Button", 8.0);
    d.get_mut(btn).unwrap().fills = vec![Paint::solid(ink)];
    let label = txt(d, btn, 40.0, 12.0, "Back", 16.0, 500, Color::WHITE);
    d.get_mut(label).unwrap().name = "Continue".into();

    let smart = Transition { kind: TransitionKind::SmartAnimate, easing: Easing::EaseInOut, duration_ms: 400 };
    let go = |dest| Reaction { trigger: Trigger::OnClick, actions: vec![Action::Navigate { dest, preserve_scroll: false }], transition: Some(smart) };
    d.get_mut(welcome_btn).unwrap().prototype.reactions.push(go(dash));
    d.get_mut(btn).unwrap().prototype.reactions.push(go(welcome));
    d.flows.push(Flow { name: "Flow 1".into(), page, start: welcome, description: String::new() });
}

fn theme_page_bg() -> u32 {
    theme::DEFAULT_PAGE_BG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_has_pages_and_laid_out_card() {
        let d = demo_document();
        assert_eq!(d.pages.len(), 3);
        let card = d.nodes.values().find(|n| n.name == "Card").unwrap();
        assert!(card.size.y > 60.0, "card hugs its children vertically");
        let kids: Vec<f32> = card.children.iter().map(|c| d.get(*c).unwrap().transform.ty).collect();
        assert!(kids.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn demo_has_a_two_screen_prototype() {
        let d = demo_document();
        let by = |n: &str| d.nodes.values().find(|x| x.name == n).unwrap();
        let (welcome, dash) = (by("Welcome").id, by("Dashboard").id);
        assert_eq!(d.flows.len(), 1);
        assert_eq!(d.flows[0].start, welcome);
        let btn = d.get(welcome).unwrap().children.iter().map(|c| d.get(*c).unwrap()).find(|n| n.name == "Button").unwrap();
        let r = &btn.prototype.reactions[0];
        assert_eq!(r.trigger, Trigger::OnClick);
        assert_eq!(r.actions, vec![Action::Navigate { dest: dash, preserve_scroll: false }]);
        assert_eq!(r.transition.unwrap().kind, TransitionKind::SmartAnimate);
        let dash_btn = d.get(dash).unwrap().children.iter().map(|c| d.get(*c).unwrap()).find(|n| n.name == "Button").unwrap();
        assert!(matches!(dash_btn.prototype.reactions[0].actions[0], Action::Navigate { dest, .. } if dest == welcome));
    }
}
