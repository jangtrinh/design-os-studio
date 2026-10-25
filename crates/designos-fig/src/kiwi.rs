//! Generic Kiwi (https://github.com/evanw/kiwi) binary schema + message codec.
//!
//! Decoding is fully dynamic: the schema embedded in the `.fig` file drives the
//! decoder, producing a [`KiwiValue`] tree. A matching encoder is provided so
//! tests (and a future `.fig` writer) can synthesize files.

use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum KiwiError {
    #[error("unexpected end of data at offset {0}")]
    Eof(usize),
    #[error("malformed data: {0}")]
    Bad(String),
}

type Result<T> = std::result::Result<T, KiwiError>;

const MAX_DEPTH: usize = 96;
/// Absolute cap on values one message may decode (every native, enum, struct, message and array element counts one).
///
/// It is independent of the input length on purpose: zero-byte elements and zero padding inflate to far more
/// values than the input holds, so any length-scaled allowance is attacker-controlled.
/// Sizing: a measured real file (`VSF - PCP.fig`: 1,058,895 Kiwi `nodeChanges` records across 31 pages -> 1,082,324
/// expanded scene nodes, or 723,136 Kiwi records -> 112,474 scene nodes on the 7-page subset) decodes 54,870,017
/// values, i.e. ~52 per record (the figure in `KIWI_VALUES_PER_NODE`). 1.06M records therefore need ~55M; the cap is
/// 2^28 = 268M, about 4.9x that.
const MAX_DECODED_VALUES: usize = 1 << 28;
const MIN_ZERO_WIRE_INSTANCES: usize = 1 << 16;
const MAX_ZERO_WIRE_INSTANCES: usize = 1 << 24;
/// Cap on decoded instances of zero-wire-size definitions (see `Schema::zero_wire_defs`) per message.
///
/// A struct whose fields are all (nested) empty structs encodes to no bytes at all, so unlike every other value it
/// is free for an attacker. Scaling with `input_len` (`(input_len * 16).clamp(65_536, 1 << 24)`) keeps a tiny hostile
/// payload bounded (<= 65,536 for <= 4 KiB, 655,360 for 40 KiB) while letting legitimate multi-megabyte files with
/// many empty marker structs decode up to `1 << 24` instances.
fn zero_wire_budget(input_len: usize) -> usize {
    input_len.saturating_mul(16).clamp(MIN_ZERO_WIRE_INSTANCES, MAX_ZERO_WIRE_INSTANCES)
}
/// Values per node measured on a real `.fig` (see [`MAX_DECODED_VALUES`]).
#[cfg(test)]
const KIWI_VALUES_PER_NODE: usize = 52;
/// Vectors reserve at most this many slots up front; they grow only as elements are really decoded (and charged).
const MAX_PREALLOC: usize = 1024;

