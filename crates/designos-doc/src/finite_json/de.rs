//! Deserializer wrapper: forwards everything to the inner deserializer, except that a `null`
//! where an `f32` / `f64` is expected becomes the neutral value of the field instead of an error.
//! `Option<f32>` is untouched: `deserialize_option` is forwarded, so `null` there stays `None`.

use super::de_access::Wrap;
use serde::de::{DeserializeSeed, Deserializer, Visitor};
use std::cell::Cell;
use std::fmt;

/// Wraps the deserializer of one value.
///
/// When `strict` is `true`, a `null` in an `f32` / `f64` slot still fails with `invalid_type`,
/// after incrementing `replaced` so [`super::from_str_repairing`] knows structurally that a `null`
/// number caused the failure without inspecting error text.
pub(super) struct Lenient<'a, D> {
    inner: D,
    replaced: &'a Cell<usize>,
    /// What a `null` number becomes here (see `neutral_for`).
    neutral: f64,
    /// Set on the deserializer of a map key: the key text is written here, so the value that
    /// follows knows the neutral value of its field.
    key_note: Option<&'a Cell<f64>>,
    strict: bool,
}

impl<'a, D> Lenient<'a, D> {
    pub(super) fn new(inner: D, replaced: &'a Cell<usize>) -> Self {
        Self { inner, replaced, neutral: 0.0, key_note: None, strict: false }
    }

    pub(super) fn strict(inner: D, replaced: &'a Cell<usize>) -> Self {
        Self { inner, replaced, neutral: 0.0, key_note: None, strict: true }
    }

    pub(super) fn slot(inner: D, replaced: &'a Cell<usize>, neutral: f64, key_note: Option<&'a Cell<f64>>, strict: bool) -> Self {
        Self { inner, replaced, neutral, key_note, strict }
    }

    fn wrap<V>(&self, visitor: V) -> Wrap<'a, V> {
        Wrap::slot(visitor, self.replaced, self.neutral, self.key_note, self.strict)
    }
}

macro_rules! forward_plain {
    ($($method:ident),* $(,)?) => {
        $(fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            let wrapped = self.wrap(visitor);
            self.inner.$method(wrapped)
        })*
    };
}

macro_rules! forward_direct {
    ($($method:ident),* $(,)?) => {
        $(fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.inner.$method(visitor)
        })*
    };
}

macro_rules! forward_str {
    ($($method:ident),* $(,)?) => {
        $(fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            if self.key_note.is_some() {
                let wrapped = self.wrap(visitor);
                self.inner.$method(wrapped)
            } else {
                self.inner.$method(visitor)
            }
        })*
    };
}

macro_rules! deserialize_float {
    ($method:ident, $double:expr) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.inner.deserialize_any(NullAsNeutral { visitor, replaced: self.replaced, neutral: self.neutral, double: $double, strict: self.strict })
        }
    };
}

impl<'de, 'a, D: Deserializer<'de>> Deserializer<'de> for Lenient<'a, D> {
    type Error = D::Error;

    forward_plain!(deserialize_any, deserialize_option, deserialize_seq, deserialize_map,);
    forward_direct!(
        deserialize_bool,
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_char,
        deserialize_bytes,
        deserialize_byte_buf,
        deserialize_unit,
        deserialize_ignored_any,
    );
    forward_str!(deserialize_str, deserialize_string, deserialize_identifier,);
    deserialize_float!(deserialize_f32, false);
    deserialize_float!(deserialize_f64, true);

    fn deserialize_unit_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error> {
        self.inner.deserialize_unit_struct(name, visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = self.wrap(visitor);
        self.inner.deserialize_newtype_struct(name, wrapped)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = self.wrap(visitor);
        self.inner.deserialize_tuple(len, wrapped)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = self.wrap(visitor);
        self.inner.deserialize_tuple_struct(name, len, wrapped)
    }
    fn deserialize_struct<V: Visitor<'de>>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = if self.strict { self.wrap(visitor) } else { self.wrap(visitor).for_struct() };
        self.inner.deserialize_struct(name, fields, wrapped)
    }
    fn deserialize_enum<V: Visitor<'de>>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = self.wrap(visitor);
        self.inner.deserialize_enum(name, variants, wrapped)
    }
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

/// Visitor for a number slot: in lenient mode, `null` becomes the neutral value and is counted; in
/// strict mode, `null` is counted and rejected with `invalid_type(Unexpected::Unit, &self.visitor)`.
/// Anything else goes to the real visitor unchanged, so every other error keeps serde's own wording.
struct NullAsNeutral<'a, V> {
    visitor: V,
    replaced: &'a Cell<usize>,
    neutral: f64,
    double: bool,
    strict: bool,
}

impl<'de, V: Visitor<'de>> Visitor<'de> for NullAsNeutral<'_, V> {
    type Value = V::Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.visitor.expecting(f)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        self.replaced.set(self.replaced.get() + 1);
        if self.strict {
            return Err(E::invalid_type(serde::de::Unexpected::Unit, &self.visitor));
        }
        if self.double {
            self.visitor.visit_f64(self.neutral)
        } else {
            self.visitor.visit_f32(self.neutral as f32)
        }
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        self.visit_unit()
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
        self.visitor.visit_bool(v)
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        self.visitor.visit_i64(v)
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        self.visitor.visit_u64(v)
    }
    fn visit_f32<E: serde::de::Error>(self, v: f32) -> Result<Self::Value, E> {
        self.visitor.visit_f32(v)
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        self.visitor.visit_f64(v)
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        self.visitor.visit_str(v)
    }
}

/// Hands the inner deserializer our wrapper for the value a seed is about to read.
pub(super) struct Seed<'a, T> {
    pub(super) seed: T,
    pub(super) replaced: &'a Cell<usize>,
    pub(super) neutral: f64,
    pub(super) key_note: Option<&'a Cell<f64>>,
    pub(super) strict: bool,
}

impl<'de, T: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<'_, T> {
    type Value = T::Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.seed.deserialize(Lenient::slot(deserializer, self.replaced, self.neutral, self.key_note, self.strict))
    }
}
