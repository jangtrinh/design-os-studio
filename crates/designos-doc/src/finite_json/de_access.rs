//! The visitor and access wrappers behind `Lenient`: every nested value a visitor pulls out of a
//! sequence, map or enum is read through the lenient deserializer too.

use super::de::{Lenient, Seed};
use super::neutral_for;
use serde::de::{DeserializeSeed, Deserializer, EnumAccess, Error, MapAccess, SeqAccess, VariantAccess, Visitor};
use std::cell::Cell;
use std::fmt;

/// Forwards each `visit_*` call; hands the nested deserializers and accesses to the real visitor
/// wrapped, so numbers at any depth are read leniently.
pub(super) struct Wrap<'a, V> {
    visitor: V,
    replaced: &'a Cell<usize>,
    neutral: f64,
    key_note: Option<&'a Cell<f64>>,
    /// The visitor is a struct's (or struct variant's): only then do its keys name fields whose
    /// neutral value may differ from zero. A map's keys are data, and its values always read zero,
    /// which is what the writer gives a map value too.
    struct_fields: bool,
    strict: bool,
}

impl<'a, V> Wrap<'a, V> {
    pub(super) fn slot(visitor: V, replaced: &'a Cell<usize>, neutral: f64, key_note: Option<&'a Cell<f64>>, strict: bool) -> Self {
        Self { visitor, replaced, neutral, key_note, struct_fields: false, strict }
    }

    pub(super) fn for_struct(mut self) -> Self {
        self.struct_fields = true;
        self
    }

    fn nested(visitor: V, replaced: &'a Cell<usize>, strict: bool) -> Self {
        Self::slot(visitor, replaced, 0.0, None, strict)
    }

    /// A struct key was read: remember the neutral value of the field it names.
    fn note_key(&self, key: &str) {
        if let Some(note) = self.key_note {
            note.set(neutral_for(key));
        }
    }
}

macro_rules! forward_visit {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method<E: Error>(self, v: $ty) -> Result<Self::Value, E> { self.visitor.$method(v) })*
    };
}

impl<'de, V: Visitor<'de>> Visitor<'de> for Wrap<'_, V> {
    type Value = V::Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.visitor.expecting(f)
    }

    forward_visit!(
        visit_bool(bool),
        visit_i8(i8),
        visit_i16(i16),
        visit_i32(i32),
        visit_i64(i64),
        visit_i128(i128),
        visit_u8(u8),
        visit_u16(u16),
        visit_u32(u32),
        visit_u64(u64),
        visit_u128(u128),
        visit_f32(f32),
        visit_f64(f64),
        visit_char(char),
        visit_bytes(&[u8]),
        visit_borrowed_bytes(&'de [u8]),
        visit_byte_buf(Vec<u8>),
    );

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        self.note_key(v);
        self.visitor.visit_str(v)
    }
    fn visit_borrowed_str<E: Error>(self, v: &'de str) -> Result<Self::Value, E> {
        self.note_key(v);
        self.visitor.visit_borrowed_str(v)
    }
    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        self.note_key(&v);
        self.visitor.visit_string(v)
    }
    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        self.visitor.visit_none()
    }
    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        self.visitor.visit_unit()
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.visitor.visit_some(Lenient::slot(deserializer, self.replaced, self.neutral, None, self.strict))
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.visitor.visit_newtype_struct(Lenient::slot(deserializer, self.replaced, self.neutral, None, self.strict))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        self.visitor.visit_seq(Items { inner: seq, replaced: self.replaced, strict: self.strict })
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        self.visitor.visit_map(Entries {
            inner: map,
            replaced: self.replaced,
            field_neutral: Cell::new(0.0),
            struct_fields: self.struct_fields,
            strict: self.strict,
        })
    }
    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
        self.visitor.visit_enum(Variants { inner: data, replaced: self.replaced, strict: self.strict })
    }
}

struct Items<'a, A> {
    inner: A,
    replaced: &'a Cell<usize>,
    strict: bool,
}

impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for Items<'_, A> {
    type Error = A::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error> {
        self.inner.next_element_seed(Seed { seed, replaced: self.replaced, neutral: 0.0, key_note: None, strict: self.strict })
    }
    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct Entries<'a, A> {
    inner: A,
    replaced: &'a Cell<usize>,
    /// Neutral value of the field whose key was read last, taken by the value that follows.
    field_neutral: Cell<f64>,
    struct_fields: bool,
    strict: bool,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Entries<'_, A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error> {
        let key_note = self.struct_fields.then_some(&self.field_neutral);
        self.inner.next_key_seed(Seed { seed, replaced: self.replaced, neutral: 0.0, key_note, strict: self.strict })
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Self::Error> {
        let neutral = self.field_neutral.replace(0.0);
        self.inner.next_value_seed(Seed { seed, replaced: self.replaced, neutral, key_note: None, strict: self.strict })
    }
    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct Variants<'a, A> {
    inner: A,
    replaced: &'a Cell<usize>,
    strict: bool,
}

impl<'de, 'a, A: EnumAccess<'de>> EnumAccess<'de> for Variants<'a, A> {
    type Error = A::Error;
    type Variant = Variant<'a, A::Variant>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error> {
        let (value, variant) = self.inner.variant_seed(Seed { seed, replaced: self.replaced, neutral: 0.0, key_note: None, strict: self.strict })?;
        Ok((value, Variant { inner: variant, replaced: self.replaced, strict: self.strict }))
    }
}

struct Variant<'a, A> {
    inner: A,
    replaced: &'a Cell<usize>,
    strict: bool,
}

impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for Variant<'_, A> {
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        self.inner.unit_variant()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Self::Error> {
        self.inner.newtype_variant_seed(Seed { seed, replaced: self.replaced, neutral: 0.0, key_note: None, strict: self.strict })
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> {
        self.inner.tuple_variant(len, Wrap::nested(visitor, self.replaced, self.strict))
    }
    fn struct_variant<V: Visitor<'de>>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> {
        let wrapped = Wrap::nested(visitor, self.replaced, self.strict);
        self.inner.struct_variant(fields, if self.strict { wrapped } else { wrapped.for_struct() })
    }
}
