use designos_doc::lint::{lint, preset_rule_ids, resolve_rule, Finding, LintConfig, LintPreset, NoLintEnv};
use designos_doc::{AutoLayout, Color, Document, FrameProps, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect, Sizing, TextAutoResize, TextStyle};
use designos_render::RenderLintEnv;
use std::time::Duration;

fn new_doc_with_page(name: &str) -> (Document, NodeId) {
    let mut doc = Document::new(name);
    let page = doc.add_page("Page 1");
    (doc, page)
}

fn add_frame(doc: &mut Document, parent: NodeId, name: &str, r: Rect, fp: FrameProps) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::frame(id, r);
    n.name = name.into();
    n.kind = NodeKind::Frame(fp);
    doc.insert(parent, n).unwrap();
    id
}

fn add_rect(doc: &mut Document, parent: NodeId, name: &str, r: Rect) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, r);
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::BLACK)];
    doc.insert(parent, n).unwrap();
    id
}

fn add_text(doc: &mut Document, parent: NodeId, name: &str, r: Rect, content: &str, style: TextStyle, auto_resize: TextAutoResize) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::text(id, r, content, style);
    n.name = name.into();
    if let NodeKind::Text(tp) = &mut n.kind {
        tp.auto_resize = auto_resize;
    }
    doc.insert(parent, n).unwrap();
    id
}

fn only_rule_config(rule: &str) -> LintConfig {
    LintConfig::from_rule_list(&[rule.to_string()]).expect("valid rule")
}

fn lint_single_rule(doc: &Document, rule: &str) -> Vec<Finding> {
    let cfg = only_rule_config(rule);
    lint(doc, &cfg, &RenderLintEnv).findings
}

#[test]
fn fail_and_clean_overflow_parent() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Overflow");
    let frame = add_frame(&mut fail_doc, page, "Parent", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: false, ..Default::default() });
    let child = add_rect(&mut fail_doc, frame, "OverflowChild", Rect::new(20.0, 10.0, 120.0, 50.0));
    let findings = lint_single_rule(&fail_doc, "overflow-parent");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "overflow-parent");
    assert_eq!(findings[0].node, child);
    assert_eq!(findings[0].bounds, Rect::new(20.0, 10.0, 120.0, 50.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Overflow");
    let cframe = add_frame(&mut clean_doc, cpage, "Parent", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: false, ..Default::default() });
    add_rect(&mut clean_doc, cframe, "InsideChild", Rect::new(10.0, 10.0, 80.0, 50.0));
    assert!(lint_single_rule(&clean_doc, "overflow-parent").is_empty());
}

#[test]
fn fail_and_clean_clipped_partial() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Clipped Partial");
    let frame = add_frame(&mut fail_doc, page, "ClipFrame", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: true, ..Default::default() });
    let child = add_rect(&mut fail_doc, frame, "HalfClipped", Rect::new(50.0, 10.0, 100.0, 40.0));
    let findings = lint_single_rule(&fail_doc, "clipped-partial");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "clipped-partial");
    assert_eq!(findings[0].node, child);
    assert_eq!(findings[0].bounds, Rect::new(50.0, 10.0, 100.0, 40.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Clipped Partial");
    let cframe = add_frame(&mut clean_doc, cpage, "ClipFrame", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: true, ..Default::default() });
    add_rect(&mut clean_doc, cframe, "Inside", Rect::new(10.0, 10.0, 50.0, 40.0));
    assert!(lint_single_rule(&clean_doc, "clipped-partial").is_empty());
}

#[test]
fn fail_and_clean_clipped_full() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Clipped Full");
    let frame = add_frame(&mut fail_doc, page, "ClipFrame", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: true, ..Default::default() });
    let child = add_rect(&mut fail_doc, frame, "FullyOutside", Rect::new(150.0, 10.0, 40.0, 40.0));
    let findings = lint_single_rule(&fail_doc, "clipped-full");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "clipped-full");
    assert_eq!(findings[0].node, child);
    assert_eq!(findings[0].bounds, Rect::new(150.0, 10.0, 40.0, 40.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Clipped Full");
    let cframe = add_frame(&mut clean_doc, cpage, "ClipFrame", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: true, ..Default::default() });
    add_rect(&mut clean_doc, cframe, "Inside", Rect::new(10.0, 10.0, 40.0, 40.0));
    assert!(lint_single_rule(&clean_doc, "clipped-full").is_empty());
}

