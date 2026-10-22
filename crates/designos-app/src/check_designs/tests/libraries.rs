//! Tests for suggesting variables and styles from enabled libraries (Issue #155, Task E3-11).

use super::*;
use designos_doc::{AutoLayout, Effect, ExternalAsset, LibraryAssetKind, LibraryRef, LineHeight};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn tmp_lib_root(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("designos-check-lib-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn publish_design_system(root: &Path, name: &str) -> String {
    publish_design_system_with_dark(root, name, BLUE)
}

/// A library whose `Brand/Primary` is BLUE in the light (default) mode and `dark` in the dark mode.
fn publish_design_system_with_dark(root: &Path, name: &str, dark: Color) -> String {
    let mut f = fx();
    f.doc.name = name.into();
    f.color_var("Brand/Primary", BLUE, dark);
    let coll = f.doc.variable_collections.get_mut("theme").unwrap();
    let num_id = "var:space/4".to_string();
    coll.variables.push(num_id.clone());
    let values = [("light".to_string(), VariableValue::Number(16.0)), ("dark".to_string(), VariableValue::Number(16.0))].into_iter().collect();
    f.doc.variables.insert(
        num_id.clone(),
        Variable {
            id: num_id,
            name: "space/4".into(),
            collection: "theme".into(),
            kind: VariableKind::Number,
            values,
            description: String::new(),
            scopes: vec!["GAP".into()],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        },
    );
    let mut app = f.app();
    app.libs.set_root(root.to_path_buf());
    assert!(app.publish_file());
    app.doc.library.published.as_ref().unwrap().library_id.clone()
}

#[test]
fn library_variable_is_suggested_with_its_library_name() {
    let root = tmp_lib_root("vars");
    let lib_id = publish_design_system(&root, "Brand DS");

    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });

    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);
    assert!(app.doc.variables.is_empty(), "library variables are not copied until used");

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let findings: Vec<Finding> = app.check_designs.report.findings.clone();
    assert_eq!(findings.len(), 2);

    let fill_finding = findings.iter().find(|x| x.node == card).unwrap();
    assert_eq!(fill_finding.rule, Rule::HardCodedValue);
    assert_eq!(fill_finding.message, "Fill #1A73E8 matches variable \"Brand/Primary\" from library \"Brand DS\"");
    assert_eq!(fill_finding.fix_label, "Bind to variable Brand/Primary from library Brand DS");
    assert!(fill_finding.is_from_library());

    let gap_finding = findings.iter().find(|x| x.node == row).unwrap();
    assert_eq!(gap_finding.rule, Rule::HorizontalSpacing);
    assert_eq!(gap_finding.message, "Gap 16 matches variable \"space/4\" from library \"Brand DS\"");
    assert_eq!(gap_finding.fix_label, "Bind to variable space/4 from library Brand DS");
    assert!(gap_finding.is_from_library());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn library_fix_imports_and_binds_in_one_undo_step() {
    let root = tmp_lib_root("fix");
    let lib_id = publish_design_system(&root, "Brand DS");

    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });

    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let findings: Vec<Finding> = app.check_designs.report.findings.clone();
    let fill_finding = findings.iter().find(|x| x.node == card).unwrap();

    // Single finding fix imports + binds in one undo step.
    assert!(app.check_designs_fix(fill_finding));
    let local_color = format!("{lib_id}/var:Brand/Primary");
    assert_eq!(app.doc.get(card).unwrap().variables.bindings.get("fills/0"), Some(&local_color));
    assert!(app.doc.variables.contains_key(&local_color));
    assert!(app.doc.library.external.contains_key(&format!("{lib_id}/var:var:Brand/Primary")));

    app.undo();
    assert!(app.doc.get(card).unwrap().variables.bindings.is_empty());
    assert!(app.doc.variables.is_empty() && app.doc.variable_collections.is_empty(), "one undo removes both binding and imported variable");
    assert!(app.doc.library.external.is_empty());

    // Fix all imports and binds both in one undo step.
    assert_eq!(app.check_designs_fix_all(&findings), 2);
    let local_gap = format!("{lib_id}/var:space/4");
    assert_eq!(app.doc.get(card).unwrap().variables.bindings.get("fills/0"), Some(&local_color));
    assert_eq!(app.doc.get(row).unwrap().variables.bindings.get("itemSpacing"), Some(&local_gap));

    app.undo();
    assert!(app.doc.get(card).unwrap().variables.bindings.is_empty());
    assert!(app.doc.get(row).unwrap().variables.bindings.is_empty());
    assert!(app.doc.variables.is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn library_fill_text_and_effect_styles_are_suggested_with_their_library_name_and_applied_in_one_undo_step() {
    let root = tmp_lib_root("styles");
    let mut lib_fx = fx();
    lib_fx.doc.name = "Brand Styles".into();
    lib_fx.style("s_fill", "Surface/Card", Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    let ts = TextStyle { font_size: 18.0, font_weight: 600, line_height: LineHeight::Pixels(24.0), ..TextStyle::default() };
    lib_fx.style("s_text", "Body/Lead", Style::Text(ts.clone()), StyleProperty::Text);
    let fx_list =
        vec![Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false }];
    lib_fx.style("s_fx", "Elevation/Low", Style::Effect(fx_list.clone()), StyleProperty::Effect);
    let mut lib_app = lib_fx.app();
    lib_app.libs.set_root(root.clone());
    assert!(lib_app.publish_file());
    let lib_id = lib_app.doc.library.published.as_ref().unwrap().library_id.clone();

    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    white_page(&mut f);
    let card = f.rect(f.page, "Card", BLUE);
    f.doc.get_mut(card).unwrap().effects = fx_list;
    let label = f.text(f.page, "Headline", 18.0, 600, gray(0x11));
    if let NodeKind::Text(t) = &mut f.doc.get_mut(label).unwrap().kind {
        t.style.line_height = LineHeight::Pixels(24.0);
        for r in &mut t.runs {
            r.style.line_height = LineHeight::Pixels(24.0);
        }
    }

    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);
    assert!(app.doc.styles.is_empty());

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let findings: Vec<Finding> = app.check_designs.report.findings.clone();
    assert_eq!(findings.len(), 3);

    let fill_f = findings.iter().find(|x| x.node == card && x.message.starts_with("Fill ")).unwrap();
    assert_eq!(fill_f.message, "Fill matches style \"Surface/Card\" from library \"Brand Styles\"");
    assert_eq!(fill_f.fix_label, "Apply style Surface/Card from library Brand Styles");

    let fx_f = findings.iter().find(|x| x.node == card && x.message.starts_with("Effects ")).unwrap();
    assert_eq!(fx_f.message, "Effects match style \"Elevation/Low\" from library \"Brand Styles\"");
    assert_eq!(fx_f.fix_label, "Apply style Elevation/Low from library Brand Styles");

    let text_f = findings.iter().find(|x| x.node == label).unwrap();
    assert_eq!(text_f.message, "Text matches style \"Body/Lead\" from library \"Brand Styles\"");
    assert_eq!(text_f.fix_label, "Apply style Body/Lead from library Brand Styles");

    assert_eq!(app.check_designs_fix_all(&findings), 3);
    assert_eq!(app.doc.get(card).unwrap().style_refs.get("fill"), Some(&format!("{lib_id}/s_fill")));
    assert_eq!(app.doc.get(card).unwrap().style_refs.get("effect"), Some(&format!("{lib_id}/s_fx")));
    assert_eq!(app.doc.get(label).unwrap().style_refs.get("text"), Some(&format!("{lib_id}/s_text")));
    assert_eq!(app.doc.styles.len(), 3);

    app.undo();
    assert!(app.doc.get(card).unwrap().style_refs.is_empty());
    assert!(app.doc.get(label).unwrap().style_refs.is_empty());
    assert!(app.doc.styles.is_empty(), "undo removes imported styles and links in one step");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn disabled_library_is_never_suggested() {
    let root = tmp_lib_root("disabled");
    let lib_id = publish_design_system(&root, "Optional Kit");

    let mut f = fx();
    let first = f.rect(f.page, "First", BLUE);
    let mut app = f.app();
    app.libs.set_root(root.clone());

    // 1. Not enabled yet -> never suggested.
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert!(app.check_designs.report.of(Rule::HardCodedValue).next().is_none());

    // 2. Enable and fix `first` so the library variable is imported into `app.doc.variables`.
    app.set_library_enabled(&lib_id, true);
    app.refresh_check_designs(false, &egui::Context::default());
    let finding = app.check_designs.report.of(Rule::HardCodedValue).next().cloned().unwrap();
    assert!(app.check_designs_fix(&finding));
    assert!(!app.doc.variables.is_empty());
    assert!(app.doc.get(first).unwrap().variables.bindings.contains_key("fills/0"));

    // 3. Disable the library and add a second unbound BLUE rectangle.
    app.set_library_enabled(&lib_id, false);
    let second_id = app.doc.alloc_id();
    let mut second = Node::rectangle(second_id, Rect::new(80.0, 10.0, 40.0, 40.0));
    second.name = "Second".into();
    second.fills = vec![Paint::solid(BLUE)];
    app.doc.insert(app.page, second).unwrap();
    app.check_designs.invalidate();
    app.refresh_check_designs(false, &egui::Context::default());
    assert!(
        app.check_designs.report.of(Rule::HardCodedValue).next().is_none(),
        "disabled library variable must not be suggested even when a local copy exists"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn local_candidate_beats_library_candidate() {
    let root = tmp_lib_root("local_wins");
    let lib_id = publish_design_system(&root, "Brand DS");

    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    let local_var = f.color_var("blue/500", BLUE, BLUE);
    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let found = only(&app.check_designs.report, Rule::HardCodedValue);
    assert_eq!(found.node, card);
    assert_eq!(found.message, "Fill #1A73E8 matches variable \"blue/500\"");
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: local_var }));
    assert!(!found.is_from_library());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn two_libraries_with_same_value_are_ambiguous() {
    let root = tmp_lib_root("ambig");
    let lib_a = publish_design_system(&root, "Alpha UI");
    let lib_b = publish_design_system(&root, "Beta UI");

    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_a, true);
    app.set_library_enabled(&lib_b, true);

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let found = only(&app.check_designs.report, Rule::HardCodedValue);
    assert_eq!(found.fix, None);
    assert_eq!(found.message, "Fill #1A73E8 matches 2 variables: \"Brand/Primary\" from library \"Alpha UI\", \"Brand/Primary\" from library \"Beta UI\"");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pinned_version_is_used_not_latest() {
    let root = tmp_lib_root("pinned");
    // Publish v1 with Brand/Primary = BLUE.
    let mut lib_fx = fx();
    lib_fx.doc.name = "Brand DS".into();
    let var_id = lib_fx.color_var("Brand/Primary", BLUE, BLUE);
    let mut lib_app = lib_fx.app();
    lib_app.libs.set_root(root.clone());
    assert!(lib_app.publish_file());
    let lib_id = lib_app.doc.library.published.as_ref().unwrap().library_id.clone();

    // Consumer enables v1.
    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);
    assert_eq!(app.doc.library.enabled[0].version, 1);

    // Library publishes v2 where Brand/Primary changes to RED.
    lib_app.doc.variables.get_mut(&var_id).unwrap().values =
        [("light".to_string(), VariableValue::Color(Color::rgb(220, 38, 38))), ("dark".to_string(), VariableValue::Color(Color::rgb(220, 38, 38)))]
            .into_iter()
            .collect();
    assert!(lib_app.publish_file());
    assert_eq!(lib_app.doc.library.published.as_ref().unwrap().version, 2);

    // Consumer is still pinned at v1 (BLUE), so BLUE card still matches Brand/Primary at v1!
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let found = only(&app.check_designs.report, Rule::HardCodedValue);
    assert_eq!(found.node, card);
    assert_eq!(found.message, "Fill #1A73E8 matches variable \"Brand/Primary\" from library \"Brand DS\"");
    assert!(app.check_designs_fix(&found));
    assert_eq!(app.doc.resolve_variable(card, &format!("{lib_id}/var:Brand/Primary")), Some(VariableValue::Color(BLUE)));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unreadable_library_is_reported_and_skipped() {
    let root = tmp_lib_root("unreadable");
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    let mut app = f.app();
    app.libs.set_root(root.clone());
    // Enable a library whose snapshot does not exist on disk.
    app.doc.library.enabled.push(LibraryRef { id: "missing-lib".into(), name: "Missing Kit".into(), version: 1 });

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.report.unreadable_libraries, vec!["Missing Kit".to_string()]);
    assert!(app.check_designs.report.findings.is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn figma_keyed_collection_is_still_suggested_and_fixable() {
    // A `.fig` import stores Figma's own `sourceLibraryKey` on the collection. That key names no
    // Studio library, so the variable stays a local candidate.
    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    let var = f.color_var("Brand/Primary", BLUE, BLUE);
    f.doc.variable_collections.get_mut("theme").unwrap().source_library_key = Some("lk-83c0".into());
    let mut app = f.app();

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let found = only(&app.check_designs.report, Rule::HardCodedValue);
    assert_eq!(found.node, card);
    assert_eq!(found.message, "Fill #1A73E8 matches variable \"Brand/Primary\"");
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: var.clone() }));

    assert!(app.check_designs_fix(&found));
    assert_eq!(app.doc.get(card).unwrap().variables.bindings.get("fills/0"), Some(&var));
}

