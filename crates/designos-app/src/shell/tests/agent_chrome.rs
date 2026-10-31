//! Agents panel chrome: palette aliases, card sizes, no shadows, no literal colours, and the painted look of its cards.

use super::agent_simplified_panel::{finish, frames, new_chat_sim, recorded, session_sim};
use super::chrome_census::{chrome_colour_sites, clean_rust};
use super::*;
use crate::terminal::AgentCliPreset;

#[test]
fn terminal_palette_aliases_contract_roles() {
    use crate::terminal::{
        STATUS_BAD_BG, STATUS_BAD_BORDER, STATUS_BAD_TEXT, STATUS_INFO_BG, STATUS_INFO_BORDER, STATUS_INFO_TEXT, STATUS_OK_BG, STATUS_OK_BORDER,
        STATUS_OK_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT, TERM_BG_BAR, TERM_BG_ELEVATED, TERM_BG_HOVER, TERM_BG_VIEWPORT, TERM_BORDER,
        TERM_ORANGE,
    };
    assert_eq!(TERM_BG_VIEWPORT, crate::theme::BG);
    assert_eq!(TERM_BG_BAR, crate::theme::BG_PRIMARY);
    assert_eq!(TERM_BG_ELEVATED, crate::theme::BG_SECONDARY);
    assert_eq!(TERM_BG_HOVER, crate::theme::BG_HOVER);
    assert_eq!(TERM_BORDER, crate::theme::BORDER);
    assert_eq!(TERM_ORANGE, crate::theme::BORDER_SELECTED);
    assert_eq!(STATUS_INFO_TEXT, crate::theme::STATUS_INFO_TEXT);
    assert_eq!(STATUS_INFO_BG, crate::theme::STATUS_INFO_BG);
    assert_eq!(STATUS_INFO_BORDER, crate::theme::STATUS_INFO_BORDER);
    assert_eq!(STATUS_OK_BG, crate::theme::BG_SUCCESS_PRIMARY);
    assert_eq!(STATUS_OK_BORDER, crate::theme::BADGE_SUCCESS_BORDER);
    assert_eq!(STATUS_OK_TEXT, crate::theme::BADGE_SUCCESS_TEXT);
    assert_eq!(STATUS_WARN_BG, crate::theme::BG_WARNING_PRIMARY);
    assert_eq!(STATUS_WARN_BORDER, crate::theme::BADGE_WARNING_BORDER);
    assert_eq!(STATUS_WARN_TEXT, crate::theme::BADGE_WARNING_TEXT);
    assert_eq!(STATUS_BAD_BG, crate::theme::BG_ERROR_PRIMARY);
    assert_eq!(STATUS_BAD_BORDER, crate::theme::BADGE_ERROR_BORDER);
    assert_eq!(STATUS_BAD_TEXT, crate::theme::BADGE_ERROR_TEXT);
}

#[test]
fn terminal_content_colours_unchanged() {
    assert_eq!(crate::terminal::TERM_FG_DEFAULT, egui::Color32::from_rgb(0xE4, 0xE4, 0xE7));
    assert_eq!(crate::terminal::TERM_SELECTION_BG, egui::Color32::from_rgb(0x5A, 0x78, 0x98));
}

#[test]
fn approval_card_height_follows_control_xs() {
    use crate::terminal::AgentCliPreset;
    assert_eq!(crate::agent_approval_card::approval_card_outer_height(AgentCliPreset::Claude), 136.0);
    for preset in [AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Gemini, AgentCliPreset::Antigravity, AgentCliPreset::Shell] {
        assert_eq!(crate::agent_approval_card::approval_card_outer_height(preset), 96.0, "{preset:?}");
    }
}

fn allowlisted_terminal_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains("TERM_FG_DEFAULT")
        || trimmed.contains("TERM_SELECTION_BG")
        || trimmed.contains("0x5C, 0x4A, 0x00")
        || trimmed.contains("0x1D, 0x1F, 0x21")
        || trimmed.contains("0xA1, 0xA1, 0xAA")
        || (trimmed.contains("Color32::WHITE") && (trimmed.contains("cursor") || trimmed.contains("CURSOR") || trimmed.contains("caret")))
}

