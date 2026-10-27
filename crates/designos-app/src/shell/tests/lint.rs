//! Layout lint: runs the app headless in several states and checks that no painted
//! text overflows the widget it sits in and that icons are centered in their square
//! buttons (see `crate::lint`).

use super::*;
use crate::lint;

fn audit(label: &str, opts: Options) -> Vec<lint::Finding> {
    lint::take_icons();
    let mut s = Sim::with(opts);
    s.frame(vec![]);
    lint::take_icons();
    s.frame(vec![]);
    let (texts, boxes) = lint::scan_shapes(&s.shapes);
    // The audit must really see the screen: a lot of text and rects.
    assert!(texts.len() > 15 && boxes.len() > 15, "[{label}] only {} texts / {} boxes painted", texts.len(), boxes.len());
    if label.contains("picker") {
        assert!(texts.iter().any(|t| t.text == "Custom"), "[{label}] picker not painted");
    }
    let mut out = lint::check_text(&texts, &boxes);
    out.extend(lint::check_icons(&lint::take_icons()));
    for f in &mut out {
        f.what = format!("[{label}] {}", f.what);
    }
    out
}

fn states() -> Vec<(&'static str, Options)> {
    let sel = |n: &str| Options { select: Some(n.into()), ..Options::default() };
    vec![
        ("no selection", Options { select: Some("none".into()), ..Options::default() }),
        ("frame", sel("Welcome")),
        ("text", sel("Continue")),
        ("instance", sel("Get started")),
        ("rectangle", sel("Card")),
        ("fill picker", Options { select: Some("Card".into()), panel: Some("picker".into()), ..Options::default() }),
        ("gradient picker", Options { select: Some("Hero".into()), panel: Some("picker-gradient".into()), ..Options::default() }),
        ("effects menu", Options { select: Some("Hero".into()), panel: Some("effects-menu".into()), ..Options::default() }),
        ("stroke settings", Options { select: Some("Hero".into()), panel: Some("stroke-settings".into()), ..Options::default() }),
        ("type settings", Options { select: Some("Continue".into()), panel: Some("type-details".into()), ..Options::default() }),
        ("noise settings", Options { select: Some("Hero".into()), panel: Some("effect-noise".into()), ..Options::default() }),
        ("texture settings", Options { select: Some("Hero".into()), panel: Some("effect-texture".into()), ..Options::default() }),
        ("glass settings", Options { select: Some("Hero".into()), panel: Some("effect-glass".into()), ..Options::default() }),
        ("main menu", Options { menu: Some("main".into()), ..Options::default() }),
        ("shape menu", Options { menu: Some("shape".into()), ..Options::default() }),
    ]
}

#[test]
fn nothing_overflows_and_icons_are_centered() {
    let mut all = Vec::new();
    for (label, opts) in states() {
        all.extend(audit(label, opts));
    }
    for f in &all {
        eprintln!("LINT {} :: {}", f.what, f.detail);
    }
    assert!(all.is_empty(), "{} layout findings", all.len());
}

fn audit_sim(label: &str, s: &mut Sim, check_canvas_purple: bool) -> Vec<lint::Finding> {
    lint::take_icons();
    s.frame(vec![]);
    lint::take_icons();
    s.frame(vec![]);
    let (texts, boxes) = lint::scan_shapes(&s.shapes);
    assert!(texts.len() > 8 && boxes.len() > 8, "[{label}] only {} texts / {} boxes painted", texts.len(), boxes.len());
    let icons = lint::take_icons();
    let mut out = lint::check_text(&texts, &boxes);
    out.extend(lint::check_icons(&icons));
    out.extend(lint::check_pointer_targets(&icons));
    let exclude = if check_canvas_purple || s.app().home.is_some() { None } else { Some(s.app().canvas_rect) };
    out.extend(lint::check_no_purple_chrome(&s.shapes, exclude));
    out.extend(lint::check_contrast(&s.shapes, exclude));
    for f in &mut out {
        f.what = format!("[{label}] {}", f.what);
    }
    out
}

