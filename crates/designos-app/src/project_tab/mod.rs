//! The Project tab: a library project's docs as a tree, a reading page and "On this page".
//!
//! One tab per library project. The tab owns a [`ProjectTabState`] (selected page, scroll, tree state) that is parked
//! with the tab when another tab shows. Every disk read, parse and write runs on the docs worker (`service`); a frame
//! only sends jobs, takes what the worker left, and paints.

mod contents;
mod delete_dialog;
#[cfg(test)]
pub(crate) mod fixture;
mod inline;
pub mod layout;
mod links;
mod menu;
mod ops;
mod reader;
mod reader_layout;
mod service;
pub mod shot;
mod tree;
mod tree_model;

use crate::app::App;
use crate::theme;
use crate::widgets;
use delete_dialog::{Answer, DeletePage};
use designos_docs::Group;
use egui::{Pos2, Rect, Ui};
use ops::TreeSnapshot;
use reader::{Reader, ReaderAction};
use service::{DocsService, Job, LoadedPage, PageEvent};
use std::sync::Arc;
use tree::{MenuTarget, TreeAction, TreeInput, TreeUi};

/// What the reading column shows for the selected page.
pub enum PageView {
    /// Nothing selected, or the selected page has not arrived yet.
    None,
    Loaded(LoadedPage),
    Failed {
        rel: String,
        message: String,
    },
    Gone(String),
}

pub struct ProjectTabState {
    pub project_id: String,
    pub project_name: String,
    service: Option<DocsService>,
    /// Why the docs worker could not start.
    start_error: Option<String>,
    pub snapshot: Arc<TreeSnapshot>,
    pub snapshot_loaded: bool,
    pub selected: Option<String>,
    pub page: PageView,
    pub tree: TreeUi,
    pub reader: Reader,
    pub delete: Option<DeletePage>,
    was_focused: bool,
    shown_manifest_error: Option<String>,
    shot: Option<shot::ShotSpec>,
    /// The doc tree overlay of a window under 800 px.
    tree_overlay: bool,
    last_left_collapsed: Option<bool>,
}

/// "<project> · Project", the label of the tab.
pub fn tab_title(project_name: &str) -> String {
    format!("{project_name} \u{b7} Project")
}