const AGENT_SOURCES: &[(&str, &str)] = &[
    ("terminal.rs", include_str!("../../terminal.rs")),
    ("agent_approval_card.rs", include_str!("../../agent_approval_card.rs")),
    ("agent_composer.rs", include_str!("../../agent_composer.rs")),
    ("agent_composer_buttons.rs", include_str!("../../agent_composer_buttons.rs")),
    ("agent_composer_chip_layout.rs", include_str!("../../agent_composer_chip_layout.rs")),
    ("agent_composer_chips.rs", include_str!("../../agent_composer_chips.rs")),
    ("agent_composer_menus.rs", include_str!("../../agent_composer_menus.rs")),
    ("agent_composer_text.rs", include_str!("../../agent_composer_text.rs")),
    ("agent_composer_toolbar.rs", include_str!("../../agent_composer_toolbar.rs")),
    ("agent_header.rs", include_str!("../../agent_header.rs")),
    ("agent_header_tabs.rs", include_str!("../../agent_header_tabs.rs")),
    ("agent_mention_popup.rs", include_str!("../../agent_mention_popup.rs")),
    ("agent_open_flow.rs", include_str!("../../agent_open_flow.rs")),
    ("agent_picker.rs", include_str!("../../agent_picker.rs")),
    ("agent_selection_chip.rs", include_str!("../../agent_selection_chip.rs")),
    ("agent_session_failure.rs", include_str!("../../agent_session_failure.rs")),
    ("agent_start_failure.rs", include_str!("../../agent_start_failure.rs")),
    ("agent_stop.rs", include_str!("../../agent_stop.rs")),
    ("agent_turn_card.rs", include_str!("../../agent_turn_card.rs")),
    ("agent_presence.rs", include_str!("../../agent_presence.rs")),
    ("agent_busy_frames.rs", include_str!("../../agent_busy_frames.rs")),
    ("canvas/agent_banner.rs", include_str!("../../canvas/agent_banner.rs")),
    ("canvas/agent_changes_chip.rs", include_str!("../../canvas/agent_changes_chip.rs")),
    ("canvas/agent_overlay.rs", include_str!("../../canvas/agent_overlay.rs")),
    ("skills/mod.rs", include_str!("../../skills/mod.rs")),
    ("skills/picker_popover.rs", include_str!("../../skills/picker_popover.rs")),
    ("skills/pills.rs", include_str!("../../skills/pills.rs")),
    ("image_jobs.rs", include_str!("../../image_jobs.rs")),
];

#[test]
fn agent_sources_paint_no_chrome_colour_literals() {
    let files = AGENT_SOURCES;
    let mut left = Vec::new();
    for (rel, src) in files {
        let cleaned = super::chrome_census::clean_rust(src);
        let lines: Vec<&str> = cleaned.lines().collect();
        for site in chrome_colour_sites(src) {
            let line = lines.get(site - 1).copied().unwrap_or("");
            if *rel == "terminal.rs" && allowlisted_terminal_line(line) {
                continue;
            }
            left.push(format!("{rel}:{site}:{line}"));
        }
    }
    assert!(left.is_empty(), "chrome colour sites remain:\n{}", left.join("\n"));
}