fn bounded_capacity(claimed: usize) -> usize {
    claimed.min(MAX_PREALLOC)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Enum,
    Struct,
    Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Native {
    Bool,
    Byte,
    Int,
    Uint,
    Float,
    String,
    Int64,
    Uint64,
}

impl Native {
    fn from_id(id: i64) -> Option<Native> {
        Some(match id {
            -1 => Native::Bool,
            -2 => Native::Byte,
            -3 => Native::Int,
            -4 => Native::Uint,
            -5 => Native::Float,
            -6 => Native::String,
            -7 => Native::Int64,
            -8 => Native::Uint64,
            _ => return None,
        })
    }
    fn id(self) -> i64 {
        match self {
            Native::Bool => -1,
            Native::Byte => -2,
            Native::Int => -3,
            Native::Uint => -4,
            Native::Float => -5,
            Native::String => -6,
            Native::Int64 => -7,
            Native::Uint64 => -8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Native(Native),
    /// Index into [`Schema::defs`].
    Def(usize),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Arc<str>,
    pub ty: FieldType,
    pub is_array: bool,
    /// Message: field id. Enum: variant value. Struct: ordinal (unused).
    pub value: u32,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub name: String,
    pub kind: DefKind,
    pub fields: Vec<Field>,
    pub field_by_value: Vec<Option<usize>>,
}

fn build_field_by_value(fields: &[Field]) -> Vec<Option<usize>> {
    let Some(max_val) = fields.iter().map(|f| f.value).max() else {
        return Vec::new();
    };
    if max_val > 4096 {
        return Vec::new();
    }
    let mut lookup = vec![None; (max_val as usize) + 1];
    for (i, f) in fields.iter().enumerate() {
        let idx = f.value as usize;
        if lookup[idx].is_none() {
            lookup[idx] = Some(i);
        }
    }
    lookup
}

#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub defs: Vec<Def>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KiwiValue {
    Bool(bool),
    Byte(u8),
    Int(i32),
    Uint(u32),
    Float(f32),
    String(String),
    Int64(i64),
    Uint64(u64),
    /// `byte[]` (image hashes, blobs...).
    Bytes(Vec<u8>),
    Enum(Arc<str>),
    /// Enum value without a known name.
    EnumRaw(u32),
    Array(Vec<KiwiValue>),
    Struct(Vec<(Arc<str>, KiwiValue)>),
    Message(Vec<(Arc<str>, KiwiValue)>),
}

impl KiwiValue {
    pub fn get(&self, name: &str) -> Option<&KiwiValue> {
        match self {
            KiwiValue::Struct(f) | KiwiValue::Message(f) => f.iter().find(|(n, _)| &**n == name).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut KiwiValue> {
        match self {
            KiwiValue::Struct(f) | KiwiValue::Message(f) => f.iter_mut().find(|(n, _)| &**n == name).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn take_field(&mut self, name: &str) -> Option<KiwiValue> {
        match self {
            KiwiValue::Struct(f) | KiwiValue::Message(f) => {
                let idx = f.iter().position(|(n, _)| &**n == name)?;
                Some(f.swap_remove(idx).1)
            }
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[KiwiValue]> {
        match self {
            KiwiValue::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            KiwiValue::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- reader

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    values_left: usize,
    /// Per definition: decoding it consumes no input bytes (empty for a reader not driven by a schema).
    zero_wire: Vec<bool>,
    zero_wire_left: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, values_left: usize::MAX, zero_wire: Vec::new(), zero_wire_left: usize::MAX }
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    fn charge(&mut self) -> Result<()> {
        self.values_left = self.values_left.checked_sub(1).ok_or_else(|| KiwiError::Bad("decoded value count exceeds budget".into()))?;
        Ok(())
    }
    fn charge_zero_wire(&mut self) -> Result<()> {
        self.zero_wire_left = self.zero_wire_left.checked_sub(1).ok_or_else(|| KiwiError::Bad("zero-size struct count exceeds budget".into()))?;
        Ok(())
    }
    pub fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }
    pub fn byte(&mut self) -> Result<u8> {
        let b = *self.data.get(self.pos).ok_or(KiwiError::Eof(self.pos))?;
        self.pos += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(KiwiError::Eof(self.pos));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn var_u64(&mut self) -> Result<u64> {
        let mut v: u64 = 0;
        let mut shift = 0u32;
        loop {
            let b = self.byte()?;
            if shift < 64 {
                v |= ((b & 0x7f) as u64) << shift;
            }
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 70 {
                return Err(KiwiError::Bad("varint too long".into()));
            }
        }
    }
    pub fn var_u32(&mut self) -> Result<u32> {
        Ok(self.var_u64()? as u32)
    }
    pub fn var_i32(&mut self) -> Result<i32> {
        let v = self.var_u32()?;
        Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
    }
    pub fn var_i64(&mut self) -> Result<i64> {
        let v = self.var_u64()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }
    pub fn var_f32(&mut self) -> Result<f32> {
        let first = *self.data.get(self.pos).ok_or(KiwiError::Eof(self.pos))?;
        if first == 0 {
            self.pos += 1;
            return Ok(0.0);
        }
        let b = self.take(4)?;
        let bits = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        Ok(f32::from_bits(bits.rotate_right(9)))
    }
    pub fn string(&mut self) -> Result<String> {
        let rest = &self.data[self.pos..];
        let n = rest.iter().position(|&b| b == 0).ok_or(KiwiError::Eof(self.data.len()))?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.pos += n + 1;
        Ok(s)
    }
    pub fn skip_string(&mut self) -> Result<()> {
        let rest = &self.data[self.pos..];
        let n = rest.iter().position(|&b| b == 0).ok_or(KiwiError::Eof(self.data.len()))?;
        self.pos += n + 1;
        Ok(())
    }
}

// ---------------------------------------------------------------- schema

/// Names the field an encode error happened in, innermost first (`NodeChange.fillPaints: Paint.color: ...`).
fn in_field(def: &str, field: &str, e: KiwiError) -> KiwiError {
    match e {
        KiwiError::Bad(m) => KiwiError::Bad(format!("{def}.{field}: {m}")),
        other => other,
    }
}

impl Schema {
    /// Decodes the binary schema (`encodeBinarySchema` in kiwi).
    pub fn decode(data: &[u8]) -> Result<Schema> {
        let mut r = Reader::new(data);
        let count = r.var_u32()? as usize;
        if count > r.remaining() {
            return Err(KiwiError::Bad("definition count exceeds data".into()));
        }
        let mut defs = Vec::with_capacity(bounded_capacity(count));
        for _ in 0..count {
            let name = r.string()?;
            let kind = match r.byte()? {
                0 => DefKind::Enum,
                1 => DefKind::Struct,
                2 => DefKind::Message,
                k => return Err(KiwiError::Bad(format!("bad definition kind {k}"))),
            };
            let fc = r.var_u32()? as usize;
            if fc > r.remaining() {
                return Err(KiwiError::Bad("field count exceeds data".into()));
            }
            let mut fields = Vec::with_capacity(bounded_capacity(fc));
            for _ in 0..fc {
                let fname = r.string()?;
                let tid = r.var_i32()? as i64;
                let is_array = r.byte()? != 0;
                let value = r.var_u32()?;
                let ty = if tid < 0 {
                    // Enums carry type 0 with no meaning; natives are negative.
                    FieldType::Native(Native::from_id(tid).ok_or_else(|| KiwiError::Bad(format!("bad native type {tid}")))?)
                } else {
                    FieldType::Def(tid as usize)
                };
                fields.push(Field { name: fname.into(), ty, is_array, value });
            }
            let field_by_value = build_field_by_value(&fields);
            defs.push(Def { name, kind, fields, field_by_value });
        }
        let schema = Schema { defs };
        for d in &schema.defs {
            for f in &d.fields {
                if let FieldType::Def(i) = f.ty {
                    if i >= schema.defs.len() {
                        return Err(KiwiError::Bad(format!("type index {i} out of range")));
                    }
                }
            }
        }
        Ok(schema)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.var_u64(self.defs.len() as u64);
        for d in &self.defs {
            w.string(&d.name);
            w.byte(match d.kind {
                DefKind::Enum => 0,
                DefKind::Struct => 1,
                DefKind::Message => 2,
            });
            w.var_u64(d.fields.len() as u64);
            for f in &d.fields {
                w.string(&f.name);
                w.var_i64(match f.ty {
                    FieldType::Native(n) => n.id(),
                    FieldType::Def(i) => i as i64,
                });
                w.byte(f.is_array as u8);
                w.var_u64(f.value as u64);
            }
        }
        w.buf
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.defs.iter().position(|d| d.name == name)
    }

    /// Decodes a root message of definition `root`.
    pub fn decode_message(&self, root: usize, data: &[u8]) -> Result<KiwiValue> {
        self.decode_message_with_limits(root, data, MAX_DECODED_VALUES, zero_wire_budget(data.len()), false)
    }

    /// Only the value budget applies; zero-size instances are unlimited (tests the value budget in isolation).
    #[cfg(test)]
    fn decode_message_with_budget(&self, root: usize, data: &[u8], budget: usize) -> Result<KiwiValue> {
        self.decode_message_with_limits(root, data, budget, usize::MAX, false)
    }

    fn decode_message_with_limits(&self, root: usize, data: &[u8], values: usize, zero_wire_instances: usize, skip_heavy: bool) -> Result<KiwiValue> {
        let mut r = Reader::new(data);
        r.values_left = values;
        r.zero_wire = self.zero_wire_defs();
        r.zero_wire_left = zero_wire_instances;
        r.charge()?;
        self.decode_def(root, &mut r, 0, skip_heavy)
    }

    /// Decodes a root message for `.fig` import, dropping unused multi-gigabyte
    /// derived fields (`editInfo`, `derivedTextData`, `derivedImmutableFrameData`)
    /// on the fly when the message payload exceeds 2 MB. The same decode budgets apply.
    pub fn decode_message_for_import(&self, root: usize, data: &[u8]) -> Result<KiwiValue> {
        self.decode_message_for_import_with_values(root, data, MAX_DECODED_VALUES)
    }

    /// [`Self::decode_message_for_import`] with a caller-chosen cap on decoded values (untrusted payloads smaller
    /// than a whole file, such as a clipboard, use a far lower cap than [`MAX_DECODED_VALUES`]).
    pub fn decode_message_for_import_with_values(&self, root: usize, data: &[u8], values: usize) -> Result<KiwiValue> {
        let skip_heavy = data.len() > 2_000_000;
        self.decode_message_with_limits(root, data, values, zero_wire_budget(data.len()), skip_heavy)
    }

    /// Per definition: whether one instance occupies zero bytes on the wire. Enums (a varint), messages (a terminator)
    /// and arrays (a length) always take bytes; a struct takes none only if every field is a non-array zero-size struct.
    /// A cyclic struct can never finish decoding, so it is not reported as zero-size.
    fn zero_wire_defs(&self) -> Vec<bool> {
        #[derive(Clone, Copy, PartialEq)]
        enum State {
            Unknown,
            Visiting,
            Zero,
            NonZero,
        }
        fn resolve(defs: &[Def], i: usize, state: &mut [State]) -> bool {
            match state[i] {
                State::Zero => return true,
                State::NonZero | State::Visiting => return false,
                State::Unknown => {}
            }
            state[i] = State::Visiting;
            let zero = defs[i].kind == DefKind::Struct
                && defs[i].fields.iter().all(|f| !f.is_array && matches!(f.ty, FieldType::Def(d) if d < defs.len() && resolve(defs, d, state)));
            state[i] = if zero { State::Zero } else { State::NonZero };
            zero
        }
        let mut state = vec![State::Unknown; self.defs.len()];
        (0..self.defs.len()).map(|i| resolve(&self.defs, i, &mut state)).collect()
    }

    /// Picks the `Message` definition (Figma's root), falling back to the last message def.
    pub fn root_message(&self) -> Option<usize> {
        self.find("Message").filter(|&i| self.defs[i].kind == DefKind::Message).or_else(|| self.defs.iter().rposition(|d| d.kind == DefKind::Message))
    }

    fn decode_def(&self, idx: usize, r: &mut Reader, depth: usize, skip_heavy: bool) -> Result<KiwiValue> {
        if depth > MAX_DEPTH {
            return Err(KiwiError::Bad("nesting too deep".into()));
        }
        let def = &self.defs[idx];
        match def.kind {
            DefKind::Enum => {
                let v = r.var_u32()?;
                let found = def.field_by_value.get(v as usize).and_then(|&x| x).map(|i| &def.fields[i]).or_else(|| def.fields.iter().find(|f| f.value == v));
                Ok(match found {
                    Some(f) => KiwiValue::Enum(f.name.clone()),
                    None => KiwiValue::EnumRaw(v),
                })
            }
            DefKind::Struct => {
                let mut out = Vec::with_capacity(def.fields.len());
                for f in &def.fields {
                    out.push((f.name.clone(), self.decode_field(f, r, depth, skip_heavy)?));
                }
                Ok(KiwiValue::Struct(out))
            }
            DefKind::Message => {
                let mut out: Vec<(Arc<str>, KiwiValue)> = Vec::new();
                loop {
                    let id = r.var_u32()?;
                    if id == 0 {
                        break;
                    }
                    let f = def
                        .field_by_value
                        .get(id as usize)
                        .and_then(|&x| x)
                        .map(|i| &def.fields[i])
                        .or_else(|| def.fields.iter().find(|f| f.value == id))
                        .ok_or_else(|| KiwiError::Bad(format!("unknown field id {id} in {}", def.name)))?;
                    if skip_heavy && matches!(&*f.name, "editInfo" | "derivedTextData" | "derivedImmutableFrameData") {
                        self.skip_field(f, r, depth)?;
                        continue;
                    }
                    let val = self.decode_field(f, r, depth, skip_heavy)?;
                    out.push((f.name.clone(), val));
                }
                if skip_heavy && out.capacity() > out.len() + 4 {
                    out.shrink_to_fit();
                }
                Ok(KiwiValue::Message(out))
            }
        }
    }

    fn skip_def(&self, idx: usize, r: &mut Reader, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(KiwiError::Bad("nesting too deep".into()));
        }
        let def = &self.defs[idx];
        match def.kind {
            DefKind::Enum => {
                let _ = r.var_u32()?;
                Ok(())
            }
            DefKind::Struct => {
                for f in &def.fields {
                    self.skip_field(f, r, depth)?;
                }
                Ok(())
            }
            DefKind::Message => {
                loop {
                    let id = r.var_u32()?;
                    if id == 0 {
                        break;
                    }
                    let f = def
                        .field_by_value
                        .get(id as usize)
                        .and_then(|&x| x)
                        .map(|i| &def.fields[i])
                        .or_else(|| def.fields.iter().find(|f| f.value == id))
                        .ok_or_else(|| KiwiError::Bad(format!("unknown field id {id} in {}", def.name)))?;
                    self.skip_field(f, r, depth)?;
                }
                Ok(())
            }
        }
    }

    fn skip_field(&self, f: &Field, r: &mut Reader, depth: usize) -> Result<()> {
        if !f.is_array {
            return self.skip_type(f.ty, r, depth + 1);
        }
        let n = r.var_u32()? as usize;
        if f.ty == FieldType::Native(Native::Byte) {
            let _ = r.take(n)?;
            return Ok(());
        }
        if n > r.remaining() {
            return Err(KiwiError::Bad(format!("array length {n} exceeds data")));
        }
        for _ in 0..n {
            self.skip_type(f.ty, r, depth + 1)?;
        }
        Ok(())
    }

    fn skip_type(&self, ty: FieldType, r: &mut Reader, depth: usize) -> Result<()> {
        match ty {
            FieldType::Native(n) => match n {
                Native::Bool | Native::Byte => {
                    let _ = r.byte()?;
                }
                Native::Int => {
                    let _ = r.var_i32()?;
                }
                Native::Uint => {
                    let _ = r.var_u32()?;
                }
                Native::Float => {
                    let _ = r.var_f32()?;
                }
                Native::String => {
                    r.skip_string()?;
                }
                Native::Int64 => {
                    let _ = r.var_i64()?;
                }
                Native::Uint64 => {
                    let _ = r.var_u64()?;
                }
            },
            FieldType::Def(i) => self.skip_def(i, r, depth)?,
        }
        Ok(())
    }

    fn decode_field(&self, f: &Field, r: &mut Reader, depth: usize, skip_heavy: bool) -> Result<KiwiValue> {
        if !f.is_array {
            return self.decode_type(f.ty, r, depth + 1, skip_heavy);
        }
        let n = r.var_u32()? as usize;
        if f.ty == FieldType::Native(Native::Byte) {
            return Ok(KiwiValue::Bytes(r.take(n)?.to_vec()));
        }
        if n > r.remaining() {
            return Err(KiwiError::Bad(format!("array length {n} exceeds data")));
        }
        let mut v = Vec::with_capacity(bounded_capacity(n));
        for _ in 0..n {
            v.push(self.decode_type(f.ty, r, depth + 1, skip_heavy)?);
        }
        Ok(KiwiValue::Array(v))
    }

    fn decode_type(&self, ty: FieldType, r: &mut Reader, depth: usize, skip_heavy: bool) -> Result<KiwiValue> {
        r.charge()?;
        if matches!(ty, FieldType::Def(i) if r.zero_wire.get(i) == Some(&true)) {
            r.charge_zero_wire()?;
        }
        Ok(match ty {
            FieldType::Native(n) => match n {
                Native::Bool => KiwiValue::Bool(r.byte()? != 0),
                Native::Byte => KiwiValue::Byte(r.byte()?),
                Native::Int => KiwiValue::Int(r.var_i32()?),
                Native::Uint => KiwiValue::Uint(r.var_u32()?),
                Native::Float => KiwiValue::Float(r.var_f32()?),
                Native::String => KiwiValue::String(r.string()?),
                Native::Int64 => KiwiValue::Int64(r.var_i64()?),
                Native::Uint64 => KiwiValue::Uint64(r.var_u64()?),
            },
            FieldType::Def(i) => self.decode_def(i, r, depth, skip_heavy)?,
        })
    }

    /// Encodes `value` as definition `root` (test/export helper).
    pub fn encode_message(&self, root: usize, value: &KiwiValue) -> Result<Vec<u8>> {
        let mut w = Writer::default();
        self.encode_def(root, value, &mut w, 0)?;
        Ok(w.buf)
    }

    fn encode_def(&self, idx: usize, v: &KiwiValue, w: &mut Writer, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(KiwiError::Bad("nesting too deep".into()));
        }
        let def = &self.defs[idx];
        match (def.kind, v) {
            (DefKind::Enum, KiwiValue::Enum(n)) => {
                let f = def.fields.iter().find(|f| f.name == *n).ok_or_else(|| KiwiError::Bad(format!("unknown enum variant {n}")))?;
                w.var_u64(f.value as u64);
            }
            (DefKind::Enum, KiwiValue::EnumRaw(x)) => w.var_u64(*x as u64),
            (DefKind::Struct, KiwiValue::Struct(fs)) => {
                for f in &def.fields {
                    let fv = fs.iter().find(|(n, _)| *n == f.name).map(|(_, v)| v).ok_or_else(|| KiwiError::Bad(format!("missing struct field {}", f.name)))?;
                    self.encode_field(f, fv, w, depth).map_err(|e| in_field(&def.name, &f.name, e))?;
                }
            }
            (DefKind::Message, KiwiValue::Message(fs)) => {
                for (n, fv) in fs {
                    let f = def.fields.iter().find(|f| f.name == *n).ok_or_else(|| KiwiError::Bad(format!("unknown field {}.{n}", def.name)))?;
                    w.var_u64(f.value as u64);
                    self.encode_field(f, fv, w, depth).map_err(|e| in_field(&def.name, &f.name, e))?;
                }
                w.byte(0);
            }
            _ => return Err(KiwiError::Bad(format!("value does not match definition {}", def.name))),
        }
        Ok(())
    }

    fn encode_field(&self, f: &Field, v: &KiwiValue, w: &mut Writer, depth: usize) -> Result<()> {
        if !f.is_array {
            return self.encode_type(f.ty, v, w, depth + 1);
        }
        match v {
            KiwiValue::Bytes(b) => {
                w.var_u64(b.len() as u64);
                w.buf.extend_from_slice(b);
            }
            KiwiValue::Array(a) => {
                w.var_u64(a.len() as u64);
                for x in a {
                    self.encode_type(f.ty, x, w, depth + 1)?;
                }
            }
            _ => return Err(KiwiError::Bad(format!("field {} expects an array", f.name))),
        }
        Ok(())
    }

    fn encode_type(&self, ty: FieldType, v: &KiwiValue, w: &mut Writer, depth: usize) -> Result<()> {
        match (ty, v) {
            (FieldType::Native(Native::Bool), KiwiValue::Bool(b)) => w.byte(*b as u8),
            (FieldType::Native(Native::Byte), KiwiValue::Byte(b)) => w.byte(*b),
            (FieldType::Native(Native::Int), KiwiValue::Int(i)) => w.var_i64(*i as i64),
            (FieldType::Native(Native::Uint), KiwiValue::Uint(u)) => w.var_u64(*u as u64),
            (FieldType::Native(Native::Float), KiwiValue::Float(x)) => w.var_f32(*x),
            (FieldType::Native(Native::String), KiwiValue::String(s)) => w.string(s),
            (FieldType::Native(Native::Int64), KiwiValue::Int64(i)) => w.var_i64(*i),
            (FieldType::Native(Native::Uint64), KiwiValue::Uint64(u)) => w.var_u64(*u),
            (FieldType::Def(i), v) => self.encode_def(i, v, w, depth)?,
            _ => return Err(KiwiError::Bad("value does not match field type".into())),
        }
        Ok(())
    }

    /// Returns `true` if enum definition `enum_name` contains `variant`.
    pub fn has_enum_variant(&self, enum_name: &str, variant: &str) -> bool {
        let Some(idx) = self.find(enum_name) else { return false };
        let def = &self.defs[idx];
        def.kind == DefKind::Enum && def.fields.iter().any(|f| &*f.name == variant)
    }

    /// Reconstructs a typed `(field_name, KiwiValue)` pair for field `field_name` of
    /// definition `def_idx` from a string value stored in `Node::fig_metadata`.
    pub fn def_field_from_metadata(&self, def_idx: usize, field_name: &str, raw_str: &str) -> Option<(Arc<str>, KiwiValue)> {
        let def = self.defs.get(def_idx)?;
        let f = def.fields.iter().find(|f| &*f.name == field_name)?;
        let kv = self.field_from_metadata(f, raw_str, 0)?;
        Some((f.name.clone(), kv))
    }

    fn field_from_metadata(&self, f: &Field, raw_str: &str, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        if !f.is_array {
            match f.ty {
                FieldType::Native(Native::String) => {
                    return Some(KiwiValue::String(raw_str.to_string()));
                }
                FieldType::Native(Native::Bool) => {
                    return match raw_str.trim() {
                        "true" => Some(KiwiValue::Bool(true)),
                        "false" => Some(KiwiValue::Bool(false)),
                        other => serde_json::from_str::<bool>(other).ok().map(KiwiValue::Bool),
                    };
                }
                FieldType::Native(Native::Byte) => {
                    let s = raw_str.trim();
                    return s.parse::<u8>().ok().or_else(|| s.parse::<f64>().ok().map(|x| x as u8)).map(KiwiValue::Byte);
                }
                FieldType::Native(Native::Int) => {
                    let s = raw_str.trim();
                    return s.parse::<i32>().ok().or_else(|| s.parse::<f64>().ok().map(|x| x as i32)).map(KiwiValue::Int);
                }
                FieldType::Native(Native::Uint) => {
                    let s = raw_str.trim();
                    return s.parse::<u32>().ok().or_else(|| s.parse::<f64>().ok().map(|x| x.max(0.0) as u32)).map(KiwiValue::Uint);
                }
                FieldType::Native(Native::Float) => {
                    let x = raw_str.trim().parse::<f32>().ok()?;
                    return x.is_finite().then_some(KiwiValue::Float(x));
                }
                FieldType::Native(Native::Int64) => {
                    let s = raw_str.trim().trim_matches('"');
                    return s.parse::<i64>().ok().or_else(|| s.parse::<f64>().ok().map(|x| x as i64)).map(KiwiValue::Int64);
                }
                FieldType::Native(Native::Uint64) => {
                    let s = raw_str.trim().trim_matches('"');
                    return s.parse::<u64>().ok().or_else(|| s.parse::<f64>().ok().map(|x| x.max(0.0) as u64)).map(KiwiValue::Uint64);
                }
                FieldType::Def(d) => {
                    let def = self.defs.get(d)?;
                    if def.kind == DefKind::Enum {
                        let s = raw_str.trim().trim_matches('"');
                        if let Some(ef) = def.fields.iter().find(|ef| &*ef.name == s) {
                            return Some(KiwiValue::Enum(ef.name.clone()));
                        }
                        if let Ok(u) = s.parse::<u32>() {
                            return Some(KiwiValue::EnumRaw(u));
                        }
                        return None;
                    }
                    let trimmed = raw_str.trim();
                    if !trimmed.starts_with('{') {
                        let unquoted = trimmed.trim_matches('"');
                        if &*def.name == "GUID" {
                            if let Some((s, l)) = parse_guid_colon(unquoted) {
                                return Some(guid_struct_kv(s, l));
                            }
                        } else if &*def.name == "WidgetPointer" {
                            if let Some((s, l)) = parse_guid_colon(unquoted) {
                                return Some(KiwiValue::Message(vec![("nodeId".into(), guid_struct_kv(s, l))]));
                            }
                        }
                    }
                    let v: serde_json::Value = serde_json::from_str(trimmed).ok()?;
                    return self.def_from_json(d, &v, depth + 1);
                }
            }
        }

        if f.ty == FieldType::Native(Native::Byte) {
            let trimmed = raw_str.trim().trim_matches('"');
            if let Some(bytes) = decode_hex_bytes(trimmed) {
                return Some(KiwiValue::Bytes(bytes));
            }
        }
        let v: serde_json::Value = serde_json::from_str(raw_str.trim()).ok()?;
        self.field_from_json(f, &v, depth + 1)
    }

    /// Reconstructs a `KiwiValue` for field `f` from a `serde_json::Value`.
    pub fn field_from_json(&self, f: &Field, v: &serde_json::Value, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        if !f.is_array {
            return self.type_from_json(f.ty, v, depth + 1);
        }
        if f.ty == FieldType::Native(Native::Byte) {
            if let Some(s) = v.as_str() {
                return decode_hex_bytes(s).map(KiwiValue::Bytes);
            }
            if let Some(arr) = v.as_array() {
                let bytes: Option<Vec<u8>> = arr.iter().map(|x| x.as_u64().and_then(|n| u8::try_from(n).ok())).collect();
                return bytes.map(KiwiValue::Bytes);
            }
            return None;
        }
        let arr = v.as_array()?;
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            if let Some(elem) = self.type_from_json(f.ty, item, depth + 1) {
                out.push(elem);
            }
        }
        if out.is_empty() && !arr.is_empty() {
            return None;
        }
        Some(KiwiValue::Array(out))
    }

    fn type_from_json(&self, ty: FieldType, v: &serde_json::Value, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        match ty {
            FieldType::Native(Native::Bool) => v.as_bool().map(KiwiValue::Bool),
            FieldType::Native(Native::Byte) => v.as_u64().and_then(|n| u8::try_from(n).ok()).map(KiwiValue::Byte),
            FieldType::Native(Native::Int) => v.as_i64().and_then(|n| i32::try_from(n).ok()).or_else(|| v.as_f64().map(|x| x as i32)).map(KiwiValue::Int),
            FieldType::Native(Native::Uint) => {
                v.as_u64().and_then(|n| u32::try_from(n).ok()).or_else(|| v.as_f64().map(|x| x.max(0.0) as u32)).map(KiwiValue::Uint)
            }
            FieldType::Native(Native::Float) => {
                let x = v.as_f64()? as f32;
                x.is_finite().then_some(KiwiValue::Float(x))
            }
            FieldType::Native(Native::String) => v.as_str().map(|s| KiwiValue::String(s.to_string())),
            FieldType::Native(Native::Int64) => v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())).map(KiwiValue::Int64),
            FieldType::Native(Native::Uint64) => v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())).map(KiwiValue::Uint64),
            FieldType::Def(idx) => self.def_from_json(idx, v, depth + 1),
        }
    }

    fn def_from_json(&self, idx: usize, v: &serde_json::Value, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        let def = self.defs.get(idx)?;
        match def.kind {
            DefKind::Enum => {
                if let Some(s) = v.as_str() {
                    if let Some(ef) = def.fields.iter().find(|ef| &*ef.name == s) {
                        return Some(KiwiValue::Enum(ef.name.clone()));
                    }
                    if let Ok(u) = s.parse::<u32>() {
                        return Some(KiwiValue::EnumRaw(u));
                    }
                    return None;
                }
                v.as_u64().and_then(|n| u32::try_from(n).ok()).map(KiwiValue::EnumRaw)
            }
            DefKind::Struct => {
                if &*def.name == "GUID" {
                    if let Some(s) = v.as_str() {
                        if let Some((sess, loc)) = parse_guid_colon(s) {
                            return Some(guid_struct_kv(sess, loc));
                        }
                    }
                }
                let obj = v.as_object()?;
                let mut fs = Vec::with_capacity(def.fields.len());
                for sf in &def.fields {
                    let kv = match obj.get(&*sf.name) {
                        Some(fv) if !fv.is_null() => self.field_from_json(sf, fv, depth + 1)?,
                        _ => self.struct_field_default(&def.name, sf, depth + 1)?,
                    };
                    fs.push((sf.name.clone(), kv));
                }
                Some(KiwiValue::Struct(fs))
            }
            DefKind::Message => {
                if &*def.name == "WidgetPointer" {
                    if let Some(s) = v.as_str() {
                        if let Some((sess, loc)) = parse_guid_colon(s) {
                            return Some(KiwiValue::Message(vec![("nodeId".into(), guid_struct_kv(sess, loc))]));
                        }
                    }
                }
                let obj = v.as_object()?;
                let mut fs = Vec::new();
                for sf in &def.fields {
                    if let Some(fv) = obj.get(&*sf.name) {
                        if !fv.is_null() {
                            if let Some(kv) = self.field_from_json(sf, fv, depth + 1) {
                                fs.push((sf.name.clone(), kv));
                            }
                        }
                    }
                }
                if fs.is_empty() && !obj.is_empty() {
                    return None;
                }
                Some(KiwiValue::Message(fs))
            }
        }
    }

    fn struct_field_default(&self, struct_name: &str, f: &Field, depth: usize) -> Option<KiwiValue> {
        if struct_name == "Color" && &*f.name == "a" && !f.is_array && f.ty == FieldType::Native(Native::Float) {
            return Some(KiwiValue::Float(1.0));
        }
        self.default_field_value(f, depth)
    }

    fn default_field_value(&self, f: &Field, depth: usize) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        if f.is_array {
            return Some(if f.ty == FieldType::Native(Native::Byte) { KiwiValue::Bytes(Vec::new()) } else { KiwiValue::Array(Vec::new()) });
        }
        match f.ty {
            FieldType::Native(Native::Bool) => Some(KiwiValue::Bool(false)),
            FieldType::Native(Native::Byte) => Some(KiwiValue::Byte(0)),
            FieldType::Native(Native::Int) => Some(KiwiValue::Int(0)),
            FieldType::Native(Native::Uint) => Some(KiwiValue::Uint(0)),
            FieldType::Native(Native::Float) => Some(KiwiValue::Float(0.0)),
            FieldType::Native(Native::String) => Some(KiwiValue::String(String::new())),
            FieldType::Native(Native::Int64) => Some(KiwiValue::Int64(0)),
            FieldType::Native(Native::Uint64) => Some(KiwiValue::Uint64(0)),
            FieldType::Def(idx) => {
                let def = self.defs.get(idx)?;
                match def.kind {
                    DefKind::Enum => Some(def.fields.first().map(|ef| KiwiValue::Enum(ef.name.clone())).unwrap_or(KiwiValue::EnumRaw(0))),
                    DefKind::Struct => {
                        let mut fs = Vec::with_capacity(def.fields.len());
                        for sf in &def.fields {
                            fs.push((sf.name.clone(), self.struct_field_default(&def.name, sf, depth + 1)?));
                        }
                        Some(KiwiValue::Struct(fs))
                    }
                    DefKind::Message => Some(KiwiValue::Message(Vec::new())),
                }
            }
        }
    }
}

fn parse_guid_colon(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once(':')?;
    Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
}

fn guid_struct_kv(session: u32, local: u32) -> KiwiValue {
    KiwiValue::Struct(vec![("sessionID".into(), KiwiValue::Uint(session)), ("localID".into(), KiwiValue::Uint(local))])
}

fn decode_hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

// ---------------------------------------------------------------- writer

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn byte(&mut self, b: u8) {
        self.buf.push(b);
    }
    pub fn var_u64(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
    }
    pub fn var_i64(&mut self, v: i64) {
        self.var_u64(((v << 1) ^ (v >> 63)) as u64);
    }
    pub fn var_f32(&mut self, v: f32) {
        let bits = v.to_bits().rotate_left(9);
        if bits & 0xff == 0 {
            self.buf.push(0);
        } else {
            self.buf.extend_from_slice(&bits.to_le_bytes());
        }
    }
    pub fn string(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
}

// ---------------------------------------------------------------- JSON

/// Converts a decoded value to JSON: enums by name, `byte[]` as lowercase hex,
/// int64/uint64 as numbers (strings when beyond 2^53).
pub fn to_json(v: &KiwiValue) -> serde_json::Value {
    use serde_json::{Number, Value};
    match v {
        KiwiValue::Bool(b) => Value::Bool(*b),
        KiwiValue::Byte(b) => Value::from(*b),
        KiwiValue::Int(i) => Value::from(*i),
        KiwiValue::Uint(u) => Value::from(*u),
        KiwiValue::Float(f) => clean_f32(*f),
        KiwiValue::String(s) => Value::String(s.clone()),
        KiwiValue::Int64(i) => {
            if i.unsigned_abs() < (1 << 53) {
                Value::from(*i)
            } else {
                Value::String(i.to_string())
            }
        }
        KiwiValue::Uint64(u) => {
            if *u < (1 << 53) {
                Value::from(*u)
            } else {
                Value::String(u.to_string())
            }
        }
        KiwiValue::Bytes(b) => Value::String(hex(b)),
        KiwiValue::Enum(n) => Value::String(n.to_string()),
        KiwiValue::EnumRaw(u) => Value::Number(Number::from(*u)),
        KiwiValue::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        KiwiValue::Struct(f) | KiwiValue::Message(f) => {
            let mut m = serde_json::Map::new();
            for (k, v) in f {
                m.insert(k.to_string(), to_json(v));
            }
            Value::Object(m)
        }
    }
}

/// Consumes a `KiwiValue` and converts it to JSON without cloning strings.
pub fn into_json(v: KiwiValue) -> serde_json::Value {
    use serde_json::{Number, Value};
    match v {
        KiwiValue::Bool(b) => Value::Bool(b),
        KiwiValue::Byte(b) => Value::from(b),
        KiwiValue::Int(i) => Value::from(i),
        KiwiValue::Uint(u) => Value::from(u),
        KiwiValue::Float(f) => clean_f32(f),
        KiwiValue::String(s) => Value::String(s),
        KiwiValue::Int64(i) => {
            if i.unsigned_abs() < (1 << 53) {
                Value::from(i)
            } else {
                Value::String(i.to_string())
            }
        }
        KiwiValue::Uint64(u) => {
            if u < (1 << 53) {
                Value::from(u)
            } else {
                Value::String(u.to_string())
            }
        }
        KiwiValue::Bytes(b) => Value::String(hex(&b)),
        KiwiValue::Enum(n) => Value::String(n.to_string()),
        KiwiValue::EnumRaw(u) => Value::Number(Number::from(u)),
        KiwiValue::Array(a) => Value::Array(a.into_iter().map(into_json).collect()),
        KiwiValue::Struct(f) | KiwiValue::Message(f) => {
            let mut m = serde_json::Map::new();
            for (k, v) in f {
                m.insert(k.to_string(), into_json(v));
            }
            Value::Object(m)
        }
    }
}

/// Fast conversion of a decoded value to JSON for internal `.fig` import,
/// converting `f32` directly to `f64` without string round-tripping.
pub fn to_json_fast(v: &KiwiValue) -> serde_json::Value {
    use serde_json::{Number, Value};
    match v {
        KiwiValue::Bool(b) => Value::Bool(*b),
        KiwiValue::Byte(b) => Value::from(*b),
        KiwiValue::Int(i) => Value::from(*i),
        KiwiValue::Uint(u) => Value::from(*u),
        KiwiValue::Float(f) => Number::from_f64(*f as f64).map(Value::Number).unwrap_or(Value::Null),
        KiwiValue::String(s) => Value::String(s.clone()),
        KiwiValue::Int64(i) => {
            if i.unsigned_abs() < (1 << 53) {
                Value::from(*i)
            } else {
                Value::String(i.to_string())
            }
        }
        KiwiValue::Uint64(u) => {
            if *u < (1 << 53) {
                Value::from(*u)
            } else {
                Value::String(u.to_string())
            }
        }
        KiwiValue::Bytes(b) => Value::String(hex(b)),
        KiwiValue::Enum(n) => Value::String(n.to_string()),
        KiwiValue::EnumRaw(u) => Value::Number(Number::from(*u)),
        KiwiValue::Array(a) => Value::Array(a.iter().map(to_json_fast).collect()),
        KiwiValue::Struct(f) | KiwiValue::Message(f) => {
            let mut m = serde_json::Map::with_capacity(f.len());
            for (k, v) in f {
                if matches!(&**k, "editInfo" | "derivedTextData" | "derivedImmutableFrameData" | "derivedSymbolData") {
                    continue;
                }
                m.insert(k.to_string(), to_json_fast(v));
            }
            Value::Object(m)
        }
    }
}

/// Consumes a `KiwiValue` for fast `.fig` import, moving strings and freeing
/// intermediate Kiwi nodes as they are converted to `serde_json::Value`.
pub fn into_json_fast(v: KiwiValue) -> serde_json::Value {
    use serde_json::{Number, Value};
    match v {
        KiwiValue::Bool(b) => Value::Bool(b),
        KiwiValue::Byte(b) => Value::from(b),
        KiwiValue::Int(i) => Value::from(i),
        KiwiValue::Uint(u) => Value::from(u),
        KiwiValue::Float(f) => Number::from_f64(f as f64).map(Value::Number).unwrap_or(Value::Null),
        KiwiValue::String(s) => Value::String(s),
        KiwiValue::Int64(i) => {
            if i.unsigned_abs() < (1 << 53) {
                Value::from(i)
            } else {
                Value::String(i.to_string())
            }
        }
        KiwiValue::Uint64(u) => {
            if u < (1 << 53) {
                Value::from(u)
            } else {
                Value::String(u.to_string())
            }
        }
        KiwiValue::Bytes(b) => Value::String(hex(&b)),
        KiwiValue::Enum(n) => Value::String(n.to_string()),
        KiwiValue::EnumRaw(u) => Value::Number(Number::from(u)),
        KiwiValue::Array(a) => Value::Array(a.into_iter().map(into_json_fast).collect()),
        KiwiValue::Struct(f) | KiwiValue::Message(f) => {
            let mut m = serde_json::Map::with_capacity(f.len());
            for (k, v) in f {
                if matches!(&*k, "editInfo" | "derivedTextData" | "derivedImmutableFrameData" | "derivedSymbolData") {
                    continue;
                }
                m.insert(k.to_string(), into_json_fast(v));
            }
            Value::Object(m)
        }
    }
}

/// f32 -> JSON number without f32->f64 widening noise (0.1f32 -> 0.1).
pub fn clean_f32(f: f32) -> serde_json::Value {
    if !f.is_finite() {
        return serde_json::Value::Null;
    }
    if f.fract() == 0.0 && f.abs() < 1e15 {
        if let Some(n) = serde_json::Number::from_f64(f as f64) {
            return serde_json::Value::Number(n);
        }
    }
    let s = format!("{f}");
    s.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null)
}

