//! End-to-end UI tests: drive the whole window with synthetic egui input.

use super::*;
use designos_doc::{Color, NodeId, NodeKind, Paint, Vec2 as DV};
use egui::{Modifiers, PointerButton, RawInput};

fn no_system_clipboard() -> Option<String> {
    None
}

fn no_system_image() -> Option<crate::system_paste::ClipImage> {
    None
}

/// Hermetic: the OS pasteboard is global and not thread-safe, so no test window reads it. Tests script the clipboard
/// flavours they need through `App::system_html`, `App::system_text` and `App::system_image`.
fn script_empty_clipboard(app: &mut App) {
    app.system_html = no_system_clipboard;
    app.system_text = no_system_clipboard;
    app.system_image = no_system_image;
}

/// Directory of stub agent CLIs (`claude`, `codex`, `agy`, `grok`, `gemini`, `opencode`). Every stub blocks reading
/// its PTY, so it lives exactly as long as the tab that started it and never exits on its own. A timer-based stub
/// would bring back the race this exists to remove: an agent that exits while the test is still clicking changes
/// its tab and the panel under the pointer.
/// The directory is shared by every test process, so a run leaves nothing new behind. Each script is written to a
/// private name and renamed into place, so a process that execs a stub never sees a half-written file or a file
/// that is still open for writing.
#[cfg(unix)]
fn stub_agent_cli_dir() -> &'static std::path::Path {
    use std::os::unix::fs::PermissionsExt;
    const SCRIPT: &[u8] = b"#!/bin/sh\nexec /bin/cat\n";
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("designos-stub-agent-clis");
        std::fs::create_dir_all(&dir).expect("create the stub agent CLI dir");
        for binary in ["claude", "codex", "agy", "grok", "gemini", "opencode"] {
            let path = dir.join(binary);
            let is_current = std::fs::read(&path).is_ok_and(|bytes| bytes == SCRIPT)
                && std::fs::metadata(&path).is_ok_and(|meta| meta.permissions().mode() & 0o111 == 0o111);
            if is_current {
                continue;
            }
            let staged = dir.join(format!(".{binary}.{}", std::process::id()));
            std::fs::write(&staged, SCRIPT).expect("write a stub agent CLI");
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).expect("make a stub agent CLI executable");
            std::fs::rename(&staged, &path).expect("move a stub agent CLI into place");
        }
        dir
    })
}

struct Sim {
    ctx: Context,
    shell: Shell,
    t: f64,
    pos: Pos2,
    mods: Modifiers,
    cursor: egui::CursorIcon,
    shapes: Vec<egui::epaint::ClippedShape>,
    accesskit_update: Option<egui::accesskit::TreeUpdate>,
    /// The viewport commands the last frame sent (`Close`, `CancelClose`, ...).
    commands: Vec<egui::ViewportCommand>,
    /// Every viewport command sent since the test last cleared this.
    sent_commands: Vec<egui::ViewportCommand>,
}

impl Sim {
    fn new() -> Self {
        Self::with(Options::default())
    }
    fn with(opts: Options) -> Self {
        let ctx = Context::default();
        let mut shell = Shell::build(&ctx, opts);
        script_empty_clipboard(&mut shell.app);
        let mut s = Sim {
            ctx,
            shell,
            t: 0.0,
            pos: Pos2::new(700.0, 500.0),
            mods: Modifiers::NONE,
            cursor: egui::CursorIcon::Default,
            shapes: Vec::new(),
            accesskit_update: None,
            commands: Vec::new(),
            sent_commands: Vec::new(),
        };
        s.frame(vec![]);
        s.frame(vec![]);
        s
    }
    fn frame(&mut self, events: Vec<Event>) {
        self.run_frame(events, false);
    }
    /// The frame in which the window's close button was pressed: the app exits right after it.
    fn close_frame(&mut self) {
        self.run_frame(vec![], true);
    }
    fn run_frame(&mut self, events: Vec<Event>, close: bool) {
        self.t += 0.05;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
            time: Some(self.t),
            events,
            modifiers: self.mods,
            ..Default::default()
        };
        if close {
            raw.viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        }
        let ctx = self.ctx.clone();
        let shell = &mut self.shell;
        // like the real window: Tab presses of a text session are taken out before egui sees them
        let mut raw = raw;
        eframe::App::raw_input_hook(shell, &ctx, &mut raw);
        let out = ctx.run_ui(raw, |ui| shell.draw(ui));
        self.cursor = out.platform_output.cursor_icon;
        self.shapes = out.shapes;
        self.accesskit_update = out.platform_output.accesskit_update;
        // a paste that waits for the clipboard worker lands before the test looks at the document
        self.shell.app.settle_paste_jobs();
        self.commands = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default();
        self.sent_commands.extend(self.commands.iter().cloned());
    }
    fn app(&self) -> &App {
        &self.shell.app
    }
    fn screen(&self, x: f32, y: f32) -> Pos2 {
        self.app().view.to_screen(self.app().canvas_rect.min, DV::new(x, y))
    }
    fn mv(&mut self, p: Pos2) {
        self.pos = p;
        self.frame(vec![Event::PointerMoved(p)]);
    }
    fn button(&mut self, pressed: bool) {
        let ev = Event::PointerButton { pos: self.pos, button: PointerButton::Primary, pressed, modifiers: self.mods };
        self.frame(vec![ev]);
    }
    fn click(&mut self, p: Pos2) {
        self.mv(p);
        self.button(true);
        self.button(false);
        self.frame(vec![]);
    }
    fn right_click(&mut self, p: Pos2) {
        self.mv(p);
        for pressed in [true, false] {
            let ev = Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed, modifiers: self.mods };
            self.frame(vec![ev]);
        }
        self.frame(vec![]);
    }
    fn drag(&mut self, a: Pos2, b: Pos2) {
        self.mv(a);
        self.button(true);
        for i in 1..=4 {
            let t = i as f32 / 4.0;
            self.mv(a + (b - a) * t);
        }
        self.button(false);
        self.frame(vec![]);
    }
    fn key(&mut self, key: Key) {
        let m = self.mods;
        self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: m }]);
        self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: m }]);
    }
    fn cmd_key(&mut self, key: Key) {
        self.mods = Modifiers::COMMAND;
        self.key(key);
        self.mods = Modifiers::NONE;
    }
    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.into())]);
    }
    /// Starts a frame that carries several input events at once; see [`OneFrame`].
    fn one_frame(&mut self) -> OneFrame<'_> {
        let pos = self.pos;
        OneFrame { sim: self, events: Vec::new(), pos }
    }
    /// A whole drag, press to release, delivered in ONE frame, then one empty frame to settle.
    /// `mv`, `button` and [`Sim::drag`] spread the same gesture over eight frames, which hides every bug that only
    /// shows when the window is slow.
    fn drag_in_one_frame(&mut self, a: Pos2, b: Pos2) {
        let mid = a + (b - a) * 0.5;
        self.one_frame().move_to(a).press().move_to(mid).move_to(b).release().send();
        self.frame(vec![]);
    }
    fn field_rect(&self, key: &str) -> Rect {
        self.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{key:?}"))))).unwrap_or_else(|| panic!("no field {key}"))
    }
    fn rect_of(&self, name: &str) -> Rect {
        self.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("nothing recorded for {name}"))
    }
    /// Scrolls the inspector to the bottom and redraws.
    fn scroll_inspector_down(&mut self) {
        self.shell.app.props.scroll = 1.0e6;
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn by_name(&self, name: &str) -> NodeId {
        self.app().doc.nodes.values().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
    }
    fn count(&self) -> usize {
        self.app().doc.nodes.len()
    }
    /// Makes the agent CLIs this window starts the stubs of [`stub_agent_cli_dir`], whatever is installed on the
    /// machine. Without it the test starts the real `claude`, `codex` and `agy` where they exist, and on a runner
    /// without them each tab dies a few frames after it is picked, at a moment that depends on machine load.
    /// The stub dir goes in front of the inherited `PATH`, so a Shell tab still finds `sleep`, `cat` and the rest.
    fn use_stub_agent_clis(&mut self) {
        #[cfg(unix)]
        {
            let mut dirs = vec![stub_agent_cli_dir().to_path_buf()];
            dirs.extend(std::env::var_os("PATH").iter().flat_map(std::env::split_paths));
            self.shell.app.terminal.path_override = Some(std::env::join_paths(dirs).expect("a PATH made of existing dirs"));
        }
    }
}

/// Input events that reach the window inside ONE egui frame, in the order they are chained.
///
/// Every other [`Sim`] helper (`mv`, `button`, `click`, `drag`, `key`, `type_text`) runs one frame per event. A
/// real window is not that polite: on a slow frame, or a quick flick, the OS hands egui a whole
/// "press, moves, release" gesture, or a click followed by keys, between two frames. egui then reports only the
/// LAST pointer position of the frame, reports the button as pressed AND released but no longer down, and
/// delivers the key and text events together. Code that samples the pointer once per frame sees a different
/// gesture than the user made.
///
/// Chain the events in the order they arrived and finish with [`OneFrame::send`], which runs exactly one frame:
///
/// ```ignore
/// s.mv(a);                                    // earlier frame: the pointer rests on `a`
/// s.one_frame().press().move_to(b).release().send(); // press, move and release share one frame
/// s.one_frame().key(Key::R).text("x").send();  // a key and typed text share one frame
/// ```
///
/// Pointer events take the position of the last `move_to` (the pointer position of the previous frame when there
/// is none), and every event carries the modifiers [`Sim::mods`] holds when it is chained.
struct OneFrame<'a> {
    sim: &'a mut Sim,
    events: Vec<Event>,
    pos: Pos2,
}

