//! Floating surfaces: command bar, dialogs, menus, Find, comments, presentation, updater.

use super::*;
use crate::util::world_aabb;

fn sources() -> Vec<(&'static str, &'static str)> {
    vec![
        ("cmdk.rs", include_str!("../../cmdk.rs")),
        ("updater.rs", include_str!("../../updater.rs")),
        ("present.rs", include_str!("../../present.rs")),
        ("panels/context.rs", include_str!("../../panels/context.rs")),
        ("home/dialog.rs", include_str!("../../home/dialog.rs")),
        ("history/modal.rs", include_str!("../../history/modal.rs")),
        ("history/panel.rs", include_str!("../../history/panel.rs")),
        ("history/banner.rs", include_str!("../../history/banner.rs")),
        ("history/mod.rs", include_str!("../../history/mod.rs")),
        ("history/session.rs", include_str!("../../history/session.rs")),
        ("history/preset.rs", include_str!("../../history/preset.rs")),
        ("find/mod.rs", include_str!("../../find/mod.rs")),
        ("find/panel.rs", include_str!("../../find/panel.rs")),
        ("find/select.rs", include_str!("../../find/select.rs")),
        ("comments/mod.rs", include_str!("../../comments/mod.rs")),
        ("comments/panel.rs", include_str!("../../comments/panel.rs")),
        ("comments/thread.rs", include_str!("../../comments/thread.rs")),
        ("comments/pins.rs", include_str!("../../comments/pins.rs")),
        ("comments/ui.rs", include_str!("../../comments/ui.rs")),
        ("comments/settings.rs", include_str!("../../comments/settings.rs")),
        ("comments/model.rs", include_str!("../../comments/model.rs")),
        ("comments/import.rs", include_str!("../../comments/import.rs")),
        ("comments/time.rs", include_str!("../../comments/time.rs")),
    ]
}

fn present_content_line(line: &str) -> bool {
    super::chrome_census::texture_tint_line(line)
        || (line.contains("Color32::WHITE") && line.contains("gamma_multiply"))
        || (line.contains("Color32::BLACK") && line.contains("PRESENT_BEZEL"))
}

fn present_chrome_sites() -> Vec<usize> {
    let src = include_str!("../../present.rs");
    let clean = super::chrome_census::clean_rust(src);
    let lines: Vec<&str> = clean.split('\n').collect();
    super::chrome_census::chrome_colour_sites(src).into_iter().filter(|n| !present_content_line(lines.get(n - 1).copied().unwrap_or(""))).collect()
}

