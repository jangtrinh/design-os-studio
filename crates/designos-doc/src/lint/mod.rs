//! Unified solver-backed geometric and semantic design linter (E15-9 / #594).

pub mod config;
pub mod finding;
pub mod fix;
pub mod registry;
pub mod rules;
pub mod style_bindings;
pub mod traverse;

pub use config::{LintConfig, LintConfigError, LintThresholds, RuleSeverityConfig, RuleSeverityLevel};
pub use finding::{Finding, FixSuggestion, LintFinding, LintFix, LintPage, LintReport, LintSummary, SkippedRule};
pub use fix::{lint_fix_commands, lint_fix_document, lint_fix_with_history};
pub use registry::{
    all_preset_names, all_rule_ids, canonical_rule_id, preset_rule_ids, resolve_rule, rule_aliases, Category, LintPreset, RuleNeeds, RuleSpec, Severity, RULES,
};
pub use style_bindings::{broken_style_binding_findings, broken_style_binding_findings_for, style_binding_check_enabled};
pub use traverse::{lint_in_slices, lint_in_slices_with_budget, lint_subtree, lint_subtree_with_components, Budget, SliceRunStats};

use crate::{Color, Document, LineHeight, Node, NodeId, Rect, TextAutoResize, TextProps, Vec2};

/// Measured text metrics returned by a real font engine (`cosmic-text`) or an approximation.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub line_count: usize,
    pub visible_lines: usize,
    pub content_height: f32,
    pub widest_line: f32,
    pub breaks_inside_word: Vec<usize>,
    pub substituted_family: Option<String>,
}

/// Sampled backdrop color behind a node.
#[derive(Debug, Clone, PartialEq)]
pub struct Backdrop {
    pub color: Color,
    pub uniform: bool,
}

/// Optional measurement environment supplied by the caller.
///
/// `designos-doc` cannot depend on `cosmic-text` or the GPU renderer, so rules that need
/// real text layout or backdrop sampling query this trait.
pub trait LintEnv {
    /// Whether a real text-shaping engine is available. When `false`, rules with
    /// `RuleNeeds::Text` are skipped and listed in `skipped_rules` with reason `"no_text_engine"`.
    fn has_text_engine(&self) -> bool {
        false
    }

    /// Measure a text node's layout metrics at `size`.
    fn text_metrics(&self, _node: &Node, _props: &TextProps, _size: Vec2) -> Option<TextMetrics> {
        None
    }

    /// Sample the effective rendered backdrop color behind `rect` on `node`'s page.
    fn backdrop(&self, _doc: &Document, _node: NodeId, _rect: Rect) -> Option<Backdrop> {
        None
    }
}

/// Headless environment without `cosmic-text` or renderer backdrop sampling.
///
/// Rules requiring `RuleNeeds::Text` are recorded in `skipped_rules` with reason `"no_text_engine"`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLintEnv;

impl LintEnv for NoLintEnv {}

/// Heuristic text-measurement environment used ONLY by the legacy `openpencil::lint_document`
/// compatibility wrapper when callers invoke `designos_doc::openpencil::lint_document` directly
/// without `designos-render`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ApproxLintEnv;

impl LintEnv for ApproxLintEnv {
    fn has_text_engine(&self) -> bool {
        true
    }

