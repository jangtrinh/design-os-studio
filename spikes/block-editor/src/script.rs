//! THROWAWAY scripted run. Synthetic `egui::Event`s are injected through eframe's `raw_input_hook`, so
//! they travel the same path as OS events (TextEdit widgets, `Event::Ime`, key handling, layout);
//! only the OS-to-winit hop is skipped. `Check` steps print PASS/FAIL, `Shot` steps save PNGs.

use crate::app::App;
use crate::model::{char_len, Kind};
use egui::{Context, Event, ImeEvent, Key, Modifiers, RawInput};
use std::collections::VecDeque;

pub enum Step {
    Ev(Vec<Event>),
    Do(fn(&mut App)),
    Wait(u32),
    Shot(&'static str),
    Check(&'static str, fn(&App) -> Result<String, String>),
}

pub struct Script {
    steps: VecDeque<Step>,
    wait: u32,
    pass: u32,
    fail: u32,
}

fn press(key: Key, m: Modifiers) -> Vec<Event> {
    let mk = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: m };
    vec![mk(true), mk(false)]
}
fn k(key: Key) -> Step {
    Step::Ev(press(key, Modifiers::NONE))
}
fn txt(s: &str) -> Step {
    Step::Ev(vec![Event::Text(s.into())])
}
fn pre(s: &str) -> Step {
    Step::Ev(vec![Event::Ime(ImeEvent::Preedit { text: s.into(), active_range_chars: None })])
}
fn commit(s: &str) -> Step {
    Step::Ev(vec![Event::Ime(ImeEvent::Commit(s.into()))])
}

/// One Telex word: the preedit states the macOS IME shows while typing, then the commit.
fn word(states: &[&str]) -> Vec<Step> {
    let mut v: Vec<Step> = states.iter().map(|s| pre(s)).collect();
    v.push(commit(states.last().unwrap()));
    v
}

fn row_of(app: &App) -> usize {
    let b = &app.doc.blocks[app.focus];
    app.galleys.get(&b.id).map(|g| g.layout_from_cursor(app.cursor()).row).unwrap_or(99)
}
fn rows_of(app: &App, idx: usize) -> usize {
    app.galleys.get(&app.doc.blocks[idx].id).map(|g| g.rows.len()).unwrap_or(0)
}
fn ok(c: bool, msg: String) -> Result<String, String> {
    if c {
        Ok(msg)
    } else {
        Err(msg)
    }
}

pub fn scenario() -> Vec<Step> {
    use Step::*;
    let cmd = Modifiers::COMMAND;
    let mut s: Vec<Step> = vec![Wait(4)];
    // (a) caret across blocks
    s.push(Check("a0 starts in block 1", |a| ok(a.focus == 1, format!("focus={}", a.focus))));
    s.push(k(Key::ArrowUp));
    s.push(Check("a1 ArrowUp at first row -> block 0", |a| ok(a.focus == 0, format!("focus={}", a.focus))));
    s.push(k(Key::ArrowDown));
    s.push(k(Key::ArrowDown));
    s.push(k(Key::ArrowDown));
    s.push(Check("a2 ArrowDown x3 lands in the wrapped block 3, first row", |a| ok(a.focus == 3 && row_of(a) == 0, format!("focus={} row={} rows={}", a.focus, row_of(a), rows_of(a, 3)))));
    s.push(k(Key::ArrowDown));
    s.push(Check("a3 ArrowDown inside a wrapped block stays in it (row 1)", |a| ok(a.focus == 3 && row_of(a) == 1, format!("focus={} row={}", a.focus, row_of(a)))));
    s.push(k(Key::ArrowDown));
    s.push(Check("a4 ArrowDown on the last row -> block 4", |a| ok(a.focus == 4, format!("focus={} row={}", a.focus, row_of(a)))));
    s.push(k(Key::ArrowUp));
    s.push(Check("a5 ArrowUp enters block 3 on its LAST row", |a| ok(a.focus == 3 && row_of(a) + 1 == rows_of(a, 3), format!("focus={} row={}/{}", a.focus, row_of(a), rows_of(a, 3)))));
    s.push(k(Key::End));
    s.push(k(Key::ArrowRight));
    s.push(Check("a6 ArrowRight at block end -> block 4 start", |a| ok(a.focus == 4 && a.head == 0, format!("focus={} head={}", a.focus, a.head))));
    s.push(k(Key::ArrowLeft));
    s.push(Check("a7 ArrowLeft at block start -> block 3 end", |a| ok(a.focus == 3 && a.head == char_len(&a.doc.blocks[3].text), format!("focus={} head={}", a.focus, a.head))));
    // (b) split / merge and (e) undo
    s.push(Do(|a| a.go(1, 31)));
    s.push(Wait(2));
    s.push(k(Key::Enter));
    s.push(Check("b1 Enter splits block 1 at the caret", |a| ok(a.doc.blocks.len() == 10 && a.focus == 2 && a.head == 0 && a.doc.blocks[2].text.starts_with("and what") && a.doc.blocks[1].text.ends_with("checkout, "), format!("blocks={} focus={} tail={:?}", a.doc.blocks.len(), a.focus, &a.doc.blocks[2].text.chars().take(12).collect::<String>()))));
    s.push(Shot("1-split"));
    s.push(k(Key::Backspace));
    s.push(Check("b2 Backspace at start merges back", |a| ok(a.doc.blocks.len() == 9 && a.focus == 1 && a.head == 31 && a.doc.blocks[1].text.starts_with("How customers") && a.doc.blocks[1].text.ends_with("fails."), format!("blocks={} focus={} head={}", a.doc.blocks.len(), a.focus, a.head))));
    s.push(Shot("2-merge"));
    s.push(Ev(press(Key::Z, cmd)));
    s.push(Check("e1 Cmd+Z undoes the merge (split is back)", |a| ok(a.doc.blocks.len() == 10, format!("blocks={}", a.doc.blocks.len()))));
    s.push(Ev(press(Key::Z, cmd)));
    s.push(Check("e2 Cmd+Z undoes the split (original text)", |a| ok(a.doc.blocks.len() == 9 && a.doc.blocks[1].text.ends_with("checkout, and what happens when a payment fails."), format!("blocks={}", a.doc.blocks.len()))));
    s.push(Ev(press(Key::Z, cmd | Modifiers::SHIFT)));
    s.push(Check("e3 Cmd+Shift+Z redoes the split", |a| ok(a.doc.blocks.len() == 10, format!("blocks={}", a.doc.blocks.len()))));
    s.push(Ev(press(Key::Z, cmd)));
    s.push(Do(|a| a.go(8, 0)));
    s.push(Wait(2));
    s.push(txt("a"));
    s.push(txt("b"));
    s.push(txt("c"));
    s.push(Check("e4 typed 'abc' in block 8", |a| ok(a.doc.blocks[8].text == "abc", format!("text={:?}", a.doc.blocks[8].text))));
    s.push(Ev(press(Key::Z, cmd)));
    s.push(Check("e5 one Cmd+Z removes the whole typing burst", |a| ok(a.doc.blocks[8].text.is_empty(), format!("text={:?}", a.doc.blocks[8].text))));
    // (c) slash menu
    s.push(txt("/"));
    s.push(Check("c1 '/' opens the menu", |a| ok(a.slash.is_some(), format!("open={}", a.slash.is_some()))));
    s.push(Shot("3-slash-menu"));
    s.push(txt("head"));
    s.push(Check("c2 'head' filters to the three headings", |a| ok(a.slash.as_ref().map(|s| crate::slash::filter(&s.query).len()) == Some(3), format!("query={:?}", a.slash.as_ref().map(|s| s.query.clone())))));
    s.push(Shot("4-slash-filtered"));
    s.push(k(Key::ArrowDown));
    s.push(k(Key::Enter));
    s.push(Check("c3 Down+Enter inserts Heading 2, slash text removed", |a| ok(a.doc.blocks[8].kind == Kind::H2 && a.doc.blocks[8].text.is_empty() && a.slash.is_none(), format!("kind={:?} text={:?}", a.doc.blocks[8].kind, a.doc.blocks[8].text))));
    s.push(txt("Th"));
    s.push(k(Key::Enter));
    s.push(Check("c4 Enter after a heading starts a plain text block", |a| ok(a.doc.blocks.len() == 10 && a.doc.blocks[9].kind == Kind::Text && a.focus == 9, format!("blocks={} kind9={:?}", a.doc.blocks.len(), a.doc.blocks[9].kind))));
    // (d) Vietnamese Telex, simulated preedit/commit
    s.extend(word(&["T", "Th", "Tha", "Than", "Thanh"]));
    s.push(txt(" "));
    for st in ["t", "to", "toa", "toan", "toán"] {
        s.push(pre(st));
    }
    s.push(Check("d1 while 'toán' is still composing", |a| ok(!a.markdown().contains("toán"), format!("document contains the composing text: {}", a.markdown().contains("toán")))));
    s.push(commit("toán"));
    s.push(txt(" "));
    s.extend(word(&["b", "bă", "bằ", "bằn", "bằng"]));
    s.push(txt(" "));
    s.extend(word(&["t", "th", "thẻ"]));
    s.push(Check("d2 composed text is Thanh toán bằng thẻ", |a| ok(a.doc.blocks[9].text == "Thanh toán bằng thẻ", format!("text={:?}", a.doc.blocks[9].text))));
    s.push(Shot("5-vietnamese"));
    // (f) bold
    s.push(k(Key::Home));
    for _ in 0..5 {
        s.push(Ev(press(Key::ArrowRight, Modifiers::SHIFT)));
    }
    s.push(Ev(press(Key::B, cmd)));
    s.push(Check("f1 Cmd+B bolds the selected word", |a| ok(a.doc.blocks[9].bold == vec![0..5] && a.markdown().contains("**Thanh** toán bằng thẻ"), format!("bold={:?}", a.doc.blocks[9].bold))));
    s.push(Shot("6-bold"));
    s.push(Wait(2));
    s
}

impl Script {
    pub fn new() -> Self {
        Script { steps: scenario().into(), wait: 0, pass: 0, fail: 0 }
    }

    /// Called once per frame from `raw_input_hook`.
    pub fn tick(&mut self, app: &mut App, ctx: &Context, raw: &mut RawInput) {
        ctx.request_repaint();
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        match self.steps.pop_front() {
            Some(Step::Ev(ev)) => {
                raw.events.extend(ev);
                self.wait = 2;
            }
            Some(Step::Do(f)) => f(app),
            Some(Step::Wait(n)) => self.wait = n,
            Some(Step::Shot(name)) => {
                app.pending_shot = Some(name.into());
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.wait = 8;
            }
            Some(Step::Check(label, f)) => match f(app) {
                Ok(m) => {
                    self.pass += 1;
                    println!("PASS {label} :: {m}");
                }
                Err(m) => {
                    self.fail += 1;
                    println!("FAIL {label} :: {m}");
                }
            },
            None => {
                println!("RESULT mode={:?} pass={} fail={}", app.mode, self.pass, self.fail);
                println!("MARKDOWN-BEGIN\n{}MARKDOWN-END", app.markdown());
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}