#[test]
fn collection_of_a_disabled_studio_library_is_not_suggested_even_when_keyed_by_its_id() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    f.color_var("Brand/Primary", BLUE, BLUE);
    f.doc.variable_collections.get_mut("theme").unwrap().source_library_key = Some("studio-lib".into());
    // The library was enabled once: an import recorded it.
    f.doc.library.external.insert(
        "studio-lib/var:other".into(),
        ExternalAsset { library_id: "studio-lib".into(), version: 1, kind: LibraryAssetKind::Variable, node: None, local_key: Some("var:other".into()) },
    );
    let mut app = f.app();

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert!(app.check_designs.report.of(Rule::HardCodedValue).next().is_none());
}

#[test]
fn library_default_mode_that_is_not_first_decides_the_suggestion() {
    let root = tmp_lib_root("default-mode");
    let red = Color::rgb(220, 38, 38);
    let mut lib_fx = fx();
    lib_fx.doc.name = "Modes DS".into();
    lib_fx.color_var("Brand/Primary", red, BLUE);
    lib_fx.doc.variable_collections.get_mut("theme").unwrap().default_mode = "dark".into();
    let mut lib_app = lib_fx.app();
    lib_app.libs.set_root(root.clone());
    assert!(lib_app.publish_file());
    let lib_id = lib_app.doc.library.published.as_ref().unwrap().library_id.clone();

    let mut f = fx();
    let in_default = f.rect(f.page, "Default mode colour", BLUE);
    f.rect(f.page, "First mode colour", red);
    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);

    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let found = only(&app.check_designs.report, Rule::HardCodedValue);
    assert_eq!(found.node, in_default, "only the default-mode colour is suggested");
    assert_eq!(found.message, "Fill #1A73E8 matches variable \"Brand/Primary\" from library \"Modes DS\"");

    assert!(app.check_designs_fix(&found));
    let local = format!("{lib_id}/var:Brand/Primary");
    assert_eq!(app.doc.get(in_default).unwrap().variables.bindings.get("fills/0"), Some(&local));
    assert_eq!(app.doc.resolve_variable(in_default, &local), Some(VariableValue::Color(BLUE)));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn refused_library_bind_in_fix_all_leaves_no_imported_variable() {
    let root = tmp_lib_root("refused");
    let lib_id = publish_design_system_with_dark(&root, "Brand DS", Color::rgb(220, 38, 38));

    let mut f = fx();
    let group = f.frame(f.page, "Group", None);
    let stale = f.rect(group, "Stale", BLUE);
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });
    let mut app = f.app();
    app.libs.set_root(root.clone());
    app.set_library_enabled(&lib_id, true);
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    let findings: Vec<Finding> = app.check_designs.report.findings.clone();
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert!(findings.iter().any(|f| f.rule == Rule::HardCodedValue && f.node == stale), "the stale fill is reported under colours");
    assert!(findings.iter().any(|f| f.rule == Rule::HorizontalSpacing && f.node == row), "the horizontal stack's gap is reported under horizontal spacing");

    // The group switches the library collection to dark mode after the audit, so once imported the
    // variable resolves to red there and the bind is refused.
    app.doc.get_mut(group).unwrap().variables.modes.insert(format!("{lib_id}/theme"), "dark".into());

    // The gap fix lands and imports its own variable (and the collection). The refused fill fix
    // must leave nothing of its import behind.
    assert_eq!(app.check_designs_fix_all(&findings), 1, "only the gap fix lands");
    assert!(app.doc.get(stale).unwrap().variables.bindings.is_empty());
    assert!(!app.doc.variables.contains_key(&format!("{lib_id}/var:Brand/Primary")), "refused bind must not leave the imported variable");
    assert!(!app.doc.library.external.contains_key(&format!("{lib_id}/var:var:Brand/Primary")));
    assert_eq!(app.doc.get(row).unwrap().variables.bindings.get("itemSpacing"), Some(&format!("{lib_id}/var:space/4")));

    let _ = std::fs::remove_dir_all(root);
}