#[test]
fn fail_and_clean_sibling_overlap() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Sibling Overlap");
    let frame = add_frame(&mut fail_doc, page, "FreeformParent", Rect::new(0.0, 0.0, 200.0, 200.0), FrameProps::default());
    add_text(&mut fail_doc, frame, "TitleA", Rect::new(10.0, 10.0, 80.0, 24.0), "Hello World", TextStyle::default(), TextAutoResize::None);
    let _b = add_text(&mut fail_doc, frame, "TitleB", Rect::new(30.0, 12.0, 80.0, 24.0), "Overlapping Text", TextStyle::default(), TextAutoResize::None);
    let findings = lint_single_rule(&fail_doc, "sibling-overlap");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "sibling-overlap");
    assert_eq!(findings[0].node, frame);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Sibling Overlap");
    let cframe = add_frame(&mut clean_doc, cpage, "FreeformParent", Rect::new(0.0, 0.0, 200.0, 200.0), FrameProps::default());
    add_text(&mut clean_doc, cframe, "TitleA", Rect::new(10.0, 10.0, 80.0, 24.0), "Hello World", TextStyle::default(), TextAutoResize::None);
    add_text(&mut clean_doc, cframe, "TitleB", Rect::new(10.0, 50.0, 80.0, 24.0), "Separated Text", TextStyle::default(), TextAutoResize::None);
    assert!(lint_single_rule(&clean_doc, "sibling-overlap").is_empty());
}

#[test]
fn fail_and_clean_text_truncated() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Text Truncated");
    let tid = add_text(
        &mut fail_doc,
        page,
        "BioText",
        Rect::new(0.0, 0.0, 80.0, 20.0),
        "Save changes to your profile settings right now before leaving",
        TextStyle { font_size: 16.0, ..Default::default() },
        TextAutoResize::None,
    );
    let findings = lint_single_rule(&fail_doc, "text-truncated");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "text-truncated");
    assert_eq!(findings[0].node, tid);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 0.0, 80.0, 20.0));
    let fix = findings[0].suggested_fix.as_ref().expect("text-truncated suggests HEIGHT fix");
    assert_eq!(fix.property, "textAutoResize");
    assert_eq!(fix.to, serde_json::json!("HEIGHT"));

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Text Truncated");
    add_text(
        &mut clean_doc,
        cpage,
        "BioText",
        Rect::new(0.0, 0.0, 300.0, 60.0),
        "Short text",
        TextStyle { font_size: 16.0, ..Default::default() },
        TextAutoResize::None,
    );
    assert!(lint_single_rule(&clean_doc, "text-truncated").is_empty());
}

#[test]
fn fail_and_clean_text_unexpected_wrap() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Text Unexpected Wrap");
    let tid = add_text(
        &mut fail_doc,
        page,
        "ButtonLabel",
        Rect::new(0.0, 0.0, 50.0, 48.0),
        "Save changes",
        TextStyle { font_size: 16.0, ..Default::default() },
        TextAutoResize::Height,
    );
    let findings = lint_single_rule(&fail_doc, "text-unexpected-wrap");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "text-unexpected-wrap");
    assert_eq!(findings[0].node, tid);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 0.0, 50.0, 48.0));
    let fix = findings[0].suggested_fix.as_ref().expect("text-unexpected-wrap suggests WIDTH_AND_HEIGHT fix");
    assert_eq!(fix.property, "textAutoResize");
    assert_eq!(fix.to, serde_json::json!("WIDTH_AND_HEIGHT"));

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Text Unexpected Wrap");
    add_text(
        &mut clean_doc,
        cpage,
        "ButtonLabel",
        Rect::new(0.0, 0.0, 240.0, 28.0),
        "Save changes",
        TextStyle { font_size: 16.0, ..Default::default() },
        TextAutoResize::Height,
    );
    assert!(lint_single_rule(&clean_doc, "text-unexpected-wrap").is_empty());
}

#[test]
fn fail_and_clean_collapsed_size() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Collapsed Size");
    let nid = add_frame(&mut fail_doc, page, "ZeroHeightFrame", Rect::new(10.0, 20.0, 120.0, 0.0), FrameProps::default());
    let findings = lint_single_rule(&fail_doc, "collapsed-size");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "collapsed-size");
    assert_eq!(findings[0].node, nid);
    assert_eq!(findings[0].bounds, Rect::new(10.0, 20.0, 120.0, 0.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Collapsed Size");
    add_frame(&mut clean_doc, cpage, "NormalFrame", Rect::new(10.0, 20.0, 120.0, 40.0), FrameProps::default());
    assert!(lint_single_rule(&clean_doc, "collapsed-size").is_empty());
}

#[test]
fn fail_and_clean_sizing_conflict() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Sizing Conflict");
    let parent = add_frame(
        &mut fail_doc,
        page,
        "HugParent",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    let cid = fail_doc.alloc_id();
    let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 80.0, 40.0));
    child.name = "FillChild".into();
    child.fills = vec![Paint::solid(Color::BLACK)];
    child.layout_child.horizontal = Sizing::Fill;
    fail_doc.insert(parent, child).unwrap();

    let findings = lint_single_rule(&fail_doc, "sizing-conflict");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "sizing-conflict");
    assert_eq!(findings[0].node, cid);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 0.0, 80.0, 40.0));
    let fix = findings[0].suggested_fix.as_ref().expect("sizing-conflict suggests FIXED fix");
    assert_eq!(fix.property, "horizontalSizing");
    assert_eq!(fix.to, serde_json::json!("Fixed"));

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Sizing Conflict");
    let cparent = add_frame(
        &mut clean_doc,
        cpage,
        "FixedParent",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    let ccid = clean_doc.alloc_id();
    let mut cchild = Node::rectangle(ccid, Rect::new(0.0, 0.0, 80.0, 40.0));
    cchild.name = "FillChild".into();
    cchild.fills = vec![Paint::solid(Color::BLACK)];
    cchild.layout_child.horizontal = Sizing::Fill;
    clean_doc.insert(cparent, cchild).unwrap();
    assert!(lint_single_rule(&clean_doc, "sizing-conflict").is_empty());
}