    fn text_metrics(&self, _node: &Node, props: &TextProps, size: Vec2) -> Option<TextMetrics> {
        let font_size = props.style.font_size.max(1.0);
        let line_height = match props.style.line_height {
            LineHeight::Auto => font_size * 1.2,
            LineHeight::Pixels(px) => px.max(1.0),
            LineHeight::Percent(pct) => (font_size * pct / 100.0).max(1.0),
        };
        let avg_char_w = font_size * 0.55;
        let wrap_w = (props.auto_resize != TextAutoResize::WidthAndHeight).then_some(size.x);
        let explicit_lines: Vec<&str> = props.content.split('\n').collect();
        let mut widest_line = 0.0_f32;
        let mut total_lines = 0usize;
        let mut breaks_inside_word = Vec::new();

        for line in &explicit_lines {
            let line_w = line.chars().count() as f32 * avg_char_w;
            if let Some(w) = wrap_w {
                if w > 0.0 && line_w > w {
                    let wrapped = (line_w / w).ceil() as usize;
                    total_lines += wrapped.max(1);
                    widest_line = widest_line.max(w);
                    for word in line.split_whitespace() {
                        if word.chars().count() as f32 * avg_char_w > w {
                            breaks_inside_word.push(0);
                        }
                    }
                } else {
                    total_lines += 1;
                    widest_line = widest_line.max(line_w);
                }
            } else {
                total_lines += 1;
                widest_line = widest_line.max(line_w);
            }
        }
        total_lines = total_lines.max(1);
        let content_height = total_lines as f32 * line_height;
        let max_by_height = if matches!(props.auto_resize, TextAutoResize::None | TextAutoResize::Truncate) {
            ((size.y / line_height).floor() as usize).max(1)
        } else {
            total_lines
        };
        let visible_lines = match props.max_lines {
            Some(max_l) if props.auto_resize == TextAutoResize::Truncate => total_lines.min(max_l as usize).min(max_by_height),
            _ => total_lines.min(max_by_height),
        };
        Some(TextMetrics { line_count: total_lines, visible_lines, content_height, widest_line, breaks_inside_word, substituted_family: None })
    }
}

