//! Modal dialogs shared by the home screen and the editor: name prompts,
//! confirmations, the project picker, keyboard shortcuts and About.

use super::Section;
use crate::app::App;
use crate::theme;
use crate::widgets::{self, kit};
use designos_library::Library;
use egui::{Align2, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Vec2};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    NewProject,
    RenameProject(String),
    NewFolder(String, Option<String>),
    RenameFolder(String, String),
    RenameFile(String),
    MoveFile(String),
    /// Store the open document as a new library file.
    SaveToLibrary,
    EmptyTrash,
    /// Settings: the name comments are posted under.
    AuthorName,
    /// Names the version history entry with this number.
    NameVersion(u32),
    OpenGithub,
    /// Opens a dev link (annotation / Dev Mode link) after confirmation.
    OpenDevLink(String),
    Shortcuts,
    About,
    /// The file of the open document changed on disk while it has unsaved edits: use the file, or keep mine.
    DiskChanged,
    /// A document with edits no save has written is about to be closed or replaced: Save, Don't save or Cancel.
    SaveChanges,
    /// Opens / imports a file by path when no system file dialog could open (`#1192`).
    ImportPath,
}

/// One row of the project picker: a project or one of its folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub project: String,
    pub folder: Option<String>,
    pub label: String,
    pub depth: usize,
}

pub const IMPORT_PATH_HINT: &str = "Path to a .fig, .designos, .json or .svg file";

#[derive(Clone, Debug)]
pub struct Dialog {
    pub intent: Intent,
    pub title: String,
    /// Body rows: `(text, "")` is a wrapped paragraph, `(key, text)` a two-column row.
    pub lines: Vec<(String, String)>,
    /// Text field content (for prompts).
    pub input: Option<String>,
    /// Placeholder shown inside the text field while `input` is empty.
    pub hint: Option<String>,
    pub choices: Vec<Choice>,
    pub picked: Option<usize>,
    pub ok: String,
    pub danger: bool,
    /// No Cancel button (information only).
    pub info: bool,
    /// The label of the safe answer, shown where Cancel would be. Enter and Escape give it, so only a click
    /// on the main button does the thing that cannot be undone.
    pub safe_label: Option<String>,
    /// A third answer, between Cancel and the main button.
    pub alt_label: Option<String>,
    fresh: bool,
    scroll: f32,
}

const W: f32 = 440.0;
const PAD: f32 = theme::INSET_MODAL;
const BTN_H: f32 = theme::CONTROL_MD;
const ROW_H: f32 = theme::CONTROL_SM;
const MAX_CHOICES: usize = 8;

impl Dialog {
    fn base(intent: Intent, title: &str, ok: &str) -> Self {
        Self {
            intent,
            title: title.into(),
            lines: vec![],
            input: None,
            hint: None,
            choices: vec![],
            picked: None,
            ok: ok.into(),
            danger: false,
            info: false,
            safe_label: None,
            alt_label: None,
            fresh: true,
            scroll: 0.0,
        }
    }

    pub fn prompt(intent: Intent, title: &str, initial: &str, ok: &str) -> Self {
        Self { input: Some(initial.into()), ..Self::base(intent, title, ok) }
    }

    pub fn confirm(intent: Intent, title: &str, body: &[&str], ok: &str) -> Self {
        Self { lines: body.iter().map(|l| (l.to_string(), String::new())).collect(), ..Self::base(intent, title, ok) }
    }

    /// A document outside the library has edits no save has written, and an action would drop them. Enter means Save
    /// (the default button: saving loses nothing); Esc and a click outside mean Cancel; only a click on Don't save
    /// drops the edits.
    pub fn save_changes(name: &str) -> Self {
        let name = if name.chars().count() > 28 { format!("{}\u{2026}", name.chars().take(27).collect::<String>()) } else { name.to_string() };
        Self {
            alt_label: Some("Don't save".into()),
            ..Self::confirm(Intent::SaveChanges, &format!("Save changes to {name}?"), &["Your changes are lost if you don't save."], "Save")
        }
    }

    /// The open document's file changed on disk under unsaved edits.
    pub fn disk_changed() -> Self {
        Self {
            danger: true,
            safe_label: Some("Keep mine".into()),
            ..Self::confirm(Intent::DiskChanged, "File changed on disk", &["Another program changed this file.", "Use file replaces your edits."], "Use file")
        }
    }

    pub fn info(intent: Intent, title: &str, lines: Vec<(String, String)>) -> Self {
        Self { lines, info: true, ..Self::base(intent, title, "Close") }
    }

