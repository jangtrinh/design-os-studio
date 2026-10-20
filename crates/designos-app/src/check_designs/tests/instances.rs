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

// ------------------------------------------------- #663 scan cap, contrast suppression & bound override

fn add_quiet_mirrors(f: &mut Fx, parent: NodeId, count: usize) -> (NodeId, NodeId) {
    let comp = component(f, "663:mirror");
    place(f, comp, 0.0, 200.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_rect = f.rect(comp, "Dot", gray(0x20));
    let inst = spawn_instance(f, comp, 0.0, 0.0);
    f.doc.reparent(inst, parent, 0).unwrap();
    // `spawn_instance` already created 1 mirror child (`Dot`); add `count - 1` more quiet mirrors.
    for _ in 1..count {
        let id = f.doc.alloc_id();
        let mut r = Node::rectangle(id, Rect::new(10.0, 10.0, 40.0, 40.0));
        r.name = "Dot".into();
        r.fills = vec![Paint::solid(gray(0x20))];
        r.component.source = Some(comp_rect);
        f.doc.insert(inst, r).unwrap();
    }
    (comp, inst)
}

#[test]
fn mirror_heavy_page_still_reaches_the_later_frame() {
    let mut f = fx();
    white_page(&mut f);
    let frame_a = f.frame(f.page, "Frame A", Some(Color::WHITE));
    add_quiet_mirrors(&mut f, frame_a, 25_000);

    let frame_b = f.frame(f.page, "Frame B", Some(Color::WHITE));
    let failing_text = f.text(frame_b, "Low contrast", 14.0, 400, gray(0x77));

    let report = f.report();
    assert!(report.complete, "mirror-heavy frame A must complete and reach frame B");
    assert!(report.scanned < 20, "scanned should count only meaningful layers, got {}", report.scanned);
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, failing_text);
}

#[test]
fn quiet_mirror_predicate_keeps_the_ancestor_opacity_row() {
    let mut f = fx();
    white_page(&mut f);
    let frame_a = f.frame(f.page, "Frame A", Some(Color::WHITE));
    add_quiet_mirrors(&mut f, frame_a, 25_000);

    // Add the 3.2 ancestor-opacity fixture after the 25,000 quiet mirrors.
    let comp = component(&mut f, "663:op-main");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_group = f.frame(comp, "Group", None);
    resize(&mut f, comp_group, 180.0, 80.0);
    place(&mut f, comp_group, 10.0, 10.0);
    let _comp_text = f.text(comp_group, "Label", 14.0, 400, gray(0x11));

    let inst = spawn_instance(&mut f, comp, 600.0, 0.0);
    let inst_group = f.doc.get(inst).unwrap().children[0];
    let inst_text = f.doc.get(inst_group).unwrap().children[0];
    {
        let g = f.doc.get_mut(inst_group).unwrap();
        g.opacity = 0.4;
        g.component.overrides.insert(OverrideField::Opacity);
    }

    let report = f.report();
    assert!(report.complete);
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, inst_text, "text under an ancestor with overridden opacity is not a quiet mirror");
}

