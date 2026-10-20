//! Per-call lint `issues` block attached to every design-mutating MCP reply (`E15-12` / `#597`).

use crate::tools::session_options::{IssuesMode, IssuesOptions, PerCallLintOverride};
use designos_doc::lint::{lint_subtree, Budget, Finding, LintConfig, LintEnv, Severity, RULES};
use designos_doc::{Document, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::time::Duration;

/// Maximum nodes in a parent container subtree before falling back to `r`'s subtree + direct siblings (`E15-12` §3.2).
pub const MAX_PARENT_SCOPE_NODES: usize = 2_000;

/// Maximum nodes visited per mutating call (`E15-12` §3.2, §11).
pub const PER_CALL_MAX_NODES: usize = 2_000;

/// Maximum wall-clock duration for a per-call lint pass (`E15-12` §3.3, §11).
pub const PER_CALL_MAX_DURATION: Duration = Duration::from_millis(50);

/// Maximum scope size for running a pre-call baseline lint to mark `preexisting: true` (`E15-12` §8.3, §11).
pub const MAX_PREEXISTING_SCOPE_NODES: usize = 500;

/// Compact fix suggestion without `from` (`E15-12` §3.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompactFix {
    pub kind: String,
    pub property: String,
    pub to: Value,
}

/// Compact finding item returned inside the `issues` array of a mutating MCP reply (`E15-12` §3.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IssueItem {
    pub rule: String,
    pub severity: Severity,
    pub node_id: u64,
    pub node_name: String,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<CompactFix>,
    pub auto_fixable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preexisting: Option<bool>,
}

/// Computed lint scope for a set of touched roots (`E15-12` §3.2).
#[derive(Clone, Debug, Default)]
pub struct LintScope {
    /// Deduplicated root node IDs to pass to [`lint_subtree`].
    pub roots: Vec<NodeId>,
    /// Direct siblings whose own node rules are evaluated without descending into their children.
    pub shallow_nodes: BTreeSet<NodeId>,
    /// Total estimated nodes in the scope (bounded by `MAX_PARENT_SCOPE_NODES + 1`).
    pub estimated_nodes: usize,
}

/// Counts nodes in `root`'s subtree, stopping early once the count exceeds `cap`.
pub fn count_subtree_nodes_bounded(doc: &Document, root: NodeId, cap: usize) -> usize {
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if !matches!(n.kind, NodeKind::Page { .. }) {
            count += 1;
            if count > cap {
                return count;
            }
        }
        for &cid in n.children.iter().rev() {
            stack.push(cid);
        }
    }
    count
}

/// Builds the layer-name path from the top-level frame under the page down to `id` (`E15-12` §3.4).
pub fn layer_name_path(doc: &Document, id: NodeId) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(id);
    while let Some(nid) = cur {
        let Some(n) = doc.get(nid) else { break };
        if matches!(n.kind, NodeKind::Page { .. }) {
            break;
        }
        let seg = if n.name.trim().is_empty() { n.kind.default_name() } else { n.name.as_str() };
        parts.push(seg.to_string());
        cur = n.parent;
    }
    parts.reverse();
    parts.join(" / ")
}

