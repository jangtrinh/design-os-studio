//! Unified design lint rule registry, presets, categories, severities, and alias resolution.

use serde::{Deserialize, Serialize};

/// Category of a lint rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Layout,
    Text,
    Tokens,
    Components,
    Content,
    Naming,
    Accessibility,
    Structure,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Layout,
        Category::Text,
        Category::Tokens,
        Category::Components,
        Category::Content,
        Category::Naming,
        Category::Accessibility,
        Category::Structure,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Layout => "layout",
            Category::Text => "text",
            Category::Tokens => "tokens",
            Category::Components => "components",
            Category::Content => "content",
            Category::Naming => "naming",
            Category::Accessibility => "accessibility",
            Category::Structure => "structure",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "layout" => Some(Category::Layout),
            "text" => Some(Category::Text),
            "tokens" => Some(Category::Tokens),
            "components" => Some(Category::Components),
            "content" => Some(Category::Content),
            "naming" => Some(Category::Naming),
            "accessibility" | "a11y" => Some(Category::Accessibility),
            "structure" => Some(Category::Structure),
            _ => None,
        }
    }
}

/// Severity of a lint finding. Ordered `Error < Warning < Info` so sorting puts errors first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" | "err" => Some(Severity::Error),
            "warning" | "warn" => Some(Severity::Warning),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

/// Runtime capability a rule needs from [`super::LintEnv`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleNeeds {
    Geometry,
    Text,
    Pixels,
}

/// Named built-in lint preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintPreset {
    #[default]
    Agent,
    CheckDesigns,
    Recommended,
    Strict,
}

impl LintPreset {
    pub const ALL: [LintPreset; 4] = [LintPreset::Agent, LintPreset::CheckDesigns, LintPreset::Recommended, LintPreset::Strict];

    pub fn as_str(self) -> &'static str {
        match self {
            LintPreset::Agent => "agent",
            LintPreset::CheckDesigns => "check-designs",
            LintPreset::Recommended => "recommended",
            LintPreset::Strict => "strict",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "agent" => Some(LintPreset::Agent),
            "check-designs" | "check_designs" | "checkdesigns" => Some(LintPreset::CheckDesigns),
            "recommended" | "default" => Some(LintPreset::Recommended),
            "strict" | "all" => Some(LintPreset::Strict),
            _ => None,
        }
    }
}

/// Metadata for one registered lint rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleSpec {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub category: Category,
    pub default_severity: Severity,
    pub fixable: bool,
    pub needs: RuleNeeds,
    pub presets: &'static [LintPreset],
}

use Category::*;
use LintPreset::*;
use RuleNeeds::Geometry;
use Severity::*;

/// Canonical registry of the design lint rules (12 new geometric/text/content/component rules,
/// the 15 non-overlapping legacy rules and the document-level `broken-style-binding`, with legacy rule ids retained as aliases).
pub const RULES: &[RuleSpec] = &[
    // --- 12 new rules defined in E15-9 §3.2 ---
    RuleSpec {
        id: "overflow-parent",
        aliases: &["overflowing-children", "child-overflow"],
        category: Layout,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "clipped-partial",
        aliases: &["clipped", "clipped-content"],
        category: Layout,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "clipped-full",
        aliases: &["completely-clipped"],
        category: Layout,
        default_severity: Error,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "sibling-overlap",
        aliases: &["overlapping-siblings", "overlap"],
        category: Layout,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "text-truncated",
        aliases: &["text-overflow"],
        category: Text,
        default_severity: Error,
        fixable: true,
        needs: RuleNeeds::Text,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "text-unexpected-wrap",
        aliases: &["unexpected-wrap", "word-break"],
        category: Text,
        default_severity: Warning,
        fixable: true,
        needs: RuleNeeds::Text,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "collapsed-size",
        aliases: &["zero-bounds"],
        category: Layout,
        default_severity: Error,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "sizing-conflict",
        aliases: &["invalid-sizing"],
        category: Layout,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "text-missing-fill",
        aliases: &["invisible-text"],
        category: Text,
        default_severity: Error,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "absolute-in-auto-layout",
        aliases: &["absolute-child"],
        category: Layout,
        default_severity: Info,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "placeholder-text",
        aliases: &["lorem-ipsum"],
        category: Content,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "detached-instance",
        aliases: &["REQUIRE_NO_DETACHED_COMPONENTS", "detached-component"],
        category: Components,
        default_severity: Info,
        fixable: true,
        needs: Geometry,
        presets: &[CheckDesigns, Recommended, Strict],
    },
    // --- Existing rules moved into the unified registry ---
    RuleSpec {
        id: "default-layer-name",
        aliases: &["generic-layer-name"],
        category: Naming,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Recommended, Strict],
    },
    RuleSpec {
        id: "empty-container",
        aliases: &["empty-frame"],
        category: Structure,
        default_severity: Info,
        fixable: true,
        needs: Geometry,
        presets: &[Recommended, Strict],
    },
    RuleSpec {
        id: "deeply-nested",
        aliases: &["excessive-nesting"],
        category: Structure,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "non-4px-grid",
        aliases: &["off-grid", "non-integer-bounds", "subpixel-position"],
        category: Layout,
        default_severity: Info,
        fixable: true,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "off-scale",
        aliases: &[
            "inconsistent-spacing",
            "non-scale-font-size",
            "inconsistent-radius",
            "off-spacing-scale",
            "irregular-autolayout-spacing",
            "off-radius-scale",
            "off-font-scale",
            "raw-font-size",
        ],
        category: Tokens,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
    RuleSpec {
        id: "hardcoded-value",
        aliases: &[
            "hard-coded-value",
            "HARD_CODED_VALUES",
            "unbound-token",
            "hardcoded-token",
            "RECOMMEND_VARIABLES",
            "REQUIRE_FILL_COLOR_VARIABLES",
            "REQUIRE_CORNER_RADIUS_VARIABLES",
            "REQUIRE_PADDING_VARIABLES",
            "REQUIRE_HORIZONTAL_SPACING_VARIABLES",
            "REQUIRE_VERTICAL_SPACING_VARIABLES",
            "REQUIRE_TYPOGRAPHY_AND_STROKE_VARIABLES",
        ],
        category: Tokens,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Agent, CheckDesigns, Recommended, Strict],
    },
    RuleSpec {
        id: "unbalanced-padding",
        aliases: &["asymmetric-padding"],
        category: Layout,
        default_severity: Info,
        fixable: true,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "missing-auto-layout",
        aliases: &["no-auto-layout"],
        category: Layout,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "misaligned-siblings",
        aliases: &["off-alignment", "unaligned-siblings", "inconsistent-sibling-alignment"],
        category: Layout,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "small-touch-target",
        aliases: &["touch-target"],
        category: Accessibility,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "tiny-text",
        aliases: &["small-text"],
        category: Accessibility,
        default_severity: Warning,
        fixable: true,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "detached-text-style",
        aliases: &["REQUIRE_TEXT_STYLES", "missing-text-style"],
        category: Tokens,
        default_severity: Info,
        fixable: false,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "hardcoded-color",
        aliases: &["unbound-color", "color-token-compliance"],
        category: Tokens,
        default_severity: Info,
        fixable: false,
        needs: Geometry,
        presets: &[Strict],
    },
    RuleSpec {
        id: "contrast",
        aliases: &["low-contrast-text", "low-contrast"],
        category: Accessibility,
        default_severity: Error,
        fixable: false,
        needs: RuleNeeds::Pixels,
        presets: &[Agent, CheckDesigns, Recommended, Strict],
    },
    // --- document-level rule (E1-4 §8.1): no owning node, evaluated by `style_bindings.rs` ---
    RuleSpec {
        id: "broken-style-binding",
        aliases: &[],
        category: Tokens,
        default_severity: Warning,
        fixable: false,
        needs: Geometry,
        presets: &[Agent, Recommended, Strict],
    },
];