    /// A project / folder picker listing every project and its folder tree.
    pub fn pick(intent: Intent, title: &str, ok: &str, lib: &Library) -> Self {
        Self { choices: choices(lib), ..Self::base(intent, title, ok) }
    }

    /// Preselects a project (and folder) in the picker.
    pub fn pick_where(&mut self, project: &str, folder: Option<&str>) {
        self.picked = self.choices.iter().position(|c| c.project == project && c.folder.as_deref() == folder);
    }

    pub fn can_submit(&self) -> bool {
        self.info || (self.input.as_ref().is_none_or(|t| !t.trim().is_empty()) && (self.choices.is_empty() || self.picked.is_some()))
    }
}

/// Projects with their folders, depth-first, for the picker.
pub fn choices(lib: &Library) -> Vec<Choice> {
    fn walk(out: &mut Vec<Choice>, project: &designos_library::Project, parent: Option<&str>, depth: usize) {
        let mut kids: Vec<_> = project.folders.iter().filter(|f| f.parent.as_deref() == parent).collect();
        kids.sort_by_key(|f| f.name.to_lowercase());
        for f in kids {
            out.push(Choice { project: project.id.clone(), folder: Some(f.id.clone()), label: f.name.clone(), depth });
            walk(out, project, Some(&f.id), depth + 1);
        }
    }
    let mut out = vec![];
    for p in lib.list_projects() {
        out.push(Choice { project: p.id.clone(), folder: None, label: p.name.clone(), depth: 0 });
        walk(&mut out, &p, None, 1);
    }
    out
}

impl App {
    /// File -> "Save to library...": pick a project for the open document.
    pub fn begin_save_to_library(&mut self) {
        crate::decks::ui::flush_notes(self);
        let Some(lib) = self.lib.clone() else {
            self.show_toast("The projects library is unavailable", true);
            return;
        };
        let name = if self.doc.name.trim().is_empty() { "Untitled".to_string() } else { self.doc.name.clone() };
        let mut d = Dialog::pick(Intent::SaveToLibrary, "Save to library", "Save", &lib);
        d.input = Some(name);
        if d.choices.is_empty() {
            match lib.create_project("My project") {
                Ok(_) => d.choices = choices(&lib),
                Err(e) => {
                    self.show_toast(e.to_string(), true);
                    return;
                }
            }
        }
        d.picked = Some(0);
        self.dialog = Some(d);
    }

    pub fn show_shortcuts(&mut self) {
        let rows = [
            ("Ctrl+S", "Save"),
            ("Ctrl+Shift+S", "Save as\u{2026}"),
            ("Ctrl+O", "Open a file"),
            ("Ctrl+K", "Command palette"),
            ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
            ("Ctrl+D", "Duplicate"),
            ("Ctrl+G / Ctrl+Shift+G", "Group / ungroup"),
            ("Shift+A", "Add auto layout"),
            ("Shift+D", "Toggle dev mode"),
            ("Shift+1 / Shift+2", "Zoom to fit / selection"),
            ("Shift+0", "Zoom to 100%"),
            ("Ctrl+\\", "Show or hide the UI"),
            ("Ctrl+Alt+Enter", "Present"),
            (crate::agent_a11y::FOCUS_SHORTCUT_LABEL, crate::agent_a11y::FOCUS_SHORTCUT_ACTION),
            ("Undo / Redo work while an agent terminal has focus (macOS; Windows and Linux for agent tabs)", ""),
        ];
        let lines = rows.iter().map(|(k, a)| (k.to_string(), a.to_string())).collect();
        self.dialog = Some(Dialog::info(Intent::Shortcuts, "Keyboard shortcuts", lines));
    }

    pub fn show_about(&mut self) {
        let mut lines = vec![
            ("design:os studio — native Rust design canvas for design:os & EaseUI.".to_string(), String::new()),
            (format!("Version {}", env!("CARGO_PKG_VERSION")), String::new()),
            ("Local-first, offline-ready. Your `.designos`, `.designos` and `.fig` files stay on this computer.".to_string(), String::new()),
        ];
        if let Some(lib) = &self.lib {
            lines.push((format!("Library: {}", lib.root().display()), String::new()));
        }
        self.dialog = Some(Dialog::info(Intent::About, "About design:os studio", lines));
    }

    pub fn confirm_open_github(&mut self) {
        self.dialog = Some(Dialog::confirm(
            Intent::OpenGithub,
            "Open the GitHub repository?",
            &["This opens the design:os studio source code page in your web browser.", super::GITHUB_URL],
            "Open",
        ));
    }