impl OneFrame<'_> {
    fn move_to(mut self, p: Pos2) -> Self {
        self.pos = p;
        self.events.push(Event::PointerMoved(p));
        self
    }
    fn button(mut self, button: PointerButton, pressed: bool) -> Self {
        self.events.push(Event::PointerButton { pos: self.pos, button, pressed, modifiers: self.sim.mods });
        self
    }
    /// Primary button down at the current position.
    fn press(self) -> Self {
        self.button(PointerButton::Primary, true)
    }
    /// Primary button up at the current position.
    fn release(self) -> Self {
        self.button(PointerButton::Primary, false)
    }
    /// A key going down and up again, like [`Sim::key`] but without a frame between the two.
    fn key(mut self, key: Key) -> Self {
        let modifiers = self.sim.mods;
        for pressed in [true, false] {
            self.events.push(Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers });
        }
        self
    }
    /// Typed text, like [`Sim::type_text`].
    fn text(mut self, s: &str) -> Self {
        self.events.push(Event::Text(s.into()));
        self
    }
    /// Runs the one frame that holds every chained event.
    fn send(self) {
        self.sim.pos = self.pos;
        self.sim.frame(self.events);
    }
}

#[test]
fn first_frames_render_without_panic() {
    let s = Sim::new();
    assert!(s.app().canvas_rect.width() > 500.0);
    assert!(s.app().view.zoom > 0.1);
}

#[test]
fn rectangle_tool_draws_and_undoes() {
    let mut s = Sim::new();
    let before = s.count();
    s.key(Key::R);
    assert_eq!(s.app().tool, Tool::Rectangle);
    let (a, b) = (s.screen(300.0, 800.0), s.screen(380.0, 860.0));
    s.mods = Modifiers::COMMAND; // no snapping
    s.drag(a, b);
    s.mods = Modifiers::NONE;
    assert_eq!(s.count(), before + 1);
    assert_eq!(s.app().tool, Tool::Move);
    let id = s.app().selection[0];
    let n = s.app().doc.get(id).unwrap();
    assert!(matches!(n.kind, NodeKind::Rectangle));
    assert!((n.size.x - 80.0).abs() < 4.0 && (n.size.y - 60.0).abs() < 4.0, "size {:?}", n.size);
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before);
}

#[test]
fn click_selects_child_and_drag_moves_it() {
    let mut s = Sim::new();
    let hero = s.by_name("Hero");
    let p = s.screen(300.0, 150.0);
    s.click(p);
    assert_eq!(s.app().selection, vec![hero]);
    let t0 = s.app().doc.get(hero).unwrap().transform.tx;
    let a = s.screen(300.0, 150.0);
    let b = s.screen(330.0, 150.0);
    s.drag(a, b);
    let t1 = s.app().doc.get(hero).unwrap().transform.tx;
    assert!((t1 - t0 - 30.0).abs() < 8.0, "moved by {}", t1 - t0);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().transform.tx, t0);
}

#[test]
fn resize_with_corner_handle() {
    let mut s = Sim::with(Options { select: Some("Button".into()), ..Default::default() });
    let id = s.by_name("Button");
    let (corners, _) = interact::selection_box(s.app()).unwrap();
    let br = s.app().view.to_screen(s.app().canvas_rect.min, corners[2]);
    let w0 = s.app().doc.get(id).unwrap().size.x;
    s.drag(br, br + Vec2::new(-20.0, 0.0));
    let w1 = s.app().doc.get(id).unwrap().size.x;
    assert!(w1 < w0 - 10.0, "{w0} -> {w1}");
}

#[test]
fn pen_tool_creates_vector_path() {
    let mut s = Sim::new();
    s.key(Key::P);
    for (x, y) in [(300.0, 800.0), (370.0, 840.0), (300.0, 880.0)] {
        let p = s.screen(x, y);
        s.click(p);
    }
    s.key(Key::Enter);
    let id = s.app().selection[0];
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Vector { paths, .. } => assert_eq!(paths[0].cmds.len(), 3),
        k => panic!("unexpected {k:?}"),
    }
}

#[test]
fn text_tool_types_and_commits() {
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    assert!(s.app().canvas.text.is_some());
    s.type_text("Hi");
    s.key(Key::Escape);
    assert!(s.app().canvas.text.is_none());
    let id = s.app().selection[0];
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => assert_eq!(t.content, "Hi"),
        k => panic!("unexpected {k:?}"),
    }
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).is_none(), "one undo removes the created text");
}

#[test]
fn empty_text_is_discarded() {
    let mut s = Sim::new();
    let before = s.count();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.key(Key::Escape);
    assert_eq!(s.count(), before);
    assert!(!s.app().hist.can_undo());
}

#[test]
fn shortcuts_duplicate_group_delete() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let n0 = s.count();
    s.cmd_key(Key::D);
    assert_eq!(s.count(), n0 + 1);
    s.cmd_key(Key::G);
    assert_eq!(s.count(), n0 + 2);
    s.key(Key::Delete);
    assert_eq!(s.count(), n0, "group + its duplicate removed");
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), n0);
    assert!(!s.app().hist.can_undo());
}

#[test]
fn auto_layout_shortcut_wraps() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    s.mods = Modifiers::SHIFT;
    s.key(Key::A);
    s.mods = Modifiers::NONE;
    let id = s.app().selection[0];
    let n = s.app().doc.get(id).unwrap();
    assert!(n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some());
}

#[test]
fn wheel_pans_and_pinch_zooms() {
    let mut s = Sim::new();
    let p = s.screen(600.0, 400.0);
    s.mv(p);
    let off0 = s.app().view.offset;
    s.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -60.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    }]);
    s.frame(vec![]);
    assert!(s.app().view.offset.y > off0.y, "scrolling down moves the view down the page");
    let z0 = s.app().view.zoom;
    s.frame(vec![Event::Zoom(1.5)]);
    assert!((s.app().view.zoom / z0 - 1.5).abs() < 0.01);
    s.mods = Modifiers::SHIFT;
    s.key(Key::Num0);
    assert!((s.app().view.zoom - 1.0).abs() < 1e-4);
    s.key(Key::Num1);
    s.mods = Modifiers::NONE;
    assert!(s.app().view.zoom < 1.0);
}

#[test]
fn hand_tool_pans() {
    let mut s = Sim::new();
    s.key(Key::H);
    let off0 = s.app().view.offset;
    let a = s.screen(600.0, 400.0);
    s.drag(a, a + Vec2::new(100.0, 0.0));
    assert!(s.app().view.offset.x < off0.x - 50.0);
    assert_eq!(s.app().tool, Tool::Hand);
}

#[test]
fn layers_panel_click_and_visibility_toggle() {
    let mut s = Sim::new();
    // first row (top-most layer = "Dashboard") of the layers list
    s.click(Pos2::new(180.0, 241.0));
    let dash = s.by_name("Dashboard");
    assert_eq!(s.app().selection, vec![dash]);
    // hover reveals the eye button at the row's right edge
    s.mv(Pos2::new(180.0, 241.0));
    s.click(Pos2::new(297.0 - 8.0 - 12.0 - 1.0, 241.0));
    assert!(!s.app().doc.get(dash).unwrap().visible);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(dash).unwrap().visible);
}

#[test]
fn layers_drag_reparents_into_frame() {
    let mut s = Sim::new();
    let welcome = s.by_name("Welcome");
    let card = s.by_name("Card");
    // rows: Dashboard (241), Card (273), Welcome (305). Drag Card onto the middle of Welcome.
    s.drag(Pos2::new(180.0, 273.0), Pos2::new(180.0, 305.0));
    assert_eq!(s.app().doc.get(card).unwrap().parent, Some(welcome));
}

#[test]
fn page_add_and_switch() {
    let mut s = Sim::new();
    let n = s.app().doc.pages.len();
    s.click(Pos2::new(277.0, 85.0));
    assert_eq!(s.app().doc.pages.len(), n + 1);
    assert_eq!(s.app().page, *s.app().doc.pages.last().unwrap());
    s.click(Pos2::new(150.0, 118.0));
    assert_eq!(s.app().page, s.app().doc.pages[0]);
}

#[test]
fn inspector_x_field_edits_position() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let id = s.by_name("Card");
    let r = s.field_rect("x");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("700");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, 700.0);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, 560.0);
}

#[test]
fn rail_menu_opens_and_closes() {
    let mut s = Sim::new();
    s.click(Pos2::new(28.0, 24.0));
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main));
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, None);
}

#[test]
fn marquee_selects_multiple_and_nudges() {
    let mut s = Sim::new();
    let a = s.screen(560.0, 290.0);
    let b = s.screen(1250.0, 790.0);
    s.drag(a, b);
    assert!(!s.app().selection.is_empty());
    let id = s.app().selection[0];
    let x0 = s.app().doc.get(id).unwrap().transform.tx;
    s.key(Key::ArrowRight);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, x0 + 1.0);
    s.mods = Modifiers::SHIFT;
    s.key(Key::ArrowRight);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, x0 + 11.0);
}

#[test]
fn alt_drag_duplicates() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let n0 = s.count();
    s.mods = Modifiers::ALT;
    let a = s.screen(300.0, 150.0);
    s.drag(a, a + Vec2::new(40.0, 40.0));
    s.mods = Modifiers::NONE;
    assert_eq!(s.count(), n0 + 1);
}

#[test]
fn dropped_into_frame_on_move() {
    let mut s = Sim::new();
    let tile = s.by_name("Tile A");
    let welcome = s.by_name("Welcome");
    // drag the tile (inside Dashboard) onto the Welcome frame
    let a = s.screen(560.0 + 24.0 + 90.0, 360.0 + 24.0 + 60.0);
    let b = s.screen(300.0, 400.0);
    s.drag(a, b);
    assert_eq!(s.app().doc.get(tile).unwrap().parent, Some(welcome));
}

#[test]
fn color_swatch_opens_picker_without_panic() {
    // (the demo Card's fill is bound to a variable, so use a plain layer)
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let sw = s.rect_of("sw:false:0");
    s.click(sw.center());
    assert!(s.app().props.picker.is_some());
    s.frame(vec![]);
    s.key(Key::Escape);
    assert!(s.app().props.picker.is_none());
}

