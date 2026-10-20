//! Text styles rule (`REQUIRE_TEXT_STYLES`): flags visible non-empty text layers that are not
//! linked to a shared text style (`style_refs["text"]`), evaluated independently of number
//! variable rules. When exactly one local or enabled-library text style matches, offers
//! `Fix::ApplyStyle`.

use super::hardcoded::{self, Index};
use super::{Finding, Fix, Rule};
use designos_doc::{Node, NodeKind};

pub(super) fn check(index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    let NodeKind::Text(t) = &n.kind else { return };
    if t.content.trim().is_empty() || n.style_refs.contains_key("text") || hardcoded::text_style_inherited(n, src) {
        return;
    }
    if let Some((fix_key, quoted, apply_label)) = index.find_matching_text_style(t) {
        out.push(Finding {
            node: n.id,
            node_name: n.name.clone(),
            rule: Rule::TextStyles,
            property: "text".into(),
            message: format!("Text matches style {quoted}"),
            fix: Some(Fix::ApplyStyle { style: fix_key }),
            fix_label: apply_label,
        });
    } else {
        out.push(Finding {
            node: n.id,
            node_name: n.name.clone(),
            rule: Rule::TextStyles,
            property: "text".into(),
            message: "No text style".into(),
            fix: None,
            fix_label: String::new(),
        });
    }
}