#[test]
fn cheap_skip_changes_no_findings() {
    let assert_same = |doc: &Document, scope: &Scope, libs: &LibrarySources, label: &str| {
        let on = super::super::audit_with_cheap_skip(doc, scope, libs, true);
        let off = super::super::audit_with_cheap_skip(doc, scope, libs, false);
        assert_eq!(on.findings, off.findings, "{label}: findings diverged");
        assert_eq!(on.scanned, off.scanned, "{label}: scanned diverged");
    };

    // 1. Existing instances.rs fixtures: un-overridden instance, overridden child, transparent instance,
    //    unpublished override edit, detached layer inside main component, and ancestor opacity override.
    {
        let mut f = fx();
        white_page(&mut f);
        f.color_var("Color/Primary", BLUE, BLUE);
        let comp = component(&mut f, "cs:1");
        f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
        f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        let _comp_swatch = f.rect(comp, "Swatch", BLUE);
        let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x77));
        place(&mut f, comp_label, 60.0, 12.0);
        spawn_instance(&mut f, comp, 260.0, 0.0);
        spawn_instance(&mut f, comp, 520.0, 0.0);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "unoverridden instances");
    }
    {
        let mut f = fx();
        white_page(&mut f);
        f.color_var("Color/Primary", BLUE, BLUE);
        let comp = component(&mut f, "cs:2");
        f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
        f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        f.rect(comp, "Swatch", gray(0x20));
        let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x11));
        place(&mut f, comp_label, 60.0, 12.0);
        let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
        let kids = f.doc.get(inst).unwrap().children.clone();
        f.doc.get_mut(kids[0]).unwrap().fills = vec![Paint::solid(BLUE)];
        f.doc.get_mut(kids[0]).unwrap().component.overrides.insert(OverrideField::Fills);
        if let NodeKind::Text(t) = &mut f.doc.get_mut(kids[1]).unwrap().kind {
            t.style.fills = vec![Paint::solid(gray(0x77))];
        }
        f.doc.get_mut(kids[1]).unwrap().component.overrides.insert(OverrideField::TextStyle);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "overridden instance child");
    }
    {
        let mut f = fx();
        white_page(&mut f);
        let dark_host = f.frame(f.page, "Dark host", Some(Color::BLACK));
        resize(&mut f, dark_host, 220.0, 100.0);
        place(&mut f, dark_host, 0.0, 0.0);
        let comp = component(&mut f, "cs:3");
        f.doc.reparent(comp, dark_host, 0).unwrap();
        f.doc.get_mut(comp).unwrap().fills.clear();
        place(&mut f, comp, 10.0, 10.0);
        f.text(comp, "Badge", 14.0, 400, Color::WHITE);
        spawn_instance(&mut f, comp, 300.0, 0.0);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "transparent instance");
    }
    {
        let mut f = fx();
        white_page(&mut f);
        f.color_var("Color/Primary", BLUE, BLUE);
        let comp = component(&mut f, "cs:4");
        f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
        f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        f.rect(comp, "Swatch", gray(0x20));
        let comp_label = f.text(comp, "Caption", 14.0, 400, gray(0x11));
        place(&mut f, comp_label, 60.0, 12.0);
        let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
        let kids = f.doc.get(inst).unwrap().children.clone();
        f.doc.get_mut(kids[0]).unwrap().fills = vec![Paint::solid(BLUE)];
        if let NodeKind::Text(t) = &mut f.doc.get_mut(kids[1]).unwrap().kind {
            t.style.fills = vec![Paint::solid(gray(0x77))];
        }
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "unpublished override edit");
    }
    {
        let mut f = fx();
        let _ = card_with_detached_button(&mut f, 3);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "detached inside main");
    }
    // 2. Covering-sibling fixture (button rectangle + label).
    {
        let mut f = fx();
        white_page(&mut f);
        let r = f.rect(f.page, "Button", Color::WHITE);
        resize(&mut f, r, 120.0, 40.0);
        place(&mut f, r, 40.0, 40.0);
        let t = f.text(f.page, "Label", 14.0, 400, gray(0xBB));
        resize(&mut f, t, 70.0, 17.0);
        place(&mut f, t, 60.0, 50.0);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "covering sibling");
    }
    // 2b. Covering-sibling inside an opaque instance frame where only the sibling fill is overridden.
    {
        let mut f = fx();
        white_page(&mut f);
        let comp = component(&mut f, "cs:cov-inst");
        place(&mut f, comp, 0.0, 0.0);
        f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 80.0);
        f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        let bg = f.rect(comp, "ButtonBg", gray(0x11));
        resize(&mut f, bg, 160.0, 48.0);
        place(&mut f, bg, 20.0, 16.0);
        let lbl = f.text(comp, "Label", 14.0, 400, Color::WHITE);
        resize(&mut f, lbl, 80.0, 20.0);
        place(&mut f, lbl, 40.0, 28.0);

        let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
        let inst_bg = f.doc.get(inst).unwrap().children[0];
        f.doc.get_mut(inst_bg).unwrap().fills = vec![Paint::solid(gray(0xCC))];
        f.doc.get_mut(inst_bg).unwrap().component.overrides.insert(OverrideField::Fills);
        assert_same(&f.doc, &Scope::Page(f.page), &LibrarySources::default(), "covering sibling inside opaque instance");
    }
    // 3. Library fixture (published library + consumer with instance and plain layers).
    {
        let mut lib_fx = fx();
        lib_fx.doc.name = "Lib DS".into();
        lib_fx.color_var("Brand/Primary", BLUE, BLUE);
        let snap = Arc::new(designos_doc::snapshot(&lib_fx.doc, "lib-cs", "Lib DS", 1));
        let libs = LibrarySources {
            libs: vec![(designos_doc::LibraryRef { id: "lib-cs".into(), name: "Lib DS".into(), version: 1 }, snap)],
            known_names: Default::default(),
        };

        let mut f = fx();
        white_page(&mut f);
        f.rect(f.page, "Card", BLUE);
        let comp = component(&mut f, "cs:lib");
        f.rect(comp, "Swatch", BLUE);
        spawn_instance(&mut f, comp, 260.0, 0.0);
        assert_same(&f.doc, &Scope::Page(f.page), &libs, "library fixture");
    }
}

