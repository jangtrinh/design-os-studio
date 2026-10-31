//! Tests of the Skills popover: its rows, its budget line and how it closes.

use super::picker_popover::thousands;
use super::SkillsAttachmentState;
use egui::{Pos2, Rect, Vec2};
use std::path::PathBuf;

fn temp_lib_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_skills_popover_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn texts_of(state: &mut SkillsAttachmentState) -> Vec<String> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx);
    let panel = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0));
    let input = egui::RawInput { screen_rect: Some(panel), ..Default::default() };
    let out = ctx.run_ui(input, |ui| state.show_picker_popover(ui, panel, 800.0));
    crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect()
}

#[test]
fn thousands_groups_by_three() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(447), "447");
    assert_eq!(thousands(7_300), "7,300");
    assert_eq!(thousands(8_000), "8,000");
    assert_eq!(thousands(1_234_567), "1,234,567");
}

#[test]
fn skills_popover_rows_hold_name_description_and_check_only() {
    let root = temp_lib_root("rows");
    let mut state = SkillsAttachmentState::with_library_root(root.clone());
    state.toggle_skill("design-system-rhythm");
    state.toggle_skill("accessibility-wcag");
    state.open_picker();
    assert!(state.budget_line().is_none(), "two small skills are far from both limits");
    let texts = texts_of(&mut state);
    assert!(texts.iter().any(|t| t == "Attach skills"), "{texts:?}");
    for banned in ["BUILT-IN", "built-in", " tokens", "READY", "User skills:", "/skills/", "@0."] {
        assert!(!texts.iter().any(|t| t.contains(banned)), "{banned:?} is painted: {texts:?}");
    }
    assert!(!texts.iter().any(|t| t.contains("/3")), "no count pill in the header: {texts:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_budget_line_shows_only_near_or_over_a_limit() {
    let root = temp_lib_root("budget");
    let mut state = SkillsAttachmentState::with_library_root(root.clone());
    let dir = root.join("skills").join("big-one");
    std::fs::create_dir_all(&dir).unwrap();
    let body = "a".repeat(29_200);
    std::fs::write(dir.join("SKILL.md"), format!("---\nname: big-one\ndescription: Near the token budget\nversion: 1.0.0\n---\n\n{body}\n")).unwrap();
    state.reload_all();
    state.set_selected_skills(vec!["big-one".into()]);
    let (line, over) = state.budget_line().expect("7,300 tokens is within 10 % of the budget");
    assert!(!over);
    assert!(line.contains("7,3") && line.contains("of 8,000 tokens") && line.contains("1 of 3 skills"), "{line}");
    state.open_picker();
    assert!(texts_of(&mut state).iter().any(|t| t.contains("of 8,000 tokens")));

    state.set_selected_skills(vec!["design-system-rhythm".into(), "accessibility-wcag".into(), "component-craft".into()]);
    assert!(state.budget_line().is_some_and(|(l, over)| !over && l.contains("3 of 3 skills")), "three skills attached is the per-run limit");

    let mut four = state.selected_names.clone();
    four.push("big-one".into());
    state.set_selected_skills(four);
    let (text, over) = state.budget_line().expect("over the limit");
    assert!(over && text.starts_with("Too many skills"), "{text}");
    let _ = std::fs::remove_dir_all(&root);
}