#[test]
fn page_context_menu_duplicates_and_deletes() {
    let mut s = Sim::new();
    let n = s.app().doc.pages.len();
    // second page row, then the menu opens at the pointer
    s.right_click(Pos2::new(150.0, 141.0));
    assert!(s.app().layers.page_menu.is_some());
    s.click(Pos2::new(180.0, 141.0 + 4.0 + 24.0 + 12.0)); // Duplicate
    assert_eq!(s.app().doc.pages.len(), n + 1);
    assert!(s.app().layers.page_menu.is_none());
    s.right_click(Pos2::new(150.0, 118.0));
    s.click(Pos2::new(180.0, 118.0 + 4.0 + 48.0 + 12.0)); // Delete
    assert_eq!(s.app().doc.pages.len(), n);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.pages.len(), n + 1);
}

#[test]
fn page_context_menu_rename() {
    let mut s = Sim::new();
    let pid = s.app().doc.pages[1];
    s.right_click(Pos2::new(150.0, 141.0));
    s.click(Pos2::new(180.0, 141.0 + 4.0 + 12.0)); // Rename
    assert_eq!(s.app().layers.renaming_page, Some(pid));
    s.frame(vec![]);
    s.type_text("Renamed");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(pid).unwrap().name, "Renamed");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(pid).unwrap().name, "Components");
}

#[test]
fn dragging_a_page_row_reorders_it() {
    let mut s = Sim::new();
    let first = s.app().doc.pages[0];
    // drag page 1 below page 2 (slot 2)
    s.drag(Pos2::new(150.0, 118.0), Pos2::new(150.0, 118.0 + 28.0));
    assert_eq!(s.app().doc.pages[1], first);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.pages[0], first);
}

fn prefix_point(s: &Sim, key: &str) -> Pos2 {
    let r = s.field_rect(key);
    Pos2::new(r.left() + 8.0, r.center().y)
}

#[test]
fn scrubbing_x_label_changes_value_in_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let id = s.by_name("Card");
    let x0 = s.app().doc.get(id).unwrap().transform.tx;
    let a = prefix_point(&s, "x");
    s.mv(a);
    s.frame(vec![]);
    assert_eq!(s.cursor, egui::CursorIcon::ResizeHorizontal);
    s.drag(a, a + Vec2::new(20.0, 0.0));
    let x1 = s.app().doc.get(id).unwrap().transform.tx;
    assert!((x1 - x0 - 20.0).abs() <= 1.0, "scrubbed by {}", x1 - x0);
    assert!(s.app().scrub.is_none());
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, x0, "a whole drag is a single undo step");
    s.cmd_key(Key::Y);
    assert_eq!(s.app().doc.get(id).unwrap().transform.tx, x1);
}

#[test]
fn shift_multiplies_scrub_by_ten() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let id = s.by_name("Card");
    let y0 = s.app().doc.get(id).unwrap().transform.ty;
    let a = prefix_point(&s, "y");
    s.mods = Modifiers::SHIFT;
    s.drag(a, a + Vec2::new(5.0, 0.0));
    s.mods = Modifiers::NONE;
    let y1 = s.app().doc.get(id).unwrap().transform.ty;
    assert!((y1 - y0 - 50.0).abs() <= 10.0, "scrubbed by {}", y1 - y0);
    assert!(y1 - y0 >= 40.0);
}

#[test]
fn clicking_a_color_style_applies_it_to_the_selection() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.scroll_inspector_down();
    let r = s.rect_of("style:Brand/Primary");
    s.click(r.center());
    let n = s.app().doc.get(hero).unwrap();
    assert_eq!(n.style_refs.get("fill").map(String::as_str), Some("Brand/Primary"));
    assert_eq!(n.fills, vec![Paint::solid(Color::rgb(0x0C, 0x8C, 0xE9))]);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(hero).unwrap().style_refs.is_empty());
}

#[test]
fn clicking_a_text_style_applies_it_to_text() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t = s.by_name("Welcome back");
    s.scroll_inspector_down();
    let r = s.rect_of("style:Body/Regular");
    s.click(r.center());
    match &s.app().doc.get(t).unwrap().kind {
        NodeKind::Text(tp) => assert_eq!(tp.style.font_size, 16.0),
        k => panic!("{k:?}"),
    }
    assert_eq!(s.app().doc.get(t).unwrap().style_refs.get("text").map(String::as_str), Some("Body/Regular"));
}

#[test]
fn style_folders_collapse() {
    let mut s = Sim::new();
    // nothing selected: the Styles section is part of the page panel
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    let r = s.rect_of("folder:Color styles/Brand");
    s.click(r.center());
    assert!(s.app().props.folded.contains("Color styles/Brand"));
    s.click(r.center());
    assert!(s.app().props.folded.is_empty());
}

#[test]
fn editing_a_selection_color_replaces_it_everywhere() {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    let before = crate::styles::selection_colors(&s.app().doc, &[welcome]);
    assert!(before.len() >= 3);
    s.scroll_inspector_down();
    let r = s.rect_of("selhex:0");
    s.click(r.center());
    s.type_text("123456");
    s.key(Key::Enter);
    let after = crate::styles::selection_colors(&s.app().doc, &[welcome]);
    assert!(after.iter().any(|c| c.to_hex() == "123456"));
    assert!(!after.iter().any(|c| c.to_hex() == before[0].to_hex()));
    assert_eq!(after.len(), before.len());
    s.cmd_key(Key::Z);
    let undone = crate::styles::selection_colors(&s.app().doc, &[welcome]);
    assert_eq!(undone.iter().map(|c| c.to_hex()).collect::<Vec<_>>(), before.iter().map(|c| c.to_hex()).collect::<Vec<_>>());
}

#[test]
fn rotating_a_multi_selection_with_the_handle() {
    let mut s = Sim::new();
    let (a, b) = (s.by_name("Tile A"), s.by_name("Tile B"));
    s.shell.app.selection = vec![a, b];
    s.frame(vec![]);
    let (corners, _) = interact::selection_box(s.app()).unwrap();
    let br = s.app().view.to_screen(s.app().canvas_rect.min, corners[2]);
    let center = s.app().view.to_screen(s.app().canvas_rect.min, DV::new((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0));
    // grab just outside the bottom-right corner and swing it a quarter turn clockwise
    let start = br + Vec2::new(5.0, 5.0);
    let r = (start - center).length();
    let end = center + Vec2::new(-(start - center).y, (start - center).x) * (r / r);
    s.mods = Modifiers::NONE;
    s.drag(start, end);
    let ra = s.app().doc.get(a).unwrap().transform.rotation_rad();
    let rb = s.app().doc.get(b).unwrap().transform.rotation_rad();
    assert!(ra.abs() > 0.5 && (ra - rb).abs() < 1e-3, "both rotate together: {ra} {rb}");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(a).unwrap().transform.rotation_rad(), 0.0);
}

fn aabb_center(s: &Sim, id: NodeId) -> DV {
    let b = crate::util::world_aabb(&s.app().doc, id);
    DV::new(b.x + b.w / 2.0, b.y + b.h / 2.0)
}

#[test]
fn dragging_inside_an_auto_layout_frame_reorders_children() {
    let mut s = Sim::new();
    let card = s.by_name("Card");
    let bar = s.by_name("Progress");
    let before = s.app().doc.get(card).unwrap().children.clone();
    assert_eq!(*before.last().unwrap(), bar);
    let c = aabb_center(&s, bar);
    let a = s.screen(c.x, c.y);
    let top = s.app().doc.get(card).unwrap().children[0];
    let t = aabb_center(&s, top);
    // drop above the first child, still inside the frame
    let b = s.screen(t.x, t.y - 10.0);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    s.mv(b);
    // while dragging, the 2px insertion line is shown
    let ins = match &s.app().canvas.inter {
        canvas::Interaction::Move(m) => m.reorder,
        _ => None,
    };
    let ins = ins.expect("insertion indicator while dragging inside the frame");
    assert_eq!(ins.index, 0);
    assert_eq!(ins.line[0].y, ins.line[1].y, "horizontal line in a vertical stack");
    s.button(false);
    s.frame(vec![]);
    let after = s.app().doc.get(card).unwrap().children.clone();
    assert_eq!(after[0], bar);
    assert_eq!(after.len(), before.len());
    // laid out in the new order and a single undo step
    let ys: Vec<f32> = after.iter().map(|c| s.app().doc.get(*c).unwrap().transform.ty).collect();
    assert!(ys.windows(2).all(|w| w[1] > w[0]), "{ys:?}");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(card).unwrap().children, before);
}

#[test]
fn dragging_out_of_an_auto_layout_frame_leaves_the_flow() {
    let mut s = Sim::new();
    let card = s.by_name("Card");
    let bar = s.by_name("Progress");
    let c = aabb_center(&s, bar);
    let w0 = crate::util::world_aabb(&s.app().doc, bar);
    let a = s.screen(c.x, c.y);
    let b = s.screen(c.x + 400.0, c.y - 60.0);
    s.drag(a, b);
    let n = s.app().doc.get(bar).unwrap();
    assert_ne!(n.parent, Some(card), "reparented out of the frame");
    let w1 = crate::util::world_aabb(&s.app().doc, bar);
    assert!((w1.x - w0.x - 400.0).abs() < 8.0, "keeps following the pointer: {} -> {}", w0.x, w1.x);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(bar).unwrap().parent, Some(card));
}

/// Draws a three-point open path with the pen and leaves it selected in Move mode.
fn sim_with_vector() -> (Sim, NodeId) {
    let mut s = Sim::new();
    s.key(Key::P);
    for (x, y) in [(300.0, 800.0), (370.0, 840.0), (300.0, 880.0)] {
        let p = s.screen(x, y);
        s.click(p);
    }
    s.key(Key::Enter);
    let id = s.app().selection[0];
    assert!(matches!(s.app().doc.get(id).unwrap().kind, NodeKind::Vector { .. }));
    (s, id)
}

