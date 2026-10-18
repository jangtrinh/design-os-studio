//! Text and numeric input fields.

use super::*;
use egui::text::{CCursor, CCursorRange};
use egui::{Align, Margin, TextEdit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prefix {
    None,
    Text(&'static str),
    Icon(Icon),
}

/// Single-line editable text rendered in `rect` (no chrome; see `field_bg`).
/// Returns the committed text when editing ends (Enter / focus loss).
pub fn edit_text(ui: &mut Ui, rect: Rect, id: Id, display: &str, font: FontId, color: Color32, pad_x: f32) -> Option<String> {
    edit_text_hint(ui, rect, id, display, font, color, pad_x, None)
}

/// Like [`edit_text`], with a hint (placeholder or "Mixed") shown while the text is empty.
///
/// Idle text is painted by us, on the exact baseline and elided with an ellipsis so it
/// never overflows; the egui `TextEdit` only draws it while it has focus.
#[allow(clippy::too_many_arguments)]
pub fn edit_text_hint(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    display: &str,
    font: FontId,
    color: Color32,
    pad_x: f32,
    hint: Option<(&str, Color32)>,
) -> Option<String> {
    let focused = ui.memory(|m| m.has_focus(id));
    let mut buf = if focused { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| display.to_string()) } else { display.to_string() };
    let te = TextEdit::singleline(&mut buf)
        .id(id)
        .font(font.clone())
        .text_color(if focused { color } else { Color32::TRANSPARENT })
        .frame(egui::Frame::NONE)
        .margin(Margin::symmetric(pad_x as i8, 0))
        .vertical_align(Align::Center)
        .min_size(rect.size())
        .desired_width(rect.width());
    let out = ui.put(rect, te);
    if out.gained_focus() {
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(buf.chars().count()))));
        state.store(ui.ctx(), id);
    }
    if out.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    }
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if buf.is_empty() {
        if let Some((h, hc)) = hint {
            super::text_on_baseline(&p, rect.left() + pad_x, super::centered_baseline(rect, &font), h, font.clone(), hc);
        }
    } else if !focused {
        super::field_text(&p, rect, pad_x, pad_x, &buf, font, color);
    }
    if out.lost_focus() {
        ui.data_mut(|d| d.remove_temp::<String>(id));
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !esc && buf != display {
            return Some(buf);
        }
    }
    None
}

/// Editable numeric text cell with live `ArrowUp`/`ArrowDown` (`±1`, `Shift ±10`, `Alt ±0.1`)
/// stepping, full-selection retention, and Figma math expression evaluation on commit.
#[allow(clippy::too_many_arguments)]
pub fn edit_num_text(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    value: Option<f32>,
    is_percent_field: bool,
    font: FontId,
    color: Color32,
    pad_x: f32,
    hint: Option<(&str, Color32)>,
) -> Option<f32> {
    let display = value.map(fmt_num).unwrap_or_default();
    let mut stepped: Option<(f32, String)> = None;
    if ui.memory(|m| m.has_focus(id)) {
        let mut key_step: Option<(f32, f32)> = None;
        ui.input_mut(|i| {
            if let Some(idx) =
                i.events.iter().position(|ev| matches!(ev, egui::Event::Key { key: egui::Key::ArrowUp | egui::Key::ArrowDown, pressed: true, .. }))
            {
                if let egui::Event::Key { key, modifiers, .. } = i.events.remove(idx) {
                    let dir = if key == egui::Key::ArrowUp { 1.0 } else { -1.0 };
                    let step = if modifiers.alt {
                        0.1
                    } else if modifiers.shift {
                        10.0
                    } else {
                        1.0
                    };
                    key_step = Some((dir, step));
                }
            }
        });
        if let Some((dir, step)) = key_step {
            let cur_buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| display.clone());
            let base = eval_number_expr(&cur_buf, value, is_percent_field).or(value).unwrap_or(0.0);
            let next = ((base + dir * step) * 100.0).round() / 100.0;
            let next_str = fmt_num(next);
            ui.data_mut(|d| d.insert_temp(id, next_str.clone()));
            stepped = Some((next, next_str));
        }
    }
    let disp_ref = stepped.as_ref().map(|(_, s)| s.as_str()).unwrap_or(&display);
    let committed = edit_text_hint(ui, rect, id, disp_ref, font, color, pad_x, hint);
    if let Some((next, next_str)) = stepped {
        ui.data_mut(|d| d.insert_temp(id, next_str.clone()));
        ui.memory_mut(|m| m.request_focus(id));
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(next_str.chars().count()))));
        state.store(ui.ctx(), id);
        return Some(next);
    }
    committed.and_then(|s| eval_number_expr(&s, value, is_percent_field))
}

