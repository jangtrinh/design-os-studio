//! Window shell: lays out the panels, routes shortcuts and drives screenshots.

use crate::app::{App, MenuKind};
use crate::canvas::tools::Tool;
use crate::canvas::{self, interact};
use crate::cli::Options;
#[cfg(any(test, feature = "fixtures"))]
use crate::demo;
use crate::icons::Icon;
use crate::panels::menus::{self, Action};
use crate::panels::{self, left, rail, right, toolbar};
use crate::theme;
use crate::widgets;
use egui::{Align2, Context, Event, Key, Pos2, Rect, Vec2};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

mod toast_layout;

/// The toast action that shows an exported file in its folder.
const REVEAL_LABEL: &str = "Show in folder";

/// Set once an `--screenshot` run has written its PNG.
pub static SHOT_OK: AtomicBool = AtomicBool::new(false);
/// The message of a `--select` (or `--edit-text`) that matched nothing in a `--screenshot` run: no PNG is written and
/// the run exits 2. Set by `Shell::new`, the real window's entry, so a test window never touches it.
pub static SELECT_MISS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Set when `--screenshot` fails to open `opts.file` during `Shell::build`.
pub static SHOT_OPEN_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

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
    /// The capture cannot happen: the file did not open, or `--select` matched nothing. The window closes at once
    /// and writes no PNG.
    open_failed: bool,
}

/// Frames a GPU `--screenshot` waits for the view to reach its at-rest frame before it
/// keeps whatever is on screen (the run closes after 240).
const SHOT_GPU_SETTLE_FRAMES: u32 = 180;

/// Height of the custom top window title bar (`🟠 design:os studio`).
pub const TOP_TITLE_BAR_HEIGHT: f32 = 28.0;