/// Resolves the deduplicated lint scope for `touched_roots` (`E15-12` §3.2).
///
/// For each touched root `r`:
/// 1. If `r` has a non-Page parent container `p`, the candidate scope is `p`'s subtree so sibling
///    overlap, overflow, and hug collapse caused by the edit are caught.
/// 2. If `p`'s subtree exceeds 2,000 nodes, `p` is still evaluated along with `r`'s direct siblings
///    (shallow, without their descendants) and `r`'s full subtree.
/// 3. Candidate scopes inside another full scope root are deduplicated.
pub fn compute_lint_scope(doc: &Document, touched_roots: &[NodeId]) -> LintScope {
    let mut touched_set: BTreeSet<NodeId> = BTreeSet::new();
    for &r in touched_roots {
        if doc.get(r).is_some() {
            touched_set.insert(r);
        }
    }
    if touched_set.is_empty() {
        return LintScope::default();
    }

    let mut full_roots: Vec<NodeId> = Vec::new();
    let mut large_parents: Vec<(NodeId, NodeId)> = Vec::new();

    for &r in &touched_set {
        let Some(r_node) = doc.get(r) else { continue };
        if matches!(r_node.kind, NodeKind::Page { .. }) {
            // A page root is only passed when set_variables / set_style had no specific target node.
            // Never lint an entire huge page: only include if the page has <= 2,000 nodes.
            if count_subtree_nodes_bounded(doc, r, MAX_PARENT_SCOPE_NODES) <= MAX_PARENT_SCOPE_NODES {
                for &cid in &r_node.children {
                    if !full_roots.contains(&cid) {
                        full_roots.push(cid);
                    }
                }
            }
            continue;
        }

        let parent_container = r_node.parent.and_then(|pid| doc.get(pid).filter(|pn| !matches!(pn.kind, NodeKind::Page { .. })).map(|_| pid));
        if let Some(pid) = parent_container {
            let count = count_subtree_nodes_bounded(doc, pid, MAX_PARENT_SCOPE_NODES);
            if count <= MAX_PARENT_SCOPE_NODES {
                if !full_roots.contains(&pid) {
                    full_roots.push(pid);
                }
            } else {
                large_parents.push((pid, r));
            }
        } else if !full_roots.contains(&r) {
            full_roots.push(r);
        }
    }

    // Deduplicate full_roots: drop any root `a` that is a strict descendant of another full_root `b`.
    let mut dedup_roots: Vec<NodeId> = Vec::with_capacity(full_roots.len());
    for &candidate in &full_roots {
        let covered = full_roots.iter().any(|&other| other != candidate && doc.is_ancestor(other, candidate));
        if !covered && !dedup_roots.contains(&candidate) {
            dedup_roots.push(candidate);
        }
    }

    let mut shallow_nodes = BTreeSet::new();
    for (pid, r) in large_parents {
        let covered_by_full = dedup_roots.iter().any(|&root| root == pid || doc.is_ancestor(root, pid));
        if covered_by_full {
            continue;
        }
        if !dedup_roots.contains(&pid) {
            dedup_roots.push(pid);
        }
        if let Some(pnode) = doc.get(pid) {
            for &sib in &pnode.children {
                if sib != r && !touched_set.contains(&sib) && !touched_set.iter().any(|&t| doc.is_ancestor(sib, t)) {
                    shallow_nodes.insert(sib);
                }
            }
        }
    }

    let mut estimated_nodes = 0usize;
    for &root in &dedup_roots {
        if shallow_nodes.is_empty() {
            estimated_nodes = estimated_nodes.saturating_add(count_subtree_nodes_bounded(doc, root, MAX_PARENT_SCOPE_NODES));
        } else {
            estimated_nodes = MAX_PREEXISTING_SCOPE_NODES + 1;
        }
    }

    LintScope { roots: dedup_roots, shallow_nodes, estimated_nodes }
}

/// Pre-call baseline of `(rule, node_id)` findings captured when the scope has `<= 500` nodes (`E15-12` §8.3).
#[derive(Clone, Debug, Default)]
pub struct PreCallBaseline {
    findings: Option<BTreeSet<(&'static str, NodeId)>>,
}

impl PreCallBaseline {
    pub fn none() -> Self {
        Self { findings: None }
    }

    /// Captures pre-existing findings on `doc` before a write if the scope around `candidate_roots`
    /// has `<= 500` nodes and linting is enabled.
    pub fn capture(
        doc: &Document,
        candidate_roots: &[NodeId],
        options: &IssuesOptions,
        per_call_override: Option<PerCallLintOverride>,
        env: &dyn LintEnv,
    ) -> Self {
        let mode = PerCallLintOverride::resolve(per_call_override, options.mode);
        if mode == IssuesMode::Off {
            return Self::none();
        }
        let scope = compute_lint_scope(doc, candidate_roots);
        Self::capture_scope(doc, scope.roots, scope.shallow_nodes, scope.estimated_nodes, options, env)
    }