/// Primary entry point for linting a document across all pages with an unlimited budget.
pub fn lint(doc: &Document, config: &LintConfig, env: &dyn LintEnv) -> LintReport {
    let mut budget = Budget::unlimited();
    lint_subtree(doc, &doc.pages, config, env, &mut budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AutoLayout, Color, Document, FrameProps, History, LayoutDirection, Node, NodeKind, Paint, Rect, Sizing, TextAutoResize, TextStyle};

    #[test]
    fn rule_id_aliases_resolve_both_ways() {
        let pairs = [
            ("overflow-parent", "overflowing-children"),
            ("overflow-parent", "child-overflow"),
            ("sibling-overlap", "overlapping-siblings"),
            ("sibling-overlap", "overlap"),
            ("collapsed-size", "zero-bounds"),
            ("non-4px-grid", "non-integer-bounds"),
            ("non-4px-grid", "subpixel-position"),
            ("tiny-text", "small-text"),
            ("small-touch-target", "touch-target"),
            ("misaligned-siblings", "unaligned-siblings"),
            ("misaligned-siblings", "inconsistent-sibling-alignment"),
            ("inconsistent-spacing", "irregular-autolayout-spacing"),
            ("clipped-partial", "clipped-content"),
            ("detached-instance", "REQUIRE_NO_DETACHED_COMPONENTS"),
            ("hardcoded-color", "REQUIRE_FILL_COLOR_VARIABLES"),
            ("hardcoded-color", "color-token-compliance"),
            ("non-scale-font-size", "raw-font-size"),
            ("detached-text-style", "REQUIRE_TEXT_STYLES"),
        ];

        for (canonical, alias) in pairs {
            let c_meta = resolve_rule(canonical).unwrap_or_else(|| panic!("canonical {canonical} failed to resolve"));
            let a_meta = resolve_rule(alias).unwrap_or_else(|| panic!("alias {alias} failed to resolve"));
            assert_eq!(c_meta.id, canonical);
            assert_eq!(a_meta.id, canonical);
            assert_eq!(canonical_rule_id(alias), Some(canonical));
            assert!(rule_aliases(canonical).contains(&alias));
        }
    }

    #[test]
    fn presets_name_only_registered_rules() {
        for preset in [LintPreset::Agent, LintPreset::CheckDesigns, LintPreset::Recommended, LintPreset::Strict] {
            let ids = preset_rule_ids(preset);
            assert!(!ids.is_empty());
            for id in ids {
                assert!(resolve_rule(id).is_some(), "preset {:?} references unknown rule {}", preset, id);
            }
        }
    }

    #[test]
    fn presets_select_expected_rule_sets() {
        let agent = LintConfig::from_preset_str("agent").unwrap();
        assert!(agent.is_enabled("overflow-parent"));
        assert!(agent.is_enabled("clipped-partial"));
        assert!(agent.is_enabled("clipped-full"));
        assert!(agent.is_enabled("sibling-overlap"));
        assert!(agent.is_enabled("text-truncated"));
        assert!(agent.is_enabled("text-unexpected-wrap"));
        assert!(agent.is_enabled("collapsed-size"));
        assert!(agent.is_enabled("sizing-conflict"));
        assert!(agent.is_enabled("text-missing-fill"));
        assert!(agent.is_enabled("absolute-in-auto-layout"));
        assert!(agent.is_enabled("placeholder-text"));
        assert!(!agent.is_enabled("detached-instance"));
        assert!(!agent.is_enabled("hardcoded-color"));

        let check_designs = LintConfig::from_preset_str("check-designs").unwrap();
        assert!(check_designs.is_enabled("detached-instance"));
        assert!(!check_designs.is_enabled("default-layer-name"));

        let recommended = LintConfig::from_preset_str("recommended").unwrap();
        assert!(recommended.is_enabled("overflow-parent"));
        assert!(recommended.is_enabled("detached-instance"));
        assert!(recommended.is_enabled("default-layer-name"));
        assert!(recommended.is_enabled("empty-container"));
        assert!(!recommended.is_enabled("hardcoded-color"));

        let strict = LintConfig::from_preset_str("strict").unwrap();
        for meta in RULES {
            assert!(strict.is_enabled(meta.id), "strict preset must enable {}", meta.id);
        }
    }

    #[test]
    fn no_lint_env_records_skipped_text_rules() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let mut txt = Node::text(tid, Rect::new(0.0, 0.0, 50.0, 20.0), "Hello world", TextStyle::default());
        txt.name = "Heading".into();
        if let NodeKind::Text(tp) = &mut txt.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        doc.insert(page, txt).unwrap();

        let cfg = LintConfig::from_preset_str("agent").unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        let skipped_ids: Vec<&str> = report.skipped_rules.iter().map(|s| s.rule.as_str()).collect();
        assert!(skipped_ids.contains(&"text-truncated"));
        assert!(skipped_ids.contains(&"text-unexpected-wrap"));
        assert!(report.skipped_rules.iter().all(|s| s.reason == "no_text_engine"));
    }

    #[test]
    fn sibling_overlap_sweep_prune_caps_at_1000_pairs() {
        let mut doc = Document::new("Overlap Cap");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 500.0, 500.0));
        frame.name = "Dense Frame".into();
        doc.insert(page, frame).unwrap();

        // 50 staggered overlapping rects -> 50 * 49 / 2 = 1225 pairs > 1000 cap.
        for i in 0..50 {
            let sid = doc.alloc_id();
            let off = i as f32;
            let mut r = Node::rectangle(sid, Rect::new(10.0 + off, 10.0 + off, 100.0, 100.0));
            r.name = format!("Item {i}");
            r.fills = vec![Paint::solid(Color::BLACK)];
            doc.insert(fid, r).unwrap();
        }

        let cfg = LintConfig::from_rule_list(&["sibling-overlap".to_string()]).unwrap();
        let report = lint(&doc, &cfg, &NoLintEnv);
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0].message.contains("truncated at 1000"));
    }

    #[test]
    fn fix_routes_through_command_batch_and_supports_undo() {
        let mut doc = Document::new("Fix Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(10.0, 20.0, 103.0, 40.0));
        frame.name = "Frame 1".into();
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 11.0, primary_sizing: Sizing::Fixed, ..Default::default() }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();

        let mut history = History::default();
        let res = lint_fix_with_history(&mut doc, &mut history, Some(page), None).unwrap();
        assert!(res["fixed_count"].as_u64().unwrap() >= 2);
        assert!(history.can_undo());

        // Undo the single batch command and verify original state is restored.
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.get(fid).unwrap().name, "Frame 1");
        assert!((doc.get(fid).unwrap().size.x - 103.0).abs() < 1e-4);
    }
}