/// Editable 6-char hex color field (`RRGGBB`) with Figma `822500` per-channel / whole-color
/// `ArrowUp`/`ArrowDown` (`±1`) and `Shift+ArrowUp`/`ArrowDown` (`±10`) live stepping.
pub fn edit_hex_text(ui: &mut Ui, rect: Rect, id: Id, display: &str, font: FontId, color: Color32, pad_x: f32) -> Option<String> {
    let mut stepped: Option<(String, usize, usize)> = None;
    if ui.memory(|m| m.has_focus(id)) {
        let mut key_step: Option<(bool, bool)> = None;
        ui.input_mut(|i| {
            if let Some(idx) =
                i.events.iter().position(|ev| matches!(ev, egui::Event::Key { key: egui::Key::ArrowUp | egui::Key::ArrowDown, pressed: true, .. }))
            {
                if let egui::Event::Key { key, modifiers, .. } = i.events.remove(idx) {
                    key_step = Some((key == egui::Key::ArrowUp, modifiers.shift));
                }
            }
        });
        if let Some((up, shift)) = key_step {
            let cur_buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| display.to_string());
            let has_hash = display.starts_with('#');
            let off = usize::from(has_hash);
            let (start, end) = TextEdit::load_state(ui.ctx(), id)
                .and_then(|st| st.cursor.char_range())
                .map(|cr| {
                    let r = cr.as_sorted_char_range();
                    (r.start.0.saturating_sub(off), r.end.0.saturating_sub(off))
                })
                .unwrap_or((0, 6));
            let collapsed = start == end;
            let mut step_r = start <= 1 || (collapsed && start == 2);
            let mut step_g = (start <= 3 && end > 2) || (collapsed && start == 4);
            let mut step_b = (start <= 5 && end > 4) || (collapsed && start == 6);
            if !step_r && !step_g && !step_b {
                step_r = true;
                step_g = true;
                step_b = true;
            }
            let base_col = designos_doc::Color::from_hex(&cur_buf).or_else(|| designos_doc::Color::from_hex(display)).unwrap_or(designos_doc::Color::BLACK);
            let [r, g, b, _] = base_col.to_u8();
            let delta: i16 = (if up { 1 } else { -1 }) * (if shift { 10 } else { 1 });
            let nr = if step_r { (r as i16 + delta).clamp(0, 255) as u8 } else { r };
            let ng = if step_g { (g as i16 + delta).clamp(0, 255) as u8 } else { g };
            let nb = if step_b { (b as i16 + delta).clamp(0, 255) as u8 } else { b };
            let prefix = if has_hash { "#" } else { "" };
            let next_hex = format!("{prefix}{nr:02X}{ng:02X}{nb:02X}");
            let (sel_start, sel_end) = match (step_r, step_g, step_b) {
                (true, false, false) => (off, off + 2),
                (false, true, false) => (off + 2, off + 4),
                (false, false, true) => (off + 4, off + 6),
                _ => (off, off + 6),
            };
            ui.data_mut(|d| d.insert_temp(id, next_hex.clone()));
            stepped = Some((next_hex, sel_start, sel_end));
        }
    }
    let disp_ref = stepped.as_ref().map(|(s, _, _)| s.as_str()).unwrap_or(display);
    let committed = edit_text_hint(ui, rect, id, disp_ref, font, color, pad_x, None);
    if let Some((next_hex, sel_start, sel_end)) = stepped {
        ui.data_mut(|d| d.insert_temp(id, next_hex.clone()));
        ui.memory_mut(|m| m.request_focus(id));
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(sel_start), CCursor::new(sel_end))));
        state.store(ui.ctx(), id);
        return Some(next_hex);
    }
    committed
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NumSuffix {
    None,
    Unit,
    Percent,
    MultX,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    Num(f32, NumSuffix),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

/// Byte-wise ASCII prefix check; unlike slicing `s[..n]` it cannot split a multi-byte character.
fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.as_bytes().get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

fn tokenize_expr(s: &str, current: Option<f32>) -> Option<Vec<Tok>> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let ch = rest.chars().next()?;
        if ch.is_whitespace() {
            i += ch.len_utf8();
            continue;
        }
        match ch {
            '(' => {
                tokens.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Tok::RParen);
                i += 1;
            }
            '+' => {
                tokens.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Tok::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                tokens.push(Tok::Caret);
                i += 1;
            }
            _ if starts_with_ignore_ascii_case(rest, "mixed") => {
                let cur = current?;
                tokens.push(Tok::Num(cur, NumSuffix::None));
                i += 5;
            }
            _ if ch.is_ascii_digit() || ch == '.' || ch == ',' => {
                let mut num_str = String::new();
                let mut seen_dot = false;
                while i < bytes.len() {
                    let b = bytes[i];
                    if b.is_ascii_digit() {
                        num_str.push(b as char);
                        i += 1;
                    } else if (b == b'.' || b == b',') && !seen_dot {
                        seen_dot = true;
                        num_str.push('.');
                        i += 1;
                    } else {
                        break;
                    }
                }
                // Optional scientific notation: e/E followed by [+-]?[0-9]+
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    let mut j = i + 1;
                    if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j].is_ascii_digit() {
                        num_str.push('e');
                        i += 1;
                        if bytes[i] == b'+' || bytes[i] == b'-' {
                            num_str.push(bytes[i] as char);
                            i += 1;
                        }
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            num_str.push(bytes[i] as char);
                            i += 1;
                        }
                    }
                }
                let val: f32 = num_str.parse().ok()?;
                // Skip horizontal spaces before unit/postfix suffix
                let saved = i;
                while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                let tail = &s[i..];
                let suffix = if tail.starts_with('%') {
                    i += 1;
                    NumSuffix::Percent
                } else if tail.starts_with('°') {
                    i += '°'.len_utf8();
                    NumSuffix::Unit
                } else if starts_with_ignore_ascii_case(tail, "deg") && !tail[3..].chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                    i += 3;
                    NumSuffix::Unit
                } else if (starts_with_ignore_ascii_case(tail, "px") || starts_with_ignore_ascii_case(tail, "ms"))
                    && !tail[2..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                {
                    i += 2;
                    NumSuffix::Unit
                } else if !tail.is_empty()
                    && (tail.starts_with('s') || tail.starts_with('S'))
                    && !tail[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                {
                    i += 1;
                    NumSuffix::Unit
                } else if !tail.is_empty()
                    && (tail.starts_with('x') || tail.starts_with('X'))
                    && !tail[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                {
                    i += 1;
                    NumSuffix::MultX
                } else {
                    i = saved;
                    NumSuffix::None
                };
                tokens.push(Tok::Num(val, suffix));
            }
            _ => return None,
        }
    }
    Some(tokens)
}

