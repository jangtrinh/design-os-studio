//! The popovers built by hand (not through `widgets::menu` or `widgets::popover`) mark themselves open too, so an agent
//! terminal that holds the keyboard hears neither their `Esc` nor their `Enter`. The Check designs panel is the one
//! popup that must not: it has no `Esc` and no outside-press close, so a mark would leave the terminal deaf.
//!
//! The rows are part of `overlay_escape::overlays()`, so the `Esc` and `Enter` tests there cover them. The tests below add
//! what those do not: an outside press closes each of them, and the Check designs panel never holds the keys.

use super::overlay_escape::{overlays, painted_text, sim_with_overlay_open, terminal_bytes, Overlay, ESC_BYTE};
use super::*;

fn settle(s: &mut Sim) {
    s.frame(vec![]);
    s.frame(vec![]);
}

/// Selects `name` and lets the inspector draw it. The inspector draws after the Agents panel, so a click in it does not take
/// the keyboard from the terminal.
fn select(s: &mut Sim, name: &str) {
    let id = s.by_name(name);
    s.shell.app.selection = vec![id];
    settle(s);
}

/// A point inside the terminal viewport of the Agents panel: the left panel, half way down.
fn terminal_point(s: &Sim) -> Pos2 {
    let canvas = s.app().canvas_rect;
    Pos2::new(canvas.left() - 60.0, canvas.center().y)
}

fn open_colour_picker(s: &mut Sim) {
    select(s, "Hero");
    let screen = s.ctx.content_rect().size();
    crate::panels::properties::preset_popup(&mut s.shell.app, "picker", screen);
}

fn open_font_dropdown(s: &mut Sim) {
    let screen = s.ctx.content_rect().size();
    // Selects a text node of the page. The Type settings popover this also opens is closed again.
    crate::panels::properties::preset_popup(&mut s.shell.app, "type-details", screen);
    s.shell.app.props.pop = None;
    settle(s);
    let field = s.field_rect("font-family");
    s.click(field.center());
}

fn font_dropdown_is_open(s: &Sim) -> bool {
    painted_text(s).iter().any(|(text, _)| text == "Search fonts")
}

fn open_prototype_popover(s: &mut Sim) {
    select(s, "Hero");
    s.shell.app.ui.right_tab = Some(crate::app::RightTab::Prototype);
    settle(s);
    let hero = s.by_name("Hero");
    s.shell.app.proto_add_reaction(hero);
}

/// The floating Variables view is the one drawn after the Agents panel.
fn open_variables_view(s: &mut Sim) {
    s.shell.app.vars.full = false;
    s.shell.app.open_variables();
    settle(s);
}

fn open_variables_edit_popover(s: &mut Sim) {
    open_variables_view(s);
    let var = s.app().doc.variable_collections.values().flat_map(|c| c.variables.iter()).next().cloned().expect("a variable in the fixture");
    s.shell.app.vars.edit = Some(crate::variables::EditPopover { var, anchor: Rect::from_min_size(Pos2::new(700.0, 300.0), Vec2::ZERO), scroll: 0.0 });
}

fn open_variables_cell_colour_picker(s: &mut Sim) {
    open_variables_view(s);
    let doc = &s.app().doc;
    let (collection, variable) = doc
        .variable_collections
        .values()
        .flat_map(|c| c.variables.iter().map(move |v| (c, v)))
        .find_map(|(c, v)| doc.variables.get(v).filter(|v| v.kind == designos_doc::VariableKind::Color).map(|v| (c.clone(), v.clone())))
        .expect("a colour variable in the fixture");
    let picker = crate::variables::CellPicker {
        var: variable.id,
        mode: collection.default_mode,
        color: egui::Color32::from_rgb(0x3b, 0x82, 0xf6),
        anchor: Rect::from_min_size(Pos2::new(700.0, 300.0), Vec2::splat(20.0)),
        before: designos_doc::VariableStore::from_doc(doc),
    };
    s.shell.app.vars.cell_picker = Some(picker);
}

fn open_variables_skipped_details_popover(s: &mut Sim) {
    open_variables_view(s);
    let skipped =
        (1..=3).map(|i| designos_fig::SkippedVariable { path: format!("Token/Item{i}"), reason: "cross-collection-alias-kept-as-literal".into() }).collect();
    s.shell.app.vars.last_import_report =
        Some(designos_fig::ImportReport { format: designos_fig::VariablesFormat::FigmaNative, added: 3, updated: 0, unchanged: 0, skipped });
    s.shell.app.vars.show_skipped_details = true;
}

/// The six popovers, as rows of the `Esc` and `Enter` tests in `overlay_escape`.
pub(super) fn rows() -> Vec<Overlay> {
    vec![
        ("colour picker", open_colour_picker, |s| s.app().props.picker.is_some()),
        ("font dropdown", open_font_dropdown, font_dropdown_is_open),
        ("prototype popover", open_prototype_popover, |s| s.app().proto.editing.is_some()),
        ("Variables edit popover", open_variables_edit_popover, |s| s.app().vars.edit.is_some()),
        ("Variables cell colour picker", open_variables_cell_colour_picker, |s| s.app().vars.cell_picker.is_some()),
        ("Variables skipped-details popover", open_variables_skipped_details_popover, |s| s.app().vars.show_skipped_details),
    ]
}

#[test]
fn an_outside_press_closes_each_hand_built_popover_and_the_terminal_hears_typing_again() {
    let mut problems = Vec::new();
    let names: Vec<&str> = rows().iter().map(|(name, _, _)| *name).collect();
    for overlay in overlays().into_iter().filter(|(name, _, _)| names.contains(name)) {
        let (name, _, is_open) = overlay;
        let mut s = match sim_with_overlay_open(&overlay) {
            Ok(s) => s,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        let at = terminal_point(&s);
        s.click(at);
        settle(&mut s);
        if is_open(&s) {
            problems.push(format!("{name}: a press in the terminal did not close it"));
        }
        s.shell.app.terminal.tabs[0].written_bytes.clear();
        s.type_text("z");
        if !terminal_bytes(&s).contains(&b'z') {
            problems.push(format!("{name}: typing after a click in the terminal did not reach it: {:?}", terminal_bytes(&s)));
        }
        finish(s);
    }
    assert_eq!(names.len(), 6, "the six popovers of the issue");
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_check_designs_panel_never_holds_the_keys_from_the_terminal() {
    // The panel has no `Esc` and no outside-press close, so a terminal that waited for it to close would stay deaf. A press in
    // the terminal must give it the keys while the panel stays open.
    let mut s = agent_sim();
    s.shell.app.check_designs.open = true;
    settle(&mut s);
    assert!(s.app().check_designs.open, "precondition: the panel is open");
    assert!(s.app().terminal.is_capturing_keyboard(true), "precondition: the terminal holds the keyboard");

    let at = terminal_point(&s);
    s.click(at);
    settle(&mut s);
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.type_text("z");
    s.key(Key::Escape);
    let bytes = terminal_bytes(&s);
    assert!(bytes.contains(&b'z'), "typing after a click in the terminal must reach it while the panel is open: {bytes:?}");
    assert!(bytes.contains(&ESC_BYTE), "Esc is the terminal's: the panel does not read it: {bytes:?}");
    assert!(s.app().check_designs.open, "the panel stays open: nothing here closes it");
    finish(s);
}
