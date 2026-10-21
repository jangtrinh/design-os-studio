//! Tests for Figma's rule set, AA/AAA contrast thresholds, and persistent ignore / session dismiss
//! (Issue #158, Task E3-13).

use super::*;
use designos_doc::{AutoLayout, ContrastLevel, ExternalAsset, IgnoreKey, LayoutDirection, LibraryAssetKind};

#[test]
fn rule_figma_ids_match_table() {
    assert_eq!(Rule::HardCodedValue.figma_id(), "RECOMMEND_VARIABLES");
    assert_eq!(Rule::Contrast.figma_id(), "CONTRAST");
    assert_eq!(Rule::DetachedInstance.figma_id(), "REQUIRE_NO_DETACHED_COMPONENTS");
    assert_eq!(Rule::LibraryAssets.figma_id(), "REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES");
    assert_eq!(Rule::TextStyles.figma_id(), "REQUIRE_TEXT_STYLES");
    assert_eq!(Rule::CornerRadius.figma_id(), "REQUIRE_CORNER_RADIUS_VARIABLES");
    assert_eq!(Rule::Padding.figma_id(), "REQUIRE_PADDING_VARIABLES");
    assert_eq!(Rule::HorizontalSpacing.figma_id(), "REQUIRE_HORIZONTAL_SPACING_VARIABLES");
    assert_eq!(Rule::VerticalSpacing.figma_id(), "REQUIRE_VERTICAL_SPACING_VARIABLES");
    assert_eq!(Rule::TypographyAndStroke.figma_id(), "REQUIRE_TYPOGRAPHY_AND_STROKE_VARIABLES");
}

#[test]
fn disabling_a_rule_suppresses_only_its_findings_and_undoes() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", None);
    f.doc.get_mut(card).unwrap().corner_radii = [8.0; 4];
    f.doc.get_mut(card).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { padding: [16.0; 4], ..AutoLayout::v1() }), ..FrameProps::default() });
    f.num_var("radius/md", 8.0, 8.0, &["CORNER_RADIUS"]);
    f.num_var("space/4", 16.0, 16.0, &["GAP"]);

    let mut app = f.app();
    let r0 = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(r0.of(Rule::Padding).count(), 1);
    assert_eq!(r0.of(Rule::CornerRadius).count(), 1);

    assert!(app.set_check_designs_rule(Rule::Padding, false));
    assert!(app.doc.check_designs.disabled_rules.contains("REQUIRE_PADDING_VARIABLES"));
    let r1 = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(r1.of(Rule::Padding).count(), 0);
    assert_eq!(r1.of(Rule::CornerRadius).count(), 1);

    app.undo();
    assert!(!app.doc.check_designs.disabled_rules.contains("REQUIRE_PADDING_VARIABLES"));
    let r2 = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(r2.of(Rule::Padding).count(), 1);
    assert!(!app.hist.can_undo());
}