/// Maximum nesting of unary signs, exponent right-hand sides, parentheses and `op=` prefixes.
/// Deeper input is an invalid expression rather than unbounded recursion (matches `prototype_expr.rs`).
const MAX_EXPR_DEPTH: usize = 64;

fn parse_expr_tokens(tokens: &[Tok], pos: &mut usize, current: Option<f32>, is_percent_field: bool, depth: usize) -> Option<f32> {
    let mut lhs = parse_term_tokens(tokens, pos, current, is_percent_field, None, depth)?;
    while let Some(&op @ (Tok::Plus | Tok::Minus)) = tokens.get(*pos) {
        *pos += 1;
        let rhs = parse_term_tokens(tokens, pos, current, is_percent_field, Some(lhs), depth)?;
        lhs = match op {
            Tok::Plus => lhs + rhs,
            _ => lhs - rhs,
        };
    }
    Some(lhs)
}

fn parse_term_tokens(tokens: &[Tok], pos: &mut usize, current: Option<f32>, is_percent_field: bool, add_lhs: Option<f32>, depth: usize) -> Option<f32> {
    let mut lhs = parse_unary_tokens(tokens, pos, current, is_percent_field, add_lhs, depth)?;
    while let Some(&op @ (Tok::Star | Tok::Slash)) = tokens.get(*pos) {
        *pos += 1;
        let rhs = parse_unary_tokens(tokens, pos, current, is_percent_field, None, depth)?;
        lhs = match op {
            Tok::Star => lhs * rhs,
            _ => {
                if rhs == 0.0 {
                    return None;
                }
                lhs / rhs
            }
        };
    }
    Some(lhs)
}