pub fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

// ---------------------------------------------------------------- builder

/// Small helper for building schemas programmatically (tests, exporter).
#[derive(Default)]
pub struct SchemaBuilder {
    schema: Schema,
    names: BTreeMap<String, usize>,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn define(&mut self, name: &str, kind: DefKind) -> usize {
        let i = self.schema.defs.len();
        self.schema.defs.push(Def { name: name.into(), kind, fields: Vec::new(), field_by_value: Vec::new() });
        self.names.insert(name.into(), i);
        i
    }
    pub fn index(&self, name: &str) -> usize {
        self.names[name]
    }
    pub fn field(&mut self, def: usize, name: &str, ty: FieldType, is_array: bool, value: u32) {
        self.schema.defs[def].fields.push(Field { name: name.into(), ty, is_array, value });
    }
    pub fn variant(&mut self, def: usize, name: &str, value: u32) {
        self.field(def, name, FieldType::Def(0), false, value);
    }
    pub fn build(mut self) -> Schema {
        for d in &mut self.schema.defs {
            d.field_by_value = build_field_by_value(&d.fields);
        }
        self.schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        let mut w = Writer::default();
        for v in [0i64, 1, -1, 63, -64, 64, 1 << 31, i32::MIN as i64, i64::MAX, i64::MIN] {
            w.var_i64(v);
        }
        let mut r = Reader::new(&w.buf);
        for v in [0i64, 1, -1, 63, -64, 64, 1 << 31, i32::MIN as i64, i64::MAX, i64::MIN] {
            assert_eq!(r.var_i64().unwrap(), v);
        }
    }