#[test]
fn library_assets_rule_flags_instance_style_and_variable_from_disabled_library() {
    let mut f = fx();
    // 1. Imported component on hidden library page from disabled library "Design System".
    let lib_page = designos_doc::ensure_library_page(&mut f.doc);
    let comp_id = f.doc.alloc_id();
    let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 80.0, 32.0));
    comp.name = "Button".into();
    comp.kind = NodeKind::Component(FrameProps::default());
    let comp_id = f.doc.insert(lib_page, comp).unwrap();
    f.doc.library.external.insert(
        "Design System/c1".into(),
        ExternalAsset { library_id: "Design System".into(), version: 1, kind: LibraryAssetKind::Component, node: Some(comp_id), local_key: None },
    );
    let inst_cmd = designos_doc::create_instance(&mut f.doc, comp_id, f.page, Vec2::new(0.0, 0.0));
    inst_cmd.apply(&mut f.doc).unwrap();

    // 2. Imported fill style from disabled library "Design System".
    f.style("Design System/s_fill", "Surface/Brand", Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    f.doc.library.external.insert(
        "Design System/style:s_fill".into(),
        ExternalAsset {
            library_id: "Design System".into(),
            version: 1,
            kind: LibraryAssetKind::Style,
            node: None,
            local_key: Some("Design System/s_fill".into()),
        },
    );
    let styled = f.rect(f.page, "Styled", BLUE);
    f.doc.get_mut(styled).unwrap().style_refs.insert("fill".into(), "Design System/s_fill".into());

    // 3. Imported variable from disabled library "Design System".
    let var_id = f.color_var("Brand/Primary", BLUE, BLUE);
    f.doc.library.external.insert(
        "Design System/var:Brand/Primary".into(),
        ExternalAsset { library_id: "Design System".into(), version: 1, kind: LibraryAssetKind::Variable, node: None, local_key: Some(var_id.clone()) },
    );
    let bound = f.rect(f.page, "Bound", BLUE);
    f.doc.get_mut(bound).unwrap().variables.bindings.insert("fills/0".into(), var_id);

    let report = f.report();
    let lib_findings: Vec<&Finding> = report.of(Rule::LibraryAssets).collect();
    assert_eq!(lib_findings.len(), 3, "{:#?}", report.findings);
    let msgs: Vec<&str> = lib_findings.iter().map(|x| x.message.as_str()).collect();
    assert!(msgs.contains(&"Uses \"Button\" from library \"Design System\", which isn't enabled in this file"), "{msgs:#?}");
    assert!(msgs.contains(&"Uses \"Surface/Brand\" from library \"Design System\", which isn't enabled in this file"), "{msgs:#?}");
    assert!(msgs.contains(&"Uses \"Brand/Primary\" from library \"Design System\", which isn't enabled in this file"), "{msgs:#?}");
    assert!(lib_findings.iter().all(|x| x.fix.is_none()));
}

#[test]
fn text_styles_rule_flags_unstyled_text_and_offers_fix_when_style_matches() {
    let mut doc = Document::new("text-styles");
    let page = doc.add_page("Page 1");
    let bg_id = doc.alloc_id();
    let mut bg = Node::frame(bg_id, Rect::new(0.0, 0.0, 300.0, 200.0));
    bg.fills = vec![Paint::solid(Color::WHITE)];
    let bg_id = doc.insert(page, bg).unwrap();

    // Unstyled text with no matching style -> "No text style", fix: None.
    let t1_id = doc.alloc_id();
    let t1 = Node::text(
        t1_id,
        Rect::new(10.0, 10.0, 120.0, 20.0),
        "Caption",
        TextStyle { font_size: 13.0, fills: vec![Paint::solid(Color::BLACK)], ..TextStyle::default() },
    );
    let t1_id = doc.insert(bg_id, t1).unwrap();

    // Unstyled text matching "Body/Regular" -> Fix::ApplyStyle.
    let body_ts = TextStyle { font_size: 16.0, font_weight: 400, ..TextStyle::default() };
    doc.styles.insert("body".into(), Style::Text(body_ts.clone()));
    doc.style_meta.insert("body".into(), StyleMeta { name: "Body/Regular".into(), property: StyleProperty::Text, ..StyleMeta::default() });
    let t2_id = doc.alloc_id();
    let t2 = Node::text(t2_id, Rect::new(10.0, 50.0, 120.0, 24.0), "Paragraph", TextStyle { fills: vec![Paint::solid(Color::BLACK)], ..body_ts });
    let t2_id = doc.insert(bg_id, t2).unwrap();

    let report = audit(&doc, &Scope::Page(page));
    let ts_findings: Vec<&Finding> = report.of(Rule::TextStyles).collect();
    assert_eq!(ts_findings.len(), 2, "{:#?}", report.findings);

    let f1 = ts_findings.iter().find(|x| x.node == t1_id).unwrap();
    assert_eq!(f1.message, "No text style");
    assert_eq!(f1.fix, None);

    let f2 = ts_findings.iter().find(|x| x.node == t2_id).unwrap();
    assert_eq!(f2.message, "Text matches style \"Body/Regular\"");
    assert_eq!(f2.fix, Some(Fix::ApplyStyle { style: "body".into() }));
}

