//! The Components page: icon components and the component sets (Button, Tag, Avatar, Toggle,
//! List row, Tab bar) with variants and properties.

use super::icons::{self, Ic};
use super::*;

/// Turns the frames into main components and combines them into a set. `items` are
/// (variant name like `Type=Primary, Size=Large`, key like `Primary/Large`, frame).
fn combine(s: &mut S, set_name: &'static str, items: &[(String, String, NodeId)]) -> NodeId {
    let mut ids = Vec::new();
    for (vname, key, id) in items {
        s.relayout(*id);
        s.n(*id).name = vname.clone();
        create_component(&mut s.d, *id).apply(&mut s.d).expect("create component");
        s.variants.insert(format!("{set_name}/{key}"), *id);
        ids.push(*id);
    }
    combine_as_variants(&mut s.d, &ids).apply(&mut s.d).expect("combine variants");
    let set = s.d.get(ids[0]).and_then(|n| n.parent).expect("variant set");
    let n = s.n(set);
    n.name = set_name.into();
    n.fills = vec![Paint::solid(Color::WHITE)];
    n.corner_radii = [10.0; 4];
    n.stroke.paints = vec![Paint::solid(hex(0x06B6D4))];
    n.stroke.weight = 1.0;
    n.stroke.dashes = vec![6.0, 4.0];
    s.sets.insert(set_name, set);
    set
}

fn add_text_prop(s: &mut S, set: NodeId, name: &str, default: &str) {
    let def = ComponentPropDef { name: name.into(), kind: PropKind::Text, default: PropValue::Text(default.into()) };
    add_prop(&mut s.d, set, def).apply(&mut s.d).expect("text property");
}

fn bind(s: &mut S, node: NodeId, field: OverrideField, prop: &str) {
    bind_prop(&mut s.d, node, field, Some(prop)).apply(&mut s.d).expect("bind property");
}

fn variant_label(s: &mut S, parent: NodeId, text: &str, style: &'static str, cl: Cl, name: &str) -> NodeId {
    let t = s.text(parent, text, style, cl);
    s.n(t).name = name.into();
    t
}

fn button(s: &mut S, parent: NodeId, ty: &str, size: &str) -> NodeId {
    let large = size == "Large";
    let px = if large { 24.0 } else { 16.0 };
    let f = s.row(parent, "Button", 8.0, [0.0, px, 0.0, px]);
    s.fixed_h(f, if large { 52.0 } else { 40.0 });
    s.centered(f);
    if large {
        s.radius_var(f, false);
    } else {
        s.radius(f, 12.0);
    }
    let (label_cl, icon_cl) = match ty {
        "Primary" => {
            s.fill(f, Cl::T(Tok::Brand));
            s.style_fill(f, "Brand/Primary");
            if large {
                s.effect(
                    f,
                    Effect::DropShadow {
                        color: Color::rgba(76, 91, 240, 90),
                        offset: Vec2::new(0.0, 8.0),
                        radius: 20.0,
                        spread: -6.0,
                        visible: true,
                        show_behind: false,
                        blend: Default::default(),
                    },
                    None,
                );
            }
            (WHITE, WHITE)
        }
        "Secondary" => {
            s.fill(f, Cl::T(Tok::BrandSoft));
            s.style_fill(f, "Brand/Soft");
            (Cl::T(Tok::Brand), Cl::T(Tok::Brand))
        }
        _ => (Cl::T(Tok::Brand), Cl::T(Tok::Brand)),
    };
    let style = if large { "Text/Button" } else { "Text/Body strong" };
    variant_label(s, f, "Button", style, label_cl, "Label");
    let icon = s.vicon(f, Ic::ArrowRight, if large { 20.0 } else { 18.0 }, icon_cl);
    s.n(icon).name = "Icon".into();
    s.n(icon).visible = false;
    f
}

fn tag(s: &mut S, parent: NodeId, tone: &str) -> NodeId {
    let f = s.row(parent, "Tag", 4.0, [4.0, 10.0, 4.0, 10.0]);
    s.centered(f);
    s.radius(f, 99.0);
    let (bg, fg) = match tone {
        "Positive" => (Tok::PosSoft, Tok::Pos),
        "Negative" => (Tok::NegSoft, Tok::Neg),
        "Brand" => (Tok::BrandSoft, Tok::Brand),
        _ => (Tok::Raised, Tok::Muted),
    };
    s.fill(f, Cl::T(bg));
    variant_label(s, f, "Label", "Text/Label", Cl::T(fg), "Label");
    f
}

fn avatar(s: &mut S, parent: NodeId, size: &str) -> NodeId {
    let (px, style) = if size == "Large" { (64.0, "Text/Title") } else { (36.0, "Text/Label") };
    let f = s.row(parent, "Avatar", 0.0, [0.0; 4]);
    s.fixed(f, px, px);
    s.centered(f);
    s.radius(f, px / 2.0);
    s.fill_paint(f, grad_d(&[(0.0, hex(0x4C5BF0)), (1.0, hex(0x9A5CF6))]));
    s.clip(f);
    variant_label(s, f, "AM", style, WHITE, "Initials");
    f
}