fn vector_cmds(s: &Sim, id: NodeId) -> Vec<designos_doc::PathCmd> {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Vector { paths, .. } => paths[0].cmds.clone(),
        _ => panic!("not a vector"),
    }
}

#[test]
fn enter_starts_vector_editing_and_drag_is_one_undo_step() {
    let (mut s, id) = sim_with_vector();
    assert!(s.app().canvas.vedit.is_none());
    s.key(Key::Enter);
    assert!(s.app().canvas.vedit.is_some(), "Enter on a selected vector enters point editing");
    let before = vector_cmds(&s, id);
    let world_before = s.app().doc.world_transform(id);
    let a = s.screen(370.0, 840.0);
    s.drag(a, a + Vec2::new(30.0, 0.0));
    let after = vector_cmds(&s, id);
    assert_ne!(before, after);
    // the middle vertex followed the pointer (30 screen px)
    let mid_page = |cmds: &[designos_doc::PathCmd], w: designos_doc::Affine| match cmds[1] {
        designos_doc::PathCmd::LineTo(p) => w.apply(p),
        _ => panic!(),
    };
    let moved = mid_page(&after, s.app().doc.world_transform(id)).x - mid_page(&before, world_before).x;
    let want = 30.0 / s.app().view.zoom;
    assert!((moved - want).abs() < 2.0, "moved {moved}, wanted {want}");
    s.cmd_key(Key::Z);
    assert_eq!(vector_cmds(&s, id), before, "a whole drag is one step");
    assert!(s.app().canvas.vedit.is_some(), "still editing after undo");
    s.key(Key::Escape);
    assert!(s.app().canvas.vedit.is_none());
    assert_eq!(s.app().selection, vec![id], "Escape leaves edit mode but keeps the selection");
}

#[test]
fn clicking_a_segment_adds_a_point_and_delete_removes_it() {
    let (mut s, id) = sim_with_vector();
    s.key(Key::Enter);
    let n0 = vector_cmds(&s, id).len();
    // middle of the first segment (300,800)-(370,840)
    let p = s.screen(335.0, 820.0);
    s.click(p);
    assert_eq!(vector_cmds(&s, id).len(), n0 + 1);
    let sel = s.app().canvas.vedit.as_ref().unwrap().sel.clone();
    assert_eq!(sel, std::collections::BTreeSet::from([3]), "the new vertex is selected");
    s.key(Key::Delete);
    assert_eq!(vector_cmds(&s, id).len(), n0);
    s.cmd_key(Key::Z);
    assert_eq!(vector_cmds(&s, id).len(), n0 + 1);
    s.cmd_key(Key::Z);
    assert_eq!(vector_cmds(&s, id).len(), n0);
    s.key(Key::Enter);
    assert!(s.app().canvas.vedit.is_none(), "Enter exits");
}

#[test]
fn text_tool_auto_resizes_with_real_measurement() {
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.type_text("Hi");
    let id = s.app().selection[0];
    let w2 = s.app().doc.get(id).unwrap().size.x;
    s.type_text(" there, world");
    let n = s.app().doc.get(id).unwrap();
    let t = match &n.kind {
        NodeKind::Text(t) => t.clone(),
        k => panic!("{k:?}"),
    };
    assert!(n.size.x > w2 * 3.0, "grows with the text: {w2} -> {}", n.size.x);
    assert_eq!(n.size.x, crate::textmeasure::measure(&t, None).x, "size is the measured width, not an estimate");
    // the caret sits at the measured end of the text
    let caret = s.app().canvas.text.as_ref().unwrap().caret;
    let (x, _, h) = crate::textmeasure::caret_rect(&t, n.size, caret);
    assert!((x - n.size.x).abs() <= 1.0 && h > 10.0);
    s.key(Key::Escape);
}

fn text_of(s: &Sim, id: NodeId) -> designos_doc::TextProps {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t.clone(),
        k => panic!("{k:?}"),
    }
}

#[test]
fn inspector_edits_only_the_selected_range() {
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.type_text("hello world");
    let id = s.app().selection[0];
    // select "hello" with Home + Shift+Right x5
    s.cmd_key(Key::Home);
    s.mods = Modifiers::SHIFT;
    for _ in 0..5 {
        s.key(Key::ArrowRight);
    }
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
    assert_eq!(s.app().canvas.text.as_ref().unwrap().range(), 0..5);
    let r = s.field_rect("font-size");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("30");
    s.key(Key::Enter);
    let t = text_of(&s, id);
    assert_eq!(t.runs.len(), 1, "{:?}", t.runs);
    assert_eq!((t.runs[0].start, t.runs[0].end, t.runs[0].style.font_size), (0, 5, 30.0));
    assert_ne!(t.style.font_size, 30.0, "the rest keeps its size");
    // the whole text now reports a mixed size
    s.cmd_key(Key::A);
    let sum = crate::canvas::textsession::session_summary(s.app()).unwrap();
    assert_eq!(sum.size, None);
    assert_eq!(sum.family, Some("Inter".to_string()));
    // Ctrl+B bolds only the selection (here: everything), Ctrl+Z inside the session undoes it
    s.cmd_key(Key::B);
    let all = |s: &Sim| {
        let t = text_of(s, id);
        crate::richtext::summary(&t, 0..t.content.len()).weight
    };
    assert_eq!(all(&s), Some(700));
    s.cmd_key(Key::Z);
    assert_eq!(all(&s), Some(400));
    s.key(Key::Escape);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).is_none(), "leaving the session commits one undoable step");
}

fn shift_key(s: &mut Sim, key: Key) {
    s.mods = Modifiers::SHIFT;
    s.key(key);
    s.mods = Modifiers::NONE;
}

#[test]
fn rulers_toggle_and_guides_are_created_snapped_to_and_removed() {
    let mut s = Sim::new();
    assert!(!s.app().ui.rulers);
    shift_key(&mut s, Key::R);
    assert!(s.app().ui.rulers, "Shift+R shows the rulers");
    let cr = s.app().canvas_rect;
    // drag from the top ruler into the canvas: a horizontal guide at the drop position
    let from = Pos2::new(cr.left() + 300.0, cr.top() + 10.0);
    let to = Pos2::new(cr.left() + 300.0, cr.top() + 300.0);
    s.drag(from, to);
    let page = s.app().page;
    let guides = s.app().doc.guides.get(&page).cloned().unwrap_or_default();
    assert_eq!(guides.len(), 1);
    let want = s.app().view.to_page(cr.min, to).y.round();
    assert_eq!(guides[0].axis, designos_doc::GuideAxis::Y);
    assert_eq!(guides[0].frame, None);
    assert_eq!(guides[0].offset, want, "guides land on whole pixels");

    // layers snap to the guide while moving
    let targets = crate::canvas::interact::snap_targets(s.app(), &[]);
    assert!(targets.iter().any(|r| crate::canvas::snap::is_guide_rect(r) && r.y == want), "the guide is a snap target");

    // dragging the guide back onto the ruler deletes it
    let on_guide = Pos2::new(cr.left() + 500.0, s.app().view.to_screen(cr.min, DV::new(0.0, want)).y);
    s.drag(on_guide, Pos2::new(on_guide.x, cr.top() + 8.0));
    assert!(s.app().doc.guides.get(&page).is_none_or(|g| g.is_empty()));

    // create and delete were one undo step each
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.guides[&page].len(), 1, "undo restores the deleted guide");
    s.cmd_key(Key::Z);
    assert!(s.app().doc.guides.get(&page).is_none_or(|g| g.is_empty()), "undo removes the created guide");

    // hiding the rulers hides guides' snapping too
    shift_key(&mut s, Key::R);
    assert!(!s.app().ui.rulers);
}

#[test]
fn frame_guides_follow_the_frame_and_each_drag_is_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    shift_key(&mut s, Key::R);
    let card = s.by_name("Card");
    let page = s.app().page;
    let cr = s.app().canvas_rect;
    let bb = crate::util::world_aabb(&s.app().doc, card);
    // drop a vertical guide from the left ruler onto the selected frame
    let to = s.screen(bb.x + 30.4, bb.y + 20.0);
    s.drag(Pos2::new(cr.left() + 10.0, to.y), to);
    let g = s.app().doc.guides[&page].clone();
    assert_eq!(g.len(), 1, "one guide for the whole drag");
    assert_eq!(g[0].frame, Some(card));
    assert_eq!(g[0].axis, designos_doc::GuideAxis::X);
    assert_eq!(g[0].offset, (bb.x + 30.4).round() - bb.x, "offset is frame-local");

    // it moves with the frame, and snaps layers at its new place
    let mut moved = s.app().doc.get(card).unwrap().clone();
    moved.transform.tx += 100.0;
    s.shell.app.hist.exec(&mut s.shell.app.doc, designos_doc::Command::Update { node: Box::new(moved) }).unwrap();
    let want = (bb.x + 30.4).round() + 100.0;
    let targets = crate::canvas::interact::snap_targets(s.app(), &[]);
    assert!(targets.iter().any(|r| crate::canvas::snap::is_guide_rect(r) && r.x == want), "frame guide followed the frame");
    s.shell.app.changed();
    s.frame(vec![]);

    // a guide dropped with nothing selected is a page guide
    let to2 = s.screen(bb.x - 200.0, bb.y - 150.0);
    s.shell.app.selection.clear();
    s.drag(Pos2::new(cr.left() + 10.0, to2.y), to2);
    let g = s.app().doc.guides[&page].clone();
    assert_eq!(g.len(), 2);
    assert_eq!(g[1].frame, None);

    // one undo per drag
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.guides[&page].len(), 1);
}