    /// Opens the in-app file path prompt when the system file chooser is unavailable (`#1192`).
    pub fn open_import_path_dialog(&mut self) {
        let mut d = Dialog::prompt(Intent::ImportPath, "Import file", "", "Import");
        d.hint = Some(IMPORT_PATH_HINT.into());
        self.dialog = Some(d);
    }
}

/// Draws the open dialog (if any) and runs its outcome. Call once per frame.
pub fn show(app: &mut App, ctx: &Context) {
    let Some(mut d) = app.dialog.take() else { return };
    let screen = ctx.content_rect();
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
    let (enter, esc) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape)));

    // ---- measure
    let measure = ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("dialog-measure")));
    let text_w = W - PAD * 2.0;
    let mut lines_h = 0.0;
    for (a, b) in &d.lines {
        lines_h += if b.is_empty() { measure.layout(a.clone(), font.clone(), theme::TEXT, text_w).size().y + 8.0 } else { 22.0 };
    }
    let visible_choices = d.choices.len().min(MAX_CHOICES);
    let mut h = PAD + 24.0 + 12.0 + lines_h;
    if d.input.is_some() {
        h += theme::CONTROL_SM + 12.0;
    }
    if !d.choices.is_empty() {
        h += visible_choices as f32 * ROW_H + 12.0;
    }
    h += BTN_H + PAD;
    let rect = Rect::from_center_size(screen.center() - Vec2::new(0.0, 24.0), Vec2::new(W, h));

    let mut submit = enter && d.can_submit() && d.safe_label.is_none();
    let mut cancel = esc || (enter && d.safe_label.is_some());
    let mut alt = false;
    egui::Area::new(Id::new("dialog")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        kit::modal_surface(&p, screen, rect);
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            cancel = true;
        }
        let x0 = rect.left() + PAD;
        let mut y = rect.top() + PAD;
        p.text(Pos2::new(x0, y + 12.0), Align2::LEFT_CENTER, &d.title, theme::font_text(theme::FONT_SIZE_TEXT_MD, theme::TextWeight::Semibold), theme::TEXT);
        y += 24.0 + 12.0;
        let key_font = if matches!(d.intent, Intent::Shortcuts) { theme::font_mono() } else { font.clone() };
        for (a, b) in &d.lines {
            if b.is_empty() {
                let galley = p.layout(a.clone(), font.clone(), theme::TEXT_TERTIARY, text_w);
                let hh = galley.size().y;
                p.galley(Pos2::new(x0, y), galley, theme::TEXT_TERTIARY);
                y += hh + 8.0;
            } else {
                p.text(Pos2::new(x0, y + 11.0), Align2::LEFT_CENTER, a, key_font.clone(), theme::TEXT);
                p.text(Pos2::new(x0 + 190.0, y + 11.0), Align2::LEFT_CENTER, b, font.clone(), theme::TEXT_SECONDARY);
                y += 22.0;
            }
        }
        if let Some(text) = d.input.as_mut() {
            let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(text_w, theme::CONTROL_SM));
            let id = Id::new("dialog-input");
            kit::paint_field(ui, r, id, kit::Size::Sm, false);
            let mut te = egui::TextEdit::singleline(text)
                .id(id)
                .font(font.clone())
                .text_color(theme::TEXT)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(10, 0))
                .vertical_align(egui::Align::Center)
                .min_size(r.size())
                .desired_width(r.width());
            if let Some(hint) = d.hint.as_deref() {
                te = te.hint_text(egui::RichText::new(hint).font(font.clone()).color(theme::TEXT_PLACEHOLDER));
            }
            // The hint colour is egui's weak text colour; point it at the placeholder role for this field only.
            ui.visuals_mut().weak_text_color = Some(theme::TEXT_PLACEHOLDER);
            let out = ui.put(r, te);
            if d.fresh {
                out.request_focus();
                let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                let n = text.chars().count();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                state.store(ui.ctx(), id);
                d.fresh = false;
            }
            y += theme::CONTROL_SM + 12.0;
        }
        if !d.choices.is_empty() {
            let list = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(text_w, visible_choices as f32 * ROW_H));
            p.rect_filled(list, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            let content = d.choices.len() as f32 * ROW_H;
            crate::panels::wheel_scroll(ui, list, &mut d.scroll, content);
            let scroll = d.scroll;
            let picked = d.picked;
            let mut new_pick = None;
            crate::panels::clipped(ui, list, |ui| {
                let p = ui.painter().clone();
                for (i, c) in d.choices.iter().enumerate() {
                    let row = Rect::from_min_size(Pos2::new(list.left(), list.top() + i as f32 * ROW_H - scroll), Vec2::new(list.width(), ROW_H));
                    if row.bottom() < list.top() || row.top() > list.bottom() {
                        continue;
                    }
                    let r = ui.interact(row, Id::new(("dialog-choice", i)), Sense::click());
                    let row_state = if picked == Some(i) {
                        kit::State::Selected
                    } else if r.hovered() {
                        kit::State::Hover
                    } else {
                        kit::State::Default
                    };
                    let row_paint = kit::row_look(row_state, kit::Surface::Primary);
                    kit::paint_frame(&p, row, theme::RADIUS_CONTROL, &row_paint);
                    let ix = row.left() + 12.0 + c.depth as f32 * 16.0;
                    crate::icons::paint(
                        &p,
                        if c.folder.is_some() { crate::icons::Icon::Folder } else { crate::icons::Icon::Frame },
                        Pos2::new(ix + 8.0, row.center().y),
                        16.0,
                        row_paint.icon,
                    );
                    p.text(Pos2::new(ix + 22.0, row.center().y), Align2::LEFT_CENTER, &c.label, font.clone(), row_paint.text);
                    if r.clicked() {
                        new_pick = Some(i);
                    }
                    if r.double_clicked() {
                        new_pick = Some(i);
                        submit = true;
                    }
                }
            });
            if new_pick.is_some() {
                d.picked = new_pick;
            }
            y += list.height() + 12.0;
        }
        // ---- buttons
        let label_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let ok_w = widgets::text_width(&p, &d.ok, &label_font).max(48.0) + 32.0;
        let ok_r = Rect::from_min_size(Pos2::new(rect.right() - PAD - ok_w, y), Vec2::new(ok_w, BTN_H));
        let enabled = d.can_submit();
        let kind = if d.danger { kit::Hierarchy::Destructive } else { kit::Hierarchy::Primary };
        if kit::button(ui, ok_r, "dialog-ok", &d.ok, kind, kit::Size::Md, enabled).clicked() {
            submit = true;
        }
        widgets::record(ui, "dialog-ok", ok_r);
        let mut left_of = ok_r.left();
        if let Some(alt_label) = d.alt_label.as_deref() {
            let aw = widgets::text_width(&p, alt_label, &label_font) + 32.0;
            let ar = Rect::from_min_size(Pos2::new(left_of - 8.0 - aw, y), Vec2::new(aw, BTN_H));
            if kit::button(ui, ar, "dialog-alt", alt_label, kit::Hierarchy::Secondary, kit::Size::Md, true).clicked() {
                alt = true;
            }
            widgets::record(ui, "dialog-alt", ar);
            left_of = ar.left();
        }
        if !d.info {
            let cancel_label = d.safe_label.as_deref().unwrap_or("Cancel");
            let cw = widgets::text_width(&p, cancel_label, &label_font) + 32.0;
            let cr = Rect::from_min_size(Pos2::new(left_of - 8.0 - cw, y), Vec2::new(cw, BTN_H));
            if kit::button(ui, cr, "dialog-cancel", cancel_label, kit::Hierarchy::Secondary, kit::Size::Md, true).clicked() {
                cancel = true;
            }
            widgets::record(ui, "dialog-cancel", cr);
        }
    });

    if d.intent == Intent::SaveChanges {
        // Three answers; the question is answered by exactly one of them, and Cancel is the quiet default.
        let answer = if alt {
            Some(super::leave::Answer::DontSave)
        } else if cancel {
            Some(super::leave::Answer::Cancel)
        } else if submit {
            Some(super::leave::Answer::Save)
        } else {
            None
        };
        match answer {
            Some(a) => app.answer_save_changes(ctx, a),
            None => app.dialog = Some(d),
        }
        return;
    }
    if d.info && (submit || cancel) {
        return;
    }
    if cancel {
        return;
    }
    if submit && d.can_submit() {
        if d.intent == Intent::DiskChanged {
            app.use_disk_version(ctx);
        } else {
            finish(app, d);
        }
        return;
    }
    app.dialog = Some(d);
}