fn parse_unary_tokens(tokens: &[Tok], pos: &mut usize, current: Option<f32>, is_percent_field: bool, add_lhs: Option<f32>, depth: usize) -> Option<f32> {
    // Every recursive cycle (unary sign, exponent rhs, parenthesis) passes through here.
    if depth > MAX_EXPR_DEPTH {
        return None;
    }
    match tokens.get(*pos) {
        Some(Tok::Plus) => {
            *pos += 1;
            parse_unary_tokens(tokens, pos, current, is_percent_field, add_lhs, depth + 1)
        }
        Some(Tok::Minus) => {
            *pos += 1;
            parse_unary_tokens(tokens, pos, current, is_percent_field, add_lhs, depth + 1).map(|v| -v)
        }
        _ => parse_power_tokens(tokens, pos, current, is_percent_field, add_lhs, depth),
    }
}

fn parse_power_tokens(tokens: &[Tok], pos: &mut usize, current: Option<f32>, is_percent_field: bool, add_lhs: Option<f32>, depth: usize) -> Option<f32> {
    let base = parse_primary_tokens(tokens, pos, current, is_percent_field, add_lhs, depth)?;
    if matches!(tokens.get(*pos), Some(Tok::Caret)) {
        *pos += 1;
        let exp = parse_unary_tokens(tokens, pos, current, is_percent_field, None, depth + 1)?;
        Some(base.powf(exp))
    } else {
        Some(base)
    }
}

fn parse_primary_tokens(tokens: &[Tok], pos: &mut usize, current: Option<f32>, is_percent_field: bool, add_lhs: Option<f32>, depth: usize) -> Option<f32> {
    match tokens.get(*pos).copied()? {
        Tok::Num(val, suffix) => {
            *pos += 1;
            match suffix {
                NumSuffix::None | NumSuffix::Unit => Some(val),
                NumSuffix::MultX => Some(current? * val),
                NumSuffix::Percent => {
                    if is_percent_field {
                        Some(val)
                    } else {
                        Some(current.or(add_lhs)? * (val / 100.0))
                    }
                }
            }
        }
        Tok::LParen => {
            *pos += 1;
            let val = parse_expr_tokens(tokens, pos, current, is_percent_field, depth + 1)?;
            if !matches!(tokens.get(*pos), Some(Tok::RParen)) {
                return None;
            }
            *pos += 1;
            Some(val)
        }
        _ => None,
    }
}

/// Full Figma math expression evaluator (`+`, `-`, `*`, `/`, `^`, `( )`, unary `+/-`,
/// scientific notation, comma decimals, unit suffixes `px`/`deg`/`°`/`ms`/`s`,
/// postfix `%` and `x`, and relative operators `+=`, `-=`, `*=`, `/=`, `*N`, `/N`, `Mixed+N`).
pub fn eval_number_expr(input: &str, current: Option<f32>, is_percent_field: bool) -> Option<f32> {
    eval_number_expr_at(input, current, is_percent_field, 0)
}