    #[test]
    fn float_roundtrip() {
        let vals = [0.0f32, 1.0, -1.0, 0.1, 123456.78, -3.5e-5, f32::MAX, 1e-30];
        let mut w = Writer::default();
        for v in vals {
            w.var_f32(v);
        }
        let mut r = Reader::new(&w.buf);
        for v in vals {
            assert_eq!(r.var_f32().unwrap(), v);
        }
        // Known kiwi encoding of 1.0: bits 0x3f800000 rotl 9 = 0x7f; high bytes...
        let mut w = Writer::default();
        w.var_f32(1.0);
        assert_eq!(w.buf.len(), 4);
        assert_eq!(w.buf[0], 127);
    }

    #[test]
    fn zero_byte_struct_fanout_fails_cleanly() {
        // 40 levels of structs with two fields each and no wire bytes: 2^40 values from a 2-byte message.
        let mut b = SchemaBuilder::new();
        let m = b.define("Message", DefKind::Message);
        let levels: Vec<usize> = (0..40).map(|i| b.define(&format!("L{i}"), DefKind::Struct)).collect();
        b.field(m, "root", FieldType::Def(levels[0]), false, 1);
        for w in levels.windows(2) {
            b.field(w[0], "a", FieldType::Def(w[1]), false, 1);
            b.field(w[0], "b", FieldType::Def(w[1]), false, 2);
        }
        let s = b.build();
        assert!(s.decode_message_with_budget(m, &[1, 0], 1 << 20).is_err());
    }

