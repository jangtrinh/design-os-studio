//! Regression and acceptance tests for `#1171`: Antigravity / Ink-style `ESC[3J` scrollback clear on `SIGWINCH`,
//! `120 ms` debounced PTY resize during UI zoom, and interactive 5,000-line terminal scrollbar with a `24 px` target.

use super::*;
use portable_pty::CommandBuilder;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Python-based fake CLI executed inside the real OS PTY of `TerminalTab`.
///
/// Modes:
/// - `"a"`: Ink / Antigravity pattern. Prints 30 history lines (`HISTORY_LINE_01`..`HISTORY_LINE_30`),
///   and on every `SIGWINCH` sends `ESC[2J ESC[3J ESC[H` and reprints all 30 history lines.
/// - `"b"`: Visible-only redraw pattern. Prints 30 history lines initially so `HISTORY_LINE_01`..`HISTORY_LINE_11`
///   sit in scrollback, and on `SIGWINCH` sends `ESC[2J ESC[H` (no `ESC[3J`) and redraws only the visible screen.
/// - `"table"`: Box-table pattern (`AC-03`). Draws history lines + a box-drawing table sized to the current terminal
///   width (`cols`), and on `SIGWINCH` sends `ESC[2J ESC[3J ESC[H` and redraws the box table for the new width.
/// - `"stream"`: Streaming pattern. Prints `STREAM_LINE_01`..`STREAM_LINE_15`, waits for `SIGWINCH`, sends
///   `ESC[2J ESC[3J ESC[H`, reprints `STREAM_LINE_01`..`STREAM_LINE_15`, and streams `STREAM_LINE_16`..`STREAM_LINE_30`.
const FAKE_CLI_PY: &str = r#"
import fcntl, os, signal, struct, sys, termios, time

mode = sys.argv[1] if len(sys.argv) > 1 else "a"
seq = 0
stream_after_winch = False

def winsz():
    try:
        s = fcntl.ioctl(1, termios.TIOCGWINSZ, b"\x00" * 8)
        r, c, _, _ = struct.unpack("HHHH", s)
        return max(int(r), 1), max(int(c), 1)
    except Exception:
        return (20, 80)

def write_out(text):
    os.write(1, text.encode("utf-8"))

def render_mode_a(is_winch):
    global seq
    r, c = winsz()
    prefix = "\x1b[2J\x1b[3J\x1b[H" if is_winch else ""
    lines = [f"HISTORY_LINE_{i:02d}" for i in range(1, 31)]
    lines.append(f"[winch_seq={seq} rows={r} cols={c}]")
    write_out(prefix + "\r\n".join(lines))

def render_mode_b(is_winch):
    global seq
    r, c = winsz()
    if not is_winch:
        lines = [f"HISTORY_LINE_{i:02d}" for i in range(1, 31)]
        lines.append(f"[winch_seq={seq} rows={r} cols={c}]")
        write_out("\r\n".join(lines))
    else:
        vis_count = max(r - 1, 1)
        start_idx = max(1, 31 - vis_count)
        lines = [f"HISTORY_LINE_{i:02d}" for i in range(start_idx, 31)]
        lines.append(f"[winch_seq={seq} rows={r} cols={c}]")
        write_out("\x1b[2J\x1b[H" + "\r\n".join(lines))

def render_table(is_winch):
    global seq
    r, c = winsz()
    w = max(c, 20)
    inner = w - 2
    prefix = "\x1b[2J\x1b[3J\x1b[H" if is_winch else ""
    top = "┌" + ("─" * inner) + "┐"
    body_txt = f" TABLE_WIDTH={w} SEQ={seq} "
    pad = max(inner - len(body_txt), 0)
    mid = "│" + body_txt + (" " * pad) + "│"
    bot = "└" + ("─" * inner) + "┘"
    lines = [top, mid, bot]
    lines.extend([f"TABLE_HISTORY_{i:02d}" for i in range(1, 19)])
    lines.append(f"[winch_seq={seq} rows={r} cols={c}]")
    write_out(prefix + "\r\n".join(lines))

def render_stream_initial():
    r, c = winsz()
    lines = [f"STREAM_LINE_{i:02d}" for i in range(1, 16)]
    lines.append(f"[stream_phase=1 winch_seq={seq} rows={r} cols={c}]")
    write_out("\r\n".join(lines) + "\r\n")