#[test]
fn fail_and_clean_text_missing_fill() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Text Missing Fill");
    let tid = fail_doc.alloc_id();
    let mut tnode = Node::text(tid, Rect::new(5.0, 5.0, 100.0, 24.0), "Invisible label", TextStyle { fills: Vec::new(), ..Default::default() });
    tnode.name = "InvisibleText".into();
    tnode.fills.clear();
    fail_doc.insert(page, tnode).unwrap();

    let findings = lint_single_rule(&fail_doc, "text-missing-fill");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "text-missing-fill");
    assert_eq!(findings[0].node, tid);
    assert_eq!(findings[0].bounds, Rect::new(5.0, 5.0, 100.0, 24.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Text Missing Fill");
    add_text(&mut clean_doc, cpage, "VisibleText", Rect::new(5.0, 5.0, 100.0, 24.0), "Visible label", TextStyle::default(), TextAutoResize::WidthAndHeight);
    assert!(lint_single_rule(&clean_doc, "text-missing-fill").is_empty());
}

#[test]
fn fail_and_clean_absolute_in_auto_layout() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Absolute In Auto Layout");
    let parent = add_frame(
        &mut fail_doc,
        page,
        "AutoFrame",
        Rect::new(0.0, 0.0, 200.0, 100.0),
        FrameProps { auto_layout: Some(AutoLayout::default()), ..Default::default() },
    );
    let cid = fail_doc.alloc_id();
    let mut child = Node::rectangle(cid, Rect::new(10.0, 10.0, 40.0, 40.0));
    child.name = "PinnedBox".into();
    child.fills = vec![Paint::solid(Color::BLACK)];
    child.layout_child.absolute = true;
    fail_doc.insert(parent, child).unwrap();

    let findings = lint_single_rule(&fail_doc, "absolute-in-auto-layout");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "absolute-in-auto-layout");
    assert_eq!(findings[0].node, cid);
    assert_eq!(findings[0].bounds, Rect::new(10.0, 10.0, 40.0, 40.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Absolute In Auto Layout");
    let cparent = add_frame(
        &mut clean_doc,
        cpage,
        "AutoFrame",
        Rect::new(0.0, 0.0, 200.0, 100.0),
        FrameProps { auto_layout: Some(AutoLayout::default()), ..Default::default() },
    );
    let bcid = clean_doc.alloc_id();
    let mut badge = Node::rectangle(bcid, Rect::new(10.0, 10.0, 20.0, 20.0));
    badge.name = "FlowChild".into();
    badge.fills = vec![Paint::solid(Color::BLACK)];
    badge.layout_child.absolute = false;
    clean_doc.insert(cparent, badge).unwrap();
    assert!(lint_single_rule(&clean_doc, "absolute-in-auto-layout").is_empty());
}

#[test]
fn fail_and_clean_placeholder_text() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Placeholder Text");
    let tid = add_text(
        &mut fail_doc,
        page,
        "BodyCopy",
        Rect::new(0.0, 0.0, 200.0, 40.0),
        "Lorem ipsum dolor sit amet",
        TextStyle::default(),
        TextAutoResize::WidthAndHeight,
    );
    let findings = lint_single_rule(&fail_doc, "placeholder-text");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "placeholder-text");
    assert_eq!(findings[0].node, tid);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 0.0, 200.0, 40.0));
    assert!(findings[0].suggested_fix.is_none());

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Placeholder Text");
    add_text(
        &mut clean_doc,
        cpage,
        "BodyCopy",
        Rect::new(0.0, 0.0, 200.0, 40.0),
        "Account security settings",
        TextStyle::default(),
        TextAutoResize::WidthAndHeight,
    );
    assert!(lint_single_rule(&clean_doc, "placeholder-text").is_empty());
}