    #[test]
    fn truncated_never_panics() {
        let mut b = SchemaBuilder::new();
        let m = b.define("Message", DefKind::Message);
        b.field(m, "a", FieldType::Native(Native::String), false, 1);
        b.field(m, "b", FieldType::Native(Native::Float), true, 2);
        let s = b.build();
        let enc = s.encode();
        for n in 0..enc.len() {
            let _ = Schema::decode(&enc[..n]);
        }
        let msg = KiwiValue::Message(vec![
            ("a".into(), KiwiValue::String("hi".into())),
            ("b".into(), KiwiValue::Array(vec![KiwiValue::Float(1.5), KiwiValue::Float(2.5)])),
        ]);
        let bytes = s.encode_message(0, &msg).unwrap();
        assert_eq!(s.decode_message(0, &bytes).unwrap(), msg);
        for n in 0..bytes.len() {
            assert!(s.decode_message(0, &bytes[..n]).is_err());
        }
    }

    /// Message with `k` array fields of zero-byte struct elements, each claiming `n` elements, followed by `pad` zero bytes.
    fn padded_empty_struct_arrays(k: u32, n: u32, pad: usize) -> (Schema, usize, Vec<u8>) {
        let mut b = SchemaBuilder::new();
        let m = b.define("Message", DefKind::Message);
        let e = b.define("Empty", DefKind::Struct);
        for i in 0..k {
            b.field(m, &format!("a{i}"), FieldType::Def(e), true, i + 1);
        }
        let mut w = Writer::default();
        for i in 0..k {
            w.var_u64(u64::from(i) + 1);
            w.var_u64(u64::from(n));
        }
        w.byte(0);
        let mut buf = w.buf;
        buf.resize(buf.len() + pad, 0);
        (b.build(), m, buf)
    }

