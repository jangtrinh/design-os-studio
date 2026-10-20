//! Document-level `broken-style-binding` check: a style binds a property to a variable that no
//! longer exists. It has no owning node, so it lives beside the node rules instead of in the registry.

use super::config::LintConfig;
use super::registry::{resolve_rule, Category};
use crate::Document;
use serde_json::{json, Value};

/// Whether the style-binding check runs for `config`: it belongs to the `tokens` category, so it
/// is active whenever the selection enables at least one token rule.
pub fn style_binding_check_enabled(config: &LintConfig) -> bool {
    config.enabled_rules.keys().any(|id| resolve_rule(id).is_some_and(|spec| spec.category == Category::Tokens))
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