#[test]
fn fail_and_clean_detached_instance() {
    let (mut fail_doc, page) = new_doc_with_page("Fail Detached Instance");
    let comp_id = fail_doc.alloc_id();
    let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 120.0, 40.0));
    comp.name = "PrimaryButton".into();
    comp.kind = NodeKind::Component(FrameProps::default());
    comp.source_id = Some("1:10".into());
    fail_doc.insert(page, comp).unwrap();

    let did = fail_doc.alloc_id();
    let mut detached = Node::frame(did, Rect::new(0.0, 60.0, 120.0, 40.0));
    detached.name = "PrimaryButtonCopy".into();
    detached.detached_symbol_id = Some("1:10".into());
    fail_doc.insert(page, detached).unwrap();

    let findings = lint_single_rule(&fail_doc, "detached-instance");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "detached-instance");
    assert_eq!(findings[0].node, did);
    assert_eq!(findings[0].bounds, Rect::new(0.0, 60.0, 120.0, 40.0));
    let fix = findings[0].suggested_fix.as_ref().expect("detached-instance suggests reconnect fix when structure matches");
    assert_eq!(fix.kind, "reconnect");
    assert_eq!(fix.to, serde_json::json!(comp_id.0));

    let (mut clean_doc, cpage) = new_doc_with_page("Clean Detached Instance");
    add_frame(&mut clean_doc, cpage, "PlainFrame", Rect::new(0.0, 0.0, 120.0, 40.0), FrameProps::default());
    assert!(lint_single_rule(&clean_doc, "detached-instance").is_empty());
}

#[test]
fn cosmic_text_measurement_catches_real_overflow() {
    let (mut doc, page) = new_doc_with_page("Cosmic Text Overflow");
    let tid = add_text(
        &mut doc,
        page,
        "I18nHeader",
        Rect::new(0.0, 0.0, 60.0, 20.0),
        "Internationalization",
        TextStyle { font_family: "Inter".into(), font_size: 14.0, ..Default::default() },
        TextAutoResize::None,
    );

    let cfg = LintConfig::from_preset(LintPreset::Agent);
    let report = lint(&doc, &cfg, &RenderLintEnv);
    assert!(report.skipped_rules.is_empty());
    assert!(
        report.findings.iter().any(|f| f.rule == "text-truncated" && f.node == tid),
        "expected text-truncated on Internationalization in 60x20 box, got: {:?}",
        report.findings
    );
    assert!(
        report.findings.iter().any(|f| f.rule == "text-unexpected-wrap" && f.node == tid),
        "expected text-unexpected-wrap on Internationalization in 60x20 box, got: {:?}",
        report.findings
    );

    let headless_report = lint(&doc, &cfg, &NoLintEnv);
    let skipped_ids: Vec<&str> = headless_report.skipped_rules.iter().map(|s| s.rule.as_str()).collect();
    assert!(skipped_ids.contains(&"text-truncated"));
    assert!(skipped_ids.contains(&"text-unexpected-wrap"));
    assert!(headless_report.skipped_rules.iter().all(|s| s.reason == "no_text_engine"));
}

#[test]
fn presets_name_only_registered_rules() {
    for preset in [LintPreset::Agent, LintPreset::CheckDesigns, LintPreset::Recommended, LintPreset::Strict] {
        for id in preset_rule_ids(preset) {
            assert!(resolve_rule(id).is_some(), "preset {} names unregistered rule {id}", preset.as_str());
        }
    }
}

#[test]
fn ignored_never_reappears() {
    let (mut doc, page) = new_doc_with_page("Ignore Persistence");
    let frame = add_frame(&mut doc, page, "Parent", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps { clips_content: false, ..Default::default() });
    let child = add_rect(&mut doc, frame, "OverflowRect", Rect::new(20.0, 20.0, 120.0, 50.0));

    let cfg = only_rule_config("overflow-parent");
    assert_eq!(lint(&doc, &cfg, &RenderLintEnv).findings.len(), 1);

    doc.ignore_lint("overflow-parent", child);
    assert!(lint(&doc, &cfg, &RenderLintEnv).findings.is_empty());

    // 1. Native JSON round-trip
    let json_text = doc.to_json().expect("serialize native JSON");
    let from_json = Document::from_json(&json_text).expect("deserialize native JSON");
    assert!(lint(&from_json, &cfg, &RenderLintEnv).findings.is_empty());

    // 2. .fig archive round-trip
    let fig_bytes = designos_fig::export_fig(&doc).expect("export .fig");
    let from_fig = designos_fig::import_fig(&fig_bytes).expect("import .fig");
    assert!(lint(&from_fig, &cfg, &RenderLintEnv).findings.is_empty(), "expected overflow-parent finding to remain ignored after .fig round-trip");
}

