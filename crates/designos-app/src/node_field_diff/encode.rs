//! A canonical byte encoding of serde's data model, made to be compared and never read back.
//!
//! Two values encode to the same bytes exactly when `serde_json::to_value` gives the same `Value` for them,
//! because the mapping is `serde_json`'s own:
//!
//! * `Some(x)` is `x` and `None`, `()` and a unit struct are `null`;
//! * a number that JSON cannot hold (NaN, infinity) is `null`, and `-0.0` is `0.0`;
//! * a unit variant is its name, any other variant is an object of one key, the variant's name;
//! * a newtype struct is its content, a struct and a map are objects, a sequence and a tuple are arrays;
//! * a field that `skip_serializing_if` leaves out is not there, which is not the same as `null`.
//!
//! The encoding is a prefix code, so no two different values share bytes. It follows `Serialize` itself, so
//! skipped defaults, custom `Serialize` impls and `serde` attributes all apply without being listed anywhere.
//!
//! [`Tape::capture`] keeps only the top-level fields it is asked for, each with its own span, so two values
//! can be compared field by field. The buffers are reused from one capture to the next: encoding allocates
//! nothing once they have grown to the size of the largest field seen.

use serde::ser::{self, Serialize};
use std::fmt;

const NULL: u8 = 0;
const FALSE: u8 = 1;
const TRUE: u8 = 2;
const UNSIGNED: u8 = 3;
const NEGATIVE: u8 = 4;
const FLOAT: u8 = 5;
const STRING: u8 = 6;
const ARRAY: u8 = 7;
const OBJECT: u8 = 8;
const END: u8 = 9;

/// The wanted top-level fields of one value, encoded.
#[derive(Default)]
pub(super) struct Tape {
    bytes: Vec<u8>,
    /// `(name, start, end)` into `bytes`, in the order the value serialized them.
    fields: Vec<(&'static str, usize, usize)>,
}

impl Tape {
    /// Serializes `value`, which must be a struct, keeping the fields named in `wanted` (at most 64) that it
    /// serializes at all. A field `skip_serializing_if` leaves out has no span. Returns false when `value`
    /// cannot be encoded, with the tape left empty.
    pub(super) fn capture<T: Serialize + ?Sized>(&mut self, value: &T, wanted: &[&'static str]) -> bool {
        debug_assert!(wanted.len() <= 64);
        self.bytes.clear();
        self.fields.clear();
        let encoded = value.serialize(Encoder { tape: self, wanted: Some(wanted) }).is_ok();
        if !encoded {
            self.bytes.clear();
            self.fields.clear();
        }
        encoded
    }

    /// The number of bytes the last capture wrote.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Drops the buffers when one huge field grew them past `limit` bytes, so a thread does not keep them for good.
    pub(super) fn release_if_larger_than(&mut self, limit: usize) {
        if self.bytes.capacity() > limit {
            self.bytes = Vec::new();
        }
    }

    /// Calls `report` with the name of every field that is in one tape and not the other, or in both with
    /// different bytes.
    pub(super) fn for_each_difference(&self, other: &Tape, mut report: impl FnMut(&'static str)) {
        let mut matched = 0u64;
        for &(name, start, end) in &self.fields {
            match other.fields.iter().position(|f| f.0 == name) {
                Some(j) => {
                    matched |= 1 << j;
                    let (_, other_start, other_end) = other.fields[j];
                    if self.bytes[start..end] != other.bytes[other_start..other_end] {
                        report(name);
                    }
                }
                None => report(name),
            }
        }
        for (j, &(name, _, _)) in other.fields.iter().enumerate() {
            if matched & (1 << j) == 0 {
                report(name);
            }
        }
    }

    fn put_string(&mut self, s: &str) {
        self.bytes.push(STRING);
        // the length as a varint: one byte for every field name and most strings
        let mut len = s.len();
        while len >= 0x80 {
            self.bytes.push(len as u8 | 0x80);
            len >>= 7;
        }
        self.bytes.push(len as u8);
        self.bytes.extend_from_slice(s.as_bytes());
    }

    fn put_unsigned(&mut self, v: u64) {
        self.bytes.push(UNSIGNED);
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    fn put_signed(&mut self, v: i64) {
        if v >= 0 {
            self.put_unsigned(v as u64);
        } else {
            self.bytes.push(NEGATIVE);
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    fn put_float(&mut self, v: f64) {
        if v.is_finite() {
            // `-0.0 == 0.0` as JSON numbers, so both take the bits of `0.0`
            let v = if v == 0.0 { 0.0 } else { v };
            self.bytes.push(FLOAT);
            self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        } else {
            self.bytes.push(NULL);
        }
    }
}

#[derive(Debug)]
pub(super) struct EncodeError;

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("value cannot be encoded for comparison")
    }
}

impl std::error::Error for EncodeError {}

impl ser::Error for EncodeError {
    fn custom<T: fmt::Display>(_msg: T) -> Self {
        Self
    }
}

/// `wanted` is set only for the value `capture` was given: its struct keeps just those fields.
struct Encoder<'a> {
    tape: &'a mut Tape,
    wanted: Option<&'a [&'static str]>,
}

impl<'a> Encoder<'a> {
    fn nested(tape: &'a mut Tape) -> Self {
        Self { tape, wanted: None }
    }

