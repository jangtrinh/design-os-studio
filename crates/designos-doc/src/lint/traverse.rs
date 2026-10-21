//! Single-pass O(1)-bounds tree traversal and budget/cursor continuation for [`super::lint_subtree`].

use super::config::LintConfig;
use super::finding::{Finding, LintReport, LintSummary, SkippedRule};
use super::registry::{Category, RuleNeeds, RULES};
use super::rules::{aabb_of_transformed_rect, detached, evaluate_node, severity_rank, ActiveRules, NodeEvalCtx, ParentEvalCtx};
use super::LintEnv;
use crate::{Affine, Document, Node, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// Execution budget for [`lint_subtree`], capping visited nodes and/or wall-clock duration.
#[derive(Clone, Debug)]
pub struct Budget {
    pub max_nodes: usize,
    pub max_duration: Option<Duration>,
    pub cursor: Option<String>,
    /// Nodes whose own rules are evaluated when visited, without descending into their children.
    pub shallow_nodes: BTreeSet<NodeId>,
}

impl Default for Budget {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl Budget {
    pub fn unlimited() -> Self {
        Self { max_nodes: usize::MAX, max_duration: None, cursor: None, shallow_nodes: BTreeSet::new() }
    }

    pub fn with_limits(max_nodes: usize, max_duration: Option<Duration>) -> Self {
        Self { max_nodes, max_duration, cursor: None, shallow_nodes: BTreeSet::new() }
    }
}

#[derive(Clone, Copy, Debug)]
struct StackItem<'a> {
    id: NodeId,
    node: Option<&'a Node>,
    parent: Option<ParentEvalCtx<'a>>,
    parent_world: Affine,
    depth: usize,
    in_component: bool,
}

fn make_parent_ctx<'a>(doc: &'a Document, parent_node: &'a Node, active: &ActiveRules) -> ParentEvalCtx<'a> {
    let has_mask = (active.overflow_parent || active.clipped_partial || active.clipped_full)
        && parent_node.kind.frame_props().is_some()
        && parent_node.children.iter().any(|&cid| doc.get(cid).is_some_and(|c| c.visible && c.is_mask));
    ParentEvalCtx { node: parent_node, has_mask }
}

/// Computes `(parent_ctx, parent_world, depth, in_component)` for an arbitrary starting `node_id` by walking its ancestor chain.
fn ancestor_state<'a>(doc: &'a Document, node_id: NodeId, active: &ActiveRules) -> (Option<ParentEvalCtx<'a>>, Affine, usize, bool) {
    let direct_parent = doc.get(node_id).and_then(|n| n.parent).and_then(|pid| doc.get(pid)).map(|pn| make_parent_ctx(doc, pn, active));
    let mut chain = Vec::new();
    let mut cur = doc.get(node_id).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let Some(pn) = doc.get(pid) else { break };
        if matches!(pn.kind, NodeKind::Page { .. }) {
            break;
        }
        chain.push(pn);
        cur = pn.parent;
    }
    let depth = chain.len() + 1;
    let mut world = Affine::IDENTITY;
    let mut in_component = false;
    for anc in chain.into_iter().rev() {
        world = world * anc.transform;
        if matches!(anc.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
            in_component = true;
        }
    }
    (direct_parent, world, depth, in_component)
}

fn push_children<'a>(
    n: &'a Node,
    children: &[&'a Node],
    node_world: Affine,
    depth: usize,
    in_component: bool,
    active: &ActiveRules,
    stack: &mut Vec<StackItem<'a>>,
) {
    if children.is_empty() {
        return;
    }
    let has_mask = (active.overflow_parent || active.clipped_partial || active.clipped_full) && children.iter().any(|c| c.visible && c.is_mask);
    let pctx = Some(ParentEvalCtx { node: n, has_mask });
    let child_in_comp = in_component || matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
    if n.paints_children_reversed() {
        for &child in children {
            stack.push(StackItem { id: child.id, node: Some(child), parent: pctx, parent_world: node_world, depth: depth + 1, in_component: child_in_comp });
        }
    } else {
        for &child in children.iter().rev() {
            stack.push(StackItem { id: child.id, node: Some(child), parent: pctx, parent_world: node_world, depth: depth + 1, in_component: child_in_comp });
        }
    }
}

