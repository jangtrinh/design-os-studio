//! Document-level `broken-style-binding` check: a style binds a property to a variable that no
//! longer exists. It has no owning node, so it is evaluated here instead of by the node traversal;
//! it is still a registered rule (`tokens` category, `agent` preset) and obeys rule selection.

use super::config::LintConfig;
use crate::Document;
use serde_json::{json, Value};

/// Whether the style-binding check runs for `config`: only when the registered
/// `broken-style-binding` rule is enabled by the active preset or rule list.
pub fn style_binding_check_enabled(config: &LintConfig) -> bool {
    config.is_enabled("broken-style-binding")
}

/// [`broken_style_binding_findings`] when [`style_binding_check_enabled`] holds for `config`.
pub fn broken_style_binding_findings_for(doc: &Document, config: &LintConfig) -> Vec<Value> {
    if style_binding_check_enabled(config) {
        broken_style_binding_findings(doc)
    } else {
        Vec::new()
    }
}

/// One finding per style property bound to a variable id missing from `doc.variables`.
pub fn broken_style_binding_findings(doc: &Document) -> Vec<Value> {
    let mut out = Vec::new();
    for (style_key, meta) in &doc.style_meta {
        for (prop, vid) in &meta.bindings {
            if !doc.variables.contains_key(vid) {
                out.push(json!({
                    "rule": "broken-style-binding",
                    "category": "tokens",
                    "severity": "warning",
                    "style_key": style_key,
                    "name": meta.name,
                    "style_name": meta.name,
                    "property": prop,
                    "variable_id": vid,
                    "message": format!("Style '{}' binds '{}' to missing variable '{}'", meta.name, prop, vid),
                    "fixable": false,
                }));
            }
        }
    }
    out
}