fn eval_number_expr_at(input: &str, current: Option<f32>, is_percent_field: bool, depth: usize) -> Option<f32> {
    if depth > MAX_EXPR_DEPTH {
        return None;
    }
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("+=") {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        return Some(cur + rhs).filter(|v| v.is_finite());
    }
    if let Some(rest) = s.strip_prefix("-=") {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        return Some(cur - rhs).filter(|v| v.is_finite());
    }
    if let Some(rest) = s.strip_prefix("*=") {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        return Some(cur * rhs).filter(|v| v.is_finite());
    }
    if let Some(rest) = s.strip_prefix("/=") {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        if rhs == 0.0 {
            return None;
        }
        return Some(cur / rhs).filter(|v| v.is_finite());
    }
    if let Some(rest) = s.strip_prefix('*') {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        return Some(cur * rhs).filter(|v| v.is_finite());
    }
    if let Some(rest) = s.strip_prefix('/') {
        let cur = current?;
        let rhs = eval_number_expr_at(rest, current, is_percent_field, depth + 1)?;
        if rhs == 0.0 {
            return None;
        }
        return Some(cur / rhs).filter(|v| v.is_finite());
    }
    let tokens = tokenize_expr(s, current)?;
    if tokens.is_empty() {
        return None;
    }
    let mut pos = 0;
    let res = parse_expr_tokens(&tokens, &mut pos, current, is_percent_field, 0)?;
    if pos != tokens.len() || !res.is_finite() {
        return None;
    }
    Some(res)
}

pub(crate) fn parse_number(s: &str) -> Option<f32> {
    eval_number_expr(s, None, true)
}

/// Numeric field with a prefix label/icon. `value == None` shows "Mixed".
///
/// Figma UI3 (`ui-spec.json` `input_fields`): prefix letter or 16px icon at x+8 in
/// `text_secondary`, value at x+24 (x+8 without a prefix), suffix right after the value
/// in `text_secondary`. Integers print without decimals, other values with at most two.
pub fn num_field(ui: &mut Ui, rect: Rect, key: impl crate::widgets::Key, prefix: Prefix, value: Option<f32>, suffix: &str) -> Option<f32> {
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        let k = format!("{key:?}");
        d.insert_temp(Id::new(("field-rect", k.clone())), rect);
        d.insert_temp(Id::new(("field-prefix", k)), prefix);
    });
    let id = ui.id().with(("num", key));
    let display = value.map(fmt_num).unwrap_or_default();
    let has_prefix = !matches!(prefix, Prefix::None);
    let pw = if has_prefix { theme::FIELD_PREFIX_W } else { 0.0 };
    field_bg(ui, rect, id);
    let p = ui.painter().clone();
    match prefix {
        Prefix::None => {}
        Prefix::Text(s) => {
            // 8px in; the value starts 24px in.
            super::field_text(
                &p,
                rect,
                theme::FIELD_PREFIX_X,
                rect.width() - theme::FIELD_PREFIX_X - theme::FIELD_PREFIX_ICON,
                s,
                theme::font_body(),
                theme::TEXT_SECONDARY,
            );
        }
        Prefix::Icon(i) => {
            let slot = Rect::from_min_size(
                Pos2::new(rect.left() + theme::FIELD_PREFIX_X, rect.center().y - theme::FIELD_PREFIX_GLYPH / 2.0),
                Vec2::splat(theme::FIELD_PREFIX_GLYPH),
            );
            super::paint_icon_in(&p, slot, i, theme::FIELD_PREFIX_GLYPH, theme::TEXT_SECONDARY);
        }
    }
    let scrubbed = if has_prefix { scrub_prefix(ui, Rect::from_min_size(rect.min, egui::vec2(pw, rect.height())), id, value) } else { None };
    let wheel_step = if ui.rect_contains_pointer(rect) {
        ui.input(|i| {
            if i.modifiers.alt {
                let delta = i.smooth_scroll_delta.y + i.smooth_scroll_delta.x;
                if delta.abs() > 0.5 {
                    let step = if i.modifiers.shift { 10.0 } else { 1.0 };
                    Some(value.unwrap_or(0.0) + delta.signum() * step)
                } else {
                    None
                }
            } else {
                None
            }
        })
    } else {
        None
    };
    // The suffix is drawn after the value, so the edit area stops short of it.
    let font = theme::font_body();
    let suffix_w = if suffix.is_empty() {
        0.0
    } else {
        super::text_width(&p, suffix.trim_start(), &font) + if suffix.starts_with(' ') { theme::FIELD_SUFFIX_GAP } else { 0.0 }
    };
    let text_rect = Rect::from_min_max(Pos2::new(rect.left() + pw, rect.top()), Pos2::new((rect.right() - suffix_w).max(rect.left() + pw), rect.bottom()));
    let hint = Some(("Mixed", theme::TEXT_SECONDARY));
    let pad = if has_prefix { 0.0 } else { theme::FIELD_PLAIN_X };
    let is_percent_field = suffix.contains('%');
    let committed = edit_num_text(ui, text_rect, id, value, is_percent_field, font.clone(), theme::TEXT, pad, hint);
    if !suffix.is_empty() && (value.is_some() || ui.memory(|m| m.has_focus(id))) {
        let shown = if ui.memory(|m| m.has_focus(id)) { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| display.clone()) } else { display.clone() };
        let x = text_rect.left() + pad + super::text_width(&p, &shown, &font) + if suffix.starts_with(' ') { theme::FIELD_SUFFIX_GAP } else { 0.0 };
        let b = super::centered_baseline(rect, &font);
        if x + suffix_w <= rect.right() + 0.5 {
            super::text_on_baseline(&p.with_clip_rect(rect), x, b, suffix.trim_start(), font, theme::TEXT_SECONDARY);
        }
    }
    committed.or(scrubbed).or(wheel_step)
}