fn push_root_items<'a>(doc: &'a Document, root_id: NodeId, active: &ActiveRules, stack: &mut Vec<StackItem<'a>>) {
    let Some(root_node) = doc.get(root_id) else {
        return;
    };
    if matches!(root_node.kind, NodeKind::Page { .. }) {
        let pctx = Some(ParentEvalCtx { node: root_node, has_mask: false });
        for &cid in root_node.children.iter().rev() {
            stack.push(StackItem { id: cid, node: doc.get(cid), parent: pctx, parent_world: Affine::IDENTITY, depth: 1, in_component: false });
        }
    } else {
        let (parent, parent_world, depth, in_component) = ancestor_state(doc, root_id, active);
        stack.push(StackItem { id: root_id, node: Some(root_node), parent, parent_world, depth, in_component });
    }
}

/// Reconstructs the pre-order DFS `stack` from `roots` down to `resume_id` in $O(\text{depth})$
/// ancestor steps so resuming from `"stack:<node_id>"` does not serialize or parse thousands of
/// sibling IDs on every slice.
fn push_stack_from_cursor_node<'a>(doc: &'a Document, roots: &[NodeId], resume_id: NodeId, active: &ActiveRules, stack: &mut Vec<StackItem<'a>>) -> bool {
    let Some(resume_node) = doc.get(resume_id) else {
        return false;
    };
    let mut chain = vec![resume_id];
    let mut cur = resume_id;
    while !roots.contains(&cur) {
        let Some(p) = doc.get(cur).and_then(|n| n.parent) else {
            return false;
        };
        chain.push(p);
        cur = p;
    }
    chain.reverse();
    let root_id = chain[0];
    let Some(r_idx) = roots.iter().position(|&id| id == root_id) else {
        return false;
    };

    for &later_root in roots[r_idx + 1..].iter().rev() {
        push_root_items(doc, later_root, active, stack);
    }

    for d in 0..chain.len().saturating_sub(1) {
        let curr_id = chain[d];
        let next_id = chain[d + 1];
        let Some(curr_node) = doc.get(curr_id) else {
            return false;
        };
        if matches!(curr_node.kind, NodeKind::Page { .. }) {
            let pctx = Some(ParentEvalCtx { node: curr_node, has_mask: false });
            if let Some(c_idx) = curr_node.children.iter().position(|&id| id == next_id) {
                for &sib_id in curr_node.children[c_idx + 1..].iter().rev() {
                    stack.push(StackItem { id: sib_id, node: doc.get(sib_id), parent: pctx, parent_world: Affine::IDENTITY, depth: 1, in_component: false });
                }
            }
        } else {
            let (_, p_world, p_depth, p_in_comp) = ancestor_state(doc, curr_id, active);
            let node_world = p_world * curr_node.transform;
            let child_in_comp = p_in_comp || matches!(curr_node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
            let pctx = Some(make_parent_ctx(doc, curr_node, active));
            let paint_children = curr_node.paint_children();
            if let Some(c_idx) = paint_children.iter().position(|&id| id == next_id) {
                for &sib_id in paint_children[c_idx + 1..].iter().rev() {
                    stack.push(StackItem {
                        id: sib_id,
                        node: doc.get(sib_id),
                        parent: pctx,
                        parent_world: node_world,
                        depth: p_depth + 1,
                        in_component: child_in_comp,
                    });
                }
            }
        }
    }

    let (parent, parent_world, depth, in_component) = ancestor_state(doc, resume_id, active);
    stack.push(StackItem { id: resume_id, node: Some(resume_node), parent, parent_world, depth, in_component });
    true
}

fn build_initial_stack<'a>(
    doc: &'a Document,
    roots: &[NodeId],
    active_cursor: Option<&str>,
    active: &ActiveRules,
    page_offset: &mut usize,
) -> Vec<StackItem<'a>> {
    let mut stack: Vec<StackItem<'a>> = Vec::new();
    if let Some(cur_str) = active_cursor {
        if let Some(rest) = cur_str.strip_prefix("stack:") {
            let trimmed = rest.trim();
            if !trimmed.contains(',') {
                if let Ok(raw_id) = trimmed.parse::<u64>() {
                    let _ = push_stack_from_cursor_node(doc, roots, NodeId(raw_id), active, &mut stack);
                }
            } else {
                for part in trimmed.split(',').rev() {
                    if let Ok(raw_id) = part.trim().parse::<u64>() {
                        let nid = NodeId(raw_id);
                        if let Some(n) = doc.get(nid) {
                            let (parent, parent_world, depth, in_component) = ancestor_state(doc, nid, active);
                            stack.push(StackItem { id: nid, node: Some(n), parent, parent_world, depth, in_component });
                        }
                    }
                }
            }
        } else if let Some(rest) = cur_str.strip_prefix("offset:") {
            *page_offset = rest.trim().parse::<usize>().unwrap_or(0);
        } else if let Ok(off) = cur_str.trim().parse::<usize>() {
            *page_offset = off;
        }
    }

    if stack.is_empty() {
        for &root_id in roots.iter().rev() {
            push_root_items(doc, root_id, active, &mut stack);
        }
    }
    stack
}