/// Looks up a rule by its canonical ID or any of its aliases (case-insensitive).
pub fn resolve_rule(id_or_alias: &str) -> Option<&'static RuleSpec> {
    let q = id_or_alias.trim();
    if q.is_empty() {
        return None;
    }
    let by_name = RULES_BY_NAME.get_or_init(|| {
        let mut names = std::collections::HashMap::new();
        // A name that two rules share belongs to the one listed first.
        for rule in RULES {
            for name in std::iter::once(&rule.id).chain(rule.aliases) {
                names.entry(name.to_ascii_lowercase()).or_insert(rule);
            }
        }
        names
    });
    if q.bytes().any(|b| b.is_ascii_uppercase()) {
        by_name.get(&q.to_ascii_lowercase()).copied()
    } else {
        by_name.get(q).copied()
    }
}

/// Every rule by its lowercase ID and alias, so that [`resolve_rule`] needs no scan of [`RULES`]
/// (it runs for each rule on every node of a lint pass).
static RULES_BY_NAME: std::sync::OnceLock<std::collections::HashMap<String, &'static RuleSpec>> = std::sync::OnceLock::new();

/// Resolves a rule ID or alias to its canonical registry ID.
pub fn canonical_rule_id(id_or_alias: &str) -> Option<&'static str> {
    resolve_rule(id_or_alias).map(|r| r.id)
}

/// Returns the aliases registered for the rule identified by `id_or_alias`.
pub fn rule_aliases(id_or_alias: &str) -> &'static [&'static str] {
    resolve_rule(id_or_alias).map(|r| r.aliases).unwrap_or(&[])
}

/// All canonical rule IDs in registration order.
pub fn all_rule_ids() -> Vec<&'static str> {
    RULES.iter().map(|r| r.id).collect()
}

/// All canonical rule IDs enabled by `preset`.
pub fn preset_rule_ids(preset: LintPreset) -> Vec<&'static str> {
    RULES.iter().filter(|r| r.presets.contains(&preset)).map(|r| r.id).collect()
}

/// All valid preset names (built-in presets + category presets).
pub fn all_preset_names() -> Vec<&'static str> {
    vec!["agent", "check-designs", "recommended", "strict", "layout", "text", "tokens", "components", "content", "naming", "accessibility", "structure"]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scan the lookup table replaced.
    fn resolve_by_scan(q: &str) -> Option<&'static RuleSpec> {
        let q = q.trim();
        if q.is_empty() {
            return None;
        }
        RULES.iter().find(|r| r.id.eq_ignore_ascii_case(q) || r.aliases.iter().any(|a| a.eq_ignore_ascii_case(q)))
    }

    #[test]
    fn resolve_rule_agrees_with_a_scan_of_the_registry_for_every_name_in_any_case() {
        let mut names = vec![String::new(), "  ".into(), "no-such-rule".into(), " contrast ".into()];
        for rule in RULES {
            for name in std::iter::once(&rule.id).chain(rule.aliases) {
                names.push((*name).to_string());
                names.push(name.to_ascii_uppercase());
                names.push(name.to_ascii_lowercase());
                names.push(format!(" {name}\t"));
            }
        }
        for name in &names {
            assert_eq!(resolve_rule(name).map(|r| r.id), resolve_by_scan(name).map(|r| r.id), "{name:?}");
        }
        assert_eq!(resolve_rule("LOW-Contrast").map(|r| r.id), Some("contrast"));
        assert!(RULES.iter().all(|r| resolve_rule(r.id).is_some_and(|got| got.id == r.id)), "an ID resolves to its own rule");
    }
}