#[test]
fn contrast_level_aaa_raises_thresholds_to_seven_and_four_point_five() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    // #767676 on white is 4.54:1 (passes AA 4.5:1, fails AAA 7:1).
    let body = f.text(card, "Body", 14.0, 400, gray(0x76));
    // #949494 on white is 3.03:1 (passes AA large 3:1, fails AAA large 4.5:1).
    let large = f.text(card, "Large", 24.0, 400, gray(0x94));

    let mut app = f.app();
    assert!(audit(&app.doc, &Scope::Page(app.page)).of(Rule::Contrast).next().is_none(), "both pass under AA");

    assert!(app.set_contrast_level(ContrastLevel::AAA));
    let report = audit(&app.doc, &Scope::Page(app.page));
    let contrast_findings: Vec<&Finding> = report.of(Rule::Contrast).collect();
    assert_eq!(contrast_findings.len(), 2, "{contrast_findings:#?}");

    let fb = contrast_findings.iter().find(|x| x.node == body).unwrap();
    assert_eq!(fb.message, "Contrast 4.54:1 is below 7:1 (WCAG AAA)");
    if let Some(Fix::SetTextColor { color }) = fb.fix {
        assert!(contrast_ratio(color, Color::WHITE) >= 7.0);
    } else {
        panic!("expected SetTextColor fix for AAA body text");
    }

    let fl = contrast_findings.iter().find(|x| x.node == large).unwrap();
    assert_eq!(fl.message, "Contrast 3.03:1 is below 4.5:1 (WCAG AAA)");

    app.undo();
    assert_eq!(app.doc.check_designs.contrast_level, ContrastLevel::AA);
    assert!(audit(&app.doc, &Scope::Page(app.page)).of(Rule::Contrast).next().is_none());
}

#[test]
fn ignore_persists_in_document_undoes_and_restores() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let finding = only(&audit(&app.doc, &Scope::Page(app.page)), Rule::HardCodedValue);
    assert_eq!(finding.node, r);

    assert!(app.check_designs_ignore(&finding));
    let key = finding.ignore_key();
    assert_eq!(key, IgnoreKey { node: r, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() });
    assert!(app.doc.check_designs.ignored.contains(&key));

    let after_ignore = audit(&app.doc, &Scope::Page(app.page));
    assert!(after_ignore.findings.is_empty());
    assert_eq!(after_ignore.ignored.len(), 1);

    app.undo();
    assert!(!app.doc.check_designs.ignored.contains(&key));
    assert_eq!(audit(&app.doc, &Scope::Page(app.page)).findings.len(), 1);

    app.redo();
    assert!(app.doc.check_designs.ignored.contains(&key));
    assert!(app.check_designs_restore(&key));
    assert!(!app.doc.check_designs.ignored.contains(&key));
    assert_eq!(audit(&app.doc, &Scope::Page(app.page)).findings.len(), 1);
}

#[test]
fn dismiss_hides_for_session_without_touching_document_or_undo_stack() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.report.findings.len(), 1);
    let finding = app.check_designs.report.findings[0].clone();

    app.check_designs_dismiss(&finding);
    assert!(app.check_designs.report.findings.is_empty());
    assert!(app.doc.check_designs.ignored.is_empty(), "dismiss does not write to document");
    assert!(!app.hist.can_undo(), "dismiss does not push an undo step");

    // Re-refreshing in the same session keeps it dismissed.
    app.check_designs.invalidate();
    app.refresh_check_designs(false, &egui::Context::default());
    assert!(app.check_designs.report.findings.is_empty());

    // Reopening the panel clears session dismissals.
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.report.findings.len(), 1);
}

#[test]
fn ignored_findings_survive_unrelated_edits_to_same_node() {
    let mut f = fx();
    let card = f.rect(f.page, "Card", BLUE);
    f.doc.get_mut(card).unwrap().corner_radii = [12.0; 4];
    f.color_var("Color/Primary", BLUE, BLUE);
    f.num_var("radius/lg", 12.0, 12.0, &["CORNER_RADIUS"]);

    let mut app = f.app();
    let fill_finding = only(&audit(&app.doc, &Scope::Page(app.page)), Rule::HardCodedValue);
    assert!(app.check_designs_ignore(&fill_finding));

    // Rename and move the node: the fill finding stays ignored while the corner radius finding remains active.
    let n = app.doc.get_mut(card).unwrap();
    n.name = "Renamed Card".into();
    n.transform.tx = 120.0;
    let report = audit(&app.doc, &Scope::Page(app.page));
    assert!(report.of(Rule::HardCodedValue).next().is_none(), "ignored fill finding stays suppressed");
    assert_eq!(report.of(Rule::CornerRadius).count(), 1, "corner radius finding on the same node is still reported");
}