#[allow(clippy::too_many_arguments)]
fn scan_stack_slice<'a>(
    doc: &'a Document,
    stack: &mut Vec<StackItem<'a>>,
    config: &LintConfig,
    active: &ActiveRules,
    env: &dyn LintEnv,
    components: &detached::Components,
    scales: &super::rules::scale::Scales,
    token_index: &super::token_match::Index,
    shallow_nodes: &BTreeSet<NodeId>,
    max_nodes: usize,
    max_duration: Option<Duration>,
    raw_findings: &mut Vec<Finding>,
) -> (usize, bool, Option<String>) {
    let mut scanned = 0usize;
    let mut complete = true;
    let mut next_cursor: Option<String> = None;
    let loop_start = Instant::now();
    let mut child_buf: Vec<&'a Node> = Vec::new();

    while let Some(item) = stack.pop() {
        let Some(n) = item.node.or_else(|| doc.get(item.id)) else {
            continue;
        };
        if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
            continue;
        }

        if scanned >= max_nodes || max_duration.is_some_and(|max_d| scanned >= 32 && (scanned & 31) == 0 && loop_start.elapsed() >= max_d) {
            stack.push(item);
            complete = false;
            next_cursor = Some(format!("stack:{}", item.id.0));
            break;
        }

        child_buf.clear();
        if !n.children.is_empty() {
            child_buf.extend(n.children.iter().filter_map(|&cid| doc.get(cid)));
        }

        let node_world = item.parent_world * n.transform;
        let page_bounds = aabb_of_transformed_rect(node_world, n.size);
        let is_instance_mirror = n.component.source.is_some() && n.component.overrides.is_empty();

        if !is_instance_mirror || config.include_instances {
            scanned += 1;
            let node_in_comp = item.in_component || matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_));
            let ctx = NodeEvalCtx {
                doc,
                node: n,
                parent: item.parent,
                children: &child_buf,
                depth: item.depth,
                in_component: node_in_comp,
                is_instance_mirror,
                page_bounds,
                config,
                active,
                env,
                components,
                scales,
                token_index,
            };
            evaluate_node(&ctx, raw_findings);
        }

        if !shallow_nodes.contains(&item.id) {
            push_children(n, &child_buf, node_world, item.depth, item.in_component, active, stack);
        }
    }

    (scanned, complete, next_cursor)
}

/// Evaluates enabled lint rules over `roots` within `budget`, returning a [`LintReport`].
pub fn lint_subtree(doc: &Document, roots: &[NodeId], config: &LintConfig, env: &dyn LintEnv, budget: &mut Budget) -> LintReport {
    lint_subtree_with_components(doc, roots, config, env, budget, None)
}

