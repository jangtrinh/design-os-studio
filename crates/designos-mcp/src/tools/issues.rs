//! Per-call lint `issues` block attached to every design-mutating MCP reply (`E15-12` / `#597`).

use crate::tools::session_options::{IssuesMode, IssuesOptions, PerCallLintOverride};
use designos_doc::lint::{lint_subtree, Budget, Finding, LintConfig, LintEnv, Severity, RULES};
use designos_doc::{Document, NodeId, NodeKind, SpecIssue};
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
    /// Set when `capture_container` could not baseline a `> 500`-node parent container: only nodes
    /// inside the newly inserted `touched_roots` subtree are newly introduced by the call.
    only_new_subtrees: bool,
}

impl PreCallBaseline {
    pub fn none() -> Self {
        Self { findings: None, only_new_subtrees: false }
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
            return Self { findings: Some(BTreeSet::new()), only_new_subtrees: false };
        }
        let estimated = count_subtree_nodes_bounded(doc, parent_container, MAX_PREEXISTING_SCOPE_NODES);
        let mut baseline = Self::capture_scope(doc, vec![parent_container], BTreeSet::new(), estimated, options, env);
        if baseline.findings.is_none() {
            baseline.only_new_subtrees = true;
        }
        baseline
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
        if post_scope.roots.is_empty() {
            return Self::none();
        }
        let pre_roots: Vec<NodeId> = post_scope.roots.into_iter().filter(|&r| pre_doc.get(r).is_some()).collect();
        if pre_roots.is_empty() {
            return Self { findings: Some(BTreeSet::new()), only_new_subtrees: false };
        }
        if post_scope.estimated_nodes > MAX_PREEXISTING_SCOPE_NODES {
            return Self::none();
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
        Self { findings: Some(set), only_new_subtrees: false }
    }

    fn contains(&self, rule: &'static str, node: NodeId) -> Option<bool> {
        self.findings.as_ref().and_then(|set| set.contains(&(rule, node)).then_some(true))
    }

