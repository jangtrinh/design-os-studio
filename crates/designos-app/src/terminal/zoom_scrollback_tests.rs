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
        let widened = offset > usize::from(rows);
        if widened {
            parser.set_size(offset as u16, cols);
        }
        let line = row_text(parser.screen(), 0, cols);
        if widened {
            parser.set_size(rows, cols);
        }
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