/// Every shape of a frame, groups flattened.
fn flat(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::Shape> {
    fn walk<'a>(shape: &'a egui::epaint::Shape, out: &mut Vec<&'a egui::epaint::Shape>) {
        match shape {
            egui::epaint::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
            other => out.push(other),
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

/// The filled rects that cover `rect` exactly: what a button paints as its face.
fn faces(shapes: &[egui::epaint::ClippedShape], rect: egui::Rect) -> Vec<&egui::epaint::RectShape> {
    flat(shapes)
        .into_iter()
        .filter_map(|shape| match shape {
            egui::epaint::Shape::Rect(r)
                if (r.rect.min - rect.min).length() < 0.6 && (r.rect.max - rect.max).length() < 0.6 && r.fill != egui::Color32::TRANSPARENT =>
            {
                Some(r)
            }
            _ => None,
        })
        .collect()
}

/// A Grok tab that waits for approval in its own terminal, with the panel showing the tab (not New chat).
fn grok_approval_sim() -> Sim {
    let mut s = new_chat_sim();
    s.shell.app.terminal.open_approval_fixture("agents-approval-open-tab", None, crate::agent_permissions::AgentPermissionMode::Ask);
    frames(&mut s, 3);
    s
}

#[test]
fn approval_card_native_cli_buttons_are_control_xs() {
    use crate::widgets::kit::{button_look, Hierarchy, State};
    let mut s = grok_approval_sim();
    let secondary = button_look(Hierarchy::Secondary, State::Default);
    assert!(!s.shell.app.terminal.in_new_chat(), "the panel shows the waiting tab");
    let show = recorded(&s, "agent-approval-show-prompt").expect("the active terminal's card reads Show prompt");
    assert_eq!(show.height(), crate::theme::CONTROL_XS, "Show prompt is Button xs");
    assert!(faces(&s.shapes, show).iter().any(|f| f.fill == secondary.fill), "and wears the Secondary look: {:?}", faces(&s.shapes, show));

    // Keyboard focus draws the kit ring, not a selection stroke.
    s.ctx.memory_mut(|m| m.request_focus(egui::Id::new(("agents-left-pty", "agent-approval-show-prompt"))));
    frames(&mut s, 2);
    let ring = flat(&s.shapes).into_iter().any(|shape| {
        matches!(shape, egui::epaint::Shape::Rect(r)
            if r.stroke.color == crate::theme::FOCUS_RING && r.stroke.width == 2.0 && r.rect.expand(1.0).contains_rect(show.expand(4.0)) && r.rect.contains_rect(show))
    });
    assert!(ring, "a focused card button draws FOCUS_RING");
    let selection_stroke = flat(&s.shapes).into_iter().any(|shape| {
        matches!(shape, egui::epaint::Shape::Rect(r) if r.stroke.color == crate::theme::BORDER_SELECTED && r.rect.contains_rect(show) && r.rect.width() < show.width() + 12.0)
    });
    assert!(!selection_stroke, "the old BORDER_SELECTED focus stroke is gone");

    // New chat covers the tab: the card points at it with Open tab.
    s.ctx.memory_mut(|m| m.surrender_focus(egui::Id::new(("agents-left-pty", "agent-approval-show-prompt"))));
    let plus = recorded(&s, "agents-new-chat-btn").expect("the new chat button");
    s.click(plus.center());
    s.ctx.data_mut(|d| d.remove_by_type::<egui::Rect>());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.in_new_chat());
    let open_tab = recorded(&s, "agent-approval-open-tab").expect("another view of a waiting tab reads Open tab");
    assert_eq!(open_tab.height(), crate::theme::CONTROL_XS, "Open tab is Button xs");
    assert!(faces(&s.shapes, open_tab).iter().any(|f| f.fill == secondary.fill), "and wears the Secondary look: {:?}", faces(&s.shapes, open_tab));
    finish(s);
}

#[test]
fn terminal_scrollbar_keeps_theme_roles() {
    let mut s = new_chat_sim();
    s.shell.app.terminal.open_approval_fixture("agents-scrollback", None, crate::agent_permissions::AgentPermissionMode::Ask);
    frames(&mut s, 4);
    let track = recorded(&s, "term-scrollbar-track").expect("the scrollbar of a tab with scrollback");
    let thumb = recorded(&s, "term-scrollbar-thumb").expect("and its thumb");
    assert_eq!(track.width(), crate::theme::TARGET_MIN, "the track is the 24 px pointer target");
    assert_eq!(thumb.width(), crate::theme::SPACE_FACT, "the thumb is 4 px wide");
    let thumb_fill = faces(&s.shapes, thumb).first().map(|f| f.fill);
    assert!(
        thumb_fill == Some(crate::theme::TEXT_TERTIARY) || thumb_fill == Some(crate::theme::TEXT_SECONDARY),
        "the thumb takes a text role of the theme: {thumb_fill:?}"
    );
    // The code that paints it reads theme roles only.
    let src = include_str!("../../terminal.rs");
    let start = src.find("// Interactive panel-aligned scrollbar").expect("the scrollbar block");
    let end = start + src[start..].find("if scrollback_offset > 0 {").expect("the end of the scrollbar block");
    let block = &src[start..end];
    assert!(block.contains("theme::TEXT_TERTIARY") && block.contains("theme::TEXT_SECONDARY"), "the thumb colours are theme roles");
    assert!(!block.contains("Color32::") && !block.contains("from_rgb"), "and no literal colour is painted: {block}");
    finish(s);
}

#[test]
fn agent_surfaces_draw_no_shadow() {
    for (rel, src) in AGENT_SOURCES {
        let cleaned = clean_rust(src);
        for banned in ["elevation_shadow(", "elevation_400(", "menu_panel("] {
            assert!(!cleaned.contains(banned), "{rel} calls {banned}: the agent surfaces draw no shadow");
        }
    }
    let cleaned_of = |name: &str| clean_rust(AGENT_SOURCES.iter().find(|(rel, _)| *rel == name).expect("a listed source").1);
    assert!(cleaned_of("agent_approval_card.rs").contains("modal_surface("), "the permissions modal is the kit's dialog surface");
    let jobs = cleaned_of("image_jobs.rs");
    assert_eq!(jobs.matches("popover_surface(").count(), 2, "the Expand and Erase popovers are the kit's popover surface");
    assert!(cleaned_of("agent_mention_popup.rs").contains("popover_surface("), "and so is the mention popup");
}

#[test]
fn provider_marks_read_theme_constants() {
    let src = AGENT_SOURCES.iter().find(|(rel, _)| *rel == "agent_open_flow.rs").expect("the open flow").1;
    let cleaned = clean_rust(src);
    let start = cleaned.find("fn paint_session_row_indicators").expect("the session row painter");
    let end = start + cleaned[start..].find("\n}\n").expect("the end of that function");
    let body = &cleaned[start..end];
    for provider in ["PROVIDER_CLAUDE", "PROVIDER_CODEX", "PROVIDER_GROK", "PROVIDER_GEMINI"] {
        assert!(body.contains(provider), "{provider} colours a provider mark: {body}");
    }
    assert!(!body.contains("Color32::from_rgb") && !body.contains("Color32::from_"), "no provider colour is a literal in the open flow");
    for rel in ["agent_header.rs", "agent_header_tabs.rs"] {
        let tab_painter = clean_rust(AGENT_SOURCES.iter().find(|(name, _)| *name == rel).expect("a header file").1);
        assert!(!tab_painter.contains("PROVIDER_") && !tab_painter.contains("Color32::from_rgb"), "{rel} paints no provider mark");
    }
}

#[test]
fn brand_buttons_use_dark_label() {
    use crate::widgets::kit::{button_look, Hierarchy, State};
    assert_eq!(button_look(Hierarchy::Primary, State::Default).text, crate::theme::TEXT_ON_BRAND, "a filled brand button reads dark on the brand fill");
    let mut white = Vec::new();
    for (rel, src) in AGENT_SOURCES {
        let cleaned = clean_rust(src);
        for (n, line) in cleaned.lines().enumerate() {
            let cursor_caret = *rel == "terminal.rs" && (line.contains("cursor") || line.contains("CURSOR") || line.contains("caret"));
            if line.contains("Color32::WHITE") && !cursor_caret {
                white.push(format!("{rel}:{}", n + 1));
            }
        }
    }
    assert!(white.is_empty(), "no agent file paints Color32::WHITE outside the terminal cursor: {white:?}");
}

#[test]
fn legacy_chat_panel_uses_contract_roles() {
    use crate::theme;
    let mut s = session_sim();
    s.shell.app.terminal.side_panel_pty_open = false;
    frames(&mut s, 4);
    let panel = recorded(&s, "agents-terminal-rect").expect("the Agents panel area");
    use crate::widgets::kit::{button_look, Hierarchy, State};
    let roles = [
        // Send is a kit button: its faces are the Primary looks.
        button_look(Hierarchy::Primary, State::Default).fill,
        button_look(Hierarchy::Primary, State::Disabled).fill,
        theme::BG,
        theme::BG_PRIMARY,
        theme::BG_SECONDARY,
        theme::BG_TERTIARY,
        theme::BG_SELECTED,
        theme::BG_HOVER,
        theme::BG_PRESSED,
        theme::BG_MENU,
        theme::BG_MENU_HOVER,
        theme::BG_BRAND,
        theme::BORDER,
        theme::BORDER_STRONG,
        theme::STATUS_INFO_BG,
        theme::BG_WARNING_PRIMARY,
    ];
    let fills: Vec<egui::Color32> = flat(&s.shapes)
        .into_iter()
        .filter_map(|shape| match shape {
            egui::epaint::Shape::Rect(r) if panel.contains_rect(r.rect) && r.fill != egui::Color32::TRANSPARENT => Some(r.fill),
            _ => None,
        })
        .collect();
    assert!(!fills.is_empty(), "the classic chat painted something");
    let strays: Vec<String> = fills.iter().filter(|f| !roles.contains(f)).map(|f| format!("{f:?}")).collect();
    assert!(strays.is_empty(), "every fill of the classic chat is a theme role: {strays:?}");
    assert!(fills.contains(&theme::BG), "the panel is the page colour, not the old #2C2C2C");
    assert!(!fills.contains(&egui::Color32::from_rgb(0x2C, 0x2C, 0x2C)));
    // The census over just this region of `terminal.rs`.
    let src = include_str!("../../terminal.rs");
    let start_line = src[..src.find("let header_h = 48.0;").expect("the classic chat header")].lines().count() + 1;
    let end_line = src[..src.find("// ---- C. Center Area").expect("the classic chat body")].lines().count() + 1;
    assert!(end_line > start_line + 50, "the region is the classic chat header and runtime bar: {start_line}..{end_line}");
    let inside: Vec<usize> = chrome_colour_sites(src).into_iter().filter(|line| (start_line..=end_line).contains(line)).collect();
    assert!(inside.is_empty(), "the classic chat region holds no chrome colour literal: {inside:?}");
    finish(s);
}

fn request(id: &str, tool: &str) -> crate::agent_permissions::ApprovalRequest {
    crate::agent_permissions::ApprovalRequest { id: id.into(), tool_name: tool.into(), input_summary: "cargo test".into(), timestamp: 1, target: String::new() }
}

/// Paints the Claude approval card for `requests` over a panel and returns what it painted and its recorded rects.
fn claude_card(requests: &[crate::agent_permissions::ApprovalRequest]) -> (Vec<String>, egui::Context) {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx);
    let mut texts = Vec::new();
    for _ in 0..2 {
        let out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0))), ..Default::default() },
            |ui| {
                let panel = egui::Rect::from_min_size(egui::pos2(48.0, 0.0), egui::vec2(420.0, 800.0));
                let _ = crate::agent_approval_card::paint_approval_card(
                    ui,
                    panel,
                    AgentCliPreset::Claude,
                    requests,
                    "test-card",
                    crate::agent_permissions::AgentPermissionMode::Ask,
                );
            },
        );
        texts = crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect();
    }
    (texts, ctx)
}

