//! Unit and acceptance tests for terminal mouse wheel in main screen and alternate screen
//! across mouse encodings, multi-row steps, cursor-key fallback, step cap, and Shift+wheel.

use super::*;

fn new_parser() -> vt100::Parser {
    vt100::Parser::new(24, 80, TERM_SCROLLBACK_LINES)
}

fn spawn_quiet_tab_state() -> TerminalState {
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    let mut cmd = CommandBuilder::new("/bin/sleep");
    cmd.arg("5");
    let mut tab = TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Claude, 24, 80, None, None, Some(cmd)).expect("spawn quiet stub tab");
    tab.writer = Box::new(std::io::sink());
    st.tabs.push(tab);
    st.active_tab = 0;
    st.picker_open = false;
    st.side_panel_pty_open = true;
    st.chat_mode = false;
    st
}

fn cleanup_tab_state(mut st: TerminalState) {
    let dir = st.session_dir.clone();
    for mut tab in st.tabs.drain(..) {
        let _ = tab.child.kill();
    }
    st.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

fn warmup_viewport(st: &mut TerminalState, ctx: &egui::Context, viewport_rect: Rect) {
    let center = viewport_rect.center();
    let warmup = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
        events: vec![Event::PointerMoved(center)],
        ..Default::default()
    };
    let _ = ctx.run_ui(warmup, |ui| {
        render_vt100_viewport(st, ui, viewport_rect, None, "test-wheel-main");
    });
}

fn send_viewport_wheel(st: &mut TerminalState, ctx: &egui::Context, viewport_rect: Rect, dy: f32) -> isize {
    let center = viewport_rect.center();
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
        events: vec![
            Event::PointerMoved(center),
            Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, dy), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE },
        ],
        ..Default::default()
    };
    let mut total_rows = 0isize;
    let _ = ctx.run_ui(raw, |ui| {
        let smooth_y = ui.input(|i| i.smooth_scroll_delta.y);
        total_rows += wheel_delta_rows(smooth_y);
        render_vt100_viewport(st, ui, viewport_rect, None, "test-wheel-main");
    });
    total_rows
}

#[test]
fn wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting() {
    let mut st = spawn_quiet_tab_state();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx);
    let viewport_rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(660.0, 400.0));
    warmup_viewport(&mut st, &ctx, viewport_rect);

    {
        let tab = &mut st.tabs[0];
        let mut parser = tab.parser.lock().expect("lock parser");
        for idx in 1..=60 {
            parser.process(format!("HISTORY_LINE_{idx:02}\r\n").as_bytes());
        }
        // Enable SGR mouse reporting on the main screen (without entering `?1049h`).
        parser.process(b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h");
        assert!(!parser.screen().alternate_screen());
        assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::AnyMotion);
        assert_eq!(parser.screen().mouse_protocol_encoding(), vt100::MouseProtocolEncoding::Sgr);
        assert_eq!(parser.screen().scrollback(), 0);
        assert_eq!(wheel_bytes(parser.screen(), 3, (4, 9), false), WheelAction::Scrollback(3));
        assert_eq!(wheel_bytes(parser.screen(), 3, (4, 9), true), WheelAction::Scrollback(3));
        tab.restore_scrollback.store(7, std::sync::atomic::Ordering::Release);
        tab.written_bytes.clear();
    }

    // Wheel up through the real `render_vt100_viewport` handler.
    let up_rows = send_viewport_wheel(&mut st, &ctx, viewport_rect, 144.0);
    assert!(up_rows >= 3, "expected at least 3 rows scrolled up, got {up_rows}");
    assert_eq!(st.tabs[0].parser.lock().unwrap().screen().scrollback(), up_rows as usize);
    assert_eq!(st.tabs[0].restore_scrollback.load(std::sync::atomic::Ordering::Acquire), 0);
    assert!(st.tabs[0].written_bytes.is_empty());

    // Wheel down through the real `render_vt100_viewport` handler.
    let down_rows = send_viewport_wheel(&mut st, &ctx, viewport_rect, -192.0);
    assert!(down_rows < 0, "expected negative row delta on wheel down, got {down_rows}");
    let expected_sb = (up_rows + down_rows).max(0) as usize;
    assert!(expected_sb < up_rows as usize);
    assert_eq!(st.tabs[0].parser.lock().unwrap().screen().scrollback(), expected_sb);
    assert!(st.tabs[0].written_bytes.is_empty());

    cleanup_tab_state(st);
}