#[test]
fn deleting_a_node_prunes_its_ignore_keys_on_save_and_load() {
    let mut f = fx();
    let a = f.rect(f.page, "A", BLUE);
    let b = f.rect(f.page, "B", BLUE);
    f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    for finding in &findings {
        assert!(app.check_designs_ignore(finding));
    }
    assert_eq!(app.doc.check_designs.ignored.len(), 2);

    app.doc.remove(a).unwrap();
    assert_eq!(app.doc.check_designs.ignored.len(), 2, "Document::remove preserves the deleted node's ignore keys in memory for undo");
    let json = app.doc.to_json().unwrap();
    let loaded = Document::from_json(&json).unwrap();
    assert_eq!(loaded.check_designs.ignored.len(), 1, "Document::from_json prunes deleted node ignore keys on load");
    assert!(loaded.check_designs.ignored.iter().all(|k| k.node == b));

    // Orphaned nodes pruned by `prune_unreachable` also drop their ignore keys.
    let mut doc = loaded;
    doc.get_mut(doc.pages[0]).unwrap().children.retain(|c| *c != b);
    assert_eq!(doc.prune_unreachable(), 1);
    assert!(doc.check_designs.ignored.is_empty());
}

#[test]
fn horizontal_and_vertical_spacing_rules_follow_stack_axis() {
    let mut f = fx();
    let h_row = f.frame(f.page, "HRow", None);
    f.doc.get_mut(h_row).unwrap().kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 16.0, counter_spacing: 12.0, ..AutoLayout::v1() }),
        ..FrameProps::default()
    });
    let v_col = f.frame(f.page, "VCol", None);
    f.doc.get_mut(v_col).unwrap().kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout { direction: LayoutDirection::Vertical, item_spacing: 16.0, counter_spacing: 12.0, ..AutoLayout::v1() }),
        ..FrameProps::default()
    });
    f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    f.num_var("space/3", 12.0, 12.0, &["GAP"]);

    let report = f.report();
    let horiz: Vec<(&str, &str)> = report.of(Rule::HorizontalSpacing).map(|x| (x.node_name.as_str(), x.property.as_str())).collect();
    let vert: Vec<(&str, &str)> = report.of(Rule::VerticalSpacing).map(|x| (x.node_name.as_str(), x.property.as_str())).collect();
    assert_eq!(horiz, vec![("HRow", "itemSpacing")]);
    // Spec 158 section 3.1: every `counterAxisSpacing` is a vertical spacing field.
    assert_eq!(vert, vec![("HRow", "counterAxisSpacing"), ("VCol", "itemSpacing"), ("VCol", "counterAxisSpacing")]);
}

#[test]
fn every_figma_rule_id_has_a_studio_rule() {
    let ids: Vec<&'static str> = Rule::ALL.iter().map(|r| r.figma_id()).collect();
    for expected in [
        "RECOMMEND_VARIABLES",
        "CONTRAST",
        "REQUIRE_NO_DETACHED_COMPONENTS",
        "REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES",
        "REQUIRE_TEXT_STYLES",
        "REQUIRE_CORNER_RADIUS_VARIABLES",
        "REQUIRE_PADDING_VARIABLES",
        "REQUIRE_HORIZONTAL_SPACING_VARIABLES",
        "REQUIRE_VERTICAL_SPACING_VARIABLES",
        "REQUIRE_TYPOGRAPHY_AND_STROKE_VARIABLES",
    ] {
        assert!(ids.contains(&expected), "missing {expected}");
    }
}

#[test]
fn aaa_level_flags_767676_which_aa_passes() {
    contrast_level_aaa_raises_thresholds_to_seven_and_four_point_five();
}