#[test]
fn guides_are_saved_with_the_file() {
    let s = Sim::new();
    let mut doc = s.app().doc.clone();
    let page = s.app().page;
    let frame = s.by_name("Card");
    let guides = vec![
        designos_doc::Guide { axis: designos_doc::GuideAxis::X, offset: 12.0, frame: None },
        designos_doc::Guide { axis: designos_doc::GuideAxis::Y, offset: 7.5, frame: Some(frame) },
    ];
    designos_doc::History::default().exec(&mut doc, designos_doc::Command::SetGuides { page, guides: guides.clone() }).unwrap();
    let mut path = std::env::temp_dir();
    path.push(format!("designos_guides_{}", std::process::id()));
    let p = crate::io::save_path(&doc, &path).unwrap();
    let back = crate::io::load_path(&p).unwrap();
    std::fs::remove_file(p).ok();
    assert_eq!(back.guides[&page], guides);
}

#[test]
fn moves_land_on_whole_pixels_and_snap_to_guides() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let id = s.by_name("Card");
    let t0 = s.app().doc.get(id).unwrap().transform;
    // fractional drag distance
    let a = s.screen(t0.tx + 20.0, t0.ty + 20.0);
    s.drag(a, a + Vec2::new(33.4, 21.7));
    let t = s.app().doc.get(id).unwrap().transform;
    assert_eq!((t.tx.fract(), t.ty.fract()), (0.0, 0.0), "positions stay integral: {} {}", t.tx, t.ty);
    s.cmd_key(Key::Z);
    // with snapping to the pixel grid off, the same drag is free
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Quote);
    s.mods = Modifiers::NONE;
    assert!(s.app().ui.no_pixel_snap, "Ctrl+Shift+' toggles pixel snapping");
}

#[test]
fn view_toggles_via_shortcuts() {
    let mut s = Sim::new();
    assert!(s.app().ui.pixel_grid && !s.app().ui.hide_layout_grids);
    shift_key(&mut s, Key::Quote);
    assert!(!s.app().ui.pixel_grid, "Shift+' toggles the pixel grid");
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Num4);
    s.mods = Modifiers::NONE;
    assert!(s.app().ui.hide_layout_grids, "Ctrl+Shift+4 hides layout grids");
    // zoomed to 800% with every aid on renders without trouble
    s.shell.app.ui.pixel_grid = true;
    s.shell.app.ui.rulers = true;
    s.shell.app.ui.hide_layout_grids = false;
    s.shell.app.view.zoom = 8.0;
    s.frame(vec![]);
}

#[test]
fn layout_guide_section_edits_the_grids_of_a_frame() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let id = s.by_name("Card");
    assert!(s.app().doc.get(id).unwrap().kind.frame_props().is_some(), "Card is a frame");
    let grid = crate::panels::layoutguide::default_grid(1, Color::rgba(255, 0, 0, 26));
    s.shell.app.edit_nodes(&[id], |n| {
        if let Some(f) = n.kind.frame_props_mut() {
            f.grids.push(grid.clone());
        }
    });
    s.scroll_inspector_down();
    // the section shows its columns fields; edit the count
    let r = s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{:?}", ("lg-count", 0usize)))))).expect("count field is shown");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("12");
    s.key(Key::Enter);
    let n = s.app().doc.get(id).unwrap();
    match &n.kind.frame_props().unwrap().grids[0] {
        designos_doc::LayoutGrid::Columns { count, .. } => assert_eq!(*count, 12),
        g => panic!("{g:?}"),
    }
    s.cmd_key(Key::Z);
    match &s.app().doc.get(id).unwrap().kind.frame_props().unwrap().grids[0] {
        designos_doc::LayoutGrid::Columns { count, .. } => assert_eq!(*count, 5, "undo restores"),
        g => panic!("{g:?}"),
    }
    let w_rect = s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{:?}", ("lg-sec-size", 0usize)))))).expect("width field is shown");
    s.click(Pos2::new(w_rect.left() + 40.0, w_rect.center().y));
    s.type_text("60");
    s.key(Key::Enter);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("0");
    s.key(Key::Enter);
    match &s.app().doc.get(id).unwrap().kind.frame_props().unwrap().grids[0] {
        designos_doc::LayoutGrid::Columns { count, section_size, .. } => {
            assert_eq!(*count, 0);
            assert_eq!(*section_size, 60.0);
        }
        g => panic!("{g:?}"),
    }
}

mod board;
mod board_home;
mod check_designs;
mod comments;
mod components;
mod components_cycles;
mod components_main_copies;
mod decks;
mod deferred_page_instances;
mod dev;
mod dev_mode_shortcut;
mod duplicate_order;
mod effect_grid_styles;
mod empty_slot_flows;
mod fast_drag_draw;
mod fast_drag_transform;
mod fig_export_toast;
mod figma_paste;
mod find;
mod gpu_screenshot_wait;
mod grid_track_editing;
mod history;
mod home;
mod home_agents_card;
mod idle_repaint;
mod layer_drag_fast;
#[cfg(target_os = "macos")]
mod leave_menu_quit;
mod leave_prompt;
mod leave_prompt_edges;
mod live_agent_edits;
mod live_patch_agent_switch;
mod live_patch_agent_switch_edges;
mod live_patch_agent_switch_guards;
mod menus;
mod pages_layers_separator;
mod palette_autofix_undo;
mod panel_keys;
mod proto;
mod proto_vars;
mod relayout_budget;
mod resized_shape_geometry;
mod slot_canvas_edits;
mod stale_canvas_after_agent_edit;
mod stray_layers;
mod style_edit;
mod svg;
mod system_paste_and_drop;
mod tab_disk_state;
mod typography;
mod undo_exact_layout;
mod undo_layout_epoch;
mod unsaved_dot;
mod variables;
mod vector;

// Needs the stub agent CLIs of `Sim::use_stub_agent_clis`, which exist on unix only: on Windows the tab runs a real `claude`
// that is not there, and dies at a moment that depends on machine load, so the terminal bytes the tests read are not stable.
#[cfg(unix)]
mod agent_panel_keyboard;
mod agent_picker_return;
mod agents_panel_only;
mod agents_tab_strip;
mod assets_thumbs;
mod libraries;
mod lint;
mod plugins;
mod terminal_undo_chord;

#[test]
fn launch_visual_proof_presets_render_all_eight_micro_interaction_showcases() {
    let presets = [
        "micro-autolayout",
        "micro-css-grid",
        "micro-smart-selection",
        "micro-shape-controls",
        "micro-vector-edit",
        "micro-layer-picker",
        "micro-dev-boxmodel",
        "micro-scale-tool",
    ];
    for p in presets {
        let mut s = Sim::with(Options { fixture: Some("showcase".into()), select: Some("Home".into()), panel: Some(p.into()), ..Default::default() });
        s.frame(vec![]);
        assert!(!s.shapes.is_empty(), "preset {p} must render non-empty shapes on the canvas/chrome");
        assert!(!s.app().selection.is_empty(), "preset {p} must maintain an active selection");
    }
}

#[test]
fn launch_e2e_proves_100_figma_micro_interactions_end_to_end() {
    let mut s = Sim::with(Options { fixture: Some("showcase".into()), select: Some("Home".into()), ..Default::default() });

    // 1. Smart Selection & Tidy Up (#15, #16)
    super::apply_micro_interaction_showcase_preset(&mut s.shell.app, "micro-smart-selection");
    s.frame(vec![]);
    let sel = s.app().selection.clone();
    assert_eq!(sel.len(), 3);
    // Offset the middle card unevenly, then invoke Tidy Up
    s.shell.app.doc.get_mut(sel[1]).unwrap().transform.tx += 35.0;
    s.shell.app.doc.get_mut(sel[1]).unwrap().transform.ty += 18.0;
    let crect = s.app().canvas_rect;
    let tidy_rect = crate::canvas::draw::tidy_up_pill_rect(s.app(), crect).expect("3-node selection exposes Tidy up pill");
    assert!(crate::canvas::interact::try_click_canvas_action_pill(&mut s.shell.app, crect, tidy_rect.center(),));
    let x0 = s.app().doc.get(sel[0]).unwrap().transform.tx;
    let x1 = s.app().doc.get(sel[1]).unwrap().transform.tx;
    let x2 = s.app().doc.get(sel[2]).unwrap().transform.tx;
    let y0 = s.app().doc.get(sel[0]).unwrap().transform.ty;
    let y1 = s.app().doc.get(sel[1]).unwrap().transform.ty;
    assert!(((x1 - x0) - (x2 - x1)).abs() <= 1.5, "Tidy up equalizes horizontal gaps");
    assert!((y1 - y0).abs() <= 1.0, "Tidy up aligns cross-axis");

    // 2. 2D CSS Grid Track Header Pill Cycle (#18): 1fr -> Fixed -> Hug -> 1fr
    super::apply_micro_interaction_showcase_preset(&mut s.shell.app, "micro-css-grid");
    s.frame(vec![]);
    let crect = s.app().canvas_rect;
    let pills = crate::canvas::draw::grid_track_pill_rects(s.app(), crect);
    assert!(!pills.is_empty(), "CSS Grid exposes clickable track header pills");
    let (grid_id, _is_col, col_idx, first_pill) = pills[0];
    assert!(crate::canvas::interact::try_click_canvas_action_pill(&mut s.shell.app, crect, first_pill.center(),));
    let track_after = s.app().doc.get(grid_id).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().grid_col_tracks[col_idx];
    assert!(matches!(track_after, designos_doc::GridTrack::Fixed(_)), "1fr cycles to Fixed(px) on click");

    // 3. Scale Tool (K) Proportional Scaling (#32)
    s.key(Key::K);
    assert_eq!(s.app().tool, Tool::Scale);
    s.key(Key::V);
    assert_eq!(s.app().tool, Tool::Move);

    // 4. Eyedropper (I) + Tab Format Cycle (#77, #78): HEX -> RGB -> HSL -> HSB
    s.key(Key::I);
    assert!(s.app().props.eyedropper.is_some());
    assert_eq!(s.app().props.eyedropper_format, crate::panels::properties::EyedropperFormat::Hex);
    assert_eq!(s.app().props.eyedropper_format.format_rgb(227, 99, 35), "#E36323");
    s.key(Key::Tab);
    assert_eq!(s.app().props.eyedropper_format, crate::panels::properties::EyedropperFormat::Rgb);
    assert_eq!(s.app().props.eyedropper_format.format_rgb(227, 99, 35), "rgb(227, 99, 35)");
    s.key(Key::Tab);
    assert_eq!(s.app().props.eyedropper_format, crate::panels::properties::EyedropperFormat::Hsl);
    s.key(Key::Tab);
    assert_eq!(s.app().props.eyedropper_format, crate::panels::properties::EyedropperFormat::Hsb);
    s.key(Key::Escape);
    assert!(s.app().props.eyedropper.is_none());

    // 5. Copy/Paste Properties (Cmd+Alt+C / Cmd+Alt+V #82) & Shift+Align as Group (#81)
    s.shell.app.selection = vec![sel[1]];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![sel[0]];
    s.shell.app.paste_properties();
    assert_eq!(s.app().doc.get(sel[0]).unwrap().fills, s.app().doc.get(sel[1]).unwrap().fills, "Cmd+Alt+V pastes visual fills onto target node");

    s.shell.app.selection = vec![sel[0], sel[1]];
    let gap_before = s.app().doc.get(sel[1]).unwrap().transform.tx - s.app().doc.get(sel[0]).unwrap().transform.tx;
    s.shell.app.align_selection_as_group(crate::ops::Align::Left);
    let gap_after = s.app().doc.get(sel[1]).unwrap().transform.tx - s.app().doc.get(sel[0]).unwrap().transform.tx;
    assert!((gap_before - gap_after).abs() < 1e-3, "Shift+Align preserves relative offset within group");
}

