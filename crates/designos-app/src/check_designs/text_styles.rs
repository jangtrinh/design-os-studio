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
    match index.matching_text_styles(t).as_slice() {
        [] => {
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
        [s] => {
            out.push(Finding {
                node: n.id,
                node_name: n.name.clone(),
                rule: Rule::TextStyles,
                property: "text".into(),
                message: format!("Text matches style {}", hardcoded::fmt_style_quoted(s)),
                fix: Some(Fix::ApplyStyle { style: s.fix_key.clone() }),
                fix_label: hardcoded::fmt_style_apply_label(s),
            });
        }
        many => {
            out.push(Finding {
                node: n.id,
                node_name: n.name.clone(),
                rule: Rule::TextStyles,
                property: "text".into(),
                message: format!("Text matches {} styles: {}", many.len(), hardcoded::style_name_list(many)),
                fix: None,
                fix_label: String::new(),
            });
        }
    }
}