fn toggle(s: &mut S, parent: NodeId, on: bool) -> NodeId {
    let f = s.row(parent, "Toggle", 0.0, [3.0; 4]);
    s.fixed(f, 48.0, 28.0);
    s.align(f, if on { End } else { Start }, Center);
    s.radius(f, 14.0);
    s.fill(f, Cl::T(if on { Tok::Brand } else { Tok::Faint }));
    let knob = s.ellipse(f, "Knob", 22.0, 22.0, WHITE);
    s.effect(
        knob,
        Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 50),
            offset: Vec2::new(0.0, 2.0),
            radius: 4.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
        None,
    );
    f
}

fn list_row(s: &mut S, parent: NodeId, ty: &str) -> NodeId {
    let f = s.row(parent, "List row", 12.0, [11.0, 16.0, 11.0, 16.0]);
    s.fixed_w(f, 350.0);
    s.align(f, Start, Center);
    let tile = s.row(f, "Icon tile", 0.0, [0.0; 4]);
    s.fixed(tile, 40.0, 40.0);
    s.centered(tile);
    s.radius(tile, 14.0);
    s.fill(tile, Cl::T(Tok::BrandSoft));
    let c = s.icon_comps[&Ic::Wallet];
    let icon = s.instance(tile, c);
    s.n(icon).name = "Icon".into();
    let col = s.col(f, "Text", 2.0, [0.0; 4]);
    s.fill_w(col);
    variant_label(s, col, "Title", "Text/Body strong", Cl::T(Tok::Ink), "Title");
    match ty {
        "Transaction" => {
            variant_label(s, col, "Subtitle", "Text/Caption", Cl::T(Tok::Muted), "Subtitle");
            variant_label(s, f, "$0.00", "Text/Amount", Cl::T(Tok::Ink), "Value");
        }
        "Navigation" => {
            variant_label(s, f, "Value", "Text/Caption", Cl::T(Tok::Muted), "Value");
            let chev = s.vicon(f, Ic::ChevronRight, 16.0, Cl::T(Tok::Faint));
            s.n(chev).name = "Chevron".into();
        }
        _ => {
            variant_label(s, col, "Subtitle", "Text/Caption", Cl::T(Tok::Muted), "Subtitle");
            let on = s.variant("Toggle/On");
            let t = s.instance(f, on);
            s.n(t).name = "Toggle".into();
        }
    }
    f
}

fn tab_bar(s: &mut S, parent: NodeId, active: Ic) -> NodeId {
    let f = s.row(parent, "Tab bar", 0.0, [10.0, 12.0, 26.0, 12.0]);
    s.fixed_w(f, 390.0);
    s.align(f, Start, Center);
    s.fill(f, Cl::T(Tok::Card));
    s.n(f).stroke.paints = vec![Paint::solid(light(Tok::Border))];
    s.n(f).stroke.weight = 1.0;
    s.n(f).stroke.side_weights = Some([1.0, 0.0, 0.0, 0.0]);
    let v = s.var(Tok::Border).to_string();
    s.bind(f, "strokes/0", &v);
    for (ic, label) in [(Ic::Home, "Home"), (Ic::Card, "Cards"), (Ic::Insights, "Insights"), (Ic::Profile, "Profile")] {
        let item = s.col(f, label, 4.0, [0.0; 4]);
        s.fill_w(item);
        s.align(item, Start, Center);
        let cl = Cl::T(if ic == active { Tok::Brand } else { Tok::Faint });
        s.vicon(item, ic, 24.0, cl);
        s.text(item, label, "Text/Label", cl);
    }
    f
}