#[test]
fn check_designs_property_ignore_does_not_suppress_other_hardcoded_colors() {
    use designos_doc::IgnoreKey;
    let (mut doc, page) = new_doc_with_page("Property Scoped Ignore");
    let rect = add_rect(&mut doc, page, "Card", Rect::new(0.0, 0.0, 120.0, 80.0));
    doc.get_mut(rect).unwrap().fills = vec![Paint::solid(Color::rgb(20, 40, 60)), Paint::solid(Color::rgb(80, 100, 120))];

    let cfg = only_rule_config("hardcoded-color");
    assert_eq!(lint(&doc, &cfg, &RenderLintEnv).findings.len(), 1);

    // 1. A Contrast ignore in Check designs uses rule "RECOMMEND_VARIABLES" with property "".
    // It must NOT suppress hardcoded-color on the same node.
    doc.check_designs.ignored.insert(IgnoreKey { node: rect, rule: "RECOMMEND_VARIABLES".into(), property: String::new() });
    assert!(!doc.is_lint_ignored("hardcoded-color", rect), "contrast ignore (empty property on RECOMMEND_VARIABLES) must not ignore hardcoded-color");
    assert_eq!(lint(&doc, &cfg, &RenderLintEnv).findings.len(), 1);

    // 2. Ignoring only fills/0 in Check designs must NOT suppress hardcoded-color while fills/1 is still unbound.
    doc.check_designs.ignored.insert(IgnoreKey { node: rect, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() });
    assert!(!doc.is_lint_ignored("hardcoded-color", rect), "fills/0 ignore alone must not mark whole-node hardcoded-color ignored");
    assert_eq!(lint(&doc, &cfg, &RenderLintEnv).findings.len(), 1, "fills/1 is still hardcoded and unignored, so hardcoded-color must still fire");

    // 3. Once fills/1 is also ignored, all unbound color slots on `rect` are ignored.
    doc.check_designs.ignored.insert(IgnoreKey { node: rect, rule: "RECOMMEND_VARIABLES".into(), property: "fills/1".into() });
    assert!(lint(&doc, &cfg, &RenderLintEnv).findings.is_empty(), "all unbound color slots are ignored");
    let mut cfg_inc = cfg.clone();
    cfg_inc.include_ignored = true;
    let inc_report = lint(&doc, &cfg_inc, &RenderLintEnv);
    assert_eq!(inc_report.findings.len(), 1);
    assert!(inc_report.findings[0].ignored);
}

#[test]
fn heuristic_low_contrast_text_is_not_registered_before_e15_10() {
    assert!(resolve_rule("low-contrast-text").is_none(), "low-contrast-text must not be registered with a heuristic before E15-10 lands");
}

#[test]
fn fail_and_clean_default_layer_name() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_frame(&mut fail_doc, fail_page, "Frame 1", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps::default());
    let f = lint_single_rule(&fail_doc, "default-layer-name");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_frame(&mut clean_doc, clean_page, "PricingCard", Rect::new(0.0, 0.0, 100.0, 100.0), FrameProps::default());
    assert!(lint_single_rule(&clean_doc, "default-layer-name").is_empty());
}

#[test]
fn fail_and_clean_deeply_nested() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let mut parent = fail_page;
    for d in 1..=7 {
        parent = add_frame(&mut fail_doc, parent, &format!("Level{d}"), Rect::new(0.0, 0.0, 80.0, 80.0), FrameProps::default());
    }
    let f = lint_single_rule(&fail_doc, "deeply-nested");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, parent);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let mut clean_parent = clean_page;
    for d in 1..=6 {
        clean_parent = add_frame(&mut clean_doc, clean_parent, &format!("Level{d}"), Rect::new(0.0, 0.0, 80.0, 80.0), FrameProps::default());
    }
    assert!(lint_single_rule(&clean_doc, "deeply-nested").is_empty());
}

#[test]
fn fail_and_clean_empty_container() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let empty = add_frame(&mut fail_doc, fail_page, "EmptyShell", Rect::new(0.0, 0.0, 80.0, 80.0), FrameProps::default());
    fail_doc.get_mut(empty).unwrap().fills.clear();
    let f = lint_single_rule(&fail_doc, "empty-container");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, empty);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let non_empty = add_frame(&mut clean_doc, clean_page, "FilledShell", Rect::new(0.0, 0.0, 80.0, 80.0), FrameProps::default());
    clean_doc.get_mut(non_empty).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    assert!(lint_single_rule(&clean_doc, "empty-container").is_empty());
}

#[test]
fn fail_and_clean_non_4px_grid() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_rect(&mut fail_doc, fail_page, "OddRect", Rect::new(0.0, 0.0, 103.0, 41.0));
    let f = lint_single_rule(&fail_doc, "non-4px-grid");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_rect(&mut clean_doc, clean_page, "AlignedRect", Rect::new(0.0, 0.0, 104.0, 40.0));
    assert!(lint_single_rule(&clean_doc, "non-4px-grid").is_empty());
}

#[test]
fn fail_and_clean_inconsistent_spacing() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_frame(
        &mut fail_doc,
        fail_page,
        "Row",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps { auto_layout: Some(AutoLayout { item_spacing: 11.0, ..Default::default() }), ..Default::default() },
    );
    let f = lint_single_rule(&fail_doc, "inconsistent-spacing");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_frame(
        &mut clean_doc,
        clean_page,
        "Row",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps { auto_layout: Some(AutoLayout { item_spacing: 12.0, ..Default::default() }), ..Default::default() },
    );
    assert!(lint_single_rule(&clean_doc, "inconsistent-spacing").is_empty());
}

