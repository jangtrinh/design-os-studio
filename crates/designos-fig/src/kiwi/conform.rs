//! Rewriting a decoded value as an instance of a definition of another schema.
//!
//! A `.fig` carries its own schema, and Studio writes with its own. A value decoded with the first is rewritten against
//! the second by field name, so it can be encoded with it, and so that what is kept of a file does not depend on the
//! numbering of the schema that wrote it.

use super::{DefKind, Field, FieldType, KiwiValue, Native, Schema, MAX_DEPTH};

impl Schema {
    /// Whether definition `idx` of this schema is definition `other_idx` of `other`: the same kind, and at every depth the
    /// same fields, by name, type, array flag and number. Bytes written with one then read the same with the other.
    pub fn same_definition(&self, idx: usize, other: &Schema, other_idx: usize) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.same_definition_at(idx, other, other_idx, &mut seen)
    }

    fn same_definition_at(&self, idx: usize, other: &Schema, other_idx: usize, seen: &mut std::collections::HashSet<(usize, usize)>) -> bool {
        if !seen.insert((idx, other_idx)) {
            return true;
        }
        let (Some(a), Some(b)) = (self.defs.get(idx), other.defs.get(other_idx)) else { return false };
        a.kind == b.kind
            && a.fields.len() == b.fields.len()
            && a.fields.iter().zip(&b.fields).all(|(x, y)| {
                x.name == y.name
                    && x.is_array == y.is_array
                    && x.value == y.value
                    && match (x.ty, y.ty) {
                        (FieldType::Native(p), FieldType::Native(q)) => p == q,
                        // the variants of an enum are fields that point nowhere, whatever `Def` index they carry
                        (FieldType::Def(_), FieldType::Def(_)) if a.kind == DefKind::Enum => true,
                        (FieldType::Def(p), FieldType::Def(q)) => self.same_definition_at(p, other, q, seen),
                        _ => false,
                    }
            })
    }

    /// `v` as an instance of definition `idx`, matched by name. `None` when it cannot be written whole, so the caller
    /// never keeps a shortened value:
    /// - a message field the definition does not name is left out (a file written by a newer Figma may hold more);
    /// - a struct field the value lacks is written as zero, as [`Schema::def_field_from_json`] does;
    /// - an enum name the definition does not have fails the conversion (an unnamed number is carried as it is);
    /// - a number goes into a field of another width only when its value fits.
    pub fn conform_def(&self, idx: usize, v: &KiwiValue) -> Option<KiwiValue> {
        self.conform_def_at(idx, v, 0)
    }

    fn conform_def_at(&self, idx: usize, v: &KiwiValue, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        let def = self.defs.get(idx)?;
        match (def.kind, v) {
            (DefKind::Enum, KiwiValue::Enum(name)) => def.fields.iter().find(|f| f.name == *name).map(|f| KiwiValue::Enum(f.name.clone())),
            (DefKind::Enum, KiwiValue::EnumRaw(n)) => {
                Some(def.fields.iter().find(|f| f.value == *n).map_or(KiwiValue::EnumRaw(*n), |f| KiwiValue::Enum(f.name.clone())))
            }
            (DefKind::Struct, KiwiValue::Struct(have) | KiwiValue::Message(have)) => {
                let mut out = Vec::with_capacity(def.fields.len());
                for f in &def.fields {
                    let value = match have.iter().find(|(n, _)| *n == f.name) {
                        Some((_, fv)) => self.conform_field(f, fv, depth + 1)?,
                        None => self.struct_field_default(&def.name, f, depth + 1)?,
                    };
                    out.push((f.name.clone(), value));
                }
                Some(KiwiValue::Struct(out))
            }
            (DefKind::Message, KiwiValue::Message(have) | KiwiValue::Struct(have)) => {
                let mut out = Vec::with_capacity(have.len());
                for (name, fv) in have {
                    let Some(f) = def.fields.iter().find(|f| f.name == *name) else { continue };
                    out.push((f.name.clone(), self.conform_field(f, fv, depth + 1)?));
                }
                Some(KiwiValue::Message(out))
            }
            _ => None,
        }
    }

    fn conform_field(&self, f: &Field, v: &KiwiValue, depth: usize) -> Option<KiwiValue> {
        if !f.is_array {
            return self.conform_type(f.ty, v, depth + 1);
        }
        match (f.ty, v) {
            (FieldType::Native(Native::Byte), KiwiValue::Bytes(b)) => Some(KiwiValue::Bytes(b.clone())),
            (FieldType::Native(Native::Byte), _) => None,
            (ty, KiwiValue::Array(items)) => items.iter().map(|item| self.conform_type(ty, item, depth + 1)).collect::<Option<Vec<_>>>().map(KiwiValue::Array),
            _ => None,
        }
    }

    fn conform_type(&self, ty: FieldType, v: &KiwiValue, depth: usize) -> Option<KiwiValue> {
        use KiwiValue as V;
        let int = |v: &V| -> Option<i128> {
            match v {
                V::Byte(n) => Some(i128::from(*n)),
                V::Int(n) => Some(i128::from(*n)),
                V::Uint(n) => Some(i128::from(*n)),
                V::Int64(n) => Some(i128::from(*n)),
                V::Uint64(n) => Some(i128::from(*n)),
                _ => None,
            }
        };
        match ty {
            FieldType::Native(Native::Bool) => matches!(v, V::Bool(_)).then(|| v.clone()),
            FieldType::Native(Native::String) => matches!(v, V::String(_)).then(|| v.clone()),
            FieldType::Native(Native::Float) => match v {
                V::Float(_) => Some(v.clone()),
                other => int(other).map(|n| V::Float(n as f32)),
            },
            FieldType::Native(Native::Byte) => int(v).and_then(|n| u8::try_from(n).ok()).map(V::Byte),
            FieldType::Native(Native::Int) => int(v).and_then(|n| i32::try_from(n).ok()).map(V::Int),
            FieldType::Native(Native::Uint) => int(v).and_then(|n| u32::try_from(n).ok()).map(V::Uint),
            FieldType::Native(Native::Int64) => int(v).and_then(|n| i64::try_from(n).ok()).map(V::Int64),
            FieldType::Native(Native::Uint64) => int(v).and_then(|n| u64::try_from(n).ok()).map(V::Uint64),
            FieldType::Def(idx) => self.conform_def_at(idx, v, depth + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DefKind, FieldType, KiwiValue as V, Native, Schema, SchemaBuilder};

    /// `Item { name, pt, tags, color, raw }` over `Pt { x, y, z }` and `Color { RED, BLUE }`.
    fn schema() -> (Schema, usize) {
        let mut b = SchemaBuilder::new();
        let color = b.define("Color", DefKind::Enum);
        b.variant(color, "RED", 0);
        b.variant(color, "BLUE", 1);
        let pt = b.define("Pt", DefKind::Struct);
        b.field(pt, "x", FieldType::Native(Native::Float), false, 1);
        b.field(pt, "y", FieldType::Native(Native::Float), false, 2);
        b.field(pt, "z", FieldType::Native(Native::Uint), false, 3);
        let item = b.define("Item", DefKind::Message);
        b.field(item, "name", FieldType::Native(Native::String), false, 1);
        b.field(item, "pt", FieldType::Def(pt), false, 2);
        b.field(item, "tags", FieldType::Native(Native::Uint), true, 3);
        b.field(item, "color", FieldType::Def(color), false, 4);
        b.field(item, "raw", FieldType::Native(Native::Byte), true, 5);
        (b.build(), item)
    }

    fn msg(fields: Vec<(&str, V)>) -> V {
        V::Message(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    #[test]
    fn a_message_keeps_the_order_of_its_value_and_leaves_out_fields_the_definition_does_not_name() {
        let (s, item) = schema();
        let value = msg(vec![("color", V::Enum("BLUE".into())), ("future", V::Uint(9)), ("name", V::String("a".into()))]);
        let out = s.conform_def(item, &value).unwrap();
        assert_eq!(out, msg(vec![("color", V::Enum("BLUE".into())), ("name", V::String("a".into()))]), "Figma's order, and no field the schema lacks");
    }

    #[test]
    fn a_struct_is_written_in_the_definition_s_order_and_a_missing_field_is_zero() {
        let (s, item) = schema();
        let pt = V::Struct(vec![("y".into(), V::Float(2.0)), ("x".into(), V::Float(1.0)), ("extra".into(), V::Uint(1))]);
        let out = s.conform_def(item, &msg(vec![("pt", pt)])).unwrap();
        let expected = V::Struct(vec![("x".into(), V::Float(1.0)), ("y".into(), V::Float(2.0)), ("z".into(), V::Uint(0))]);
        assert_eq!(out, msg(vec![("pt", expected)]));
    }

    #[test]
    fn an_enum_name_the_definition_lacks_fails_and_an_unnamed_number_is_carried() {
        let (s, item) = schema();
        assert!(s.conform_def(item, &msg(vec![("color", V::Enum("GREEN".into()))])).is_none(), "never a shortened value");
        assert_eq!(
            s.conform_def(item, &msg(vec![("color", V::EnumRaw(1))])).unwrap(),
            msg(vec![("color", V::Enum("BLUE".into()))]),
            "a number the schema names is named"
        );
        assert_eq!(s.conform_def(item, &msg(vec![("color", V::EnumRaw(7))])).unwrap(), msg(vec![("color", V::EnumRaw(7))]));
    }

    #[test]
    fn a_number_goes_into_a_field_of_another_width_only_when_it_fits() {
        let (s, item) = schema();
        let tags = |v: Vec<V>| msg(vec![("tags", V::Array(v))]);
        assert_eq!(s.conform_def(item, &tags(vec![V::Int(5), V::Uint64(7)])).unwrap(), tags(vec![V::Uint(5), V::Uint(7)]));
        assert!(s.conform_def(item, &tags(vec![V::Int(-1)])).is_none());
        assert!(s.conform_def(item, &tags(vec![V::Uint64(u64::MAX)])).is_none());
        assert!(s.conform_def(item, &msg(vec![("name", V::Uint(1))])).is_none(), "a string field takes a string");
        assert_eq!(s.conform_def(item, &msg(vec![("raw", V::Bytes(vec![1, 2]))])).unwrap(), msg(vec![("raw", V::Bytes(vec![1, 2]))]));
        assert!(s.conform_def(item, &msg(vec![("raw", V::Array(vec![]))])).is_none());
    }

    #[test]
    fn the_result_encodes_and_decodes_to_itself() {
        let (s, item) = schema();
        let value = msg(vec![
            ("name", V::String("n".into())),
            ("pt", V::Struct(vec![("x".into(), V::Float(0.5)), ("y".into(), V::Float(-2.0)), ("z".into(), V::Uint(3))])),
            ("tags", V::Array(vec![V::Uint(1), V::Uint(2)])),
            ("color", V::Enum("RED".into())),
        ]);
        let conformed = s.conform_def(item, &value).unwrap();
        assert_eq!(conformed, value);
        assert_eq!(s.decode_message(item, &s.encode_message(item, &conformed).unwrap()).unwrap(), value);
    }
}