/// Evaluates enabled lint rules over `roots` within `budget`, optionally reusing a precomputed
/// [`detached::Components`] index across slices.
pub fn lint_subtree_with_components(
    doc: &Document,
    roots: &[NodeId],
    config: &LintConfig,
    env: &dyn LintEnv,
    budget: &mut Budget,
    precomputed_components: Option<&detached::Components>,
) -> LintReport {
    let start = Instant::now();
    let active = ActiveRules::from_config(config);

    let mut skipped_rules = Vec::new();
    if !env.has_text_engine() {
        for r in RULES {
            if r.needs == RuleNeeds::Text && config.is_enabled(r.id) {
                skipped_rules.push(SkippedRule { rule: r.id.to_string(), reason: "no_text_engine".to_string() });
            }
        }
    }

    let owned_components;
    let components: &detached::Components = if let Some(pre) = precomputed_components {
        pre
    } else {
        owned_components = if active.detached_instance { detached::Components::new(doc) } else { detached::Components::default() };
        &owned_components
    };

    let scales = super::rules::scale::scales_for(doc, config.archetype.as_ref());
    let token_index = if config.is_enabled("hardcoded-value") { super::token_match::Index::new(doc) } else { super::token_match::Index::default() };

    let active_cursor = budget.cursor.as_deref().or(config.cursor.as_deref());
    let mut page_offset = 0usize;
    let mut stack = build_initial_stack(doc, roots, active_cursor, &active, &mut page_offset);

    let mut raw_findings: Vec<Finding> = Vec::new();
    let (scanned, complete, mut next_cursor) = scan_stack_slice(
        doc,
        &mut stack,
        config,
        &active,
        env,
        components,
        &scales,
        &token_index,
        &budget.shallow_nodes,
        budget.max_nodes,
        budget.max_duration,
        &mut raw_findings,
    );

    // Filter or mark ignored findings in place when ignores exist.
    if !doc.check_designs.ignored.is_empty() {
        if config.include_ignored {
            for f in &mut raw_findings {
                if !f.ignored && doc.is_lint_ignored_property(f.rule, f.node, f.property.as_deref()) {
                    f.ignored = true;
                }
            }
        } else {
            raw_findings.retain(|f| !f.ignored && !doc.is_lint_ignored_property(f.rule, f.node, f.property.as_deref()));
        }
    }
    let mut filtered = raw_findings;

    // Stable sort by severity (error, warning, info), preserving document paint order within each.
    filtered.sort_by_key(|f| severity_rank(f.severity));

    let summary = summarize_findings(&filtered);
    let total = summary.total;

    let findings = if let Some(limit) = config.limit {
        let start_idx = page_offset.min(total);
        let end_idx = (start_idx + limit).min(total);
        if end_idx < total && next_cursor.is_none() {
            next_cursor = Some(format!("offset:{end_idx}"));
        }
        filtered.into_iter().skip(start_idx).take(end_idx - start_idx).collect()
    } else if page_offset > 0 {
        filtered.into_iter().skip(page_offset.min(total)).collect()
    } else {
        filtered
    };

    let truncated = !complete || findings.len() < total;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    budget.cursor = next_cursor.clone();

    LintReport {
        findings,
        total,
        by_severity: summary.by_severity.clone(),
        by_rule: summary.by_rule.clone(),
        skipped_rules,
        scanned,
        scanned_nodes: scanned,
        complete,
        truncated,
        elapsed_ms,
        cursor: next_cursor,
        summary,
    }
}

fn summarize_findings(findings: &[Finding]) -> LintSummary {
    let total = findings.len();
    let mut sev_counts = [0usize; 3];
    let mut cat_counts = [0usize; 8];
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();

    for f in findings {
        sev_counts[severity_rank(f.severity) as usize] += 1;
        cat_counts[f.category as usize] += 1;
        if let Some(count) = by_rule.get_mut(f.rule) {
            *count += 1;
        } else {
            by_rule.insert(f.rule.to_string(), 1);
        }
    }

    let errors = sev_counts[0];
    let warnings = sev_counts[1];
    let infos = sev_counts[2];
    let by_severity = BTreeMap::from([("error".to_string(), errors), ("warning".to_string(), warnings), ("info".to_string(), infos)]);
    let mut by_category: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, cat) in Category::ALL.iter().enumerate() {
        by_category.insert(cat.as_str().to_string(), cat_counts[idx]);
    }

    LintSummary { total, errors, warnings, infos, by_severity, by_rule, by_category }
}

/// Summary and per-slice timing metrics of a sliced lint run ([`lint_in_slices_with_budget`]).
#[derive(Clone, Debug)]
pub struct SliceRunStats {
    pub report: LintReport,
    pub slices: usize,
    pub max_slice_duration: Duration,
    pub total_duration: Duration,
}

/// Drives a complete scan across `roots` by repeated [`lint_subtree`] calls bounded by
/// `(max_nodes_per_slice, max_slice_duration)`, returning `(merged_report, slice_count)`.
pub fn lint_in_slices(
    doc: &Document,
    roots: &[NodeId],
    config: &LintConfig,
    env: &dyn LintEnv,
    max_nodes_per_slice: usize,
    max_slice_duration: Duration,
) -> (LintReport, usize) {
    let mut total_budget = Budget::unlimited();
    let stats = lint_in_slices_with_budget(doc, roots, config, env, max_nodes_per_slice, max_slice_duration, &mut total_budget);
    (stats.report, stats.slices)
}

