//! Window shell: lays out the panels, routes shortcuts and drives screenshots.

use crate::app::{App, MenuKind};
use crate::canvas::tools::Tool;
use crate::canvas::{self, interact};
use crate::cli::Options;
use crate::demo;
use crate::icons::Icon;
use crate::panels::menus::{self, Action};
use crate::panels::{self, left, rail, right, toolbar};
use crate::theme;
use crate::widgets;
use egui::{Align2, Context, Event, Key, Pos2, Rect, Vec2};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once an `--screenshot` run has written its PNG.
pub static SHOT_OK: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Default)]
pub struct MicroSeqHud {
    pub focus_page: designos_doc::Vec2,
    pub zoom: f32,
    pub cursor_page: designos_doc::Vec2,
    pub drag_start_page: Option<designos_doc::Vec2>,
    pub cursor_screen_override: Option<Pos2>,
    pub dragging: bool,
    pub badge_text: String,
    pub keycap: String,
    pub caption: String,
    pub accent_orange: bool,
}

struct MicroSeqCapture {
    slug: String,
    cur: usize,
    total: usize,
    base_doc: designos_doc::Document,
    hud: MicroSeqHud,
    wait_frames: u32,
    /// `--no-caption`: every recorded frame drops the top shortcut caption bar.
    no_caption: bool,
}

struct Shot {
    path: PathBuf,
    size: (u32, u32),
    /// Device pixel ratio: the PNG is `size * scale` pixels, laid out as `size` points.
    scale: f32,
    frames: u32,
    requested: bool,
    /// The screenshot is due this frame: decided before the frame is drawn, sent once it has
    /// been, so the request sees the canvas as this frame left it (`Shell::request_screenshot`).
    due: bool,
    /// Every frame drawn since the request showed the canvas at rest, so the one the window
    /// captured did too. A font or an agent edit that lands between the request and the capture
    /// is drawn with the capped compile first.
    captured_at_rest: bool,
    /// The request came after `SHOT_GPU_SETTLE_FRAMES`: its image is kept whatever it shows.
    last_try: bool,
    seq: Option<MicroSeqCapture>,
    /// --no-toast: transient toasts are never painted into the capture.
    no_toast: bool,
}

/// Frames a GPU `--screenshot` waits for the view to reach its at-rest frame before it
/// keeps whatever is on screen (the run closes after 240).
const SHOT_GPU_SETTLE_FRAMES: u32 = 180;

/// Height of the custom top window title bar (`🟠 design:os studio`).
pub const TOP_TITLE_BAR_HEIGHT: f32 = 28.0;

