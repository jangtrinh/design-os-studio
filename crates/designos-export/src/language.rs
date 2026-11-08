//! Shared code-generation target language enum (`T7.3` / `T7.29`).
//!
//! [`CodeLanguage`] is the single source of truth for codegen targets shared across
//! `designos-export`, the Dev Mode inspect panel (`designos-app`), the headless CLI
//! (`--dev` / `--codegen`), and the MCP `generate_code` tool (`designos-mcp`).

use crate::{
    android_xml_for, compose_for, css_for_with, html::html_with_format, react::react_with_format, swiftui_for, tailwind_for_with, uikit_for, ColorFormat,
    HtmlOptions, ReactOptions,
};
use designos_doc::{Document, NodeId};

/// Options passed to [`CodeLanguage::generate`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CodeLanguageOptions {
    /// Switches React output from inline styles (`false`) to Tailwind classes (`true`).
    pub react_tailwind: bool,
    /// Colour notation for CSS, Tailwind, React, and HTML output.
    pub color_format: ColorFormat,
}

impl From<bool> for CodeLanguageOptions {
    fn from(react_tailwind: bool) -> Self {
        Self { react_tailwind, color_format: ColorFormat::default() }
    }
}

impl From<ColorFormat> for CodeLanguageOptions {
    fn from(color_format: ColorFormat) -> Self {
        Self { react_tailwind: false, color_format }
    }
}

impl From<(bool, ColorFormat)> for CodeLanguageOptions {
    fn from((react_tailwind, color_format): (bool, ColorFormat)) -> Self {
        Self { react_tailwind, color_format }
    }
}

impl From<crate::ReactOptions> for CodeLanguageOptions {
    fn from(opts: crate::ReactOptions) -> Self {
        Self { react_tailwind: opts.tailwind, color_format: ColorFormat::default() }
    }
}

impl From<&crate::ReactOptions> for CodeLanguageOptions {
    fn from(opts: &crate::ReactOptions) -> Self {
        Self { react_tailwind: opts.tailwind, color_format: ColorFormat::default() }
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
            Self::Css => css_for_with(doc, node, opts.color_format),
            Self::Tailwind => tailwind_for_with(doc, node, opts.color_format),
            Self::SwiftUi => swiftui_for(doc, node),
            Self::UiKit => uikit_for(doc, node),
            Self::Compose => compose_for(doc, node),
            Self::AndroidXml => android_xml_for(doc, node),
            Self::React => react_with_format(doc, node, &ReactOptions { tailwind: opts.react_tailwind }, opts.color_format),
            Self::Html => html_with_format(doc, node, &HtmlOptions { fragment: false }, opts.color_format),
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