impl ProjectTabState {
    pub fn new(project_id: &str, project_name: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            project_name: project_name.to_string(),
            service: None,
            start_error: None,
            snapshot: Arc::new(TreeSnapshot::empty()),
            snapshot_loaded: false,
            selected: None,
            page: PageView::None,
            tree: TreeUi::default(),
            reader: Reader::default(),
            delete: None,
            was_focused: true,
            shown_manifest_error: None,
            shot: None,
            tree_overlay: false,
            last_left_collapsed: None,
        }
    }

    /// A tab that opens on what a screenshot run asked for.
    pub fn with_shot(project_id: &str, project_name: &str, spec: shot::ShotSpec) -> Self {
        let mut st = Self::new(project_id, project_name);
        st.shot = Some(spec);
        st
    }

    fn ensure_service(&mut self, app: &App, ctx: &egui::Context) {
        if self.service.is_some() || self.start_error.is_some() {
            return;
        }
        let Some(lib) = app.lib.clone() else {
            self.start_error = Some("Studio has no library open, so the project's docs cannot be read.".into());
            return;
        };
        match DocsService::start(ctx.clone(), lib, self.project_id.clone(), self.project_name.clone()) {
            Ok(service) => self.service = Some(service),
            Err(e) => self.start_error = Some(e),
        }
    }

    fn send(&self, job: Job) {
        if let Some(service) = &self.service {
            service.send(job);
        }
    }

    pub fn select(&mut self, rel: &str) {
        if self.selected.as_deref() == Some(rel) && matches!(self.page, PageView::Loaded(_)) {
            return;
        }
        self.selected = Some(rel.to_string());
        self.page = PageView::None;
        self.reader.reset();
        self.send(Job::Open(rel.to_string()));
    }

    /// The first page of the first group that has one (Overview first).
    fn first_page(&self) -> Option<String> {
        Group::ALL.into_iter().find_map(|g| self.snapshot.group_pages(g).into_iter().next())
    }

    /// Takes what the worker left and applies it.
    fn pump(&mut self, app: &mut App) {
        let Some(service) = &self.service else { return };
        let inbox = service.take();
        if let Some(snapshot) = inbox.snapshot {
            self.snapshot = snapshot;
            self.snapshot_loaded = true;
            if let Some(err) = self.snapshot.manifest_error.clone() {
                if self.shown_manifest_error.as_deref() != Some(err.as_str()) {
                    app.show_toast(err.clone(), true);
                    self.shown_manifest_error = Some(err);
                }
            } else {
                self.shown_manifest_error = None;
            }
        }
        if let Some((old, new)) = inbox.renamed {
            if self.selected.as_deref() == Some(old.as_str()) {
                self.selected = Some(new);
            }
        }
        if let Some(rel) = inbox.select {
            self.select(&rel);
        }
        if let Some(rel) = inbox.rename {
            let title = self.snapshot.title_of(&rel);
            self.tree.start_rename(&rel, &title);
        }
        if let Some(event) = inbox.page {
            if self.selected.as_deref() == Some(event.rel()) {
                self.apply_page(event);
            }
        }
        if let (Some(rel), PageView::Loaded(page)) = (inbox.oversize, &mut self.page) {
            if page.rel == rel {
                page.oversize = true;
            }
        }
        for e in inbox.errors {
            app.show_toast(e, true);
        }
        for n in inbox.notes {
            app.show_toast(n, false);
        }
        if self.snapshot_loaded && self.selected.is_none() {
            if let Some(rel) = self.first_page() {
                self.select(&rel);
            }
        }
        // The open page is gone from the project (deleted here or elsewhere): land on another page.
        if self.snapshot_loaded {
            if let Some(rel) = self.selected.clone() {
                let listed = self.snapshot.pages.contains_key(&rel) || Group::ALL.into_iter().any(|g| self.snapshot.manifest.group(g).pages.contains(&rel));
                if !listed && matches!(self.page, PageView::Gone(_) | PageView::None) {
                    self.selected = None;
                    if let Some(next) = self.first_page() {
                        self.select(&next);
                    }
                }
            }
        }
    }

    fn apply_page(&mut self, event: PageEvent) {
        match event {
            PageEvent::Loaded(page) => self.page = PageView::Loaded(page),
            PageEvent::Failed { rel, message, oversize } => {
                let message = if oversize { OVERSIZE_NOTE.to_string() } else { message };
                self.page = PageView::Failed { rel, message };
            }
            PageEvent::Gone(rel) => self.page = PageView::Gone(rel),
        }
    }

    /// Runs the shot the screenshot run asked for, once the docs have arrived.
    fn apply_shot(&mut self, layout: &layout::ProjectLayout) {
        if !self.snapshot_loaded || self.selected.is_none() || self.shot.is_none() {
            return;
        }
        let Some(spec) = self.shot.take() else { return };
        if let Some(rel) = &spec.page {
            self.select(rel);
        }
        if let Some(q) = &spec.search {
            self.tree.open_search();
            self.tree.query = q.clone();
        }
        let rows = tree_model::build_rows(&self.snapshot, &self.tree.collapsed, &self.tree.query);
        let regions = tree::regions(layout.tree, self.tree.search_open);
        if let Some(rel) = &spec.menu {
            if let Some((i, group)) = rows.iter().enumerate().find_map(|(i, r)| match &r.kind {
                tree_model::RowKind::Page { rel: p, group } if p == rel => Some((i, *group)),
                _ => None,
            }) {
                let at = Pos2::new(
                    regions.list.left() + theme::PROJECT_TREE_SHOT_MENU_DX,
                    regions.list.top() + i as f32 * theme::LAYER_ROW_HEIGHT + theme::LAYER_ROW_HEIGHT / 2.0,
                );
                self.tree.menu = Some((MenuTarget::Page(rel.clone(), group), at));
            }
        }
        if let Some(rel) = &spec.delete {
            let title = self.snapshot.title_of(rel);
            self.delete = Some(DeletePage::new(rel.clone(), title));
        }
    }

    fn notes(&self) -> Vec<String> {
        let PageView::Loaded(p) = &self.page else { return Vec::new() };
        let mut notes = Vec::new();
        if let Some(front) = p.page.front_matter() {
            let problems = front.problems();
            if let Some(first) = problems.first() {
                let more = if problems.len() > 1 { format!(" (+{} more)", problems.len() - 1) } else { String::new() };
                notes.push(format!("Front matter: {} {}; fix: {}{more}", first.field, first.problem, first.fix));
            }
        }
        if p.oversize {
            notes.push(OVERSIZE_NOTE.to_string());
        }
        notes
    }

    fn run_tree_action(&mut self, app: &mut App, ctx: &egui::Context, action: TreeAction) {
        match action {
            TreeAction::Open(rel) => self.select(&rel),
            TreeAction::Rename { rel, title } => {
                if title != self.snapshot.title_of(&rel) {
                    self.send(Job::Rename { rel, title });
                }
            }
            TreeAction::Reorder { group, order } => self.send(Job::Reorder { group, order }),
            TreeAction::AskDelete(rel) => {
                let title = self.snapshot.title_of(&rel);
                self.delete = Some(DeletePage::new(rel, title));
            }
            TreeAction::NewPage(group) => self.send(Job::NewPage { group }),
            TreeAction::OpenInHome => {
                let mut tabs = std::mem::take(&mut app.tabs);
                tabs.enter_home(app);
                app.tabs = tabs;
                if let Some(h) = app.home.as_mut() {
                    h.section = crate::home::Section::Project(self.project_id.clone(), None);
                }
            }
            TreeAction::RevealDocs => {
                let dir = app.lib.as_ref().and_then(|l| designos_docs::DocsStore::for_project(l.root(), &self.project_id).ok()).map(|s| s.dir().to_path_buf());
                match dir {
                    Some(dir) if dir.is_dir() => {
                        if let Err(e) = crate::home::reveal(&dir) {
                            app.show_toast(e, true);
                        }
                    }
                    _ => app.show_toast("This project has no docs folder yet", true),
                }
            }
            TreeAction::CopyLink(rel) => {
                let title = self.snapshot.title_of(&rel);
                ctx.copy_text(format!("[{title}]({rel})"));
                app.show_toast("Link copied", false);
            }
            TreeAction::EditSource(rel) => {
                let path = app.lib.as_ref().and_then(|l| designos_docs::DocsStore::for_project(l.root(), &self.project_id).ok()).map(|s| s.dir().join(&rel));
                match path {
                    Some(path) => {
                        if let Err(e) = crate::home::reveal(&path) {
                            app.show_toast(e, true);
                        }
                    }
                    None => app.show_toast("This page has no file to open", true),
                }
            }
        }
    }

    fn run_reader_action(&mut self, app: &mut App, action: ReaderAction) {
        match action {
            ReaderAction::OpenPage { rel, anchor } => {
                let known = self.snapshot.pages.contains_key(&rel) || Group::ALL.into_iter().any(|g| self.snapshot.manifest.group(g).pages.contains(&rel));
                if !known {
                    app.show_toast(format!("There is no page {rel} in this project"), true);
                    return;
                }
                if self.selected.as_deref() != Some(rel.as_str()) {
                    self.select(&rel);
                }
                if let Some(anchor) = anchor {
                    self.reader.scroll_to_anchor(anchor);
                }
            }
            ReaderAction::OpenUrl(url) => app.dev_open_link("", &url),
        }
    }
}