#[test]
fn aaa_large_text_threshold_is_4_5() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    // 24 px #767676 on white: ratio 4.54 >= 4.5, so AAA large text passes (§8 edge case 3).
    f.text(card, "Large", 24.0, 400, gray(0x76));
    let mut app = f.app();
    app.set_contrast_level(ContrastLevel::AAA);
    assert!(audit(&app.doc, &Scope::Page(app.page)).of(Rule::Contrast).next().is_none());
}

#[test]
fn ignored_finding_never_reappears_after_rescan_and_reload() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let finding = only(&audit(&app.doc, &Scope::Page(app.page)), Rule::HardCodedValue);
    assert!(app.check_designs_ignore(&finding));
    let json = app.doc.to_json().unwrap();
    let reloaded = Document::from_json(&json).unwrap();
    let after = audit(&reloaded, &Scope::Page(app.page));
    assert!(after.findings.is_empty());
    assert_eq!(after.ignored.len(), 1);
}

#[test]
fn ignore_is_one_undo_step() {
    ignore_persists_in_document_undoes_and_restores();
}

#[test]
fn disabled_rule_is_not_evaluated() {
    disabling_a_rule_suppresses_only_its_findings_and_undoes();
}

#[test]
fn text_without_text_style_is_flagged_with_apply_style_when_exact_match() {
    text_styles_rule_flags_unstyled_text_and_offers_fix_when_style_matches();
}

#[test]
fn asset_from_disabled_library_is_flagged_without_fix() {
    library_assets_rule_flags_instance_style_and_variable_from_disabled_library();
}

#[test]
fn a_figma_source_library_key_on_a_collection_is_not_a_disabled_studio_library() {
    // A `.fig` import stores Figma's own `sourceLibraryKey` on the collection. It names no Studio
    // library, so a binding to such a variable must not be reported as a disabled-library asset.
    let mut f = fx();
    let var_id = f.color_var("Brand/Primary", BLUE, BLUE);
    f.doc.variable_collections.get_mut("theme").unwrap().source_library_key = Some("lk-83c0".into());
    let bound = f.rect(f.page, "Bound", BLUE);
    f.doc.get_mut(bound).unwrap().variables.bindings.insert("fills/0".into(), var_id);
    let report = f.report();
    assert_eq!(report.of(Rule::LibraryAssets).count(), 0, "{:#?}", report.findings);
}

#[test]
fn two_text_styles_that_equal_the_text_offer_no_fix() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(card, "Heading", 32.0, 700, gray(0x11));
    let ts = TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() };
    f.style("H1", "Heading/H1", Style::Text(ts.clone()), StyleProperty::Text);
    f.style("H1b", "Heading/H1 copy", Style::Text(ts), StyleProperty::Text);
    let report = f.report();
    let found = only(&report, Rule::TextStyles);
    assert_eq!(found.node, t);
    assert_eq!(found.message, "Text matches 2 styles: \"Heading/H1\", \"Heading/H1 copy\"", "the user picks between two equal styles");
    assert_eq!(found.fix, None);
}

#[test]
fn a_typography_number_match_or_binding_does_not_hide_the_text_style_rule() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let size_var = f.number_var("Size/Title", 32.0, 32.0);
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(card, "Heading", 32.0, 700, gray(0x11));
    f.style("H1", "Heading/H1", Style::Text(TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() }), StyleProperty::Text);

    // 1. Both TypographyAndStroke and TextStyles enabled: both rules emit independently.
    let report = f.report();
    assert_eq!(report.of(Rule::TypographyAndStroke).count(), 1);
    let found = only(&report, Rule::TextStyles);
    assert_eq!(found.node, t);
    assert_eq!(found.fix, Some(Fix::ApplyStyle { style: "H1".into() }));

    // 2. TypographyAndStroke disabled: TextStyles still emits ApplyStyle.
    f.doc.check_designs.disabled_rules.insert(Rule::TypographyAndStroke.figma_id().into());
    let report_off = f.report();
    assert_eq!(report_off.of(Rule::TypographyAndStroke).count(), 0);
    let found_off = only(&report_off, Rule::TextStyles);
    assert_eq!(found_off.fix, Some(Fix::ApplyStyle { style: "H1".into() }));

    // 3. After `fontSize` is bound to the variable, the unstyled layer still gets `ApplyStyle`.
    f.doc.check_designs.disabled_rules.remove(Rule::TypographyAndStroke.figma_id());
    f.doc.get_mut(t).unwrap().variables.bindings.insert("fontSize".into(), size_var);
    let report_bound = f.report();
    assert_eq!(report_bound.of(Rule::TypographyAndStroke).count(), 0);
    let found_bound = only(&report_bound, Rule::TextStyles);
    assert_eq!(found_bound.node, t);
    assert_eq!(found_bound.fix, Some(Fix::ApplyStyle { style: "H1".into() }));
}