    #[test]
    fn value_budget_ignores_zero_padding() {
        // 20 arrays x 50k zero-byte elements = 1M values from ~50 KB of padding. A budget that scales with
        // the input length would grant 8 values per padding byte and let this through.
        let (s, m, data) = padded_empty_struct_arrays(20, 50_000, 50_000);
        let err = s.decode_message_with_budget(m, &data, 100_000).unwrap_err();
        assert!(err.to_string().contains("budget"), "{err}");
        // The same message fits a budget that really covers it, so zero-byte elements are counted exactly.
        assert!(s.decode_message_with_budget(m, &data, 1_100_000).is_ok());
        assert!(s.decode_message_with_budget(m, &data, 1_000_000).is_err());
    }

    #[test]
    fn value_budget_covers_real_files_with_margin() {
        const REAL_NODES: usize = 1_060_000;
        const { assert!(MAX_DECODED_VALUES >= 4 * REAL_NODES * KIWI_VALUES_PER_NODE) };
    }

    #[test]
    fn claimed_counts_reserve_a_bounded_prefix() {
        assert_eq!(bounded_capacity(usize::MAX), MAX_PREALLOC);
        assert_eq!(bounded_capacity(7), 7);
        // Tiny inputs claiming counts up to their own length still decode (or fail) cleanly.
        let (s, m, data) = padded_empty_struct_arrays(1, 300, 300);
        assert!(s.decode_message(m, &data).is_ok());
        let mut huge = vec![1u8, 0xff, 0xff, 0xff, 0xff, 0x0f];
        huge.resize(64, 0);
        assert!(s.decode_message(m, &huge).is_err());
        assert!(Schema::decode(&[0xff, 0xff, 0xff, 0xff, 0x0f]).is_err());
    }