const OVERSIZE_NOTE: &str = "This page is larger than 8 MB and is not watched for outside changes.";

/// Draws the Project tab into `full` (everything under the tab strip, rail included).
pub fn show(app: &mut App, ui: &mut Ui, full: Rect, st: &mut ProjectTabState) {
    let ctx = ui.ctx().clone();
    let lay = layout::compute(full);
    st.ensure_service(app, &ctx);
    if let Some(service) = &st.service {
        service.mark_shown(ctx.cumulative_pass_nr());
    }
    st.pump(app);
    let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
    if focused && !st.was_focused {
        st.send(Job::Refresh);
    }
    st.was_focused = focused;
    st.apply_shot(&lay);
    // Cmd+\ (the left panel toggle) opens and closes the tree overlay of a narrow window.
    if st.last_left_collapsed.is_some_and(|prev| prev != app.ui.left_collapsed) {
        st.tree_overlay = !st.tree_overlay;
    }
    st.last_left_collapsed = Some(app.ui.left_collapsed);

    ui.painter().rect_filled(full, 0.0, theme::BG);
    widgets::record(ui, "project-tab", full);
    widgets::record(ui, "project-column", lay.column);
    if let Some(c) = lay.contents {
        widgets::record(ui, "project-contents", c);
    }
    crate::panels::rail::show(app, ui, lay.rail);
    let modal = st.delete.is_some();
    paint_page(app, ui, &lay, st, !modal);
    if let Some(error) = st.start_error.clone() {
        let p = ui.painter().clone();
        p.text(
            Pos2::new(lay.column.left(), lay.column.top() + theme::PROJECT_TITLE_LINE / 2.0),
            egui::Align2::LEFT_CENTER,
            error,
            theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular),
            theme::TEXT_ERROR,
        );
    }
    if !lay.tree_is_overlay || st.tree_overlay {
        let selected = st.selected.clone();
        let snapshot = st.snapshot.clone();
        let input = TreeInput { project_name: &st.project_name, snapshot: &snapshot, selected: selected.as_deref(), modal };
        widgets::record(ui, "project-tree", lay.tree);
        let actions = tree::show(ui, lay.tree, &input, &mut st.tree);
        for a in actions {
            st.run_tree_action(app, &ctx, a);
        }
    }
    if let Some(d) = st.delete.as_mut() {
        match delete_dialog::show(&ctx, d) {
            Answer::Open => {}
            Answer::Cancel => st.delete = None,
            Answer::Delete => {
                if let Some(d) = st.delete.take() {
                    st.send(Job::Trash { rel: d.rel });
                }
            }
        }
    }
}