#[test]
fn fail_and_clean_unbalanced_padding() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_frame(
        &mut fail_doc,
        fail_page,
        "Card",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps { auto_layout: Some(AutoLayout { padding: [8.0, 24.0, 8.0, 8.0], ..Default::default() }), ..Default::default() },
    );
    let f = lint_single_rule(&fail_doc, "unbalanced-padding");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_frame(
        &mut clean_doc,
        clean_page,
        "Card",
        Rect::new(0.0, 0.0, 200.0, 80.0),
        FrameProps { auto_layout: Some(AutoLayout { padding: [12.0, 16.0, 12.0, 16.0], ..Default::default() }), ..Default::default() },
    );
    assert!(lint_single_rule(&clean_doc, "unbalanced-padding").is_empty());
}

#[test]
fn fail_and_clean_missing_auto_layout() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_frame(&mut fail_doc, fail_page, "PlainContainer", Rect::new(0.0, 0.0, 200.0, 100.0), FrameProps::default());
    add_rect(&mut fail_doc, bad, "ChildA", Rect::new(0.0, 0.0, 40.0, 40.0));
    add_rect(&mut fail_doc, bad, "ChildB", Rect::new(60.0, 0.0, 40.0, 40.0));
    let f = lint_single_rule(&fail_doc, "missing-auto-layout");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let clean = add_frame(
        &mut clean_doc,
        clean_page,
        "AutoContainer",
        Rect::new(0.0, 0.0, 200.0, 100.0),
        FrameProps { auto_layout: Some(AutoLayout::default()), ..Default::default() },
    );
    add_rect(&mut clean_doc, clean, "ChildA", Rect::new(0.0, 0.0, 40.0, 40.0));
    add_rect(&mut clean_doc, clean, "ChildB", Rect::new(60.0, 0.0, 40.0, 40.0));
    assert!(lint_single_rule(&clean_doc, "missing-auto-layout").is_empty());
}

#[test]
fn fail_and_clean_misaligned_siblings() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let parent = add_frame(&mut fail_doc, fail_page, "PlainRow", Rect::new(0.0, 0.0, 200.0, 100.0), FrameProps::default());
    add_rect(&mut fail_doc, parent, "Left", Rect::new(10.0, 10.0, 40.0, 40.0));
    add_rect(&mut fail_doc, parent, "RightOffBy2", Rect::new(70.0, 12.0, 40.0, 40.0));
    let f = lint_single_rule(&fail_doc, "misaligned-siblings");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, parent);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let clean_parent = add_frame(&mut clean_doc, clean_page, "AlignedRow", Rect::new(0.0, 0.0, 200.0, 100.0), FrameProps::default());
    add_rect(&mut clean_doc, clean_parent, "Left", Rect::new(10.0, 10.0, 40.0, 40.0));
    add_rect(&mut clean_doc, clean_parent, "RightAligned", Rect::new(70.0, 10.0, 40.0, 40.0));
    assert!(lint_single_rule(&clean_doc, "misaligned-siblings").is_empty());
}

#[test]
fn fail_and_clean_inconsistent_radius() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_rect(&mut fail_doc, fail_page, "OddRadius", Rect::new(0.0, 0.0, 80.0, 40.0));
    fail_doc.get_mut(bad).unwrap().corner_radii = [7.0, 7.0, 7.0, 7.0];
    // `off-scale` reports each off-scale corner as its own property finding.
    let f = lint_single_rule(&fail_doc, "inconsistent-radius");
    assert_eq!(f.len(), 4);
    assert!(f.iter().all(|x| x.node == bad && x.rule == "off-scale"));

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let clean = add_rect(&mut clean_doc, clean_page, "ScaleRadius", Rect::new(0.0, 0.0, 80.0, 40.0));
    clean_doc.get_mut(clean).unwrap().corner_radii = [8.0, 8.0, 8.0, 8.0];
    assert!(lint_single_rule(&clean_doc, "inconsistent-radius").is_empty());
}

#[test]
fn fail_and_clean_small_touch_target() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_frame(&mut fail_doc, fail_page, "Save Button", Rect::new(0.0, 0.0, 32.0, 28.0), FrameProps::default());
    let f = lint_single_rule(&fail_doc, "small-touch-target");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_frame(&mut clean_doc, clean_page, "Save Button", Rect::new(0.0, 0.0, 48.0, 44.0), FrameProps::default());
    assert!(lint_single_rule(&clean_doc, "small-touch-target").is_empty());
}