#[test]
fn overlay_sources_paint_no_chrome_colour_literals() {
    let mut bad = Vec::new();
    for (name, src) in sources() {
        let sites = if name == "present.rs" { present_chrome_sites() } else { super::chrome_census::chrome_colour_sites(src) };
        if !sites.is_empty() {
            bad.push(format!("{name} {sites:?}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn overlay_surfaces_draw_no_shadow() {
    for (name, src) in sources() {
        let clean = super::chrome_census::clean_rust(src);
        for call in ["elevation_shadow(", "elevation_400(", "menu_panel("] {
            assert!(!clean.contains(call), "{name} still calls {call}");
        }
    }
}

#[test]
fn cmdk_palette_uses_contract_roles() {
    let mut s = Sim::new();
    s.shell.app.cmdk.open = true;
    s.frame(vec![]);
    let a = s.rect_of("cmdk-open-terminal");
    let b = s.rect_of("cmdk-populate-content");
    let src = include_str!("../../cmdk.rs");
    let pitch = b.min.y - a.min.y;
    assert!(
        !src.contains("CMDK_") && (a.height() - theme::CONTROL_XS).abs() < 0.1 && (pitch - 36.0).abs() < 0.1,
        "palette_const={} row_h={} pitch={pitch}",
        src.contains("CMDK_"),
        a.height()
    );
}

#[test]
fn dialog_footer_buttons_are_control_md() {
    let mut s = Sim::new();
    s.shell.app.confirm_open_github();
    s.frame(vec![]);
    let ok = s.rect_of("dialog-ok");
    let cancel = s.rect_of("dialog-cancel");
    let src = include_str!("../../home/dialog.rs");
    assert!(
        (ok.height() - theme::CONTROL_MD).abs() < 0.1 && (cancel.height() - theme::CONTROL_MD).abs() < 0.1 && src.contains("theme::INSET_MODAL"),
        "ok_h={} cancel_h={} inset={}",
        ok.height(),
        cancel.height(),
        src.contains("theme::INSET_MODAL")
    );
}

#[test]
fn present_link_prompt_is_a_card() {
    let mut s = Sim::with(Options { present: true, ..Default::default() });
    s.shell.app.proto.present.as_mut().expect("presentation").pending_url = Some(("https://example.com".into(), false));
    s.frame(vec![]);
    let open = s.rect_of("present-url-open");
    let cancel = s.rect_of("present-url-cancel");
    let card_top = open.top() - 12.0;
    let src = include_str!("../../present.rs");
    let body = &src[src.find("fn confirm_bar").expect("confirm bar")..];
    assert!(
        (open.height() - theme::CONTROL_SM).abs() < 0.1
            && (cancel.height() - theme::CONTROL_SM).abs() < 0.1
            && (1088.0 - 88.0 - card_top).abs() < 1.0
            && !body.contains("elevation_shadow"),
        "open_h={} cancel_h={} card_top={card_top} shadow={}",
        open.height(),
        cancel.height(),
        body.contains("elevation_shadow")
    );
}

#[test]
fn brand_labels_are_dark() {
    assert_eq!(crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Primary, crate::widgets::kit::State::Default).text, theme::TEXT_ON_BRAND);
    let mut bad = Vec::new();
    for (name, src) in sources() {
        let sites = if name == "present.rs" { present_chrome_sites() } else { super::chrome_census::chrome_colour_sites(src) };
        if !sites.is_empty() {
            bad.push(format!("{name} {sites:?}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn context_menu_rows_are_32() {
    let mut s = Sim::new();
    let b = world_aabb(&s.app().doc, s.by_name("Welcome"));
    s.right_click(s.screen(b.x + 40.0, b.y + 40.0));
    let row = s.rect_of("menu-row-0-0");
    assert!((row.height() - theme::MENU_ITEM_HEIGHT).abs() < 0.1, "row h {}", row.height());
}

#[test]
fn find_controls_follow_r5() {
    let mut s = Sim::new();
    s.cmd_key(Key::F);
    assert!(s.app().find.open);
    s.shell.app.find.show_replace = true;
    s.type_text("quarter");
    s.frame(vec![]);
    let search = s.rect_of("find:search");
    let one = s.rect_of("find:replace-one");
    let all = s.rect_of("find:replace-all");
    let hit = s.rect_of("find:hit0");
    assert!(
        (search.height() - theme::FIND_FIELD_H).abs() < 0.1
            && (one.height() - theme::CONTROL_XS).abs() < 0.1
            && (all.height() - theme::CONTROL_XS).abs() < 0.1
            && (hit.height() - theme::FIND_GROUP_ROW_H).abs() < 0.1,
        "search={} replace={} replace_all={} hit={}",
        search.height(),
        one.height(),
        all.height(),
        hit.height()
    );
}

#[test]
fn comment_controls_follow_r5() {
    let s = Sim::with(Options { fixture: Some("comments".into()), panel: Some("comments".into()), ..Default::default() });
    let search = s.rect_of("comments-search");
    let pages = s.rect_of("comments-filter-pages");
    let status = s.rect_of("comments-filter-status");
    let post = s.rect_of("comment-reply-post");
    assert!(
        (search.height() - theme::CONTROL_XS).abs() < 0.1
            && (pages.height() - theme::COMMENT_FILTER_H).abs() < 0.1
            && (status.height() - theme::COMMENT_FILTER_H).abs() < 0.1
            && (post.height() - theme::CONTROL_XS).abs() < 0.1,
        "search={} pages={} status={} post={}",
        search.height(),
        pages.height(),
        status.height(),
        post.height()
    );
}

#[test]
fn toast_rules_for_d() {
    let surface = crate::widgets::kit::toast_look();
    let rest = crate::widgets::kit::toast_action_look(crate::widgets::kit::State::Default);
    let hot = crate::widgets::kit::toast_action_look(crate::widgets::kit::State::Hover);
    assert_eq!(surface.fill, theme::BG_PRIMARY_ALT);
    assert_eq!(rest.fill.a(), 0);
    assert_eq!(hot.fill, theme::BG_SECONDARY_HOVER);
    assert_ne!(hot.fill, theme::BG_HOVER);
    assert_eq!(hot.text, theme::TEXT_SECONDARY_HOVER);
}

#[test]
fn updater_stays_hidden() {
    let enabled = crate::home::updater::UPDATER_UI_ENABLED;
    assert!(!enabled, "the updater stays hidden");
    let sites = super::chrome_census::chrome_colour_sites(include_str!("../../updater.rs"));
    assert!(sites.is_empty(), "updater {sites:?}");
}

#[test]
fn import_path_prompt_follows_dialog_rules() {
    let mut s = Sim::new();
    s.shell.app.open_import_path_dialog();
    s.frame(vec![]);
    let ok = s.rect_of("dialog-ok");
    let cancel = s.rect_of("dialog-cancel");
    let src = include_str!("../../home/dialog.rs");
    assert!(
        (ok.height() - theme::CONTROL_MD).abs() < 0.1
            && (cancel.height() - theme::CONTROL_MD).abs() < 0.1
            && src.contains("theme::INSET_MODAL")
            && src.contains("theme::TEXT_PLACEHOLDER"),
        "ok_h={} cancel_h={} inset={} placeholder={}",
        ok.height(),
        cancel.height(),
        src.contains("theme::INSET_MODAL"),
        src.contains("theme::TEXT_PLACEHOLDER")
    );
}