#[test]
fn wheel_in_alt_screen_with_sgr_mouse_sends_one_sgr_report_per_row() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::PressRelease);
    assert_eq!(parser.screen().mouse_protocol_encoding(), vt100::MouseProtocolEncoding::Sgr);

    // 3 rows up (`delta_y = 48.0`, `ROW_H = 16.0`) at 0-based `(r0=4, c0=9)` -> 1-based `col=10, row=5`.
    let up_rows = wheel_delta_rows(48.0);
    assert_eq!(up_rows, 3);
    assert_eq!(wheel_bytes(parser.screen(), up_rows, (4, 9), false), WheelAction::WritePty(b"\x1b[<64;10;5M\x1b[<64;10;5M\x1b[<64;10;5M".to_vec()));

    // 2 rows down (`delta_y = -20.0`, `ceil(20.0 / 16.0) = 2`) at 0-based `(r0=0, c0=0)` -> 1-based `col=1, row=1`.
    let down_rows = wheel_delta_rows(-20.0);
    assert_eq!(down_rows, -2);
    assert_eq!(wheel_bytes(parser.screen(), down_rows, (0, 0), false), WheelAction::WritePty(b"\x1b[<65;1;1M\x1b[<65;1;1M".to_vec()));
}

#[test]
fn wheel_in_alt_screen_with_default_mouse_encoding_sends_x10_reports() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::PressRelease);
    assert_eq!(parser.screen().mouse_protocol_encoding(), vt100::MouseProtocolEncoding::Default);

    // 0-based `(r0=4, c0=9)` -> 1-based `col=10`, `row=5` -> `32+64=96`, `32+10=42`, `32+5=37`.
    assert_eq!(wheel_bytes(parser.screen(), 2, (4, 9), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 42, 37, 0x1b, b'[', b'M', 96, 42, 37]));
    assert_eq!(wheel_bytes(parser.screen(), -1, (4, 9), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 97, 42, 37]));

    // Cells past 1-based 223 are clamped to 223 (`32 + 223 = 255 = 0xff`), matching xterm.
    assert_eq!(wheel_bytes(parser.screen(), 1, (249, 299), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 0xff, 0xff]));
}

#[test]
fn wheel_in_alt_screen_with_utf8_mouse_encoding_sends_utf8_coordinates() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1005h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::PressRelease);
    assert_eq!(parser.screen().mouse_protocol_encoding(), vt100::MouseProtocolEncoding::Utf8);

    // 0-based `(r0=4, c0=99)` -> 1-based `col=100` (`32 + 100 = 132 = U+0084` -> `[0xc2, 0x84]`),
    // `row=5` (`32 + 5 = 37 = 0x25`).
    assert_eq!(
        wheel_bytes(parser.screen(), 2, (4, 99), false),
        WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 0xc2, 0x84, 37, 0x1b, b'[', b'M', 96, 0xc2, 0x84, 37])
    );

    // 0-based `(r0=199, c0=249)` -> 1-based `col=250` (`32 + 250 = 282 = U+011A` -> `[0xc4, 0x9a]`),
    // `row=200` (`32 + 200 = 232 = U+00E8` -> `[0xc3, 0xa8]`).
    assert_eq!(wheel_bytes(parser.screen(), -1, (199, 249), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 97, 0xc4, 0x9a, 0xc3, 0xa8]));
}

#[test]
fn wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1005h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::PressRelease);
    assert_eq!(parser.screen().mouse_protocol_encoding(), vt100::MouseProtocolEncoding::Utf8);

    // Above 250 and below 2015: 0-based `(r0=4, c0=299)` -> 1-based `col=300` (`32 + 300 = 332 = U+014C` -> `[0xc5, 0x8c]`),
    // `row=5` (`32 + 5 = 37 = 0x25`).
    assert_eq!(wheel_bytes(parser.screen(), 1, (4, 299), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 0xc5, 0x8c, 37]));

    // At the 2015 boundary: 0-based `(r0=2014, c0=2014)` -> 1-based `col=2015`, `row=2015` (`32 + 2015 = 2047 = U+07FF` -> `[0xdf, 0xbf]`).
    assert_eq!(wheel_bytes(parser.screen(), 1, (2014, 2014), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 0xdf, 0xbf, 0xdf, 0xbf]));

    // Above 2015: 0-based `(r0=2400, c0=3000)` -> 1-based `col=3001`, `row=2401`, clamped to `2015` (`32 + 2015 = 2047 = U+07FF` -> `[0xdf, 0xbf]`).
    assert_eq!(wheel_bytes(parser.screen(), 1, (2400, 3000), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 96, 0xdf, 0xbf, 0xdf, 0xbf]));
    assert_eq!(wheel_bytes(parser.screen(), -1, (2400, 3000), false), WheelAction::WritePty(vec![0x1b, b'[', b'M', 97, 0xdf, 0xbf, 0xdf, 0xbf]));
}