#[allow(dead_code)]
pub fn is_typing_in_agent_panel(app: &App, ctx: &Context) -> bool {
    let agent_rail_open = !app.ui.left_collapsed && matches!(app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
    ctx.egui_wants_keyboard_input() || app.terminal.is_capturing_keyboard(agent_rail_open)
}

pub struct Shell {
    app: App,
    shot: Option<Shot>,
    title: String,
    paste_chord: crate::clipboard::PasteChord,
}

impl Shell {
    pub fn new(cc: &eframe::CreationContext<'_>, mut opts: Options) -> Self {
        // A real launch always has a library; screenshot runs only touch it for the home screen or --open-id.
        if opts.library.is_none() && (opts.screenshot.is_none() || opts.home || opts.open_id.is_some()) {
            opts.library = Some(designos_library::Library::default_root());
        }
        opts.home = opts.wants_home();
        let screenshot = opts.screenshot.is_some();
        let panel_was_none = opts.panel.is_none();
        let has_file = opts.file.is_some() || opts.open_id.is_some();
        let (gpu_flag, library) = (opts.gpu, opts.library.clone());
        let mut shell = Self::build(&cc.egui_ctx, opts);
        if !screenshot && panel_was_none && !has_file {
            shell.app.ui.rail = Some(crate::app::RailItem::Agents);
            shell.app.ui.left_collapsed = false;
        }
        if let Some(rs) = &cc.wgpu_render_state {
            if let Some(max) = canvas::gpu::init(rs) {
                shell.app.canvas.gpu.available = true;
                shell.app.canvas.gpu.set_max_texture(max);
            }
        }
        // GPU canvas on by default; small test fixture screenshots stay on the deterministic CPU path unless --gpu
        let saved = library.as_deref().and_then(|l| crate::comments::settings::load_flag(l, "use_gpu"));
        shell.app.canvas.gpu.pref = if screenshot { gpu_flag || shell.app.doc.nodes.len() > 5_000 } else { gpu_flag || saved.unwrap_or(true) };
        shell.app.tabs.always_visible = true;
        if shell.app.home.is_none() {
            let mut tabs = std::mem::take(&mut shell.app.tabs);
            tabs.sync_from_app(&shell.app);
            shell.app.tabs = tabs;
        }
        shell
    }

    pub fn build(ctx: &Context, opts: Options) -> Self {
        theme::install_fonts(ctx);
        theme::apply(ctx);
        let mut app = App::new(match opts.fixture.as_deref() {
            Some("no-selection") => demo::reference_document(),
            Some("variables") => demo::variables_document(),
            Some("variables-library" | "variables-library-open") => demo::variables_library_document(),
            Some(crate::showcase::FIXTURE) => crate::showcase::document(),
            Some(crate::protovars::FIXTURE) => crate::protovars::document(),
            Some(crate::check_designs::FIXTURE) => crate::check_designs::fixture::document(),
            Some("deck") => crate::decks::demo::document(),
            Some("board") => crate::board::demo::document(),
            Some("board-empty") => crate::board::create::new_document("Untitled"),
            Some(f) if crate::vecfixture::FIXTURES.contains(&f) => crate::vecfixture::document(),
            None if opts.file.is_some() || opts.open_id.is_some() || opts.home => designos_doc::Document::new("Untitled"),
            _ => demo::demo_document(),
        });
        // A shell started during build (`--panel agents`, `terminal`, `agents-chat`) captures its
        // session directory once; bind the terminal to the id the tab bar keeps before any can start.
        let session_id = app.tabs.active_session_id();
        app.terminal.bind_session(&session_id);
        if !matches!(
            opts.fixture.as_deref(),
            Some("variables" | "variables-library" | "variables-library-open" | "deck" | crate::showcase::FIXTURE | crate::check_designs::FIXTURE)
        ) {
            app.doc.name = "Untitled".into();
        }
        if opts.fixture.as_deref() == Some("deck") {
            if let Some(id) = app.deck_slides().first().copied() {
                app.select_slide(id);
            }
        }
        if opts.fixture.as_deref() == Some("comments") {
            crate::comments::fixture::apply(&mut app);
        }
        if let Some(f) = opts.fixture.as_deref().filter(|f| crate::vecfixture::FIXTURES.contains(f)) {
            crate::vecfixture::apply(&mut app, f);
        }
        if let Some(root) = opts.library.clone() {
            app.attach_library(root);
        }
        if let Some(f) = &opts.file {
            app.open_path(f.clone());
        }
        if let Some(id) = &opts.open_id {
            app.open_library_file(id);
        } else if opts.home {
            app.go_home();
        }
        if let Some(name) = opts.select.as_ref().filter(|n| n.as_str() != "none") {
            // a top-level layer (screen) wins over same-named layers deeper in the tree
            let named =
                |top: bool| app.doc.nodes.values().find(|n| &n.name == name && (!top || n.parent.is_some_and(|p| app.doc.pages.contains(&p)))).map(|n| n.id);
            if let Some(id) = named(true).or_else(|| named(false)) {
                if let Some(pg) = app.doc.page_of(id) {
                    app.doc.ensure_page_hydrated(pg);
                    app.page = pg;
                }
                app.selection = vec![id];
            }
        }
        if let Some(spec) = &opts.menu {
            let (w, h) = opts.size.unwrap_or((theme::DEFAULT_WINDOW.0 as u32, theme::DEFAULT_WINDOW.1 as u32));
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(w as f32, h as f32));
            if !menus::preset_menu(ctx, &mut app, spec, screen) {
                eprintln!("designos: unknown menu '{spec}'");
            }
        }
        if let Some(l) = &opts.dev {
            app.ui.dev_mode = true;
            app.ui.right_tab = Some(crate::app::RightTab::Design);
            app.dev.lang = match l.as_str() {
                "tailwind" => crate::dev::Language::Tailwind,
                "swiftui" => crate::dev::Language::SwiftUi,
                "uikit" => crate::dev::Language::UiKit,
                "compose" => crate::dev::Language::Compose,
                "android-xml" => crate::dev::Language::AndroidXml,
                "react" => crate::dev::Language::React,
                _ => crate::dev::Language::Css,
            };
            if opts.dev_demo {
                crate::dev::preset::apply(&mut app);
            }
        }
        app.ui.rulers = opts.rulers || !cfg!(test);
        if opts.grid {
            app.ui.pixel_grid = true;
        }
        app.startup_zoom = opts.zoom.map(|z| z / 100.0);
        if let Some(kind) = &opts.layout_grid {
            let target = app.selection.first().copied();
            if let Some(id) = target {
                let k = ["grid", "columns", "rows"].iter().position(|n| n == kind).unwrap_or(1);
                // screenshot aid: a red 10% grid, columns with the usual 5 x 20px gutter
                let grid = crate::panels::layoutguide::default_grid(k, designos_doc::Color::rgba(255, 0, 0, 26));
                app.edit_nodes(&[id], |n| {
                    if let Some(f) = n.kind.frame_props_mut() {
                        f.grids.push(grid.clone());
                    }
                });
            }
        }
        if let Some(name) = &opts.edit_text {
            let found = app.doc.nodes.values().find(|n| &n.name == name && matches!(n.kind, designos_doc::NodeKind::Text(_))).map(|n| n.id);
            if let Some(id) = found {
                if let Some(pg) = app.doc.page_of(id) {
                    app.doc.ensure_page_hydrated(pg);
                    app.page = pg;
                }
                canvas::interact::begin_text_edit(&mut app, id);
                if let (Some((a, b)), Some(sess)) = (opts.select_text, app.canvas.text.as_mut()) {
                    if let Some(designos_doc::NodeKind::Text(t)) = app.doc.get(id).map(|n| &n.kind) {
                        let byte = |c: usize| t.content.char_indices().nth(c).map(|(i, _)| i).unwrap_or(t.content.len());
                        sess.anchor = byte(a);
                        sess.caret = byte(b);
                    }
                }
            } else {
                eprintln!("designos: no text layer named '{name}'");
            }
        }
        if opts.tab.as_deref() == Some("prototype") {
            app.ui.right_tab = Some(crate::app::RightTab::Prototype);
        }
        if opts.present {
            app.start_present();
            if let Some(st) = &mut app.proto.present {
                st.start_fullscreen = opts.fullscreen;
            }
            if opts.panel.as_deref() == Some("present-variables") {
                if opts.fixture.as_deref() == Some(crate::protovars::FIXTURE) {
                    crate::protovars::prime_present(&mut app);
                } else if let Some(st) = app.proto.present.as_mut() {
                    st.show_vars = true;
                }
            }
        }
        if let Some(q) = &opts.find {
            app.find.query = q.clone();
            app.open_find();
            app.find.focus = false;
            if let Some(r) = &opts.replace_with {
                app.find.replacement = r.clone();
                app.find.show_replace = true;
            }
        }
        match opts.panel.as_deref() {
            Some("agents") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.ensure_default_tab(app.file_path.as_deref());
            }
            Some("agents-chat") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.submit_chat_prompt("Create a SaaS analytics card in Claude Code", app.file_path.as_deref());
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
                while std::time::Instant::now() < deadline {
                    app.terminal.sync_chat_from_pty();
                    if app.terminal.chat_turns.last().is_some_and(|t| !t.text.trim().is_empty()) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
            }
            Some("agents-start-failure") => {
                // Real failure: the installed `agy` rejects the `--mcp-config` flag it never had (#642).
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.open_failed_start_fixture(app.file_path.as_deref());
            }
            Some("specs") => {
                app.ui.rail = Some(crate::app::RailItem::Specs);
                app.specs.open = true;
                app.specs.ensure_loaded();
            }
            Some("terminal") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.dock_open = true;
                app.terminal.ensure_default_tab(app.file_path.as_deref());
            }
            Some(p @ ("check-designs" | "check-designs-fixed")) => {
                app.open_check_designs();
                if p == "check-designs-fixed" {
                    app.refresh_check_designs(false);
                    let findings = app.check_designs.report.findings.clone();
                    app.check_designs_fix_all(&findings);
                    app.check_designs.invalidate();
                }
            }
            Some("cmdk") => {
                app.cmdk.open = true;
                app.cmdk.last_audit = Some(crate::cmdk::run_design_gate_audit(&app.doc, app.page, &app.selection));
            }
            Some("assets") => {
                app.ui.rail = Some(crate::app::RailItem::Assets);
            }
            Some("interaction") => {
                app.ui.right_tab = Some(crate::app::RightTab::Prototype);
                crate::protovars::open_editor(&mut app);
            }
            Some("present-variables") => {}
            Some("libraries") => app.open_libraries(),
            Some(panel @ ("plugins" | "plugin-install")) => {
                app.open_plugins();
                app.plugins.install_form = panel == "plugin-install";
                app.plugins.fresh = app.plugins.install_form;
            }
            Some("version-history") => crate::history::preset::apply(&mut app, opts.library.clone()),
            Some(p @ ("dev-annotate" | "dev-compare")) => crate::dev::preset::panel(&mut app, p),
            Some(panel @ ("variables" | "variables-floating")) => {
                app.vars.full = panel == "variables";
                app.open_variables();
                if opts.fixture.as_deref() == Some("variables-library-open") {
                    // the capture of a library collection: start on it rather than on the file's own
                    app.vars.collection = app.doc.variable_collections.values().find(|c| c.is_library()).map(|c| c.id.clone());
                }
            }
            Some(p) if p.starts_with("micro-") => {
                if !p.contains(":seq:") {
                    apply_micro_interaction_showcase_preset(&mut app, p);
                }
            }
            Some(spec) => {
                let (w, h) = opts.size.unwrap_or((theme::DEFAULT_WINDOW.0 as u32, theme::DEFAULT_WINDOW.1 as u32));
                panels::properties::preset_popup(&mut app, spec, Vec2::new(w as f32, h as f32));
            }
            None => {}
        }
        app.canvas.tiles.unlimited_budget = opts.screenshot.is_some();
        let mut seq_capture = None;
        if let Some(p) = opts.panel.as_deref() {
            if let Some((slug, rest)) = p.split_once(":seq:") {
                let total = rest.trim().parse::<usize>().unwrap_or(60).max(2);
                if opts.size.is_some_and(|(w, h)| w < h) {
                    app.ui.left_collapsed = true;
                }
                let base_doc = app.doc.clone();
                let mut hud = apply_micro_interaction_continuous_frame(&mut app, &base_doc, slug, 0, total);
                if opts.no_caption {
                    hud.caption.clear();
                }
                seq_capture = Some(MicroSeqCapture { slug: slug.to_string(), cur: 0, total, base_doc, hud, wait_frames: 0, no_caption: opts.no_caption });
            }
        }
        let shot = opts.screenshot.map(|path| Shot {
            path,
            size: opts.size.unwrap_or((theme::DEFAULT_WINDOW.0 as u32, theme::DEFAULT_WINDOW.1 as u32)),
            scale: opts.scale.unwrap_or(1.0),
            frames: 0,
            requested: false,
            due: false,
            captured_at_rest: true,
            last_try: false,
            seq: seq_capture,
            no_toast: opts.no_toast,
        });
        Self { app, shot, title: String::new(), paste_chord: Default::default() }
    }

    fn drive_screenshot(&mut self, ctx: &Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        ctx.request_repaint();
        if shot.frames == 1 {
            // `scale` physical pixels per egui point (1 by default, so the PNG matches the requested size).
            ctx.set_pixels_per_point(shot.scale);
            let npp = ctx.input(|i| i.viewport().native_pixels_per_point).unwrap_or(1.0).max(0.1);
            let phys = Vec2::new(shot.size.0 as f32 * shot.scale, shot.size.1 as f32 * shot.scale);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(phys / npp));
        }
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            if let Event::Screenshot { image, .. } = ev {
                if !shot.captured_at_rest {
                    if !shot.last_try {
                        // Captured from a capped frame: ask again for the at-rest one.
                        shot.requested = false;
                        return;
                    }
                    eprintln!("designos: the GPU view did not settle in {SHOT_GPU_SETTLE_FRAMES} frames, saving the interactive frame");
                }
                let (w, h) = (image.width() as u32, image.height() as u32);
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w, h, raw) {
                    let want = ((shot.size.0 as f32 * shot.scale).round() as u32, (shot.size.1 as f32 * shot.scale).round() as u32);
                    let buf = if (w, h) != want { image::imageops::resize(&buf, want.0, want.1, image::imageops::FilterType::Lanczos3) } else { buf };
                    if let Some(seq) = &mut shot.seq {
                        let _ = std::fs::create_dir_all(&shot.path);
                        let frame_path = shot.path.join(format!("frame-{:04}.jpg", seq.cur));
                        let rgb = image::DynamicImage::ImageRgba8(buf).into_rgb8();
                        if let Err(e) = rgb.save(&frame_path) {
                            eprintln!("designos: cannot write {}: {e}", frame_path.display());
                        }
                        seq.cur += 1;
                        if seq.cur < seq.total {
                            seq.hud = apply_micro_interaction_continuous_frame(&mut self.app, &seq.base_doc, &seq.slug, seq.cur, seq.total);
                            if seq.no_caption {
                                seq.hud.caption.clear();
                            }
                            shot.requested = false;
                            seq.wait_frames = 2;
                            shot.frames = 12;
                            return;
                        } else {
                            SHOT_OK.store(true, Ordering::SeqCst);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            return;
                        }
                    } else {
                        if let Some(dir) = shot.path.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        match buf.save(&shot.path) {
                            Ok(()) => SHOT_OK.store(true, Ordering::SeqCst),
                            Err(e) => eprintln!("designos: cannot write {}: {e}", shot.path.display()),
                        }
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        if let Some(seq) = &mut shot.seq {
            if seq.wait_frames > 0 {
                seq.wait_frames -= 1;
                return;
            }
        }
        let sized = {
            let r = ctx.content_rect();
            (r.width() - shot.size.0 as f32).abs() < 1.5 && (r.height() - shot.size.1 as f32).abs() < 1.5
        };
        shot.due = !shot.requested && ((shot.frames >= 12 && sized) || shot.frames >= 60);
        if shot.frames > 240 && shot.seq.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Sends the screenshot request that `drive_screenshot` found due, once this frame has drawn.
    /// A GPU canvas first shows the capped interactive frame; the full-quality frame of the at-rest
    /// view follows once the idle refine has run. Ask for that one, but never wait past the point
    /// where the run would give up (`shot.frames > 240` in `drive_screenshot`).
    fn request_screenshot(&mut self, ctx: &Context) {
        let Some(shot) = &mut self.shot else { return };
        let at_rest = self.app.canvas.gpu.at_rest();
        if shot.requested {
            // The window captures a later frame than the one that asked: if any of the frames
            // until the image arrives was capped, so may be the image.
            shot.captured_at_rest &= at_rest;
            return;
        }
        if !std::mem::take(&mut shot.due) || (!at_rest && shot.frames < SHOT_GPU_SETTLE_FRAMES) {
            return;
        }
        shot.requested = true;
        shot.captured_at_rest = at_rest;
        shot.last_try = shot.frames >= SHOT_GPU_SETTLE_FRAMES;
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    }

    fn handle_keys(&mut self, ctx: &Context) {
        self.paste_chord.begin_frame(ctx.input(|i| i.modifiers.command));
        if self.app.plugins.open || self.app.dialog.is_some() || self.app.history.save_modal.is_some() || self.app.dev.compare.is_some() {
            return;
        }
        let agent_rail_open = !self.app.ui.left_collapsed && matches!(self.app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
        let capturing_terminal = self.app.terminal.is_capturing_keyboard(agent_rail_open);
        if capturing_terminal {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        if self.app.vars.open {
            // The Variables view handles its own keys (and undo / redo).
            return;
        }
        // Open menus own the keyboard (arrows, Enter, Escape, typing in the palette).
        if self.app.ui.menu.is_some() || self.app.ui.ctx_menu.is_some() || self.app.vars.open {
            return;
        }
        let editing_text = self.app.canvas.text.is_some();
        let eyedropper_active = self.app.props.eyedropper.is_some() || self.app.props.picker.as_ref().is_some_and(|pk| pk.eyedrop);
        if typing && !editing_text && !eyedropper_active {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            // egui-winit emits `Event::Paste` only for a non-empty `text/plain` and never a V press for a paste chord;
            // a Figma copy can carry HTML alone, which only the V release reveals (see `PasteChord`).
            if self.paste_chord.on_event(&ev) && !editing_text {
                if let Some(html) = (self.app.system_html)() {
                    self.app.paste_with_html(None, Some(&html));
                }
                continue;
            }
            match ev {
                Event::Copy if !editing_text => {
                    if let Some(s) = self.app.copy_selection() {
                        ctx.copy_text(s);
                    }
                }
                Event::Cut if !editing_text => {
                    if let Some(s) = self.app.cut_selection() {
                        ctx.copy_text(s);
                    }
                }
                Event::Paste(s) if !editing_text => {
                    self.app.paste_from_system(Some(&s));
                }
                Event::Key { key, pressed: true, modifiers, .. } => {
                    if self.try_typography_shortcut(key, modifiers) {
                        continue;
                    }
                    if editing_text {
                        continue;
                    }
                    self.key(ctx, key, modifiers);
                }
                _ => {}
            }
        }
    }

    /// Handles Figma typography keyboard shortcuts (`S2-09`) when editing text or when a Text node is selected:
    /// - `Cmd/Ctrl + Shift + .` / `,`: font size `±1px` (`Alt` -> `±10px`)
    /// - `Cmd/Ctrl + Alt + L / T / R / J`: horizontal text alignment (`Left`, `Center`, `Right`, `Justified`)
    /// - `Alt + ArrowUp / ArrowDown`: line height `±1px` (`Shift` -> `±10px`)
    /// - `Alt + ArrowLeft / ArrowRight`: letter spacing `±0.5px` (`Shift` -> `±2.0px`)
    fn try_typography_shortcut(&mut self, key: Key, m: egui::Modifiers) -> bool {
        let app = &mut self.app;
        let has_text =
            app.canvas.text.is_some() || app.selection.iter().any(|id| app.doc.get(*id).is_some_and(|n| matches!(n.kind, designos_doc::NodeKind::Text(_))));
        if !has_text {
            return false;
        }
        let sel = app.selection.clone();
        if m.command && m.shift && matches!(key, Key::Period | Key::Comma) {
            let delta = if m.alt { 10.0 } else { 1.0 } * if key == Key::Period { 1.0 } else { -1.0 };
            crate::panels::typography::set_style(app, &sel, |st| {
                st.font_size = (st.font_size + delta).clamp(1.0, 1000.0);
            });
            return true;
        }
        if m.command && m.alt && !m.shift {
            let align = match key {
                Key::L => Some(designos_doc::TextAlignH::Left),
                Key::T => Some(designos_doc::TextAlignH::Center),
                Key::R => Some(designos_doc::TextAlignH::Right),
                Key::J => Some(designos_doc::TextAlignH::Justified),
                _ => None,
            };
            if let Some(a) = align {
                if app.canvas.text.is_some() {
                    crate::canvas::textsession::base_style(app, |s| s.align_h = a);
                } else {
                    app.edit_nodes(&sel, |n| {
                        if let designos_doc::NodeKind::Text(t) = &mut n.kind {
                            t.style.align_h = a;
                            n.size = crate::layout::fit_text(t, n.size);
                        }
                    });
                }
                return true;
            }
        }
        if !m.command && m.alt {
            match key {
                Key::ArrowUp | Key::ArrowDown => {
                    let step = if m.shift { 10.0 } else { 1.0 } * if key == Key::ArrowUp { 1.0 } else { -1.0 };
                    crate::panels::typography::set_style(app, &sel, |st| {
                        let base_px = crate::richtext::line_height_px(st.line_height, st.font_size).unwrap_or_else(|| (st.font_size * 1.2).round());
                        st.line_height = designos_doc::LineHeight::Pixels((base_px + step).max(1.0));
                    });
                    return true;
                }
                Key::ArrowLeft | Key::ArrowRight => {
                    let step = if m.shift { 2.0 } else { 0.5 } * if key == Key::ArrowRight { 1.0 } else { -1.0 };
                    crate::panels::typography::set_style(app, &sel, |st| {
                        st.letter_spacing = ((st.letter_spacing + step) * 100.0).round() / 100.0;
                    });
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn key(&mut self, ctx: &Context, key: Key, m: egui::Modifiers) {
        let app = &mut self.app;
        let cmd = m.command;
        if cmd {
            let act = match (key, m.shift, m.alt) {
                (Key::E, false, _) => Some(Action::Flatten),
                (Key::O, false, true) => Some(Action::OutlineStroke),
                (Key::J, false, false) => Some(Action::VectorJoin),
                (Key::J, true, false) => Some(Action::VectorSplit),
                (Key::Z, false, _) => Some(Action::Undo),
                (Key::Z, true, _) | (Key::Y, false, _) => Some(Action::Redo),
                (Key::O, false, _) => Some(Action::Open),
                (Key::S, false, true) => Some(Action::SaveVersion),
                (Key::S, false, _) => Some(Action::Save),
                (Key::S, true, _) => Some(Action::SaveAs),
                (Key::D, false, _) => Some(Action::Duplicate),
                (Key::C, false, true) => Some(Action::CopyProperties),
                (Key::V, false, true) => Some(Action::PasteProperties),
                (Key::V, true, false) => Some(Action::PasteOverSelection),
                (Key::R, true, _) => Some(Action::PasteToReplace),
                (Key::G, false, false) => Some(Action::Group),
                (Key::G, true, _) => Some(Action::Ungroup),
                (Key::G, false, true) => Some(Action::FrameSelection),
                (Key::K, false, true) => Some(Action::CreateComponent),
                (Key::B, false, true) => Some(Action::Detach),
                (Key::K, true, false) => Some(Action::CombineVariants),
                (Key::A, true, true) => {
                    if let Ok(res) = designos_doc::changelog_features::suggest_auto_layout(&mut app.doc, &app.selection) {
                        app.selection = vec![res.root_frame];
                        app.canvas.tiles.invalidate_all();
                        app.changed();
                        app.show_toast(format!("Suggested Auto-Layout ({:?})", res.kind), false);
                    }
                    None
                }
                (Key::A, true, false) => Some(Action::SelectInverse),
                (Key::A, false, true) => {
                    if app.select_matching_layers() == 0 {
                        return menus::run(app, ctx, Action::SelectMatching);
                    }
                    None
                }
                (Key::A, false, _) => Some(Action::SelectAll),
                (Key::F, false, false) => Some(Action::Find),
                (Key::Quote, false, _) => Some(Action::TogglePixelGrid),
                (Key::Quote, true, _) => Some(Action::ToggleSnapPixel),
                (Key::Num4, true, _) => Some(Action::ToggleLayoutGrids),
                (Key::B, false, false) => Some(Action::TextBold),
                (Key::I, false, false) => Some(Action::TextItalic),
                (Key::U, false, false) => Some(Action::TextUnderline),
                (Key::Period, true, false) => Some(Action::TextSizeUp),
                (Key::Comma, true, false) => Some(Action::TextSizeDown),
                (Key::Num8, true, false) => Some(Action::TextBulletList),
                (Key::Num7, true, false) => Some(Action::TextNumberedList),
                (Key::CloseBracket, false, true) => Some(Action::BringToFront),
                (Key::OpenBracket, false, true) => Some(Action::SendToBack),
                (Key::CloseBracket, false, false) => Some(Action::BringForward),
                (Key::OpenBracket, false, false) => Some(Action::SendBackward),
                (Key::CloseBracket, true, _) => Some(Action::BringToFront),
                (Key::OpenBracket, true, _) => Some(Action::SendToBack),
                (Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown, shift, false) => {
                    let step = if shift { theme::NUDGE_BIG } else { theme::NUDGE };
                    let (dw, dh) = match key {
                        Key::ArrowLeft => (-step, 0.0),
                        Key::ArrowRight => (step, 0.0),
                        Key::ArrowUp => (0.0, -step),
                        _ => (0.0, step),
                    };
                    app.resize_selection_by(dw, dh);
                    None
                }
                (Key::Num0, false, false) => Some(Action::Zoom100),
                (Key::R, false, false) => {
                    app.rename_selection();
                    None
                }
                (Key::H, true, false) => {
                    app.toggle_selection_visible();
                    None
                }
                (Key::L, true, false) => {
                    app.toggle_selection_locked();
                    None
                }
                (Key::Equals | Key::Plus, _, _) => Some(Action::ZoomIn),
                (Key::Minus, _, _) => Some(Action::ZoomOut),
                (Key::Backslash, _, _) => Some(Action::ToggleSidebar),
                (Key::K, false, false) => Some(Action::OpenPalette),
                (Key::Backtick, _, _) => {
                    app.terminal.dock_open = !app.terminal.dock_open;
                    if app.terminal.dock_open {
                        app.terminal.ensure_default_tab(app.file_path.as_deref());
                    }
                    None
                }
                (Key::P, false, false) => {
                    app.cmdk.open = !app.cmdk.open;
                    if app.cmdk.open {
                        app.cmdk.last_audit = Some(crate::cmdk::run_design_gate_audit(&app.doc, app.page, &app.selection));
                    }
                    None
                }
                (Key::Enter, false, true) => {
                    app.start_present();
                    None
                }
                (Key::Tab, false, _) => {
                    app.next_tab();
                    None
                }
                (Key::Tab, true, _) => {
                    app.prev_tab();
                    None
                }
                (Key::T, false, false) => {
                    app.open_new_design_tab();
                    None
                }
                (Key::W, false, false) => {
                    if !app.tabs.entries.is_empty() {
                        let idx = app.tabs.active_idx;
                        app.close_tab(idx);
                    }
                    None
                }
                _ => None,
            };
            if let Some(a) = act {
                menus::run(app, ctx, a);
            }
            return;
        }
        if app.canvas.vedit.is_some() && canvas::vector::key(app, key, m) {
            return;
        }
        if app.canvas.builder.is_some() && matches!(key, Key::Escape | Key::Enter) {
            // a brush stroke in progress is dropped first; otherwise the result is applied
            if matches!(app.canvas.inter, canvas::Interaction::Builder) {
                canvas::builder::cancel_brush(app);
            } else {
                canvas::builder::finish(app);
                app.tool = Tool::Move;
            }
            return;
        }
        match key {
            Key::Delete | Key::Backspace => menus::run(app, ctx, Action::Delete),
            // an open comment draft or thread takes the Escape itself (egui drops text focus on Escape,
            // so the shortcut handler cannot tell that the key was meant for the text box)
            Key::Escape if app.comments.captures_escape() => {}
            Key::Escape => {
                if !matches!(app.canvas.inter, canvas::Interaction::None) {
                    app.cancel_interaction();
                } else if app.canvas.pen.is_some() {
                    interact::finish_pen(app);
                    app.tool = Tool::Move;
                } else if app.props.eyedropper.is_some() {
                    app.props.eyedropper = None;
                } else if app.ui.menu.is_some() {
                    app.ui.menu = None;
                } else if app.proto.editing.is_some() {
                    app.proto.editing = None;
                } else if app.find.open {
                    app.close_find();
                } else if app.layers.filter_open {
                    app.close_layer_filter();
                } else if app.tool != Tool::Move {
                    app.tool = Tool::Move;
                } else {
                    app.clear_selection();
                }
            }
            Key::Enter => {
                if app.canvas.pen.is_some() {
                    interact::finish_pen(app);
                    app.tool = Tool::Move;
                } else if m.shift {
                    if !crate::canvas::interact::navigate_hierarchy_enter(app, true) {
                        menus::run(app, ctx, Action::SelectParent);
                    }
                } else if let Some(n) = app.selection.first().filter(|_| app.selection.len() == 1).and_then(|s| app.doc.get(*s)) {
                    // A single text or vector layer is entered for editing; anything else selects its children.
                    let id = n.id;
                    if matches!(n.kind, designos_doc::NodeKind::Text(_))
                        || matches!(n.kind, designos_doc::NodeKind::Sticky(_) | designos_doc::NodeKind::ShapeWithText(_))
                    {
                        interact::begin_text_edit(app, id);
                    } else if matches!(n.kind, designos_doc::NodeKind::Connector(_)) {
                        crate::board::create::edit_label(app, id);
                    } else if canvas::vector::is_vector(app, id) {
                        // vectors and basic shapes (which become vectors on the first edit)
                        canvas::vector::enter(app, id);
                    } else if !crate::canvas::interact::navigate_hierarchy_enter(app, false) {
                        menus::run(app, ctx, Action::SelectChildren);
                    }
                } else if !crate::canvas::interact::navigate_hierarchy_enter(app, false) {
                    menus::run(app, ctx, Action::SelectChildren);
                }
            }
            // board: Tab starts the next sticky note to the right of the selected one; design: Tab/Shift+Tab traverses siblings
            Key::Tab if !m.alt => {
                if let Some(fid) = ctx.memory(|mem| mem.focused()) {
                    ctx.memory_mut(|mem| mem.surrender_focus(fid));
                }
                if app.props.eyedropper.is_some() || app.props.picker.as_ref().is_some_and(|pk| pk.eyedrop) {
                    app.props.eyedropper_format = app.props.eyedropper_format.next();
                } else if app.is_board() && !m.shift {
                    crate::board::create::tab_next(app);
                } else {
                    app.select_sibling(!m.shift);
                }
            }
            Key::Backslash => menus::run(app, ctx, Action::SelectParent),
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                if !app.selection.is_empty() {
                    let step = if m.shift { theme::NUDGE_BIG } else { theme::NUDGE };
                    let (dx, dy) = match key {
                        Key::ArrowLeft => (-step, 0.0),
                        Key::ArrowRight => (step, 0.0),
                        Key::ArrowUp => (0.0, -step),
                        _ => (0.0, step),
                    };
                    app.nudge(dx, dy);
                }
            }
            Key::Num1 if m.shift => menus::run(app, ctx, Action::FitAll),
            Key::Num2 if m.shift => menus::run(app, ctx, Action::FitSelection),
            Key::Num0 if m.shift => menus::run(app, ctx, Action::Zoom100),
            Key::A if m.shift && m.alt => app.remove_auto_layout(),
            Key::A if m.shift && !m.alt => menus::run(app, ctx, Action::AutoLayout),
            Key::A if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::Left)),
            Key::W if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::Top)),
            Key::S if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::Bottom)),
            Key::D if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::Right)),
            Key::H if m.shift && !m.alt => app.flip_selection(true),
            Key::V if m.shift && !m.alt => app.flip_selection(false),
            Key::H | Key::V if m.shift && m.alt => menus::run(app, ctx, Action::Distribute),
            Key::H if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::HCenter)),
            Key::V if m.alt && !m.shift => menus::run(app, ctx, Action::Align(crate::ops::Align::VCenter)),
            Key::OpenBracket if !m.shift && !m.alt => menus::run(app, ctx, Action::SendToBack),
            Key::CloseBracket if !m.shift && !m.alt => menus::run(app, ctx, Action::BringToFront),
            Key::N if !m.alt && !app.is_board() => app.step_frame(!m.shift, true),
            Key::Home if !m.alt => app.step_frame(false, false),
            Key::End if !m.alt => app.step_frame(true, false),
            Key::PageUp => app.step_page(false),
            Key::PageDown => app.step_page(true),
            Key::Num0 | Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5 | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9
                if !m.shift && !m.alt && !app.selection.is_empty() =>
            {
                let digit = match key {
                    Key::Num0 => 0,
                    Key::Num1 => 1,
                    Key::Num2 => 2,
                    Key::Num3 => 3,
                    Key::Num4 => 4,
                    Key::Num5 => 5,
                    Key::Num6 => 6,
                    Key::Num7 => 7,
                    Key::Num8 => 8,
                    Key::Num9 => 9,
                    _ => return,
                };
                app.apply_opacity_digit(digit);
            }
            Key::Q if !m.shift && !m.alt && !app.selection.is_empty() => {
                app.select_matching_layers();
            }
            Key::I if !m.shift && !m.alt && !app.selection.is_empty() => {
                app.props.eyedropper = Some(crate::panels::properties::EyedropTarget::Fill(0));
            }
            Key::R if m.shift => menus::run(app, ctx, Action::ToggleRulers),
            Key::Quote if m.shift => menus::run(app, ctx, Action::TogglePixelGrid),
            k => {
                if !m.alt {
                    let tool = if app.is_board() { Tool::from_key_board(k, m.shift) } else { Tool::from_key(k, m.shift) };
                    if let Some(t) = tool {
                        app.set_tool(t);
                    }
                }
            }
        }
    }

    /// Right-click on the canvas selects the layer under the pointer and opens the selection menu
    /// (or the `"Select layer"` picker menu when `Cmd/Ctrl` is held `#27`).
    fn context_menu(&mut self, ui: &egui::Ui, canvas_rect: Rect) {
        let (clicked, pos, cmd) = ui.ctx().input(|i| (i.pointer.secondary_clicked(), i.pointer.interact_pos(), i.modifiers.command));
        let Some(pos) = pos.filter(|p| clicked && canvas_rect.contains(*p) && !self.app.vars.open) else { return };
        if self.app.ui.rulers && !self.app.is_board() && (pos.x - canvas_rect.left() <= theme::RULER_SIZE || pos.y - canvas_rect.top() <= theme::RULER_SIZE) {
            return;
        }
        let app = &mut self.app;
        if !matches!(app.canvas.inter, canvas::Interaction::None) || app.canvas.text.is_some() {
            return;
        }
        let page = app.view.to_page(app.canvas_rect.min, pos);
        if cmd {
            let idx = interact::active_index(app);
            let candidates = crate::canvas::hit::hit_all_under(&app.doc, app.page, page, idx);
            if !candidates.is_empty() {
                app.ui.ctx_menu = None;
                app.ui.layer_picker = Some((pos, candidates));
                return;
            }
        }
        match interact::hit_under(app, page, false) {
            Some(hit) if !app.is_selected(hit) => app.select_only(hit),
            Some(_) => {}
            None => app.clear_selection(),
        }
        app.ui.ctx_menu = Some(pos);
    }

    fn toast(&mut self, ui: &egui::Ui, screen: Rect) {
        if self.shot.as_ref().is_some_and(|s| s.no_toast) {
            return;
        }
        let Some(t) = &self.app.toast else { return };
        if self.app.now > t.until {
            self.app.toast = None;
            return;
        }
        let p = ui.painter();
        let font = theme::font_body();
        let w = widgets::text_width(p, &t.text, &font) + 48.0;
        let r = Rect::from_center_size(
            Pos2::new(screen.center().x, screen.bottom() - theme::TOAST_BOTTOM - theme::TOAST_HEIGHT / 2.0),
            Vec2::new(w, theme::TOAST_HEIGHT),
        );
        widgets::elevation_shadow(p, r, theme::RADIUS_FRAME);
        p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        p.circle_filled(Pos2::new(r.left() + 16.0, r.center().y), 4.0, if t.error { theme::DANGER } else { theme::BG_BRAND });
        p.text(Pos2::new(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, &t.text, font, theme::TEXT);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
}

impl eframe::App for Shell {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.17, 0.17, 0.17, 1.0]
    }

    /// Tab belongs to the text session (it nests list items) or to sibling layer traversal on the
    /// canvas: take it out of the input before egui uses it to move keyboard focus between widgets.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.app.plugins.open {
            return;
        }
        if ctx.memory(|m| m.focused()).is_some() {
            return;
        }
        if let Some(sess) = self.app.canvas.text.as_mut() {
            raw.events.retain(|e| match e {
                egui::Event::Key { key: egui::Key::Tab, pressed, modifiers, .. } if !modifiers.command => {
                    if *pressed {
                        sess.tabs.push(modifiers.shift);
                    }
                    false
                }
                _ => true,
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.poll_fonts();
        if self.app.fonts.is_loading() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.draw(ui);
    }
}

impl Shell {
    /// Files dropped on the window: `.svg` files are placed at the drop point,
    /// anything else (the first file) is opened as a document.
    fn handle_drops(&mut self, ctx: &Context) {
        let (files, pos) = ctx.input(|i| (i.raw.dropped_files.clone(), i.pointer.hover_pos().or(i.pointer.latest_pos())));
        let is_svg = |f: &egui::DroppedFile| {
            let name = f.path.as_ref().and_then(|p| p.to_str()).unwrap_or(&f.name);
            name.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        };
        if let Some(path) = files.iter().filter(|f| !is_svg(f)).find_map(|f| f.path.clone()) {
            self.app.open_path(path);
            return;
        }
        for (nth, f) in files.iter().filter(|f| is_svg(f)).enumerate() {
            let text = match (&f.path, &f.bytes) {
                (Some(p), _) => std::fs::read_to_string(p).map_err(|e| format!("Cannot read {}: {e}", p.display())),
                (None, Some(b)) => String::from_utf8(b.to_vec()).map_err(|_| "The SVG is not valid UTF-8".to_string()),
                (None, None) => continue,
            };
            let stem = f
                .path
                .as_deref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .or_else(|| f.name.strip_suffix(".svg"))
                .filter(|s| !s.is_empty())
                .unwrap_or("svg")
                .to_string();
            match text {
                Ok(t) => crate::clipboard::drop_svg(&mut self.app, &t, &stem, pos, nth),
                Err(e) => self.app.show_toast(e, true),
            }
        }
    }

    fn sync_title(&mut self, ctx: &Context) {
        let title = theme::APP_TITLE.to_string();
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// One frame of the home screen (file browser): files dropped on the window are imported.
    fn home_frame(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() && self.app.dialog.is_none() {
            let target = self.app.home_target().or_else(|| self.app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))));
            if let Some((p, f)) = target {
                self.app.import_into_library(&dropped, &p, f.as_deref());
            }
        }
        crate::home::show(&mut self.app, ui);
        crate::home::dialog::show(&mut self.app, ctx);
        let full = ui.max_rect();
        self.toast(ui, full);
        self.sync_title(ctx);
    }

    /// One full UI pass over the window, then the screenshot request it may owe.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.draw_window(ui);
        let ctx = ui.ctx().clone();
        self.request_screenshot(&ctx);
    }

    fn draw_window(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.app.now = ctx.input(|i| i.time);
        self.app.poll_plugins();
        self.drive_screenshot(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            crate::decks::ui::flush_notes(&mut self.app);
            self.app.save_close_version();
            self.app.flush_autosave();
        }
        self.app.scrub_sync(&ctx);
        if self.app.step_pending_relayout() {
            ctx.request_repaint();
        }
        let window_full = ui.max_rect();
        let show_title_bar = self.app.tabs.always_visible && self.shot.is_none() && self.app.proto.present.is_none();
        let after_title_rect = if show_title_bar {
            let title_bar_rect = Rect::from_min_max(window_full.min, Pos2::new(window_full.right(), window_full.top() + TOP_TITLE_BAR_HEIGHT));
            let tp = ui.painter_at(title_bar_rect);
            tp.rect_filled(title_bar_rect, 0.0, egui::Color32::from_rgb(0x18, 0x18, 0x1B));
            tp.hline(title_bar_rect.x_range(), title_bar_rect.bottom() - 0.5, egui::Stroke::new(1.0, egui::Color32::from_rgb(0x27, 0x27, 0x2A)));
            let cy = title_bar_rect.center().y;
            let x = title_bar_rect.left() + 76.0;
            let font_bold = theme::font_body_strong();
            let font_reg = theme::font_body();
            let w_design = widgets::text_width(&tp, "design", &font_bold);
            let w_rest = widgets::text_width(&tp, ":os studio", &font_reg);
            let brand_rect = Rect::from_min_size(Pos2::new(x, title_bar_rect.top() + 2.0), Vec2::new(22.0 + w_design + w_rest + 8.0, 24.0));
            widgets::record(ui, "window-title-brand", brand_rect);
            crate::icons::paint(&tp, Icon::Logo, Pos2::new(x + 8.0, cy), 14.0, theme::BG_BRAND);
            let text_x = x + 19.0;
            tp.text(Pos2::new(text_x, cy), Align2::LEFT_CENTER, "design", font_bold, theme::TEXT);
            tp.text(Pos2::new(text_x + w_design, cy), Align2::LEFT_CENTER, ":os studio", font_reg, theme::TEXT_SECONDARY);
            let drag_resp = ui.interact(title_bar_rect, ui.id().with("window-title-bar-drag"), egui::Sense::click_and_drag());
            if drag_resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            Rect::from_min_max(Pos2::new(window_full.left(), title_bar_rect.bottom()), window_full.max)
        } else {
            window_full
        };
        let show_tab_bar = self.app.tabs.is_bar_visible(self.app.home.is_some()) && self.app.proto.present.is_none();
        let full = if show_tab_bar {
            let bar_rect =
                Rect::from_min_max(after_title_rect.min, Pos2::new(after_title_rect.right(), after_title_rect.top() + crate::tabs::TOP_TAB_BAR_HEIGHT));
            crate::tabs::show_top_tab_bar_ui(ui, bar_rect, &mut self.app);
            Rect::from_min_max(Pos2::new(after_title_rect.left(), bar_rect.bottom()), after_title_rect.max)
        } else {
            let mut tabs = std::mem::take(&mut self.app.tabs);
            tabs.reconcile_frame(&mut self.app);
            self.app.tabs = tabs;
            after_title_rect
        };
        if self.app.home.is_some() {
            if show_tab_bar {
                let mut home_ui = ui.new_child(egui::UiBuilder::new().max_rect(full));
                self.home_frame(&mut home_ui, &ctx);
            } else {
                self.home_frame(ui, &ctx);
            }
            crate::plugins::ui::show(&mut self.app, &ctx);
            return;
        }
        self.app.autosave_tick(&ctx);
        self.app.history_tick();
        self.app.dev_tick(&ctx);
        if self.app.proto.present.is_some() {
            crate::present::show(&mut self.app, ui);
            return;
        }
        self.handle_keys(&ctx);
        if !self.app.plugins.open {
            self.handle_drops(&ctx);
        }

        ui.painter().rect_filled(full, 0.0, theme::BG);
        let board = self.app.is_board();
        let right_w = if board { 0.0 } else { theme::RIGHT_PANEL_WIDTH + theme::BORDER_WIDTH };
        let rail_w = theme::RAIL_WIDTH + theme::BORDER_WIDTH;
        let is_ai_panel = matches!(self.app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
        let max_left_w = (full.width() - right_w - rail_w - 160.0).max(220.0);
        let base_panel_w =
            if is_ai_panel { self.app.ui.ai_panel_width.clamp(260.0, max_left_w) } else { self.app.ui.left_panel_width.clamp(200.0, max_left_w) };
        let panel_w = base_panel_w + theme::BORDER_WIDTH;
        let rail_rect = Rect::from_min_max(full.min, Pos2::new(full.left() + rail_w, full.bottom()));
        let left_rect = Rect::from_min_max(Pos2::new(rail_rect.right(), full.top()), Pos2::new(rail_rect.right() + panel_w, full.bottom()));
        let right_rect = Rect::from_min_max(Pos2::new(full.right() - right_w, full.top()), full.max);
        let canvas_left = if self.app.ui.left_collapsed { rail_rect.right() } else { left_rect.right() };
        let deck = self.app.is_deck();
        let notes_h = if deck { 132.0_f32.min(full.height() * 0.28) } else { 0.0 };
        let canvas_rect = Rect::from_min_max(Pos2::new(canvas_left, full.top()), Pos2::new(right_rect.left(), full.bottom() - notes_h));
        let notes_rect = Rect::from_min_max(Pos2::new(canvas_left, canvas_rect.bottom()), Pos2::new(right_rect.left(), full.bottom()));

        let full_vars = self.app.vars.open && self.app.vars.full;
        if self.shot.is_none() && (is_ai_panel || self.app.terminal.dock_open) {
            self.app.terminal.ensure_default_tab(self.app.file_path.as_deref());
        }
        crate::live_patch_apply::publish_live_context(&mut self.app);
        if let Some(ai_copy) = self.app.specs.last_ai_copy.take() {
            self.app.terminal.send_copy_for_ai(&ai_copy);
        }
        let bridge_actions = self.app.terminal.poll_live_agent_bridge();
        if !bridge_actions.is_empty() {
            execute_terminal_actions(&mut self.app, bridge_actions);
        }
        // Agent edits that arrived during a drag or text edit land once it has ended.
        crate::live_patch_apply::apply_pending_live_patches(&mut self.app);
        if full_vars {
            // The local-variables full view replaces the left panel, canvas and right panel.
            rail::show(&mut self.app, ui, rail_rect);
            let content = Rect::from_min_max(Pos2::new(rail_rect.right(), full.top()), full.max);
            crate::variables::view::show(&mut self.app, ui, content);
        } else {
            let specs_reader_open = false;
            let max_dock_h = (canvas_rect.height() - 100.0).max(200.0);
            let dock_h = if self.app.terminal.dock_open { self.app.terminal.dock_height.clamp(160.0, max_dock_h) } else { 0.0 };
            let main_canvas_rect =
                Rect::from_min_max(canvas_rect.min, Pos2::new(canvas_rect.right(), (canvas_rect.bottom() - dock_h).max(canvas_rect.top() + 80.0)));
            let dock_rect = Rect::from_min_max(Pos2::new(canvas_rect.left(), main_canvas_rect.bottom()), canvas_rect.max);
            if specs_reader_open {
                self.app.canvas_rect = main_canvas_rect;
                crate::specs::show_reader(&mut self.app, ui, main_canvas_rect);
            } else {
                if let Some(seq) = self.shot.as_ref().and_then(|s| s.seq.as_ref()) {
                    if seq.hud.zoom > 0.0 {
                        self.app.first_fit_done = true;
                        let aspect_scale = (main_canvas_rect.width() / 1080.0).clamp(0.70, 1.05);
                        let z = (seq.hud.zoom * aspect_scale).max(0.25);
                        self.app.view.zoom = z;
                        self.app.view.offset = designos_doc::Vec2::new(
                            seq.hud.focus_page.x - (main_canvas_rect.width() * 0.5) / z,
                            seq.hud.focus_page.y - (main_canvas_rect.height() * 0.5) / z,
                        );
                    }
                }
                canvas::show(&mut self.app, ui, main_canvas_rect);
                if deck {
                    crate::decks::ui::notes(&mut self.app, ui, notes_rect);
                }
            }
            if self.app.terminal.dock_open && dock_rect.height() >= 80.0 {
                let file_path = self.app.file_path.clone();
                let term_actions = crate::terminal::show_dock(&mut self.app.terminal, ui, dock_rect, file_path.as_deref());
                execute_terminal_actions(&mut self.app, term_actions);
                let dock_handle_rect =
                    Rect::from_min_max(Pos2::new(dock_rect.left(), dock_rect.top() - 3.0), Pos2::new(dock_rect.right(), dock_rect.top() + 3.0));
                widgets::record(ui, "dock-resize-handle", dock_handle_rect);
                let dock_drag = ui.interact(dock_handle_rect, ui.id().with("dock-resize-handle"), egui::Sense::drag());
                if dock_drag.hovered() || dock_drag.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    ui.painter().hline(dock_rect.x_range(), dock_rect.top(), egui::Stroke::new(2.0, theme::BG_BRAND));
                }
                if dock_drag.dragged() {
                    if let Some(pointer) = ctx.input(|i| i.pointer.interact_pos()) {
                        self.app.terminal.dock_height = (canvas_rect.bottom() - pointer.y).clamp(160.0, max_dock_h);
                    }
                }
            }
            rail::show(&mut self.app, ui, rail_rect);
            if self.app.ui.left_collapsed {
                let b = Rect::from_min_size(Pos2::new(canvas_rect.left() + 8.0, canvas_rect.top() + 8.0), Vec2::splat(theme::PANEL_ACTION_BUTTON));
                ui.painter().rect_filled(b, theme::cr(theme::RADIUS_CONTROL), theme::BG);
                if widgets::icon_button(ui, b, "show-sidebar", Icon::Sidebar, false, "Show sidebar").clicked() {
                    self.app.ui.left_collapsed = false;
                }
            } else if deck {
                let mode = Rect::from_min_size(left_rect.min, Vec2::new(left_rect.width(), 36.0));
                crate::decks::ui::mode_bar(&mut self.app, ui, mode);
                let body = Rect::from_min_max(Pos2::new(left_rect.left(), mode.bottom()), left_rect.max);
                if self.app.decks.design_mode {
                    left::show(&mut self.app, ui, body);
                } else {
                    crate::decks::ui::sidebar(&mut self.app, ui, body);
                }
            } else {
                left::show(&mut self.app, ui, left_rect);
            }
            if !self.app.ui.left_collapsed {
                let handle_rect =
                    Rect::from_min_max(Pos2::new(left_rect.right() - 3.0, left_rect.top()), Pos2::new(left_rect.right() + 3.0, left_rect.bottom()));
                widgets::record(ui, "left-panel-resize-handle", handle_rect);
                let drag_resp = ui.interact(handle_rect, ui.id().with("left-panel-resize-handle"), egui::Sense::drag());
                if drag_resp.hovered() || drag_resp.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    ui.painter().vline(left_rect.right() - 0.5, left_rect.y_range(), egui::Stroke::new(2.0, theme::BG_BRAND));
                }
                if drag_resp.dragged() {
                    if let Some(pointer) = ctx.input(|i| i.pointer.interact_pos()) {
                        let new_w = (pointer.x - rail_rect.right()).clamp(200.0, max_left_w);
                        if is_ai_panel {
                            self.app.ui.ai_panel_width = new_w.max(260.0);
                        } else {
                            self.app.ui.left_panel_width = new_w;
                        }
                    }
                }
            }
            if board {
                if !specs_reader_open && !self.app.terminal.dock_open {
                    crate::board::toolbar::show(&mut self.app, ui, canvas_rect_for_toolbar(full));
                    crate::board::bar::show(&mut self.app, ui, main_canvas_rect);
                    crate::board::canvas::zoom_pill(&mut self.app, ui, main_canvas_rect);
                }
            } else {
                if specs_reader_open {
                    crate::specs::show_right(&mut self.app, ui, right_rect);
                } else {
                    right::show(&mut self.app, ui, right_rect);
                }
                if !specs_reader_open {
                    let tb_rect = if deck || self.app.terminal.dock_open { main_canvas_rect } else { canvas_rect_for_toolbar(full) };
                    toolbar::show(&mut self.app, ui, tb_rect);
                }
            }
            if !specs_reader_open {
                crate::history::banner::show(&mut self.app, ui, main_canvas_rect);
                self.context_menu(ui, main_canvas_rect);
            }
            menus::show(&mut self.app, &ctx);
            panels::context::show(&mut self.app, &ctx);
            panels::check_designs::show(&mut self.app, &ctx);
            // the floating panel (or only the inspector's binding picker when closed)
            crate::variables::view::show(&mut self.app, ui, full);
            if let Some(seq) = self.shot.as_ref().and_then(|s| s.seq.as_ref()) {
                paint_micro_seq_hud(ui, &self.app, main_canvas_rect, right_rect, &seq.hud, seq.cur, seq.total);
            }
        }
        if let Some(cmdk_act) = crate::cmdk::show(&ctx, &mut self.app.cmdk, &self.app.doc, self.app.page, &self.app.selection) {
            execute_cmdk_action(&mut self.app, cmdk_act);
        }
        crate::home::dialog::show(&mut self.app, &ctx);
        crate::history::modal::show(&mut self.app, &ctx);
        crate::dev::compare::show(&mut self.app, &ctx);
        crate::libraries::show(&mut self.app, &ctx);
        crate::plugins::ui::show(&mut self.app, &ctx);
        self.toast(ui, full);

        self.sync_title(&ctx);
        if self.app.ui.menu == Some(MenuKind::Main) && self.app.ui.menu_anchor.is_none() {
            self.app.ui.menu = None;
        }
    }
}

