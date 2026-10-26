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