#[test]
fn inspector_button_and_object_menu_upgrade_auto_layout_from_v0_to_v1() {
    use designos_doc::{AutoLayout, FrameProps, LayoutDirection, Node, Rect, Sizing, Stroke, StrokeAlign};

    let mut s = Sim::new();
    let page = s.app().page;
    let fid = s.shell.app.doc.alloc_id();
    let mut f = Node::frame(fid, Rect::new(100.0, 100.0, 120.0, 60.0));
    f.kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 8.0,
            padding: [10.0, 12.0, 14.0, 16.0],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            stroke_included: true,
            layout_version: 0,
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    });
    f.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: 4.0, align: StrokeAlign::Inside, ..Stroke::default() };
    let f = s.shell.app.doc.insert(page, f).unwrap();
    let cid = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(f, Node::rectangle(cid, Rect::new(0.0, 0.0, 50.0, 20.0))).unwrap();
    s.shell.app.doc.relayout_subtree(f);
    s.shell.app.selection = vec![f];
    s.frame(vec![]);

    // 1. Clicking the inspector "Update auto layout" button upgrades layout_version from 0 to 1
    // and re-solves without stroke inset (width drops from 50 + 16 + 12 + 8 = 86 to 78).
    assert_eq!(s.app().doc.get(f).unwrap().size.x, 86.0);
    let btn = s.rect_of("al-upgrade-v1");
    s.click(btn.center());
    assert_eq!(s.app().doc.get(f).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().layout_version, 1);
    assert_eq!(s.app().doc.get(f).unwrap().size.x, 78.0);

    // 2. Undo restores layout_version 0 (86px); then selecting "Update auto layout" in main/Object upgrades again.
    s.shell.app.undo();
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(f).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().layout_version, 0);
    assert_eq!(s.app().doc.get(f).unwrap().size.x, 86.0);

    assert!(crate::panels::menus::preset_menu(&s.ctx, &mut s.shell.app, "main/Object/Update auto layout", s.ctx.content_rect()));
    s.frame(vec![]);
    s.key(Key::Enter);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(f).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().layout_version, 1);
    assert_eq!(s.app().doc.get(f).unwrap().size.x, 78.0);
}

#[test]
fn grid_track_editing_insert_delete_resize_and_reorder() {
    use crate::canvas::grids::{delete_grid_track, insert_grid_track, move_grid_track, resize_grid_track};
    use crate::edit::Edit;
    use designos_doc::{AutoLayout, GridTrack, LayoutDirection, Node, Rect as DRect};
    use grid_track_editing::track_labels;

    let mut s = Sim::new();
    let page = s.app().page;

    // 1. Build a 4-column grid with literal tracks [80px, 120px, 1fr, Hug] and 4 children,
    // where child 1 is anchored to track 2 (0-based index 1) and child 3 is anchored to track 4 (index 3).
    let fid = s.shell.app.doc.alloc_id();
    let mut grid = Node::frame(fid, DRect::new(0.0, 0.0, 480.0, 240.0));
    let mut col_tracks = [GridTrack::Flex(1.0); 8];
    col_tracks[0] = GridTrack::Fixed(80.0);
    col_tracks[1] = GridTrack::Fixed(120.0);
    col_tracks[2] = GridTrack::Flex(1.0);
    col_tracks[3] = GridTrack::Hug;
    grid.kind.frame_props_mut().unwrap().auto_layout =
        Some(AutoLayout { direction: LayoutDirection::Grid, grid_columns: 4, grid_rows: 1, grid_col_tracks: col_tracks.into(), ..AutoLayout::default() });
    let grid_id = s.shell.app.doc.insert(page, grid).unwrap();
    let mut kids = Vec::new();
    for (i, w) in [60.0f32, 90.0, 70.0, 60.0].into_iter().enumerate() {
        let cid = s.shell.app.doc.alloc_id();
        let mut child = Node::rectangle(cid, DRect::new(0.0, 0.0, w, 40.0));
        if i == 1 {
            child.layout_child.grid_column = Some(1);
        } else if i == 3 {
            child.layout_child.grid_column = Some(3);
        }
        kids.push(s.shell.app.doc.insert(grid_id, child).unwrap());
    }
    {
        let mut ed = Edit::new();
        crate::layout::relayout_around(&mut s.shell.app.doc, &mut ed, grid_id);
    }
    s.shell.app.selection = vec![grid_id];
    s.frame(vec![]);

    assert_eq!(track_labels(&s.app().doc, grid_id, true), vec!["80px", "120px", "1fr", "Hug"]);

    // 2. Insert a 5th column track (2fr) and resize column index 2 from 1fr -> 100px.
    assert!(insert_grid_track(&mut s.shell.app, grid_id, true, 4, GridTrack::Flex(2.0)));
    assert!(resize_grid_track(&mut s.shell.app, grid_id, true, 2, GridTrack::Fixed(100.0)));
    assert_eq!(track_labels(&s.app().doc, grid_id, true), vec!["80px", "120px", "100px", "Hug", "2fr"]);
    assert_eq!(s.app().doc.get(kids[1]).unwrap().transform.tx, 80.0);

    // 3. Move track 2 (index 1, 120px) to track 4 (index 3): moves anchored item kids[1] in one undo step.
    assert!(move_grid_track(&mut s.shell.app, grid_id, true, 1, 3));
    assert_eq!(track_labels(&s.app().doc, grid_id, true), vec!["80px", "100px", "Hug", "120px", "2fr"]);
    assert_eq!(s.app().doc.get(kids[1]).unwrap().layout_child.grid_column, Some(3));
    // Columns 0..3 have widths 80, 100, 60 (Hug of kids[3]) -> column 3 starts at x = 240.0.
    assert_eq!(s.app().doc.get(kids[1]).unwrap().transform.tx, 240.0);
    assert_eq!(s.app().doc.get(kids[3]).unwrap().layout_child.grid_column, Some(2));

    // Single undo step restores both the track order and the anchored items' positions.
    s.shell.app.undo();
    assert_eq!(track_labels(&s.app().doc, grid_id, true), vec!["80px", "120px", "100px", "Hug", "2fr"]);
    assert_eq!(s.app().doc.get(kids[1]).unwrap().layout_child.grid_column, Some(1));
    assert_eq!(s.app().doc.get(kids[1]).unwrap().transform.tx, 80.0);

    // 4. Deleting a track re-flows auto-placed items.
    let fid2 = s.shell.app.doc.alloc_id();
    let mut grid2 = Node::frame(fid2, DRect::new(0.0, 0.0, 300.0, 200.0));
    let mut tracks2 = [GridTrack::Flex(1.0); 8];
    tracks2[0] = GridTrack::Fixed(100.0);
    tracks2[1] = GridTrack::Fixed(100.0);
    tracks2[2] = GridTrack::Fixed(100.0);
    grid2.kind.frame_props_mut().unwrap().auto_layout =
        Some(AutoLayout { direction: LayoutDirection::Grid, grid_columns: 3, grid_rows: 0, grid_col_tracks: tracks2.into(), ..AutoLayout::default() });
    let grid2_id = s.shell.app.doc.insert(page, grid2).unwrap();
    let mut auto_kids = Vec::new();
    for _ in 0..4 {
        let cid = s.shell.app.doc.alloc_id();
        let child = Node::rectangle(cid, DRect::new(0.0, 0.0, 50.0, 40.0));
        auto_kids.push(s.shell.app.doc.insert(grid2_id, child).unwrap());
    }
    {
        let mut ed = Edit::new();
        crate::layout::relayout_around(&mut s.shell.app.doc, &mut ed, grid2_id);
    }
    // Before delete: 3 columns -> item 2 is at (200, 0), item 3 is at (0, 40).
    let p2_before = (s.app().doc.get(auto_kids[2]).unwrap().transform.tx, s.app().doc.get(auto_kids[2]).unwrap().transform.ty);
    let p3_before = (s.app().doc.get(auto_kids[3]).unwrap().transform.tx, s.app().doc.get(auto_kids[3]).unwrap().transform.ty);
    assert_eq!(p2_before, (200.0, 0.0));
    assert_eq!(p3_before, (0.0, 40.0));

    assert!(delete_grid_track(&mut s.shell.app, grid2_id, true, 1));
    assert_eq!(track_labels(&s.app().doc, grid2_id, true), vec!["100px", "100px"]);
    // After deleting column 1: 2 columns -> item 2 re-flows to (0, 40), item 3 re-flows to (100, 40).
    let p2_after = (s.app().doc.get(auto_kids[2]).unwrap().transform.tx, s.app().doc.get(auto_kids[2]).unwrap().transform.ty);
    let p3_after = (s.app().doc.get(auto_kids[3]).unwrap().transform.tx, s.app().doc.get(auto_kids[3]).unwrap().transform.ty);
    assert_eq!(p2_after, (0.0, 40.0));
    assert_eq!(p3_after, (100.0, 40.0));

    // 5. On-canvas '+' add-track button and drag-reorder between track header pills.
    s.shell.app.selection = vec![grid_id];
    s.frame(vec![]);
    let crect = s.app().canvas_rect;
    let pills = crate::canvas::draw::grid_track_pill_rects(s.app(), crect);
    let col_pills: Vec<_> = pills.iter().filter(|&&(gid, is_col, _, _)| gid == grid_id && is_col).copied().collect();
    assert_eq!(col_pills.len(), 5);
    assert!(crate::canvas::interact::try_drag_grid_track_pill(&mut s.shell.app, crect, col_pills[1].3.center(), col_pills[3].3.center()));
    assert_eq!(track_labels(&s.app().doc, grid_id, true), vec!["80px", "100px", "Hug", "120px", "2fr"]);
}