pub fn execute_terminal_actions(app: &mut App, actions: Vec<crate::terminal::TerminalAction>) {
    for action in actions {
        match action {
            crate::terminal::TerminalAction::ApplySpecJson(json) => match serde_json::from_str::<designos_doc::NodeSpec>(&json) {
                Ok(spec) => {
                    app.apply_node_spec(&spec, "Agent Terminal");
                }
                Err(e) => {
                    app.show_toast(format!("Spec parse error: {e}"), true);
                }
            },
            crate::terminal::TerminalAction::ApplySpecFile(path) => match std::fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<designos_doc::NodeSpec>(&raw) {
                    Ok(spec) => {
                        let label = path.file_name().and_then(|s| s.to_str()).unwrap_or("Spec");
                        app.apply_node_spec(&spec, label);
                    }
                    Err(e) => app.show_toast(format!("Invalid spec JSON: {e}"), true),
                },
                Err(e) => app.show_toast(format!("Failed to read {}: {e}", path.display()), true),
            },
            crate::terminal::TerminalAction::ApplyLiveLog(lines) => {
                for error in app.terminal.live_patch.ingest(lines) {
                    app.show_toast(error, true);
                }
                crate::live_patch_apply::apply_pending_live_patches(app);
                if app.terminal.live_patch.has_held() {
                    app.terminal.status_msg = Some("Agent edit waits until you finish the current edit".into());
                }
            }
            crate::terminal::TerminalAction::LiveSyncError(msg) => app.show_toast(msg, true),
            crate::terminal::TerminalAction::ReloadFromDisk => {
                if let Some(p) = app.file_path.clone() {
                    app.open_path(p);
                    app.show_toast("Synced Canvas from disk", false);
                } else {
                    app.show_toast("No file path open to sync", true);
                }
            }
            crate::terminal::TerminalAction::ExportSelectionToTerminal => {
                match crate::terminal::export_selection_and_comments_to_pty(&mut app.terminal, &app.doc, app.page, &app.selection, app.file_path.as_deref()) {
                    Ok(p) => app.show_toast(format!("Sent selection & comments -> {}", p.display()), false),
                    Err(e) => app.show_toast(e, true),
                }
            }
        }
    }
}