def render_stream_winch():
    r, c = winsz()
    lines = [f"STREAM_LINE_{i:02d}" for i in range(1, 16)]
    write_out("\x1b[2J\x1b[3J\x1b[H" + "\r\n".join(lines) + "\r\n")
    for i in range(16, 31):
        write_out(f"STREAM_LINE_{i:02d}\r\n")
    write_out(f"[stream_done winch_seq={seq} rows={r} cols={c}]")

def on_winch(signum, frame):
    global seq, stream_after_winch
    seq += 1
    if mode == "a":
        render_mode_a(True)
    elif mode == "b":
        render_mode_b(True)
    elif mode == "table":
        render_table(True)
    elif mode == "stream":
        stream_after_winch = True

signal.signal(signal.SIGWINCH, on_winch)

if mode == "a":
    render_mode_a(False)
elif mode == "b":
    render_mode_b(False)
elif mode == "table":
    render_table(False)
elif mode == "stream":
    render_stream_initial()

while True:
    if mode == "stream" and stream_after_winch:
        stream_after_winch = False
        render_stream_winch()
    time.sleep(0.02)
"#;

pub(super) fn spawn_fake_cli_tab(mode: &str, rows: u16, cols: u16) -> (TerminalState, usize) {
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    let mut cmd = CommandBuilder::new("python3");
    cmd.arg("-u");
    cmd.arg("-c");
    cmd.arg(FAKE_CLI_PY);
    cmd.arg(mode);
    let tab = TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Antigravity, rows, cols, None, None, Some(cmd)).expect("spawn fake CLI in PTY");
    st.tabs.push(tab);
    st.active_tab = 0;
    st.picker_open = false;
    st.side_panel_pty_open = true;
    st.chat_mode = false;
    (st, 0)
}

pub(super) fn cleanup_state(mut st: TerminalState) {
    let dir = st.session_dir.clone();
    crate::agent_approval_channel::remove_channel(&dir);
    st.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Reads the real scrollback length and every line in scrollback + visible screen in top-to-bottom order.
pub(super) fn all_terminal_lines(parser: &mut vt100::Parser) -> (usize, Vec<String>) {
    let (rows, cols) = parser.screen().size();
    let prev_offset = parser.screen().scrollback();
    parser.set_scrollback(usize::MAX);
    let sb_len = parser.screen().scrollback();

    let mut out = Vec::with_capacity(sb_len + usize::from(rows));
    for offset in (1..=sb_len).rev() {
        parser.set_scrollback(offset);
        let line = with_scrollback_safe_screen(parser, |screen| row_text(screen, 0, cols));
        out.push(line);
    }
    parser.set_scrollback(0);
    for r in 0..rows {
        out.push(row_text(parser.screen(), r, cols));
    }
    parser.set_scrollback(prev_offset);
    (sb_len, out)
}

pub(super) fn row_text(screen: &vt100::Screen, row: u16, cols: u16) -> String {
    let mut s = String::new();
    for c in 0..cols {
        if let Some(cell) = screen.cell(row, c) {
            if cell.is_wide_continuation() {
                continue;
            }
            let txt = cell.contents();
            if txt.is_empty() {
                s.push(' ');
            } else {
                s.push_str(&txt);
            }
        }
    }
    s.trim_end().to_string()
}

pub(super) fn wait_for_marker(st: &TerminalState, tab_idx: usize, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if let Ok(mut p) = st.tabs[tab_idx].parser.lock() {
            let (_, lines) = all_terminal_lines(&mut p);
            if lines.iter().any(|l| l.contains(needle)) {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let dump = st.tabs[tab_idx].parser.lock().map(|mut p| all_terminal_lines(&mut p).1.join("\n")).unwrap_or_default();
    panic!("timed out waiting for marker {needle:?} in terminal output:\n{dump}");
}

#[test]
fn ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom() {
    // 1. Mode (b): redraws only the visible screen with no ESC[3J; scrollback must stay untouched across 5 resizes.
    {
        let (mut st_b, idx) = spawn_fake_cli_tab("b", 20, 80);
        wait_for_marker(&st_b, idx, "[winch_seq=0 rows=20 cols=80]");
        let initial_sb_b = {
            let mut p = st_b.tabs[idx].parser.lock().unwrap();
            let (sb_len, lines) = all_terminal_lines(&mut p);
            assert_eq!(sb_len, 11, "mode (b) initial scrollback must hold 11 scrolled lines");
            lines[..sb_len].to_vec()
        };
        for (step, cols) in [82u16, 84, 86, 88, 90].into_iter().enumerate() {
            let seq = step + 1;
            st_b.tabs[idx].resize(20, cols);
            wait_for_marker(&st_b, idx, &format!("[winch_seq={seq} rows=20 cols={cols}]"));
        }
        {
            let mut p = st_b.tabs[idx].parser.lock().unwrap();
            let (sb_len, lines) = all_terminal_lines(&mut p);
            assert_eq!(sb_len, 11, "mode (b) scrollback length must remain untouched after 5 resizes");
            assert_eq!(&lines[..sb_len], &initial_sb_b[..], "mode (b) scrollback lines must remain identical");
        }
        cleanup_state(st_b);
    }

    // 2. Step 0 & AC-01 with fake CLI mode (a): on SIGWINCH sends ESC[2J ESC[3J ESC[H and reprints history.
    let (mut st_a, idx) = spawn_fake_cli_tab("a", 20, 80);
    wait_for_marker(&st_a, idx, "[winch_seq=0 rows=20 cols=80]");
    for (step, cols) in [82u16, 84, 86, 88, 90].into_iter().enumerate() {
        let seq = step + 1;
        st_a.tabs[idx].resize(20, cols);
        wait_for_marker(&st_a, idx, &format!("[winch_seq={seq} rows=20 cols={cols}]"));
    }

    let (sb_len, lines) = {
        let mut p = st_a.tabs[idx].parser.lock().unwrap();
        all_terminal_lines(&mut p)
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for i in 1..=30 {
        let key = format!("HISTORY_LINE_{i:02}");
        let c = lines.iter().filter(|l| l.trim() == key).count();
        counts.insert(key, c);
    }
    eprintln!("STEP0_MODE_A_COUNTS: scrollback_len={sb_len} counts={counts:?}");
    cleanup_state(st_a);

    assert_eq!(sb_len, 11, "AC-01 mode (a): scrollback must hold 11 lines after 5 resizes, got {sb_len} (counts={counts:?})");
    for (line, count) in &counts {
        assert_eq!(*count, 1, "AC-01 mode (a): {line} must appear exactly once after 5 resize steps, got {count}");
    }
}

pub(super) fn recorded_rect(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string()))))
}

