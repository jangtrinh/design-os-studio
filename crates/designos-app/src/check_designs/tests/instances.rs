//! Auditing inside component instances (#52): un-overridden layers that merely mirror their main
//! component are reported once on the main component, while layers whose properties or effective
//! backdrop diverge in an instance are reported and fixed via instance overrides without mutating
//! the main component.

use super::*;
use designos_doc::{create_instance, instance_component, sync_instances, OverrideField, Vec2};

fn spawn_instance(f: &mut Fx, comp: NodeId, x: f32, y: f32) -> NodeId {
    let parent = f.page;
    let cmd = create_instance(&mut f.doc, comp, parent, Vec2::new(x, y));
    let root = match &cmd {
        designos_doc::Command::Insert { nodes, .. } => nodes[0].id,
        other => panic!("expected Insert from create_instance, got {other:?}"),
    };
    cmd.apply(&mut f.doc).unwrap();
    root
}

#[test]
fn unoverridden_instance_child_with_main_component_issue_is_reported_once_on_main_component() {
    let mut f = fx();
    white_page(&mut f);
    f.color_var("Color/Primary", BLUE, BLUE);
    let comp = component(&mut f, "10:1");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_swatch = f.rect(comp, "Swatch", BLUE);
    let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x77));
    place(&mut f, comp_label, 60.0, 12.0);

    let _inst_a = spawn_instance(&mut f, comp, 260.0, 0.0);
    let _inst_b = spawn_instance(&mut f, comp, 520.0, 0.0);

    let report = f.report();
    let nodes: Vec<(Rule, NodeId)> = report.findings.iter().map(|x| (x.rule, x.node)).collect();
    assert_eq!(
        nodes,
        vec![(Rule::HardCodedValue, comp_swatch), (Rule::Contrast, comp_label)],
        "main-component issues must not be duplicated per un-overridden instance: {:#?}",
        report.findings
    );
}

#[test]
fn overridden_instance_child_text_or_fill_is_reported_on_the_instance_child() {
    let mut f = fx();
    white_page(&mut f);
    f.color_var("Color/Primary", BLUE, BLUE);
    let comp = component(&mut f, "10:2");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let _comp_swatch = f.rect(comp, "Swatch", gray(0x20));
    let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x11));
    place(&mut f, comp_label, 60.0, 12.0);

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let kids = f.doc.get(inst).unwrap().children.clone();
    let (inst_swatch, inst_label) = (kids[0], kids[1]);

    // Override swatch fill to hard-coded BLUE and label colour to low-contrast #777777 on the instance.
    {
        let s = f.doc.get_mut(inst_swatch).unwrap();
        s.fills = vec![Paint::solid(BLUE)];
        s.component.overrides.insert(OverrideField::Fills);
    }
    {
        let l = f.doc.get_mut(inst_label).unwrap();
        if let NodeKind::Text(t) = &mut l.kind {
            t.style.fills = vec![Paint::solid(gray(0x77))];
        }
        l.component.overrides.insert(OverrideField::TextStyle);
    }

    let report = f.report();
    let nodes: Vec<(Rule, NodeId)> = report.findings.iter().map(|x| (x.rule, x.node)).collect();
    assert_eq!(nodes, vec![(Rule::HardCodedValue, inst_swatch), (Rule::Contrast, inst_label)], "{:#?}", report.findings);
}

#[test]
fn transparent_instance_over_a_different_backdrop_reports_contrast_failure_on_instance_child() {
    let mut f = fx();
    white_page(&mut f);
    let dark_host = f.frame(f.page, "Dark host", Some(Color::BLACK));
    resize(&mut f, dark_host, 220.0, 100.0);
    place(&mut f, dark_host, 0.0, 0.0);
    let comp = component(&mut f, "10:3");
    // Place the transparent-fill main component inside the dark host where white text passes (21:1).
    f.doc.reparent(comp, dark_host, 0).unwrap();
    f.doc.get_mut(comp).unwrap().fills.clear();
    place(&mut f, comp, 10.0, 10.0);
    let _comp_label = f.text(comp, "Badge", 14.0, 400, Color::WHITE);

    // Place an un-overridden instance on the white page (outside dark_host), where white on white fails (1:1).
    let inst = spawn_instance(&mut f, comp, 300.0, 0.0);
    let inst_label = f.doc.get(inst).unwrap().children[0];

    let report = f.report();
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, inst_label, "the instance sits on white while the main component sits on black");
}