    fn is_ineligible_for_safe_auto_fix(&self, doc: &Document, touched_roots: &[NodeId], rule: &'static str, node: NodeId) -> bool {
        match &self.findings {
            Some(set) => set.contains(&(rule, node)),
            None if self.only_new_subtrees => !touched_roots.iter().any(|&r| r == node || doc.is_ancestor(r, node)),
            None => true,
        }
    }
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

fn spec_severity_rank(s: &str) -> u8 {
    match s {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

fn rule_order_index(rule_id: &str) -> usize {
    RULES.iter().position(|r| r.id == rule_id).unwrap_or(RULES.len())
}

fn is_safe_tier_auto_fixable(f: &Finding) -> bool {
    f.safe && matches!(f.rule, "sizing-conflict" | "hardcoded-value")
}

fn compact_fix_for_finding(doc: &Document, f: &Finding) -> Option<CompactFix> {
    if let Some(s) = &f.suggested_fix {
        return Some(CompactFix { kind: s.kind.clone(), property: s.property.clone(), to: s.to.clone() });
    }
    match f.rule {
        "text-missing-fill" => {
            Some(CompactFix { kind: "set".into(), property: "fills".into(), to: json!(designos_doc::lint::contrast_aware_text_fill_hex(doc, f.node)) })
        }
        "absolute-in-auto-layout" => Some(CompactFix { kind: "set".into(), property: "layout_child.absolute".into(), to: json!(false) }),
        _ => None,
    }
}

/// Runs the scoped lint pass on `doc` for `touched_roots` and returns the raw findings (`E15-13` §3.3).
pub fn lint_findings_for_scope(doc: &Document, touched_roots: &[NodeId], options: &IssuesOptions, env: &dyn LintEnv) -> Vec<Finding> {
    let scope = compute_lint_scope(doc, touched_roots);
    if scope.roots.is_empty() {
        return Vec::new();
    }
    let cfg = LintConfig::from_preset_str(&options.preset).unwrap_or_default();
    let mut budget = Budget::with_limits(PER_CALL_MAX_NODES, Some(PER_CALL_MAX_DURATION));
    budget.shallow_nodes = scope.shallow_nodes;
    let mut findings = lint_subtree(doc, &scope.roots, &cfg, env, &mut budget).findings;
    findings.sort_by_key(|f| (severity_rank(f.severity), rule_order_index(f.rule)));
    findings
}

/// Evaluates the per-call lint `issues` block on `doc` for `touched_roots` and inserts the
/// `issues*` fields into `reply` (`E15-12` §3.1–§3.5, §4, §5, §8).
///
/// `extra` carries findings produced outside the lint pass (`apply_spec` slot issues, `E15-2`); they
/// follow the same mode filter, severity sort, `max_items` cap and `issues_total`/`issues_truncated`
/// accounting as lint findings. Pass `&[]` when there are none.
///
/// When effective mode is `Off` (via session options or per-call `lint: false`), `reply` is left
/// untouched with no `issues*` keys.
#[allow(clippy::too_many_arguments)]
pub fn attach_issues_block(
    doc: &Document,
    touched_roots: &[NodeId],
    options: &IssuesOptions,
    per_call_override: Option<PerCallLintOverride>,
    baseline: &PreCallBaseline,
    env: &dyn LintEnv,
    extra: &[SpecIssue],
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

    let mut extras: Vec<&SpecIssue> = extra
        .iter()
        .filter(|i| match mode {
            IssuesMode::ErrorsOnly => i.severity == "error",
            IssuesMode::Preset | IssuesMode::Off => true,
        })
        .collect();
    extras.sort_by_key(|i| spec_severity_rank(&i.severity));

    let issues_total = findings.len() + extras.len();
    let issues_truncated = issues_total > max_items || !report.complete;

    // Merge the two severity-sorted lists; lint findings win ties.
    let mut items: Vec<Value> = Vec::with_capacity(max_items.min(issues_total));
    let mut lint_iter = findings.into_iter().peekable();
    let mut extra_iter = extras.into_iter().peekable();
    while items.len() < max_items {
        let take_lint = match (lint_iter.peek(), extra_iter.peek()) {
            (Some(f), Some(x)) => severity_rank(f.severity) <= spec_severity_rank(&x.severity),
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        if take_lint {
            let Some(f) = lint_iter.next() else { break };
            let preexisting = baseline.contains(f.rule, f.node);
            let auto_fixable = is_safe_tier_auto_fixable(&f) && !baseline.is_ineligible_for_safe_auto_fix(doc, touched_roots, f.rule, f.node);
            let fix = compact_fix_for_finding(doc, &f);
            let item = IssueItem {
                rule: f.rule.to_string(),
                severity: f.severity,
                node_id: f.node.0,
                node_name: f.node_name,
                path: layer_name_path(doc, f.node),
                message: f.message,
                fix,
                auto_fixable,
                preexisting,
            };
            items.push(serde_json::to_value(item).unwrap_or(Value::Null));
        } else if let Some(x) = extra_iter.next() {
            items.push(serde_json::to_value(x).unwrap_or(Value::Null));
        }
    }

    if let Some(obj) = reply.as_object_mut() {
        obj.insert("issues".into(), Value::Array(items));
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

/// Runs `E15-13` auto-fix planning in `report_only` mode against `trial_doc` (for dry runs or
/// no-op calls) and attaches both `auto_fix` and `issues*` blocks to `reply`.
pub fn finish_dry_run_call(
    engine: &crate::engine::Engine,
    trial_doc: &Document,
    touched_roots: &[NodeId],
    per_call_lint_override: Option<PerCallLintOverride>,
    baseline: &PreCallBaseline,
    extra: &[SpecIssue],
    reply: &mut Value,
) {
    if matches!(per_call_lint_override, Some(PerCallLintOverride::Bool(false)) | Some(PerCallLintOverride::Mode(IssuesMode::Off))) {
        attach_issues_block(trial_doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
        return;
    }

    let resolved_af = engine.resolved_auto_fix_config(trial_doc);
    if resolved_af.mode == designos_doc::AutoFixMode::Off {
        attach_issues_block(trial_doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
        return;
    }

    let mut findings = lint_findings_for_scope(trial_doc, touched_roots, &engine.session_options.issues, engine.lint_env());
    for f in &mut findings {
        if baseline.is_ineligible_for_safe_auto_fix(trial_doc, touched_roots, f.rule, f.node) {
            f.safe = false;
        }
    }
    if let Some(hook) = &engine.test_lint_hook {
        findings.extend(hook(trial_doc, touched_roots, 0));
    }
    let owned = engine.resolved_owned_frames();
    let (planned, suggested) = designos_doc::lint::plan_fixes(trial_doc, &findings, &resolved_af, owned.as_deref());
    let report = designos_doc::lint::AutoFixReport {
        mode: resolved_af.mode,
        undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
        passes: usize::from(!planned.is_empty()),
        applied: Vec::new(),
        would_apply: planned.iter().map(designos_doc::lint::AutoFixDiffEntry::from).collect(),
        suggested,
        error: None,
    };

    attach_issues_block(trial_doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
    if let Some(obj) = reply.as_object_mut() {
        if let Ok(v) = serde_json::to_value(report) {
            obj.insert("auto_fix".into(), v);
        }
    }
}

/// Runs `E15-13` safe-tier auto-fix planning and execution (up to 2 passes into a single
/// `"Auto-fix"` undo entry) after a mutating call commits, then attaches `auto_fix` and
/// post-fix `issues*` blocks to `reply`.
pub fn finish_mutating_call(
    engine: &mut crate::engine::Engine,
    touched_roots: &[NodeId],
    per_call_lint_override: Option<PerCallLintOverride>,
    baseline: &PreCallBaseline,
    refresh_node_id: Option<NodeId>,
    extra: &[SpecIssue],
    reply: &mut Value,
) {
    if matches!(per_call_lint_override, Some(PerCallLintOverride::Bool(false)) | Some(PerCallLintOverride::Mode(IssuesMode::Off))) {
        attach_issues_block(&engine.doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
        return;
    }

    let resolved_af = engine.resolved_auto_fix_config(&engine.doc);
    match resolved_af.mode {
        designos_doc::AutoFixMode::Off => {
            attach_issues_block(&engine.doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
        }
        designos_doc::AutoFixMode::ReportOnly => {
            finish_dry_run_call(engine, &engine.doc, touched_roots, per_call_lint_override, baseline, extra, reply);
        }
        designos_doc::AutoFixMode::On => {
            let mut findings_p1 = lint_findings_for_scope(&engine.doc, touched_roots, &engine.session_options.issues, engine.lint_env());
            for f in &mut findings_p1 {
                if baseline.is_ineligible_for_safe_auto_fix(&engine.doc, touched_roots, f.rule, f.node) {
                    f.safe = false;
                }
            }
            if let Some(hook) = &engine.test_lint_hook {
                findings_p1.extend(hook(&engine.doc, touched_roots, 0));
            }
            let owned = engine.resolved_owned_frames();
            let (planned_p1, suggested_p1) = designos_doc::lint::plan_fixes(&engine.doc, &findings_p1, &resolved_af, owned.as_deref());

            if planned_p1.is_empty() {
                let report = designos_doc::lint::AutoFixReport {
                    mode: designos_doc::AutoFixMode::On,
                    undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
                    passes: 0,
                    applied: Vec::new(),
                    would_apply: Vec::new(),
                    suggested: suggested_p1,
                    error: None,
                };
                attach_issues_block(
                    &engine.doc,
                    touched_roots,
                    &engine.session_options.issues,
                    per_call_lint_override,
                    baseline,
                    engine.lint_env(),
                    extra,
                    reply,
                );
                if let Some(obj) = reply.as_object_mut() {
                    if let Ok(v) = serde_json::to_value(report) {
                        obj.insert("auto_fix".into(), v);
                    }
                }
                return;
            }

            let pre_doc = engine.doc.clone();
            let pre_undo_depth = engine.history.undo_depth();
            let pre_revision = engine.revision;
            let raw_reply_template = reply.clone();

            let cmds_p1: Vec<designos_doc::Command> = planned_p1.iter().flat_map(|p| p.commands.clone()).collect();
            let batch_p1 = designos_doc::Command::Batch(cmds_p1);
            let apply_p1 = engine
                .live_ops_for(&batch_p1)
                .map_err(|e| e.message)
                .and_then(|ops| engine.history.exec(&mut engine.doc, batch_p1).map(|()| ops).map_err(|e| e.to_string()));

            let ops_p1 = match apply_p1 {
                Ok(ops) => ops,
                Err(err_msg) => {
                    engine.doc = pre_doc;
                    engine.history.drop_undo_steps_after(pre_undo_depth);
                    engine.revision = pre_revision;
                    engine.auto_fix_live_ops.clear();
                    let report = designos_doc::lint::AutoFixReport {
                        mode: designos_doc::AutoFixMode::On,
                        undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
                        passes: 0,
                        applied: Vec::new(),
                        would_apply: Vec::new(),
                        suggested: suggested_p1,
                        error: Some(err_msg),
                    };
                    attach_issues_block(
                        &engine.doc,
                        touched_roots,
                        &engine.session_options.issues,
                        per_call_lint_override,
                        baseline,
                        engine.lint_env(),
                        extra,
                        reply,
                    );
                    if let Some(obj) = reply.as_object_mut() {
                        if let Ok(v) = serde_json::to_value(report) {
                            obj.insert("auto_fix".into(), v);
                        }
                    }
                    return;
                }
            };

            engine.revision = engine.revision.wrapping_add(1);
            engine.auto_fix_live_ops.extend(ops_p1);
            for p in &planned_p1 {
                crate::tools::batch::relayout_for_node(&mut engine.doc, p.node);
            }
            for &r in touched_roots {
                crate::tools::batch::relayout_for_node(&mut engine.doc, r);
            }
            engine.settle_layout();

            let mut applied: Vec<designos_doc::lint::AutoFixDiffEntry> = planned_p1.iter().map(designos_doc::lint::AutoFixDiffEntry::from).collect();
            let mut passes = 1usize;

            // Re-lint the scope once; if new safe findings appear, run one more pass (max 2 passes)
            // folded into the same `"Auto-fix"` undo entry (`E15-13` §3.3).
            let mut findings_p2 = lint_findings_for_scope(&engine.doc, touched_roots, &engine.session_options.issues, engine.lint_env());
            for f in &mut findings_p2 {
                if baseline.is_ineligible_for_safe_auto_fix(&engine.doc, touched_roots, f.rule, f.node) {
                    f.safe = false;
                }
            }
            if let Some(hook) = &engine.test_lint_hook {
                findings_p2.extend(hook(&engine.doc, touched_roots, 1));
            }
            let (planned_p2, mut final_suggested) = designos_doc::lint::plan_fixes(&engine.doc, &findings_p2, &resolved_af, owned.as_deref());

            if !planned_p2.is_empty() && passes < designos_doc::lint::MAX_AUTO_FIX_PASSES {
                let cmds_p2: Vec<designos_doc::Command> = planned_p2.iter().flat_map(|p| p.commands.clone()).collect();
                let batch_p2 = designos_doc::Command::Batch(cmds_p2);
                let apply_p2 = engine
                    .live_ops_for(&batch_p2)
                    .map_err(|e| e.message)
                    .and_then(|ops| engine.history.fold_into_last(&mut engine.doc, batch_p2).map(|()| ops).map_err(|e| e.to_string()));

                match apply_p2 {
                    Err(err_msg) => {
                        engine.doc = pre_doc;
                        engine.history.drop_undo_steps_after(pre_undo_depth);
                        engine.revision = pre_revision;
                        engine.auto_fix_live_ops.clear();
                        let report = designos_doc::lint::AutoFixReport {
                            mode: designos_doc::AutoFixMode::On,
                            undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
                            passes: 0,
                            applied: Vec::new(),
                            would_apply: Vec::new(),
                            suggested: suggested_p1,
                            error: Some(err_msg),
                        };
                        attach_issues_block(
                            &engine.doc,
                            touched_roots,
                            &engine.session_options.issues,
                            per_call_lint_override,
                            baseline,
                            engine.lint_env(),
                            extra,
                            reply,
                        );
                        if let Some(obj) = reply.as_object_mut() {
                            if let Ok(v) = serde_json::to_value(report) {
                                obj.insert("auto_fix".into(), v);
                            }
                        }
                        return;
                    }
                    Ok(ops_p2) => {
                        engine.auto_fix_live_ops.extend(ops_p2);
                        for p in &planned_p2 {
                            crate::tools::batch::relayout_for_node(&mut engine.doc, p.node);
                        }
                        for &r in touched_roots {
                            crate::tools::batch::relayout_for_node(&mut engine.doc, r);
                        }
                        engine.settle_layout();
                        applied.extend(planned_p2.iter().map(designos_doc::lint::AutoFixDiffEntry::from));
                        passes = 2;
                        let mut findings_post = lint_findings_for_scope(&engine.doc, touched_roots, &engine.session_options.issues, engine.lint_env());
                        for f in &mut findings_post {
                            if baseline.is_ineligible_for_safe_auto_fix(&engine.doc, touched_roots, f.rule, f.node) {
                                f.safe = false;
                            }
                        }
                        let (_, sugg_post) = designos_doc::lint::plan_fixes(&engine.doc, &findings_post, &resolved_af, owned.as_deref());
                        final_suggested = sugg_post;
                    }
                }
            }

            if let Some(nid) = refresh_node_id {
                if let Some(obj) = reply.as_object_mut() {
                    if obj.contains_key("node") {
                        obj.insert("node".into(), json!(engine.doc.get(nid)));
                    }
                }
            }

            let report = designos_doc::lint::AutoFixReport {
                mode: designos_doc::AutoFixMode::On,
                undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
                passes,
                applied,
                would_apply: Vec::new(),
                suggested: final_suggested,
                error: None,
            };
            attach_issues_block(&engine.doc, touched_roots, &engine.session_options.issues, per_call_lint_override, baseline, engine.lint_env(), extra, reply);
            if let Some(obj) = reply.as_object_mut() {
                if let Ok(v) = serde_json::to_value(&report) {
                    obj.insert("auto_fix".into(), v);
                }
            }

            let mut pre_reply = raw_reply_template;
            let pre_report = designos_doc::lint::AutoFixReport {
                mode: designos_doc::AutoFixMode::On,
                undo_label: designos_doc::lint::AUTO_FIX_UNDO_LABEL.to_string(),
                passes: 0,
                applied: Vec::new(),
                would_apply: Vec::new(),
                suggested: suggested_p1,
                error: None,
            };
            attach_issues_block(
                &pre_doc,
                touched_roots,
                &engine.session_options.issues,
                per_call_lint_override,
                baseline,
                engine.lint_env(),
                extra,
                &mut pre_reply,
            );
            if let Some(obj) = pre_reply.as_object_mut() {
                if let Ok(v) = serde_json::to_value(pre_report) {
                    obj.insert("auto_fix".into(), v);
                }
            }
            let post_undo_depth = engine.history.undo_depth();
            engine.last_auto_fix_checkpoint = Some(crate::engine::AutoFixCheckpoint { pre_doc, pre_undo_depth, post_undo_depth, pre_revision, pre_reply });
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

    fn slot_engine() -> (Engine, u64) {
        let mut e =
            Engine { doc: designos_doc::fixtures::card_with_slots(), history: Default::default(), active_page_id: None, live_ops: None, ..Engine::default() };
        let page = e.doc.pages[0];
        e.active_page_id = Some(page);
        (e, page.0)
    }

    fn slot_fill_args(page: u64, extra: Value) -> Value {
        let frames: Vec<Value> = (0..4).map(|i| json!({ "kind": "frame", "name": format!("Adhoc{i}"), "height": 32 })).collect();
        let mut args = json!({
            "parent_id": page,
            "spec": { "kind": "instance", "name": "Card1", "component": "Card", "slots": { "body": frames } }
        });
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        args
    }

    #[test]
    fn slot_issues_absent_when_mode_off_or_lint_false() {
        let (mut e, page) = slot_engine();
        e.call("designos_configure_session", json!({ "issues": { "mode": "off" } })).unwrap();
        let reply = e.call("designos_apply_spec", slot_fill_args(page, json!({}))).unwrap();
        for k in ["issues", "issues_total", "issues_truncated", "issues_partial", "issues_scanned", "issues_skipped_rules"] {
            assert!(reply.get(k).is_none(), "{k} must be absent in off mode: {reply}");
        }

        let (mut e, page) = slot_engine();
        let reply = e.call("designos_apply_spec", slot_fill_args(page, json!({ "lint": false }))).unwrap();
        assert!(reply.get("issues").is_none() && reply.get("issues_total").is_none(), "lint:false must omit issues: {reply}");
    }

    #[test]
    fn slot_issues_warnings_dropped_in_errors_only_mode() {
        let (mut e, page) = slot_engine();
        e.call("designos_configure_session", json!({ "issues": { "mode": "errors_only" } })).unwrap();
        let reply = e.call("designos_apply_spec", slot_fill_args(page, json!({}))).unwrap();
        let items = reply["issues"].as_array().expect("issues array");
        assert!(items.iter().all(|i| i["severity"] == "error"), "warnings leaked in errors_only: {items:?}");
        assert_eq!(reply["issues_total"], items.len());
    }

    #[test]
    fn slot_issues_capped_with_total_and_truncated() {
        let (mut e, page) = slot_engine();
        e.call("designos_configure_session", json!({ "issues": { "max_items": 2 } })).unwrap();
        let reply = e.call("designos_apply_spec", slot_fill_args(page, json!({}))).unwrap();
        let items = reply["issues"].as_array().expect("issues array");
        assert_eq!(items.len(), 2, "{reply}");
        assert_eq!(reply["issues_total"], 4);
        assert_eq!(reply["issues_truncated"], true);
        assert!(items.iter().all(|i| i["rule"] == "slot-non-recommended" && i["severity"] == "warning"));
        // Recommended components stay visible on the capped slot items.
        assert!(items[0]["recommended"].is_array());
    }

    // ------------------------------------------------------------------
    // E15-13 (#598): Engine auto-fix tests (§10 + §8 edge cases)
    // ------------------------------------------------------------------

    #[test]
    fn fill_child_without_layout_parent_fixed() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "StaticCard",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "FillChild",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec");

        let applied = reply["auto_fix"]["applied"].as_array().expect("auto_fix.applied");
        assert_eq!(applied.len(), 1, "expected 1 applied fix: {reply}");
        assert_eq!(applied[0]["rule"], "sizing-conflict");
        assert_eq!(applied[0]["property"], "layoutSizingHorizontal");
        assert_eq!(applied[0]["before"], "FILL");
        assert_eq!(applied[0]["after"], "FIXED");
        assert_eq!(reply["auto_fix"]["passes"], 1);
        assert_eq!(reply["auto_fix"]["undo_label"], "Auto-fix");
        assert_eq!(reply["issues"], json!([]));

        let child_id = applied[0]["node_id"].as_u64().unwrap();
        assert_eq!(engine.doc.get(NodeId(child_id)).unwrap().layout_child.horizontal, designos_doc::Sizing::Fixed);
    }

    #[test]
    fn report_only_lists_and_changes_nothing() {
        let mut engine = Engine::default();
        engine
            .call(
                "designos_configure_session",
                json!({
                    "auto_fix": { "mode": "report_only" }
                }),
            )
            .expect("configure report_only");

        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "StaticCard",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "FillChild",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec in report_only");

        assert_eq!(reply["auto_fix"]["mode"], "report_only");
        assert_eq!(reply["auto_fix"]["passes"], 1);
        assert_eq!(reply["auto_fix"]["applied"], json!([]));
        let would_apply = reply["auto_fix"]["would_apply"].as_array().expect("would_apply");
        assert_eq!(would_apply.len(), 1);
        assert_eq!(would_apply[0]["rule"], "sizing-conflict");
        assert_eq!(would_apply[0]["property"], "layoutSizingHorizontal");
        assert_eq!(would_apply[0]["before"], "FILL");
        assert_eq!(would_apply[0]["after"], "FIXED");

        let child_id = would_apply[0]["node_id"].as_u64().unwrap();
        assert_eq!(engine.doc.get(NodeId(child_id)).unwrap().layout_child.horizontal, designos_doc::Sizing::Fill, "report_only must not mutate the document");
        let issues = reply["issues"].as_array().unwrap();
        assert!(issues.iter().any(|i| i["rule"] == "sizing-conflict" && i["node_id"] == child_id));
    }

    #[test]
    fn undo_once_removes_only_fixes() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "StaticCard",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "DualFillChild",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "height_sizing": "fill",
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec with 2 auto-fixes");

        let card_id = reply["root_node_id"].as_u64().unwrap();
        let applied = reply["auto_fix"]["applied"].as_array().unwrap();
        assert_eq!(applied.len(), 2, "expected 2 applied fixes (horizontal + vertical): {applied:?}");
        let child_id = applied[0]["node_id"].as_u64().unwrap();
        assert_eq!(engine.doc.get(NodeId(child_id)).unwrap().layout_child.horizontal, designos_doc::Sizing::Fixed);
        assert_eq!(engine.doc.get(NodeId(child_id)).unwrap().layout_child.vertical, designos_doc::Sizing::Fixed);

        // First undo reverts ONLY the `"Auto-fix"` entry (both fixes at once), leaving the write intact.
        engine.call("designos_undo", json!({})).expect("first undo");
        let child = engine.doc.get(NodeId(child_id)).expect("child must still exist after undoing only Auto-fix");
        assert_eq!(child.layout_child.horizontal, designos_doc::Sizing::Fill);
        assert_eq!(child.layout_child.vertical, designos_doc::Sizing::Fill);
        assert!(engine.doc.get(NodeId(card_id)).is_some());

        // Second undo reverts the original write itself.
        engine.call("designos_undo", json!({})).expect("second undo");
        assert!(engine.doc.get(NodeId(card_id)).is_none());
        assert!(engine.doc.get(NodeId(child_id)).is_none());
    }

    #[test]
    fn contrast_never_auto_applied() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "LowContrastCard",
                        "kind": "frame",
                        "width": 240.0,
                        "height": 80.0,
                        "fill": "#888888",
                        "children": [
                            {
                                "name": "LowContrastLabel",
                                "kind": "text",
                                "text": "Hard to read",
                                "font_size": 14.0,
                                "text_color": "#777777",
                                "width": 160.0,
                                "height": 24.0
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec low contrast");

        let applied = reply["auto_fix"]["applied"].as_array().unwrap();
        assert!(applied.is_empty(), "contrast must never be auto-applied: {applied:?}");
        let suggested = reply["auto_fix"]["suggested"].as_array().unwrap();
        let c_sugg = suggested.iter().find(|s| s["rule"] == "contrast").expect("contrast in auto_fix.suggested");
        assert!(c_sugg["fix"].is_null());
        assert!(c_sugg["reason"].as_str().unwrap_or("").contains("contrast"));

        let issues = reply["issues"].as_array().unwrap();
        assert!(issues.iter().any(|i| i["rule"] == "contrast"));
    }

    #[test]
    fn text_missing_fill_is_suggestion_only() {
        let mut engine = Engine::default();
        // 1. Light background (#FFFFFF) -> suggests "#111111"
        let light_reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "LightFrame",
                        "kind": "frame",
                        "width": 240.0,
                        "height": 80.0,
                        "fill": "#FFFFFF",
                        "children": [
                            {
                                "name": "UnfilledOnLight",
                                "kind": "text",
                                "text": "Invisible on white",
                                "font_size": 16.0,
                                "text_color": "none",
                                "width": 160.0,
                                "height": 24.0
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec light");
        assert_eq!(light_reply["auto_fix"]["applied"], json!([]));
        let light_sugg = light_reply["auto_fix"]["suggested"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["rule"] == "text-missing-fill")
            .expect("text-missing-fill suggested on light");
        assert_eq!(light_sugg["property"], "fills");
        assert_eq!(light_sugg["current"], json!([]));
        assert_eq!(light_sugg["suggested"], "#111111");
        assert_eq!(
            light_sugg["fix"],
            json!({
                "kind": "set",
                "property": "fills",
                "from": [],
                "to": "#111111"
            })
        );

        // 2. Dark background (#111111) -> suggests "#FFFFFF"
        let dark_reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "DarkFrame",
                        "kind": "frame",
                        "width": 240.0,
                        "height": 80.0,
                        "fill": "#111111",
                        "children": [
                            {
                                "name": "UnfilledOnDark",
                                "kind": "text",
                                "text": "Invisible on dark",
                                "font_size": 16.0,
                                "text_color": "none",
                                "width": 160.0,
                                "height": 24.0
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec dark");
        let dark_sugg = dark_reply["auto_fix"]["suggested"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["rule"] == "text-missing-fill")
            .expect("text-missing-fill suggested on dark");
        assert_eq!(dark_sugg["property"], "fills");
        assert_eq!(dark_sugg["current"], json!([]));
        assert_eq!(dark_sugg["suggested"], "#FFFFFF");
        assert_eq!(
            dark_sugg["fix"],
            json!({
                "kind": "set",
                "property": "fills",
                "from": [],
                "to": "#FFFFFF"
            })
        );
    }

    #[test]
    fn absolute_in_auto_layout_is_suggestion_only() {
        let mut engine = Engine::default();
        let frame_reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "AutoStack",
                        "kind": "frame",
                        "layout": {
                            "direction": "vertical",
                            "padding": [16.0, 16.0, 16.0, 16.0],
                            "gap": 8.0
                        },
                        "width": 240.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "Floater",
                                "kind": "rectangle",
                                "width": 40.0,
                                "height": 40.0,
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("create AutoStack");
        let floater_id = frame_reply["node"]["children"][0].as_u64().unwrap();
        engine.doc.get_mut(NodeId(floater_id)).unwrap().layout_child.absolute = true;

        let update_reply = engine
            .call(
                "designos_update_node",
                json!({
                    "node_id": floater_id,
                    "name": "FloaterAbs"
                }),
            )
            .expect("update absolute child");

        assert_eq!(update_reply["auto_fix"]["applied"], json!([]));
        let abs_sugg = update_reply["auto_fix"]["suggested"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["rule"] == "absolute-in-auto-layout")
            .expect("absolute-in-auto-layout in suggested");
        assert_eq!(abs_sugg["property"], "layout_child.absolute");
        assert_eq!(abs_sugg["current"], true);
        assert_eq!(abs_sugg["suggested"], false);
        assert_eq!(
            abs_sugg["fix"],
            json!({
                "kind": "set",
                "property": "layout_child.absolute",
                "from": true,
                "to": false
            })
        );
        assert!(engine.doc.get(NodeId(floater_id)).unwrap().layout_child.absolute);
    }

    #[test]
    fn off_scale_is_suggestion_only() {
        let mut engine = Engine::default();
        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "OffScaleCard",
                        "kind": "frame",
                        "layout": {
                            "direction": "vertical",
                            "padding": [13.0, 13.0, 13.0, 13.0],
                            "gap": 8.0
                        },
                        "width": 240.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "Item",
                                "kind": "rectangle",
                                "width": 80.0,
                                "height": 32.0,
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec off_scale");

        assert_eq!(reply["auto_fix"]["applied"], json!([]));
        let suggested = reply["auto_fix"]["suggested"].as_array().unwrap();
        let off = suggested.iter().find(|s| s["rule"] == "off-scale").expect("off-scale in suggested");
        assert!(off["reason"].as_str().unwrap_or("").contains("off the spacing scale"));
        assert_eq!(off["fix"]["to"], 12.0);

        let card_id = reply["root_node_id"].as_u64().unwrap();
        let al = engine.doc.get(NodeId(card_id)).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al.padding, [13.0, 13.0, 13.0, 13.0]);
    }

    fn seed_single_color_variable(doc: &mut designos_doc::Document, var_id: &str, name: &str, color: designos_doc::Color) {
        use designos_doc::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};
        use std::collections::BTreeMap;

        let coll_id = "coll_tokens".to_string();
        let mode_id = "mode_default".to_string();
        doc.variable_collections.entry(coll_id.clone()).or_insert_with(|| VariableCollection {
            id: coll_id.clone(),
            name: "Color".to_string(),
            modes: vec![VariableMode { id: mode_id.clone(), name: "Default".to_string() }],
            default_mode: mode_id.clone(),
            variables: Vec::new(),
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        });
        doc.variable_collections.get_mut(&coll_id).unwrap().variables.push(var_id.to_string());
        let mut values = BTreeMap::new();
        values.insert(mode_id, VariableValue::Color(color));
        doc.variables.insert(
            var_id.to_string(),
            Variable {
                id: var_id.to_string(),
                name: name.to_string(),
                collection: coll_id,
                kind: VariableKind::Color,
                values,
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }

    #[test]
    fn bind_unchanged_value_applied() {
        let mut engine = Engine::default();
        seed_single_color_variable(&mut engine.doc, "var_primary", "Primary", designos_doc::Color::rgb(0x25, 0x63, 0xEB));

        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "PrimarySwatch",
                        "kind": "rectangle",
                        "width": 120.0,
                        "height": 40.0,
                        "fill": "#2563EB"
                    }
                }),
            )
            .expect("apply_spec with literal #2563EB");

        let rect_id = reply["root_node_id"].as_u64().unwrap();
        let applied = reply["auto_fix"]["applied"].as_array().unwrap();
        assert_eq!(applied.len(), 1, "expected 1 applied binding fix: {reply}");
        assert_eq!(applied[0]["rule"], "hardcoded-value");
        assert_eq!(applied[0]["property"], "fills/0");
        assert_eq!(applied[0]["before"], "#2563EB");
        assert_eq!(applied[0]["after"], "var:Color/Primary");
        assert_eq!(reply["issues"], json!([]));

        let node = engine.doc.get(NodeId(rect_id)).unwrap();
        assert_eq!(node.variables.bindings.get("fills/0").map(String::as_str), Some("var_primary"));
        assert_eq!(node.fills[0].kind, designos_doc::PaintKind::Solid(designos_doc::Color::rgb(0x25, 0x63, 0xEB)));
    }

    #[test]
    fn binding_never_overwritten() {
        let mut engine = Engine::default();
        seed_single_color_variable(&mut engine.doc, "var_primary", "Primary", designos_doc::Color::rgb(0x25, 0x63, 0xEB));
        seed_single_color_variable(&mut engine.doc, "var_secondary", "Secondary", designos_doc::Color::rgb(0x25, 0x63, 0xEB));

        let created = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "BoundRect",
                        "kind": "rectangle",
                        "width": 120.0,
                        "height": 40.0,
                        "fill": "var:Color/Secondary"
                    }
                }),
            )
            .expect("create bound rect");
        let rect_id = created["root_node_id"].as_u64().unwrap();
        assert_eq!(engine.doc.get(NodeId(rect_id)).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("var_secondary"));

        let updated = engine
            .call(
                "designos_update_node",
                json!({
                    "node_id": rect_id,
                    "name": "BoundRectRenamed"
                }),
            )
            .expect("update bound rect");

        assert_eq!(updated["auto_fix"]["applied"], json!([]));
        assert_eq!(engine.doc.get(NodeId(rect_id)).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("var_secondary"));
    }

    #[test]
    fn instance_mirror_skipped() {
        let mut engine = Engine::default();
        // Create a component directly in the document whose child has a safe-fixable finding (`sizing-conflict`),
        // then instantiate that component via `designos_apply_spec` and verify the instance mirror is never mutated.
        let page = engine.resolve_default_page().unwrap();
        let comp_id = engine.doc.alloc_id();
        let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 200.0, 80.0));
        comp.name = "MasterBadge".to_string();
        comp.kind = designos_doc::NodeKind::Component(designos_doc::FrameProps::default());
        engine.doc.insert(page, comp).unwrap();

        let comp_child_id = engine.doc.alloc_id();
        let mut comp_child = Node::rectangle(comp_child_id, Rect::new(0.0, 0.0, 80.0, 32.0));
        comp_child.name = "MasterInner".to_string();
        comp_child.layout_child.horizontal = designos_doc::Sizing::Fill;
        engine.doc.insert(comp_id, comp_child).unwrap();

        let inst_reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "BadgeInstance",
                        "kind": "instance",
                        "component": "MasterBadge"
                    }
                }),
            )
            .expect("instantiate MasterBadge");

        let inst_id = inst_reply["root_node_id"].as_u64().unwrap();
        let applied = inst_reply["auto_fix"]["applied"].as_array().unwrap();
        assert!(applied.is_empty(), "instance mirrors must never be mutated by auto-fix: {applied:?}");

        let inst_children = &engine.doc.get(NodeId(inst_id)).unwrap().children;
        assert_eq!(inst_children.len(), 1);
        let mirror = engine.doc.get(inst_children[0]).unwrap();
        assert!(mirror.component.source.is_some());
        assert_eq!(mirror.layout_child.horizontal, designos_doc::Sizing::Fill);
    }

    #[test]
    fn undo_of_auto_fix_keeps_write_key_valid() {
        let tmp = std::env::temp_dir().join(format!(
            "designos-mcp-autofix-ledger-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let ws = tmp.join("ws");
        let sess_dir = tmp.join("session");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&sess_dir).unwrap();

        let mut session = Session::with_session_dir(&ws, &sess_dir).expect("session");
        let spec_args = json!({
            "idempotency_key": "k1",
            "parent_id": 1,
            "spec": {
                "name": "StaticCard",
                "kind": "frame",
                "width": 320.0,
                "height": 120.0,
                "fill": "#18181B",
                "children": [
                    {
                        "name": "FillChild",
                        "kind": "rectangle",
                        "width": 120.0,
                        "height": 40.0,
                        "width_sizing": "fill",
                        "fill": "#3B82F6"
                    }
                ]
            }
        });

        let first = session.call("designos_apply_spec", spec_args.clone()).expect("first call with k1");
        assert_eq!(first["auto_fix"]["applied"].as_array().unwrap().len(), 1);

        // Undo once: reverts ONLY the `"Auto-fix"` entry; the write itself is still present.
        session.call("designos_undo", json!({})).expect("undo Auto-fix");

        // Retrying with `"k1"` MUST replay as a no-op (not IDEMPOTENCY_KEY_STALE) and return the pre-auto-fix reply.
        let (replayed, meta) = session.call_with_meta("designos_apply_spec", spec_args.clone()).expect("replay k1 after undoing only Auto-fix");
        assert_eq!(meta.and_then(|m| m.get("designos/idempotent_replay").cloned()), Some(json!(true)));
        assert_eq!(replayed["auto_fix"]["passes"], 0);
        assert_eq!(replayed["auto_fix"]["applied"], json!([]));

        // Undoing a second time undoes the write itself -> `"k1"` becomes stale.
        session.call("designos_undo", json!({})).expect("undo the write itself");
        let stale_err = session.call("designos_apply_spec", spec_args).unwrap_err();
        assert_eq!(stale_err.code, crate::tools::errors::ErrorCode::IdempotencyKeyStale);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn two_pass_limit() {
        use designos_doc::lint::{Category, Finding, FixSuggestion, Severity};
        use std::sync::Arc;

        let mut engine2 = Engine::default();
        seed_single_color_variable(&mut engine2.doc, "var_p2", "Pass2Blue", designos_doc::Color::rgb(0x25, 0x63, 0xEB));
        let p_id = engine2.resolve_default_page().unwrap();
        let target_rect = engine2.doc.alloc_id();
        let mut rnode = Node::rectangle(target_rect, Rect::new(0.0, 0.0, 100.0, 40.0));
        rnode.name = "Pass2Target".to_string();
        rnode.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0x25, 0x63, 0xEB))];
        engine2.doc.insert(p_id, rnode).unwrap();

        engine2.test_lint_hook = Some(Arc::new(move |_doc, _roots, pass_idx| {
            if pass_idx == 1 {
                vec![Finding {
                    rule: "hardcoded-value",
                    category: Category::Tokens,
                    severity: Severity::Warning,
                    node: target_rect,
                    node_name: "Pass2Target".into(),
                    xpath: String::new(),
                    bounds: Rect::new(0.0, 0.0, 100.0, 40.0),
                    message: "Pass 2 safe finding".into(),
                    measured_ratio: None,
                    required_ratio: None,
                    level: None,
                    large: None,
                    method: None,
                    worst_sample: None,
                    foreground_hex: None,
                    backdrop_hex: None,
                    suggested_fix: Some(FixSuggestion {
                        kind: "bind".into(),
                        property: "fills/0".into(),
                        from: json!("#2563EB"),
                        to: json!("var:Color/Pass2Blue"),
                    }),
                    fixable: true,
                    safe: true,
                    property: Some("fills/0".into()),
                    value: None,
                    nearest: None,
                    scale_source: None,
                    candidates: None,
                    ignored: false,
                }]
            } else {
                Vec::new()
            }
        }));

        let reply2 = engine2
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "Pass1Frame",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "Pass1Child",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "fill": "#112233"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec 2 passes");

        assert_eq!(reply2["auto_fix"]["passes"], 2);
        assert_eq!(reply2["auto_fix"]["applied"].as_array().unwrap().len(), 2);
        // Both passes are folded into a single `"Auto-fix"` undo entry!
        assert_eq!(engine2.history.undo_depth(), 2); // 1 for apply_spec + 1 for folded Auto-fix
    }

    #[test]
    fn auto_fix_edge_cases_disabled_by_precedence_fig_roundtrip_and_patch_rollback() {
        // 1. Rule disabled per file (`Document.auto_fix`) -> `disabled_by: "file"` in `auto_fix.suggested` (§8.1).
        let mut engine = Engine::default();
        let mut file_af = designos_doc::AutoFixSettings::default();
        file_af.rules.insert("sizing-conflict".into(), false);
        engine.doc.auto_fix = Some(file_af);

        let reply = engine
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "FileDisabledCard",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "FillChild",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec with file rule disabled");
        assert_eq!(reply["auto_fix"]["applied"], json!([]));
        let sugg = reply["auto_fix"]["suggested"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["rule"] == "sizing-conflict")
            .expect("sizing-conflict in suggested when disabled by file");
        assert_eq!(sugg["disabled_by"], "file");

        // 2. Session overrides user `off` (§8.1).
        let mut engine2 = Engine {
            user_auto_fix: Some(designos_doc::AutoFixSettings { mode: Some(designos_doc::AutoFixMode::Off), rules: std::collections::BTreeMap::new() }),
            ..Default::default()
        };
        engine2.call("designos_configure_session", json!({ "auto_fix": { "mode": "on" } })).expect("session override on");
        let reply2 = engine2
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "SessionOverrideCard",
                        "kind": "frame",
                        "width": 320.0,
                        "height": 120.0,
                        "fill": "#18181B",
                        "children": [
                            {
                                "name": "FillChild",
                                "kind": "rectangle",
                                "width": 120.0,
                                "height": 40.0,
                                "width_sizing": "fill",
                                "fill": "#3B82F6"
                            }
                        ]
                    }
                }),
            )
            .expect("apply_spec with session override");
        assert_eq!(reply2["auto_fix"]["applied"].as_array().unwrap().len(), 1);

        // 3. Suggest-tier rule (`"off-scale"`) in `auto_fix.rules` is rejected with `UNKNOWN_FIELD` naming the safe rules (§8.1).
        let err = engine2
            .call(
                "designos_configure_session",
                json!({
                    "auto_fix": {
                        "rules": { "off-scale": true }
                    }
                }),
            )
            .unwrap_err();
        assert_eq!(err.code, crate::tools::errors::ErrorCode::UnknownField);
        assert_eq!(err.valid_options, vec!["sizing-conflict".to_string(), "hardcoded-value".to_string()]);

        // 4. `.fig` roundtrip preserves `Document.auto_fix` (`designos:auto_fix` pluginData on DOCUMENT).
        let fig_bytes = designos_fig::export_fig(&engine.doc).expect("export_fig");
        let reimported = designos_fig::import_fig(&fig_bytes).expect("import_fig");
        assert_eq!(reimported.auto_fix, engine.doc.auto_fix);
    }

    #[test]
    fn preexisting_safe_finding_in_large_parent_not_auto_fixed() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let parent_frame = engine.doc.alloc_id();
        let mut pnode = Node::frame(parent_frame, Rect::new(0.0, 0.0, 1200.0, 1200.0));
        pnode.name = "LargeParent".to_string();
        engine.doc.insert(page, pnode).unwrap();

        // Untouched sibling with a pre-existing safe-fixable `sizing-conflict` (FILL inside non-auto-layout parent).
        let sibling_id = engine.doc.alloc_id();
        let mut sib = Node::rectangle(sibling_id, Rect::new(0.0, 0.0, 80.0, 32.0));
        sib.name = "PreexistingFillSibling".to_string();
        sib.layout_child.horizontal = designos_doc::Sizing::Fill;
        engine.doc.insert(parent_frame, sib).unwrap();

        // Populate > 500 nodes inside `parent_frame` so `PreCallBaseline` exceeds `MAX_PREEXISTING_SCOPE_NODES` (500).
        for i in 0..510 {
            let cid = engine.doc.alloc_id();
            let mut r = Node::rectangle(cid, Rect::new((i % 25) as f32 * 20.0, (i / 25) as f32 * 20.0, 16.0, 16.0));
            r.name = format!("Filler{i}");
            engine.doc.insert(parent_frame, r).unwrap();
        }

        // 1. In `report_only` mode, creating a clean child inside `LargeParent` must NOT report the untouched sibling in `would_apply`.
        engine.call("designos_configure_session", json!({ "auto_fix": { "mode": "report_only" } })).unwrap();
        let rep_only = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": parent_frame.0,
                    "kind": "rectangle",
                    "name": "CleanNewChild",
                    "x": 600.0,
                    "y": 600.0,
                    "width": 40.0,
                    "height": 40.0
                }),
            )
            .expect("create_node in report_only");
        let would_apply = rep_only["auto_fix"]["would_apply"].as_array().expect("would_apply");
        assert!(
            would_apply.iter().all(|e| e["node_id"] != sibling_id.0),
            "pre-existing sibling in >500-node parent must not appear in would_apply: {would_apply:?}"
        );
        assert_eq!(engine.doc.get(sibling_id).unwrap().layout_child.horizontal, designos_doc::Sizing::Fill);

        // 2. Also test `report_only` in a <= 500-node parent where `baseline.contains(...) == Some(true)`.
        let small_parent = engine.doc.alloc_id();
        let mut spnode = Node::frame(small_parent, Rect::new(0.0, 1300.0, 400.0, 200.0));
        spnode.name = "SmallParent".to_string();
        engine.doc.insert(page, spnode).unwrap();
        let small_sib_id = engine.doc.alloc_id();
        let mut small_sib = Node::rectangle(small_sib_id, Rect::new(0.0, 0.0, 80.0, 32.0));
        small_sib.name = "SmallPreexistingFillSibling".to_string();
        small_sib.layout_child.horizontal = designos_doc::Sizing::Fill;
        engine.doc.insert(small_parent, small_sib).unwrap();

        let rep_small = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": small_parent.0,
                    "kind": "rectangle",
                    "name": "SmallCleanChild",
                    "x": 100.0,
                    "y": 0.0,
                    "width": 40.0,
                    "height": 40.0
                }),
            )
            .expect("create_node in small parent report_only");
        let small_would_apply = rep_small["auto_fix"]["would_apply"].as_array().expect("small would_apply");
        assert!(
            small_would_apply.iter().all(|e| e["node_id"] != small_sib_id.0),
            "pre-existing sibling in <=500-node parent must not appear in would_apply: {small_would_apply:?}"
        );

        // 3. In `on` mode, creating a child inside `LargeParent` must NOT mutate the untouched sibling.
        engine.call("designos_configure_session", json!({ "auto_fix": { "mode": "on" } })).unwrap();
        let on_reply = engine
            .call(
                "designos_create_node",
                json!({
                    "parent_id": parent_frame.0,
                    "kind": "rectangle",
                    "name": "SecondCleanChild",
                    "x": 660.0,
                    "y": 600.0,
                    "width": 40.0,
                    "height": 40.0
                }),
            )
            .expect("create_node in on mode");
        let applied = on_reply["auto_fix"]["applied"].as_array().expect("applied");
        assert!(applied.iter().all(|e| e["node_id"] != sibling_id.0), "pre-existing sibling in >500-node parent must not be auto-fixed: {applied:?}");
        assert_eq!(engine.doc.get(sibling_id).unwrap().layout_child.horizontal, designos_doc::Sizing::Fill, "untouched sibling sizing must remain Fill");
    }
}