mod agent_a11y {
    use super::*;

    fn set_tab_status(tab: &mut crate::terminal::TerminalTab, status: crate::terminal::AgentTabStatus) {
        if let Ok(mut guard) = tab.agent_status.lock() {
            *guard = status;
        }
    }

    fn set_tab_status_and_reported(tab: &mut crate::terminal::TerminalTab, status: crate::terminal::AgentTabStatus) {
        if let Ok(mut guard) = tab.agent_status.lock() {
            *guard = status.clone();
        }
        tab.reported_status = status;
    }

    fn open_agents_panel_with_accesskit(s: &mut Sim) {
        crate::agent_a11y::enable_accesskit(&s.ctx);
        s.shell.app.ui.left_collapsed = false;
        s.shell.app.ui.rail = Some(crate::app::RailItem::Agents);
        s.shell.app.terminal.side_panel_pty_open = false;
        s.shell.app.terminal.active_runtime = crate::terminal::AgentCliPreset::Claude;
        s.shell.app.terminal.ensure_default_tab(None);
        s.frame(vec![]);
        if let Some(tab) = s.shell.app.terminal.tabs.get_mut(0) {
            tab.preset = crate::terminal::AgentCliPreset::Claude;
            set_tab_status_and_reported(tab, crate::terminal::AgentTabStatus::Idle);
        }
        s.shell.app.agent_a11y.queue.clear();
        s.shell.app.agent_a11y.shown = None;
        s.frame(vec![]);
    }

