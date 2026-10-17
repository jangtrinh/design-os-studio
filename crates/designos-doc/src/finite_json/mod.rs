//! JSON that never carries a non-finite number, in both directions.
//!
//! `serde_json` writes a NaN or infinite float as `null` and then refuses `null` where an `f32`
//! or `f64` is expected, so one bad number used to turn a saved file into
//! `json: invalid type: null, expected f32` on the next open. Two wrappers close that gap:
//!
//! * [`to_string_finite`] writes compact JSON in which every non-finite number is replaced by a
//!   finite one, so a file written by this version always opens. For a finite document the text
//!   is byte-identical to `serde_json::to_string`.
//! * [`from_str_repairing`] reads a file written by an older version, where a damaged slot holds
//!   `null`, and substitutes the same replacement value. A file that parses is read exactly as
//!   `serde_json::from_str` reads it, at the same speed.
//!
//! The replacement is the neutral value of the slot: `1.0` for the fields whose default is one
//! (`opacity`, a colour's alpha `a`, the diagonal `a` / `d` of an identity matrix) and `0.0` for
//! every other number. Zero also keeps infinity out of later transform math, which a clamp to
//! `f32::MAX` would not. An `Option<f32>` is left alone: a non-finite value there is written as
//! `null`, which reads back as `None`, so nothing is lost and nothing fails.
//!
//! Both work on any `serde` type, so a number added to the model later is covered without
//! touching a field attribute. Every replacement is counted so the caller can report it.

mod de;
mod de_access;
mod ser;

use serde::{Deserialize, Serialize};
use std::cell::Cell;

/// The value a non-finite number is replaced with, by the name of the field that holds it.
pub(crate) fn neutral_for(field: &str) -> f64 {
    match field {
        "opacity" | "a" | "d" => 1.0,
        _ => 0.0,
    }
}

/// Serialises `value` as compact JSON, replacing every non-finite `f32` / `f64` with the neutral
/// value of its field. Returns the text and how many numbers were replaced.
pub fn to_string_finite<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<(String, usize)> {
    let replaced = Cell::new(0);
    let mut out = Vec::with_capacity(128);
    value.serialize(ser::Finite::new(&mut serde_json::Serializer::new(&mut out), &replaced))?;
    // SAFETY: `serde_json::Serializer` writes only valid UTF-8 (it escapes every string and writes
    // numbers and punctuation as ASCII), which is why `serde_json::to_string` skips the check too.
    let text = unsafe { String::from_utf8_unchecked(out) };
    Ok((text, replaced.get()))
}

/// Parses `s`, reading a `null` where an `f32` / `f64` is expected as the neutral value of that
/// field instead of failing. Returns the value and how many nulls were replaced. Slower than
/// `serde_json::from_str`; use it only after the strict parse failed.
pub fn from_str_lenient<'a, T: Deserialize<'a>>(s: &'a str) -> serde_json::Result<(T, usize)> {
    let replaced = Cell::new(0);
    let mut parser = serde_json::Deserializer::from_str(s);
    let value = T::deserialize(de::Lenient::new(&mut parser, &replaced))?;
    parser.end()?;
    Ok((value, replaced.get()))
}

/// `serde_json::from_str`, falling back to [`from_str_lenient`] when the strict parse says the data
/// was wrong (typically a `null` number saved by an older version). Returns the value and how many
/// nulls were replaced (zero when the file parsed strictly). When both fail, the lenient error is
/// returned: it comes from the first problem that is not a `null` number.
pub fn from_str_repairing<'a, T: Deserialize<'a>>(s: &'a str) -> serde_json::Result<(T, usize)> {
    match serde_json::from_str(s) {
        Ok(value) => Ok((value, 0)),
        Err(strict) if strict.is_data() => from_str_lenient(s),
        Err(strict) => Err(strict),
    }
}