fn paint_page(app: &mut App, ui: &mut Ui, lay: &layout::ProjectLayout, st: &mut ProjectTabState, interactive: bool) {
    let main = lay.main;
    match &st.page {
        PageView::Loaded(page) => {
            let page = page.clone();
            let notes = st.notes();
            let snapshot = st.snapshot.clone();
            let actions = st.reader.show(ui, main, lay.column, &page, &snapshot, &notes, interactive);
            if let Some(rect) = lay.contents {
                let headings = st.reader.headings().to_vec();
                let active = st.reader.active_heading();
                if let Some(i) = contents::show(ui, rect, &headings, active) {
                    st.reader.scroll_to_heading(ui.ctx(), i);
                }
            }
            for a in actions {
                st.run_reader_action(app, a);
            }
        }
        PageView::Failed { rel, message } => message_card(ui, lay, &st.snapshot.title_of(rel), message),
        PageView::Gone(rel) => message_card(
            ui,
            lay,
            &st.snapshot.title_of(rel),
            "This page's file is gone. It was deleted or moved outside Studio; pick another page in the tree.",
        ),
        PageView::None => {}
    }
}

/// A page that cannot be shown: its name, and why.
fn message_card(ui: &mut Ui, lay: &layout::ProjectLayout, title: &str, message: &str) {
    let p = ui.painter().clone();
    let x = lay.column.left();
    let mut y = lay.column.top();
    let title_font = theme::font_text(30.0, theme::TextWeight::Semibold);
    let shown = widgets::elide(&p, title, &title_font, lay.column.width());
    p.text(Pos2::new(x, y + theme::PROJECT_TITLE_LINE / 2.0), egui::Align2::LEFT_CENTER, shown, title_font, theme::TEXT);
    y += theme::PROJECT_TITLE_LINE + 12.0;
    let galley =
        p.layout(message.to_string(), theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular), theme::TEXT_TERTIARY, lay.column.width());
    p.galley(Pos2::new(x, y), galley, theme::TEXT_TERTIARY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_is_named_after_the_project() {
        assert_eq!(tab_title("Checkout"), "Checkout \u{b7} Project");
    }
}