#[test]
fn design_os_gates_audit_all_studio_screens_zero_overflow_zero_purple_and_valid_contrast() {
    let mut all = Vec::new();

    // Seed a temporary showcase library so Home sections render real project cards, folders, and thumbnails.
    let lib_dir = std::env::temp_dir().join(format!("designos-gate-lint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lib_dir);
    crate::showcase::seed_library(&lib_dir).expect("seed showcase library");

    // 1. Audit Home sections (Recents, Templates, AllProjects, Resources, Drafts)
    for (label, section) in [
        ("home recents", crate::home::Section::Recents),
        ("home templates", crate::home::Section::Templates),
        ("home all_folders", crate::home::Section::AllProjects),
        ("home resources", crate::home::Section::Resources),
        ("home drafts", crate::home::Section::Drafts),
    ] {
        let mut s = Sim::with(Options { home: true, library: Some(lib_dir.clone()), ..Options::default() });
        s.shell.app.tabs.always_visible = true;
        if let Some(h) = s.shell.app.home.as_mut() {
            h.section = section;
        }
        all.extend(audit_sim(label, &mut s, true));

        // Verify Home Header & Sidebar primary interactive controls meet >= 24x24px pointer target floor
        for key in ["home-search", "home-new-project", "tab-bar-home", "tab-bar-new"] {
            let r = s.rect_of(key);
            if r.width() < 24.0 || r.height() < 24.0 {
                all.push(lint::Finding {
                    what: format!("[{label}] primary button {key} below 24x24px floor"),
                    detail: format!("rect {:?} ({}x{})", r, r.width(), r.height()),
                });
            }
        }
    }

    // 2. Audit Dev Mode, Assets Panel, Variables Modal, Plugins Modal, Specs Reader, Agents Panel, Libraries Modal, Cmd+K Modal
    for (label, opts) in [
        ("dev mode", Options { select: Some("Welcome".into()), dev: Some("css".into()), dev_demo: true, ..Options::default() }),
        ("assets panel", Options { panel: Some("assets".into()), ..Options::default() }),
        ("variables modal", Options { panel: Some("variables".into()), ..Options::default() }),
        ("plugins modal", Options { panel: Some("plugins".into()), ..Options::default() }),
        ("specs reader", Options { panel: Some("specs".into()), ..Options::default() }),
        ("libraries modal", Options { panel: Some("libraries".into()), ..Options::default() }),
        ("cmdk modal", Options { panel: Some("cmdk".into()), ..Options::default() }),
        ("check designs panel", Options { fixture: Some(crate::check_designs::FIXTURE.into()), panel: Some("check-designs".into()), ..Options::default() }),
    ] {
        let mut s = Sim::with(opts);
        s.shell.app.tabs.always_visible = true;
        all.extend(audit_sim(label, &mut s, false));
    }

    // Also audit Agents panel UI (without spawning external PTY subprocesses during unit tests)
    {
        let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Options::default() });
        s.shell.app.tabs.always_visible = true;
        s.shell.app.ui.rail = Some(crate::app::RailItem::Agents);
        s.shell.app.ui.left_collapsed = false;
        all.extend(audit_sim("agents panel", &mut s, false));
    }

    // 3. Verify Editor primary interactive buttons on Top Tab Bar, Left Rail/Panel, Right Panel, Bottom Toolbar meet >= 24x24px floor
    {
        let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Options::default() });
        s.shell.app.tabs.always_visible = true;
        s.frame(vec![]);
        for key in [
            "tab-bar-home",
            "tab-bar-new",
            "rail-menu",
            "rail-File",
            "rail-Assets",
            "file-header",
            "layers-search",
            "present-button",
            "share-button",
            "right-tab-Design",
            "right-tab-Prototype",
        ] {
            let r = s.rect_of(key);
            if r.width() < 24.0 || r.height() < 24.0 {
                all.push(lint::Finding {
                    what: format!("[editor chrome] primary button {key} below 24x24px floor"),
                    detail: format!("rect {:?} ({}x{})", r, r.width(), r.height()),
                });
            }
        }
        let bar = crate::panels::toolbar::toolbar_rect(s.ctx.content_rect());
        if bar.height() < 24.0 {
            all.push(lint::Finding { what: "[editor chrome] bottom toolbar below 24px height floor".into(), detail: format!("rect {:?}", bar) });
        }
        for gi in 0..crate::canvas::tools::GROUPS.len() {
            let gb = crate::panels::toolbar::group_button_rect(s.ctx.content_rect(), gi);
            if gb.width() < 24.0 || gb.height() < 24.0 {
                all.push(lint::Finding { what: format!("[editor chrome] toolbar group button {gi} below 24x24px floor"), detail: format!("rect {:?}", gb) });
            }
        }
    }

    let _ = std::fs::remove_dir_all(&lib_dir);

    for f in &all {
        eprintln!("STUDIO_GATE_LINT {} :: {}", f.what, f.detail);
    }
    assert!(all.is_empty(), "{} studio screen gate findings", all.len());
}

fn run_of(rect: Rect, clip: Rect) -> lint::TextRun {
    lint::TextRun { text: "-".into(), rect, clip }
}

fn band(top: f32, bottom: f32) -> Rect {
    Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(240.0, bottom))
}

fn label(top: f32, bottom: f32) -> Rect {
    Rect::from_min_max(Pos2::new(20.0, top), Pos2::new(60.0, bottom))
}

#[test]
fn a_box_under_the_hidden_half_does_not_hold_the_line() {
    // a scroll region ends at y 100, a bar fills 100..136 below it, and a line sits across the cut
    let bar = band(100.0, 136.0);
    let line = Rect::from_min_size(Pos2::new(20.0, 97.0), Vec2::new(40.0, 12.0));
    assert!(lint::check_text(&[run_of(line, band(0.0, 100.0))], &[bar]).is_empty(), "the bar lies wholly under the hidden half");
    // the same line with nothing cutting it does sit across the bar's edge
    assert_eq!(lint::check_text(&[run_of(line, band(0.0, 136.0))], &[bar]).len(), 1);
}

#[test]
fn a_label_poking_into_the_row_above_is_flagged_when_its_clip_cuts_at_106() {
    // row 100..124, label 94..118: 94..100 is painted over the row above, and the cut leaves 94..106 showing
    let findings = lint::check_text(&[run_of(label(94.0, 118.0), band(0.0, 106.0))], &[band(100.0, 124.0)]);
    assert_eq!(findings.len(), 1, "{findings:?}");
}

#[test]
fn a_label_poking_into_the_row_above_is_flagged_when_its_clip_cuts_at_105() {
    // one pixel higher: the label's own centre (106) is hidden, and the row is still under the part that shows
    let findings = lint::check_text(&[run_of(label(94.0, 118.0), band(0.0, 105.0))], &[band(100.0, 124.0)]);
    assert_eq!(findings.len(), 1, "{findings:?}");
}

#[test]
fn a_label_poking_into_the_row_below_is_flagged_when_its_clip_starts_at_14() {
    // row 0..24, label 8..32: 24..32 is painted over the row below, and the cut leaves 14..32 showing
    let findings = lint::check_text(&[run_of(label(8.0, 32.0), band(14.0, 200.0))], &[band(0.0, 24.0)]);
    assert_eq!(findings.len(), 1, "{findings:?}");
}
