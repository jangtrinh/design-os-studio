use super::*;

/// A top-level frame (screen) by name; layer names like "Home" also name tabs and icons.
fn by_name(d: &Document, name: &str) -> NodeId {
    d.pages
        .iter()
        .flat_map(|p| d.get(*p).unwrap().children.iter().copied())
        .find(|c| d.get(*c).unwrap().name == name)
        .unwrap_or_else(|| panic!("no screen named {name}"))
}

#[test]
fn builds_with_the_expected_pages_and_screens() {
    let d = document();
    let pages: Vec<_> = d.pages.iter().map(|p| d.get(*p).unwrap().name.as_str()).collect();
    assert_eq!(pages, ["Mobile", "Web", "Components"]);
    let mobile = d.get(d.pages[0]).unwrap();
    let screens: Vec<_> = mobile.children.iter().map(|c| d.get(*c).unwrap()).collect();
    assert_eq!(screens.len(), 4);
    for n in &screens {
        assert_eq!((n.size.x, n.size.y), (390.0, 844.0), "{} is phone sized", n.name);
    }
    let web = d.get(d.get(d.pages[1]).unwrap().children[0]).unwrap();
    assert_eq!((web.size.x, web.name.as_str()), (1440.0, "Web dashboard"));
}

#[test]
fn has_the_component_sets_with_variants_and_properties() {
    let d = document();
    let set = |name: &str| d.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::ComponentSet(_))).unwrap_or_else(|| panic!("no set {name}"));
    for (name, variants) in [("Button", 6), ("Tag", 4), ("Avatar", 2), ("Toggle", 2), ("List row", 3), ("Tab bar", 4)] {
        assert_eq!(set(name).children.len(), variants, "{name} variants");
    }
    let button = set("Button");
    let props: Vec<_> = button.component.props.iter().map(|p| p.name.as_str()).collect();
    assert!(props.contains(&"Type") && props.contains(&"Size") && props.contains(&"Label") && props.contains(&"Show icon"), "{props:?}");
    assert!(set("List row").component.props.iter().any(|p| matches!(p.kind, PropKind::InstanceSwap { .. })));
    let icons = d.nodes.values().filter(|n| n.name.starts_with("Icon/") && matches!(n.kind, NodeKind::Component(_))).count();
    assert_eq!(icons, icons::ALL.len());
}

#[test]
fn every_instance_resolves_to_a_main_component() {
    let d = document();
    let mut instances = 0;
    for n in d.nodes.values() {
        if let NodeKind::Instance { component, .. } = &n.kind {
            instances += 1;
            let c = component.unwrap_or_else(|| panic!("instance {} has no component", n.name));
            let main = d.get(c).unwrap_or_else(|| panic!("instance {} points at a missing node", n.name));
            assert!(matches!(main.kind, NodeKind::Component(_)), "instance {} points at a non-component", n.name);
        }
        if let Some(src) = n.component.source {
            assert!(d.get(src).is_some(), "{} mirrors a missing node", n.name);
        }
    }
    assert!(instances > 60, "the screens use plenty of instances, got {instances}");
}

#[test]
fn overrides_show_in_the_instances() {
    let d = document();
    let home = by_name(&d, "Home");
    let mut found = Vec::new();
    d.walk(home, |n, _| {
        if n.name == "Value" && matches!(&n.kind, NodeKind::Text(t) if t.content == "+$3,850.00") {
            found.push(n.component.overrides.clone());
        }
    });
    assert_eq!(found.len(), 1);
    assert!(found[0].contains(&OverrideField::TextStyle), "the positive amount recolors its text");
}

#[test]
fn auto_layout_content_fits_inside_fixed_frames() {
    let d = document();
    for n in d.nodes.values() {
        if n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_none() {
            continue;
        }
        for c in n.children.iter().filter_map(|c| d.get(*c)).filter(|c| !c.layout_child.absolute && c.visible) {
            let (r, b) = (c.transform.tx + c.size.x, c.transform.ty + c.size.y);
            assert!(
                c.transform.tx >= -0.5 && c.transform.ty >= -0.5,
                "{} at ({}, {}) size {:?} starts outside {} {:?}",
                c.name,
                c.transform.tx,
                c.transform.ty,
                c.size,
                n.name,
                n.size
            );
            assert!(r <= n.size.x + 0.6 && b <= n.size.y + 0.6, "{} ({r:.1} x {b:.1}) overflows {} ({} x {})", c.name, n.name, n.size.x, n.size.y);
        }
    }
}

#[test]
fn theme_variables_have_both_modes_and_are_bound() {
    let d = document();
    let theme = d.variable_collections.values().find(|c| c.name == "Theme").expect("Theme collection");
    let modes: Vec<_> = theme.modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(modes, ["Light", "Dark"]);
    let bound = d.nodes.values().filter(|n| n.variables.bindings.contains_key("fills/0")).count();
    assert!(bound > 200, "surfaces and text are bound to theme variables, got {bound}");
    // the settings screen is switched to Dark and resolves differently from home
    let (home, settings) = (by_name(&d, "Home"), by_name(&d, "Settings"));
    let bg = |screen: NodeId| resolved_node(&d, screen).unwrap().fills[0].kind.clone();
    assert_ne!(bg(home), bg(settings));
}

#[test]
fn styles_flows_and_prototype_links() {
    let d = document();
    for key in ["Brand/Primary", "Text/Title", "Text/Body", "Gradient/Brand", "Shadow/Card"] {
        assert!(d.styles.contains_key(key), "style {key}");
    }
    assert_eq!(d.flows.len(), 2);
    let screens: Vec<NodeId> = ["Welcome", "Home", "Spending", "Settings"].iter().map(|n| by_name(&d, n)).collect();
    assert_eq!(d.flows[0].start, screens[0]);
    let links = d.nodes.values().filter(|n| !n.prototype.reactions.is_empty()).count();
    assert!(links >= 5, "{links} interactive layers");
    for n in d.nodes.values() {
        for r in &n.prototype.reactions {
            for a in &r.actions {
                if let Action::Navigate { dest, .. } = a {
                    assert!(screens.contains(dest), "{} navigates to a screen", n.name);
                }
            }
        }
    }
}

#[test]
fn no_third_party_names_or_images() {
    let d = document();
    assert!(d.images.is_empty(), "no embedded artwork");
    for n in d.nodes.values() {
        let mut texts = vec![n.name.clone()];
        if let NodeKind::Text(t) = &n.kind {
            texts.push(t.content.clone());
        }
        for t in texts {
            for banned in ["figma", "apple", "google", "visa", "mastercard", "stripe", "paypal", "amazon", "starbucks"] {
                assert!(!t.to_lowercase().contains(banned), "'{t}' mentions {banned}");
            }
        }
        assert!(n.fills.iter().all(|p| !matches!(p.kind, PaintKind::Image { .. })), "{} has an image fill", n.name);
    }
}

#[test]
fn library_samples_lead_with_a_frame_worth_a_thumbnail() {
    let docs = library_samples();
    assert_eq!(docs.len(), 4);
    for d in &docs {
        assert!(crate::home::thumb::thumbnail_png(d).is_some(), "{} has a thumbnail", d.name);
    }
}