#[test]
fn fixing_an_instance_child_records_override_without_mutating_main_component_and_undoes_cleanly() {
    let mut f = fx();
    white_page(&mut f);
    let blue_var = f.color_var("Color/Primary", BLUE, BLUE);
    let comp = component(&mut f, "10:4");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_swatch = f.rect(comp, "Swatch", gray(0x20));
    let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x11));
    place(&mut f, comp_label, 60.0, 12.0);

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let kids = f.doc.get(inst).unwrap().children.clone();
    let (inst_swatch, inst_label) = (kids[0], kids[1]);

    // Override swatch fill to BLUE (without yet recording OverrideField::Fills to prove the fix records it)
    // and override label colour to #777777.
    f.doc.get_mut(inst_swatch).unwrap().fills = vec![Paint::solid(BLUE)];
    if let NodeKind::Text(t) = &mut f.doc.get_mut(inst_label).unwrap().kind {
        t.style.fills = vec![Paint::solid(gray(0x77))];
    }

    let mut app = f.app();
    let before_comp = snapshot(&app, &[comp_swatch, comp_label]);
    let before_inst = snapshot(&app, &[inst_swatch, inst_label]);
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    assert_eq!(findings.len(), 2, "{findings:#?}");

    assert_eq!(app.check_designs_fix_all(&findings), 2);
    assert_eq!(snapshot(&app, &[comp_swatch, comp_label]), before_comp, "main component must not be mutated");
    assert_eq!(app.doc.get(inst_swatch).unwrap().variables.bindings.get("fills/0"), Some(&blue_var));
    assert!(app.doc.get(inst_swatch).unwrap().component.overrides.contains(&OverrideField::Fills));
    assert!(app.doc.get(inst_label).unwrap().component.overrides.contains(&OverrideField::TextStyle));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());

    // Syncing the main component must preserve the fixed overrides on the instance.
    sync_instances(&mut app.doc, comp).apply(&mut app.doc).unwrap();
    assert_eq!(app.doc.get(inst_swatch).unwrap().variables.bindings.get("fills/0"), Some(&blue_var));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());

    app.undo();
    assert_eq!(snapshot(&app, &[inst_swatch, inst_label]), before_inst);
    assert!(!app.hist.can_undo());
}

// ------------------------------------------------- detached layer inside a main component

/// A main component "Card" (`10:1`) that holds a frame detached from the "Button" component
/// (`12:34`) and still equal to it, plus `copies` instances of the card. Returns
/// (card, button, detached frame inside the card, the instances).
fn card_with_detached_button(f: &mut Fx, copies: usize) -> (NodeId, NodeId, NodeId, Vec<NodeId>) {
    let button = component(f, "12:34");
    f.text(button, "Button", 14.0, 400, gray(0x10));
    let card = component(f, "10:1");
    f.doc.get_mut(card).unwrap().name = "Card".into();
    f.doc.get_mut(card).unwrap().size = Vec2::new(200.0, 120.0);
    let detached = detached_copy_in(f, button, card, "Save", "guid:12:34");
    let instances = (0..copies).map(|i| spawn_instance(f, card, 260.0 * (i as f32 + 1.0), 200.0)).collect();
    (card, button, detached, instances)
}

#[test]
fn a_detached_layer_inside_a_main_component_is_reported_once_on_the_main_not_on_every_instance() {
    let mut f = fx();
    let (_card, button, detached, instances) = card_with_detached_button(&mut f, 3);
    for inst in &instances {
        let copy = f.doc.get(*inst).unwrap().children[0];
        assert_eq!(f.doc.get(copy).unwrap().component.source, Some(detached), "fixture: the instance mirrors the detached frame");
    }
    let report = f.report();
    let found: Vec<&Finding> = report.of(Rule::DetachedInstance).collect();
    assert_eq!(found.len(), 1, "{:#?}", report.findings);
    assert_eq!(found[0].node, detached);
    assert_eq!(found[0].fix, Some(Fix::Reconnect { component: button }));
}

#[test]
fn a_reconnect_on_the_main_component_survives_a_sync_of_its_instances() {
    let mut f = fx();
    let (card, button, detached, instances) = card_with_detached_button(&mut f, 3);
    let mut app = f.app();
    let finding = fix_of(&app, Rule::DetachedInstance);
    assert_eq!(finding.node, detached);
    assert!(app.check_designs_fix(&finding));
    sync_instances(&mut app.doc, card).apply(&mut app.doc).unwrap();

    let swapped = app.doc.get(card).unwrap().children.clone();
    assert_eq!(swapped.len(), 1);
    let main_inst = app.doc.get(swapped[0]).unwrap();
    assert_eq!(instance_component(main_inst), Some(button), "{:?}", main_inst.kind);
    for inst in &instances {
        let kids = app.doc.get(*inst).unwrap().children.clone();
        assert_eq!(kids.len(), 1, "each card instance mirrors the reconnected layer");
        let mirror = app.doc.get(kids[0]).unwrap();
        assert_eq!(mirror.component.source, Some(swapped[0]));
        assert_eq!(instance_component(mirror), Some(button));
    }
    assert_eq!(audit(&app.doc, &Scope::Page(app.page)).of(Rule::DetachedInstance).count(), 0);
}