    #[test]
    fn skip_heavy_fields_without_allocation() {
        let mut b = SchemaBuilder::new();
        let edit_info = b.define("EditInfo", DefKind::Message);
        b.field(edit_info, "userId", FieldType::Native(Native::String), false, 1);
        b.field(edit_info, "createdAt", FieldType::Native(Native::Uint), false, 2);

        let derived_text = b.define("DerivedTextData", DefKind::Message);
        b.field(derived_text, "fontDigest", FieldType::Native(Native::Byte), true, 1);
        b.field(derived_text, "baselines", FieldType::Native(Native::Float), true, 2);

        let node_change = b.define("NodeChange", DefKind::Message);
        b.field(node_change, "name", FieldType::Native(Native::String), false, 1);
        b.field(node_change, "editInfo", FieldType::Def(edit_info), false, 2);
        b.field(node_change, "derivedTextData", FieldType::Def(derived_text), false, 3);
        b.field(node_change, "opacity", FieldType::Native(Native::Float), false, 4);

        let root = b.define("Message", DefKind::Message);
        b.field(root, "nodeChanges", FieldType::Def(node_change), true, 1);
        let s = b.build();

        let msg = KiwiValue::Message(vec![(
            "nodeChanges".into(),
            KiwiValue::Array(vec![KiwiValue::Message(vec![
                ("name".into(), KiwiValue::String("Button Label".into())),
                (
                    "editInfo".into(),
                    KiwiValue::Message(vec![("userId".into(), KiwiValue::String("user-999".into())), ("createdAt".into(), KiwiValue::Uint(1_700_000_000))]),
                ),
                (
                    "derivedTextData".into(),
                    KiwiValue::Message(vec![
                        ("fontDigest".into(), KiwiValue::Bytes(vec![1, 2, 3, 4, 5])),
                        ("baselines".into(), KiwiValue::Array(vec![KiwiValue::Float(12.0), KiwiValue::Float(28.0)])),
                    ]),
                ),
                ("opacity".into(), KiwiValue::Float(0.85)),
            ])]),
        )]);
        let bytes = s.encode_message(root, &msg).unwrap();
        let filtered = s.decode_def(root, &mut Reader::new(&bytes), 0, true).unwrap();
        let nc = &filtered.get("nodeChanges").unwrap().as_array().unwrap()[0];
        assert_eq!(nc.get("name"), Some(&KiwiValue::String("Button Label".into())));
        assert!(nc.get("editInfo").is_none());
        assert!(nc.get("derivedTextData").is_none());
        assert_eq!(nc.get("opacity"), Some(&KiwiValue::Float(0.85)));
    }