#[test]
fn fail_and_clean_non_scale_font_size() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_text(
        &mut fail_doc,
        fail_page,
        "Heading",
        Rect::new(0.0, 0.0, 120.0, 24.0),
        "Account settings",
        TextStyle { font_size: 17.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    let f = lint_single_rule(&fail_doc, "non-scale-font-size");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_text(
        &mut clean_doc,
        clean_page,
        "Heading",
        Rect::new(0.0, 0.0, 120.0, 24.0),
        "Account settings",
        TextStyle { font_size: 16.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    assert!(lint_single_rule(&clean_doc, "non-scale-font-size").is_empty());
}

#[test]
fn fail_and_clean_tiny_text() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_text(
        &mut fail_doc,
        fail_page,
        "Footnote",
        Rect::new(0.0, 0.0, 120.0, 16.0),
        "Fine print",
        TextStyle { font_size: 9.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    let f = lint_single_rule(&fail_doc, "tiny-text");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    add_text(
        &mut clean_doc,
        clean_page,
        "Footnote",
        Rect::new(0.0, 0.0, 120.0, 16.0),
        "Readable print",
        TextStyle { font_size: 12.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    assert!(lint_single_rule(&clean_doc, "tiny-text").is_empty());
}

#[test]
fn fail_and_clean_detached_text_style() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_text(
        &mut fail_doc,
        fail_page,
        "BodyCopy",
        Rect::new(0.0, 0.0, 120.0, 20.0),
        "Unlinked typography",
        TextStyle { font_size: 14.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    let f = lint_single_rule(&fail_doc, "detached-text-style");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let clean = add_text(
        &mut clean_doc,
        clean_page,
        "BodyCopy",
        Rect::new(0.0, 0.0, 120.0, 20.0),
        "Linked typography",
        TextStyle { font_size: 14.0, ..Default::default() },
        TextAutoResize::WidthAndHeight,
    );
    clean_doc.get_mut(clean).unwrap().style_refs.insert("text".into(), "Body/Regular".into());
    assert!(lint_single_rule(&clean_doc, "detached-text-style").is_empty());
}

#[test]
fn fail_and_clean_hardcoded_color() {
    let (mut fail_doc, fail_page) = new_doc_with_page("Fail");
    let bad = add_rect(&mut fail_doc, fail_page, "RawChip", Rect::new(0.0, 0.0, 80.0, 40.0));
    let f = lint_single_rule(&fail_doc, "hardcoded-color");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].node, bad);

    let (mut clean_doc, clean_page) = new_doc_with_page("Clean");
    let clean = add_rect(&mut clean_doc, clean_page, "BoundChip", Rect::new(0.0, 0.0, 80.0, 40.0));
    clean_doc.get_mut(clean).unwrap().variables.bindings.insert("fills/0".into(), "color/primary".into());
    assert!(lint_single_rule(&clean_doc, "hardcoded-color").is_empty());
}

/// Builds the 112,474-node (723,136 Kiwi-record scale) synthetic Figma Design System v2 page.
fn build_synthetic_design_system_page(layer_count: usize) -> (Document, NodeId) {
    let (mut doc, page) = new_doc_with_page("Figma Design System v2 (112,474 nodes / 723,136 records)");
    let per_card = 50usize;
    let mut created = 0usize;
    let mut card_idx = 0usize;
    let mut page_children = Vec::with_capacity(layer_count / per_card + 2);

    while created < layer_count {
        let card_id = doc.alloc_id();
        let cx = (card_idx % 25) as f32 * 240.0;
        let cy = (card_idx / 25) as f32 * 180.0;
        let mut card = Node::frame(card_id, Rect::new(cx, cy, 200.0, 140.0));
        card.name = format!("Card {card_idx}");
        card.parent = Some(page);
        card.fills = vec![Paint::solid(Color::WHITE)];
        page_children.push(card_id);
        created += 1;
        card_idx += 1;

        let children_to_make = (layer_count - created).min(per_card - 1);
        card.children.reserve(children_to_make);
        for i in 0..children_to_make {
            let id = doc.alloc_id();
            let rx = (i % 10) as f32 * 18.0 + 4.0;
            let ry = (i / 10) as f32 * 22.0 + 4.0;
            let mut n = if i == 0 && card_idx.is_multiple_of(100) {
                // Overflowing child + fixed-box text so both geometric and cosmic-text rules fire.
                let mut r = Node::rectangle(id, Rect::new(rx, ry, 260.0, 16.0));
                r.name = format!("OverflowSwatch {card_idx}");
                r.fills = vec![Paint::solid(Color::rgb(30, 100, 220))];
                r
            } else if i == 1 && card_idx.is_multiple_of(200) {
                let ts = TextStyle { font_size: 14.0, font_weight: 400, fills: vec![Paint::solid(Color::rgb(34, 34, 34))], ..TextStyle::default() };
                let mut t = Node::text(id, Rect::new(rx, ry, 48.0, 18.0), "Design system label", ts);
                t.name = format!("Label {card_idx}");
                if let NodeKind::Text(tp) = &mut t.kind {
                    tp.auto_resize = TextAutoResize::None;
                }
                t
            } else if i % 5 == 0 {
                let ts = TextStyle { font_size: 14.0, font_weight: 400, fills: vec![Paint::solid(Color::rgb(34, 34, 34))], ..TextStyle::default() };
                let mut t = Node::text(id, Rect::new(rx, ry, 24.0, 16.0), "OK", ts);
                t.name = format!("Tag {i}");
                t
            } else {
                let mut r = Node::rectangle(id, Rect::new(rx, ry, 16.0, 16.0));
                r.name = format!("Box {i}");
                r.fills = vec![Paint::solid(Color::rgb(238, 238, 238))];
                r
            };
            n.parent = Some(card_id);
            card.children.push(id);
            doc.nodes.insert(id, n);
            created += 1;
        }
        doc.nodes.insert(card_id, card);
    }
    doc.get_mut(page).unwrap().children = page_children;
    (doc, page)
}

#[test]
fn large_fixture_off_thread_budget() {
    use designos_doc::lint::Budget;
    use std::sync::Arc;
    use std::time::Instant;

    // 112,474 scene nodes (the 723,136-Kiwi-record VSF-PCP / Figma Design System v2 benchmark scale).
    const FIXTURE_NODES: usize = 112_474;
    let (doc, page) = build_synthetic_design_system_page(FIXTURE_NODES);
    let full_cfg = LintConfig::from_preset(LintPreset::Agent);

    let shared_doc = Arc::new(doc);
    let roots = vec![page];
    let total_start = Instant::now();
    let worker = designos_render::lint_env::lint_on_worker_with_budget(
        Arc::clone(&shared_doc),
        roots.clone(),
        full_cfg.clone(),
        512,
        Duration::from_millis(4),
        Budget::unlimited(),
    );

    // Simulate real UI-thread frame work (viewport culling + hit-test traversal across top-level frames)
    // concurrently while the worker thread executes `lint_in_slices_with_budget`.
    let viewport = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let mut worst_ui_frame = Duration::ZERO;
    let mut ui_frames = 0usize;
    let mut visible_hits = 0usize;
    while !worker.is_finished() {
        let frame_start = Instant::now();
        if let Some(page_node) = shared_doc.get(page) {
            for &card_id in page_node.children.iter().take(256) {
                if let Some(card) = shared_doc.get(card_id) {
                    if card.local_bounds().intersects(&viewport) {
                        visible_hits += card.children.len();
                    }
                }
            }
        }
        let frame_elapsed = frame_start.elapsed();
        worst_ui_frame = worst_ui_frame.max(frame_elapsed);
        ui_frames += 1;
        std::thread::yield_now();
    }
    std::hint::black_box(visible_hits);
    let stats = worker.join().expect("worker thread panicked");
    let total_elapsed = total_start.elapsed();

    assert!(
        worst_ui_frame <= Duration::from_millis(8),
        "UI frame budget exceeded while worker linted 112,474 nodes: worst_ui_frame={worst_ui_frame:?} (frames={ui_frames})"
    );
    assert!(total_elapsed <= Duration::from_secs(6), "total worker lint elapsed {total_elapsed:?} exceeded 6s budget");
    assert_eq!(stats.report.scanned, FIXTURE_NODES, "worker scan must visit all 112,474 nodes");
    assert!(stats.slices >= 200, "expected 112,474-node worker scan to span >= 200 slices of 512 nodes, got {}", stats.slices);
    assert!(stats.report.complete);
    assert!(stats.report.cursor.is_none());
    assert!(!stats.report.findings.is_empty());

    // Verify budgeted worker calls stop when the call budget expires (`complete: false`, `cursor: "stack:..."`)
    // and resume across calls without rescanning earlier nodes.
    let mut resumed_scanned = 0usize;
    let mut resumed_findings = Vec::new();
    let mut next_cursor: Option<String> = None;
    let mut chunks = 0usize;
    loop {
        let mut call_budget = Budget::with_limits(40_000, None);
        call_budget.cursor = next_cursor.clone();
        let step_stats = designos_render::lint_env::lint_scoped_on_worker(&shared_doc, &roots, &full_cfg, 512, Duration::from_millis(4), &mut call_budget);
        chunks += 1;
        resumed_scanned += step_stats.report.scanned;
        resumed_findings.extend(step_stats.report.findings);
        if step_stats.report.complete {
            assert!(step_stats.report.cursor.is_none());
            break;
        }
        assert!(step_stats.report.truncated);
        let cur = step_stats.report.cursor.clone().expect("incomplete budgeted run must emit stack:<id> cursor");
        assert!(cur.starts_with("stack:"), "expected stack cursor, got {cur}");
        next_cursor = Some(cur);
    }
    assert_eq!(chunks, 3, "112,474 nodes with a 40,000-node call budget must complete in 3 resumed calls");
    assert_eq!(resumed_scanned, FIXTURE_NODES, "stack cursor resumption must scan every node exactly once without rescanning");
    resumed_findings.sort_by_key(|f| designos_doc::lint::rules::severity_rank(f.severity));
    let mut full_findings = stats.report.findings;
    full_findings.sort_by_key(|f| designos_doc::lint::rules::severity_rank(f.severity));
    assert_eq!(resumed_findings, full_findings);
}
