//! The wording of a `--select` that matched no node: what was searched and what the lookup did before giving up.

use super::resolve::{for_each_layer, Sel, LARGE_FILE_NODES};
use super::SelectError;
use designos_doc::Document;

/// Rule 11: an id made by expanding instances differs from run to run, so every miss says what lasts.
const PER_SESSION: &str = "ids of expanded instance content are per session; use fig:<id> or a name";

/// `expanded` is true only when a page expansion ran and changed the document.
pub(super) fn miss(doc: &Document, sel: &Sel, raw: &str, expanded: bool) -> SelectError {
    let mut layers = 0;
    for_each_layer(doc, |_| layers += 1);
    let pages = doc.pages.len();
    let by = match sel {
        Sel::Id(_) => "id",
        Sel::Figma(_) => "Figma id",
        Sel::Bare(v) if v.parse::<u64>().is_ok() => "name or id",
        _ => "name",
    };
    let none_has = if let Sel::Id(id) = sel { format!("no node has id {}; ", id.0) } else { String::new() };
    let mut searched = format!(
        "({none_has}searched {layers} layers on {pages} page{} by {by}; use --inspect-json to list screens, or id:<n> / fig:<id>)",
        if pages == 1 { "" } else { "s" }
    );
    if expanded {
        let left = doc.hydration_unexpanded_instances();
        if left > 0 {
            searched.push_str(&format!(" (all pages expanded; {left} instances left unexpanded by the clone budget)"));
        } else {
            searched.push_str(" (all pages expanded)");
        }
    }
    if matches!(sel, Sel::Figma(_)) {
        if !doc.nodes.values().any(|n| n.source_id.is_some()) {
            searched.push_str(" (this document has no Figma ids)");
        }
        if doc.nodes.len() >= LARGE_FILE_NODES {
            searched.push_str(" (instance sublayers in files over 10,000 nodes keep no Figma id; select the instance and use id:<n>)");
        }
    }
    searched.push_str(&format!(" ({PER_SESSION})"));
    SelectError::NoMatch { raw: raw.into(), searched }
}
