//! Floating surfaces: command bar, dialogs, menus, Find, comments, presentation, updater.

use super::*;

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
        || line.contains("let tint = Color32::WHITE.gamma_multiply(")
        || (line.contains("Color32::BLACK") && line.contains("PRESENT_BEZEL"))
}

#[test]
fn present_allowlist_names_the_content_lines_exactly() {
    assert!(present_content_line("let tint = Color32::WHITE.gamma_multiply(alpha.clamp(0.0, 1.0));"));
    assert!(present_content_line("pc.image(t.id(), stage, ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);"));
    assert!(!present_content_line("let label = Color32::WHITE.gamma_multiply(0.5);"));
    assert!(!present_content_line("p.rect_filled(bar, 0.0, Color32::WHITE);"));
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
fn find_controls_use_xs_heights() {
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
fn comment_controls_use_xs_heights() {
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
fn updater_stays_hidden() {
    let enabled = crate::home::updater::UPDATER_UI_ENABLED;
    assert!(!enabled, "the updater stays hidden");
    let sites = super::chrome_census::chrome_colour_sites(include_str!("../../updater.rs"));
    assert!(sites.is_empty(), "updater {sites:?}");
}