/// Drives repeated [`lint_subtree`] slices bounded by `(max_nodes_per_slice, max_slice_duration)`
/// until `roots` are complete OR `total_budget` (`max_nodes` / `max_duration`) is spent, returning
/// [`SliceRunStats`] with `complete = false` and `cursor = Some("stack:<next_node_id>")` when stopped
/// by `total_budget`.
pub fn lint_in_slices_with_budget(
    doc: &Document,
    roots: &[NodeId],
    config: &LintConfig,
    env: &dyn LintEnv,
    max_nodes_per_slice: usize,
    max_slice_duration: Duration,
    total_budget: &mut Budget,
) -> SliceRunStats {
    let start = Instant::now();
    let active = ActiveRules::from_config(config);

    let mut skipped_rules: Vec<SkippedRule> = Vec::new();
    if !env.has_text_engine() {
        for r in RULES {
            if r.needs == RuleNeeds::Text && config.is_enabled(r.id) {
                skipped_rules.push(SkippedRule { rule: r.id.to_string(), reason: "no_text_engine".to_string() });
            }
        }
    }

    let components = if active.detached_instance { detached::Components::new(doc) } else { detached::Components::default() };
    let scales = super::rules::scale::scales_for(doc, config.archetype.as_ref());
    let token_index = if config.is_enabled("hardcoded-value") { super::token_match::Index::new(doc) } else { super::token_match::Index::default() };

    let initial_cursor = total_budget.cursor.as_deref().or_else(|| config.cursor.as_deref().filter(|c| c.starts_with("stack:")));
    let mut unused_offset = 0usize;
    let mut stack = build_initial_stack(doc, roots, initial_cursor, &active, &mut unused_offset);

    let mut raw_findings: Vec<Finding> = Vec::new();
    let mut total_scanned = 0usize;
    let mut cursor: Option<String> = None;
    let mut slices = 0usize;
    let mut worst_slice_duration = Duration::ZERO;
    let complete: bool;

    loop {
        let remaining_nodes = total_budget.max_nodes.saturating_sub(total_scanned);
        if remaining_nodes == 0 {
            complete = false;
            break;
        }
        slices += 1;
        let slice_nodes = max_nodes_per_slice.max(1).min(remaining_nodes);
        let t_slice = Instant::now();
        let (step_scanned, step_complete, step_cursor) = scan_stack_slice(
            doc,
            &mut stack,
            config,
            &active,
            env,
            &components,
            &scales,
            &token_index,
            &total_budget.shallow_nodes,
            slice_nodes,
            Some(max_slice_duration),
            &mut raw_findings,
        );
        worst_slice_duration = worst_slice_duration.max(t_slice.elapsed());
        total_scanned += step_scanned;
        cursor = step_cursor;
        if step_complete || cursor.is_none() {
            complete = true;
            cursor = None;
            break;
        }
        if total_scanned >= total_budget.max_nodes || total_budget.max_duration.is_some_and(|max_d| start.elapsed() >= max_d) {
            complete = false;
            break;
        }
    }

    if !doc.check_designs.ignored.is_empty() {
        if config.include_ignored {
            for f in &mut raw_findings {
                if !f.ignored && doc.is_lint_ignored_property(f.rule, f.node, f.property.as_deref()) {
                    f.ignored = true;
                }
            }
        } else {
            raw_findings.retain(|f| !f.ignored && !doc.is_lint_ignored_property(f.rule, f.node, f.property.as_deref()));
        }
    }
    let mut all_findings = raw_findings;

    all_findings.sort_by_key(|f| severity_rank(f.severity));
    let summary = summarize_findings(&all_findings);
    let total = summary.total;

    let mut next_cursor = cursor;
    let findings = if let Some(limit) = config.limit {
        let page_offset = config
            .cursor
            .as_deref()
            .filter(|c| !c.starts_with("stack:"))
            .and_then(|c| c.strip_prefix("offset:").or(Some(c)))
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let start_idx = page_offset.min(total);
        let end_idx = (start_idx + limit).min(total);
        if end_idx < total && next_cursor.is_none() {
            next_cursor = Some(format!("offset:{end_idx}"));
        }
        all_findings.into_iter().skip(start_idx).take(end_idx - start_idx).collect()
    } else {
        all_findings
    };

    let truncated = !complete || findings.len() < total;
    let total_duration = start.elapsed();
    let elapsed_ms = total_duration.as_millis() as u64;
    total_budget.cursor = next_cursor.clone();

    SliceRunStats {
        report: LintReport {
            findings,
            total,
            by_severity: summary.by_severity.clone(),
            by_rule: summary.by_rule.clone(),
            skipped_rules,
            scanned: total_scanned,
            scanned_nodes: total_scanned,
            complete,
            truncated,
            elapsed_ms,
            cursor: next_cursor,
            summary,
        },
        slices,
        max_slice_duration: worst_slice_duration,
        total_duration,
    }
}