#[test]
fn approval_card_has_no_badge_and_names_the_verb() {
    for (tool, title) in [("Bash", "Run a command"), ("Read", "Read a file"), ("Write", "Change a file"), ("designos_apply_spec", "Use designos_apply_spec")] {
        let (texts, _) = claude_card(&[request("1", tool)]);
        assert!(texts.iter().any(|t| t == title), "a {tool} request reads {title:?}: {texts:?}");
        assert!(!texts.iter().any(|t| t.to_uppercase().contains("CLAUDE")), "no agent badge is painted: {texts:?}");
        assert!(!texts.iter().any(|t| t.contains("wants to run")), "and no \"<agent> wants to run\" title: {texts:?}");
    }
    let (texts, ctx) = claude_card(&[request("1", "Bash"), request("2", "Edit"), request("3", "Write")]);
    assert!(texts.iter().any(|t| t == "1 of 3"), "three pending read 1 of 3: {texts:?}");
    let rect = |name: &str| {
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("nothing recorded for {name}"))
    };
    assert_eq!(rect("agent-approval-approve").height(), crate::theme::CONTROL_XS, "Approve is Button xs");
    assert_eq!(rect("agent-approval-deny").height(), crate::theme::CONTROL_XS, "Deny is Button xs");
    assert!(rect("agent-approval-deny").right() < rect("agent-approval-approve").left(), "Deny sits left of Approve");
}