pub fn execute_cmdk_action(app: &mut App, action: crate::cmdk::CmdKAction) {
    match action {
        crate::cmdk::CmdKAction::ApplySpec(spec) => {
            if !app.terminal.tabs.is_empty() {
                let fp = app.file_path.clone();
                app.terminal.dispatch_figma_ai_prompt("Make Design", &spec.name, fp.as_deref());
            }
            app.apply_node_spec(&spec, "Cmd+K Synthesizer");
        }
        crate::cmdk::CmdKAction::OpenSpecReader => {
            app.ui.rail = Some(crate::app::RailItem::Agents);
            app.ui.left_collapsed = false;
            app.specs.open = false;
            app.terminal.ensure_default_tab(app.file_path.as_deref());
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::OpenTerminal(preset) => {
            let file_path = app.file_path.clone();
            let _ = app.terminal.spawn_tab(preset, file_path.as_deref());
            app.terminal.dock_open = true;
            app.ui.rail = Some(crate::app::RailItem::Agents);
            app.ui.left_collapsed = false;
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::SendSelectionToTerminal => {
            app.terminal.dock_open = true;
            app.ui.rail = Some(crate::app::RailItem::Agents);
            app.ui.left_collapsed = false;
            match crate::terminal::export_selection_and_comments_to_pty(&mut app.terminal, &app.doc, app.page, &app.selection, app.file_path.as_deref()) {
                Ok(p) => app.show_toast(format!("Sent selection & comments -> {}", p.display()), false),
                Err(e) => app.show_toast(e, true),
            }
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::ExportFig => {
            let out_path = app.file_path.as_ref().map(|p| p.with_extension("fig")).unwrap_or_else(|| std::path::PathBuf::from("/tmp/designos-export.fig"));
            match designos_fig::export_fig(&app.doc) {
                Ok(bytes) => {
                    if std::fs::write(&out_path, bytes).is_ok() {
                        app.show_toast(format!("Exported Figma .fig to {}", out_path.display()), false);
                    }
                }
                Err(e) => app.show_toast(format!("Export .fig failed: {e}"), true),
            }
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::SelectNode(id) => {
            app.select_only(id);
            app.fit_selection();
        }
        crate::cmdk::CmdKAction::OpenCheckDesigns => app.open_check_designs(),
        crate::cmdk::CmdKAction::AutoFixDesignGate => {
            let sel = app.selection.clone();
            let page = app.page;
            let fixed_count = crate::cmdk::auto_fix_design_gate(&mut app.doc, page, &sel);
            app.changed();
            app.cmdk.last_audit = Some(crate::cmdk::run_design_gate_audit(&app.doc, app.page, &app.selection));
            if !app.terminal.tabs.is_empty() {
                let fp = app.file_path.clone();
                app.terminal.dispatch_figma_ai_prompt("Figma AI Action", &format!("Applied {fixed_count} Canvas changes (Undo with Cmd+Z)"), fp.as_deref());
            }
            app.show_toast(format!("Applied {fixed_count} Figma AI change(s) (Cmd+Z to Undo)"), false);
        }
    }
}

/// Configures `app` with a rich, deterministic visual showcase scene for each of the 8
/// Figma A-to-Z Micro-Interaction & Feature categories (`--panel micro-*`).
pub fn apply_micro_interaction_showcase_preset(app: &mut App, preset: &str) {
    use designos_doc::vector::End as VEnd;
    use designos_doc::{
        Affine, AutoLayout, Color, DevStatus, GridTrack, HandleMirror, LayoutAlign, LayoutDirection, Node, NodeKind, Paint, Rect as DRect, Sizing,
        Stroke as DStroke, Vec2 as DV,
    };
    app.ui.rulers = true;
    let page = app.page;
    let (base, step) = match preset.split_once(':') {
        Some((b, s)) => (b, s.parse::<usize>().unwrap_or(0)),
        None => (preset, 0),
    };
    match base {
        "micro-autolayout" => {
            type FlowCards = Vec<(&'static str, Color, Sizing)>;
            let (gap, pad, primary_sz, width, align_p, cards): (f32, [f32; 4], Sizing, f32, LayoutAlign, FlowCards) = match step {
                1 => (
                    12.0,
                    [16.0, 16.0, 16.0, 16.0],
                    Sizing::Hug,
                    420.0,
                    LayoutAlign::Start,
                    vec![
                        ("Badge · Hug", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Hug", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fixed),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
                2 => (
                    28.0,
                    [18.0, 18.0, 18.0, 18.0],
                    Sizing::Hug,
                    460.0,
                    LayoutAlign::Start,
                    vec![
                        ("Badge · Hug", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Gap 28", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fixed),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
                3 => (
                    36.0,
                    [20.0, 20.0, 20.0, 20.0],
                    Sizing::Hug,
                    480.0,
                    LayoutAlign::Start,
                    vec![
                        ("Badge · Hug", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Inline 36", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fixed),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
                4 => (
                    24.0,
                    [36.0, 44.0, 36.0, 44.0],
                    Sizing::Hug,
                    510.0,
                    LayoutAlign::Start,
                    vec![
                        ("Badge · Pad 44", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Fill", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fill),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
                5 => (
                    20.0,
                    [28.0, 32.0, 28.0, 32.0],
                    Sizing::Fixed,
                    560.0,
                    LayoutAlign::SpaceBetween,
                    vec![
                        ("Badge · Hug", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Fill Stretch", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fill),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
                6 => (
                    -18.0,
                    [28.0, 28.0, 28.0, 28.0],
                    Sizing::Hug,
                    420.0,
                    LayoutAlign::Start,
                    vec![
                        ("Stack · 01", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Stack · 02", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fixed),
                        ("Stack · 03", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                        ("Stack · 04", Color::rgb(0xFF, 0xFF, 0xFF), Sizing::Fixed),
                    ],
                ),
                _ => (
                    20.0,
                    [28.0, 28.0, 28.0, 28.0],
                    Sizing::Hug,
                    440.0,
                    LayoutAlign::Start,
                    vec![
                        ("Badge · Hug", Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                        ("Metric · Fill", Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fill),
                        ("Status · Ready", Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ],
                ),
            };
            let fid = app.doc.alloc_id();
            let mut frame = Node::frame(fid, DRect::new(2400.0, 200.0, width, 220.0));
            frame.name = "Auto-Layout 5.0 · Padding & Negative Gap".into();
            frame.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
            frame.corner_radii = [16.0; 4];
            if let Some(fp) = frame.kind.frame_props_mut() {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: gap,
                    padding: pad,
                    primary_sizing: primary_sz,
                    counter_sizing: Sizing::Hug,
                    primary_align: align_p,
                    ..Default::default()
                });
            }
            let fid = app.doc.insert(page, frame).unwrap();
            for (label, col, h_sz) in cards {
                let cid = app.doc.alloc_id();
                let mut card = Node::rectangle(cid, DRect::new(0.0, 0.0, 116.0, 124.0));
                card.name = label.into();
                card.fills = vec![Paint::solid(col)];
                card.corner_radii = [10.0; 4];
                card.layout_child.horizontal = h_sz;
                let _ = app.doc.insert(fid, card);
            }
            let mut ed = crate::edit::Edit::default();
            crate::layout::relayout_around(&mut app.doc, &mut ed, fid);
            app.selection = vec![fid];
            app.startup_zoom = Some(1.45);
            if matches!(step, 0 | 2 | 3) {
                let val_str = if step == 2 {
                    "28"
                } else if step == 3 {
                    "36"
                } else {
                    "20"
                };
                app.canvas.al_inline = Some(crate::canvas::AutoLayoutInlineEdit {
                    id: fid,
                    kind: crate::canvas::handles::AutoLayoutHandleKind::Gap(0),
                    anchor_page: DV::new(2544.0, 290.0),
                    text: val_str.into(),
                    request_focus: false,
                });
            } else if step == 4 {
                app.canvas.al_inline = Some(crate::canvas::AutoLayoutInlineEdit {
                    id: fid,
                    kind: crate::canvas::handles::AutoLayoutHandleKind::Padding(1),
                    anchor_page: DV::new(2422.0, 290.0),
                    text: "44".into(),
                    request_focus: false,
                });
            }
        }
        "micro-css-grid" => {
            type GridCellSpecs = Vec<(&'static str, Color, u16, u16, f32, f32)>;
            let (col1, col2, gap, pad, fw, fh, specs): (GridTrack, GridTrack, f32, f32, f32, f32, GridCellSpecs) = match step {
                1 => (
                    GridTrack::Flex(1.0),
                    GridTrack::Flex(1.0),
                    12.0,
                    20.0,
                    540.0,
                    320.0,
                    vec![
                        ("Hero 1×1", Color::rgb(0x0D, 0x99, 0xFF), 1, 1, 150.0, 120.0),
                        ("KPI Card", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 150.0, 120.0),
                        ("Users Cell", Color::rgb(0xFF, 0xFF, 0xFF), 1, 1, 150.0, 120.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 150.0, 120.0),
                        ("Retention", Color::rgb(0x27, 0x27, 0x2A), 1, 1, 150.0, 120.0),
                        ("Latency", Color::rgb(0x3F, 0x3F, 0x46), 1, 1, 150.0, 120.0),
                    ],
                ),
                2 => (
                    GridTrack::Fixed(160.0),
                    GridTrack::Flex(1.0),
                    14.0,
                    22.0,
                    550.0,
                    330.0,
                    vec![
                        ("Hero 1×1", Color::rgb(0x0D, 0x99, 0xFF), 1, 1, 160.0, 120.0),
                        ("KPI Fixed 160", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 160.0, 120.0),
                        ("Users Flex", Color::rgb(0xFF, 0xFF, 0xFF), 1, 1, 150.0, 120.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 150.0, 120.0),
                        ("Retention", Color::rgb(0x27, 0x27, 0x2A), 1, 1, 160.0, 120.0),
                        ("Latency", Color::rgb(0x3F, 0x3F, 0x46), 1, 1, 150.0, 120.0),
                    ],
                ),
                3 => (
                    GridTrack::Fixed(160.0),
                    GridTrack::Hug,
                    16.0,
                    24.0,
                    560.0,
                    340.0,
                    vec![
                        ("Hero Flex 1fr", Color::rgb(0x0D, 0x99, 0xFF), 1, 1, 180.0, 120.0),
                        ("KPI Fixed 160", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 160.0, 120.0),
                        ("Users Hug", Color::rgb(0xFF, 0xFF, 0xFF), 1, 1, 118.0, 120.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 180.0, 120.0),
                        ("Retention", Color::rgb(0x27, 0x27, 0x2A), 1, 1, 160.0, 120.0),
                        ("Latency Hug", Color::rgb(0x3F, 0x3F, 0x46), 1, 1, 118.0, 120.0),
                    ],
                ),
                4 => (
                    GridTrack::Fixed(160.0),
                    GridTrack::Hug,
                    16.0,
                    24.0,
                    560.0,
                    340.0,
                    vec![
                        ("Hero Span 2×1", Color::rgb(0x0D, 0x99, 0xFF), 2, 1, 260.0, 120.0),
                        ("KPI Card", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 130.0, 120.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 150.0, 120.0),
                        ("Retention", Color::rgb(0x27, 0x27, 0x2A), 1, 1, 160.0, 120.0),
                        ("Latency", Color::rgb(0xFF, 0xFF, 0xFF), 1, 1, 130.0, 120.0),
                    ],
                ),
                6 => (
                    GridTrack::Fixed(172.0),
                    GridTrack::Hug,
                    24.0,
                    32.0,
                    600.0,
                    368.0,
                    vec![
                        ("Hero Span 2×1", Color::rgb(0x0D, 0x99, 0xFF), 2, 1, 280.0, 132.0),
                        ("KPI Card", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 140.0, 132.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 160.0, 132.0),
                        ("Retention Span 2×1", Color::rgb(0xFF, 0xFF, 0xFF), 2, 1, 300.0, 132.0),
                    ],
                ),
                _ => (
                    GridTrack::Fixed(160.0),
                    GridTrack::Hug,
                    16.0,
                    24.0,
                    560.0,
                    340.0,
                    vec![
                        ("Hero Span 2×1", Color::rgb(0x0D, 0x99, 0xFF), 2, 1, 260.0, 120.0),
                        ("KPI Card", Color::rgb(0xE3, 0x63, 0x23), 1, 1, 130.0, 120.0),
                        ("Revenue Cell", Color::rgb(0x14, 0xAE, 0x5C), 1, 1, 150.0, 120.0),
                        ("Retention Cell", Color::rgb(0x27, 0x27, 0x2A), 2, 1, 280.0, 120.0),
                    ],
                ),
            };
            let fid = app.doc.alloc_id();
            let mut grid_frame = Node::frame(fid, DRect::new(2400.0, 600.0, fw, fh));
            grid_frame.name = "2D CSS Grid · Track Pills & Span 2×1".into();
            grid_frame.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
            grid_frame.corner_radii = [16.0; 4];
            let mut col_tracks = [GridTrack::Flex(1.0); 8];
            col_tracks[0] = GridTrack::Flex(1.0);
            col_tracks[1] = col1;
            col_tracks[2] = col2;
            let mut row_tracks = [GridTrack::Hug; 8];
            row_tracks[0] = GridTrack::Fixed(130.0);
            row_tracks[1] = GridTrack::Flex(1.0);
            if let Some(fp) = grid_frame.kind.frame_props_mut() {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Grid,
                    grid_columns: 3,
                    grid_rows: 2,
                    item_spacing: gap,
                    counter_spacing: gap,
                    padding: [pad, pad, pad, pad],
                    grid_col_tracks: col_tracks.into(),
                    grid_row_tracks: row_tracks.into(),
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..Default::default()
                });
            }
            let fid = app.doc.insert(page, grid_frame).unwrap();
            let mut child_ids = Vec::new();
            for (name, col, cs, rs, w, h) in specs {
                let cid = app.doc.alloc_id();
                let mut c = Node::rectangle(cid, DRect::new(0.0, 0.0, w, h));
                c.name = name.into();
                c.fills = vec![Paint::solid(col)];
                c.corner_radii = [10.0; 4];
                c.layout_child.col_span = cs;
                c.layout_child.row_span = rs;
                child_ids.push(app.doc.insert(fid, c).unwrap());
            }
            let mut ed = crate::edit::Edit::default();
            crate::layout::relayout_around(&mut app.doc, &mut ed, fid);
            app.selection = match (step, child_ids.first().copied(), child_ids.last().copied()) {
                (5, _, Some(last)) => vec![fid, last],
                (6, _, _) => vec![fid],
                (_, Some(c0), _) => vec![fid, c0],
                _ => vec![fid],
            };
            app.startup_zoom = Some(1.30);
        }
        "micro-smart-selection" => {
            let cards: Vec<(&str, Color, f32, f32, f32)> = match step {
                1 => vec![
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2394.0, 1122.0, 190.0),
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2562.0, 1084.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2786.0, 1112.0, 190.0),
                ],
                2 => vec![
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2400.0, 1100.0, 190.0),
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2572.0, 1100.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2744.0, 1100.0, 190.0),
                ],
                3 => vec![
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2376.0, 1100.0, 190.0),
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2572.0, 1100.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2768.0, 1100.0, 190.0),
                ],
                4 => vec![
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2400.0, 1100.0, 190.0),
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2580.0, 1100.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2760.0, 1100.0, 190.0),
                ],
                5 => vec![
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2400.0, 1100.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2580.0, 1100.0, 190.0),
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2760.0, 1100.0, 190.0),
                ],
                6 => vec![
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2412.0, 1094.0, 204.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2576.0, 1094.0, 204.0),
                    ("Plan · Starter", Color::rgb(0x14, 0xAE, 0x5C), 2740.0, 1094.0, 204.0),
                ],
                _ => vec![
                    ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), 2400.0, 1100.0, 190.0),
                    ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), 2584.0, 1100.0, 190.0),
                    ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), 2768.0, 1100.0, 190.0),
                ],
            };
            let mut ids = Vec::new();
            for (name, col, x, y, h) in cards {
                let cid = app.doc.alloc_id();
                let mut card = Node::rectangle(cid, DRect::new(x, y, 148.0, h));
                card.name = name.into();
                card.fills = vec![Paint::solid(col)];
                card.corner_radii = [14.0; 4];
                ids.push(app.doc.insert(page, card).unwrap());
            }
            app.selection = ids;
            app.startup_zoom = Some(1.30);
        }
        "micro-shape-controls" => {
            let (arc_start, arc_end, arc_ratio, star_cnt, star_ratio, select_star, add_poly) = match step {
                1 => (0.0, std::f32::consts::TAU, 0.0, 5u32, 0.38, false, false),
                2 => (-0.45, 4.15, 0.0, 5, 0.38, false, false),
                3 => (-0.45, 4.35, 0.58, 5, 0.38, false, false),
                4 => (-0.45, 4.35, 0.58, 7, 0.42, true, false),
                5 => (-0.45, 4.35, 0.58, 8, 0.62, true, false),
                6 => (-0.35, 4.65, 0.62, 8, 0.56, false, true),
                _ => (-0.45, 4.15, 0.52, 7, 0.44, false, false),
            };
            let eid = app.doc.alloc_id();
            let mut ell = Node::ellipse(eid, DRect::new(2420.0, 1450.0, 220.0, 220.0));
            if let NodeKind::Ellipse(arc) = &mut ell.kind {
                arc.start = arc_start;
                arc.end = arc_end;
                arc.inner_radius = arc_ratio;
            }
            ell.name = "Donut Arc · Sweep & Ratio".into();
            ell.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
            let eid = app.doc.insert(page, ell).unwrap();

            let sid = app.doc.alloc_id();
            let mut star = Node::new(sid, NodeKind::Star { count: star_cnt, inner_ratio: star_ratio });
            star.name = format!("Star · {star_cnt} pts · {:.0}%", star_ratio * 100.0);
            star.transform = Affine::translate(2680.0, 1450.0);
            star.size = DV::new(220.0, 220.0);
            star.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
            let sid = app.doc.insert(page, star).unwrap();

            if add_poly {
                let pid = app.doc.alloc_id();
                let mut poly = Node::new(pid, NodeKind::Polygon { count: 6 });
                poly.name = "Polygon · 6 Sides".into();
                poly.transform = Affine::translate(2930.0, 1465.0);
                poly.size = DV::new(190.0, 190.0);
                poly.corner_radii = [14.0; 4];
                poly.fills = vec![Paint::solid(Color::rgb(0x14, 0xAE, 0x5C))];
                let pid = app.doc.insert(page, poly).unwrap();
                app.selection = vec![eid, sid, pid];
                app.startup_zoom = Some(1.22);
            } else {
                let active_id = if select_star { sid } else { eid };
                let kind = match step {
                    3 => crate::canvas::handles::ShapeControlKind::EllipseInnerRadius,
                    4 => crate::canvas::handles::ShapeControlKind::StarCount,
                    5 => crate::canvas::handles::ShapeControlKind::StarInnerRatio,
                    _ => crate::canvas::handles::ShapeControlKind::EllipseSweep,
                };
                app.selection = vec![active_id];
                app.canvas.inter = crate::canvas::Interaction::ShapeControl(crate::canvas::ShapeControlDragState {
                    tx: crate::edit::Tx::new(),
                    id: active_id,
                    kind,
                    start_page: DV::new(2530.0, 1560.0),
                    start_count: star_cnt,
                    start_ratio: if select_star { star_ratio } else { arc_ratio },
                    start_end_angle: arc_end,
                });
                app.startup_zoom = Some(1.50);
            }
        }
        "micro-vector-edit" => {
            let vid = app.doc.alloc_id();
            let mut vec_node = Node::new(vid, NodeKind::Polygon { count: 6 });
            vec_node.name = "Vector Network · Bezier & Midpoint Handles".into();
            vec_node.transform = Affine::translate(2420.0, 1820.0);
            vec_node.size = DV::new(340.0, 220.0);
            vec_node.fills = vec![Paint::solid(Color::rgba(13, 153, 255, 68))];
            vec_node.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))], weight: 3.0, ..Default::default() };
            let vid = app.doc.insert(page, vec_node).unwrap();
            app.selection = vec![vid];
            crate::canvas::vector::enter(app, vid);
            match step {
                1 => {
                    if let Some(v) = app.canvas.vedit.as_mut() {
                        v.sel = [0usize, 1usize].into_iter().collect();
                    }
                }
                2 => {
                    crate::canvas::vector::edit(app, |net, sel| {
                        let nv = net.split_segment(0, 0.5);
                        net.vertices[nv].pos.y -= 42.0;
                        *sel = [nv].into_iter().collect();
                        true
                    });
                }
                3 => {
                    crate::canvas::vector::edit(app, |net, sel| {
                        let nv = net.split_segment(0, 0.5);
                        net.vertices[nv].pos.y -= 48.0;
                        net.set_tangent(0, VEnd::End, DV::new(-46.0, -18.0));
                        let last_seg = net.segments.len() - 1;
                        net.set_tangent(last_seg, VEnd::Start, DV::new(46.0, -18.0));
                        net.vertices[nv].mirror = HandleMirror::AngleAndLength;
                        *sel = [nv, 1usize].into_iter().collect();
                        true
                    });
                }
                4 => {
                    crate::canvas::vector::edit(app, |net, sel| {
                        let nv = net.split_segment(0, 0.5);
                        net.vertices[nv].pos.y -= 48.0;
                        net.vertices[nv].mirror = HandleMirror::None;
                        net.set_tangent(0, VEnd::End, DV::new(-64.0, -34.0));
                        let last_seg = net.segments.len() - 1;
                        net.set_tangent(last_seg, VEnd::Start, DV::new(52.0, 26.0));
                        net.set_tangent(1, VEnd::Start, DV::new(36.0, -30.0));
                        *sel = [nv, 1usize, 2usize].into_iter().collect();
                        true
                    });
                }
                5 => {
                    crate::canvas::vector::edit(app, |net, sel| {
                        let nv = net.split_segment(0, 0.5);
                        net.vertices[nv].pos.y -= 44.0;
                        net.set_tangent(0, VEnd::End, DV::new(-48.0, -18.0));
                        if net.vertices.len() > 4 {
                            net.delete_vertex(3);
                        }
                        *sel = [0usize, 1usize, 2usize].into_iter().collect();
                        true
                    });
                }
                6 => {
                    crate::canvas::vector::edit(app, |net, sel| {
                        let nv = net.split_segment(0, 0.5);
                        net.vertices[nv].pos.y -= 44.0;
                        net.set_tangent(0, VEnd::End, DV::new(-48.0, -18.0));
                        *sel = [0usize, 1usize].into_iter().collect();
                        true
                    });
                    crate::canvas::vector::exit(app);
                    if let Some(n) = app.doc.get_mut(vid) {
                        n.fills = vec![Paint::solid(Color::rgba(20, 174, 92, 92))];
                        n.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x14, 0xAE, 0x5C))], weight: 3.5, ..Default::default() };
                    }
                }
                _ => {
                    if let Some(v) = app.canvas.vedit.as_mut() {
                        v.sel = [0usize, 1usize, 2usize].into_iter().collect();
                    }
                }
            }
            app.startup_zoom = Some(1.60);
        }
        "micro-layer-picker" => {
            if let Some(&sel) = app.selection.first() {
                let bb = crate::canvas::fast_world_aabb(&app.doc, None, sel);
                let center_page = DV::new(bb.x + bb.w * 0.5, bb.y + bb.h * 0.45);
                let mut cands = crate::canvas::hit::hit_all_under(&app.doc, app.page, center_page, None);
                if cands.is_empty() {
                    cands.push(sel);
                }
                match step {
                    2 => {
                        let target = cands.get(1).copied().or_else(|| cands.first().copied()).unwrap_or(sel);
                        app.selection = vec![target];
                        app.hover_layer = Some(target);
                        app.startup_zoom = Some(1.18);
                    }
                    3 => {
                        if let Some(n) = app.doc.get_mut(sel) {
                            n.dev.status = Some(DevStatus::ReadyForDev);
                        }
                        app.hover = Some(sel);
                        app.startup_zoom = Some(1.15);
                    }
                    4 => {
                        app.props.eyedropper = Some(crate::panels::properties::PickTarget::Fill(0));
                        app.props.eyedropper_format = crate::panels::properties::EyedropperFormat::Hex;
                        app.hover = cands.first().copied();
                        app.startup_zoom = Some(1.22);
                    }
                    5 => {
                        app.props.eyedropper = Some(crate::panels::properties::PickTarget::Fill(0));
                        app.props.eyedropper_format = crate::panels::properties::EyedropperFormat::Rgb;
                        app.hover = cands.get(1).copied().or_else(|| cands.first().copied());
                        app.startup_zoom = Some(1.22);
                    }
                    6 => {
                        app.props.eyedropper = Some(crate::panels::properties::PickTarget::Fill(0));
                        app.props.eyedropper_format = crate::panels::properties::EyedropperFormat::Hsl;
                        app.hover = cands.first().copied();
                        app.startup_zoom = Some(1.12);
                    }
                    _ => {
                        if let Some(first) = cands.first().copied() {
                            app.hover_layer = Some(first);
                        }
                        app.ui.layer_picker = Some((Pos2::new(680.0, 410.0), cands));
                        app.startup_zoom = Some(1.05);
                    }
                }
            } else {
                app.startup_zoom = Some(1.05);
            }
        }
        "micro-dev-boxmodel" => {
            app.ui.dev_mode = true;
            app.ui.right_tab = Some(crate::app::RightTab::Design);
            app.dev.lang = match step {
                3 => crate::dev::Language::Tailwind,
                4 => crate::dev::Language::Css,
                5 => crate::dev::Language::SwiftUi,
                6 => crate::dev::Language::Compose,
                _ => crate::dev::Language::React,
            };
            crate::dev::preset::apply(app);
            let mut al_frames = Vec::new();
            for (&id, n) in &mut app.doc.nodes {
                if n.parent == Some(page) && n.kind.frame_props().is_some() {
                    n.dev.status = Some(DevStatus::ReadyForDev);
                }
                if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                    if al.padding.iter().all(|&p| p >= 12.0) {
                        al_frames.push(id);
                    }
                }
            }
            if let Some(&chosen) = al_frames.get(step.saturating_sub(1) % al_frames.len().max(1)).or_else(|| al_frames.first()) {
                app.selection = vec![chosen];
                app.hover = Some(chosen);
            }
            app.startup_zoom = match step {
                1 => Some(1.18),
                6 => Some(1.08),
                _ => Some(1.40),
            };
        }
        "micro-scale-tool" => {
            app.tool = crate::canvas::tools::Tool::Scale;
            let mult: f32 = match step {
                1 => 1.00,
                2 => 1.25,
                3 => 1.50,
                4 => 1.75,
                5 => 2.00,
                6 => 1.50,
                _ => 1.25,
            };
            if step >= 3 {
                let ref_id = app.doc.alloc_id();
                let mut ref_card = Node::frame(ref_id, DRect::new(2130.0, 2200.0, 256.0, 160.0));
                ref_card.name = "Baseline · 1.00× (256×160)".into();
                ref_card.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
                ref_card.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x71, 0x71, 0x7A))], weight: 2.0, ..Default::default() };
                ref_card.corner_radii = [14.0; 4];
                let _ = app.doc.insert(page, ref_card);
            }
            let sid = app.doc.alloc_id();
            let w = (256.0 * mult).round();
            let h = (160.0 * mult).round();
            let mut card = Node::frame(sid, DRect::new(2420.0, 2200.0, w, h));
            card.name = format!("Scale Tool (K) · {mult:.2}× Proportional");
            card.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
            let stroke_col = if step == 6 { Color::rgb(0x14, 0xAE, 0x5C) } else { Color::rgb(0x0D, 0x99, 0xFF) };
            card.stroke = DStroke { paints: vec![Paint::solid(stroke_col)], weight: 2.0 * mult, ..Default::default() };
            card.corner_radii = [14.0 * mult; 4];
            let sid = app.doc.insert(page, card).unwrap();

            let badge_id = app.doc.alloc_id();
            let mut badge_node = Node::rectangle(badge_id, DRect::new(24.0 * mult, 24.0 * mult, 112.0 * mult, 44.0 * mult));
            badge_node.name = format!("Badge · {mult:.2}×");
            badge_node.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
            badge_node.corner_radii = [8.0 * mult; 4];
            let _ = app.doc.insert(sid, badge_node);

            let pill_id = app.doc.alloc_id();
            let mut pill_node = Node::rectangle(pill_id, DRect::new(24.0 * mult, 84.0 * mult, 188.0 * mult, 48.0 * mult));
            pill_node.name = format!("Metric · {w:.0}×{h:.0}");
            pill_node.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
            pill_node.corner_radii = [10.0 * mult; 4];
            let _ = app.doc.insert(sid, pill_node);

            app.selection = vec![sid];
            app.canvas.inter = crate::canvas::Interaction::Resize(crate::canvas::ResizeState {
                tx: crate::edit::Tx::new(),
                handle: crate::canvas::handles::Handle::BR,
                kind: crate::canvas::ResizeKind::Single {
                    id: sid,
                    world0: Affine::translate(2420.0, 2200.0),
                    t0: Affine::translate(2420.0, 2200.0),
                    size0: DV::new(256.0, 160.0),
                },
                targets: vec![],
            });
            app.startup_zoom = if step >= 3 { Some(1.18) } else { Some(1.45) };
        }
        "micro-inspector-math" => {
            let (w, h, rad, rot_deg, fill_col, opacity) = match step {
                1 => (280.0, 160.0, 10.0, 0.0f32, Color::rgb(0x0D, 0x99, 0xFF), 1.0),
                2 => (340.0, 184.0, 14.0, 0.0, Color::rgb(0x0D, 0x99, 0xFF), 1.0),
                3 => (404.0, 208.0, 18.0, 6.0, Color::rgb(0x0D, 0x99, 0xFF), 1.0),
                4 => (404.0, 208.0, 28.0, 0.0, Color::rgb(0xE3, 0x63, 0x23), 1.0),
                5 => (404.0, 208.0, 28.0, 0.0, Color::rgb(0xE3, 0x63, 0x23), 0.65),
                _ => (420.0, 216.0, 24.0, 0.0, Color::rgb(0x14, 0xAE, 0x5C), 1.0),
            };
            let cid = app.doc.alloc_id();
            let mut card = Node::frame(cid, DRect::new(2420.0, 2520.0, w, h));
            card.name = format!("Inspector Math · {w:.0}×{h:.0} · R{rad:.0}");
            card.fills = vec![Paint::solid(fill_col)];
            card.opacity = opacity;
            card.corner_radii = [rad; 4];
            if rot_deg.abs() > 0.01 {
                card.transform = Affine::translate(2420.0, 2520.0) * Affine::rotate(rot_deg.to_radians());
            }
            let cid = app.doc.insert(page, card).unwrap();
            app.selection = vec![cid];
            app.startup_zoom = Some(1.38);
        }
        "micro-selection-corner" => {
            let count = match step {
                1 => 2,
                2 | 3 => 4,
                _ => 3,
            };
            let radii = match step {
                4 => [28.0, 28.0, 28.0, 28.0],
                5 => [44.0, 8.0, 44.0, 8.0],
                6 => [36.0, 12.0, 36.0, 12.0],
                _ => [12.0, 12.0, 12.0, 12.0],
            };
            let mut ids = Vec::new();
            for i in 0..count {
                let id = app.doc.alloc_id();
                let mut n = Node::rectangle(id, DRect::new(2400.0 + i as f32 * 168.0, 2840.0, 144.0, 180.0));
                n.name = format!("Card · {:02} (Cmd+D)", i + 1);
                let col = match i % 3 {
                    0 => Color::rgb(0x0D, 0x99, 0xFF),
                    1 => Color::rgb(0xE3, 0x63, 0x23),
                    _ => Color::rgb(0x14, 0xAE, 0x5C),
                };
                n.fills = vec![Paint::solid(col)];
                n.corner_radii = if i == 0 { radii } else { [14.0; 4] };
                ids.push(app.doc.insert(page, n).unwrap());
            }
            app.selection = if step >= 4 { ids.into_iter().take(1).collect() } else { ids };
            app.startup_zoom = Some(1.32);
        }
        "micro-typography-multiedit" => {
            let mut selected = Vec::new();
            for i in 0..3 {
                let fid = app.doc.alloc_id();
                let mut f = Node::frame(fid, DRect::new(2400.0 + i as f32 * 196.0, 3160.0, 176.0, 200.0));
                f.name = format!("Tier Card · {:02}", i + 1);
                f.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
                f.corner_radii = [14.0; 4];
                let fid = app.doc.insert(page, f).unwrap();

                let bid = app.doc.alloc_id();
                let col = if step >= 4 { Color::rgb(0x14, 0xAE, 0x5C) } else { Color::rgb(0x0D, 0x99, 0xFF) };
                let bw = if step >= 3 { 140.0 } else { 112.0 };
                let mut badge = Node::rectangle(bid, DRect::new(18.0, 20.0, bw, 42.0));
                badge.name = "Plan CTA".into();
                badge.fills = vec![Paint::solid(col)];
                badge.corner_radii = [10.0; 4];
                let bid = app.doc.insert(fid, badge).unwrap();
                selected.push(bid);
            }
            app.selection = selected;
            app.startup_zoom = Some(1.25);
        }
        "micro-components-variables" => {
            app.open_variables();
            app.vars.full = false;
            if step >= 5 {
                app.ui.right_tab = Some(crate::app::RightTab::Prototype);
            }
            app.startup_zoom = Some(1.15);
        }
        _ => {}
    }
}