    /// Captures pre-existing findings inside `parent_container` before inserting a new child node
    /// (`designos_create_node`, `designos_apply_spec`).
    pub fn capture_container(
        doc: &Document,
        parent_container: NodeId,
        options: &IssuesOptions,
        per_call_override: Option<PerCallLintOverride>,
        env: &dyn LintEnv,
    ) -> Self {
        let mode = PerCallLintOverride::resolve(per_call_override, options.mode);
        if mode == IssuesMode::Off {
            return Self::none();
        }
        let Some(pnode) = doc.get(parent_container) else {
            return Self::none();
        };
        if matches!(pnode.kind, NodeKind::Page { .. }) {
            return Self::none();
        }
        let estimated = count_subtree_nodes_bounded(doc, parent_container, MAX_PREEXISTING_SCOPE_NODES);
        Self::capture_scope(doc, vec![parent_container], BTreeSet::new(), estimated, options, env)
    }

    /// Captures pre-existing findings on `pre_doc` matching the post-write scope computed on `post_doc`
    /// (`designos_apply_batch`, `designos_apply_spec`).
    pub fn capture_from_post_doc(
        pre_doc: &Document,
        post_doc: &Document,
        touched_roots: &[NodeId],
        options: &IssuesOptions,
        per_call_override: Option<PerCallLintOverride>,
        env: &dyn LintEnv,
    ) -> Self {
        let mode = PerCallLintOverride::resolve(per_call_override, options.mode);
        if mode == IssuesMode::Off {
            return Self::none();
        }
        let post_scope = compute_lint_scope(post_doc, touched_roots);
        if post_scope.roots.is_empty() || post_scope.estimated_nodes > MAX_PREEXISTING_SCOPE_NODES {
            return Self::none();
        }
        let pre_roots: Vec<NodeId> = post_scope.roots.into_iter().filter(|&r| pre_doc.get(r).is_some()).collect();
        if pre_roots.is_empty() {
            return Self { findings: Some(BTreeSet::new()) };
        }
        let pre_shallow: BTreeSet<NodeId> = post_scope.shallow_nodes.into_iter().filter(|&s| pre_doc.get(s).is_some()).collect();
        Self::capture_scope(pre_doc, pre_roots, pre_shallow, post_scope.estimated_nodes, options, env)
    }

    fn capture_scope(
        doc: &Document,
        roots: Vec<NodeId>,
        shallow_nodes: BTreeSet<NodeId>,
        estimated_nodes: usize,
        options: &IssuesOptions,
        env: &dyn LintEnv,
    ) -> Self {
        if roots.is_empty() || estimated_nodes > MAX_PREEXISTING_SCOPE_NODES {
            return Self::none();
        }
        let Ok(cfg) = LintConfig::from_preset_str(&options.preset) else {
            return Self::none();
        };
        let mut budget = Budget::with_limits(MAX_PREEXISTING_SCOPE_NODES, Some(PER_CALL_MAX_DURATION));
        budget.shallow_nodes = shallow_nodes;
        let report = lint_subtree(doc, &roots, &cfg, env, &mut budget);
        if !report.complete {
            return Self::none();
        }
        let set = report.findings.into_iter().map(|f| (f.rule, f.node)).collect();
        Self { findings: Some(set) }
    }

