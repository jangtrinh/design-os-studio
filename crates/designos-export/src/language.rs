//! Shared code-generation target language enum (`T7.3` / `T7.29`).
//!
//! [`CodeLanguage`] is the single source of truth for codegen targets shared across
//! `designos-export`, the Dev Mode inspect panel (`designos-app`), the headless CLI
//! (`--dev` / `--codegen`), and the MCP `generate_code` tool (`designos-mcp`).

use crate::{android_xml_for, compose_for, css_for, html_for, react_with, swiftui_for, tailwind_for, uikit_for, ReactOptions};
use designos_doc::{Document, NodeId};

/// Options passed to [`CodeLanguage::generate`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CodeLanguageOptions {
    /// Switches React output from inline styles (`false`) to Tailwind classes (`true`).
    pub react_tailwind: bool,
}

impl From<bool> for CodeLanguageOptions {
    fn from(react_tailwind: bool) -> Self {
        Self { react_tailwind }
    }
}

impl From<crate::ReactOptions> for CodeLanguageOptions {
    fn from(opts: crate::ReactOptions) -> Self {
        Self { react_tailwind: opts.tailwind }
    }
}

impl From<&crate::ReactOptions> for CodeLanguageOptions {
    fn from(opts: &crate::ReactOptions) -> Self {
        Self { react_tailwind: opts.tailwind }
    }
}

/// Target language for code generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeLanguage {
    #[default]
    Css,
    Tailwind,
    SwiftUi,
    UiKit,
    Compose,
    AndroidXml,
    React,
    Html,
}

impl CodeLanguage {
    /// Every supported code-generation language in canonical UI / schema order.
    pub const ALL: [Self; 8] = [Self::Css, Self::Tailwind, Self::SwiftUi, Self::UiKit, Self::Compose, Self::AndroidXml, Self::React, Self::Html];

    /// Stable machine identifier used by CLI flags and the MCP `generate_code` schema.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Css => "css",
            Self::Tailwind => "tailwind",
            Self::SwiftUi => "swiftui",
            Self::UiKit => "uikit",
            Self::Compose => "compose",
            Self::AndroidXml => "android-xml",
            Self::React => "react",
            Self::Html => "html",
        }
    }

    /// Human-readable label shown in the Dev Mode language dropdown.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Css => "CSS",
            Self::Tailwind => "Tailwind",
            Self::SwiftUi => "SwiftUI",
            Self::UiKit => "UIKit",
            Self::Compose => "Compose",
            Self::AndroidXml => "Android XML",
            Self::React => "React",
            Self::Html => "HTML",
        }
    }

    /// Parses a machine identifier (case-insensitive) into a [`CodeLanguage`].
    pub fn from_id(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|lang| lang.id() == lower)
    }

    /// Generates code for `node` in `doc` using this target language.
    pub fn generate(self, doc: &Document, node: NodeId, opts: impl Into<CodeLanguageOptions>) -> String {
        let opts = opts.into();
        match self {
            Self::Css => css_for(doc, node),
            Self::Tailwind => tailwind_for(doc, node),
            Self::SwiftUi => swiftui_for(doc, node),
            Self::UiKit => uikit_for(doc, node),
            Self::Compose => compose_for(doc, node),
            Self::AndroidXml => android_xml_for(doc, node),
            Self::React => react_with(doc, node, &ReactOptions { tailwind: opts.react_tailwind }),
            Self::Html => html_for(doc, node),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_ids_round_trip_in_dropdown_order() {
        assert_eq!(CodeLanguage::ALL.map(CodeLanguage::id), ["css", "tailwind", "swiftui", "uikit", "compose", "android-xml", "react", "html"]);
        for lang in CodeLanguage::ALL {
            assert_eq!(CodeLanguage::from_id(lang.id()), Some(lang));
        }
        assert_eq!(CodeLanguage::from_id("svg"), None);
    }
}
