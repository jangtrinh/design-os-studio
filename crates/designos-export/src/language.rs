//! Shared Dev Mode and MCP code-generation language list.

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

/// Code languages of the Inspect panel and MCP `designos_generate_code`.
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
}

impl CodeLanguage {
    /// Dropdown order: Figma's groups (CSS, iOS: SwiftUI + UIKit, Android: Compose + XML), then the web frameworks.
    pub const ALL: [CodeLanguage; 7] = [
        CodeLanguage::Css,
        CodeLanguage::Tailwind,
        CodeLanguage::SwiftUi,
        CodeLanguage::UiKit,
        CodeLanguage::Compose,
        CodeLanguage::AndroidXml,
        CodeLanguage::React,
    ];

    /// Human-readable UI label for the Dev Mode dropdown.
    pub fn label(self) -> &'static str {
        match self {
            Self::Css => "CSS",
            Self::Tailwind => "Tailwind",
            Self::SwiftUi => "SwiftUI",
            Self::UiKit => "UIKit",
            Self::Compose => "Compose",
            Self::AndroidXml => "Android XML",
            Self::React => "React",
        }
    }

    /// Canonical machine identifier used by MCP tools (`css`, `tailwind`, `swiftui`, `uikit`, `compose`, `android-xml`, `react`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Css => "css",
            Self::Tailwind => "tailwind",
            Self::SwiftUi => "swiftui",
            Self::UiKit => "uikit",
            Self::Compose => "compose",
            Self::AndroidXml => "android-xml",
            Self::React => "react",
        }
    }

    /// Resolve a language from its MCP/CLI identifier.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.id() == id)
    }

    /// Code for `node`. `opts` may be a `bool` (`react_tailwind`) or [`CodeLanguageOptions`].
    pub fn generate(self, doc: &Document, node: NodeId, opts: impl Into<CodeLanguageOptions>) -> String {
        let opts = opts.into();
        match self {
            Self::Css => crate::css_for(doc, node),
            Self::Tailwind => crate::tailwind_for(doc, node),
            Self::SwiftUi => crate::swiftui_for(doc, node),
            Self::UiKit => crate::uikit_for(doc, node),
            Self::Compose => crate::compose_for(doc, node),
            Self::AndroidXml => crate::android_xml_for(doc, node),
            Self::React => crate::react_with(doc, node, &crate::ReactOptions { tailwind: opts.react_tailwind }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_ids_round_trip_in_dropdown_order() {
        assert_eq!(CodeLanguage::ALL.map(CodeLanguage::id), ["css", "tailwind", "swiftui", "uikit", "compose", "android-xml", "react"]);
        for lang in CodeLanguage::ALL {
            assert_eq!(CodeLanguage::from_id(lang.id()), Some(lang));
        }
        assert_eq!(CodeLanguage::from_id("svg"), None);
    }
}