/// Runs the confirmed dialog.
pub fn finish(app: &mut App, d: Dialog) {
    if crate::history::finish_dialog(app, &d) || crate::dev::finish_dialog(app, &d) {
        return;
    }
    if d.intent == Intent::ImportPath {
        let raw = d.input.unwrap_or_default();
        let shown = unquote_typed(&raw);
        if shown.is_empty() {
            return;
        }
        match typed_import_path(&raw) {
            Ok(path) => {
                if let Err(reason) = app.try_open_path(path.clone()) {
                    warn_import_path(app, &path, &reason);
                }
            }
            Err(reason) => {
                warn_import_path(app, Path::new(shown), &reason);
                app.show_toast(reason, true);
            }
        }
        return;
    }
    let Some(lib) = app.lib.clone() else { return };
    let text = d.input.clone().unwrap_or_default().trim().to_string();
    let choice = d.picked.and_then(|i| d.choices.get(i)).cloned();
    let mut dirty = true;
    let res: designos_library::Result<()> = match d.intent.clone() {
        Intent::NewProject => lib.create_project(&text).map(|p| {
            if let Some(h) = app.home.as_mut() {
                h.section = Section::Project(p.id, None);
                h.search.clear();
            }
        }),
        Intent::RenameProject(id) => lib.rename_project(&id, &text).map(|_| {
            if let Some(f) = app.lib_file.as_mut().filter(|f| f.project_id == id) {
                f.project = text.clone();
            }
        }),
        Intent::NewFolder(p, parent) => lib.create_folder(&p, parent.as_deref(), &text).map(|f| {
            if let Some(h) = app.home.as_mut() {
                h.collapsed.remove(&p);
                h.section = Section::Project(p, Some(f.id));
            }
        }),
        Intent::RenameFolder(p, f) => lib.rename_folder(&p, &f, &text),
        Intent::RenameFile(id) => lib.rename_file(&id, &text).map(|_| {
            if app.lib_file.as_ref().is_some_and(|f| f.id == id) {
                app.rename_library_file(&text);
            }
        }),
        Intent::MoveFile(id) => match choice {
            Some(c) => lib.move_file(&id, &c.project, c.folder.as_deref()).map(|_| {
                if let Some(lf) = app.lib_file.as_mut().filter(|f| f.id == id) {
                    lf.project_id = c.project.clone();
                    lf.project = lib.project(&c.project).map(|p| p.name).unwrap_or_default();
                }
            }),
            None => Ok(()),
        },
        Intent::SaveToLibrary => {
            dirty = false;
            if let Some(c) = choice {
                app.save_to_library_as(&c.project, c.folder.as_deref(), &text);
            }
            Ok(())
        }
        Intent::EmptyTrash => lib.empty_trash().inspect(|_| app.show_toast("Trash emptied", false)),
        Intent::OpenGithub => {
            dirty = false;
            if let Err(e) = super::open_link(super::GITHUB_URL) {
                app.show_toast(e, true);
            }
            Ok(())
        }
        Intent::Shortcuts
        | Intent::About
        | Intent::AuthorName
        | Intent::NameVersion(_)
        | Intent::OpenDevLink(_)
        | Intent::DiskChanged
        | Intent::SaveChanges
        | Intent::ImportPath => {
            dirty = false;
            Ok(())
        }
    };
    if let Err(e) = res {
        app.show_toast(e.to_string(), true);
    }
    if dirty {
        if let Some(h) = app.home.as_mut() {
            h.dirty = true;
        }
    }
}

