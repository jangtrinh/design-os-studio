//! "Fix all" must never report a fix that a later step of the same run throws away, and must never
//! drop a finding without saying so. A re-link to a main component runs last for that reason.

use super::*;
use designos_doc::{OverrideField, Vec2};

fn label_color(app: &App, id: NodeId) -> Option<Color> {
    let n = app.doc.get(id)?;
    let NodeKind::Text(t) = &n.kind else { return None };
    match t.style.fills.first().map(|p| &p.kind) {
        Some(PaintKind::Solid(c)) => Some(*c),
        _ => None,
    }
}

/// A main component with a white card fill and a label that fails AA, big enough to hold the label.
fn card_component(f: &mut Fx, source_id: &str) -> NodeId {
    let comp = component(f, source_id);
    let n = f.doc.get_mut(comp).unwrap();
    n.size = Vec2::new(200.0, 60.0);
    n.fills = vec![Paint::solid(Color::WHITE)];
    f.text(comp, "Button", 14.0, 400, gray(0x77));
    comp
}

#[test]
fn fix_all_on_a_detached_frame_keeps_the_contrast_fix_of_its_label() {
    let mut f = fx();
    let comp = card_component(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let label = f.doc.get(frame).unwrap().children[0];
    let mut app = f.app();
    // Only the detached frame is checked, the main component is not in scope.
    let scope = Scope::Layers(vec![frame]);
    let findings = audit(&app.doc, &scope).findings;
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert!(findings.iter().any(|x| x.rule == Rule::Contrast && x.node == label), "{findings:#?}");
    assert!(findings.iter().any(|x| x.rule == Rule::DetachedInstance && x.node == frame), "{findings:#?}");
    let applied = app.check_designs_fix_all(&findings);
    let recoloured = label_color(&app, label).expect("the label layer is still there");
    assert_ne!(recoloured, gray(0x77), "the label must not be left at the colour the audit flagged");
    assert!(audit(&app.doc, &scope).of(Rule::Contrast).next().is_none(), "the label passes now");
    assert_eq!(applied, 1, "one fix landed: the recolour");
    assert!(app.doc.get(frame).is_some(), "a reconnect would have thrown the recolour away, so the frame stays");
    let again = audit(&app.doc, &scope);
    assert_eq!(only(&again, Rule::DetachedInstance).fix, None, "and it says why it is not reconnected");
    app.undo();
    assert_eq!(label_color(&app, label), Some(gray(0x77)));
    assert!(!app.hist.can_undo(), "Fix all is a single undo step");
}

#[test]
fn fix_all_on_a_page_recolours_both_copies_then_reconnects() {
    let mut f = fx();
    let comp = card_component(&mut f, "12:34");
    let comp_label = f.doc.get(comp).unwrap().children[0];
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let mut app = f.app();
    let page = Scope::Page(app.page);
    let findings = audit(&app.doc, &page).findings;
    assert_eq!(findings.len(), 3, "{findings:#?}");
    assert_eq!(app.check_designs_fix_all(&findings), 3);
    assert_ne!(label_color(&app, comp_label), Some(gray(0x77)), "the main component's label is recoloured");
    assert!(app.doc.get(frame).is_none(), "both copies were recoloured the same, so the frame equals the component again and is swapped");
    let after = audit(&app.doc, &page);
    assert!(after.findings.is_empty(), "{:#?}", after.findings);
    app.undo();
    assert_eq!(label_color(&app, comp_label), Some(gray(0x77)));
    assert!(app.doc.get(frame).is_some());
    assert!(!app.hist.can_undo());
}

#[test]
fn fix_all_keeps_the_own_name_of_a_frame_it_reconnects_after_editing_the_component() {
    let mut f = fx();
    let comp = card_component(&mut f, "12:34");
    detached_copy(&mut f, comp, "Save", "guid:12:34");
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    // The component is edited in the same step, so its instances are synced from it afterwards.
    assert_eq!(app.check_designs_fix_all(&findings), 3);
    let page = app.doc.get(app.page).unwrap();
    let instance =
        page.children.iter().filter_map(|c| app.doc.get(*c)).find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("the frame became an instance");
    assert_eq!(instance.name, "Save", "the sync must not rename it to the component's name");
    assert!(
        instance.component.overrides.contains(&OverrideField::Name),
        "the name is recorded as the instance's own, so a later rename of the component leaves it"
    );
}

#[test]
fn fix_all_on_a_detached_frame_keeps_a_bind_made_inside_it() {
    let mut f = fx();
    let comp = component(&mut f, "12:34");
    f.doc.get_mut(comp).unwrap().fills = vec![Paint::solid(BLUE)];
    let var = f.color_var("Color/Primary", BLUE, BLUE);
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let mut app = f.app();
    let scope = Scope::Layers(vec![frame]);
    let findings = audit(&app.doc, &scope).findings;
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert!(findings.iter().any(|x| x.rule == Rule::HardCodedValue && x.node == frame), "{findings:#?}");
    assert!(findings.iter().any(|x| x.rule == Rule::DetachedInstance && x.node == frame), "{findings:#?}");
    assert_eq!(app.check_designs_fix_all(&findings), 1);
    let kept = app.doc.get(frame).expect("the frame is not swapped for an instance that would lose the bind");
    assert_eq!(kept.variables.bindings.get("fills/0"), Some(&var));
}