/// Context-data key that is true while any numeric label is being scrubbed;
/// the app snapshots the document on its rising edge and folds all edits
/// made during the drag into one undo step on its falling edge.
pub const SCRUB_FLAG: &str = "designos-scrub-active";

/// Figma-style scrubbing: dragging a field's prefix changes the value by one
/// unit per pixel (x10 with Shift, x0.1 with Alt) and shows the horizontal-resize cursor.
/// Clicking the prefix without dragging focuses the numeric field.
fn scrub_prefix(ui: &mut Ui, prefix: Rect, id: Id, value: Option<f32>) -> Option<f32> {
    let sid = id.with("scrub");
    let resp = ui.interact(prefix, sid, Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if ui.rect_contains_pointer(prefix) && ui.input(|i| i.pointer.primary_clicked()) && !resp.dragged() {
        ui.memory_mut(|m| m.request_focus(id));
        let display = value.map(fmt_num).unwrap_or_default();
        ui.data_mut(|d| d.insert_temp(id, display.clone()));
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(display.chars().count()))));
        state.store(ui.ctx(), id);
    }
    let flag = Id::new(SCRUB_FLAG);
    if resp.drag_started() {
        ui.ctx().data_mut(|d| {
            d.insert_temp(flag, true);
            d.insert_temp(sid, (value.unwrap_or(0.0), 0.0f32, f32::NAN));
        });
    }
    let mut out = None;
    if resp.dragged() {
        let (dx, shift, alt) = ui.input(|i| (i.pointer.delta().x, i.modifiers.shift, i.modifiers.alt));
        let (start, acc, last) = ui.ctx().data(|d| d.get_temp::<(f32, f32, f32)>(sid)).unwrap_or((value.unwrap_or(0.0), 0.0, f32::NAN));
        let mult = if shift {
            10.0
        } else if alt {
            0.1
        } else {
            1.0
        };
        let acc = acc + dx * mult;
        let delta = if alt { (acc * 10.0).round() / 10.0 } else { acc.round() };
        let v = ((start + delta) * 100.0).round() / 100.0;
        if v != last && !(last.is_nan() && delta == 0.0) {
            out = Some(v);
        }
        ui.ctx().data_mut(|d| d.insert_temp(sid, (start, acc, if out.is_some() { v } else { last })));
    }
    if resp.drag_stopped() {
        ui.ctx().data_mut(|d| d.insert_temp(flag, false));
    }
    out
}