#[allow(dead_code)]
pub fn is_typing_in_agent_panel(app: &App, ctx: &Context) -> bool {
    let agent_rail_open = !app.ui.left_collapsed && matches!(app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
    crate::agent_a11y::non_canvas_has_focus(ctx) || app.terminal.is_capturing_keyboard(agent_rail_open)
}

pub struct Shell {
    app: App,
    shot: Option<Shot>,
    title: String,
    paste_chord: crate::clipboard::PasteChord,
    /// Why a `--select` or `--edit-text` named nothing, when it did not.
    select_miss: Option<String>,
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
        let has_file = opts.file.is_some() || opts.import_capture.is_some() || opts.open_id.is_some();
        let (gpu_flag, library) = (opts.gpu, opts.library.clone());
        let mut shell = Self::build(&cc.egui_ctx, opts);
        if shell.app.ui.pages_height.is_none() {
            let settings_root = library.clone().unwrap_or_else(designos_library::Library::default_root);
            shell.app.ui.pages_height = crate::comments::settings::load_pages_height(&settings_root);
        }
        if let Some(miss) = shell.select_miss.as_ref().filter(|_| screenshot) {
            let _ = SELECT_MISS.set(miss.clone());
        }
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
        // GPU canvas on by default, except on a software renderer; small test fixture screenshots stay on the deterministic CPU path unless --gpu
        let saved = library.as_deref().and_then(|l| crate::comments::settings::load_flag(l, "use_gpu"));
        let software = cc.wgpu_render_state.as_ref().is_some_and(canvas::gpu_default::adapter_is_software);
        let start = canvas::gpu_default::decide(saved, gpu_flag, software);
        shell.app.canvas.gpu.pref = if screenshot { gpu_flag || shell.app.doc.nodes.len() > 5_000 } else { start.is_on() };
        shell.app.canvas.gpu.off_for_software = !screenshot && start == canvas::gpu_default::Start::SoftwareOff;
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
            #[cfg(any(test, feature = "fixtures"))]
            Some("demo") => demo::demo_document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("no-selection") => demo::reference_document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("variables") => demo::variables_document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("variables-library" | "variables-library-open") => demo::variables_library_document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some(crate::showcase::FIXTURE) => crate::showcase::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some(crate::protovars::FIXTURE) => crate::protovars::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some(crate::check_designs::FIXTURE) => crate::check_designs::fixture::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("deck") => crate::decks::demo::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("board") => crate::board::demo::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("board-empty") => crate::board::create::new_document("Untitled"),
            #[cfg(any(test, feature = "fixtures"))]
            Some(f) if crate::vecfixture::FIXTURES.contains(&f) => crate::vecfixture::document(),
            #[cfg(any(test, feature = "fixtures"))]
            Some("comments") => demo::demo_document(),
            _ => {
                let mut d = designos_doc::Document::new("Untitled");
                d.add_page("Page 1");
                d
            }
        });
        // A shell started during build (`--panel agents`, `terminal`, `agents-chat`) captures its
        // session directory once; bind the terminal to the id the tab bar keeps before any can start.
        let session_id = app.tabs.active_session_id();
        app.terminal.bind_session(&session_id);
        #[cfg(any(test, feature = "fixtures"))]
        {
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
        }
        if let Some(root) = opts.library.clone() {
            app.attach_library(root);
            if let Some(lib) = &app.lib {
                app.agent_presence.cursor_size = crate::agent_presence::AgentCursorSize::load(lib.root());
            }
        }
        let mut shot_open_failed = false;
        if let Some(f) = &opts.file {
            if let Err(err) = app.try_open_path(f.clone()) {
                if opts.screenshot.is_some() {
                    let reason = err.lines().next().unwrap_or(&err).trim().trim_end_matches('.');
                    let _ = SHOT_OPEN_ERROR.set(format!("{} could not be opened: {reason}. Check the file, then try again.", f.display()));
                    shot_open_failed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if let Some(cap) = &opts.import_capture {
            app.import_web_capture_path(cap.clone());
        }
        if let Some(id) = &opts.open_id {
            app.open_library_file(id);
        } else if opts.home {
            app.go_home();
            #[cfg(any(debug_assertions, test))]
            if let Some(mode) = std::env::var_os("DESIGNOS_FORCE_NO_FILE_DIALOG") {
                if mode == "toast" || mode == "1" {
                    crate::home::import_files_dialog(&mut app, &crate::io::SystemDialogs);
                } else if mode == "dialog" {
                    app.open_import_path_dialog();
                }
            }
        }
        let mut select_miss = None;
        if opts.select.is_some() && !shot_open_failed {
            match crate::select_target::resolve_flag(&mut app.doc, opts.select.as_deref()) {
                Ok(Some(target)) => {
                    if let Some(note) = target.note(opts.select.as_deref().unwrap_or("").trim()) {
                        eprintln!("designos: {note}");
                    }
                    if let Some(id) = target.node {
                        if let Some(pg) = app.doc.page_of(id) {
                            app.doc.ensure_page_hydrated(pg);
                            app.page = pg;
                        }
                        if app.doc.pages.contains(&id) {
                            app.selection.clear();
                        } else {
                            app.selection = vec![id];
                        }
                    }
                }
                Ok(None) => app.selection.clear(),
                Err(err) => {
                    select_miss = Some(err.to_string());
                    // A capture run reports it and exits 2 (no PNG); a person is not thrown out of the app for a typo.
                    if opts.screenshot.is_none() {
                        eprintln!("designos: {err}");
                        app.show_toast(err.toast(), true);
                    }
                }
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
            app.dev.lang = crate::dev::Language::from_id(l).unwrap_or(crate::dev::Language::Css);
            if opts.dev_demo {
                crate::dev::preset::apply(&mut app);
            }
        }
        app.ui.rulers = opts.rulers || !cfg!(test);
        if opts.grid {
            app.ui.pixel_grid = true;
        }
        app.startup_zoom = opts.zoom.map(|z| z / 100.0);
        app.startup_fit_selection = opts.zoom_fit;
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
                if opts.screenshot.is_some() {
                    select_miss.get_or_insert(format!("--edit-text \"{name}\" matched no text layer"));
                }
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
                #[cfg(any(test, feature = "fixtures"))]
                let is_proto_vars = opts.fixture.as_deref() == Some(crate::protovars::FIXTURE);
                #[cfg(not(any(test, feature = "fixtures")))]
                let is_proto_vars = false;
                if is_proto_vars {
                    #[cfg(any(test, feature = "fixtures"))]
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
            Some("agents" | "agents-picker") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.open_for_agent_use(app.file_path.as_deref());
            }
            Some("agents-chat") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.submit_chat_prompt("Create a SaaS analytics card in Claude Code", app.file_path.as_deref());
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
                while std::time::Instant::now() < deadline {
                    let _ = app.terminal.poll_live_agent_bridge();
                    if app.terminal.active_composer().pending_send.is_none() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
            }
            Some("agents-skills") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                // The default Agents panel, so the capture shows the header `Skills` button beside `+`, the composer chip and the picker.
                app.terminal.open_for_agent_use(app.file_path.as_deref());
                app.terminal.active_runtime = crate::terminal::AgentCliPreset::Claude;
                app.terminal.skills.set_selected_skills(vec!["design-system-rhythm".into(), "accessibility-wcag".into()]);
                app.terminal.skills.open_picker();
            }
            Some("agents-start-failure") => {
                // Real failure: the installed `agy` rejects the `--mcp-config` flag it never had (#642).
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.open_failed_start_fixture(app.file_path.as_deref());
            }
            Some("agent-permissions") => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
                app.terminal.ensure_default_tab(app.file_path.as_deref());
                app.ui.permissions_modal_open = true;
            }
            Some(
                p
                @ ("agents-approval-command" | "agents-approval-queued" | "agents-approval-open-tab" | "agents-approval-background-tab" | "agents-scrollback"),
            ) => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                app.terminal.open_approval_fixture(p, app.file_path.as_deref());
            }
            Some(p @ ("agents-turn-card" | "agents-turn-card-collapsed" | "agents-turn-card-small")) => {
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                crate::agent_turn_card::seed_turn_card_fixture(ctx, &mut app, p);
            }
            Some("specs") => {
                app.ui.rail = Some(crate::app::RailItem::Specs);
                app.specs.open = true;
                app.specs.ensure_loaded();
            }
            Some("terminal") => {
                app.show_agents_panel();
                app.terminal.open_for_agent_use(app.file_path.as_deref());
            }
            Some(p @ ("check-designs" | "check-designs-fixed")) => {
                app.open_check_designs();
                if p == "check-designs-fixed" {
                    app.refresh_check_designs(false, ctx);
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
                if let Some(add) = app.doc.nodes.values().find(|n| n.name == "Add").map(|n| n.id) {
                    app.selection = vec![add];
                    app.proto.editing = Some((add, 0));
                }
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
            Some("web-capture-report") => {
                app.web_capture_report_open = true;
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
            open_failed: shot_open_failed || select_miss.is_some(),
        });
        Self { app, shot, title: String::new(), paste_chord: Default::default(), select_miss }
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
        if self.app.plugins.open
            || self.app.dialog.is_some()
            || self.app.history.save_modal.is_some()
            || self.app.dev.compare.is_some()
            || self.app.ui.permissions_modal_open
        {
            return;
        }
        if self.app.canvas.text.is_none() && crate::agent_a11y::try_focus_shortcut(&mut self.app, ctx) {
            return;
        }
        let agent_rail_open = !self.app.ui.left_collapsed && matches!(self.app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
        let capturing_terminal = self.app.terminal.is_capturing_keyboard(agent_rail_open);
        if capturing_terminal {
            self.handle_terminal_undo_keys(ctx);
            return;
        }
        // A click on a text field gives it the focus only while the panel draws, after this runs: the keys
        // that arrive in the same frame are the field's.
        let typing = crate::agent_a11y::non_canvas_has_focus(ctx) || crate::widgets::field_tab::press_lands_on_field(ctx);
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
                self.app.paste_from_system(None);
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

    /// Routes `Cmd+Z` / `Shift+Cmd+Z` / `Cmd+Y` on macOS (or `Ctrl+Z` / `Ctrl+Shift+Z` / `Ctrl+Y`
    /// on Windows and Linux for agent tabs whose CLI does not reserve `Ctrl+Z`) to document
    /// Undo / Redo while the terminal viewport captures the keyboard (`#685`).
    fn handle_terminal_undo_keys(&mut self, ctx: &Context) {
        // An empty composer has no text for `Cmd+Z` to undo, so the chord still reaches the document there.
        let composer_idle_focus = self.app.terminal.side_panel_pty_open
            && self.app.terminal.composer_draft().is_empty()
            && ctx.memory(|m| m.has_focus(crate::agent_a11y::composer_id()));
        let egui_wants_kb = ctx.egui_wants_keyboard_input() && !composer_idle_focus;
        if egui_wants_kb || self.app.vars.open || self.app.ui.menu.is_some() || self.app.ui.ctx_menu.is_some() {
            return;
        }
        let (is_mac, tab_is_agent, cli_uses_ctrl_z) = self.app.terminal.undo_chord_context();
        let mut actions = Vec::new();
        ctx.input_mut(|i| {
            i.events.retain(|ev| match ev {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    match crate::terminal_undo_chord::terminal_undo_chord(*key, *modifiers, is_mac, tab_is_agent, cli_uses_ctrl_z) {
                        Some(act) => {
                            actions.push(act);
                            false
                        }
                        None => true,
                    }
                }
                _ => true,
            });
        });
        if crate::live_patch_apply::user_edit_in_progress(&self.app) {
            return;
        }
        for act in actions {
            menus::run(&mut self.app, ctx, act);
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
                    app.toggle_agents_panel();
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
                    if let Some(id) = app.tabs.entries.get(app.tabs.active_idx).map(|t| t.id) {
                        app.request_leave(Some(ctx), crate::home::Leave::CloseTab(id));
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
            // A board has no `</>` button and no Inspect panel, so it has no Dev Mode to switch.
            Key::D if m.shift && !m.alt && !app.is_board() => app.set_dev_mode(!app.ui.dev_mode),
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
        // The lifetime starts at the first paint. A blocked frame before it (a slow export, a file
        // dialog, a large file opening) must not spend the toast before anyone sees it.
        if let Some(t) = &mut self.app.toast {
            if !t.armed {
                t.armed = true;
                t.until = self.app.now + t.duration;
            }
        }
        // A capture waits out the load and, on the GPU, the idle refine. Hold the latest toast
        // through the paint that asks for the screenshot. `--no-toast` already returned above.
        let hold_for_capture = self.shot.as_ref().is_some_and(|s| !s.requested);
        if !hold_for_capture && self.app.now > self.app.toast.as_ref().map_or(0.0, |t| t.until) {
            self.app.toast = self.app.toast_queue.pop_front().map(|mut next| {
                let duration = if next.reveal.is_some() { theme::TOAST_REVEAL_SECONDS } else { theme::TOAST_SECONDS };
                next.duration = duration;
                next.until = self.app.now + duration;
                next.armed = false;
                next
            });
            if let Some(t) = &mut self.app.toast {
                t.armed = true;
                t.until = self.app.now + t.duration;
            }
        }
        let Some(t) = &self.app.toast else { return };
        let (text, error, reveal, enter_path) = (t.text.clone(), t.error, t.reveal.clone(), t.enter_path);
        let details = self.app.web_capture_report.is_some() && text.ends_with("Details");
        let action_meta = if reveal.is_some() {
            Some(("toast-reveal", REVEAL_LABEL))
        } else if enter_path {
            Some(("toast-enter-path", "Enter a path\u{2026}"))
        } else {
            None
        };
        let lay = toast_layout::layout(ui.painter(), &text, &theme::font_body(), screen, action_meta.map(|(_, l)| l));
        let (r, ctx) = (lay.rect, ui.ctx().clone());
        // Only the "Details" link and the action take clicks, each as its own small Area. The body has no widget at all:
        // any widget in the layer, even a hover-only one, hides the canvas and the cards below it from the pointer.
        let details_clicked = details && toast_click_target(&ctx, "toast-details", r, "Details").clicked();
        let action = lay.action.zip(action_meta).map(|((target, label), (id, text_lbl))| (toast_click_target(&ctx, id, target, text_lbl), target, label));
        // Painted on its own top-most layer: the full-screen Variables view is an Area above the root layer and would
        // hide the toast. No fade-in: the toast has always appeared at full strength.
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("shell-toast")));
        widgets::elevation_shadow(&p, r, theme::RADIUS_FRAME);
        p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        p.circle_filled(lay.dot, 4.0, if error { theme::DANGER } else { theme::BG_BRAND });
        p.galley(lay.text_pos, lay.text, theme::TEXT);
        let mut action_clicked = false;
        if let Some((button, target, label)) = action {
            if button.hovered() {
                p.rect_filled(target, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            p.galley(Pos2::new(target.left() + (target.width() - label.size().x) / 2.0, target.center().y - label.size().y / 2.0), label, theme::TEXT_BRAND);
            action_clicked = button.clicked();
        }
        if details_clicked {
            self.app.web_capture_report_open = !self.app.web_capture_report_open;
        }
        if let (true, Some(file)) = (action_clicked, reveal) {
            if let Err(e) = (self.app.reveal_file)(&file) {
                self.app.toast = None;
                self.app.show_toast(e, true);
            }
        } else if action_clicked && enter_path {
            self.app.toast = None;
            self.app.open_import_path_dialog();
            ctx.request_repaint();
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn show_web_capture_report(&mut self, ctx: &Context) {
        if !self.app.web_capture_report_open {
            return;
        }
        let Some(rep) = self.app.web_capture_report.clone() else {
            self.app.web_capture_report_open = false;
            return;
        };
        // A floating sheet, not a dialog: while the agent's terminal holds the keyboard Esc is the terminal's key, so one
        // press never closes the sheet and interrupts the agent at once (`#923`).
        let agent_rail_open = !self.app.ui.left_collapsed && matches!(self.app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
        if !self.app.terminal.is_capturing_keyboard(agent_rail_open) && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.app.web_capture_report_open = false;
            return;
        }
        let screen = ctx.content_rect();
        let w = 480.0_f32.min((screen.width() - 48.0).max(320.0));
        let row_h = 28.0;
        let header_h = 36.0;
        let summary_rows = 4usize;
        let fb_rows = 1 + rep.absolute_fallbacks.len();
        let font_rows = 1 + rep.font_substitutions.len();
        let style_rows = 1 + rep.unsupported_css.len();
        let img_rows = 1 + rep.images_not_captured.len();
        let note_rows = if rep.notes.is_empty() { 0 } else { 1 + rep.notes.len() };
        let total_rows = summary_rows + fb_rows + font_rows + style_rows + img_rows + note_rows;
        let content_h = total_rows as f32 * row_h;
        let max_body_h = (screen.height() - 140.0).clamp(180.0, 460.0);
        let body_h = content_h.min(max_body_h);
        let h = header_h + 12.0 + body_h;
        let rect = Rect::from_min_size(
            Pos2::new((screen.right() - theme::RIGHT_PANEL_WIDTH - w - 20.0).max(screen.left() + 24.0), screen.top() + 56.0),
            Vec2::new(w, h),
        );
        let mut close_clicked = false;
        egui::Area::new(egui::Id::new("web-capture-report-sheet")).order(egui::Order::Foreground).fixed_pos(rect.min).interactable(true).show(ctx, |ui| {
            let p = ui.painter().clone();
            widgets::elevation_400(&p, rect, theme::RADIUS_FRAME);
            p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
            p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), egui::Stroke::new(1.0, theme::BORDER_MENU), egui::StrokeKind::Inside);

            let hdr = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
            p.text(Pos2::new(hdr.left() + 14.0, hdr.center().y), Align2::LEFT_CENTER, "Web capture import report", theme::font_body_strong(), theme::TEXT);
            let close_r = Rect::from_min_size(Pos2::new(hdr.right() - 30.0, hdr.center().y - 12.0), Vec2::splat(24.0));
            if widgets::icon_button(ui, close_r, "close-web-capture-report", Icon::Close, false, "Close").clicked() {
                close_clicked = true;
            }
            widgets::hline(&p, rect.left(), rect.right(), hdr.bottom(), theme::BORDER_SUBTLE);

            let body_rect = Rect::from_min_size(Pos2::new(rect.left(), hdr.bottom() + 6.0), Vec2::new(rect.width(), body_h));
            ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                egui::ScrollArea::vertical().id_salt("web-capture-report-scroll").max_height(body_h).show(ui, |ui| {
                    // A section heading shows its item count as a pill beside the label, never inside the label text.
                    let draw_row = |ui: &mut egui::Ui, label: &str, count: Option<usize>, value: &str, mono_val: bool, strong_label: bool| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(body_rect.width() - 28.0, row_h), egui::Sense::hover());
                        let row_p = ui.painter();
                        let lf = if strong_label { theme::font_body_strong() } else { theme::font_body() };
                        let lc = if strong_label { theme::TEXT } else { theme::TEXT_SECONDARY };
                        let elided_lbl = widgets::elide(row_p, label, &lf, (r.width() * 0.48).max(100.0));
                        let label_r = row_p.text(Pos2::new(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, elided_lbl, lf, lc);
                        if let Some(n) = count {
                            crate::skills::pills::paint_pill(
                                row_p,
                                label_r.right() + theme::SPACE_INLINE,
                                r.center().y,
                                &n.to_string(),
                                theme::BG_TERTIARY,
                                theme::TEXT_SECONDARY,
                            );
                        }
                        let vf = if mono_val { theme::font_mono() } else { theme::font_body() };
                        let elided_val = widgets::elide(row_p, value, &vf, (r.width() * 0.50).max(110.0));
                        row_p.text(Pos2::new(r.right(), r.center().y), Align2::RIGHT_CENTER, elided_val, vf, theme::TEXT);
                    };
                    let draw_kv = |ui: &mut egui::Ui, label: &str, value: &str, mono_val: bool, strong_label: bool| {
                        draw_row(ui, label, None, value, mono_val, strong_label)
                    };

                    draw_kv(ui, "Imported frame", &rep.root_name, false, false);
                    draw_kv(
                        ui,
                        "Layers created",
                        &format!(
                            "{} {} · {} DOM {}",
                            rep.nodes_created,
                            widgets::plural(rep.nodes_created, "layer", "layers"),
                            rep.elements,
                            widgets::plural(rep.elements, "element", "elements")
                        ),
                        true,
                        false,
                    );
                    draw_kv(
                        ui,
                        "Flattened wrappers",
                        &format!("{} inline {}", rep.flattened_wrappers, widgets::plural(rep.flattened_wrappers, "wrapper", "wrappers")),
                        true,
                        false,
                    );
                    draw_kv(ui, "Capture status", if rep.truncated { "truncated (budget reached)" } else { "complete" }, true, false);

                    draw_row(
                        ui,
                        "Absolute fallbacks",
                        Some(rep.absolute_fallbacks.len()).filter(|n| *n > 0),
                        if rep.absolute_fallbacks.is_empty() { "none" } else { "element · reason" },
                        true,
                        true,
                    );
                    for fb in &rep.absolute_fallbacks {
                        draw_kv(ui, &fb.element_id, &fb.reason, true, false);
                    }

                    draw_row(
                        ui,
                        "Font substitutions",
                        Some(rep.font_substitutions.len()).filter(|n| *n > 0),
                        if rep.font_substitutions.is_empty() { "all matched" } else { "requested → substituted" },
                        true,
                        true,
                    );
                    for sf in &rep.font_substitutions {
                        draw_kv(ui, &sf.requested, &format!("→ {}", sf.substituted), true, false);
                    }

                    draw_row(
                        ui,
                        "Unsupported CSS",
                        Some(rep.unsupported_css.len()).filter(|n| *n > 0),
                        if rep.unsupported_css.is_empty() { "0 declarations" } else { "property (element) · value" },
                        true,
                        true,
                    );
                    for us in &rep.unsupported_css {
                        draw_kv(ui, &format!("{} ({})", us.property, us.element_id), &us.value, true, false);
                    }

                    draw_row(
                        ui,
                        "Uncaptured images",
                        Some(rep.images_not_captured.len()).filter(|n| *n > 0),
                        if rep.images_not_captured.is_empty() { "0 placeholders" } else { "element (reason) · src" },
                        true,
                        true,
                    );
                    for bi in &rep.images_not_captured {
                        draw_kv(ui, &format!("{} ({})", bi.element_id, bi.reason), &bi.src, true, false);
                    }

                    if !rep.notes.is_empty() {
                        draw_row(ui, "Notes", Some(rep.notes.len()), "details", true, true);
                        for note in &rep.notes {
                            draw_kv(ui, "Note", note, true, false);
                        }
                    }
                });
            });
        });
        if close_clicked {
            self.app.web_capture_report_open = false;
        }
    }
}