#[test]
fn overridden_sibling_behind_label_in_opaque_instance_reports_contrast_on_instance_text() {
    let mut f = fx();
    white_page(&mut f);
    let comp = component(&mut f, "10:cov-sibling");
    place(&mut f, comp, 0.0, 0.0);
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 80.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let bg = f.rect(comp, "ButtonBg", gray(0x11));
    resize(&mut f, bg, 160.0, 48.0);
    place(&mut f, bg, 20.0, 16.0);
    let lbl = f.text(comp, "Label", 14.0, 400, Color::WHITE);
    resize(&mut f, lbl, 80.0, 20.0);
    place(&mut f, lbl, 40.0, 28.0);

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let kids = f.doc.get(inst).unwrap().children.clone();
    let inst_bg = kids[0];
    let inst_label = kids[1];
    {
        let b = f.doc.get_mut(inst_bg).unwrap();
        b.fills = vec![Paint::solid(gray(0xCC))];
        b.component.overrides.insert(OverrideField::Fills);
    }

    let report = f.report();
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, inst_label);
}

#[test]
fn ancestor_opacity_override_reports_contrast_on_the_instance_text() {
    // 1. Main text passes (#111111 on white); in the instance, set the text's parent frame opacity to 0.4
    //    with an override -> one Rule::Contrast row on the instance text.
    let mut f = fx();
    white_page(&mut f);
    let comp = component(&mut f, "10:op-pass");
    place(&mut f, comp, 0.0, 0.0);
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_group = f.frame(comp, "Group", None);
    resize(&mut f, comp_group, 180.0, 80.0);
    place(&mut f, comp_group, 10.0, 10.0);
    let _comp_text = f.text(comp_group, "Label", 14.0, 400, gray(0x11));

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let inst_group = f.doc.get(inst).unwrap().children[0];
    let inst_text = f.doc.get(inst_group).unwrap().children[0];
    {
        let g = f.doc.get_mut(inst_group).unwrap();
        g.opacity = 0.4;
        g.component.overrides.insert(OverrideField::Opacity);
    }

    let report = f.report();
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, inst_text);

    // 2. When the main text already fails (#777777 on white, 4.47:1) and the instance's parent frame
    //    has opacity 0.4, the instance text fails with a lower ratio (1.63:1) and must NOT be suppressed
    //    just because `sf.bg == f.bg` (both sit over white).
    if let NodeKind::Text(t) = &mut f.doc.get_mut(_comp_text).unwrap().kind {
        t.style.fills = vec![Paint::solid(gray(0x77))];
    }
    if let NodeKind::Text(t) = &mut f.doc.get_mut(inst_text).unwrap().kind {
        t.style.fills = vec![Paint::solid(gray(0x77))];
    }
    let report_both = f.report();
    let contrast_nodes: Vec<NodeId> = report_both.of(Rule::Contrast).map(|x| x.node).collect();
    assert_eq!(contrast_nodes, vec![_comp_text, inst_text], "different contrast ratio in instance must not be suppressed: {:#?}", report_both.findings);
}