/// Paints the standard field background with hover/focus border.
pub fn field_bg(ui: &Ui, rect: Rect, id: Id) {
    let focused = ui.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(rect);
    ui.painter().rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if hovered {
        ui.painter().rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
}

/// Plain text field with standard background; returns committed text.
pub fn text_input(ui: &mut Ui, rect: Rect, key: impl crate::widgets::Key, display: &str) -> Option<String> {
    text_input_ph(ui, rect, key, display, "")
}

/// [`text_input`] with a placeholder (in `text_tertiary`) shown while empty.
pub fn text_input_ph(ui: &mut Ui, rect: Rect, key: impl crate::widgets::Key, display: &str, placeholder: &str) -> Option<String> {
    let id = ui.id().with(("txt", key));
    field_bg(ui, rect, id);
    let hint = (!placeholder.is_empty()).then_some((placeholder, theme::TEXT_TERTIARY));
    edit_text_hint(ui, rect, id, display, theme::font_body(), theme::TEXT, theme::FIELD_PLAIN_X, hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbers_and_expressions() {
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number(" 12.5% "), Some(12.5));
        assert_eq!(parse_number("90°"), Some(90.0));
        assert_eq!(parse_number("100/2"), Some(50.0));
        assert_eq!(parse_number("10+5"), Some(15.0));
        assert_eq!(parse_number("-4"), Some(-4.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("1/0"), None);
    }

    #[test]
    fn non_ascii_input_never_panics() {
        for input in ["5€", "100 ₫", "日本", "10 độ", "日本語テキスト", "5 日本", "1e€", "€", "10 pxé", "mixé€"] {
            assert_eq!(eval_number_expr(input, Some(3.0), false), None, "{input}");
        }
        assert_eq!(eval_number_expr("Mixed+2", Some(3.0), false), Some(5.0));
        assert_eq!(eval_number_expr("45 deg", None, false), Some(45.0));
        assert_eq!(eval_number_expr("12 px", None, false), Some(12.0));
    }

    #[test]
    fn evaluates_full_figma_math_expressions() {
        assert_eq!(eval_number_expr("100+24", None, false), Some(124.0));
        assert_eq!(eval_number_expr("10 + 5 * 2", None, false), Some(20.0));
        assert_eq!(eval_number_expr("(10 + 5) * 2", None, false), Some(30.0));
        assert_eq!(eval_number_expr("2^3", None, false), Some(8.0));
        assert_eq!(eval_number_expr("12,5 + 0,5", None, false), Some(13.0));
        assert_eq!(eval_number_expr("100px + 20px", None, false), Some(120.0));
        assert_eq!(eval_number_expr("90deg + 45°", None, false), Some(135.0));
        assert_eq!(eval_number_expr("50%", Some(200.0), false), Some(100.0));
        assert_eq!(eval_number_expr("50%", Some(200.0), true), Some(50.0));
        assert_eq!(eval_number_expr("200 + 25%", Some(200.0), false), Some(250.0));
        assert_eq!(eval_number_expr("200 + 25%", None, false), Some(250.0));
        assert_eq!(eval_number_expr("2x", Some(150.0), false), Some(300.0));
        assert_eq!(eval_number_expr("+=10", Some(50.0), false), Some(60.0));
        assert_eq!(eval_number_expr("-=5", Some(50.0), false), Some(45.0));
        assert_eq!(eval_number_expr("*=2", Some(50.0), false), Some(100.0));
        assert_eq!(eval_number_expr("/=2", Some(50.0), false), Some(25.0));
        assert_eq!(eval_number_expr("*2", Some(50.0), false), Some(100.0));
        assert_eq!(eval_number_expr("/2", Some(50.0), false), Some(25.0));
        assert_eq!(eval_number_expr("Mixed+20", Some(80.0), false), Some(100.0));
        assert_eq!(eval_number_expr("Mixed-10", Some(80.0), false), Some(70.0));
    }

    /// Runs `f` on a 256 KiB stack so unbounded recursion aborts the test process instead of passing.
    fn eval_on_small_stack(input: String) -> Option<f32> {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || eval_number_expr(&input, Some(3.0), false))
            .expect("spawn small-stack thread")
            .join()
            .expect("evaluator must not panic")
    }

    #[test]
    fn deep_unary_signs_are_rejected_without_stack_overflow() {
        assert_eq!(eval_on_small_stack(format!("{}1", "-".repeat(100_000))), None);
        assert_eq!(eval_on_small_stack(format!("{}1", "+".repeat(100_000))), None);
    }

    #[test]
    fn deep_parentheses_are_rejected_without_stack_overflow() {
        assert_eq!(eval_on_small_stack("(".repeat(100_000)), None);
        assert_eq!(eval_on_small_stack(format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000))), None);
    }

    #[test]
    fn deep_exponent_chain_is_rejected_without_stack_overflow() {
        assert_eq!(eval_on_small_stack(vec!["2"; 50_000].join("^")), None);
    }

    #[test]
    fn deep_relative_operator_prefixes_are_rejected_without_stack_overflow() {
        assert_eq!(eval_on_small_stack(format!("{}1", "+=".repeat(100_000))), None);
    }

    #[test]
    fn moderate_nesting_still_evaluates() {
        assert_eq!(eval_number_expr(&format!("{}1", "-".repeat(10)), None, false), Some(1.0));
        assert_eq!(eval_number_expr(&format!("{}5{}", "(".repeat(20), ")".repeat(20)), None, false), Some(5.0));
        assert_eq!(eval_number_expr("2^2^3", None, false), Some(256.0));
    }

    #[test]
    fn arrow_and_shift_arrow_step_num_field_and_hex_text() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(120.0, 24.0));
        let mut val = 100.0f32;
        let mut num_id = Id::NULL;

        // Frame 1: render num_field and focus it.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            num_id = ui.id().with(("num", "test-w"));
            ui.memory_mut(|m| m.request_focus(num_id));
            if let Some(v) = num_field(ui, rect, "test-w", Prefix::Text("W"), Some(val), "") {
                val = v;
            }
        });

        // Frame 2: press ArrowUp -> +1 (101.0)
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: egui::Key::ArrowUp, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        let _ = ctx.run_ui(input, |ui| {
            if let Some(v) = num_field(ui, rect, "test-w", Prefix::Text("W"), Some(val), "") {
                val = v;
            }
        });
        assert_eq!(val, 101.0);
        assert!(ctx.memory(|m| m.has_focus(num_id)));

        // Frame 3: press Shift + ArrowUp -> +10 (111.0)
        let mut input = egui::RawInput { modifiers: egui::Modifiers::SHIFT, ..Default::default() };
        input.events.push(egui::Event::Key { key: egui::Key::ArrowUp, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::SHIFT });
        let _ = ctx.run_ui(input, |ui| {
            if let Some(v) = num_field(ui, rect, "test-w", Prefix::Text("W"), Some(val), "") {
                val = v;
            }
        });
        assert_eq!(val, 111.0);

        // Frame 4: press Alt + ArrowDown -> -0.1 (110.9)
        let mut input = egui::RawInput { modifiers: egui::Modifiers::ALT, ..Default::default() };
        input.events.push(egui::Event::Key { key: egui::Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::ALT });
        let _ = ctx.run_ui(input, |ui| {
            if let Some(v) = num_field(ui, rect, "test-w", Prefix::Text("W"), Some(val), "") {
                val = v;
            }
        });
        assert!((val - 110.9).abs() < 1e-4);

        // Test edit_hex_text stepping (whole hex and single channel).
        let mut hex = "102030".to_string();
        let hex_id = Id::new("test-hex");
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.memory_mut(|m| m.request_focus(hex_id));
            if let Some(h) = edit_hex_text(ui, rect, hex_id, &hex, theme::font_body(), theme::TEXT, 0.0) {
                hex = h;
            }
        });

        // Whole selection (0..6) + Shift+ArrowUp -> +10 (0x0A) to all 3 channels: 1A2A3A
        let mut input = egui::RawInput { modifiers: egui::Modifiers::SHIFT, ..Default::default() };
        input.events.push(egui::Event::Key { key: egui::Key::ArrowUp, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::SHIFT });
        let _ = ctx.run_ui(input, |ui| {
            if let Some(h) = edit_hex_text(ui, rect, hex_id, &hex, theme::font_body(), theme::TEXT, 0.0) {
                hex = h;
            }
        });
        assert_eq!(hex, "1A2A3A");

        // Place cursor at index 3 (inside G channel) and press ArrowUp -> only G increments: 1A2B3A
        let mut state = TextEdit::load_state(&ctx, hex_id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(3))));
        state.store(&ctx, hex_id);

        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: egui::Key::ArrowUp, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        let _ = ctx.run_ui(input, |ui| {
            if let Some(h) = edit_hex_text(ui, rect, hex_id, &hex, theme::font_body(), theme::TEXT, 0.0) {
                hex = h;
            }
        });
        assert_eq!(hex, "1A2B3A");
    }
}