fn smooth01(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_dv(a: designos_doc::Vec2, b: designos_doc::Vec2, t: f32) -> designos_doc::Vec2 {
    designos_doc::Vec2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

fn lerp_color(a: designos_doc::Color, b: designos_doc::Color, t: f32) -> designos_doc::Color {
    designos_doc::Color { r: lerp_f32(a.r, b.r, t), g: lerp_f32(a.g, b.g, t), b: lerp_f32(a.b, b.b, t), a: lerp_f32(a.a, b.a, t) }
}

/// Mutates `app` continuously at normalized time `u = cur / (total - 1)` (`0.0..=1.0`)
/// from a clean `base_doc` snapshot so every frame of `--panel micro-<slug>:seq:<N>`
/// renders real interpolated canvas geometry, handles, and inspector states.
pub fn apply_micro_interaction_continuous_frame(app: &mut App, base_doc: &designos_doc::Document, slug: &str, cur: usize, total: usize) -> MicroSeqHud {
    use designos_doc::vector::End as VEnd;
    use designos_doc::{
        Affine, AutoLayout, Color, DevStatus, GridTrack, HandleMirror, LayoutAlign, LayoutDirection, Node, NodeKind, Paint, Rect as DRect, Sizing,
        Stroke as DStroke, TextStyle, Vec2 as DV,
    };

    app.doc = base_doc.clone();
    app.ui.rulers = true;
    app.ui.dev_mode = false;
    app.ui.right_tab = Some(crate::app::RightTab::Design);
    app.ui.layer_picker = None;
    app.props.eyedropper = None;
    app.canvas.inter = crate::canvas::Interaction::None;
    app.canvas.al_inline = None;
    app.canvas.vedit = None;
    app.vars.open = false;
    app.vars.full = false;
    app.tool = crate::canvas::tools::Tool::Move;

    let page = app.page;
    let u = if total <= 1 { 0.0 } else { (cur as f32) / ((total - 1) as f32) };
    let mut hud = MicroSeqHud {
        focus_page: DV::new(2660.0, 310.0),
        zoom: 1.38,
        cursor_page: DV::new(2660.0, 310.0),
        drag_start_page: None,
        cursor_screen_override: None,
        dragging: false,
        badge_text: String::new(),
        keycap: String::new(),
        caption: String::new(),
        accent_orange: false,
    };

    match slug {
        "micro-autolayout" => {
            // 4 continuous phases across u in 0..1:
            // [0.00..0.28]: Drag Gap handle continuously from 12px -> 38px
            // [0.28..0.55]: Drag Horizontal Padding handle continuously from 16px -> 44px
            // [0.55..0.78]: Switch middle card to Fill & drag Frame width 480px -> 600px
            // [0.78..1.00]: Drag Gap handle into Negative Stack (-20px) with 4 overlapping cards
            let (gap, pad_h, pad_v, primary_sz, width, align_p, is_neg_stack) = if u < 0.28 {
                let t = smooth01(u / 0.28);
                let g = lerp_f32(12.0, 38.0, t);
                (g, 18.0, 18.0, Sizing::Hug, 440.0, LayoutAlign::Start, false)
            } else if u < 0.55 {
                let t = smooth01((u - 0.28) / 0.27);
                let ph = lerp_f32(18.0, 46.0, t);
                let pv = lerp_f32(18.0, 34.0, t);
                (28.0, ph, pv, Sizing::Hug, 480.0, LayoutAlign::Start, false)
            } else if u < 0.78 {
                let t = smooth01((u - 0.55) / 0.23);
                let w = lerp_f32(470.0, 610.0, t);
                (20.0, 28.0, 26.0, Sizing::Fixed, w, LayoutAlign::SpaceBetween, false)
            } else {
                let t = smooth01((u - 0.78) / 0.22);
                let g = lerp_f32(16.0, -22.0, t);
                (g, 28.0, 26.0, Sizing::Hug, 460.0, LayoutAlign::Start, true)
            };

            let fid = app.doc.alloc_id();
            let mut frame = Node::frame(fid, DRect::new(2400.0, 200.0, width, 210.0));
            frame.name = format!("Auto-Layout 5.0 · Gap {gap:.0}px · Pad {pad_h:.0}px");
            frame.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
            frame.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))], weight: 1.5, ..Default::default() };
            frame.corner_radii = [16.0; 4];
            if let Some(fp) = frame.kind.frame_props_mut() {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: gap,
                    padding: [pad_v, pad_h, pad_v, pad_h],
                    primary_sizing: primary_sz,
                    counter_sizing: Sizing::Hug,
                    primary_align: align_p,
                    ..Default::default()
                });
            }
            let fid = app.doc.insert(page, frame).unwrap();

            let cards: Vec<(String, Color, Sizing)> = if is_neg_stack {
                vec![
                    ("Stack · 01".into(), Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                    ("Stack · 02".into(), Color::rgb(0x0D, 0x99, 0xFF), Sizing::Fixed),
                    ("Stack · 03".into(), Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                    ("Stack · 04".into(), Color::rgb(0xFF, 0xFF, 0xFF), Sizing::Fixed),
                ]
            } else {
                let mid_sz = if u >= 0.55 { Sizing::Fill } else { Sizing::Fixed };
                vec![
                    ("Badge · Hug".into(), Color::rgb(0xE3, 0x63, 0x23), Sizing::Fixed),
                    (format!("Metric · {}", if mid_sz == Sizing::Fill { "Fill" } else { "Fixed" }), Color::rgb(0x0D, 0x99, 0xFF), mid_sz),
                    ("Status · Ready".into(), Color::rgb(0x14, 0xAE, 0x5C), Sizing::Fixed),
                ]
            };
            for (label, col, h_sz) in cards {
                let cid = app.doc.alloc_id();
                let mut card = Node::rectangle(cid, DRect::new(0.0, 0.0, 120.0, 126.0));
                card.name = label;
                card.fills = vec![Paint::solid(col)];
                card.stroke = DStroke { paints: vec![Paint::solid(Color::rgba(255, 255, 255, 48))], weight: 1.5, ..Default::default() };
                card.corner_radii = [12.0; 4];
                card.layout_child.horizontal = h_sz;
                let _ = app.doc.insert(fid, card);
            }
            let mut ed = crate::edit::Edit::default();
            crate::layout::relayout_around(&mut app.doc, &mut ed, fid);
            app.selection = vec![fid];

            let f_size = app.doc.get(fid).map(|n| n.size).unwrap_or(DV::new(480.0, 180.0));
            hud.focus_page = DV::new(2650.0, 290.0);
            hud.zoom = 1.36;

            if u < 0.28 {
                let gx = 2400.0 + pad_h + 120.0 + gap * 0.5;
                let gy = 200.0 + f_size.y * 0.5;
                app.canvas.al_inline = Some(crate::canvas::AutoLayoutInlineEdit {
                    id: fid,
                    kind: crate::canvas::handles::AutoLayoutHandleKind::Gap(0),
                    anchor_page: DV::new(gx, gy),
                    text: format!("{gap:.0}"),
                    request_focus: false,
                });
                hud.cursor_page = DV::new(gx, gy);
                hud.drag_start_page = Some(DV::new(2400.0 + 18.0 + 126.0, gy));
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Gap: {gap:.0} px");
                hud.keycap = "⇧A".into();
                hud.caption = "Auto-Layout 5.0 · Dragging on-canvas pink gap handle".into();
            } else if u < 0.55 {
                let px = 2400.0 + pad_h * 0.5;
                let py = 200.0 + f_size.y * 0.5;
                app.canvas.al_inline = Some(crate::canvas::AutoLayoutInlineEdit {
                    id: fid,
                    kind: crate::canvas::handles::AutoLayoutHandleKind::Padding(1),
                    anchor_page: DV::new(px, py),
                    text: format!("{pad_h:.0}"),
                    request_focus: false,
                });
                hud.cursor_page = DV::new(px, py);
                hud.drag_start_page = Some(DV::new(2409.0, py));
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Padding H: {pad_h:.0} px · V: {pad_v:.0} px");
                hud.keycap = "Alt+Drag".into();
                hud.caption = "Mirror-scrubbing horizontal & vertical frame padding".into();
            } else if u < 0.78 {
                let rx = 2400.0 + f_size.x;
                let ry = 200.0 + f_size.y * 0.5;
                hud.cursor_page = DV::new(rx, ry);
                hud.drag_start_page = Some(DV::new(2400.0 + 470.0, ry));
                hud.dragging = true;
                hud.badge_text = format!("Frame W: {:.0} px · Metric = Fill", f_size.x);
                hud.keycap = "Fill".into();
                hud.caption = "Stretching parent frame with Fill Container child sizing".into();
            } else {
                let gx = 2400.0 + pad_h + 120.0 + gap * 0.5;
                let gy = 200.0 + f_size.y * 0.5;
                hud.cursor_page = DV::new(gx, gy);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Negative Gap: {gap:.0} px (Overlap Stack)");
                hud.keycap = "-Gap".into();
                hud.caption = "Negative gap (-22px) creates overlapping avatar/card stacks".into();
            }
        }
        "micro-css-grid" => {
            // [0.00..0.34]: Drag Col 2 Fixed track width from 124px -> 188px
            // [0.34..0.67]: Expand Hero cell span to 2x1 & scrub grid gap 12px -> 22px
            // [0.67..1.00]: Expand Retention cell span to 2x1 & scrub padding 20px -> 32px
            let (col1_w, gap, pad, fw, fh, hero_span, ret_span) = if u < 0.34 {
                let t = smooth01(u / 0.34);
                (lerp_f32(124.0, 188.0, t), 12.0, 20.0, lerp_f32(520.0, 560.0, t), 324.0, 1u16, 1u16)
            } else if u < 0.67 {
                let t = smooth01((u - 0.34) / 0.33);
                (168.0, lerp_f32(12.0, 22.0, t), lerp_f32(20.0, 26.0, t), lerp_f32(560.0, 584.0, t), 344.0, 2u16, 1u16)
            } else {
                let t = smooth01((u - 0.67) / 0.33);
                (172.0, lerp_f32(22.0, 26.0, t), lerp_f32(26.0, 34.0, t), lerp_f32(584.0, 612.0, t), 368.0, 2u16, 2u16)
            };

            let fid = app.doc.alloc_id();
            let mut grid_frame = Node::frame(fid, DRect::new(2400.0, 600.0, fw, fh));
            grid_frame.name = format!("2D CSS Grid · 3×2 · Gap {gap:.0}px");
            grid_frame.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
            grid_frame.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))], weight: 1.5, ..Default::default() };
            grid_frame.corner_radii = [16.0; 4];
            let mut col_tracks = [GridTrack::Flex(1.0); 8];
            col_tracks[0] = GridTrack::Flex(1.0);
            col_tracks[1] = GridTrack::Fixed(col1_w);
            col_tracks[2] = GridTrack::Flex(1.0);
            let mut row_tracks = [GridTrack::Hug; 8];
            row_tracks[0] = GridTrack::Fixed(132.0);
            row_tracks[1] = GridTrack::Flex(1.0);
            if let Some(fp) = grid_frame.kind.frame_props_mut() {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Grid,
                    grid_columns: 3,
                    grid_rows: 2,
                    item_spacing: gap,
                    counter_spacing: gap,
                    padding: [pad, pad, pad, pad],
                    grid_col_tracks: col_tracks.into(),
                    grid_row_tracks: row_tracks.into(),
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..Default::default()
                });
            }
            let fid = app.doc.insert(page, grid_frame).unwrap();

            let mut specs: Vec<(&str, Color, u16, u16)> = vec![
                ("Hero · Span", Color::rgb(0x0D, 0x99, 0xFF), hero_span, 1),
                ("KPI · Fixed", Color::rgb(0xE3, 0x63, 0x23), 1, 1),
                ("Revenue", Color::rgb(0x14, 0xAE, 0x5C), 1, 1),
                ("Retention", Color::rgb(0xFF, 0xFF, 0xFF), ret_span, 1),
            ];
            if hero_span == 1 {
                specs.push(("Active Users", Color::rgb(0x27, 0x27, 0x2A), 1, 1));
                specs.push(("Latency", Color::rgb(0x3F, 0x3F, 0x46), 1, 1));
            } else if ret_span == 1 {
                specs.push(("Latency", Color::rgb(0x3F, 0x3F, 0x46), 1, 1));
            }

            let mut child_ids = Vec::new();
            for (name, col, cs, rs) in specs {
                let cid = app.doc.alloc_id();
                let mut c = Node::rectangle(cid, DRect::new(0.0, 0.0, 150.0, 120.0));
                c.name = name.into();
                c.fills = vec![Paint::solid(col)];
                c.corner_radii = [10.0; 4];
                c.layout_child.col_span = cs;
                c.layout_child.row_span = rs;
                child_ids.push(app.doc.insert(fid, c).unwrap());
            }
            let mut ed = crate::edit::Edit::default();
            crate::layout::relayout_around(&mut app.doc, &mut ed, fid);
            app.selection = vec![fid];

            hud.focus_page = DV::new(2690.0, 775.0);
            hud.zoom = 1.24;
            if u < 0.34 {
                hud.cursor_page = DV::new(2400.0 + pad + 160.0 + col1_w * 0.5, 600.0);
                hud.dragging = true;
                hud.badge_text = format!("Col 2 Track: {col1_w:.0}px Fixed · 1fr Flex");
                hud.keycap = "Grid".into();
                hud.caption = "Scrubbing 2D CSS Grid column track pill (Fixed px ↔ 1fr)".into();
            } else if u < 0.67 {
                if let Some(&first) = child_ids.first() {
                    app.selection = vec![fid, first];
                }
                hud.cursor_page = DV::new(2400.0 + fw * 0.62, 600.0 + pad + 66.0);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Hero Cell: Span 2×1 · Gap {gap:.0}px");
                hud.keycap = "Span 2×1".into();
                hud.caption = "Dragging cell span handle across 2 columns with live reflow".into();
            } else {
                if let Some(&last) = child_ids.last() {
                    app.selection = vec![fid, last];
                }
                hud.cursor_page = DV::new(2400.0 + fw * 0.68, 600.0 + fh - pad - 64.0);
                hud.dragging = true;
                hud.badge_text = format!("Bento 3×2 · Pad {pad:.0}px · Gap {gap:.0}px");
                hud.keycap = "CSS Grid".into();
                hud.caption = "2D Bento Grid reflows both multi-column spans at 60fps".into();
            }
        }
        "micro-smart-selection" => {
            // [0.00..0.28]: Tidy Up smoothly aligns 3 scattered cards into a clean horizontal row
            // [0.28..0.64]: Pink Smart Selection gap handle drags spacing continuously 20px -> 48px -> 26px
            // [0.64..1.00]: Ring handle on middle card ("Plan · Pro") drags left to swap with "Plan · Starter"
            let (x0, y0, x1, y1, x2, y2, h_card) = if u < 0.28 {
                let t = smooth01(u / 0.28);
                (
                    lerp_f32(2388.0, 2400.0, t),
                    lerp_f32(1134.0, 1100.0, t),
                    lerp_f32(2560.0, 2568.0, t),
                    lerp_f32(1072.0, 1100.0, t),
                    lerp_f32(2790.0, 2736.0, t),
                    lerp_f32(1120.0, 1100.0, t),
                    190.0,
                )
            } else if u < 0.64 {
                let t = smooth01((u - 0.28) / 0.36);
                let wave = (t * std::f32::consts::PI).sin();
                let gap = 20.0 + 30.0 * wave;
                let w = 148.0;
                (2400.0, 1100.0, 2400.0 + w + gap, 1100.0, 2400.0 + (w + gap) * 2.0, 1100.0, 190.0)
            } else {
                let t = smooth01((u - 0.64) / 0.36);
                let w = 148.0;
                let gap = 24.0;
                let slot0 = 2400.0;
                let slot1 = 2400.0 + w + gap;
                let slot2 = 2400.0 + (w + gap) * 2.0;
                // Card 0 (Starter) glides from slot0 -> slot1; Card 1 (Pro) glides from slot1 -> slot0 with a slight vertical arc
                let arc = (t * std::f32::consts::PI).sin() * -18.0;
                (lerp_f32(slot0, slot1, t), 1100.0, lerp_f32(slot1, slot0, t), 1100.0 + arc, slot2, 1100.0, lerp_f32(190.0, 202.0, t))
            };

            let specs = [
                ("Plan · Starter", Color::rgb(0x27, 0x27, 0x2A), x0, y0),
                ("Plan · Pro", Color::rgb(0xE3, 0x63, 0x23), x1, y1),
                ("Plan · Enterprise", Color::rgb(0x0D, 0x99, 0xFF), x2, y2),
            ];
            let mut ids = Vec::new();
            for (name, col, x, y) in specs {
                let cid = app.doc.alloc_id();
                let mut card = Node::rectangle(cid, DRect::new(x, y, 148.0, h_card));
                card.name = name.into();
                card.fills = vec![Paint::solid(col)];
                card.stroke = DStroke { paints: vec![Paint::solid(Color::rgba(255, 255, 255, 42))], weight: 1.5, ..Default::default() };
                card.corner_radii = [14.0; 4];
                ids.push(app.doc.insert(page, card).unwrap());
            }
            app.selection = ids;
            hud.focus_page = DV::new(2650.0, 1195.0);
            hud.zoom = 1.28;

            if u < 0.28 {
                hud.cursor_page = DV::new(x1 + 74.0, y1 + 95.0);
                hud.dragging = true;
                hud.badge_text = "Tidy Up · Equalizing Y & Gap = 20px".into();
                hud.keycap = "Tidy Up".into();
                hud.caption = "Smart Selection detects 3 sibling cards and aligns row spacing".into();
            } else if u < 0.64 {
                let gap_val = (x1 - (x0 + 148.0)).max(0.0);
                let gx = x0 + 148.0 + gap_val * 0.5;
                hud.cursor_page = DV::new(gx, 1195.0);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Smart Gap: {gap_val:.0} px");
                hud.keycap = "Drag Gap".into();
                hud.caption = "Dragging pink Smart Selection gap handle adjusts uniform spacing".into();
            } else {
                hud.cursor_page = DV::new(x1 + 74.0, y1 + 95.0);
                hud.drag_start_page = Some(DV::new(2572.0 + 74.0, 1195.0));
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = "Ring Swap: Plan · Pro → Plan · Starter".into();
                hud.keycap = "Swap".into();
                hud.caption = "Dragging center concentric ring swaps sibling card order live".into();
            }
        }
        "micro-shape-controls" => {
            // [0.00..0.35]: Sweep arc angle from 360 deg (TAU) -> 235 deg (4.10 rad)
            // [0.35..0.68]: Pull Donut inner_radius from 0.0 -> 0.62
            // [0.68..1.00]: Morph Star count (5 -> 8) and inner_ratio (0.36 -> 0.64) + Polygon
            let (arc_start, arc_end, arc_ratio, star_cnt, star_ratio, select_star, show_poly) = if u < 0.35 {
                let t = smooth01(u / 0.35);
                (lerp_f32(0.0, -0.45, t), lerp_f32(std::f32::consts::TAU, 4.15, t), 0.0, 5u32, 0.38, false, false)
            } else if u < 0.68 {
                let t = smooth01((u - 0.35) / 0.33);
                (-0.45, 4.25, lerp_f32(0.0, 0.62, t), 5u32, 0.38, false, false)
            } else {
                let t = smooth01((u - 0.68) / 0.32);
                let cnt = (5.0 + (t * 3.0).round()) as u32;
                (-0.40, 4.45, 0.60, cnt.clamp(5, 8), lerp_f32(0.38, 0.64, t), true, t > 0.45)
            };

            let eid = app.doc.alloc_id();
            let mut ell = Node::ellipse(eid, DRect::new(2420.0, 1450.0, 220.0, 220.0));
            if let NodeKind::Ellipse(arc) = &mut ell.kind {
                arc.start = arc_start;
                arc.end = arc_end;
                arc.inner_radius = arc_ratio;
            }
            ell.name = "Donut Arc · Sweep & Ratio".into();
            ell.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
            let eid = app.doc.insert(page, ell).unwrap();

            let sid = app.doc.alloc_id();
            let mut star = Node::new(sid, NodeKind::Star { count: star_cnt, inner_ratio: star_ratio });
            star.name = format!("Star · {star_cnt} pts · {:.0}%", star_ratio * 100.0);
            star.transform = Affine::translate(2680.0, 1450.0);
            star.size = DV::new(220.0, 220.0);
            star.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
            let sid = app.doc.insert(page, star).unwrap();

            if show_poly {
                let pid = app.doc.alloc_id();
                let mut poly = Node::new(pid, NodeKind::Polygon { count: 6 });
                poly.name = "Polygon · 6 Sides".into();
                poly.transform = Affine::translate(2930.0, 1465.0);
                poly.size = DV::new(190.0, 190.0);
                poly.corner_radii = [14.0; 4];
                poly.fills = vec![Paint::solid(Color::rgb(0x14, 0xAE, 0x5C))];
                let pid = app.doc.insert(page, poly).unwrap();
                app.selection = vec![eid, sid, pid];
            } else {
                let active_id = if select_star { sid } else { eid };
                let kind = if u < 0.35 {
                    crate::canvas::handles::ShapeControlKind::EllipseSweep
                } else if u < 0.68 {
                    crate::canvas::handles::ShapeControlKind::EllipseInnerRadius
                } else {
                    crate::canvas::handles::ShapeControlKind::StarInnerRatio
                };
                app.selection = vec![active_id];
                app.canvas.inter = crate::canvas::Interaction::ShapeControl(crate::canvas::ShapeControlDragState {
                    tx: crate::edit::Tx::new(),
                    id: active_id,
                    kind,
                    start_page: DV::new(2530.0, 1560.0),
                    start_count: star_cnt,
                    start_ratio: if select_star { star_ratio } else { arc_ratio },
                    start_end_angle: arc_end,
                });
            }

            hud.focus_page = DV::new(2680.0, 1560.0);
            hud.zoom = 1.32;
            if u < 0.35 {
                let cx = 2530.0 + 95.0 * arc_end.cos();
                let cy = 1560.0 + 95.0 * arc_end.sin();
                hud.cursor_page = DV::new(cx, cy);
                hud.dragging = true;
                hud.badge_text = format!("Arc Sweep: {:.0}°", (arc_end - arc_start).to_degrees());
                hud.keycap = "O · Arc".into();
                hud.caption = "Dragging on-canvas Ellipse Sweep handle carves live sector".into();
            } else if u < 0.68 {
                hud.cursor_page = DV::new(2530.0 + arc_ratio * 65.0, 1560.0);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Donut Inner Ratio: {:.0}%", arc_ratio * 100.0);
                hud.keycap = "Donut".into();
                hud.caption = "Pulling center ratio handle converts pie sector into Donut ring".into();
            } else {
                hud.cursor_page = DV::new(2790.0, 1560.0 - star_ratio * 75.0);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Star: {star_cnt} Points · Inner Ratio {:.0}%", star_ratio * 100.0);
                hud.keycap = "Star".into();
                hud.caption = "Live Star point-count & inner-radius handles + rounded Polygon".into();
            }
        }
        "micro-vector-edit" => {
            // [0.00..0.33]: Split top segment midpoint & pull new vertex upward (dy 0 -> -54px)
            // [0.33..0.72]: Drag symmetric Bezier tangent handles outward (HandleMirror::AngleAndLength)
            // [0.72..1.00]: Break tangent symmetry (Alt) & commit vector path in emerald
            let vid = app.doc.alloc_id();
            let mut vec_node = Node::new(vid, NodeKind::Polygon { count: 6 });
            vec_node.name = "Vector Network · Bezier & Midpoint Handles".into();
            vec_node.transform = Affine::translate(2420.0, 1820.0);
            vec_node.size = DV::new(340.0, 220.0);
            let done = u > 0.88;
            vec_node.fills = vec![Paint::solid(if done { Color::rgba(20, 174, 92, 92) } else { Color::rgba(13, 153, 255, 68) })];
            vec_node.stroke = DStroke {
                paints: vec![Paint::solid(if done { Color::rgb(0x14, 0xAE, 0x5C) } else { Color::rgb(0x0D, 0x99, 0xFF) })],
                weight: 3.2,
                ..Default::default()
            };
            let vid = app.doc.insert(page, vec_node).unwrap();
            app.selection = vec![vid];
            crate::canvas::vector::enter(app, vid);

            let dy = if u < 0.33 { lerp_f32(-6.0, -54.0, smooth01(u / 0.33)) } else { -54.0 };
            let (tx_l, ty_l, tx_r, ty_r, broken) = if u < 0.33 {
                (0.0, 0.0, 0.0, 0.0, false)
            } else if u < 0.72 {
                let t = smooth01((u - 0.33) / 0.39);
                (lerp_f32(-4.0, -58.0, t), lerp_f32(-2.0, -24.0, t), lerp_f32(4.0, 58.0, t), lerp_f32(-2.0, -24.0, t), false)
            } else {
                let t = smooth01((u - 0.72) / 0.28);
                (-58.0, -24.0, lerp_f32(58.0, 52.0, t), lerp_f32(-24.0, 32.0, t), true)
            };

            crate::canvas::vector::edit(app, |net, sel| {
                let nv = net.split_segment(0, 0.5);
                net.vertices[nv].pos.y += dy;
                if tx_l.abs() > 0.5 {
                    net.set_tangent(0, VEnd::End, DV::new(tx_l, ty_l));
                    let last_seg = net.segments.len() - 1;
                    net.set_tangent(last_seg, VEnd::Start, DV::new(tx_r, ty_r));
                    net.vertices[nv].mirror = if broken { HandleMirror::None } else { HandleMirror::AngleAndLength };
                }
                *sel = [nv, 1usize].into_iter().collect();
                true
            });
            if done {
                crate::canvas::vector::exit(app);
            }

            hud.focus_page = DV::new(2590.0, 1910.0);
            hud.zoom = 1.48;
            let apex = DV::new(2590.0, 1820.0 + dy);
            if u < 0.33 {
                hud.cursor_page = apex;
                hud.dragging = true;
                hud.badge_text = format!("Midpoint Split: ΔY {dy:.0} px");
                hud.keycap = "Enter · P".into();
                hud.caption = "Clicking segment midpoint diamond splits edge & drags new vertex".into();
            } else if u < 0.72 {
                hud.cursor_page = DV::new(apex.x + tx_l, apex.y + ty_l);
                hud.drag_start_page = Some(apex);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Mirrored Tangent: ({tx_l:.0}, {ty_l:.0})");
                hud.keycap = "Bezier".into();
                hud.caption = "Dragging cubic Bezier tangent with AngleAndLength mirroring".into();
            } else {
                hud.cursor_page = DV::new(apex.x + tx_r, apex.y + ty_r);
                hud.drag_start_page = Some(apex);
                hud.dragging = !done;
                hud.badge_text = if done { "Vector Network Committed".into() } else { format!("Asymmetric Tangent: ({tx_r:.0}, {ty_r:.0})") };
                hud.keycap = "Alt+Drag".into();
                hud.caption = "Alt-drag breaks tangent handle symmetry for sharp cusp control".into();
            }
        }
        "micro-layer-picker" => {
            // [0.00..0.36]: Cmd+Right-Click Select Layer popup menu cycling overlapping candidates
            // [0.36..0.62]: Ready-for-Dev green status badge on screen frame
            // [0.62..1.00]: Eyedropper (I) gliding across canvas cards cycling HEX -> RGB -> HSL
            let sel = app.selection.first().copied().or_else(|| app.doc.get(page).and_then(|p| p.children.first().copied()));
            if let Some(sel) = sel {
                let bb = crate::canvas::fast_world_aabb(&app.doc, None, sel);
                let center_page = DV::new(bb.x + bb.w * 0.45, bb.y + bb.h * 0.42);
                let mut cands = crate::canvas::hit::hit_all_under(&app.doc, app.page, center_page, None);
                if cands.is_empty() {
                    cands.push(sel);
                }
                hud.focus_page = center_page;
                hud.zoom = 1.15;

                if u < 0.36 {
                    let t = (u / 0.36).clamp(0.0, 0.999);
                    let idx = ((t * cands.len() as f32) as usize).min(cands.len().saturating_sub(1));
                    let chosen = cands[idx];
                    app.hover_layer = Some(chosen);
                    app.selection = vec![chosen];
                    app.ui.layer_picker = Some((Pos2::new(620.0, 360.0), cands));
                    hud.cursor_screen_override = Some(Pos2::new(670.0, 392.0 + idx as f32 * 24.0));
                    hud.dragging = false;
                    hud.badge_text = format!("Select Layer #{}: Overlap Hit-Test", idx + 1);
                    hud.keycap = "⌘+Right-Click".into();
                    hud.caption = "Cycling overlapping layers at cursor via Select Layer menu (#27)".into();
                } else if u < 0.62 {
                    if let Some(n) = app.doc.get_mut(sel) {
                        n.dev.status = Some(DevStatus::ReadyForDev);
                    }
                    app.selection = vec![sel];
                    app.hover = Some(sel);
                    hud.cursor_page = DV::new(bb.x + 80.0, bb.y - 14.0);
                    hud.badge_text = "DevStatus::ReadyForDev Badge".into();
                    hud.keycap = "Ready".into();
                    hud.caption = "Toggling Ready for Dev badge on top-level screen header (#23)".into();
                } else {
                    let t = smooth01((u - 0.62) / 0.38);
                    app.props.eyedropper = Some(crate::panels::properties::PickTarget::Fill(0));
                    app.props.eyedropper_format = if t < 0.33 {
                        crate::panels::properties::EyedropperFormat::Hex
                    } else if t < 0.66 {
                        crate::panels::properties::EyedropperFormat::Rgb
                    } else {
                        crate::panels::properties::EyedropperFormat::Hsl
                    };
                    let fmt_lbl = match app.props.eyedropper_format {
                        crate::panels::properties::EyedropperFormat::Hex => "HEX #0D99FF",
                        crate::panels::properties::EyedropperFormat::Rgb => "RGB(13, 153, 255)",
                        crate::panels::properties::EyedropperFormat::Hsl => "HSL(205°, 100%, 53%)",
                        crate::panels::properties::EyedropperFormat::Hsb => "HSB(205°, 95%, 100%)",
                    };
                    let p0 = DV::new(bb.x + bb.w * 0.28, bb.y + bb.h * 0.32);
                    let p1 = DV::new(bb.x + bb.w * 0.64, bb.y + bb.h * 0.56);
                    hud.cursor_page = lerp_dv(p0, p1, t);
                    hud.dragging = true;
                    hud.accent_orange = true;
                    hud.badge_text = format!("Eyedropper (I) · {fmt_lbl}");
                    hud.keycap = "I · Tab".into();
                    hud.caption = "Sampling live canvas pixels & cycling HEX / RGB / HSL with Tab (#51)".into();
                }
            }
        }
        "micro-dev-boxmodel" => {
            app.ui.dev_mode = true;
            app.ui.right_tab = Some(crate::app::RightTab::Design);
            app.dev.lang = if u < 0.22 {
                crate::dev::Language::React
            } else if u < 0.44 {
                crate::dev::Language::Tailwind
            } else if u < 0.66 {
                crate::dev::Language::Css
            } else if u < 0.84 {
                crate::dev::Language::SwiftUi
            } else {
                crate::dev::Language::Compose
            };
            crate::dev::preset::apply(app);
            let mut al_frames = Vec::new();
            for (&id, n) in &mut app.doc.nodes {
                if n.parent == Some(page) && n.kind.frame_props().is_some() {
                    n.dev.status = Some(DevStatus::ReadyForDev);
                }
                if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
                    if al.padding.iter().all(|&p| p >= 12.0) {
                        al_frames.push(id);
                    }
                }
            }
            al_frames.sort_by_key(|id| id.0);
            if !al_frames.is_empty() {
                let idx = ((u * al_frames.len() as f32) as usize).min(al_frames.len() - 1);
                let chosen = al_frames[idx];
                app.selection = vec![chosen];
                app.hover = Some(chosen);
                let bb = crate::canvas::fast_world_aabb(&app.doc, None, chosen);
                hud.focus_page = DV::new(bb.x + bb.w * 0.5, bb.y + bb.h * 0.5);
                hud.zoom = lerp_f32(1.22, 1.42, (u * std::f32::consts::PI).sin());
                hud.cursor_page = DV::new(bb.x + bb.w * 0.55, bb.y + bb.h * 0.48);
            }
            let lang_name = match app.dev.lang {
                crate::dev::Language::React => "React JSX",
                crate::dev::Language::Tailwind => "Tailwind v4",
                crate::dev::Language::Css => "CSS Flexbox/Grid",
                crate::dev::Language::SwiftUi => "SwiftUI",
                crate::dev::Language::UiKit => "UIKit",
                crate::dev::Language::Compose => "Jetpack Compose",
                crate::dev::Language::AndroidXml => "Android XML",
            };
            hud.badge_text = format!("Dev Mode · Box Model · {lang_name}");
            hud.keycap = "⇧D".into();
            hud.caption = format!("Inspecting live padding/gap box model & {lang_name} codegen (#78..#85)");
        }
        "micro-scale-tool" => {
            app.tool = crate::canvas::tools::Tool::Scale;
            let mult = if u < 0.72 { lerp_f32(1.00, 1.88, smooth01(u / 0.72)) } else { lerp_f32(1.88, 1.50, smooth01((u - 0.72) / 0.28)) };
            let ref_id = app.doc.alloc_id();
            let mut ref_card = Node::frame(ref_id, DRect::new(2130.0, 2200.0, 256.0, 160.0));
            ref_card.name = "Baseline · 1.00× (256×160)".into();
            ref_card.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            ref_card.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x71, 0x71, 0x7A))], weight: 2.0, ..Default::default() };
            ref_card.corner_radii = [14.0; 4];
            let _ = app.doc.insert(page, ref_card);

            let sid = app.doc.alloc_id();
            let w = (256.0 * mult).round();
            let h = (160.0 * mult).round();
            let mut card = Node::frame(sid, DRect::new(2420.0, 2200.0, w, h));
            card.name = format!("Scale Tool (K) · {mult:.2}× Proportional");
            card.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
            let stroke_col = if u > 0.85 { Color::rgb(0x14, 0xAE, 0x5C) } else { Color::rgb(0x0D, 0x99, 0xFF) };
            card.stroke = DStroke { paints: vec![Paint::solid(stroke_col)], weight: 2.0 * mult, ..Default::default() };
            card.corner_radii = [14.0 * mult; 4];
            let sid = app.doc.insert(page, card).unwrap();

            let badge_id = app.doc.alloc_id();
            let mut badge_node = Node::rectangle(badge_id, DRect::new(24.0 * mult, 24.0 * mult, 112.0 * mult, 44.0 * mult));
            badge_node.name = format!("Badge · {mult:.2}×");
            badge_node.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
            badge_node.corner_radii = [8.0 * mult; 4];
            let _ = app.doc.insert(sid, badge_node);

            let pill_id = app.doc.alloc_id();
            let mut pill_node = Node::rectangle(pill_id, DRect::new(24.0 * mult, 84.0 * mult, 188.0 * mult, 48.0 * mult));
            pill_node.name = format!("Metric · {w:.0}×{h:.0}");
            pill_node.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
            pill_node.corner_radii = [10.0 * mult; 4];
            let _ = app.doc.insert(sid, pill_node);

            app.selection = vec![sid];
            app.canvas.inter = crate::canvas::Interaction::Resize(crate::canvas::ResizeState {
                tx: crate::edit::Tx::new(),
                handle: crate::canvas::handles::Handle::BR,
                kind: crate::canvas::ResizeKind::Single {
                    id: sid,
                    world0: Affine::translate(2420.0, 2200.0),
                    t0: Affine::translate(2420.0, 2200.0),
                    size0: DV::new(256.0, 160.0),
                },
                targets: vec![],
            });
            hud.focus_page = DV::new(2540.0, 2330.0);
            hud.zoom = 1.16;
            hud.cursor_page = DV::new(2420.0 + w, 2200.0 + h);
            hud.drag_start_page = Some(DV::new(2420.0 + 256.0, 2200.0 + 160.0));
            hud.dragging = u <= 0.88;
            hud.accent_orange = true;
            hud.badge_text = format!("Scale: {mult:.2}× ({w:.0}×{h:.0}) · Radius {:.1}px", 14.0 * mult);
            hud.keycap = "K · Scale".into();
            hud.caption = "Proportional Scale Tool (K) scales geometry, strokes & radii in lockstep".into();
        }
        "micro-inspector-math" => {
            // M09: Inspector Scrubbing, Math Expressions (`280+140`) & Hex Shorthand (`#09F` -> `#0D99FF`)
            let (w, h, rad, col, op, msg, key) = if u < 0.35 {
                let t = smooth01(u / 0.35);
                let w = lerp_f32(260.0, 420.0, t);
                let h = lerp_f32(156.0, 212.0, t);
                (w, h, 14.0, Color::rgb(0x0D, 0x99, 0xFF), 1.0, format!("Scrub W: {w:.0} px · Math H: 156+56 = {h:.0} px"), "Alt+Scrub")
            } else if u < 0.68 {
                let t = smooth01((u - 0.35) / 0.33);
                let r = lerp_f32(14.0, 38.0, t);
                let c = lerp_color(Color::rgb(0x0D, 0x99, 0xFF), Color::rgb(0xE3, 0x63, 0x23), t);
                (420.0, 212.0, r, c, 1.0, format!("Corner Scrub: {r:.0} px · Hex #E62 → #E36323"), "#E62")
            } else {
                let t = smooth01((u - 0.68) / 0.32);
                let op = lerp_f32(0.50, 1.0, t);
                let c = lerp_color(Color::rgb(0xE3, 0x63, 0x23), Color::rgb(0x14, 0xAE, 0x5C), t);
                (420.0, 212.0, 28.0, c, op, format!("Opacity Shortcut: {:.0}% · Hex #1A5 → #14AE5C", op * 100.0), "0..9 Opacity")
            };
            let cid = app.doc.alloc_id();
            let mut card = Node::frame(cid, DRect::new(2420.0, 2520.0, w, h));
            card.name = format!("Inspector Math · {w:.0}×{h:.0} · R{rad:.0}");
            card.fills = vec![Paint::solid(col)];
            card.opacity = op;
            card.corner_radii = [rad; 4];
            card.stroke = DStroke { paints: vec![Paint::solid(Color::rgba(255, 255, 255, 70))], weight: 2.0, ..Default::default() };
            let cid = app.doc.insert(page, card).unwrap();

            let inner = app.doc.alloc_id();
            let mut pill = Node::rectangle(inner, DRect::new(24.0, 24.0, (w - 48.0).max(80.0), 54.0));
            pill.name = "Math Expression · 260 + 160".into();
            pill.fills = vec![Paint::solid(Color::rgba(17, 18, 22, 190))];
            pill.corner_radii = [(rad * 0.5).min(16.0); 4];
            let _ = app.doc.insert(cid, pill);

            app.selection = vec![cid];
            hud.focus_page = DV::new(2630.0, 2625.0);
            hud.zoom = 1.34;
            hud.cursor_page = DV::new(2420.0 + w, 2520.0 + h * 0.5);
            hud.dragging = true;
            hud.accent_orange = (0.35..0.68).contains(&u);
            hud.badge_text = msg;
            hud.keycap = key.into();
            hud.caption = "Inspector label scrubbing, inline math evaluation & hex shorthand (#1..#9, #50..#54)".into();
        }
        "micro-selection-corner" => {
            // M10: Selection Traversal, Cmd+D Offset Memory & Independent 4-Corner Radius Handles
            // [0.00..0.38]: Alt+Drag & Cmd+D spawns 4 rhythmic cards with offset memory (+164px)
            // [0.38..0.70]: Drag symmetric on-canvas Corner Radius diamond (8px -> 38px)
            // [0.70..1.00]: Alt-drag independent corners into asymmetric leaf card [46, 8, 46, 8]
            let count: usize = if u < 0.12 {
                1
            } else if u < 0.24 {
                2
            } else if u < 0.36 {
                3
            } else {
                4
            };
            let radii = if u < 0.38 {
                [12.0; 4]
            } else if u < 0.70 {
                let r = lerp_f32(12.0, 38.0, smooth01((u - 0.38) / 0.32));
                [r; 4]
            } else {
                let t = smooth01((u - 0.70) / 0.30);
                let r_big = lerp_f32(38.0, 48.0, t);
                let r_sm = lerp_f32(38.0, 8.0, t);
                [r_big, r_sm, r_big, r_sm]
            };

            let mut ids = Vec::new();
            for i in 0..count {
                let id = app.doc.alloc_id();
                let slide = if i == count - 1 && u < 0.38 {
                    let frac = ((u * 8.33) % 1.0).clamp(0.0, 1.0);
                    lerp_f32(-48.0, 0.0, smooth01(frac))
                } else {
                    0.0
                };
                let mut n = Node::rectangle(id, DRect::new(2400.0 + i as f32 * 164.0 + slide, 2840.0, 144.0, 184.0));
                n.name = format!("Card · {:02} (⌘D)", i + 1);
                let col = match i % 3 {
                    0 => Color::rgb(0x0D, 0x99, 0xFF),
                    1 => Color::rgb(0xE3, 0x63, 0x23),
                    _ => Color::rgb(0x14, 0xAE, 0x5C),
                };
                n.fills = vec![Paint::solid(col)];
                n.corner_radii = if i == 0 { radii } else { [14.0; 4] };
                ids.push(app.doc.insert(page, n).unwrap());
            }
            app.selection = if u < 0.38 { ids.clone() } else { ids.into_iter().take(1).collect() };
            hud.focus_page = DV::new(2640.0, 2932.0);
            hud.zoom = 1.28;
            if u < 0.38 {
                let last_x = 2400.0 + (count.saturating_sub(1)) as f32 * 164.0 + 72.0;
                hud.cursor_page = DV::new(last_x, 2932.0);
                hud.dragging = true;
                hud.badge_text = format!("⌘D Offset Memory: {count}× Cards (+164px)");
                hud.keycap = "⌘D · Tab".into();
                hud.caption = "Alt-drag + Cmd+D repeats exact translation offset across siblings (#16, #24)".into();
            } else if u < 0.70 {
                let r = radii[0];
                hud.cursor_page = DV::new(2400.0 + r.max(16.0), 2840.0 + r.max(16.0));
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Uniform Corner Radius: {r:.0} px");
                hud.keycap = "Radius".into();
                hud.caption = "Dragging on-canvas corner diamond rounds all 4 corners live (#35)".into();
            } else {
                hud.cursor_page = DV::new(2400.0 + 144.0 - 14.0, 2840.0 + 14.0);
                hud.dragging = true;
                hud.accent_orange = true;
                hud.badge_text = format!("Asymmetric Radii: [{:.0}, {:.0}, {:.0}, {:.0}] px", radii[0], radii[1], radii[2], radii[3]);
                hud.keycap = "Alt+Corner".into();
                hud.caption = "Holding Alt on a corner handle adjusts individual corner radius (#36)".into();
            }
        }
        "micro-typography-multiedit" => {
            // M11: Multi-Edit (`Q`) across sibling frames + Typography tracking & Auto-Height fit
            let t_all = smooth01(u);
            let badge_w = lerp_f32(104.0, 148.0, t_all);
            let badge_r = lerp_f32(8.0, 20.0, t_all);
            let badge_col = if u < 0.5 {
                lerp_color(Color::rgb(0x0D, 0x99, 0xFF), Color::rgb(0xE3, 0x63, 0x23), smooth01(u * 2.0))
            } else {
                lerp_color(Color::rgb(0xE3, 0x63, 0x23), Color::rgb(0x14, 0xAE, 0x5C), smooth01((u - 0.5) * 2.0))
            };
            let mut selected = Vec::new();
            for i in 0..3 {
                let fid = app.doc.alloc_id();
                let mut f = Node::frame(fid, DRect::new(2400.0 + i as f32 * 196.0, 3160.0, 176.0, 210.0));
                f.name = format!("Multi-Edit Frame · {:02}", i + 1);
                f.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
                f.stroke = DStroke { paints: vec![Paint::solid(Color::rgb(0x3F, 0x3F, 0x46))], weight: 1.5, ..Default::default() };
                f.corner_radii = [14.0; 4];
                let fid = app.doc.insert(page, f).unwrap();

                let bid = app.doc.alloc_id();
                let mut badge = Node::rectangle(bid, DRect::new(16.0, 22.0, badge_w, 46.0));
                badge.name = "Synced CTA · Multi-Edit (Q)".into();
                badge.fills = vec![Paint::solid(badge_col)];
                badge.corner_radii = [badge_r; 4];
                let bid = app.doc.insert(fid, badge).unwrap();
                selected.push(bid);

                let tid = app.doc.alloc_id();
                let style = TextStyle { font_size: lerp_f32(15.0, 18.0, t_all), letter_spacing: lerp_f32(0.0, 1.4, t_all), ..TextStyle::default() };
                let txt = Node::text(tid, DRect::new(16.0, 88.0, 144.0, 84.0), format!("Tracking +{:.1}px\nAuto-Height", style.letter_spacing), style);
                let _ = app.doc.insert(fid, txt);
            }
            app.selection = selected;
            hud.focus_page = DV::new(2684.0, 3265.0);
            hud.zoom = 1.24;
            hud.cursor_page = DV::new(2400.0 + 196.0 + 16.0 + badge_w, 3160.0 + 45.0);
            hud.dragging = true;
            hud.accent_orange = u < 0.55;
            hud.badge_text = format!("Multi-Edit (Q): 3 Layers · W {badge_w:.0}px · R {badge_r:.0}px");
            hud.keycap = "Q · Multi-Edit".into();
            hud.caption = "Pressing Q selects matching layers across sibling frames for lockstep editing (#26, #55..#70)".into();
        }
        "micro-components-variables" => {
            // M12: Components (`Opt+Cmd+K`), Variables (`=`) Live Mode Switch & Prototype Noodles
            let t = smooth01(u);
            let is_dark = (0.35..0.72).contains(&u);
            let is_proto = u >= 0.72;
            if (0.28..0.72).contains(&u) {
                app.open_variables();
                app.vars.full = false;
            }
            if is_proto {
                app.ui.right_tab = Some(crate::app::RightTab::Prototype);
                crate::protovars::open_editor(app);
            }

            let surf = if is_dark { Color::rgb(0x14, 0x14, 0x18) } else { Color::rgb(0xF4, 0xF4, 0xF6) };
            let accent = if is_dark { Color::rgb(0xFF, 0xFF, 0xFF) } else { Color::rgb(0x0D, 0x99, 0xFF) };
            let inst_accent = Color::rgb(0xE3, 0x63, 0x23);
            let comp_w = lerp_f32(180.0, 212.0, (t * std::f32::consts::PI).sin());

            let mut comp_ids = Vec::new();
            for i in 0..3 {
                let cid = app.doc.alloc_id();
                let mut card = Node::frame(cid, DRect::new(2390.0 + i as f32 * 228.0, 3500.0, comp_w, 188.0));
                card.name = if i == 0 {
                    "❖ Master Component · Card/Metric".into()
                } else {
                    format!("◇ Instance {:02} · Mode={}", i, if is_dark { "Dark" } else { "Light" })
                };
                card.fills = vec![Paint::solid(surf)];
                card.stroke = DStroke {
                    paints: vec![Paint::solid(if is_dark { Color::rgb(0xFF, 0xFF, 0xFF) } else { Color::rgb(0x12, 0x12, 0x15) })],
                    weight: if i == 0 { 2.5 } else { 1.5 },
                    ..Default::default()
                };
                card.corner_radii = [16.0; 4];
                let cid = app.doc.insert(page, card).unwrap();

                let pill_id = app.doc.alloc_id();
                let mut pill = Node::rectangle(pill_id, DRect::new(20.0, 24.0, comp_w - 40.0, 48.0));
                pill.name = "Token Bound Header".into();
                pill.fills = vec![Paint::solid(if i == 1 { inst_accent } else { accent })];
                pill.corner_radii = [10.0; 4];
                let _ = app.doc.insert(cid, pill);
                comp_ids.push(cid);
            }
            app.selection = comp_ids;
            hud.focus_page = DV::new(2690.0, 3594.0);
            hud.zoom = 1.18;
            hud.cursor_page = DV::new(2390.0 + comp_w, 3594.0);
            hud.dragging = u < 0.72;
            hud.accent_orange = is_dark;
            hud.badge_text = if u < 0.35 {
                format!("❖ Master → 2 Instances Live Sync (W: {comp_w:.0}px)")
            } else if u < 0.72 {
                "Variables (=) · Subtree Mode Switch: Light → Dark".into()
            } else {
                "Prototype Tab · Interactive Variant & Flow Wiring".into()
            };
            hud.keycap = if u < 0.35 {
                "⌥⌘K".into()
            } else if u < 0.72 {
                "= · Modes".into()
            } else {
                "Prototype".into()
            };
            hud.caption = "Master Component propagation, Variables mode switching & Prototype flows (#71..#100)".into();
        }
        _ => {}
    }

    app.canvas.tiles.invalidate_all();
    app.changed();
    hud
}