    fn compound(self, closers: u8) -> Compound<'a> {
        Compound { tape: self.tape, wanted: None, closers }
    }
}

macro_rules! signed {
    ($($name:ident: $ty:ty),*) => {
        $(fn $name(self, v: $ty) -> Result<(), EncodeError> {
            self.tape.put_signed(i64::from(v));
            Ok(())
        })*
    };
}

macro_rules! unsigned {
    ($($name:ident: $ty:ty),*) => {
        $(fn $name(self, v: $ty) -> Result<(), EncodeError> {
            self.tape.put_unsigned(u64::from(v));
            Ok(())
        })*
    };
}

impl<'a> ser::Serializer for Encoder<'a> {
    type Ok = ();
    type Error = EncodeError;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    signed!(serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64);
    unsigned!(serialize_u8: u8, serialize_u16: u16, serialize_u32: u32, serialize_u64: u64);

    fn serialize_i128(self, v: i128) -> Result<(), EncodeError> {
        // `serde_json` holds no number wider than 64 bits
        match (u64::try_from(v), i64::try_from(v)) {
            (Ok(u), _) => self.tape.put_unsigned(u),
            (_, Ok(i)) => self.tape.put_signed(i),
            _ => return Err(EncodeError),
        }
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<(), EncodeError> {
        self.tape.put_unsigned(u64::try_from(v).map_err(|_| EncodeError)?);
        Ok(())
    }

    fn serialize_bool(self, v: bool) -> Result<(), EncodeError> {
        self.tape.bytes.push(if v { TRUE } else { FALSE });
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), EncodeError> {
        self.tape.put_float(f64::from(v));
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<(), EncodeError> {
        self.tape.put_float(v);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<(), EncodeError> {
        self.tape.put_string(v.encode_utf8(&mut [0; 4]));
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<(), EncodeError> {
        self.tape.put_string(v);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), EncodeError> {
        // an array of numbers, as `serde_json::Value` holds bytes
        self.tape.bytes.push(ARRAY);
        for b in v {
            self.tape.put_unsigned(u64::from(*b));
        }
        self.tape.bytes.push(END);
        Ok(())
    }

    fn serialize_none(self) -> Result<(), EncodeError> {
        self.tape.bytes.push(NULL);
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), EncodeError> {
        self.tape.bytes.push(NULL);
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), EncodeError> {
        self.tape.bytes.push(NULL);
        Ok(())
    }

    fn serialize_unit_variant(self, _name: &'static str, _index: u32, variant: &'static str) -> Result<(), EncodeError> {
        self.tape.put_string(variant);
        Ok(())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _name: &'static str, value: &T) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _name: &'static str, _index: u32, variant: &'static str, value: &T) -> Result<(), EncodeError> {
        self.tape.bytes.push(OBJECT);
        self.tape.put_string(variant);
        value.serialize(Encoder::nested(&mut *self.tape))?;
        self.tape.bytes.push(END);
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Compound<'a>, EncodeError> {
        self.tape.bytes.push(ARRAY);
        Ok(self.compound(1))
    }

    fn serialize_tuple(self, len: usize) -> Result<Compound<'a>, EncodeError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Compound<'a>, EncodeError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(self, _name: &'static str, _index: u32, variant: &'static str, _len: usize) -> Result<Compound<'a>, EncodeError> {
        self.tape.bytes.push(OBJECT);
        self.tape.put_string(variant);
        self.tape.bytes.push(ARRAY);
        Ok(self.compound(2))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Compound<'a>, EncodeError> {
        self.tape.bytes.push(OBJECT);
        Ok(self.compound(1))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Compound<'a>, EncodeError> {
        match self.wanted {
            // the captured value: its fields are kept apart, so there is no object around them
            Some(wanted) => Ok(Compound { tape: self.tape, wanted: Some(wanted), closers: 0 }),
            None => {
                self.tape.bytes.push(OBJECT);
                Ok(self.compound(1))
            }
        }
    }

    fn serialize_struct_variant(self, _name: &'static str, _index: u32, variant: &'static str, _len: usize) -> Result<Compound<'a>, EncodeError> {
        self.tape.bytes.push(OBJECT);
        self.tape.put_string(variant);
        self.tape.bytes.push(OBJECT);
        Ok(self.compound(2))
    }
}

/// The open array, object or variant being encoded; `closers` is how many `END`s it writes when it ends.
struct Compound<'a> {
    tape: &'a mut Tape,
    wanted: Option<&'a [&'static str]>,
    closers: u8,
}

impl Compound<'_> {
    fn element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        value.serialize(Encoder::nested(&mut *self.tape))
    }

    fn close(self) -> Result<(), EncodeError> {
        for _ in 0..self.closers {
            self.tape.bytes.push(END);
        }
        Ok(())
    }
}

impl ser::SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeTupleStruct for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeTupleVariant for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), EncodeError> {
        self.element(key)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), EncodeError> {
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), EncodeError> {
        match self.wanted {
            Some(wanted) => {
                if wanted.contains(&key) {
                    let start = self.tape.bytes.len();
                    self.element(value)?;
                    let end = self.tape.bytes.len();
                    self.tape.fields.push((key, start, end));
                }
                Ok(())
            }
            None => {
                self.tape.put_string(key);
                self.element(value)
            }
        }
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}

impl ser::SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = EncodeError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), EncodeError> {
        self.tape.put_string(key);
        self.element(value)
    }
    fn end(self) -> Result<(), EncodeError> {
        self.close()
    }
}