const TYPE_THE_FULL_PATH: &str = "Type the full path to the file, starting with / or ~/.";

/// The path a person typed into the import prompt, before `~` and `file://` resolution.
fn unquote_typed(raw: &str) -> &str {
    let trimmed = raw.trim();
    trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')).or_else(|| trimmed.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))).unwrap_or(trimmed)
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(home) = TEST_IMPORT_HOME.with(|slot| slot.borrow().clone()) {
        return Some(home);
    }
    crate::io::user_home()
}

/// One line on stderr when a typed import path cannot be opened. The error toast still shows.
fn warn_import_path(app: &mut App, path: &Path, reason: &str) {
    let line = format!("designos: warn: import path {}: {reason}", path.display());
    eprintln!("{line}");
    app.last_dialog_warn = Some(line);
}

fn strip_file_scheme(text: &str) -> Option<&str> {
    const SCHEME: &str = "file://";
    if text.len() >= SCHEME.len() && text[..SCHEME.len()].eq_ignore_ascii_case(SCHEME) {
        Some(&text[SCHEME.len()..])
    } else {
        None
    }
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decodes `%XX` in a `file://` path. A `%` that is not two hex digits stays as written.
fn percent_decode(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(hi), Some(lo)) = (bytes.get(i + 1).copied().and_then(hex_val), bytes.get(i + 2).copied().and_then(hex_val)) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| "The file link is not valid text.".to_string())
}