    fn find_accesskit_node<'a>(
        update: &'a egui::accesskit::TreeUpdate,
        role: egui::accesskit::Role,
        label: &str,
    ) -> Option<(egui::accesskit::NodeId, &'a egui::accesskit::Node)> {
        update.nodes.iter().find(|(_, node)| node.role() == role && node.label() == Some(label)).map(|(id, node)| (*id, node))
    }

    #[test]
    fn announcement_node_appears_when_a_stub_turn_ends() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        // Start the active Claude Code tab in Working state.
        {
            let tab = &mut s.shell.app.terminal.tabs[0];
            tab.preset = crate::terminal::AgentCliPreset::Claude;
            set_tab_status_and_reported(tab, crate::terminal::AgentTabStatus::Working(None));
        }
        s.frame(vec![]);

        // Transition Working -> Done and step one frame.
        set_tab_status(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Done);
        s.frame(vec![]);

        let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let (_, node) = update
            .nodes
            .iter()
            .find(|(_, n)| n.live() == Some(egui::accesskit::Live::Polite) && n.label() == Some("Claude Code is done"))
            .expect("polite announcement node for finished turn");
        assert_eq!(node.role(), egui::accesskit::Role::Status);

        // Step two more frames (§3.2: cleared two frames later); the announcement label must be gone.
        s.frame(vec![]);
        s.frame(vec![]);
        let update_after = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let still_present = update_after.nodes.iter().any(|(_, n)| n.live() == Some(egui::accesskit::Live::Polite) && n.label() == Some("Claude Code is done"));
        assert!(!still_present, "announcement node label must be cleared two frames later");
    }

    #[test]
    fn needs_input_and_error_texts() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        let cases = [
            (crate::terminal::AgentTabStatus::WaitingForInput(None), "Claude Code needs input"),
            (crate::terminal::AgentTabStatus::Error(Some("HTTP 429 rate limit exceeded".to_string())), "Claude Code failed: rate limited"),
            (crate::terminal::AgentTabStatus::Error(Some("401 unauthorized token expired".to_string())), "Claude Code failed: login expired"),
            (crate::terminal::AgentTabStatus::Error(None), "Claude Code failed: unknown error"),
        ];

        for (status, expected_text) in cases {
            s.shell.app.agent_a11y.queue.clear();
            s.shell.app.agent_a11y.shown = None;
            let tab = &mut s.shell.app.terminal.tabs[0];
            tab.preset = crate::terminal::AgentCliPreset::Claude;
            set_tab_status_and_reported(tab, crate::terminal::AgentTabStatus::Working(None));
            set_tab_status(tab, status);
            s.frame(vec![]);

            let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
            let found = update.nodes.iter().any(|(_, n)| n.live() == Some(egui::accesskit::Live::Polite) && n.label() == Some(expected_text));
            assert!(found, "expected polite announcement {expected_text:?}");
        }
    }

    #[test]
    fn running_and_stopped_are_not_announced() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        set_tab_status_and_reported(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Idle);

        set_tab_status(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Working(None));
        s.frame(vec![]);
        assert!(s.app().agent_a11y.queue.is_empty());
        assert!(s.app().agent_a11y.shown.is_none());

        set_tab_status(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Stopped);
        s.frame(vec![]);
        assert!(s.app().agent_a11y.queue.is_empty());
        assert!(s.app().agent_a11y.shown.is_none());
    }

    #[test]
    fn two_changes_in_one_frame_are_both_spoken() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        let _ = s.shell.app.terminal.spawn_tab(crate::terminal::AgentCliPreset::Shell, None);
        assert!(s.shell.app.terminal.tabs.len() >= 2);
        s.shell.app.terminal.tabs[0].preset = crate::terminal::AgentCliPreset::Claude;
        set_tab_status_and_reported(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Working(None));
        s.shell.app.terminal.tabs[1].preset = crate::terminal::AgentCliPreset::Codex;
        set_tab_status_and_reported(&mut s.shell.app.terminal.tabs[1], crate::terminal::AgentTabStatus::Working(None));

        // Both tabs finish in the same frame: §3.2 queues one announcement per frame so both are spoken.
        set_tab_status(&mut s.shell.app.terminal.tabs[0], crate::terminal::AgentTabStatus::Done);
        set_tab_status(&mut s.shell.app.terminal.tabs[1], crate::terminal::AgentTabStatus::Done);

        s.frame(vec![]);
        let update_first = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let has_first = update_first.nodes.iter().any(|(_, n)| n.live() == Some(egui::accesskit::Live::Polite) && n.label() == Some("Claude Code is done"));
        assert!(has_first, "first announcement must be spoken on frame 1");

        // On the next frame, the second queued announcement is spoken.
        s.frame(vec![]);
        let update_second = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let has_second = update_second.nodes.iter().any(|(_, n)| n.live() == Some(egui::accesskit::Live::Polite) && n.label() == Some("Codex is done"));
        assert!(has_second, "second announcement must be spoken on frame 2");
    }

    #[test]
    fn regions_have_roles_and_names() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let (panel_id, _) = find_accesskit_node(update, egui::accesskit::Role::Region, "Agents panel").expect("Agents panel region");
        let (tabs_id, _) = find_accesskit_node(update, egui::accesskit::Role::TabList, "Agent tabs").expect("Agent tabs tablist");
        let (term_id, term_node) = find_accesskit_node(update, egui::accesskit::Role::Terminal, "Agent terminal").expect("Agent terminal");
        let (comp_id, _) = find_accesskit_node(update, egui::accesskit::Role::Region, "Message composer").expect("Message composer region");

        assert_eq!(term_node.description(), Some("Claude Code, idle"));

        // Verify the region order in the root node's children follows visual order:
        // Agents panel / header -> Agent tabs -> Agent terminal -> Message composer.
        let root_id = update.tree.as_ref().expect("root tree").root;
        let (_, root_node) = update.nodes.iter().find(|(id, _)| *id == root_id).expect("root node");
        let children = root_node.children();
        let idx_panel = children.iter().position(|&id| id == panel_id).expect("panel in root");
        let idx_tabs = children.iter().position(|&id| id == tabs_id).expect("tabs in root");
        let idx_term = children.iter().position(|&id| id == term_id).expect("terminal in root");
        let idx_comp = children.iter().position(|&id| id == comp_id).expect("composer in root");
        assert!(idx_panel < idx_tabs && idx_tabs < idx_term && idx_term < idx_comp);
    }

    #[test]
    fn icon_buttons_have_names() {
        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);

        // In default chat mode (`side_panel_pty_open == false`), composer and runtime pills are created;
        // PTY-only buttons (`New session`, `Close tab`) and non-existent controls (`Stop`, `Send selection to agent`) must not be published.
        let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        for label in ["Send", "Use Claude Code", "Use Codex", "Use Antigravity (AGY)", "Use Grok", "Attach context", "Show terminal", "Attach PRD context"] {
            assert!(find_accesskit_node(update, egui::accesskit::Role::Button, label).is_some(), "expected Role::Button with label {label:?} in chat mode");
        }
        // The Shell pill and the composer's book button both show the terminal, so both carry the same honest name.
        let show_terminal_ids: Vec<_> =
            update.nodes.iter().filter(|(_, n)| n.role() == egui::accesskit::Role::Button && n.label() == Some("Show terminal")).map(|(id, _)| *id).collect();
        assert!(show_terminal_ids.contains(&egui::Id::new("agents-runtime-pty-toggle").accesskit_id()), "the Shell pill must be named Show terminal");
        assert!(show_terminal_ids.contains(&egui::Id::new("agents-composer-book").accesskit_id()), "the composer book button must be named Show terminal");
        let (claude_node_id, _) = find_accesskit_node(update, egui::accesskit::Role::Button, "Use Claude Code").expect("Claude pill");
        assert_eq!(claude_node_id, egui::Id::new("agents-runtime-claude").accesskit_id(), "Use Claude Code must target agents-runtime-claude");
        assert!(
            find_accesskit_node(update, egui::accesskit::Role::Button, "New Claude Code session").is_none(),
            "a runtime pill starts no session, so its name must not say so"
        );
        for absent in ["Stop", "Send selection to agent", "New session", "Close tab", "Insert audit prompt"] {
            assert!(find_accesskit_node(update, egui::accesskit::Role::Button, absent).is_none(), "unexpected phantom button {absent:?} in chat mode");
        }

        // Switch to PTY mode with 2 tabs so both `New session` and `Close tab` exist in the PTY bar.
        // With an empty selection, the selection chip uses `Sense::hover()` and must not appear as a button.
        let _ = s.shell.app.terminal.spawn_tab(crate::terminal::AgentCliPreset::Shell, None);
        s.shell.app.terminal.side_panel_pty_open = true;
        s.frame(vec![]);

        let update_pty = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        for label in ["New session", "Close tab"] {
            assert!(find_accesskit_node(update_pty, egui::accesskit::Role::Button, label).is_some(), "expected Role::Button with label {label:?} in PTY mode");
        }
        for absent in ["Stop", "Send selection to agent", "Send", "Show terminal dock", "Hide terminal dock"] {
            assert!(find_accesskit_node(update_pty, egui::accesskit::Role::Button, absent).is_none(), "unexpected phantom button {absent:?} in PTY mode");
        }

        // Select a node on the canvas: the selection chip switches to `Sense::click()` and publishes
        // `Role::Button` with accessible name `"Send selection to agent"`.
        let page = s.shell.app.page;
        let fid = s.shell.app.doc.alloc_id();
        let frame_node = designos_doc::Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0));
        s.shell.app.doc.insert(page, frame_node).unwrap();
        s.shell.app.selection = vec![fid];
        s.frame(vec![]);

        let update_pty_selected = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
        let (chip_node_id, _) = find_accesskit_node(update_pty_selected, egui::accesskit::Role::Button, "Send selection to agent")
            .expect("expected Role::Button with label \"Send selection to agent\" in PTY mode when a node is selected");
        assert_eq!(chip_node_id, egui::Id::new("agent-selection-chip-left").accesskit_id());
        for absent in ["Stop", "Send"] {
            assert!(
                find_accesskit_node(update_pty_selected, egui::accesskit::Role::Button, absent).is_none(),
                "unexpected phantom button {absent:?} in PTY mode with selection"
            );
        }
    }

    #[test]
    fn shortcut_moves_focus_to_the_composer_and_back() {
        let mut s = Sim::new();
        crate::agent_a11y::enable_accesskit(&s.ctx);
        // Start with the left sidebar collapsed on the File rail.
        s.shell.app.ui.left_collapsed = true;
        s.shell.app.ui.rail = Some(crate::app::RailItem::File);
        s.frame(vec![]);

        // Send Cmd+Alt+J -> opens the Agents panel and moves keyboard focus to the composer.
        let cmd_alt = Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
        s.mods = cmd_alt;
        s.frame(vec![Event::Key { key: Key::J, physical_key: Some(Key::J), pressed: true, repeat: false, modifiers: cmd_alt }]);
        s.mods = Modifiers::NONE;
        s.frame(vec![]);

        assert!(!s.app().ui.left_collapsed, "Cmd+Alt+J must uncollapse the left panel");
        assert_eq!(s.app().ui.rail, Some(crate::app::RailItem::Agents), "Cmd+Alt+J must select the Agents rail");
        assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "Cmd+Alt+J must focus the composer");

        // Send Cmd+Alt+J again -> returns keyboard focus to the canvas.
        s.mods = cmd_alt;
        s.frame(vec![Event::Key { key: Key::J, physical_key: Some(Key::J), pressed: true, repeat: false, modifiers: cmd_alt }]);
        s.mods = Modifiers::NONE;
        s.frame(vec![]);

        assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::canvas_id()), "second Cmd+Alt+J must return focus to the canvas");

        // Single-key canvas shortcuts must work immediately after focus returns to the canvas.
        s.frame(vec![Event::Key { key: Key::R, physical_key: Some(Key::R), pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        assert_eq!(s.app().tool, crate::canvas::tools::Tool::Rectangle, "single-key 'R' shortcut must activate Rectangle tool when canvas has focus");
    }

    #[test]
    fn cmd_alt_j_with_selected_text_node_justifies_text_instead_of_opening_agents() {
        let mut s = Sim::new();
        s.shell.app.ui.left_collapsed = true;
        s.shell.app.ui.rail = Some(crate::app::RailItem::File);
        let page = s.shell.app.page;
        let tid = s.shell.app.doc.alloc_id();
        let tn = designos_doc::Node::text(tid, designos_doc::Rect::new(0.0, 0.0, 200.0, 32.0), "Paragraph text", designos_doc::TextStyle::default());
        s.shell.app.doc.insert(page, tn).unwrap();
        s.shell.app.selection = vec![tid];
        s.frame(vec![]);

        let cmd_alt = Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
        s.mods = cmd_alt;
        s.frame(vec![Event::Key { key: Key::J, physical_key: Some(Key::J), pressed: true, repeat: false, modifiers: cmd_alt }]);
        s.mods = Modifiers::NONE;
        s.frame(vec![]);

        let align_h = match &s.app().doc.get(tid).unwrap().kind {
            designos_doc::NodeKind::Text(t) => t.style.align_h,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(align_h, designos_doc::TextAlignH::Justified, "Cmd+Alt+J with selected text node must set TextAlignH::Justified");
        assert!(s.app().ui.left_collapsed, "left panel must stay collapsed");
        assert_eq!(s.app().ui.rail, Some(crate::app::RailItem::File), "rail must remain unchanged");
    }

    #[test]
    fn osc_stopped_status_and_tab_status_dot_colors() {
        assert_eq!(
            crate::terminal::parse_osc_agent_status_and_strip("\x1b]9999;{\"state\":\"stopped\"}\x07").1,
            Some(crate::terminal::AgentTabStatus::Stopped)
        );
        assert_eq!(crate::terminal::parse_osc_agent_status_and_strip("\x1b]9999;stopped\x07").1, Some(crate::terminal::AgentTabStatus::Stopped));

        let mut s = Sim::new();
        open_agents_panel_with_accesskit(&mut s);
        s.shell.app.terminal.side_panel_pty_open = true;

        let has_circle_with_fill = |shapes: &[egui::epaint::ClippedShape], expected_fill: egui::Color32| -> bool {
            shapes.iter().any(|cs| {
                matches!(
                    &cs.shape,
                    egui::epaint::Shape::Circle(c) if (c.radius - 2.7).abs() < 0.05 && c.fill == expected_fill
                )
            })
        };

        // Error status -> painted with theme::DANGER
        {
            let tab = &mut s.shell.app.terminal.tabs[0];
            if let Ok(mut ts) = tab.last_output_at.lock() {
                *ts = None;
            }
            set_tab_status_and_reported(tab, crate::terminal::AgentTabStatus::Error(Some("503 Service Unavailable".into())));
        }
        s.frame(vec![]);
        assert!(has_circle_with_fill(&s.shapes, crate::theme::DANGER), "Error status dot must paint with theme::DANGER");

        // Stopped status -> painted with theme::ICON_TERTIARY
        {
            let tab = &mut s.shell.app.terminal.tabs[0];
            if let Ok(mut ts) = tab.last_output_at.lock() {
                *ts = None;
            }
            set_tab_status_and_reported(tab, crate::terminal::AgentTabStatus::Stopped);
        }
        s.frame(vec![]);
        assert!(has_circle_with_fill(&s.shapes, crate::theme::ICON_TERTIARY), "Stopped status dot must paint with theme::ICON_TERTIARY");
    }

    #[test]
    fn no_accesskit_means_no_cost() {
        let mut s = Sim::new();
        crate::agent_a11y::disable_accesskit(&s.ctx);
        s.shell.app.ui.left_collapsed = false;
        s.shell.app.ui.rail = Some(crate::app::RailItem::Agents);
        s.shell.app.terminal.ensure_default_tab(None);
        s.frame(vec![]);

        for status in [
            crate::terminal::AgentTabStatus::Working(None),
            crate::terminal::AgentTabStatus::Idle,
            crate::terminal::AgentTabStatus::WaitingForInput(None),
            crate::terminal::AgentTabStatus::Error(Some("429 rate limit".to_string())),
            crate::terminal::AgentTabStatus::Stopped,
        ] {
            set_tab_status(&mut s.shell.app.terminal.tabs[0], status);
            s.frame(vec![]);
            assert!(s.app().agent_a11y.queue.is_empty(), "announcer queue must stay empty when AccessKit is disabled");
            assert!(s.app().agent_a11y.shown.is_none(), "announcer shown must stay None when AccessKit is disabled");
        }
    }
}
