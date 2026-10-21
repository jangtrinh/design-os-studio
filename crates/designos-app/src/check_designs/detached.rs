//! Detached instances rule: a frame that carries `detachedSymbolId` provenance (set by a `.fig`
//! import) and is no longer an instance. When the main component is in this file and the frame
//! still equals it (apart from its name and where it sits), the fix swaps the frame for a fresh
//! instance of it. A frame that was edited after detaching is reported without a fix, because an
//! instance would silently throw those edits away. A copy of such a frame inside an instance is not
//! reported: it mirrors the detached frame of the main component, which is where it is reported
//! and reconnected once. A reconnect on the copy would be undone by the next sync of the instance.

use super::{Finding, Fix, Rule};
use designos_doc::lint::rules::detached as lint_detached;
use designos_doc::{Document, Node, NodeId};

pub(super) use lint_detached::{instance_name, is_main_component, matches_component, symbol_of, Comparison, Components};

/// What the detached check decided for one layer.
pub(super) enum Detached {
    /// Nothing to report.
    Nothing,
    /// A finding that needs no comparison.
    Found(Finding),
    /// A plain detached frame of `component`: the finding depends on whether it still equals it,
    /// which [`Comparison`] decides (and [`compared`] turns into the finding).
    Compare { component: NodeId },
}

pub(super) fn plan(doc: &Document, components: &Components, n: &Node) -> Detached {
    match lint_detached::plan(doc, components, n) {
        lint_detached::DetachedPlan::Nothing => Detached::Nothing,
        lint_detached::DetachedPlan::Unfixable { message } => {
            let mut cfg = designos_doc::lint::LintConfig::from_preset(designos_doc::lint::LintPreset::CheckDesigns);
            cfg.include_ignored = true;
            cfg.include_instances = true;
            let mut lint_budget = designos_doc::lint::Budget::with_limits(1, None);
            let report =
                designos_doc::lint::lint_subtree_with_components(doc, &[n.id], &cfg, &designos_render::RenderLintEnv, &mut lint_budget, Some(components));
            let msg = report.findings.into_iter().find(|f| f.rule == "detached-instance").map(|f| f.message).unwrap_or(message);
            Detached::Found(finding(n, msg, None, String::new()))
        }
        lint_detached::DetachedPlan::Compare { component } => Detached::Compare { component },
    }
}

/// The finding for a detached frame once it is known whether it still `equal`s its main component.
pub(super) fn compared(doc: &Document, n: &Node, component: NodeId, equal: bool) -> Finding {
    let (message, reconnect, fix_label) = lint_detached::compared_outcome(doc, component, equal);
    let fix = reconnect.map(|c| Fix::Reconnect { component: c });
    finding(n, message, fix, fix_label)
}

fn finding(n: &Node, message: String, fix: Option<Fix>, fix_label: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule: Rule::DetachedInstance, property: String::new(), message, fix, fix_label }
}
