//! Studio neutral dark tokens, copied by value from
//! `crates/designos-app/src/theme.rs` (hex parity). No violet anywhere
//! (Purple Ban). One accent only: the error red used for failed states.

pub const BG_PRIMARY: &str = "#0A0A0A"; // page background
pub const BG_SECONDARY: &str = "#171717"; // card / diagram body
pub const BG_TERTIARY: &str = "#262626"; // node fill
pub const BORDER: &str = "#262626";
pub const BORDER_STRONG: &str = "#404040"; // node outline
pub const TEXT_SECONDARY: &str = "#D4D4D4"; // labels
pub const TEXT_TERTIARY: &str = "#A3A3A3"; // edges, lines
pub const TEXT_PLACEHOLDER: &str = "#737373"; // sequence lifelines
pub const FONT: &str = "Inter";

/// `mermaid-rs-renderer` theme: struct fields are public, so the theme is
/// plain data and needs no CSS post-processing.
#[cfg(feature = "mmdr")]
pub fn mmdr_theme() -> mermaid_rs_renderer::Theme {
    let mut t = mermaid_rs_renderer::Theme::modern();
    t.font_family = FONT.into();
    t.font_size = 14.0;
    t.background = BG_SECONDARY.into();
    t.primary_color = BG_TERTIARY.into();
    t.primary_text_color = TEXT_SECONDARY.into();
    t.primary_border_color = BORDER_STRONG.into();
    t.secondary_color = BG_TERTIARY.into();
    t.tertiary_color = BG_TERTIARY.into();
    t.line_color = TEXT_TERTIARY.into();
    t.text_color = TEXT_SECONDARY.into();
    t.edge_label_background = BG_SECONDARY.into();
    t.cluster_background = BG_PRIMARY.into();
    t.cluster_border = BORDER.into();
    t.sequence_actor_fill = BG_TERTIARY.into();
    t.sequence_actor_border = BORDER_STRONG.into();
    t.sequence_actor_line = TEXT_PLACEHOLDER.into();
    t.sequence_note_fill = BG_PRIMARY.into();
    t.sequence_note_border = BORDER_STRONG.into();
    t.sequence_activation_fill = BORDER_STRONG.into();
    t.sequence_activation_border = TEXT_TERTIARY.into();
    t
}

/// Host CSS applied after merman's resvg-safe pass. Experiment: the autonumber
/// digit is drawn but invisible (same light grey as its disc), so pin its fill.
#[cfg(feature = "merman")]
pub const MERMAN_HOST_CSS: &str = ".sequenceNumber { fill: #0A0A0A !important; font-weight: 600; }\n";

/// `merman` takes Mermaid `themeVariables` (the same JSON a user could put in
/// an `%%{init}%%` directive), applied as host site config.
#[cfg(any(feature = "merman", feature = "merman06"))]
pub fn merman_site_config() -> serde_json::Value {
    serde_json::json!({
        "theme": "base",
        "fontFamily": FONT,
        // usvg has no foreignObject: ask for plain SVG <text> labels at the source.
        "htmlLabels": false,
        "flowchart": { "htmlLabels": false },
        "themeVariables": {
            "fontFamily": FONT,
            "background": BG_SECONDARY,
            "primaryColor": BG_TERTIARY,
            "primaryTextColor": TEXT_SECONDARY,
            "primaryBorderColor": BORDER_STRONG,
            "secondaryColor": BG_TERTIARY,
            "tertiaryColor": BG_TERTIARY,
            "textColor": TEXT_SECONDARY,
            "lineColor": TEXT_TERTIARY,
            "mainBkg": BG_TERTIARY,
            "nodeBorder": BORDER_STRONG,
            "nodeTextColor": TEXT_SECONDARY,
            "clusterBkg": BG_PRIMARY,
            "clusterBorder": BORDER,
            "edgeLabelBackground": BG_SECONDARY,
            "titleColor": TEXT_SECONDARY,
            "actorBkg": BG_TERTIARY,
            "actorBorder": BORDER_STRONG,
            "actorTextColor": TEXT_SECONDARY,
            "actorLineColor": TEXT_PLACEHOLDER,
            "signalColor": TEXT_TERTIARY,
            "signalTextColor": TEXT_SECONDARY,
            "labelBoxBkgColor": BG_TERTIARY,
            "labelBoxBorderColor": BORDER_STRONG,
            "labelTextColor": TEXT_SECONDARY,
            "loopTextColor": TEXT_SECONDARY,
            "noteBkgColor": BG_PRIMARY,
            "noteBorderColor": BORDER_STRONG,
            "noteTextColor": TEXT_SECONDARY,
            "activationBkgColor": BORDER_STRONG,
            "activationBorderColor": TEXT_TERTIARY,
            "sequenceNumberColor": BG_PRIMARY
        }
    })
}