#[test]
fn same_opacity_on_the_main_is_reported_once_on_the_main() {
    let mut f = fx();
    white_page(&mut f);
    let comp = component(&mut f, "10:op-same");
    place(&mut f, comp, 0.0, 0.0);
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_group = f.frame(comp, "Group", None);
    resize(&mut f, comp_group, 180.0, 80.0);
    place(&mut f, comp_group, 10.0, 10.0);
    f.doc.get_mut(comp_group).unwrap().opacity = 0.4;
    let comp_text = f.text(comp_group, "Label", 14.0, 400, gray(0x11));

    let _inst_a = spawn_instance(&mut f, comp, 260.0, 0.0);
    let _inst_b = spawn_instance(&mut f, comp, 520.0, 0.0);

    let report = f.report();
    let found = only(&report, Rule::Contrast);
    assert_eq!(found.node, comp_text, "instances with the same opacity and ratio as the main stay suppressed");
}

#[test]
fn bound_main_property_replaced_by_raw_value_is_listed_with_bind_fix() {
    let accent = Color::rgb(220, 38, 38);
    let mut f = fx();
    white_page(&mut f);
    let primary_var = f.color_var("Color/Primary", BLUE, BLUE);
    let accent_var = f.color_var("Color/Accent", accent, accent);

    let comp = component(&mut f, "10:bound-main");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_swatch = f.rect(comp, "Swatch", BLUE);
    f.doc.get_mut(comp_swatch).unwrap().variables.bindings.insert("fills/0".into(), primary_var.clone());

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let inst_swatch = f.doc.get(inst).unwrap().children[0];

    // Even if the raw fill colour still equals BLUE, unbinding `fills/0` in the instance diverges from the
    // bound main property (`same_bindings` is false while `same_paints` is true) and must be listed.
    {
        let s = f.doc.get_mut(inst_swatch).unwrap();
        s.variables.bindings.remove("fills/0");
        s.component.overrides.insert(OverrideField::Fills);
    }
    let same_paint_report = f.report();
    let same_paint_finding = only(&same_paint_report, Rule::HardCodedValue);
    assert_eq!(same_paint_finding.node, inst_swatch);
    assert_eq!(same_paint_finding.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: primary_var.clone() }));

    // Now replace the instance child's fill with the raw colour of `Color/Accent` (`OverrideField::Fills`).
    f.doc.get_mut(inst_swatch).unwrap().fills = vec![Paint::solid(accent)];

    let mut app = f.app();
    let before_comp = snapshot(&app, &[comp_swatch]);
    let before_inst = snapshot(&app, &[inst_swatch]);
    let report = audit(&app.doc, &Scope::Page(app.page));
    let found = only(&report, Rule::HardCodedValue);
    assert_eq!(found.node, inst_swatch);
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: accent_var.clone() }));

    assert_eq!(app.check_designs_fix_all(&report.findings), 1);
    assert_eq!(snapshot(&app, &[comp_swatch]), before_comp, "main component must remain untouched");
    assert_eq!(app.doc.get(inst_swatch).unwrap().variables.bindings.get("fills/0"), Some(&accent_var));
    assert!(app.doc.get(inst_swatch).unwrap().component.overrides.contains(&OverrideField::Fills));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());

    sync_instances(&mut app.doc, comp).apply(&mut app.doc).unwrap();
    assert_eq!(app.doc.get(inst_swatch).unwrap().variables.bindings.get("fills/0"), Some(&accent_var));

    app.undo();
    assert_eq!(snapshot(&app, &[inst_swatch]), before_inst);
    assert!(!app.hist.can_undo());
}