pub(super) fn run_viewport_frame(st: &mut TerminalState, ctx: &egui::Context, viewport_rect: Rect, time: Option<f64>, events: Vec<egui::Event>) {
    for tag in ["term-scrollbar-track", "term-scrollbar-thumb"] {
        ctx.data_mut(|d| d.remove::<Rect>(Id::new(("test-rect", tag.to_string()))));
    }
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), time, events, ..Default::default() };
    let _ = ctx.run_ui(raw, |ui| {
        render_vt100_viewport(st, ui, viewport_rect, None, "test-pty-vp");
    });
}

#[test]
fn ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size() {
    let (mut st, idx) = spawn_fake_cli_tab("b", 24, 80);
    wait_for_marker(&st, idx, "[winch_seq=0 rows=24 cols=80]");

    // Fill the scrollback to its full 5,000-line capacity (5,024 total lines on a 24-row screen).
    {
        let mut p = st.tabs[idx].parser.lock().unwrap();
        let mut bulk = String::from("\x1b[2J\x1b[H");
        for i in 1..=5024 {
            if i > 1 {
                bulk.push_str("\r\n");
            }
            bulk.push_str(&format!("SB_ROW_{i:04}"));
        }
        p.process(bulk.as_bytes());
        let (sb_len, _) = all_terminal_lines(&mut p);
        assert_eq!(sb_len, 5000, "scrollback must hold the full 5,000 lines");
        p.set_scrollback(0);
    }

    let ctx = super::gemini_test_support::panel_ctx();
    let normal_vp = Rect::from_min_size(Pos2::new(48.0, 40.0), Vec2::new(560.0, 400.0));
    run_viewport_frame(&mut st, &ctx, normal_vp, Some(1.0), Vec::new());

    let track =
        recorded_rect(&ctx, "term-scrollbar-track").unwrap_or_else(|| panic!("AC-02: terminal scrollbar track must be recorded when scrollback_len > 0"));
    let thumb_bottom =
        recorded_rect(&ctx, "term-scrollbar-thumb").unwrap_or_else(|| panic!("AC-02: terminal scrollbar thumb must be recorded when scrollback_len > 0"));
    assert!(track.width() >= crate::theme::TARGET_MIN - 0.5, "AC-02 / P3: scrollbar track pointer target width must be >= 24px, got {}", track.width());
    assert!(
        (thumb_bottom.bottom() - track.bottom()).abs() <= 1.5,
        "AC-02: at scrollback_offset=0 the thumb must sit at the bottom of the track (thumb={thumb_bottom:?}, track={track:?})"
    );

    // 1. Drag from the bottom thumb to the top of the track -> scrollback_offset reaches 5,000.
    let start_pos = thumb_bottom.center();
    let top_pos = Pos2::new(track.center().x, track.top() + 2.0);
    run_viewport_frame(
        &mut st,
        &ctx,
        normal_vp,
        Some(1.02),
        vec![
            egui::Event::PointerMoved(start_pos),
            egui::Event::PointerButton { pos: start_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerMoved(top_pos),
            egui::Event::PointerButton { pos: top_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    let sb_top = st.tabs[idx].parser.lock().unwrap().screen().scrollback();
    let thumb_top = recorded_rect(&ctx, "term-scrollbar-thumb").expect("thumb at top");
    assert_eq!(sb_top, 5000, "AC-02: dragging to the top of the track must reach scrollback_offset=5000");
    assert!(
        (thumb_top.top() - track.top()).abs() <= 1.5,
        "AC-02: at scrollback_offset=5000 the thumb must sit at the top of the track (thumb={thumb_top:?}, track={track:?})"
    );
    assert!(st.tabs[idx].selection.is_none(), "AC-02: dragging the scrollbar must not select terminal text");

    // 2. Click mid-track -> scrollback_offset lands near 2,500 (proving real 5,000-line mapping, not / 500).
    let mid_pos = track.center();
    run_viewport_frame(
        &mut st,
        &ctx,
        normal_vp,
        Some(1.04),
        vec![
            egui::Event::PointerMoved(mid_pos),
            egui::Event::PointerButton { pos: mid_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos: mid_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    let sb_mid = st.tabs[idx].parser.lock().unwrap().screen().scrollback();
    let thumb_mid = recorded_rect(&ctx, "term-scrollbar-thumb").expect("thumb at mid");
    assert!((2400..=2600).contains(&sb_mid), "AC-02: clicking mid-track on a 5,000-line scrollback must land near 2,500, got {sb_mid}");
    assert!(
        thumb_mid.top() > thumb_top.top() + 50.0 && thumb_mid.bottom() < thumb_bottom.bottom() - 50.0,
        "AC-02: mid-track thumb must sit mid-track between top and bottom (mid={thumb_mid:?}, top={thumb_top:?}, bottom={thumb_bottom:?})"
    );

    // 3. Verify offset=500 vs offset=5000 thumb separation (proving / 500 saturation is gone).
    st.tabs[idx].parser.lock().unwrap().set_scrollback(500);
    run_viewport_frame(&mut st, &ctx, normal_vp, Some(1.05), Vec::new());
    let thumb_500 = recorded_rect(&ctx, "term-scrollbar-thumb").expect("thumb at 500");
    assert!(
        thumb_500.top() > thumb_mid.top() + 50.0,
        "AC-02: offset 500 of 5,000 must sit near 10% from bottom, well below mid-track (thumb_500={thumb_500:?}, thumb_mid={thumb_mid:?})"
    );

    // 4. Drag/click to the bottom of the track -> scrollback_offset reaches 0.
    let bot_pos = Pos2::new(track.center().x, track.bottom() - 2.0);
    run_viewport_frame(
        &mut st,
        &ctx,
        normal_vp,
        Some(1.06),
        vec![
            egui::Event::PointerMoved(bot_pos),
            egui::Event::PointerButton { pos: bot_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos: bot_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    assert_eq!(st.tabs[idx].parser.lock().unwrap().screen().scrollback(), 0, "AC-02: clicking bottom of track must reach offset 0");

    // 5. Repeat at the smallest viewport size (rows=4, cols=16 -> 16*7.2 + 24 = 139.2w, 4*16 + 16 = 80h).
    let min_vp = Rect::from_min_size(Pos2::new(48.0, 40.0), Vec2::new(139.0, 80.0));
    run_viewport_frame(&mut st, &ctx, min_vp, Some(1.10), Vec::new());
    let min_track = recorded_rect(&ctx, "term-scrollbar-track").expect("min size track");
    assert!(min_track.width() >= crate::theme::TARGET_MIN - 0.5, "min size track width >= 24px");

    let min_top = Pos2::new(min_track.center().x, min_track.top() + 1.0);
    run_viewport_frame(
        &mut st,
        &ctx,
        min_vp,
        Some(1.12),
        vec![
            egui::Event::PointerMoved(min_top),
            egui::Event::PointerButton { pos: min_top, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos: min_top, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    assert_eq!(st.tabs[idx].parser.lock().unwrap().screen().scrollback(), 5000, "AC-02 min size: reach top (5000)");

    let min_mid = min_track.center();
    run_viewport_frame(
        &mut st,
        &ctx,
        min_vp,
        Some(1.14),
        vec![
            egui::Event::PointerMoved(min_mid),
            egui::Event::PointerButton { pos: min_mid, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos: min_mid, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    let min_sb_mid = st.tabs[idx].parser.lock().unwrap().screen().scrollback();
    assert!((2300..=2700).contains(&min_sb_mid), "AC-02 min size: click mid-track lands near 2500, got {min_sb_mid}");

    let min_bot = Pos2::new(min_track.center().x, min_track.bottom() - 1.0);
    run_viewport_frame(
        &mut st,
        &ctx,
        min_vp,
        Some(1.16),
        vec![
            egui::Event::PointerMoved(min_bot),
            egui::Event::PointerButton { pos: min_bot, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos: min_bot, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
    );
    assert_eq!(st.tabs[idx].parser.lock().unwrap().screen().scrollback(), 0, "AC-02 min size: reach bottom (0)");

    cleanup_state(st);
}

#[test]
fn ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy() {
    let (mut st, idx) = spawn_fake_cli_tab("table", 20, 100);
    wait_for_marker(&st, idx, "[winch_seq=0 rows=20 cols=100]");

    // Resize to 70 columns (narrow zoom), then back to 100 columns.
    st.tabs[idx].resize(20, 70);
    wait_for_marker(&st, idx, "[winch_seq=1 rows=20 cols=70]");

    st.tabs[idx].resize(20, 100);
    wait_for_marker(&st, idx, "[winch_seq=2 rows=20 cols=100]");

    let (sb_len, lines) = {
        let mut p = st.tabs[idx].parser.lock().unwrap();
        all_terminal_lines(&mut p)
    };
    cleanup_state(st);

    let top_100 = format!("┌{}┐", "─".repeat(98));
    let bot_100 = format!("└{}┘", "─".repeat(98));
    let top_70 = format!("┌{}┐", "─".repeat(68));
    let has_stale_70 = lines.iter().any(|l| l.contains("TABLE_WIDTH=70") || l.trim() == top_70);
    let count_100 = lines.iter().filter(|l| l.contains("TABLE_WIDTH=100")).count();
    let count_top_100 = lines.iter().filter(|l| l.trim() == top_100).count();
    let count_bot_100 = lines.iter().filter(|l| l.trim() == bot_100).count();

    assert!(!has_stale_70, "AC-03: no stale narrow 70-column table copy may remain after resizing back to 100 columns (sb_len={sb_len}, lines={lines:?})");
    assert_eq!(count_100, 1, "AC-03: 100-column table body must appear exactly once, got {count_100}");
    assert_eq!(count_top_100, 1, "AC-03: 100-column top border must appear exactly once, got {count_top_100}");
    assert_eq!(count_bot_100, 1, "AC-03: 100-column bottom border must appear exactly once, got {count_bot_100}");
}

#[test]
fn ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab() {
    let (mut st, idx) = spawn_fake_cli_tab("a", 28, 100);
    st.tabs[idx].preset = AgentCliPreset::Claude;
    st.tabs[idx].spawn_preset = AgentCliPreset::Claude;
    wait_for_marker(&st, idx, "[winch_seq=0 rows=28 cols=100]");
    assert_eq!(st.tabs[idx].resize_count, 0);

    let ctx = super::gemini_test_support::panel_ctx();

    // Simulate holding the zoom key across 5 rapid steps (20 ms apart: 1.00, 1.02, 1.04, 1.06, 1.08 s).
    let steps = [
        (1.00, Vec2::new(660.0, 420.0)),
        (1.02, Vec2::new(610.0, 390.0)),
        (1.04, Vec2::new(560.0, 360.0)),
        (1.06, Vec2::new(510.0, 330.0)),
        (1.08, Vec2::new(460.0, 300.0)),
    ];
    for (t, sz) in steps {
        let vp = Rect::from_min_size(Pos2::new(48.0, 40.0), sz);
        run_viewport_frame(&mut st, &ctx, vp, Some(t), Vec::new());
    }

    assert_eq!(
        st.tabs[idx].resize_count, 0,
        "AC-05: no PTY resize may be sent while zoom steps are still changing within the 120 ms debounce window, got {}",
        st.tabs[idx].resize_count
    );

    // At t = 1.15 s (70 ms after the last zoom step at 1.08 s, still < 120 ms), still no resize.
    let final_vp = Rect::from_min_size(Pos2::new(48.0, 40.0), Vec2::new(460.0, 300.0));
    run_viewport_frame(&mut st, &ctx, final_vp, Some(1.15), Vec::new());
    assert_eq!(st.tabs[idx].resize_count, 0, "AC-05: 70 ms after last zoom step (< 120 ms), resize_count must still be 0");

    // At t = 1.21 s (130 ms after 1.08 s, >= 120 ms), exactly one PTY resize is sent!
    run_viewport_frame(&mut st, &ctx, final_vp, Some(1.21), Vec::new());
    assert_eq!(st.tabs[idx].resize_count, 1, "AC-05: once the size settles for >= 120 ms, exactly 1 PTY resize must fire");
    assert_eq!((st.tabs[idx].rows, st.tabs[idx].cols), st.viewport_size, "AC-05: tab size must match settled viewport_size");

    // Subsequent settled frames do not re-trigger resize.
    run_viewport_frame(&mut st, &ctx, final_vp, Some(1.30), Vec::new());
    assert_eq!(st.tabs[idx].resize_count, 1, "AC-05: settled frames must not emit extra resizes");

    // AC-04 regression check: Claude Code tab key input and wheel scrollback still work as before.
    run_viewport_frame(&mut st, &ctx, final_vp, Some(1.32), vec![egui::Event::Text("ok".to_string())]);
    assert!(st.tabs[idx].written_bytes.ends_with(b"ok"), "AC-04: Claude tab keyboard input must still reach PTY stdin");

    cleanup_state(st);
}

#[test]
fn edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn() {
    // Edge case 1: Zoom while streaming output.
    {
        let (mut st, idx) = spawn_fake_cli_tab("stream", 10, 80);
        wait_for_marker(&st, idx, "[stream_phase=1 winch_seq=0 rows=10 cols=80]");
        st.tabs[idx].resize(10, 86);
        wait_for_marker(&st, idx, "[stream_done winch_seq=1 rows=10 cols=86]");

        let (_, lines) = {
            let mut p = st.tabs[idx].parser.lock().unwrap();
            all_terminal_lines(&mut p)
        };
        cleanup_state(st);

        for i in 1..=30 {
            let key = format!("STREAM_LINE_{i:02}");
            let c = lines.iter().filter(|l| l.trim() == key).count();
            assert_eq!(c, 1, "streaming edge case: {key} must appear exactly once after mid-stream resize, got {c} (lines={lines:?})");
        }
    }

    // Edge case 2: Zoom while scrolled into history (keep reading position; do not jump to bottom).
    {
        let (mut st, idx) = spawn_fake_cli_tab("a", 20, 80);
        wait_for_marker(&st, idx, "[winch_seq=0 rows=20 cols=80]");
        st.tabs[idx].parser.lock().unwrap().set_scrollback(8);
        assert_eq!(st.tabs[idx].parser.lock().unwrap().screen().scrollback(), 8);

        st.tabs[idx].resize(20, 86);
        wait_for_marker(&st, idx, "[winch_seq=1 rows=20 cols=86]");

        let sb_after = st.tabs[idx].parser.lock().unwrap().screen().scrollback();
        cleanup_state(st);
        assert_eq!(sb_after, 8, "scrolled-history edge case: reading position (scrollback_offset=8) must be preserved across zoom reprint, got {sb_after}");
    }

    // Edge case 3: Alternate screen (?1049h) with mouse reporting (?1000h?1006h).
    {
        let (mut st, idx) = spawn_fake_cli_tab("b", 20, 80);
        wait_for_marker(&st, idx, "[winch_seq=0 rows=20 cols=80]");
        {
            let mut p = st.tabs[idx].parser.lock().unwrap();
            let (sb_before, _) = all_terminal_lines(&mut p);
            assert_eq!(sb_before, 11);
            p.process(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[HALT_SCREEN_ACTIVE");
            assert!(p.screen().alternate_screen());
            assert_ne!(p.screen().mouse_protocol_mode(), vt100::MouseProtocolMode::None);
        }

        let ctx = super::gemini_test_support::panel_ctx();
        let vp = Rect::from_min_size(Pos2::new(48.0, 40.0), Vec2::new(560.0, 400.0));
        let hover_pos = vp.center();
        run_viewport_frame(&mut st, &ctx, vp, Some(1.9), vec![egui::Event::PointerMoved(hover_pos)]);
        st.tabs[idx].written_bytes.clear();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
            time: Some(2.0),
            events: vec![
                egui::Event::PointerMoved(hover_pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, 24.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            render_vt100_viewport(&mut st, ui, vp, None, "test-pty-vp");
        });
        let written = String::from_utf8_lossy(&st.tabs[idx].written_bytes).into_owned();
        assert!(written.contains("\x1b[<64;"), "alternate-screen mouse reporting: wheel up must emit SGR mouse sequence \\x1b[<64;..., got {written:?}");
        cleanup_state(st);
    }

    // Edge case 4 & 5: Minimum clamp (rows=4, cols=16) and a tab opened after zooming starts with the zoomed size.
    {
        let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
        st.picker_open = false;
        let ctx = super::gemini_test_support::panel_ctx();

        // Render empty state at smallest viewport rect (rows=4, cols=16).
        let tiny_vp = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(80.0, 40.0));
        run_viewport_frame(&mut st, &ctx, tiny_vp, Some(1.0), Vec::new());
        assert_eq!(st.viewport_size, (4, 16), "smallest-size edge case: viewport_size must clamp to (rows=4, cols=16), got {:?}", st.viewport_size);

        let min_tab_idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell tab at min size");
        assert_eq!((st.tabs[min_tab_idx].rows, st.tabs[min_tab_idx].cols), (4, 16), "tab opened at smallest size must start with (rows=4, cols=16)");

        // Now zoom viewport to a custom size (rows=18, cols=64 -> width = 64*7.8 + 24 = 523.2, height = 18*16 + 16 = 304)
        let zoomed_vp = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(524.0, 304.0));
        run_viewport_frame(&mut st, &ctx, zoomed_vp, Some(2.0), Vec::new());
        assert_eq!(st.viewport_size, (18, 64), "zoomed viewport_size must be (18, 64)");

        let post_zoom_idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell tab after zooming");
        assert_eq!(
            (st.tabs[post_zoom_idx].rows, st.tabs[post_zoom_idx].cols),
            (18, 64),
            "post-zoom spawn edge case: a tab opened after zooming must start at (18, 64) from the start"
        );
        assert_eq!(st.tabs[post_zoom_idx].resize_count, 0, "newly spawned post-zoom tab must not need a post-spawn resize");

        cleanup_state(st);
    }
}

#[test]
fn agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled() {
    // 1. `agents-approval-open-tab` is a #1126 layout-gate preset and must have no scrollback.
    let mut st_open = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    st_open.open_approval_fixture("agents-approval-open-tab", None, crate::agent_permissions::AgentPermissionMode::Ask);
    let (open_sb_len, open_sb_offset) = {
        let tab = st_open.active_tab().expect("active tab for agents-approval-open-tab");
        let mut p = tab.parser.lock().unwrap();
        (parser_scrollback_len(&mut p), p.screen().scrollback())
    };
    cleanup_state(st_open);
    assert_eq!(open_sb_len, 0, "agents-approval-open-tab must have no scrollback lines, got {open_sb_len}");
    assert_eq!(open_sb_offset, 0, "agents-approval-open-tab must not be scrolled into history, got {open_sb_offset}");

    // 2. `--panel agents-scrollback` is registered in CLI PANELS and has 120 history lines scrolled into history.
    assert!(crate::cli::PANELS.contains(&"agents-scrollback"), "CLI PANELS must register agents-scrollback");
    let parsed = crate::cli::parse(["--panel".to_string(), "agents-scrollback".to_string()]).expect("parse --panel agents-scrollback");
    assert_eq!(parsed.panel.as_deref(), Some("agents-scrollback"));

    let mut st_sb = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    st_sb.open_approval_fixture("agents-scrollback", None, crate::agent_permissions::AgentPermissionMode::Ask);
    let (sb_len, sb_offset, lines) = {
        let tab = st_sb.active_tab().expect("active tab for agents-scrollback");
        let mut p = tab.parser.lock().unwrap();
        let offset = p.screen().scrollback();
        let (len, all_lines) = all_terminal_lines(&mut p);
        (len, offset, all_lines)
    };
    cleanup_state(st_sb);

    let history_matches = (1..=120)
        .filter(|idx| {
            let expected = format!("[{idx:03}] Inspecting layer tokens and scrollback");
            lines.iter().any(|l| l.trim() == expected)
        })
        .count();
    assert_eq!(history_matches, 120, "agents-scrollback must contain all 120 history lines, got {history_matches}");
    assert!(sb_len > 0, "agents-scrollback must have scrollback history (sb_len={sb_len})");
    assert_eq!(sb_offset, 45, "agents-scrollback must have its view scrolled into history at offset 45, got {sb_offset}");
}

#[test]
fn deep_scrollback_offset_3000_of_5000_paints_expected_history_rows() {
    let (mut st, idx) = spawn_fake_cli_tab("b", 24, 80);
    wait_for_marker(&st, idx, "[winch_seq=0 rows=24 cols=80]");

    // Populate 5,024 lines on a 24-row screen so 5,000 lines sit in scrollback (SB_ROW_0001..SB_ROW_5000)
    // and 24 lines sit on the active grid (SB_ROW_5001..SB_ROW_5024), then scroll back to offset 3,000.
    {
        let mut p = st.tabs[idx].parser.lock().unwrap();
        let mut bulk = String::from("\x1b[2J\x1b[H");
        for i in 1..=5024 {
            if i > 1 {
                bulk.push_str("\r\n");
            }
            bulk.push_str(&format!("SB_ROW_{i:04}"));
        }
        p.process(bulk.as_bytes());
        assert_eq!(parser_scrollback_len(&mut p), 5000, "scrollback buffer must hold 5,000 lines");
        p.set_scrollback(3000);
        assert_eq!(p.screen().scrollback(), 3000);
    }

    // At offset 3,000 of 5,000 scrollback lines on a 24-row viewport, `skip(5000 - 3000) = skip(2000)`
    // yields `SB_ROW_2001`..`SB_ROW_2024`.
    let expected_rows: Vec<String> = (2001..=2024).map(|i| format!("SB_ROW_{i:04}")).collect();

    let (size_in_callback, callback_rows, copied_rows) = {
        let mut p = st.tabs[idx].parser.lock().unwrap();
        with_scrollback_safe_screen(&mut p, |screen| {
            let rows_vec: Vec<String> = (0..24).map(|r| row_text(screen, r, 80)).collect();
            let copied = extract_selected_terminal_text(screen, ((0, 0), (23, 79)), 24, 80);
            let copied_vec: Vec<String> = copied.lines().map(|l| l.trim().to_string()).collect();
            (screen.size(), rows_vec, copied_vec)
        })
    };
    assert_eq!(callback_rows, expected_rows, "with_scrollback_safe_screen rows at offset 3,000 of 5,000 must equal SB_ROW_2001..SB_ROW_2024");
    assert_eq!(copied_rows, expected_rows, "extract_selected_terminal_text at offset 3,000 of 5,000 must equal SB_ROW_2001..SB_ROW_2024");

    #[cfg(not(debug_assertions))]
    assert_eq!(
        size_in_callback,
        (24, 80),
        "release build must not call parser.set_size(offset, cols) inside with_scrollback_safe_screen, got {size_in_callback:?}"
    );
    #[cfg(debug_assertions)]
    assert_eq!(size_in_callback.1, 80);

    // Paint a 24x80 viewport frame (`80 * 7.8 + 24 = 648`, `24 * 16 + 16 = 400`) and verify the 24 painted text rows.
    let ctx = super::gemini_test_support::panel_ctx();
    let vp_24x80 = Rect::from_min_size(Pos2::new(48.0, 40.0), Vec2::new(648.0, 400.0));
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), time: Some(1.0), ..Default::default() };
    let out = ctx.run_ui(raw, |ui| {
        render_vt100_viewport(&mut st, ui, vp_24x80, None, "test-pty-vp");
    });
    let resolved_vp_size = st.viewport_size;
    cleanup_state(st);

    assert_eq!(resolved_vp_size, (24, 80), "viewport_rect must resolve to (rows=24, cols=80)");
    let (mut texts, _) = crate::lint::scan_shapes(&out.shapes);
    texts.retain(|t| t.text.starts_with("SB_ROW_"));
    texts.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));
    let painted_rows: Vec<String> = texts.into_iter().map(|t| t.text.trim().to_string()).collect();
    assert_eq!(painted_rows, expected_rows, "painted rows at offset 3,000 of 5,000 (24 rows) must equal SB_ROW_2001..SB_ROW_2024");

    // Verify `with_scrollback_safe_screen` only widens `parser.set_size` under `cfg(debug_assertions)`.
    let term_src = include_str!("../terminal.rs");
    let fn_start = term_src.find("fn with_scrollback_safe_screen").expect("find with_scrollback_safe_screen");
    let fn_end = term_src[fn_start..].find("fn clear_parser_scrollback").map(|i| fn_start + i).unwrap_or(term_src.len());
    let fn_body = &term_src[fn_start..fn_end];
    assert!(
        fn_body.contains("cfg(debug_assertions)") || fn_body.contains("cfg!(debug_assertions)"),
        "with_scrollback_safe_screen must guard parser.set_size widening under cfg(debug_assertions) so release builds do not allocate per frame"
    );
}