/// Builds every component of the kit; fills `s.sets`, `s.variants` and `s.icon_comps`.
pub fn build(s: &mut S) {
    let page = s.page(2);
    let ds = s.frame(page, "Design system");
    s.size(ds, 1800.0, 1260.0);
    s.fill(ds, Cl::C(hex(0xFAFAFD)));

    // Icons first: other components nest them.
    for ic in icons::ALL {
        let f = s.frame(ds, &format!("Icon/{}", ic.name()));
        s.size(f, 24.0, 24.0);
        let v = s.vicon(f, ic, 24.0, Cl::T(Tok::Brand));
        s.n(v).name = ic.name().into();
        create_component(&mut s.d, f).apply(&mut s.d).expect("icon component");
        s.icon_comps.insert(ic, f);
    }

    // Toggle
    let items: Vec<_> = ["On", "Off"].iter().map(|st| (format!("State={st}"), st.to_string(), toggle(s, ds, *st == "On"))).collect();
    combine(s, "Toggle", &items);
    // keys are "Toggle/On" and "Toggle/Off"

    // Avatar
    let items: Vec<_> = ["Small", "Large"].iter().map(|sz| (format!("Size={sz}"), sz.to_string(), avatar(s, ds, sz))).collect();
    let set = combine(s, "Avatar", &items);
    add_text_prop(s, set, "Initials", "AM");
    for (_, key, id) in &items {
        let _ = key;
        let t = s.find(*id, "Initials").expect("initials");
        bind(s, t, OverrideField::Characters, "Initials");
    }

    // Tag
    let items: Vec<_> = ["Neutral", "Brand", "Positive", "Negative"].iter().map(|t| (format!("Tone={t}"), t.to_string(), tag(s, ds, t))).collect();
    let set = combine(s, "Tag", &items);
    add_text_prop(s, set, "Label", "Label");
    for (_, _, id) in &items {
        let t = s.find(*id, "Label").expect("label");
        bind(s, t, OverrideField::Characters, "Label");
    }

    // Button: Type x Size
    let mut items = Vec::new();
    for ty in ["Primary", "Secondary", "Ghost"] {
        for sz in ["Large", "Compact"] {
            items.push((format!("Type={ty}, Size={sz}"), format!("{ty}/{sz}"), button(s, ds, ty, sz)));
        }
    }
    let set = combine(s, "Button", &items);
    add_text_prop(s, set, "Label", "Button");
    add_prop(&mut s.d, set, ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) })
        .apply(&mut s.d)
        .expect("bool property");
    for (_, _, id) in &items {
        let l = s.find(*id, "Label").expect("label");
        bind(s, l, OverrideField::Characters, "Label");
        let i = s.find(*id, "Icon").expect("icon");
        bind(s, i, OverrideField::Visible, "Show icon");
    }

    // List row
    let items: Vec<_> = ["Transaction", "Navigation", "Toggle"].iter().map(|t| (format!("Type={t}"), t.to_string(), list_row(s, ds, t))).collect();
    let set = combine(s, "ListRow", &items);
    s.n(set).name = "List row".into();
    for p in ["Title", "Subtitle", "Value"] {
        add_text_prop(s, set, p, if p == "Value" { "$0.00" } else { p });
    }
    let all_icons: Vec<NodeId> = icons::ALL.iter().map(|i| s.icon_comps[i]).collect();
    let def = ComponentPropDef {
        name: "Icon".into(),
        kind: PropKind::InstanceSwap { preferred: all_icons },
        default: PropValue::Instance(s.icon_comps[&Ic::Wallet]),
    };
    add_prop(&mut s.d, set, def).apply(&mut s.d).expect("icon property");
    for (_, _, id) in &items {
        for p in ["Title", "Subtitle", "Value"] {
            if let Some(t) = s.find(*id, p) {
                bind(s, t, OverrideField::Characters, p);
            }
        }
        let icon = s.find(*id, "Icon").expect("icon instance");
        bind(s, icon, OverrideField::MainComponent, "Icon");
    }

    // Tab bar
    let items: Vec<_> = [(Ic::Home, "Home"), (Ic::Card, "Cards"), (Ic::Insights, "Insights"), (Ic::Profile, "Profile")]
        .iter()
        .map(|(ic, n)| (format!("Active={n}"), n.to_string(), tab_bar(s, ds, *ic)))
        .collect();
    let set = combine(s, "TabBar", &items);
    s.n(set).name = "Tab bar".into();

    layout_page(s, ds);
}

/// Places the sets and icons on the page frame with a label above each group.
fn layout_page(s: &mut S, ds: NodeId) {
    let title = s.text(ds, "Marlow design system", "Text/Headline", Cl::C(hex(0x12131C)));
    s.at(title, 64.0, 40.0);
    let sub = s.text(ds, "Components, variants and tokens used by the mobile and web screens.", "Text/Body", Cl::C(hex(0x6A6E85)));
    s.at(sub, 64.0, 84.0);

    let place = |s: &mut S, set: &'static str, label: &str, x: f32, y: f32| -> f32 {
        let id = s.sets[set];
        let t = s.text(ds, label, "Text/Heading", Cl::C(hex(0x12131C)));
        s.at(t, x, y);
        s.at(id, x, y + 36.0);
        x + s.n(id).size.x + 56.0
    };
    let mut x = place(s, "Button", "Button", 64.0, 150.0);
    x = place(s, "Tag", "Tag", x, 150.0);
    x = place(s, "Avatar", "Avatar", x, 150.0);
    place(s, "Toggle", "Toggle", x, 150.0);
    place(s, "ListRow", "List row", 64.0, 470.0);
    place(s, "TabBar", "Tab bar", 64.0, 680.0);

    let t = s.text(ds, "Icons", "Text/Heading", Cl::C(hex(0x12131C)));
    s.at(t, 64.0, 930.0);
    for (i, ic) in icons::ALL.iter().enumerate() {
        let (col, row) = (i % 12, i / 12);
        let cx = 64.0 + col as f32 * 118.0 + 40.0;
        let y = 984.0 + row as f32 * 112.0;
        let comp = s.icon_comps[ic];
        s.n(comp).size = Vec2::new(24.0, 24.0);
        s.at(comp, cx - 12.0, y);
        let l = s.text(ds, ic.name(), "Text/Label", Cl::C(hex(0x6A6E85)));
        let w = s.n(l).size.x;
        s.at(l, cx - w / 2.0, y + 40.0);
    }
}