#[test]
fn fix_all_reconnects_the_detached_layer_once_and_leaves_the_instance_copies_to_the_sync() {
    let mut f = fx();
    let (_card, _button, detached, instances) = card_with_detached_button(&mut f, 3);
    let mut app = f.app();
    let before: Vec<NodeId> = instances.iter().map(|i| app.doc.get(*i).unwrap().children[0]).collect();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    assert_eq!(findings.iter().filter(|x| x.fix.is_some()).count(), 1, "{findings:#?}");
    assert_eq!(app.check_designs_fix_all(&findings), 1);
    assert!(app.doc.get(detached).is_none());
    // The copies were replaced by the sync that follows the edit to the main, not rewritten in place.
    for (inst, old_copy) in instances.iter().zip(before) {
        assert!(app.doc.get(old_copy).is_none(), "the stale mirror of the detached frame is gone");
        assert_eq!(app.doc.get(*inst).unwrap().children.len(), 1);
    }
    assert_eq!(audit(&app.doc, &Scope::Page(app.page)).of(Rule::DetachedInstance).count(), 0);
    app.undo();
    assert!(app.doc.get(detached).is_some());
    assert!(!app.hist.can_undo());
}

#[test]
fn a_reconnect_aimed_at_a_mirror_inside_an_instance_is_refused() {
    let mut f = fx();
    let (_card, button, _detached, instances) = card_with_detached_button(&mut f, 1);
    let mut app = f.app();
    let mirror = app.doc.get(instances[0]).unwrap().children[0];
    let stale = Finding {
        node: mirror,
        node_name: "Save".into(),
        rule: Rule::DetachedInstance,
        property: String::new(),
        message: String::new(),
        fix: Some(Fix::Reconnect { component: button }),
        fix_label: String::new(),
    };
    assert!(!app.check_designs_fix(&stale), "the next sync of the instance would put the mirror back");
    assert!(app.doc.get(mirror).is_some());
    assert!(!app.hist.can_undo());
}

#[test]
fn an_instance_mirroring_an_unstyled_main_text_reports_no_text_style_once_on_the_main_component() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    white_page(&mut f);
    let comp = component(&mut f, "10:3");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x11));
    place(&mut f, comp_label, 60.0, 12.0);

    let _inst_a = spawn_instance(&mut f, comp, 260.0, 0.0);
    let _inst_b = spawn_instance(&mut f, comp, 520.0, 0.0);

    let report = f.report();
    let nodes: Vec<NodeId> = report.of(Rule::TextStyles).map(|x| x.node).collect();
    assert_eq!(nodes, vec![comp_label], "the text of an instance that mirrors its main is not listed again: {:#?}", report.findings);
}

fn disabled_library_fill_style(f: &mut Fx, key: &str, name: &str) {
    f.style(key, name, Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    f.doc.library.external.insert(
        format!("Design System/style:{key}"),
        designos_doc::ExternalAsset {
            library_id: "Design System".into(),
            version: 1,
            kind: designos_doc::LibraryAssetKind::Style,
            node: None,
            local_key: Some(key.into()),
        },
    );
}

#[test]
fn a_disabled_library_style_on_a_main_component_is_reported_once_not_per_instance() {
    let mut f = fx();
    white_page(&mut f);
    disabled_library_fill_style(&mut f, "Design System/s1", "Surface/Brand");
    let comp = component(&mut f, "10:5");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    let swatch = f.rect(comp, "Swatch", BLUE);
    f.doc.get_mut(swatch).unwrap().style_refs.insert("fill".into(), "Design System/s1".into());
    let _a = spawn_instance(&mut f, comp, 260.0, 0.0);
    let _b = spawn_instance(&mut f, comp, 520.0, 0.0);

    let report = f.report();
    let nodes: Vec<NodeId> = report.of(Rule::LibraryAssets).map(|x| x.node).collect();
    assert_eq!(nodes, vec![swatch], "{:#?}", report.findings);
}

#[test]
fn a_disabled_library_style_applied_inside_an_instance_is_reported_on_that_layer() {
    let mut f = fx();
    white_page(&mut f);
    disabled_library_fill_style(&mut f, "Design System/s2", "Surface/Brand");
    let comp = component(&mut f, "10:6");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    let _swatch = f.rect(comp, "Swatch", BLUE);
    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let inst_swatch = f.doc.get(inst).unwrap().children[0];
    f.doc.get_mut(inst_swatch).unwrap().style_refs.insert("fill".into(), "Design System/s2".into());

    let report = f.report();
    let nodes: Vec<NodeId> = report.of(Rule::LibraryAssets).map(|x| x.node).collect();
    assert_eq!(nodes, vec![inst_swatch], "{:#?}", report.findings);
}
