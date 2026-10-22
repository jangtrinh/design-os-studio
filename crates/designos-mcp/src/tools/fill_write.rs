//! The colour of a node written through `create_node`, `update_node` and `apply_batch`.
//!
//! A literal colour (hex or `$easeui.*`) replaces the paints; a `var:` reference binds the variable the
//! way `apply_spec` does. Either way the node is changed in place on the caller's copy, so the caller
//! commits it as one `Command` and one undo step undoes the colour together with its binding.

use crate::tools::errors::{ErrorCode, ToolError};
use crate::tools::library_source::enabled_snapshots;
use crate::tools::set_node_fills;
use designos_doc::{
    bind_variable_on_node, names_library_variable, resolve_colour_ref, resolve_easeui_color, Document, Node, NodeId, Paint, ResolvedColourRef, SpecBindIndex,
    SpecError, SpecWarning, VariableValue,
};
use serde_json::json;

/// Whether a `fill` string names a variable (`var:<collection>/<name>`) instead of a literal colour.
pub(crate) fn is_variable_ref(raw: &str) -> bool {
    raw.trim_start().starts_with("var:")
}

fn invalid_colour(raw: &str) -> ToolError {
    ToolError::new(ErrorCode::ValueOutOfRange, "/fill", format!("invalid fill color '{raw}'"))
        .with_received(json!(raw))
        .with_expected("#RRGGBB or #RRGGBBAA hex, a $easeui.* token, or var:<collection>/<name>")
}

/// Whether a colour is written to a layer being built or to one that already exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FillTarget {
    NewLayer,
    ExistingLayer,
}

/// The reference names a variable of an enabled library. This write cannot import it, because an import is a document
/// change of its own, so the error states the limit and where to go instead. It carries no `corrected_example` and sets
/// no retry flag: the session rewrites an example into this same call's arguments and sets the flag itself, so neither
/// can say "call another tool".
fn library_variable_error(raw: &str, target: FillTarget) -> ToolError {
    let next_step = match target {
        FillTarget::NewLayer => "Build this layer with designos_apply_spec using the same var:.",
        FillTarget::ExistingLayer => {
            "Library variables bind only through designos_apply_spec in this version. Rebuild the layer that way, or use a local variable."
        }
    };
    ToolError::new(ErrorCode::UnknownVariable, "/fill", "Library variable. Import it with designos_apply_spec first.")
        .with_received(json!(raw))
        .with_next_step(next_step)
}

/// Checks a `fill` string that is not applied yet. A `var:` reference is only resolved against a
/// document when it is written, so it passes here.
pub(crate) fn validate_fill_color(fill: Option<&str>) -> Result<(), ToolError> {
    match fill {
        Some(f) if !is_variable_ref(f) && resolve_easeui_color(f).is_none() => Err(invalid_colour(f)),
        _ => Ok(()),
    }
}

/// Writes `fills` (which win) or the `fill` colour on `node`, a copy of the node as it will be committed.
///
/// `mode_anchor` is the node whose variable modes apply to a bound value: the node itself when it is
/// updated, its parent when it is about to be created. Only variables of the document itself resolve;
/// a library variable has to be imported by `apply_spec` first, because importing is a document change
/// of its own and would not share this write's undo step. `target` only shapes that error's advice.
pub(crate) fn write_fill(
    doc: &mut Document,
    node: &mut Node,
    mode_anchor: NodeId,
    target: FillTarget,
    fill: Option<&str>,
    fills: Option<Vec<Paint>>,
    warnings: &mut Vec<SpecWarning>,
) -> Result<(), ToolError> {
    if let Some(paints) = fills {
        set_node_fills(node, paints);
        return Ok(());
    }
    let Some(raw) = fill else { return Ok(()) };
    if !is_variable_ref(raw) {
        let colour = resolve_easeui_color(raw).ok_or_else(|| invalid_colour(raw))?;
        set_node_fills(node, vec![Paint::solid(colour)]);
        return Ok(());
    }
    let kind = node.kind.clone();
    let mut index = SpecBindIndex::build(doc);
    let resolved = resolve_colour_ref(doc, &mut index, &[], mode_anchor, &kind, "fills/0", "fill", raw, "/fill", warnings).map_err(|e| {
        if e.code == "UNKNOWN_VARIABLE" && names_library_variable(doc, &enabled_snapshots(doc), raw) {
            return library_variable_error(raw, target);
        }
        crate::tools::errors::from_spec_error(SpecError::from(e), "")
    })?;
    let ResolvedColourRef::Variable { var_id, color } = resolved else {
        return Err(ToolError::new(ErrorCode::InternalError, "/fill", format!("'{raw}' did not resolve to a variable")));
    };
    set_node_fills(node, vec![Paint::solid(color)]);
    bind_variable_on_node(node, &["fills/0"], &var_id, Some(&VariableValue::Color(color)));
    Ok(())
}