#[test]
fn wheel_in_alt_screen_without_mouse_sends_cursor_keys() {
    let mut parser = new_parser();
    // Alternate screen with mouse reporting off (including when the program sends `?1007h` alternate scroll).
    parser.process(b"\x1b[?1049h\x1b[?1007h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::None);
    assert!(!parser.screen().application_cursor());

    assert_eq!(wheel_bytes(parser.screen(), 3, (4, 9), false), WheelAction::WritePty(b"\x1b[A\x1b[A\x1b[A".to_vec()));
    assert_eq!(wheel_bytes(parser.screen(), -2, (4, 9), false), WheelAction::WritePty(b"\x1b[B\x1b[B".to_vec()));
    // Shift+wheel in alternate screen without mouse reporting also sends cursor keys and never scrolls Studio scrollback.
    assert_eq!(wheel_bytes(parser.screen(), 2, (4, 9), true), WheelAction::WritePty(b"\x1b[A\x1b[A".to_vec()));
    assert_eq!(wheel_bytes(parser.screen(), -1, (4, 9), true), WheelAction::WritePty(b"\x1b[B".to_vec()));
}

#[test]
fn wheel_in_alt_screen_with_application_cursor_sends_ss3_cursor_keys() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::None);
    assert!(parser.screen().application_cursor());

    assert_eq!(wheel_bytes(parser.screen(), 3, (0, 0), false), WheelAction::WritePty(b"\x1bOA\x1bOA\x1bOA".to_vec()));
    assert_eq!(wheel_bytes(parser.screen(), -2, (0, 0), false), WheelAction::WritePty(b"\x1bOB\x1bOB".to_vec()));
    assert_eq!(wheel_bytes(parser.screen(), 1, (0, 0), true), WheelAction::WritePty(b"\x1bOA".to_vec()));
}

#[test]
fn fast_wheel_steps_are_capped_per_frame() {
    assert_eq!(MAX_WHEEL_STEPS_PER_FRAME, 10);
    let fast_up = wheel_delta_rows(400.0);
    let fast_down = wheel_delta_rows(-400.0);
    assert_eq!(fast_up, 25);
    assert_eq!(fast_down, -25);

    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h");
    assert_eq!(wheel_bytes(parser.screen(), fast_up, (0, 0), false), WheelAction::WritePty(b"\x1b[<64;1;1M".repeat(MAX_WHEEL_STEPS_PER_FRAME)));
    assert_eq!(wheel_bytes(parser.screen(), fast_down, (0, 0), false), WheelAction::WritePty(b"\x1b[<65;1;1M".repeat(MAX_WHEEL_STEPS_PER_FRAME)));

    parser.process(b"\x1b[?1000l");
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::None);
    assert_eq!(wheel_bytes(parser.screen(), fast_up, (0, 0), false), WheelAction::WritePty(b"\x1b[A".repeat(MAX_WHEEL_STEPS_PER_FRAME)));
    assert_eq!(wheel_bytes(parser.screen(), fast_down, (0, 0), false), WheelAction::WritePty(b"\x1b[B".repeat(MAX_WHEEL_STEPS_PER_FRAME)));
}

#[test]
fn shift_wheel_in_alt_screen_with_mouse_writes_nothing() {
    let mut parser = new_parser();
    parser.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::PressRelease);

    assert_eq!(wheel_bytes(parser.screen(), 3, (4, 9), true), WheelAction::Noop);
    assert_eq!(wheel_bytes(parser.screen(), -3, (4, 9), true), WheelAction::Noop);
}