#[test]
fn raw_value_matching_no_variable_is_not_listed() {
    let mut f = fx();
    white_page(&mut f);
    let primary_var = f.color_var("Color/Primary", BLUE, BLUE);
    let comp = component(&mut f, "10:unbound-none");
    f.doc.get_mut(comp).unwrap().size = Vec2::new(200.0, 100.0);
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let comp_swatch = f.rect(comp, "Swatch", BLUE);
    f.doc.get_mut(comp_swatch).unwrap().variables.bindings.insert("fills/0".into(), primary_var);

    let inst = spawn_instance(&mut f, comp, 260.0, 0.0);
    let inst_swatch = f.doc.get(inst).unwrap().children[0];
    {
        let s = f.doc.get_mut(inst_swatch).unwrap();
        s.fills = vec![Paint::solid(Color::rgb(12, 34, 56))];
        s.variables.bindings.remove("fills/0");
        s.component.overrides.insert(OverrideField::Fills);
    }

    assert!(f.report().findings.is_empty(), "a raw override colour that matches no variable is not listed");
}

#[test]
#[ignore = "release perf"]
fn measure_mirror_and_large_page_audit_perf() {
    use std::time::Instant;

    fn best_ms(mut run: impl FnMut()) -> f64 {
        let mut best = f64::MAX;
        for _ in 0..7 {
            let t0 = Instant::now();
            run();
            best = best.min(t0.elapsed().as_secs_f64() * 1000.0);
        }
        best
    }

    for target_visited in [10_000usize, 50_000, 100_000, 200_000] {
        let mut f = fx();
        white_page(&mut f);
        let frame_a = f.frame(f.page, "Frame A", Some(Color::WHITE));
        // frame_a (1) + comp (1) + comp_rect (1) + inst (1) + (target_visited - 4) mirrors = target_visited visited layers.
        add_quiet_mirrors(&mut f, frame_a, target_visited - 4);
        let libs = LibrarySources::default();
        let scope = Scope::Page(f.page);
        let ms = best_ms(|| {
            let r = audit_with_libraries(&f.doc, &scope, &libs);
            std::hint::black_box(r);
        });
        eprintln!("MIRROR_PERF visited={target_visited} best_ms={ms:.3}");
        assert!(ms < 150.0, "audit at {target_visited} visited layers took {ms:.2} ms (>= 150 ms budget)");
    }

    // 112,474-node synthetic large fixture (`large_file_perf` generator pattern: boards -> cards -> children).
    let mut f = fx();
    white_page(&mut f);
    // 1 page + 90 boards * (1 board + 25 cards * (1 card + 48 leaf layers)) = 1 + 90 * 1226 = 110,341;
    // top up with 2,133 leaf layers in one final board to reach exactly 112,474 scene nodes.
    for b_idx in 0..90 {
        let board = f.frame(f.page, &format!("Board {b_idx}"), Some(Color::rgb(0xFA, 0xFA, 0xFC)));
        for c_idx in 0..25 {
            let card = f.frame(board, &format!("Card {b_idx}-{c_idx}"), Some(Color::WHITE));
            for l_idx in 0..48 {
                f.rect(card, &format!("Item {l_idx}"), gray(0x20));
            }
        }
    }
    let tail_board = f.frame(f.page, "Board Tail", Some(Color::WHITE));
    while f.doc.nodes.len() < 112_474 {
        f.rect(tail_board, "Tail Item", gray(0x20));
    }
    assert_eq!(f.doc.nodes.len(), 112_474);
    let scope = Scope::Page(f.page);
    let libs = LibrarySources::default();
    let off_ms = best_ms(|| {
        let r = super::super::audit_with_cheap_skip(&f.doc, &scope, &libs, false);
        std::hint::black_box(r);
    });
    let on_ms = best_ms(|| {
        let r = super::super::audit_with_cheap_skip(&f.doc, &scope, &libs, true);
        std::hint::black_box(r);
    });
    eprintln!("LARGE_112K_PERF pre_change_ms={off_ms:.3} post_change_ms={on_ms:.3}");
    assert!(on_ms <= off_ms * 1.05 + 0.5, "cheap-skip must stay within 5% of pre-change time: off={off_ms:.2}ms on={on_ms:.2}ms");
}
