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
//! The replacement is the neutral value of the slot: `1.0` for a struct field whose default is one
//! (`opacity`, a colour's alpha `a`, the diagonal `a` / `d` of an identity matrix) and `0.0` for
//! every other number, including every map value and sequence element whatever its key is called,
//! so a write and a read of the same slot agree. Zero also keeps infinity out of later transform
//! math, which a clamp to `f32::MAX` would not. An `Option<f32>` is left alone: a non-finite value
//! there is written as `null`, which reads back as `None`, so nothing is lost and nothing fails.
//!
//! Both work on any `serde` type, so a number added to the model later is covered without
//! touching a field attribute. Every replacement is counted so the caller can report it.

mod de;
mod de_access;
mod ser;

use serde::{Deserialize, Serialize};
use std::cell::Cell;

// Counts the lenient parses a thread has started, so a test can show that an unrelated error never
// costs a second parse of the whole file.
#[cfg(test)]
thread_local! {
    static LENIENT_PARSES: Cell<usize> = const { Cell::new(0) };
    static WRAPPED_STR_VISITS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn lenient_parses_on_this_thread() -> usize {
    LENIENT_PARSES.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn record_wrapped_str_visit() {
    WRAPPED_STR_VISITS.with(|n| n.set(n.get() + 1));
}

#[cfg(test)]
pub(crate) fn wrapped_str_visits_on_this_thread() -> usize {
    WRAPPED_STR_VISITS.with(Cell::get)
}

thread_local! {
    static SERIALIZE_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// How many times [`to_string_finite`] built a JSON text on this thread. A save is expected to build
/// exactly one, so a test reads it before and after to show that no path serializes a document twice.
#[doc(hidden)]
pub fn serialize_calls_on_this_thread() -> usize {
    SERIALIZE_CALLS.with(Cell::get)
}

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
    SERIALIZE_CALLS.with(|n| n.set(n.get() + 1));
    let replaced = Cell::new(0);
    let mut out = Vec::with_capacity(128);
    value.serialize(ser::Finite::new(&mut serde_json::Serializer::new(&mut out), &replaced))?;
    // SAFETY: `serde_json::Serializer` writes only valid UTF-8 (it escapes every string and writes
    // numbers and punctuation as ASCII), which is why `serde_json::to_string` skips the check too.
    let text = unsafe { String::from_utf8_unchecked(out) };
    Ok((text, replaced.get()))
}

/// Counts the numbers [`to_string_finite`] would replace in `value`, without building the text.
/// The walk formats into a sink, so it allocates no output (an embedded image is still hex encoded
/// into a temporary buffer on the way), which makes it cheap enough to run on a document that is
/// usually clean before deciding whether anything needs the full serialization.
pub fn count_non_finite<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<usize> {
    let replaced = Cell::new(0);
    value.serialize(ser::Finite::new(&mut serde_json::Serializer::new(std::io::sink()), &replaced))?;
    Ok(replaced.get())
}

/// Parses `s`, reading a `null` where an `f32` / `f64` is expected as the neutral value of that
/// field instead of failing. Returns the value and how many nulls were replaced. Slower than
/// `serde_json::from_str`; use it only after the strict parse failed.
pub fn from_str_lenient<'a, T: Deserialize<'a>>(s: &'a str) -> serde_json::Result<(T, usize)> {
    #[cfg(test)]
    LENIENT_PARSES.with(|n| n.set(n.get() + 1));
    let replaced = Cell::new(0);
    let mut parser = serde_json::Deserializer::from_str(s);
    let value = T::deserialize(de::Lenient::new(&mut parser, &replaced))?;
    parser.end()?;
    Ok((value, replaced.get()))
}

/// Parses `s` strictly while recording in `null_numbers` whether any `f32` / `f64` slot encountered
/// a `null` token, the one error a lenient parse can repair.
fn from_str_strict<'a, T: Deserialize<'a>>(s: &'a str, null_numbers: &Cell<usize>) -> serde_json::Result<T> {
    let mut parser = serde_json::Deserializer::from_str(s);
    let value = T::deserialize(de::Lenient::strict(&mut parser, null_numbers))?;
    parser.end()?;
    Ok(value)
}

/// Parses `s` strictly, falling back to [`from_str_lenient`] only when the strict parse failed on a
/// `null` number (what an older version saved for a NaN or infinity). Any other error is returned
/// at once from the strict parse, without a second pass over the text. Returns the value and how
/// many nulls were replaced (zero when the text parsed strictly). When the lenient parse fails too,
/// its error is returned: it comes from the first problem that is not a `null` number.
pub fn from_str_repairing<'a, T: Deserialize<'a>>(s: &'a str) -> serde_json::Result<(T, usize)> {
    let null_numbers = Cell::new(0);
    match from_str_strict(s, &null_numbers) {
        Ok(value) => Ok((value, 0)),
        Err(strict) if strict.is_data() && null_numbers.get() > 0 => from_str_lenient(s),
        Err(strict) => Err(strict),
    }
}

#[derive(Debug)]
struct NonFiniteFound;

impl std::fmt::Display for NonFiniteFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("non-finite float")
    }
}

impl std::error::Error for NonFiniteFound {}

impl serde::ser::Error for NonFiniteFound {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self
    }
}

/// Zero-allocation `Serializer` probe that aborts with `Err(NonFiniteFound)` on the first
/// non-finite `f32` or `f64` it visits.
struct NonFiniteProbe;

impl serde::Serializer for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i8(self, _: i8) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i16(self, _: i16) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i32(self, _: i32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i64(self, _: i64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i128(self, _: i128) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u8(self, _: u8) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u16(self, _: u16) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u32(self, _: u32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u64(self, _: u64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u128(self, _: u128) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(NonFiniteFound)
        }
    }
    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(NonFiniteFound)
        }
    }
    fn serialize_char(self, _: char) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_str(self, _: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _: &'static str, _: u32, _: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(Self)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(Self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(Self)
    }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(Self)
    }
}

impl serde::ser::SerializeSeq for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTuple for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleStruct for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleVariant for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeMap for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(Self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStruct for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStructVariant for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Replaces any non-finite numbers in `value` with the neutral value of their field in place,
/// returning how many numbers were replaced. When `value` is already finite, no allocation or
/// serialization occurs.
pub fn neutralize_in_place<T: Serialize + for<'de> Deserialize<'de>>(value: &mut T) -> usize {
    if value.serialize(NonFiniteProbe).is_ok() {
        return 0;
    }
    let replaced = Cell::new(0);
    let mut out = Vec::with_capacity(128);
    if value.serialize(ser::Finite::new(&mut serde_json::Serializer::new(&mut out), &replaced)).is_ok() {
        if let Ok(clean) = serde_json::from_slice(&out) {
            *value = clean;
        }
    }
    replaced.get()
}