#[test]
fn ignoring_contrast_keeps_hardcoded_value_in_both_check_designs_and_linter_and_switch_toggles_both() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    let muted = gray(0xB0);
    f.color_var("Color/Muted", muted, muted);
    let t = f.text(card, "Muted label", 14.0, 400, muted);

    let mut app = f.app();
    let r0 = audit(&app.doc, &Scope::Page(app.page));
    let contrast_finding = only(&r0, Rule::Contrast);
    assert_eq!(contrast_finding.node, t);
    assert_eq!(only(&r0, Rule::HardCodedValue).node, t);

    // Ignore the Contrast row: stores rule "CONTRAST", property "".
    assert!(app.check_designs_ignore(&contrast_finding));
    assert!(app.doc.check_designs.ignored.contains(&IgnoreKey { node: t, rule: "CONTRAST".into(), property: String::new() }));

    // Check designs still shows the colour bind on the same text node.
    let r1 = audit(&app.doc, &Scope::Page(app.page));
    assert!(r1.of(Rule::Contrast).next().is_none(), "contrast finding is ignored");
    assert_eq!(only(&r1, Rule::HardCodedValue).node, t, "Check designs keeps the fills/0 colour bind");

    // designos_lint also keeps hardcoded-value on fills/0 for the same node.
    let lint_cfg = designos_doc::lint::LintConfig::from_rule_list(&["hardcoded-value".into()]).unwrap();
    let lint_report = designos_doc::lint::lint(&app.doc, &lint_cfg, &designos_doc::lint::NoLintEnv);
    assert!(
        lint_report.findings.iter().any(|x| x.node == t && x.rule == "hardcoded-value" && x.property.as_deref() == Some("fills/0")),
        "designos_lint must still report hardcoded-value on fills/0 after ignoring contrast: {:#?}",
        lint_report.findings
    );

    // Toggling the single "Colours & contrast" switch writes both RECOMMEND_VARIABLES and CONTRAST.
    app.undo(); // restore contrast finding
    assert!(app.set_check_designs_rule(Rule::HardCodedValue, false));
    assert!(app.doc.check_designs.disabled_rules.contains("RECOMMEND_VARIABLES"));
    assert!(app.doc.check_designs.disabled_rules.contains("CONTRAST"));
    let r_off = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(r_off.of(Rule::HardCodedValue).count(), 0);
    assert_eq!(r_off.of(Rule::Contrast).count(), 0);

    app.undo();
    assert!(!app.doc.check_designs.disabled_rules.contains("RECOMMEND_VARIABLES"));
    assert!(!app.doc.check_designs.disabled_rules.contains("CONTRAST"));

    // A legacy {RECOMMEND_VARIABLES, ""} ignore key hides the Contrast row into ignored, and Restore removes it.
    app.doc.check_designs.ignored.insert(IgnoreKey { node: t, rule: "RECOMMEND_VARIABLES".into(), property: String::new() });
    let r_legacy = audit(&app.doc, &Scope::Page(app.page));
    assert!(r_legacy.of(Rule::Contrast).next().is_none(), "legacy key hides Contrast from active findings");
    assert!(r_legacy.ignored.iter().any(|x| x.node == t && x.rule == Rule::Contrast), "Contrast row is listed in ignored");
    assert!(app.check_designs_restore(&contrast_finding.ignore_key()));
    assert!(!app.doc.check_designs.ignored.contains(&IgnoreKey { node: t, rule: "RECOMMEND_VARIABLES".into(), property: String::new() }));
    let r_restored = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(only(&r_restored, Rule::Contrast).node, t, "Restore brings back a legacy-ignored Contrast row");
}