/// `file:///tmp/a%20b.fig` is `/tmp/a b.fig`. On Windows, `file:///C:/...` drops the extra slash.
fn path_from_file_url(decoded: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let trimmed = decoded.strip_prefix('/').filter(|rest| rest.as_bytes().get(1) == Some(&b':')).unwrap_or(decoded);
        return PathBuf::from(trimmed);
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(decoded)
    }
}

/// `~` or `~/…` (and `~\…` on Windows) becomes the home folder. Anything else is `None`.
fn expand_tilde(text: &str) -> Result<Option<PathBuf>, String> {
    let rest = if text == "~" {
        ""
    } else if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
        rest
    } else {
        return Ok(None);
    };
    let Some(home) = home_dir() else {
        return Err("The home folder is not set.".to_string());
    };
    if rest.is_empty() {
        Ok(Some(home))
    } else {
        Ok(Some(home.join(rest)))
    }
}

/// Resolves a path typed into the import prompt.
///
/// Trims, strips one pair of matching quotes, expands `~` / `~/…` to the home folder, and turns a
/// `file://` link (as pasted from a file manager) into a path with `%XX` decoded. A path that is
/// still not absolute is refused: the prompt has no shell to expand a relative path.
pub(crate) fn typed_import_path(raw: &str) -> Result<PathBuf, String> {
    let text = unquote_typed(raw);
    if text.is_empty() {
        return Err(TYPE_THE_FULL_PATH.to_string());
    }
    let path = if let Some(rest) = strip_file_scheme(text) {
        path_from_file_url(&percent_decode(rest)?)
    } else if let Some(expanded) = expand_tilde(text)? {
        expanded
    } else {
        PathBuf::from(text)
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(TYPE_THE_FULL_PATH.to_string())
    }
}

#[cfg(test)]
thread_local! {
    static TEST_IMPORT_HOME: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Restores the typed-import home when the test ends, including on a panic.
#[cfg(test)]
pub struct ImportHomeGuard;

#[cfg(test)]
impl Drop for ImportHomeGuard {
    fn drop(&mut self) {
        TEST_IMPORT_HOME.with(|slot| *slot.borrow_mut() = None);
    }
}

/// This thread's home folder for `~/` in the import prompt. Other tests keep the process `HOME`.
#[cfg(test)]
pub fn set_import_home_for_test(home: PathBuf) -> ImportHomeGuard {
    TEST_IMPORT_HOME.with(|slot| *slot.borrow_mut() = Some(home));
    ImportHomeGuard
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_PATH: &str = "Type the full path to the file, starting with / or ~/.";

    /// An absolute path on every platform: `/home/designer` is not absolute on Windows.
    fn home() -> (PathBuf, ImportHomeGuard) {
        let home = std::env::temp_dir().join("designer-home");
        let guard = set_import_home_for_test(home.clone());
        (home, guard)
    }

    #[test]
    fn typed_import_path_expands_tilde_file() {
        let (home, _guard) = home();
        assert_eq!(typed_import_path("~/x.fig").unwrap(), home.join("x.fig"));
    }

    #[test]
    fn typed_import_path_expands_tilde_alone() {
        let (home, _guard) = home();
        assert_eq!(typed_import_path("~").unwrap(), home);
    }

    #[test]
    fn typed_import_path_expands_quoted_tilde_with_space() {
        let (home, _guard) = home();
        assert_eq!(typed_import_path("'~/a b.fig'").unwrap(), home.join("a b.fig"));
    }

    // A Unix file URL; Windows drive-letter URLs (`file:///C:/…`) are not covered here.
    #[cfg(unix)]
    #[test]
    fn typed_import_path_decodes_file_url() {
        assert_eq!(typed_import_path("file:///tmp/a%20b.fig").unwrap(), PathBuf::from("/tmp/a b.fig"));
    }

    #[test]
    fn typed_import_path_refuses_relative() {
        let err = typed_import_path("docs/x.fig").unwrap_err();
        assert_eq!(err, FULL_PATH);
    }

    #[test]
    fn typed_import_path_keeps_absolute() {
        let abs = std::env::temp_dir().join("x.fig");
        assert_eq!(typed_import_path(abs.to_str().unwrap()).unwrap(), abs);
    }
}
