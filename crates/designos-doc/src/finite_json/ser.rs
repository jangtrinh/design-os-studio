//! Serializer wrapper: forwards everything to the inner serializer, except that a non-finite
//! float is replaced by the neutral value of the field that holds it.

use super::neutral_for;
use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::cell::Cell;

/// Wraps the serializer of one value.
pub(super) struct Finite<'a, S> {
    inner: S,
    replaced: &'a Cell<usize>,
    /// What a non-finite number becomes here (see [`neutral_for`]).
    neutral: f64,
    /// The value sits inside an `Option`: a non-finite number is left to serialize as `null`
    /// (read back as `None`) instead of being replaced by a number.
    optional: bool,
}

impl<'a, S> Finite<'a, S> {
    pub(super) fn new(inner: S, replaced: &'a Cell<usize>) -> Self {
        Self { inner, replaced, neutral: 0.0, optional: false }
    }
}

/// `Serialize` adapter that hands the inner serializer our wrapper for one nested value.
struct Via<'a, T: ?Sized> {
    value: &'a T,
    replaced: &'a Cell<usize>,
    neutral: f64,
    optional: bool,
}

impl<'a, T: ?Sized> Via<'a, T> {
    fn plain(value: &'a T, replaced: &'a Cell<usize>) -> Self {
        Self { value, replaced, neutral: 0.0, optional: false }
    }
}

impl<T: Serialize + ?Sized> Serialize for Via<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(Finite { inner: serializer, replaced: self.replaced, neutral: self.neutral, optional: self.optional })
    }
}

/// Wrapper for every compound serializer (sequence, tuple, map, struct, ...): each element goes
/// through [`Via`], so numbers at any depth are checked.
pub(super) struct Compound<'a, C> {
    inner: C,
    replaced: &'a Cell<usize>,
}

macro_rules! forward_scalar {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method(self, v: $ty) -> Result<Self::Ok, Self::Error> { self.inner.$method(v) })*
    };
}

macro_rules! serialize_float {
    ($method:ident, $ty:ty) => {
        fn $method(self, v: $ty) -> Result<Self::Ok, Self::Error> {
            if v.is_finite() || self.optional {
                return self.inner.$method(v);
            }
            self.replaced.set(self.replaced.get() + 1);
            self.inner.$method(self.neutral as $ty)
        }
    };
}

impl<'a, S: Serializer> Serializer for Finite<'a, S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Compound<'a, S::SerializeSeq>;
    type SerializeTuple = Compound<'a, S::SerializeTuple>;
    type SerializeTupleStruct = Compound<'a, S::SerializeTupleStruct>;
    type SerializeTupleVariant = Compound<'a, S::SerializeTupleVariant>;
    type SerializeMap = Compound<'a, S::SerializeMap>;
    type SerializeStruct = Compound<'a, S::SerializeStruct>;
    type SerializeStructVariant = Compound<'a, S::SerializeStructVariant>;

    forward_scalar!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_char(char),
        serialize_str(&str),
        serialize_bytes(&[u8]),
    );
    serialize_float!(serialize_f32, f32);
    serialize_float!(serialize_f64, f64);

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_some(&Via { value, replaced: self.replaced, neutral: self.neutral, optional: true })
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }
    fn serialize_unit_variant(self, name: &'static str, index: u32, variant: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_variant(name, index, variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, name: &'static str, value: &T) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_newtype_struct(name, &Via { value, replaced: self.replaced, neutral: self.neutral, optional: self.optional })
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_newtype_variant(name, index, variant, &Via::plain(value, self.replaced))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_seq(len)?, replaced: self.replaced })
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_tuple(len)?, replaced: self.replaced })
    }
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_tuple_struct(name, len)?, replaced: self.replaced })
    }
    fn serialize_tuple_variant(self, name: &'static str, index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_tuple_variant(name, index, variant, len)?, replaced: self.replaced })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_map(len)?, replaced: self.replaced })
    }
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_struct(name, len)?, replaced: self.replaced })
    }
    fn serialize_struct_variant(self, name: &'static str, index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(Compound { inner: self.inner.serialize_struct_variant(name, index, variant, len)?, replaced: self.replaced })
    }
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

impl<C: SerializeSeq> SerializeSeq for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&Via::plain(value, self.replaced))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTuple> SerializeTuple for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&Via::plain(value, self.replaced))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleStruct> SerializeTupleStruct for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&Via::plain(value, self.replaced))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleVariant> SerializeTupleVariant for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&Via::plain(value, self.replaced))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeMap> SerializeMap for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.inner.serialize_key(&Via::plain(key, self.replaced))
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_value(&Via::plain(value, self.replaced))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

/// A struct field is the one place the field name is known, so it picks the neutral value.
fn field<'a, T: ?Sized>(value: &'a T, name: &str, replaced: &'a Cell<usize>) -> Via<'a, T> {
    Via { value, replaced, neutral: neutral_for(name), optional: false }
}

impl<C: SerializeStruct> SerializeStruct for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(key, &field(value, key, self.replaced))
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeStructVariant> SerializeStructVariant for Compound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(key, &field(value, key, self.replaced))
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}