    fn contains(&self, rule: &'static str, node: NodeId) -> Option<bool> {
        self.findings.as_ref().and_then(|set| set.contains(&(rule, node)).then_some(true))
    }
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

fn rule_order_index(rule_id: &str) -> usize {
    RULES.iter().position(|r| r.id == rule_id).unwrap_or(RULES.len())
}

fn is_safe_tier_auto_fixable(f: &Finding) -> bool {
    f.fixable && matches!(f.rule, "sizing-conflict" | "hardcoded-value")
}

/// Evaluates the per-call lint `issues` block on `doc` for `touched_roots` and inserts the
/// `issues*` fields into `reply` (`E15-12` §3.1–§3.5, §4, §5, §8).
///
/// When effective mode is `Off` (via session options or per-call `lint: false`), `reply` is left
/// untouched with no `issues*` keys.
pub fn attach_issues_block(
    doc: &Document,
    touched_roots: &[NodeId],
    options: &IssuesOptions,
    per_call_override: Option<PerCallLintOverride>,
    baseline: &PreCallBaseline,
    env: &dyn LintEnv,
    reply: &mut Value,
) {
    let mode = PerCallLintOverride::resolve(per_call_override, options.mode);
    if mode == IssuesMode::Off {
        if let Some(obj) = reply.as_object_mut() {
            obj.remove("issues");
            obj.remove("issues_total");
            obj.remove("issues_truncated");
            obj.remove("issues_partial");
            obj.remove("issues_scanned");
            obj.remove("issues_skipped_rules");
        }
        return;
    }

    let scope = compute_lint_scope(doc, touched_roots);
    let cfg = LintConfig::from_preset_str(&options.preset).unwrap_or_default();
    let max_items = options.max_items.clamp(1, 100);

    let mut budget = Budget::with_limits(PER_CALL_MAX_NODES, Some(PER_CALL_MAX_DURATION));
    budget.shallow_nodes = scope.shallow_nodes;

    let report = if scope.roots.is_empty() {
        designos_doc::lint::LintReport { complete: true, ..Default::default() }
    } else {
        lint_subtree(doc, &scope.roots, &cfg, env, &mut budget)
    };

    let mut findings: Vec<Finding> = report
        .findings
        .into_iter()
        .filter(|f| match mode {
            IssuesMode::ErrorsOnly => f.severity == Severity::Error,
            IssuesMode::Preset | IssuesMode::Off => true,
        })
        .collect();

    // Sort by severity, then rule order in the registry/preset, then paint order (stable sort).
    findings.sort_by_key(|f| (severity_rank(f.severity), rule_order_index(f.rule)));

    let issues_total = findings.len();
    let issues_truncated = issues_total > max_items || !report.complete;

    let items: Vec<IssueItem> = findings
        .into_iter()
        .take(max_items)
        .map(|f| {
            let preexisting = baseline.contains(f.rule, f.node);
            let auto_fixable = is_safe_tier_auto_fixable(&f);
            let fix = f.suggested_fix.map(|s| CompactFix { kind: s.kind, property: s.property, to: s.to });
            IssueItem {
                rule: f.rule.to_string(),
                severity: f.severity,
                node_id: f.node.0,
                node_name: f.node_name,
                path: layer_name_path(doc, f.node),
                message: f.message,
                fix,
                auto_fixable,
                preexisting,
            }
        })
        .collect();

    if let Some(obj) = reply.as_object_mut() {
        obj.insert("issues".into(), serde_json::to_value(items).unwrap_or_else(|_| json!([])));
        obj.insert("issues_total".into(), json!(issues_total));
        obj.insert("issues_truncated".into(), json!(issues_truncated));
        if !report.complete {
            obj.insert("issues_partial".into(), json!(true));
            obj.insert("issues_scanned".into(), json!(report.scanned));
        }
        if !report.skipped_rules.is_empty() {
            obj.insert("issues_skipped_rules".into(), serde_json::to_value(&report.skipped_rules).unwrap_or_else(|_| json!([])));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::Engine;
    use crate::server::Session;
    use designos_doc::{IgnoreKey, Node, NodeId, Rect};
    use serde_json::{json, Value};

    fn strip_issues_keys(mut v: Value) -> Value {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("issues");
            obj.remove("issues_total");
            obj.remove("issues_truncated");
            obj.remove("issues_partial");
            obj.remove("issues_scanned");
            obj.remove("issues_skipped_rules");
        }
        v
    }

    #[test]
    fn clipped_text_named_in_reply() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "PlanHeader",
                        "kind": "frame",
                        "width": 100.0,
                        "height": 40.0,
                        "clips_content": true,
                        "children": [
                            {
                                "name": "Enterprise annual plan",
                                "kind": "text",
                                "text": "Enterprise annual plan",
                                "font_size": 16.0,
                                "width": 200.0,
                                "height": 20.0
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec");

        let issues = reply["issues"].as_array().expect("issues array");
        assert!(!issues.is_empty(), "expected issues on clipped 200x20 text in 100x40 frame: {reply}");
        assert!(reply["issues_total"].as_u64().unwrap() >= 1);
        assert_eq!(reply["issues_truncated"], false);
        assert!(
            issues.iter().any(|item| {
                let rule = item["rule"].as_str().unwrap_or("");
                (rule == "clipped-partial" || rule == "text-truncated" || rule == "overflow-parent")
                    && item["node_name"] == "Enterprise annual plan"
                    && item["path"].as_str().unwrap_or("").contains("PlanHeader / Enterprise annual plan")
            }),
            "expected clipped-partial or text-truncated naming 'Enterprise annual plan', got {issues:?}"
        );
    }

    #[test]
    fn apply_spec_returns_issues_block() {
        clipped_text_named_in_reply();
    }

    #[test]
    fn clean_write_returns_empty_issues() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "CleanCard",
                        "kind": "frame",
                        "layout": {
                            "direction": "vertical",
                            "padding": [16.0, 16.0, 16.0, 16.0],
                            "gap": 8.0
                        },
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "Headline",
                                "kind": "text",
                                "text": "Ready",
                                "font_size": 16.0,
                                "text_color": "#FFFFFF",
                                "width": 200.0,
                                "height": 24.0
                            }
                        ]
                    }
                }),
            )
            .expect("clean apply_spec");

        assert_eq!(reply["issues"], json!([]));
        assert_eq!(reply["issues_total"], 0);
        assert_eq!(reply["issues_truncated"], false);
    }

    #[test]
    fn clean_frame_returns_zero_issues() {
        clean_write_returns_empty_issues();
    }

    #[test]
    fn off_removes_only_the_block() {
        let spec = json!({
            "parent_id": 1,
            "spec": {
                "name": "Card",
                "kind": "frame",
                "width": 100.0,
                "height": 40.0,
                "clips_content": true,
                "children": [
                    {
                        "name": "WideBox",
                        "kind": "rectangle",
                        "x": 0.0,
                        "y": 0.0,
                        "width": 180.0,
                        "height": 40.0,
                        "fill": "#FF5500"
                    }
                ]
            }
        });

        let mut e_preset = Engine::default();
        let reply_preset = e_preset.call("designos_apply_spec", spec.clone()).expect("preset apply_spec");
        assert!(reply_preset.get("issues").is_some());
        assert!(reply_preset["issues_total"].as_u64().unwrap() > 0);

        let mut e_off = Engine::default();
        e_off.call("designos_configure_session", json!({ "issues": { "mode": "off" } })).expect("configure off");
        let reply_off = e_off.call("designos_apply_spec", spec).expect("off apply_spec");
        assert!(reply_off.get("issues").is_none());
        assert!(reply_off.get("issues_total").is_none());
        assert!(reply_off.get("issues_truncated").is_none());

        assert_eq!(strip_issues_keys(reply_preset), reply_off);
    }

    #[test]
    fn lint_false_skips_lint_pass() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": 1,
                    "kind": "rectangle",
                    "name": "CollapsedRect",
                    "width": 0.0,
                    "height": 40.0,
                    "lint": false
                }),
            )
            .expect("create_node with lint: false");
        assert!(reply.get("issues").is_none());
        assert!(reply.get("issues_total").is_none());
        assert!(reply.get("issues_truncated").is_none());

        let batch_reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "lint": false,
                    "ops": [
                        {
                            "op": "insert",
                            "parent": 1,
                            "kind": "rectangle",
                            "name": "CollapsedBatchRect",
                            "width": 0.0,
                            "height": 20.0
                        }
                    ]
                }),
            )
            .expect("apply_batch with lint: false");
        assert!(batch_reply.get("issues").is_none());
        assert!(batch_reply.get("issues_total").is_none());
        assert!(batch_reply.get("issues_truncated").is_none());
    }

    #[test]
    fn errors_only_filters() {
        let mut engine = Engine::default();
        // Configure preset="strict" so non-4px-grid (warning) and collapsed-size (error) both run.
        engine
            .call(
                "designos_configure_session",
                json!({
                    "issues": {
                        "mode": "preset",
                        "preset": "strict"
                    }
                }),
            )
            .expect("configure strict");

        let spec = json!({
            "parent_id": 1,
            "spec": {
                "name": "MixedContainer",
                "kind": "frame",
                "width": 200.0,
                "height": 200.0,
                "children": [
                    {
                        "name": "OffGridWarn",
                        "kind": "rectangle",
                        "x": 3.0,
                        "y": 3.0,
                        "width": 40.0,
                        "height": 40.0,
                        "fill": "#336699"
                    },
                    {
                        "name": "CollapsedErr",
                        "kind": "rectangle",
                        "x": 60.0,
                        "y": 0.0,
                        "width": 0.0,
                        "height": 40.0,
                        "fill": "#FF0000"
                    }
                ]
            }
        });

        let preset_reply = engine.call("designos_apply_spec", spec.clone()).expect("preset apply_spec");
        let preset_issues = preset_reply["issues"].as_array().unwrap();
        assert!(preset_issues.iter().any(|i| i["severity"] == "warning"), "expected warning in preset mode: {preset_issues:?}");
        assert!(preset_issues.iter().any(|i| i["severity"] == "error"), "expected error in preset mode: {preset_issues:?}");

        let mut err_engine = Engine::default();
        err_engine
            .call(
                "designos_configure_session",
                json!({
                    "issues": {
                        "mode": "errors_only",
                        "preset": "strict"
                    }
                }),
            )
            .expect("configure errors_only");
        let err_reply = err_engine.call("designos_apply_spec", spec).expect("errors_only apply_spec");
        let err_issues = err_reply["issues"].as_array().unwrap();
        assert!(!err_issues.is_empty());
        assert!(err_issues.iter().all(|i| i["severity"] == "error"), "expected only errors: {err_issues:?}");
        assert!(err_issues.iter().any(|i| i["rule"] == "collapsed-size" && i["node_name"] == "CollapsedErr"));
    }

    #[test]
    fn budget_truncates_and_counts() {
        let mut engine = Engine::default();
        let mut children = Vec::with_capacity(60);
        for i in 0..60 {
            children.push(json!({
                "name": format!("ZeroRect{i}"),
                "kind": "rectangle",
                "x": (i as f32) * 8.0,
                "y": 0.0,
                "width": 0.0,
                "height": 20.0,
                "fill": "#FF0000"
            }));
        }
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "SixtyErrorsFrame",
                        "kind": "frame",
                        "width": 600.0,
                        "height": 100.0,
                        "children": children
                    }
                }),
            )
            .expect("apply_spec with 60 collapsed rects");

        let issues = reply["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 20);
        assert_eq!(reply["issues_total"], 60);
        assert_eq!(reply["issues_truncated"], true);
    }

    #[test]
    fn issues_capped_at_ten_with_total_count() {
        let mut engine = Engine::default();
        engine.call("designos_configure_session", json!({ "issues": { "max_items": 10 } })).expect("configure max_items=10");

        let mut children = Vec::with_capacity(25);
        for i in 0..25 {
            children.push(json!({
                "name": format!("ZeroRect{i}"),
                "kind": "rectangle",
                "x": (i as f32) * 8.0,
                "y": 0.0,
                "width": 0.0,
                "height": 20.0,
                "fill": "#FF0000"
            }));
        }
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "TwentyFiveErrorsFrame",
                        "kind": "frame",
                        "width": 400.0,
                        "height": 100.0,
                        "children": children
                    }
                }),
            )
            .expect("apply_spec");

        let issues = reply["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 10);
        assert_eq!(reply["issues_total"], 25);
        assert_eq!(reply["issues_truncated"], true);
    }

    #[test]
    fn options_survive_reconnect() {
        let tmp = std::env::temp_dir().join(format!(
            "designos-mcp-issues-reconnect-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let ws = tmp.join("ws");
        let sess_dir = tmp.join("session");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&sess_dir).unwrap();

        {
            let mut s1 = Session::with_session_dir(&ws, &sess_dir).expect("session 1");
            let configured = s1
                .call(
                    "designos_configure_session",
                    json!({
                        "issues": {
                            "mode": "errors_only",
                            "preset": "strict",
                            "max_items": 7
                        }
                    }),
                )
                .expect("configure");
            assert_eq!(configured["issues"]["mode"], "errors_only");
            assert_eq!(configured["issues"]["preset"], "strict");
            assert_eq!(configured["issues"]["max_items"], 7);
        }

        // Reconnect on the same session_dir: options must survive.
        let mut s2 = Session::with_session_dir(&ws, &sess_dir).expect("session 2");
        assert_eq!(s2.engine.session_options.issues.mode, crate::tools::session_options::IssuesMode::ErrorsOnly);
        assert_eq!(s2.engine.session_options.issues.preset, "strict");
        assert_eq!(s2.engine.session_options.issues.max_items, 7);

        // Document switch resets session options to default and persists the reset.
        s2.call("designos_new_document", json!({ "name": "FreshDoc", "discard_changes": true })).expect("new_document");
        assert_eq!(s2.engine.session_options.issues.mode, crate::tools::session_options::IssuesMode::Preset);
        assert_eq!(s2.engine.session_options.issues.preset, "agent");
        assert_eq!(s2.engine.session_options.issues.max_items, 20);

        let s3 = Session::with_session_dir(&ws, &sess_dir).expect("session 3");
        assert_eq!(s3.engine.session_options.issues.mode, crate::tools::session_options::IssuesMode::Preset);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sibling_overlap_caught_in_parent_frame() {
        let mut engine = Engine::default();
        let frame_id = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": 1,
                    "kind": "frame",
                    "name": "CanvasCard",
                    "width": 300.0,
                    "height": 200.0
                }),
            )
            .expect("create frame")["node"]["id"]
            .as_u64()
            .unwrap();

        let _first = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": frame_id,
                    "kind": "rectangle",
                    "name": "BoxA",
                    "x": 16.0,
                    "y": 16.0,
                    "width": 100.0,
                    "height": 80.0
                }),
            )
            .expect("create BoxA");

        // Creating BoxB overlapping BoxA inside CanvasCard only touches BoxB directly,
        // so the parent-container scope expansion to CanvasCard is what catches `sibling-overlap`.
        let second = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": frame_id,
                    "kind": "rectangle",
                    "name": "BoxB",
                    "x": 20.0,
                    "y": 20.0,
                    "width": 100.0,
                    "height": 80.0
                }),
            )
            .expect("create BoxB");

        let issues = second["issues"].as_array().expect("issues");
        assert!(issues.iter().any(|i| i["rule"] == "sibling-overlap"), "expected sibling-overlap in parent container scope, got {issues:?}");
    }

    #[test]
    fn budget_caps_large_subtree() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let root_frame = engine.doc.alloc_id();
        engine.doc.insert(page, Node::frame(root_frame, Rect::new(0.0, 0.0, 1000.0, 1000.0))).unwrap();

        // Populate 50 row frames x 45 cells = 2,300 descendants so `root_frame`'s subtree
        // exceeds the 2,000-node per-call budget while keeping per-container sibling count small.
        for row_idx in 0..50 {
            let row_id = engine.doc.alloc_id();
            let mut row = Node::frame(row_id, Rect::new(0.0, row_idx as f32 * 16.0, 1000.0, 14.0));
            row.name = format!("Row{row_idx}");
            engine.doc.insert(root_frame, row).unwrap();
            for col_idx in 0..45 {
                let cid = engine.doc.alloc_id();
                let mut r = Node::rectangle(cid, Rect::new(col_idx as f32 * 16.0, 0.0, 12.0, 12.0));
                r.name = format!("Cell{row_idx}_{col_idx}");
                engine.doc.insert(row_id, r).unwrap();
            }
        }

        let first_row = engine.doc.get(root_frame).unwrap().children[0];
        let scope = super::compute_lint_scope(&engine.doc, &[first_row]);
        assert_eq!(scope.roots, vec![root_frame]);
        assert_eq!(scope.shallow_nodes.len(), 49);

        // Updating `root_frame` directly causes `compute_lint_scope` to lint `root_frame`'s 2,301-node
        // subtree, hitting the per-call budget and reporting `issues_partial: true`.
        let reply = engine
            .call(
                "designos_update_node",
                json!({
                    "node_id": root_frame.0,
                    "name": "HugeGrid"
                }),
            )
            .expect("update HugeGrid");

        assert_eq!(reply["issues_partial"], true);
        assert_eq!(reply["issues_truncated"], true);
        let scanned = reply["issues_scanned"].as_u64().expect("issues_scanned");
        assert!((1..=2_000).contains(&scanned), "expected scanned in 1..=2000, got {scanned}");
    }

    #[test]
    fn edge_cases_delete_collapsed_ignored_preexisting_and_skipped_text_rules() {
        let mut engine = Engine::default();

        // 1. Delete the only child of a hugging auto-layout frame -> parent frame collapses to 0x0
        // and `collapsed-size` is reported on the parent frame (§8.1).
        let applied = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "HugBadge",
                        "kind": "frame",
                        "layout": {
                            "direction": "horizontal",
                            "padding": [0.0, 0.0, 0.0, 0.0]
                        },
                        "width_sizing": "hug",
                        "height_sizing": "hug",
                        "children": [
                            {
                                "name": "OnlyChild",
                                "kind": "rectangle",
                                "width": 40.0,
                                "height": 20.0,
                                "fill": "#00AA55"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec HugBadge");
        let badge_id = applied["root_node_id"].as_u64().unwrap();
        let child_id = applied["node"]["children"][0].as_u64().unwrap();

        let del_reply = engine.call("designos_delete_node", json!({ "node_id": child_id })).expect("delete only child");
        let del_issues = del_reply["issues"].as_array().unwrap();
        assert!(
            del_issues.iter().any(|i| i["rule"] == "collapsed-size" && i["node_id"] == badge_id),
            "expected collapsed-size on HugBadge after deleting its only child, got {del_issues:?}"
        );

        // 2. Pre-existing finding in a <= 500-node scope gets `preexisting: true` while a newly
        // created finding in the same container omits `preexisting` (§8.3).
        let card_id = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": 1,
                    "kind": "frame",
                    "name": "PreexistingCard",
                    "width": 200.0,
                    "height": 200.0
                }),
            )
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();
        let pre_rect_id = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": card_id,
                    "kind": "rectangle",
                    "name": "AlreadyCollapsed",
                    "x": 0.0,
                    "y": 0.0,
                    "width": 0.0,
                    "height": 20.0
                }),
            )
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();

        let second_reply = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": card_id,
                    "kind": "rectangle",
                    "name": "NewlyCollapsed",
                    "x": 40.0,
                    "y": 0.0,
                    "width": 0.0,
                    "height": 20.0
                }),
            )
            .unwrap();
        let new_rect_id = second_reply["node"]["id"].as_u64().unwrap();
        let issues = second_reply["issues"].as_array().unwrap();
        let pre_item = issues.iter().find(|i| i["node_id"] == pre_rect_id).expect("preexisting item");
        let new_item = issues.iter().find(|i| i["node_id"] == new_rect_id).expect("new item");
        assert_eq!(pre_item["preexisting"], true);
        assert!(new_item.get("preexisting").is_none());

        // 3. Ignored findings on a node are excluded from `issues` and `issues_total` (§8.2).
        engine.doc.check_designs.ignored.insert(IgnoreKey::new("collapsed-size", NodeId(pre_rect_id)));
        let after_ignore = engine
            .call(
                "designos_update_node",
                json!({
                    "node_id": new_rect_id,
                    "name": "NewlyCollapsedRenamed"
                }),
            )
            .unwrap();
        let after_issues = after_ignore["issues"].as_array().unwrap();
        assert!(after_issues.iter().all(|i| i["node_id"] != pre_rect_id), "ignored finding must be excluded: {after_issues:?}");

        // 4. Text engine unavailable (`disable_text_lint_engine = true`) reports `issues_skipped_rules` (§8.5).
        engine.disable_text_lint_engine = true;
        let no_text_reply = engine
            .call(
                "designos_update_node",
                json!({
                    "node_id": new_rect_id,
                    "name": "NoTextEngineCheck"
                }),
            )
            .unwrap();
        let skipped = no_text_reply["issues_skipped_rules"].as_array().expect("issues_skipped_rules");
        assert!(
            skipped.iter().any(|s| s["rule"] == "text-truncated" && s["reason"] == "no_text_engine"),
            "expected text-truncated in issues_skipped_rules: {skipped:?}"
        );
    }
}