    #[test]
    fn zero_size_cap_follows_the_input_length_and_keeps_the_hostile_bound() {
        assert_eq!(zero_wire_budget(0), 65_536);
        assert_eq!(zero_wire_budget(1_024), 65_536);
        assert_eq!(zero_wire_budget(10_000), 160_000);
        assert_eq!(zero_wire_budget(50 * 1024 * 1024), 1 << 24);
        assert_eq!(zero_wire_budget(usize::MAX), 1 << 24);

        // 1 KB input cannot decode more than 65,536 zero-size struct instances (140 * 500 = 70,000 > 65,536).
        let (s, m, mut data_1k) = padded_empty_struct_arrays(140, 500, 0);
        data_1k.resize(1_024, 0);
        let err = s.decode_message(m, &data_1k).unwrap_err();
        assert!(err.to_string().contains("zero-size struct count exceeds budget"), "{err}");

        let (s_ok, m_ok, mut data_1k_ok) = padded_empty_struct_arrays(120, 500, 0);
        data_1k_ok.resize(1_024, 0);
        assert!(s_ok.decode_message(m_ok, &data_1k_ok).is_ok());

        // 10 KB input is capped at 160,000 zero-size struct instances (allowing 100,000 > 65,536, rejecting 170,000 > 160,000).
        let (s_10k_ok, m_10k_ok, mut data_10k_ok) = padded_empty_struct_arrays(20, 5_000, 0);
        data_10k_ok.resize(10_000, 0);
        assert!(s_10k_ok.decode_message(m_10k_ok, &data_10k_ok).is_ok());

        let (s_10k_over, m_10k_over, mut data_10k_over) = padded_empty_struct_arrays(34, 5_000, 0);
        data_10k_over.resize(10_000, 0);
        let err = s_10k_over.decode_message(m_10k_over, &data_10k_over).unwrap_err();
        assert!(err.to_string().contains("zero-size struct count exceeds budget"), "{err}");
    }

    #[test]
    fn old_cap_inputs_decode_unchanged() {
        let (s, m, data) = padded_empty_struct_arrays(1, 300, 300);
        assert!(s.decode_message(m, &data).is_ok());
        assert!(s.decode_message_for_import(m, &data).is_ok());
        let (s_over, m_over, data_over) = padded_empty_struct_arrays(20, 4_000, 4_000);
        assert!(s_over.decode_message(m_over, &data_over).is_err());
        assert!(s_over.decode_message_for_import(m_over, &data_over).is_err());
    }
}