/// Paints the native on-canvas pointer, pulsing drag ring, live metric pill, and bottom shortcut HUD
/// directly into the `egui` foreground layer during `--panel micro-<slug>:seq:<N>` recording.
fn paint_micro_seq_hud(ui: &egui::Ui, app: &App, canvas_rect: Rect, _right_rect: Rect, hud: &MicroSeqHud, cur: usize, total: usize) {
    let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("micro-seq-hud")));
    let cursor_pos = hud.cursor_screen_override.unwrap_or_else(|| app.view.to_screen(canvas_rect.min, hud.cursor_page));
    let accent = if hud.accent_orange { egui::Color32::from_rgb(0xE3, 0x63, 0x23) } else { egui::Color32::from_rgb(0x0D, 0x99, 0xFF) };

    if let Some(start_page) = hud.drag_start_page {
        let start_pos = app.view.to_screen(canvas_rect.min, start_page);
        if (cursor_pos - start_pos).length() > 6.0 {
            p.line_segment([start_pos, cursor_pos], egui::Stroke::new(2.0, accent.gamma_multiply(0.75)));
            p.circle_filled(start_pos, 4.5, accent);
        }
    }

    if hud.dragging {
        let pulse = ((cur as f32 * 0.35).sin() * 0.5 + 0.5) * 6.0;
        p.circle_filled(cursor_pos, 14.0 + pulse * 0.4, accent.gamma_multiply(0.22));
        p.circle_stroke(cursor_pos, 16.0 + pulse, egui::Stroke::new(2.0, accent));
    }

    // Crisp vector pointer arrow at `cursor_pos`
    let tip = cursor_pos;
    let arrow = vec![
        tip,
        Pos2::new(tip.x, tip.y + 20.0),
        Pos2::new(tip.x + 5.5, tip.y + 15.0),
        Pos2::new(tip.x + 9.5, tip.y + 23.0),
        Pos2::new(tip.x + 12.5, tip.y + 21.5),
        Pos2::new(tip.x + 8.5, tip.y + 13.5),
        Pos2::new(tip.x + 15.0, tip.y + 13.5),
    ];
    p.add(egui::Shape::convex_polygon(arrow.clone(), egui::Color32::from_rgb(0x11, 0x12, 0x16), egui::Stroke::new(2.0, egui::Color32::WHITE)));

    // Floating live metric pill near cursor
    if !hud.badge_text.is_empty() {
        let font = theme::font_body_strong();
        let tw = widgets::text_width(&p, &hud.badge_text, &font);
        let pill_w = tw + 24.0;
        let pill_h = 28.0;
        let mut px = (tip.x + 22.0).min(canvas_rect.right() - pill_w - 12.0).max(canvas_rect.left() + 12.0);
        let mut py = (tip.y - 34.0).max(canvas_rect.top() + 16.0);
        if py + pill_h > canvas_rect.bottom() - 64.0 {
            py = tip.y - pill_h - 12.0;
        }
        if px < canvas_rect.left() + 12.0 {
            px = canvas_rect.left() + 12.0;
        }
        let pill_rect = Rect::from_min_size(Pos2::new(px, py), Vec2::new(pill_w, pill_h));
        widgets::elevation_shadow(&p, pill_rect, 8);
        p.rect_filled(pill_rect, 8.0, egui::Color32::from_rgba_unmultiplied(17, 18, 22, 242));
        p.rect_stroke(pill_rect, 8.0, egui::Stroke::new(1.5, accent), egui::StrokeKind::Outside);
        p.text(pill_rect.center(), Align2::CENTER_CENTER, &hud.badge_text, font, egui::Color32::WHITE);
    }

    // Top-center HUD caption bar with keycap badge + progress bar
    if !hud.caption.is_empty() {
        let font_cap = theme::font_body_strong();
        let font_key = theme::font_body_strong();
        let kw = widgets::text_width(&p, &hud.keycap, &font_key) + 18.0;
        let cw = widgets::text_width(&p, &hud.caption, &font_cap);
        let bar_w = (kw + cw + 46.0).min(canvas_rect.width() - 24.0);
        let bar_h = 38.0;
        let bar_rect = Rect::from_center_size(Pos2::new(canvas_rect.center().x, canvas_rect.top() + 48.0), Vec2::new(bar_w, bar_h));
        widgets::elevation_shadow(&p, bar_rect, 10);
        p.rect_filled(bar_rect, 10.0, egui::Color32::from_rgba_unmultiplied(17, 18, 22, 238));
        p.rect_stroke(bar_rect, 10.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 36)), egui::StrokeKind::Outside);
        let key_rect = Rect::from_min_size(Pos2::new(bar_rect.left() + 10.0, bar_rect.top() + 7.0), Vec2::new(kw, 24.0));
        p.rect_filled(key_rect, 6.0, accent);
        p.text(key_rect.center(), Align2::CENTER_CENTER, &hud.keycap, font_key, egui::Color32::WHITE);
        p.text(Pos2::new(key_rect.right() + 12.0, bar_rect.center().y), Align2::LEFT_CENTER, &hud.caption, font_cap, egui::Color32::from_rgb(0xF4, 0xF4, 0xF6));
        let prog = if total <= 1 { 1.0 } else { (cur as f32 + 1.0) / (total as f32) };
        let prog_rect = Rect::from_min_size(Pos2::new(bar_rect.left() + 8.0, bar_rect.bottom() - 3.0), Vec2::new((bar_w - 16.0) * prog, 2.0));
        p.rect_filled(prog_rect, 1.0, accent);
    }
}

/// The toolbar is centered on the window (not the canvas).
fn canvas_rect_for_toolbar(full: Rect) -> Rect {
    full
}

#[cfg(test)]
mod tests;