impl eframe::App for Shell {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.17, 0.17, 0.17, 1.0]
    }

    /// Tab belongs to the text session (it nests list items) or to sibling layer traversal on the
    /// canvas: take it out of the input before egui uses it to move keyboard focus between widgets.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // macOS: a Quit chosen while the window is hidden waits a few seconds for it. The wait starts at the first frame
        // after the request, and eframe runs this hook for hidden frames too; `ctx.input` still shows the previous one.
        #[cfg(target_os = "macos")]
        crate::macos_menu::stamp_pending_quit(ctx, raw.time);
        if self.app.plugins.open {
            return;
        }
        // A layer being renamed in the Layers panel moves its rename to the next row on Tab, so Tab is left to it.
        if self.app.layers.renaming.is_none() {
            crate::widgets::field_tab::take_tab_press(ctx, raw);
        }
        if crate::agent_a11y::non_canvas_has_focus(ctx) {
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
        if self.app.text_refit_pending() {
            // The next slice of the re-fit after a face arrived. A gesture holds it: look again a few times
            // a second, so the pass goes on once the gesture ends. Nothing is asked for once it is done.
            if self.app.text_refit_held() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
}

impl Shell {
    /// Files dropped on the window: `.svg`, `.png`, `.jpg` and `.webp` files are placed at the drop point,
    /// `.webcapture` / `.capture.json` files are imported at the drop point,
    /// anything else (the first file) is opened as a document.
    fn handle_drops(&mut self, ctx: &Context) {
        // A question is modal: a file dropped while one is open is ignored, as on Home. It must not replace the
        // action that is pending (a quit, a close) or put content on the canvas behind the question.
        if self.app.dialog.is_some() || self.app.leave.is_asking() {
            return;
        }
        let (files, pos) = ctx.input(|i| (i.raw.dropped_files.clone(), i.pointer.hover_pos().or(i.pointer.latest_pos())));
        let extension_of = |f: &egui::DroppedFile| {
            let name = f.path.as_ref().and_then(|p| p.to_str()).unwrap_or(&f.name);
            name.rsplit('.').next().map(str::to_ascii_lowercase).unwrap_or_default()
        };
        let is_svg = |f: &egui::DroppedFile| extension_of(f) == "svg";
        let is_placed = |f: &egui::DroppedFile| crate::canvas::interact::IMAGE_EXTENSIONS.contains(&extension_of(f).as_str());
        if let Some(path) = files.iter().filter(|f| !is_placed(f)).find_map(|f| f.path.clone()) {
            if crate::io::is_web_capture_path(&path) {
                let page_pos = pos.map(|s| self.app.view.to_page(self.app.canvas_rect.min, s));
                self.app.import_web_capture_path_at(path, page_pos);
            } else {
                self.app.open_path(path);
            }
            return;
        }
        // Every file of the drop goes through one transaction: one undo step, and all the new layers are selected.
        let mut tx = crate::edit::Tx::new();
        let (mut placed, mut any_image) = (Vec::new(), false);
        for (nth, f) in files.iter().filter(|f| is_placed(f)).enumerate() {
            let stem = f
                .path
                .as_deref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .or_else(|| f.name.rsplit_once('.').map(|(stem, _)| stem))
                .filter(|s| !s.is_empty())
                .unwrap_or(if is_svg(f) { "svg" } else { "Image" })
                .to_string();
            if is_svg(f) {
                let text = match (&f.path, &f.bytes) {
                    (Some(p), _) => std::fs::read_to_string(p).map_err(|e| format!("Cannot read {}: {e}", p.display())),
                    (None, Some(b)) => String::from_utf8(b.to_vec()).map_err(|_| "The SVG is not valid UTF-8".to_string()),
                    (None, None) => continue,
                };
                match text {
                    Ok(t) => placed.extend(crate::clipboard::drop_svg(&mut self.app, &mut tx, &t, &stem, pos, nth)),
                    Err(e) => self.app.show_toast(e, true),
                }
            } else {
                let bytes = match (&f.path, &f.bytes) {
                    (Some(p), _) => std::fs::read(p).map_err(|e| format!("Cannot read {}: {e}", p.display())),
                    (None, Some(b)) => Ok(b.to_vec()),
                    (None, None) => continue,
                };
                match bytes {
                    Ok(b) => {
                        let id = crate::clipboard::drop_image(&mut self.app, &mut tx, b, &stem, pos, nth);
                        any_image |= id.is_some();
                        placed.extend(id);
                    }
                    Err(e) => self.app.show_toast(e, true),
                }
            }
        }
        crate::clipboard::commit_dropped(&mut self.app, tx, placed, any_image);
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
        if self.app.ui.permissions_modal_open {
            let mut mode = self.app.terminal.permission_mode;
            let mut open = self.app.ui.permissions_modal_open;
            let picked = crate::agent_approval_card::paint_permissions_modal(ctx, &mut open, &mut mode, None);
            self.app.ui.permissions_modal_open = open;
            if let Some(new_mode) = picked {
                self.app.set_agent_permission_mode(new_mode);
            }
        }
        let full = ui.max_rect();
        self.toast(ui, full);
        self.sync_title(ctx);
    }

    /// One full UI pass over the window, then the screenshot request it may owe.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        if self.shot.as_ref().is_some_and(|s| s.open_failed) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.draw_window(ui);
        let ctx = ui.ctx().clone();
        // A paste that started during this frame (the key or the menu ran after the poll) waits for its clipboard
        // worker; an idle window draws nothing more, so ask for the frame that finishes it.
        if !self.app.paste_jobs.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.app.repaint_if_dot_moved(&ctx);
        self.request_screenshot(&ctx);
    }

    fn draw_window(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        crate::agent_a11y::sync_frame(&ctx);
        crate::widgets::field_tab::start_frame(&ctx);
        self.app.now = ctx.input(|i| i.time);
        self.app.poll_plugins();
        self.app.poll_image_jobs();
        self.app.poll_paste_jobs();
        if self.app.image_jobs.has_active_jobs() || self.app.tabs.entries.iter().any(|e| e.image_jobs.has_active_jobs()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.drive_screenshot(&ctx);
        // macOS: a Quit chosen while the window was hidden brought it back and waits here for a frame that can draw the
        // question; the close it asks for comes round to the branch below on the next frame.
        #[cfg(target_os = "macos")]
        crate::macos_menu::send_pending_quit(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.shot.is_some() {
                self.app.terminal.shutdown();
                for tab in &mut self.app.tabs.entries {
                    tab.terminal.shutdown();
                }
            } else if !self.app.may_quit(&ctx) {
                // Edits no save has written are being asked about: the window stays open, and the close comes round again.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else {
                crate::decks::ui::flush_notes(&mut self.app);
                self.app.save_close_version();
                self.app.flush_autosave();
                self.app.terminal.shutdown();
                for tab in &mut self.app.tabs.entries {
                    tab.terminal.shutdown();
                }
            }
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
            crate::agent_a11y::poll_and_step(&mut self.app, &ctx);
            return;
        }
        self.app.autosave_tick(&ctx);
        self.app.history_tick();
        self.app.dev_tick(&ctx);
        if self.app.proto.present.is_some() {
            crate::present::show(&mut self.app, ui);
            crate::agent_a11y::poll_and_step(&mut self.app, &ctx);
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
        crate::live_patch_apply::publish_live_context(&mut self.app);
        if let Some(ai_copy) = self.app.specs.last_ai_copy.take() {
            self.app.terminal.send_copy_for_ai(&ai_copy);
        }
        let win_focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        self.app.terminal.set_window_focused(win_focused);
        self.app.terminal.surface_visible = !self.app.ui.left_collapsed && is_ai_panel;
        // An agent that writes while no frame runs (Agents panel closed, no input) still reaches the canvas.
        self.app.terminal.live_patch.wake_on_writes(&ctx, &self.app.terminal.live_spec_path);
        self.app.terminal.overlay_owns_keyboard = crate::terminal::overlay_owns_keyboard(&self.app, &ctx);
        let bridge_actions = self.app.terminal.poll_live_agent_bridge();
        if !bridge_actions.is_empty() {
            execute_terminal_actions_with_ctx(&mut self.app, bridge_actions, Some(&ctx));
        }
        // Agent edits that arrived during a drag or text edit land once it has ended.
        crate::live_patch_apply::apply_pending_live_patches(&mut self.app);
        if !self.app.doc.prune_in_progress().is_empty() {
            self.app.in_progress_changed();
            ctx.request_repaint();
        }
        if full_vars {
            // The local-variables full view replaces the left panel, canvas and right panel.
            rail::show(&mut self.app, ui, rail_rect);
            let content = Rect::from_min_max(Pos2::new(rail_rect.right(), full.top()), full.max);
            crate::variables::view::show(&mut self.app, ui, content);
        } else {
            let specs_reader_open = false;
            if specs_reader_open {
                self.app.canvas_rect = canvas_rect;
                crate::specs::show_reader(&mut self.app, ui, canvas_rect);
            } else {
                if let Some(seq) = self.shot.as_ref().and_then(|s| s.seq.as_ref()) {
                    if seq.hud.zoom > 0.0 {
                        self.app.first_fit_done = true;
                        let aspect_scale = (canvas_rect.width() / 1080.0).clamp(0.70, 1.05);
                        let z = (seq.hud.zoom * aspect_scale).max(0.25);
                        self.app.view.zoom = z;
                        self.app.view.offset = designos_doc::Vec2::new(
                            seq.hud.focus_page.x - (canvas_rect.width() * 0.5) / z,
                            seq.hud.focus_page.y - (canvas_rect.height() * 0.5) / z,
                        );
                    }
                }
                canvas::show(&mut self.app, ui, canvas_rect);
                if deck {
                    crate::decks::ui::notes(&mut self.app, ui, notes_rect);
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
                if !specs_reader_open {
                    crate::board::toolbar::show(&mut self.app, ui, canvas_rect_for_toolbar(full));
                    crate::board::bar::show(&mut self.app, ui, canvas_rect);
                    crate::board::canvas::zoom_pill(&mut self.app, ui, canvas_rect);
                }
            } else {
                if specs_reader_open {
                    crate::specs::show_right(&mut self.app, ui, right_rect);
                } else {
                    right::show(&mut self.app, ui, right_rect);
                }
                if !specs_reader_open {
                    let tb_rect = if deck { canvas_rect } else { canvas_rect_for_toolbar(full) };
                    toolbar::show(&mut self.app, ui, tb_rect);
                }
            }
            if !specs_reader_open {
                crate::history::banner::show(&mut self.app, ui, canvas_rect);
                self.context_menu(ui, canvas_rect);
            }
            menus::show(&mut self.app, &ctx);
            panels::context::show(&mut self.app, &ctx);
            crate::image_jobs::show_popovers(&mut self.app, &ctx);
            panels::check_designs::show(&mut self.app, &ctx);
            // the floating panel (or only the inspector's binding picker when closed)
            crate::variables::view::show(&mut self.app, ui, full);
            if let Some(seq) = self.shot.as_ref().and_then(|s| s.seq.as_ref()) {
                paint_micro_seq_hud(ui, &self.app, canvas_rect, right_rect, &seq.hud, seq.cur, seq.total);
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
        self.show_web_capture_report(&ctx);
        if self.app.ui.permissions_modal_open {
            let mut mode = self.app.terminal.permission_mode;
            let mut open = self.app.ui.permissions_modal_open;
            let picked = crate::agent_approval_card::paint_permissions_modal(&ctx, &mut open, &mut mode, None);
            self.app.ui.permissions_modal_open = open;
            if let Some(new_mode) = picked {
                self.app.set_agent_permission_mode(new_mode);
            }
        }
        self.toast(ui, full);
        crate::agent_a11y::poll_and_step(&mut self.app, &ctx);

        self.sync_title(&ctx);
        if self.app.ui.menu == Some(MenuKind::Main) && self.app.ui.menu_anchor.is_none() {
            self.app.ui.menu = None;
        }
    }
}

#[cfg(test)]
pub fn execute_terminal_actions(app: &mut App, actions: Vec<crate::terminal::TerminalAction>) {
    execute_terminal_actions_with_ctx(app, actions, None);
}

pub fn execute_terminal_actions_with_ctx(app: &mut App, actions: Vec<crate::terminal::TerminalAction>, ctx: Option<&egui::Context>) {
    for action in actions {
        match action {
            crate::terminal::TerminalAction::ApplySpecJson(json) => match serde_json::from_str::<designos_doc::NodeSpec>(&json) {
                Ok(spec) => {
                    let before = app.hist.undo_depth();
                    app.apply_node_spec(&spec, "Agent Terminal");
                    if app.hist.undo_depth() > before {
                        app.hist.mark_last_undo_as_agent();
                        app.terminal.current_undo_depth = app.hist.undo_depth();
                    }
                }
                Err(e) => {
                    app.show_toast(format!("Spec parse error: {}", designos_doc::format_spec_serde_error(&e)), true);
                }
            },
            // A spec that cannot be read (a link, a FIFO, a file that is gone) is no spec: nothing is applied and nothing is said.
            crate::terminal::TerminalAction::ApplySpecFile(path) => {
                if let Some(raw) = crate::agent_session_files::read_session_text(&path) {
                    match serde_json::from_str::<designos_doc::NodeSpec>(&raw) {
                        Ok(spec) => {
                            let label = path.file_name().and_then(|s| s.to_str()).unwrap_or("Spec");
                            let before = app.hist.undo_depth();
                            app.apply_live_spec(&path, &raw, &spec, label);
                            if app.hist.undo_depth() > before {
                                app.hist.mark_last_undo_as_agent();
                                app.terminal.current_undo_depth = app.hist.undo_depth();
                            }
                        }
                        Err(e) => app.show_toast(format!("Invalid spec JSON: {}", designos_doc::format_spec_serde_error(&e)), true),
                    }
                }
            }
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
            crate::terminal::TerminalAction::ExportSelectionToTerminal => {
                match crate::terminal::export_selection_and_comments_to_pty(&mut app.terminal, &app.doc, app.page, &app.selection, app.file_path.as_deref()) {
                    Ok(p) => app.show_toast(format!("Sent selection & comments -> {}", p.display()), false),
                    Err(e) => app.show_toast(e, true),
                }
            }
            crate::terminal::TerminalAction::SendSelectionToAgent => {
                if let Err(e) = crate::terminal::send_selection_to_agent_pty(&mut app.terminal, &app.doc, app.page, &app.selection) {
                    app.show_toast(e, true);
                }
            }
            crate::terminal::TerminalAction::OpenTurnReceipt(tab_id) => {
                if let Some(tab) = app.terminal.tabs.iter().find(|t| t.id == tab_id) {
                    let existing: Vec<designos_doc::NodeId> =
                        tab.turn_tracker.touched_layers.iter().copied().map(designos_doc::NodeId).filter(|id| app.doc.get(*id).is_some()).collect();
                    if !existing.is_empty() {
                        app.selection = existing;
                    }
                    if let Some(receipt) = tab.last_turn_receipt() {
                        app.show_toast(receipt.format_line(), false);
                    }
                }
            }
            crate::terminal::TerminalAction::FocusAgentTab(tab_id) => {
                if let Some(ctx) = ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                app.ui.rail = Some(crate::app::RailItem::Agents);
                app.ui.left_collapsed = false;
                let _ = app.terminal.focus_tab_by_id(tab_id);
            }
            crate::terminal::TerminalAction::CloseStoppedTurn { tab_id, turn_start_depth, run_id } => {
                crate::agent_stop::close_stopped_turn_on_app(app, tab_id, turn_start_depth, run_id.as_deref());
            }
            crate::terminal::TerminalAction::ClearInProgress { run_id, .. } => {
                if !app.doc.clear_in_progress_for_run_or_agent(run_id.as_deref(), None).is_empty() {
                    app.in_progress_changed();
                }
                if let Some(rid) = run_id.as_deref() {
                    app.agent_busy_frames.clear_run(rid);
                }
                let next_turn = app.agent_busy_frames.current_turn.saturating_add(1);
                app.agent_busy_frames.start_turn(next_turn);
                app.sync_busy_frame_banner();
            }
        }
    }
}

pub fn execute_cmdk_action(app: &mut App, action: crate::cmdk::CmdKAction) {
    match action {
        crate::cmdk::CmdKAction::ApplySpec(spec) => {
            let fp = app.file_path.clone();
            app.show_agents_panel();
            app.terminal.dispatch_figma_ai_prompt("Make Design", &spec.name, fp.as_deref());
            app.apply_node_spec(&spec, "Cmd+K Synthesizer");
        }
        crate::cmdk::CmdKAction::OpenSpecReader => {
            app.ui.rail = Some(crate::app::RailItem::Agents);
            app.ui.left_collapsed = false;
            app.specs.open = false;
            app.terminal.open_for_agent_use(app.file_path.as_deref());
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::OpenTerminal(preset) => {
            let file_path = app.file_path.clone();
            let _ = app.terminal.spawn_tab(preset, file_path.as_deref());
            app.show_agents_panel();
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::SendSelectionToTerminal => {
            app.show_agents_panel();
            match crate::terminal::export_selection_and_comments_to_pty(&mut app.terminal, &app.doc, app.page, &app.selection, app.file_path.as_deref()) {
                Ok(p) => app.show_toast(format!("Sent selection & comments -> {}", p.display()), false),
                Err(e) => app.show_toast(e, true),
            }
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::ExportFig => {
            app.export_fig_copy();
            app.cmdk.open = false;
        }
        crate::cmdk::CmdKAction::SelectNode(id) => {
            app.select_only(id);
            app.fit_selection();
        }
        crate::cmdk::CmdKAction::OpenCheckDesigns => app.open_check_designs(),
        crate::cmdk::CmdKAction::AutoFixDesignGate => {
            let (op, fixed_count) = match crate::cmdk::run_auto_fix_design_gate(app) {
                crate::cmdk::AutoFixOutcome::Committed { op, count } => (op, count),
                // A refused edit (old version, failed commit) was explained by `run_tx`; only a clean no-op needs words.
                crate::cmdk::AutoFixOutcome::Refused => return,
                crate::cmdk::AutoFixOutcome::NoChange => {
                    app.show_toast("Nothing changed", false);
                    return;
                }
            };
            app.cmdk.last_audit = Some(crate::cmdk::run_design_gate_audit(&app.doc, app.page, &app.selection));
            // The palette's search field keeps the keyboard while it is open, so Cmd+Z would not reach the canvas.
            app.cmdk.open = false;
            let fp = app.file_path.clone();
            app.show_agents_panel();
            app.terminal.dispatch_figma_ai_prompt("Figma AI Action", &format!("Applied {fixed_count} Canvas changes (Undo with Cmd+Z)"), fp.as_deref());
            app.show_toast(crate::cmdk::done_toast(op), false);
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
                crate::dev::Language::Html => "HTML5 + CSS3",
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
                if let Some(add) = app.doc.nodes.values().find(|n| n.name == "Add").map(|n| n.id) {
                    app.selection = vec![add];
                    app.proto.editing = Some((add, 0));
                }
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
        let mut px = (tip.x + 22.0).min(canvas_rect.right() - pill_w - theme::SPACE_GROUP).max(canvas_rect.left() + theme::SPACE_GROUP);
        let mut py = (tip.y - 34.0).max(canvas_rect.top() + theme::SPACE_BLOCK);
        if py + pill_h > canvas_rect.bottom() - (theme::SPACE_REGION + theme::SPACE_BLOCK) {
            py = tip.y - pill_h - theme::SPACE_GROUP;
        }
        if px < canvas_rect.left() + theme::SPACE_GROUP {
            px = canvas_rect.left() + theme::SPACE_GROUP;
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
        let bar_rect = Rect::from_center_size(Pos2::new(canvas_rect.center().x, canvas_rect.top() + theme::SPACE_REGION), Vec2::new(bar_w, bar_h));
        widgets::elevation_shadow(&p, bar_rect, 10);
        p.rect_filled(bar_rect, 10.0, egui::Color32::from_rgba_unmultiplied(17, 18, 22, 238));
        p.rect_stroke(bar_rect, 10.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 36)), egui::StrokeKind::Outside);
        let key_rect = Rect::from_min_size(
            Pos2::new(bar_rect.left() + theme::INSET_CONTROL + theme::INSET_SEGMENT, bar_rect.top() + theme::SPACE_INLINE - theme::BORDER_WIDTH),
            Vec2::new(kw, 24.0),
        );
        p.rect_filled(key_rect, 6.0, accent);
        p.text(key_rect.center(), Align2::CENTER_CENTER, &hud.keycap, font_key, egui::Color32::WHITE);
        p.text(
            Pos2::new(key_rect.right() + theme::SPACE_GROUP, bar_rect.center().y),
            Align2::LEFT_CENTER,
            &hud.caption,
            font_cap,
            egui::Color32::from_rgb(0xF4, 0xF4, 0xF6),
        );
        let prog = if total <= 1 { 1.0 } else { (cur as f32 + 1.0) / (total as f32) };
        let prog_rect = Rect::from_min_size(
            Pos2::new(bar_rect.left() + theme::SPACE_INLINE, bar_rect.bottom() - (theme::SPACE_ROW - theme::BORDER_WIDTH)),
            Vec2::new((bar_w - 16.0) * prog, 2.0),
        );
        p.rect_filled(prog_rect, 1.0, accent);
    }
}

/// An invisible click target over `rect` on the top-most layer, named `name` for screen readers. It is its own small
/// Area so the toast around it, which registers no widget, lets pointer input through.
fn toast_click_target(ctx: &Context, id: &str, rect: Rect, name: &str) -> egui::Response {
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .constrain(false)
        .fade_in(false)
        .interactable(true)
        .show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(rect.size(), egui::Sense::click());
            widgets::record(ui, id, rect);
            widgets::labelled(ui, response.id, egui::accesskit::Role::Button, name);
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        })
        .inner
}

/// The toolbar is centered on the window (not the canvas).
fn canvas_rect_for_toolbar(full: Rect) -> Rect {
    full
}

#[cfg(test)]
mod tests;
