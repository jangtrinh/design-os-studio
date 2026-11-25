//! Generic Kiwi (https://github.com/evanw/kiwi) binary schema + message codec.
//!
//! Decoding is fully dynamic: the schema embedded in the `.fig` file drives the
//! decoder, producing a [`KiwiValue`] tree. A matching encoder is provided so
//! tests (and a future `.fig` writer) can synthesize files.

use crate::stored_glyphs;

mod conform;
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
pub(crate) const MAX_DECODED_VALUES: usize = 1 << 28;

/// A message payload over this many bytes is decoded heavy: unused derived fields are skipped on the fly.
pub(crate) const HEAVY_PAYLOAD_BYTES: usize = 2_000_000;
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
    /// Field name to position in `fields`; read through [`Def::field_position`].
    field_by_name: std::collections::HashMap<Box<str>, usize>,
}

#[cfg(test)]
thread_local! {
    /// Lookups by name that fell back to a scan of `fields` on this thread.
    static NAME_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Def {
    fn new(name: String, kind: DefKind, fields: Vec<Field>) -> Def {
        let field_by_value = build_field_by_value(&fields);
        let field_by_name = build_field_by_name(&fields);
        Def { name, kind, fields, field_by_value, field_by_name }
    }

    /// The position of the field called `name` (the first one when a hostile schema repeats a name), in constant time.
    ///
    /// The table is built with the definition. `fields` is public, so a caller that removes or reorders fields afterwards
    /// leaves the table stale; a table whose size differs from `fields`, or whose position no longer holds the name, is
    /// not trusted and the lookup scans instead, so the answer is the one a scan gives (unless a field was renamed in place, which nothing does).
    pub fn field_position(&self, name: &str) -> Option<usize> {
        if self.is_indexed() {
            match self.field_by_name.get(name) {
                Some(&i) => {
                    if self.fields.get(i).is_some_and(|f| &*f.name == name) {
                        return Some(i);
                    }
                }
                None => return None,
            }
        }
        #[cfg(test)]
        NAME_SCANS.with(|c| c.set(c.get() + 1));
        self.fields.iter().position(|f| &*f.name == name)
    }

    /// The field called `name`; see [`Self::field_position`].
    pub fn field_named(&self, name: &str) -> Option<&Field> {
        self.field_position(name).map(|i| &self.fields[i])
    }

    /// Whether the name table describes `fields`: one entry per field, so no name repeats and none was added or removed since.
    fn is_indexed(&self) -> bool {
        self.field_by_name.len() == self.fields.len()
    }
}

fn build_field_by_name(fields: &[Field]) -> std::collections::HashMap<Box<str>, usize> {
    let mut by_name = std::collections::HashMap::with_capacity(fields.len());
    for (i, f) in fields.iter().enumerate() {
        by_name.entry(Box::<str>::from(&*f.name)).or_insert(i);
    }
    by_name
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
    /// The answer of [`Schema::reaches`] for the first target asked about: the target and one flag per definition.
    reach: std::sync::OnceLock<(usize, Vec<bool>)>,
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
    /// Wire bytes of `derivedTextData` an import may still keep (see `stored_glyphs`), and how many texts it had to leave out.
    stored_glyphs_left: usize,
    stored_glyphs_dropped: usize,
    /// Wire bytes of the `derivedTextData` inside `derivedSymbolData` entries (the data of a text in an instance) an import may
    /// still keep. `None`: those draw on the same allowance as the texts themselves (a paste has one).
    nested_glyphs_left: Option<usize>,
    /// How many `derivedSymbolData` fields the decoder is inside of; above 0, a `derivedTextData` is an instance's.
    nested_depth: usize,
    /// While set, skipping a value counts what decoding it would charge instead of charging it (see [`Schema::keep_stored_glyphs`]).
    tallying: bool,
    tally_values: usize,
    tally_zero_wire: usize,
    /// Whether the definition of `derivedTextData` (by index in the schema being read) is the one Studio writes, so a kept
    /// span can be copied as it is. Worked out once per decode.
    derived_verbatim: Option<(usize, bool)>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            values_left: usize::MAX,
            zero_wire: Vec::new(),
            zero_wire_left: usize::MAX,
            stored_glyphs_left: 0,
            stored_glyphs_dropped: 0,
            nested_glyphs_left: None,
            nested_depth: 0,
            tallying: false,
            tally_values: 0,
            tally_zero_wire: 0,
            derived_verbatim: None,
        }
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
    /// Charges what a walk counted while `tallying`, as the same number of single charges would.
    fn charge_tally(&mut self) -> Result<()> {
        self.values_left = self.values_left.checked_sub(self.tally_values).ok_or_else(|| KiwiError::Bad("decoded value count exceeds budget".into()))?;
        self.zero_wire_left =
            self.zero_wire_left.checked_sub(self.tally_zero_wire).ok_or_else(|| KiwiError::Bad("zero-size struct count exceeds budget".into()))?;
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
        // Kiwi caps a 64-bit varint at 9 bytes: the first 8 carry 7 bits plus a continuation flag, the ninth
        // carries a full 8 bits and never continues.
        let mut v: u64 = 0;
        for i in 0..8 {
            let b = self.byte()?;
            v |= ((b & 0x7f) as u64) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Ok(v | (self.byte()? as u64) << 56)
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
            defs.push(Def::new(name, kind, fields));
        }
        let schema = Schema { defs, reach: Default::default() };
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

    /// Whether a value of definition `from` can hold a value of definition `target` through message fields, `from == target`
    /// included. Arrays and nesting are followed; structs are not (a struct holds no message). Worked out once per schema for
    /// the first target asked about, which in practice is always the same one.
    pub fn reaches(&self, from: usize, target: usize) -> bool {
        let compute = || {
            let mut flags: Vec<bool> = (0..self.defs.len()).map(|i| i == target).collect();
            loop {
                let mut changed = false;
                for (i, d) in self.defs.iter().enumerate() {
                    if flags[i] || d.kind != DefKind::Message {
                        continue;
                    }
                    let hit = d.fields.iter().any(|f| matches!(f.ty, FieldType::Def(c) if flags.get(c).copied().unwrap_or(false)));
                    if hit {
                        flags[i] = true;
                        changed = true;
                    }
                }
                if !changed {
                    return flags;
                }
            }
        };
        let (cached_target, flags) = self.reach.get_or_init(|| (target, compute()));
        if *cached_target == target {
            return flags.get(from).copied().unwrap_or(false);
        }
        compute().get(from).copied().unwrap_or(false)
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
        self.decode_message_counting(root, data, values, zero_wire_instances, skip_heavy, 0, None).map(|(message, _)| message)
    }

    /// The decoder every entry point shares. `stored_glyph_allowance` is how many wire bytes of kept `derivedTextData`
    /// a `skip_heavy` decode may take; `nested_allowance` is the same for the `derivedTextData` of the entries of
    /// `derivedSymbolData` (the data of a text in an instance), or `None` when those draw on the allowance of the texts. The second
    /// value is how many texts it left out because they did not fit.
    #[allow(clippy::too_many_arguments)]
    fn decode_message_counting(
        &self,
        root: usize,
        data: &[u8],
        values: usize,
        zero_wire_instances: usize,
        skip_heavy: bool,
        stored_glyph_allowance: usize,
        nested_allowance: Option<usize>,
    ) -> Result<(KiwiValue, usize)> {
        let mut r = Reader::new(data);
        r.values_left = values;
        r.zero_wire = self.zero_wire_defs();
        r.zero_wire_left = zero_wire_instances;
        r.stored_glyphs_left = stored_glyph_allowance;
        r.nested_glyphs_left = nested_allowance;
        r.charge()?;
        let message = self.decode_def(root, &mut r, 0, skip_heavy)?;
        Ok((message, r.stored_glyphs_dropped))
    }

    /// Decodes a root message for `.fig` import, dropping unused multi-gigabyte
    /// derived fields (`editInfo`, `derivedTextData`, `derivedImmutableFrameData`)
    /// on the fly when the message payload exceeds 2 MB. The same decode budgets apply. The one exception is the
    /// `derivedTextData` of a text (see `stored_glyphs`): it holds the glyphs Figma laid out, which an unedited text
    /// draws and a save writes back. It is kept as wire bytes in Studio's schema and counts toward the decode value
    /// cap; the paste allowance of [`stored_glyphs::ALLOWANCE_PASTE_WIRE_BYTES`] bounds what is retained (a file import
    /// names its own allowance through [`Self::decode_message_for_import_counting`]).
    pub fn decode_message_for_import(&self, root: usize, data: &[u8]) -> Result<KiwiValue> {
        self.decode_message_for_import_with_values(root, data, MAX_DECODED_VALUES)
    }

    /// [`Self::decode_message_for_import`] with a caller-chosen cap on decoded values (untrusted payloads smaller
    /// than a whole file, such as a clipboard, use a far lower cap than [`MAX_DECODED_VALUES`]). The `derivedTextData`
    /// that is kept counts toward that cap like any other field, so a payload close to the cap can fail where it
    /// would have decoded with the glyph data skipped.
    pub fn decode_message_for_import_with_values(&self, root: usize, data: &[u8], values: usize) -> Result<KiwiValue> {
        self.decode_message_for_import_counting(root, data, values, stored_glyphs::ALLOWANCE_PASTE_WIRE_BYTES).map(|(message, _)| message)
    }

    /// [`Self::decode_message_for_import_with_values`] with a caller-chosen allowance for kept `derivedTextData`, which also
    /// returns how many texts that could have drawn their stored glyphs were left out because they did not fit.
    pub fn decode_message_for_import_counting(&self, root: usize, data: &[u8], values: usize, stored_glyph_allowance: usize) -> Result<(KiwiValue, usize)> {
        self.decode_message_for_import_counting_nested(root, data, values, stored_glyph_allowance, None)
    }

    /// [`Self::decode_message_for_import_counting`] with an allowance of its own for the `derivedTextData` of the entries
    /// of `derivedSymbolData`, which hold the data Figma stored for a text in an instance. With `None` those texts draw on
    /// `stored_glyph_allowance` with the rest.
    pub fn decode_message_for_import_counting_nested(
        &self,
        root: usize,
        data: &[u8],
        values: usize,
        stored_glyph_allowance: usize,
        nested_allowance: Option<usize>,
    ) -> Result<(KiwiValue, usize)> {
        let skip_heavy = data.len() > HEAVY_PAYLOAD_BYTES;
        self.decode_message_counting(root, data, values, zero_wire_budget(data.len()), skip_heavy, stored_glyph_allowance, nested_allowance)
    }

    /// A copy of this schema whose field names are allocations of its own. A decoded value holds a clone of the name of each field
    /// it was read into, so threads that decode with one schema all count references on the same few names; each thread that
    /// decodes with a copy counts on its own.
    pub(crate) fn private_copy(&self) -> Schema {
        let defs = self
            .defs
            .iter()
            .map(|d| Def {
                name: d.name.clone(),
                kind: d.kind,
                fields: d.fields.iter().map(|f| Field { name: Arc::from(&*f.name), ty: f.ty, is_array: f.is_array, value: f.value }).collect(),
                field_by_value: d.field_by_value.clone(),
                field_by_name: d.field_by_name.clone(),
            })
            .collect();
        Schema { defs, reach: Default::default() }
    }

    /// The table [`Self::decode_message_prepared`] reads, built once for a schema that decodes many small messages.
    pub(crate) fn zero_wire_table(&self) -> Vec<bool> {
        self.zero_wire_defs()
    }

    /// Decodes one small message of definition `root` written by this program (a stored `derivedTextData`), with the
    /// per-definition table of [`Self::zero_wire_table`] instead of one built per call. The caps scale with the input,
    /// since the bytes may come from a document file: at most 64 values per byte, and the zero-size cap of the length.
    pub(crate) fn decode_message_prepared(&self, root: usize, data: &[u8], zero_wire: &[bool]) -> Result<KiwiValue> {
        let mut r = Reader::new(data);
        r.values_left = data.len().saturating_mul(64).saturating_add(4096).min(MAX_DECODED_VALUES);
        r.zero_wire = zero_wire.to_vec();
        r.zero_wire_left = zero_wire_budget(data.len());
        r.charge()?;
        self.decode_def(root, &mut r, 0, false)
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
                    if skip_heavy && &*f.name == "derivedTextData" {
                        if let Some(kept) = self.keep_stored_glyphs(f, r, depth, &out)? {
                            out.push((f.name.clone(), kept));
                        }
                        continue;
                    }
                    if skip_heavy && matches!(&*f.name, "editInfo" | "derivedImmutableFrameData") {
                        self.skip_field(f, r, depth)?;
                        continue;
                    }
                    let val = if skip_heavy && &*f.name == "derivedSymbolData" {
                        // what its entries hold under `derivedTextData` is the data of a text in an instance
                        r.nested_depth += 1;
                        let val = self.decode_field(f, r, depth, skip_heavy);
                        r.nested_depth -= 1;
                        val?
                    } else {
                        self.decode_field(f, r, depth, skip_heavy)?
                    };
                    out.push((f.name.clone(), val));
                }
                if skip_heavy && out.capacity() > out.len() + 4 {
                    out.shrink_to_fit();
                }
                Ok(KiwiValue::Message(out))
            }
        }
    }

    /// Reads or skips the `derivedTextData` field `f` of the message whose fields so far are `out`, in a heavy decode.
    ///
    /// It is kept for a text (`stored_glyphs::is_text_node`: its `type` precedes this field on the wire, and a message
    /// that does not show it by then is skipped, as every other node's is) and for the entry of a `derivedSymbolData`
    /// (the data Figma stored for a text in an instance; the entry names its node by path and carries no `type`), and only
    /// while the allowance lasts: the one of the texts, or the one of the instances' texts when the import has one. The wire
    /// size is learned by skipping first, so a text that does not fit is never decoded. The span counts toward the decode
    /// value cap and the zero-size cap like any other field once it is kept, and nothing is given back.
    ///
    /// What is kept is the wire bytes of the export schema's `DerivedTextData` (see `derived_text_wire`), returned as
    /// [`KiwiValue::Bytes`], so the decoded tree is dropped here, before the next text is read. A value that cannot be
    /// written in that schema is left out and counted like a text that did not fit. Kept glyphs count toward the value
    /// cap; the allowance bounds what is retained.
    fn keep_stored_glyphs(&self, f: &Field, r: &mut Reader, depth: usize, out: &[(Arc<str>, KiwiValue)]) -> Result<Option<KiwiValue>> {
        let start = r.pos;
        let nested = r.nested_depth > 0;
        if !nested && !stored_glyphs::is_text_node(out) {
            self.skip_field(f, r, depth)?;
            return Ok(None);
        }
        // A file written by a Figma whose `DerivedTextData` is Studio's own definition keeps the span as it is: its walk counts
        // what a decode would charge, and no value tree is built.
        let verbatim = match r.derived_verbatim {
            Some((def, verbatim)) if matches!(f.ty, FieldType::Def(i) if i == def) => verbatim,
            _ => {
                let verbatim = match f.ty {
                    FieldType::Def(i) if !f.is_array => crate::derived_text_wire::is_studio_definition(self, i),
                    _ => false,
                };
                if let FieldType::Def(i) = f.ty {
                    r.derived_verbatim = Some((i, verbatim));
                }
                verbatim
            }
        };
        r.tallying = verbatim;
        r.tally_values = 0;
        r.tally_zero_wire = 0;
        let walked = self.skip_field(f, r, depth);
        r.tallying = false;
        walked?;
        let end = r.pos;
        let wire = end - start;
        // the allowance never wraps: a charge that does not fit means the field is not kept
        let shared = nested && r.nested_glyphs_left.is_none();
        let left = match (nested, r.nested_glyphs_left) {
            (true, Some(left)) => left,
            _ => r.stored_glyphs_left,
        };
        let Some(left_after) = left.checked_sub(wire) else {
            r.stored_glyphs_dropped = r.stored_glyphs_dropped.saturating_add(1);
            return Ok(None);
        };
        let keep = |r: &mut Reader| match (nested && !shared, &mut r.nested_glyphs_left) {
            (true, Some(nested_left)) => *nested_left = left_after,
            _ => r.stored_glyphs_left = left_after,
        };
        if verbatim {
            r.charge_tally()?;
            keep(r);
            return Ok(Some(KiwiValue::Bytes(r.data[start..end].to_vec())));
        }
        // Decoded without the heavy rules, so nothing inside it is kept or charged again. It draws on the same value and
        // zero-size budgets as every other field, and nothing is given back if the text is then left out.
        r.pos = start;
        let field = self.decode_field(f, r, depth, false)?;
        debug_assert_eq!(r.pos, end, "skipping and decoding a field read different spans");
        r.pos = end;
        let Some(bytes) = crate::derived_text_wire::wire_from_kiwi(&field) else {
            r.stored_glyphs_dropped = r.stored_glyphs_dropped.saturating_add(1);
            return Ok(None);
        };
        keep(r);
        Ok(Some(KiwiValue::Bytes(bytes)))
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
        if r.tallying {
            // what `decode_type` would charge for this value
            r.tally_values += 1;
            if matches!(ty, FieldType::Def(i) if r.zero_wire.get(i) == Some(&true)) {
                r.tally_zero_wire += 1;
            }
        }
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
                let f = def.field_named(n).ok_or_else(|| KiwiError::Bad(format!("unknown enum variant {n}")))?;
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
                    let f = def.field_named(n).ok_or_else(|| KiwiError::Bad(format!("unknown field {}.{n}", def.name)))?;
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
        def.kind == DefKind::Enum && def.field_named(variant).is_some()
    }

    /// Reconstructs a typed `(field_name, KiwiValue)` pair for field `field_name` of
    /// definition `def_idx` from a string value stored in `Node::fig_metadata`.
    pub fn def_field_from_metadata(&self, def_idx: usize, field_name: &str, raw_str: &str) -> Option<(Arc<str>, KiwiValue)> {
        let def = self.defs.get(def_idx)?;
        let f = def.field_named(field_name)?;
        let kv = self.field_from_metadata(f, raw_str, 0)?;
        Some((f.name.clone(), kv))
    }

    /// Reconstructs a typed `(field_name, KiwiValue)` pair for field `field_name` of
    /// definition `def_idx` directly from a `serde_json::Value`.
    pub fn def_field_from_json(&self, def_idx: usize, field_name: &str, v: &serde_json::Value) -> Option<(Arc<str>, KiwiValue)> {
        let def = self.defs.get(def_idx)?;
        let f = def.field_named(field_name)?;
        let kv = self.field_from_json(f, v, 0)?;
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
                        if let Some(ef) = def.field_named(s) {
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
                    return self.def_from_json(d, &v, depth + 1, false);
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

    /// Reconstructs a `KiwiValue` for field `f` from a `serde_json::Value`. Lenient: an array element or a message field
    /// that does not convert is left out and the rest is kept (a passthrough value stays usable with one bad part).
    pub fn field_from_json(&self, f: &Field, v: &serde_json::Value, depth: usize) -> Option<KiwiValue> {
        self.field_from_json_mode(f, v, depth, false)
    }

    /// Like [`Self::field_from_json`], but every non-null part of `v` must convert: a value that does not fails the whole
    /// conversion, so the caller never writes a shortened value. For data that is written back whole.
    pub fn field_from_json_strict(&self, f: &Field, v: &serde_json::Value, depth: usize) -> Option<KiwiValue> {
        self.field_from_json_mode(f, v, depth, true)
    }

    fn field_from_json_mode(&self, f: &Field, v: &serde_json::Value, depth: usize, strict: bool) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        if !f.is_array {
            return self.type_from_json(f.ty, v, depth + 1, strict);
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
            match self.type_from_json(f.ty, item, depth + 1, strict) {
                Some(elem) => out.push(elem),
                None if strict => return None,
                None => {}
            }
        }
        if !strict && out.is_empty() && !arr.is_empty() {
            return None;
        }
        Some(KiwiValue::Array(out))
    }

    /// The first key of `v` that is not a field of the definition `f` reads it against, as a path from the field's own
    /// name (`customEffectId.futureFlag`, `componentPropAssignments[1].value.future`), or `None` when every key is named.
    /// Messages, structs and arrays of them are walked at every depth. [`Self::field_from_json_strict`] builds only the
    /// fields a definition names, so a key reported here would not survive an encode.
    pub fn first_unknown_json_key(&self, f: &Field, v: &serde_json::Value) -> Option<String> {
        if v.is_null() {
            return None;
        }
        let FieldType::Def(idx) = f.ty else { return None };
        if !f.is_array {
            return self.unknown_key_in_def(idx, v, &f.name, 0);
        }
        v.as_array()?.iter().enumerate().find_map(|(i, item)| self.unknown_key_in_def(idx, item, &format!("{}[{i}]", f.name), 0))
    }

    fn unknown_key_in_def(&self, idx: usize, v: &serde_json::Value, path: &str, depth: usize) -> Option<String> {
        if depth > MAX_DEPTH {
            return None;
        }
        let def = self.defs.get(idx)?;
        if def.kind == DefKind::Enum {
            return None;
        }
        // A GUID or a widget pointer may also be written as a "session:local" string: no keys to read.
        let obj = v.as_object()?;
        for (key, value) in obj {
            if value.is_null() {
                continue;
            }
            let child = format!("{path}.{key}");
            let Some(field) = def.field_named(key) else { return Some(child) };
            let FieldType::Def(inner) = field.ty else { continue };
            let found = if field.is_array {
                value
                    .as_array()
                    .and_then(|items| items.iter().enumerate().find_map(|(i, item)| self.unknown_key_in_def(inner, item, &format!("{child}[{i}]"), depth + 1)))
            } else {
                self.unknown_key_in_def(inner, value, &child, depth + 1)
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    fn type_from_json(&self, ty: FieldType, v: &serde_json::Value, depth: usize, strict: bool) -> Option<KiwiValue> {
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
            FieldType::Def(idx) => self.def_from_json(idx, v, depth + 1, strict),
        }
    }

    fn def_from_json(&self, idx: usize, v: &serde_json::Value, depth: usize, strict: bool) -> Option<KiwiValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        let def = self.defs.get(idx)?;
        match def.kind {
            DefKind::Enum => {
                if let Some(s) = v.as_str() {
                    if let Some(ef) = def.field_named(s) {
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
                        Some(fv) if !fv.is_null() => self.field_from_json_mode(sf, fv, depth + 1, strict)?,
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
                let mut convert = |sf: &Field, fv: &serde_json::Value| -> Option<()> {
                    match self.field_from_json_mode(sf, fv, depth + 1, strict) {
                        Some(kv) => fs.push((sf.name.clone(), kv)),
                        None if strict => return None,
                        None => {}
                    }
                    Some(())
                };
                if obj.len() < def.fields.len() && def.is_indexed() {
                    // A short object against a long definition (a record of a few keys against `NodeChange`'s hundreds of
                    // fields): look up the object's keys and write them in the definition's order.
                    let mut named: Vec<(usize, &serde_json::Value)> = Vec::with_capacity(obj.len());
                    for (key, fv) in obj {
                        if !fv.is_null() {
                            named.extend(def.field_position(key).map(|pos| (pos, fv)));
                        }
                    }
                    named.sort_unstable_by_key(|(pos, _)| *pos);
                    for (pos, fv) in named {
                        convert(&def.fields[pos], fv)?;
                    }
                } else {
                    for sf in &def.fields {
                        if let Some(fv) = obj.get(&*sf.name) {
                            if !fv.is_null() {
                                convert(sf, fv)?;
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

pub(crate) fn decode_hex_bytes(s: &str) -> Option<Vec<u8>> {
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
        // Mirror of `Reader::var_u64`: at most 9 bytes, the ninth a full 8 bits.
        for _ in 0..8 {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
        self.buf.push(v as u8);
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
        self.schema.defs.push(Def::new(name.into(), kind, Vec::new()));
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
            d.field_by_name = build_field_by_name(&d.fields);
        }
        self.schema
    }
}

/// A hostile payload for the decoder's caps: one text in a font that is not bundled whose `derivedTextData` holds
/// `pad_len` elements of a struct chain `depth + 1` levels deep, each element one wire byte, in a message padded
/// past the 2 MB that makes the decode heavy. Struct levels cost values but no wire bytes.
#[cfg(test)]
pub(crate) fn hostile_chain_payload(depth: usize, pad_len: usize) -> (Schema, usize, Vec<u8>) {
    let mut b = SchemaBuilder::new();
    let node_type = b.define("NodeType", DefKind::Enum);
    b.variant(node_type, "FRAME", 0);
    b.variant(node_type, "TEXT", 1);
    let font_name = b.define("FontName", DefKind::Struct);
    b.field(font_name, "family", FieldType::Native(Native::String), false, 1);
    let mut chain: Vec<usize> = (0..=depth).map(|i| b.define(&format!("Level{i}"), DefKind::Struct)).collect();
    let leaf = chain.pop().unwrap();
    b.field(leaf, "b", FieldType::Native(Native::Byte), false, 1);
    let mut below = leaf;
    for level in chain.into_iter().rev() {
        b.field(level, "a", FieldType::Def(below), false, 1);
        below = level;
    }
    let first = below;
    let derived = b.define("DerivedTextData", DefKind::Message);
    b.field(derived, "pad", FieldType::Def(first), true, 1);
    let node_change = b.define("NodeChange", DefKind::Message);
    b.field(node_change, "type", FieldType::Def(node_type), false, 1);
    b.field(node_change, "fontName", FieldType::Def(font_name), false, 2);
    b.field(node_change, "derivedTextData", FieldType::Def(derived), false, 3);
    let root = b.define("Message", DefKind::Message);
    b.field(root, "nodeChanges", FieldType::Def(node_change), true, 1);
    b.field(root, "blobs", FieldType::Native(Native::Byte), true, 2);
    let schema = b.build();
    let mut element = KiwiValue::Struct(vec![("b".into(), KiwiValue::Byte(1))]);
    for _ in 0..depth {
        element = KiwiValue::Struct(vec![("a".into(), element)]);
    }
    let derived_value = KiwiValue::Message(vec![("pad".into(), KiwiValue::Array(vec![element; pad_len]))]);
    let change = KiwiValue::Message(vec![
        ("type".into(), KiwiValue::Enum("TEXT".into())),
        ("fontName".into(), KiwiValue::Struct(vec![("family".into(), KiwiValue::String("Some Icon Font".into()))])),
        ("derivedTextData".into(), derived_value),
    ]);
    let message = KiwiValue::Message(vec![("nodeChanges".into(), KiwiValue::Array(vec![change])), ("blobs".into(), KiwiValue::Bytes(vec![0; 2_100_000]))]);
    let bytes = schema.encode_message(root, &message).unwrap();
    (schema, root, bytes)
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

    /// Kiwi's 64-bit varint is at most 9 bytes: the ninth carries a full 8 bits and no continuation flag.
    /// Figma's random `strokeSeed` values hit this whenever bit 63 is set.
    #[test]
    fn varint64_ninth_byte_is_full_width() {
        // Real `strokeSeed` bytes from a Figma file, followed by the next field id (0x1c0 = c0 03).
        let wire = [0x86, 0xc4, 0xa9, 0xed, 0xff, 0xae, 0xd2, 0xc4, 0xa3, 0xc0, 0x03];
        let mut r = Reader::new(&wire);
        let v = r.var_u64().unwrap();
        assert_eq!(r.var_u32().unwrap(), 0x1c0);
        assert!(r.at_end());

        let mut w = Writer::default();
        w.var_u64(v);
        assert_eq!(w.buf, wire[..9]);

        let mut w = Writer::default();
        w.var_u64(u64::MAX);
        assert_eq!(w.buf, [0xff; 9]);

        let vals = [0u64, 127, 128, (1 << 56) - 1, 1 << 56, 1 << 63, u64::MAX - 1, u64::MAX];
        let mut w = Writer::default();
        for v in vals {
            w.var_u64(v);
        }
        let mut r = Reader::new(&w.buf);
        for v in vals {
            assert_eq!(r.var_u64().unwrap(), v);
        }
        assert!(r.at_end());
    }

    fn node_change_with(names: &[&str]) -> (Schema, usize, KiwiValue) {
        let s = crate::testkit::figma_like_schema();
        let nc = s.find("NodeChange").expect("NodeChange");
        let json = serde_json::json!({
            "visible": true, "opacity": 0.5, "name": "n", "locked": false, "size": {"x": 1.0, "y": 2.0}, "fontSize": 12.0, "blendMode": "NORMAL",
            "mask": false, "cornerRadius": 4.0, "strokeWeight": 1.0, "textData": {"characters": "x"}, "stackMode": "VERTICAL",
        });
        let obj = json.as_object().unwrap();
        let mut fields = Vec::new();
        for n in names {
            let f = s.defs[nc].field_named(n).unwrap_or_else(|| panic!("{n} is a NodeChange field"));
            fields.push((f.name.clone(), s.field_from_json(f, &obj[*n], 0).unwrap_or_else(|| panic!("{n} converts"))));
        }
        (s, nc, KiwiValue::Message(fields))
    }

    #[test]
    fn a_field_is_found_by_name_without_scanning_the_definition() {
        let names = ["visible", "opacity", "name", "locked", "size", "fontSize", "blendMode", "mask", "cornerRadius", "strokeWeight", "stackMode"];
        let (s, nc, value) = node_change_with(&names);
        assert!(s.defs[nc].fields.len() > 600, "the export schema holds Figma's whole NodeChange");
        NAME_SCANS.with(|c| c.set(0));
        let wire = s.encode_message(nc, &value).expect("encodes");
        assert!(!wire.is_empty());
        // The last field of the definition, a name that is not there, and the first: all answered from the table.
        let last = s.defs[nc].fields.last().unwrap().name.clone();
        assert_eq!(s.defs[nc].field_named(&last).map(|f| f.name.clone()), Some(last));
        assert!(s.defs[nc].field_named("noSuchField").is_none());
        assert!(s.def_field_from_json(nc, "visible", &serde_json::json!(true)).is_some());
        assert_eq!(NAME_SCANS.with(|c| c.get()), 0, "no lookup by name scanned the fields");
    }

    #[test]
    fn a_name_table_that_no_longer_describes_the_fields_falls_back_to_the_scan_and_agrees() {
        let (mut s, nc, _) = node_change_with(&["visible"]);
        let first = s.defs[nc].fields[0].name.clone();
        let second = s.defs[nc].fields[1].name.clone();
        s.defs[nc].fields.retain(|f| f.name != first);
        NAME_SCANS.with(|c| c.set(0));
        assert!(s.defs[nc].field_named(&first).is_none(), "a removed field is gone");
        assert!(s.defs[nc].field_named(&second).is_some(), "a field that moved is still found");
        assert!(NAME_SCANS.with(|c| c.get()) > 0, "the stale table was not trusted");
        // A repeated name resolves to the first field, as a scan does.
        let dup = s.defs[nc].fields[3].clone();
        s.defs[nc].fields.push(Field { value: 9999, ..dup.clone() });
        let found = s.defs[nc].field_named(&dup.name).expect("found");
        assert_eq!(found.value, dup.value);
    }

    /// A message built from a JSON object of a few keys, whichever way the keys are ordered, holds its fields in the
    /// definition's order and equals the result of the walk over every field of the definition.
    #[test]
    fn a_short_json_object_converts_to_the_same_message_as_the_walk_over_every_field() {
        let s = crate::testkit::figma_like_schema();
        let nc = s.find("NodeChange").expect("NodeChange");
        let objects = [
            serde_json::json!({"stackMode": "VERTICAL", "name": "n", "visible": true, "size": {"x": 1.0, "y": 2.0}, "opacity": 0.5}),
            serde_json::json!({"opacity": 0.5, "visible": null, "zzUnknown": 1, "name": "n", "strokeWeight": 2.0}),
            serde_json::json!({"cornerRadius": 4.0, "mask": false, "locked": true, "fontSize": 12.0, "blendMode": "NORMAL", "textData": {"characters": "x"}}),
            // a value that does not convert drops (lenient) or fails (strict) the same way in both
            serde_json::json!({"visible": "not a bool", "name": "n"}),
            serde_json::json!({"zzUnknown": 1}),
            serde_json::json!({}),
        ];
        let mut walked = s.clone();
        walked.defs[nc].field_by_name.clear();
        assert!(!walked.defs[nc].is_indexed());
        for obj in &objects {
            for strict in [false, true] {
                let fast = s.def_from_json(nc, obj, 0, strict);
                let slow = walked.def_from_json(nc, obj, 0, strict);
                assert_eq!(fast, slow, "object {obj} strict {strict}");
                if let Some(KiwiValue::Message(fields)) = &fast {
                    let pos: Vec<usize> = fields.iter().map(|(n, _)| s.defs[nc].field_position(n).unwrap()).collect();
                    assert!(pos.windows(2).all(|w| w[0] < w[1]), "definition order: {pos:?}");
                }
            }
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

    /// A heavy payload (over 2 MB through a padding field) of text node changes, each with a derived font list of
    /// glyphs, followed by a `name` that a desynchronised stream would misread.
    fn text_payload(texts: &[(&str, usize)]) -> (Schema, usize, Vec<u8>) {
        text_payload_with_fonts(texts, 1)
    }

    /// [`text_payload`] with `fonts` entries in each derived font list (a text with more than one is never drawn).
    fn text_payload_with_fonts(texts: &[(&str, usize)], fonts: usize) -> (Schema, usize, Vec<u8>) {
        let mut b = SchemaBuilder::new();
        let node_type = b.define("NodeType", DefKind::Enum);
        b.variant(node_type, "FRAME", 0);
        b.variant(node_type, "TEXT", 1);
        let font_name = b.define("FontName", DefKind::Struct);
        b.field(font_name, "family", FieldType::Native(Native::String), false, 1);
        let font_meta = b.define("FontMeta", DefKind::Message);
        b.field(font_meta, "key", FieldType::Def(font_name), false, 1);
        let glyph = b.define("Glyph", DefKind::Message);
        b.field(glyph, "commandsBlob", FieldType::Native(Native::Uint), false, 1);
        let vector = b.define("Vector", DefKind::Struct);
        b.field(vector, "x", FieldType::Native(Native::Float), false, 1);
        b.field(vector, "y", FieldType::Native(Native::Float), false, 2);
        let derived = b.define("DerivedTextData", DefKind::Message);
        b.field(derived, "fontMetaData", FieldType::Def(font_meta), true, 1);
        b.field(derived, "glyphs", FieldType::Def(glyph), true, 2);
        b.field(derived, "layoutSize", FieldType::Def(vector), false, 3);
        let node_change = b.define("NodeChange", DefKind::Message);
        b.field(node_change, "type", FieldType::Def(node_type), false, 1);
        b.field(node_change, "fontName", FieldType::Def(font_name), false, 2);
        b.field(node_change, "derivedTextData", FieldType::Def(derived), false, 3);
        b.field(node_change, "name", FieldType::Native(Native::String), false, 4);
        let root = b.define("Message", DefKind::Message);
        b.field(root, "nodeChanges", FieldType::Def(node_change), true, 1);
        b.field(root, "pad", FieldType::Native(Native::Byte), true, 2);
        let schema = b.build();
        let font_key = |family: &str| KiwiValue::Struct(vec![("family".into(), KiwiValue::String(family.into()))]);
        let layout_size = || KiwiValue::Struct(vec![("x".into(), KiwiValue::Float(20.0)), ("y".into(), KiwiValue::Float(20.0))]);
        let changes = texts
            .iter()
            .map(|&(family, floats)| {
                KiwiValue::Message(vec![
                    ("type".into(), KiwiValue::Enum("TEXT".into())),
                    ("fontName".into(), KiwiValue::Struct(vec![("family".into(), KiwiValue::String(family.into()))])),
                    (
                        "derivedTextData".into(),
                        KiwiValue::Message(vec![
                            ("layoutSize".into(), layout_size()),
                            ("fontMetaData".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("key".into(), font_key(family))]); fonts])),
                            ("glyphs".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("commandsBlob".into(), KiwiValue::Uint(7))]); floats])),
                        ]),
                    ),
                    ("name".into(), KiwiValue::String(format!("after {family}"))),
                ])
            })
            .collect();
        let message = KiwiValue::Message(vec![("nodeChanges".into(), KiwiValue::Array(changes)), ("pad".into(), KiwiValue::Bytes(vec![0; 2_100_000]))]);
        let bytes = schema.encode_message(root, &message).unwrap();
        (schema, root, bytes)
    }

    fn kept_text_count(message: &KiwiValue) -> usize {
        message.get("nodeChanges").unwrap().as_array().unwrap().iter().filter(|nc| nc.get("derivedTextData").is_some()).count()
    }

    #[test]
    fn a_heavy_decode_keeps_every_text_as_wire_bytes_and_reads_on_after_it() {
        let (s, root, bytes) = text_payload(&[("Some Icon Font", 40), ("Inter", 40), ("Another Icon Font", 40)]);
        let (message, dropped) = s.decode_message_for_import_counting(root, &bytes, MAX_DECODED_VALUES, 1 << 30).unwrap();
        let changes = message.get("nodeChanges").unwrap().as_array().unwrap();
        assert_eq!(dropped, 0);
        for (nc, family) in changes.iter().zip(["Some Icon Font", "Inter", "Another Icon Font"]) {
            let Some(KiwiValue::Bytes(wire)) = nc.get("derivedTextData") else { panic!("{family} keeps its stored data as wire bytes") };
            let derived = crate::derived_text_wire::decode_wire(wire).expect("the kept bytes are a derivedTextData of Studio's schema");
            assert_eq!(derived.get("glyphs").and_then(KiwiValue::as_array).map(<[_]>::len), Some(40), "{family}: every glyph is in the bytes");
            assert_eq!(nc.get("name"), Some(&KiwiValue::String(format!("after {family}"))), "the field after it is read from the right place");
        }
    }

    #[test]
    fn a_heavy_decode_keeps_only_a_text_node() {
        let (s, root, bytes) = text_payload(&[("Some Icon Font", 40)]);
        // the same payload with the node typed FRAME: `type` precedes `derivedTextData`, so it is judged a frame and skipped
        let frame = {
            let mut message = s.decode_message(root, &bytes).unwrap();
            let KiwiValue::Array(changes) = message.get_mut("nodeChanges").unwrap() else { unreachable!() };
            *changes[0].get_mut("type").unwrap() = KiwiValue::Enum("FRAME".into());
            s.encode_message(root, &message).unwrap()
        };
        let (message, dropped) = s.decode_message_for_import_counting(root, &frame, MAX_DECODED_VALUES, 1 << 30).unwrap();
        assert_eq!((kept_text_count(&message), dropped), (0, 0), "a node that is not a text keeps nothing, and costs no allowance");
    }

    #[test]
    fn a_heavy_decode_leaves_out_what_does_not_fit_and_counts_it() {
        let (s, root, bytes) = text_payload(&[("Some Icon Font", 40), ("Another Icon Font", 40), ("Third Icon Font", 4000)]);
        let decode = |allowance: usize| s.decode_message_for_import_counting(root, &bytes, MAX_DECODED_VALUES, allowance).unwrap();
        // the smallest allowance that keeps the two small texts, found by search so the test does not hard-code a wire size
        let two = (0..4000).find(|&a| kept_text_count(&decode(a).0) == 2).expect("two small texts fit some allowance");
        let (message, dropped) = decode(two);
        assert_eq!((kept_text_count(&message), dropped), (2, 1), "the large third text does not fit and is counted");
        let (message, dropped) = decode(two - 1);
        assert_eq!((kept_text_count(&message), dropped), (1, 2), "one byte less leaves out the second text, in file order, and the third");
        let changes = message.get("nodeChanges").unwrap().as_array().unwrap();
        assert!(changes[0].get("derivedTextData").is_some() && changes[1].get("derivedTextData").is_none());
        assert_eq!(changes[2].get("name"), Some(&KiwiValue::String("after Third Icon Font".into())), "the stream stays in step past a skipped text");
        assert_eq!(decode(0).1, 3, "with no allowance every drawable text is counted");
    }

    /// The fewest decoded values with which `decode` succeeds, found by search so no test hard-codes a count.
    fn fewest_values(decode: impl Fn(usize) -> Result<(KiwiValue, usize)>) -> usize {
        let (mut low, mut high) = (1usize, 1usize << 20);
        assert!(decode(high).is_ok(), "the payload decodes under a generous cap");
        while low < high {
            let mid = (low + high) / 2;
            if decode(mid).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        low
    }

    #[test]
    fn the_value_cap_holds_over_the_whole_message_kept_glyphs_included() {
        let (s, root, bytes) = text_payload(&[("Some Icon Font", 400), ("Another Icon Font", 400)]);
        let with_glyphs = |cap: usize| s.decode_message_for_import_counting(root, &bytes, cap, 1 << 30);
        let needed = fewest_values(with_glyphs);
        // the cap that is enough when the glyph data is skipped is not enough when it is kept: the kept span is counted
        let skipping = fewest_values(|cap| s.decode_message_for_import_counting(root, &bytes, cap, 0));
        assert!(needed > skipping, "{needed} values with the glyphs kept, {skipping} without");
        let over = s.decode_message_for_import_counting(root, &bytes, skipping, 1 << 30);
        assert!(
            matches!(&over, Err(e) if e.to_string().contains("decoded value count exceeds budget")),
            "kept glyphs must not fit the cap that skipping them fits"
        );
        let (message, _) = with_glyphs(needed).expect("the payload decodes at exactly the cap it needs");
        assert_eq!(kept_text_count(&message), 2, "both texts are kept at that cap");
        let err = with_glyphs(needed - 1).expect_err("one value less is over the cap");
        assert!(err.to_string().contains("decoded value count exceeds budget"), "{err}");
    }

    #[test]
    fn keeping_glyph_data_costs_values_and_every_decoded_text_is_a_kept_text() {
        let (s, root, bytes) = text_payload(&[("Some Icon Font", 400), ("Another Icon Font", 400)]);
        let at = |allowance: usize| fewest_values(|cap| s.decode_message_for_import_counting(root, &bytes, cap, allowance));
        let (without, with) = (at(0), at(1 << 30));
        assert!(with > without + 400, "keeping 800 glyphs must cost values: {with} against {without}");

        // a text with two fonts is read, kept and charged like any other: what the decoder decodes is what it retains
        let (s2, root2, bytes2) = text_payload_with_fonts(&[("Some Icon Font", 400), ("Another Icon Font", 400)], 2);
        let kept = fewest_values(|cap| s2.decode_message_for_import_counting(root2, &bytes2, cap, 1 << 30));
        let skipped = fewest_values(|cap| s2.decode_message_for_import_counting(root2, &bytes2, cap, 0));
        assert!(kept > skipped + 400, "a decoded span counts: {kept} against {skipped}");
        let (message, dropped) = s2.decode_message_for_import_counting(root2, &bytes2, kept, 1 << 30).unwrap();
        assert_eq!((kept_text_count(&message), dropped), (2, 0), "two fonts: read, kept, charged to the allowance");
        let smallest = (0..100_000).find(|&a| kept_text_count(&s2.decode_message_for_import_counting(root2, &bytes2, kept, a).unwrap().0) == 2).unwrap();
        assert_eq!(kept_text_count(&s2.decode_message_for_import_counting(root2, &bytes2, kept, smallest - 1).unwrap().0), 1, "and the allowance counts it");
    }

    /// A heavy payload over Studio's own schema (so the kept span is copied as it is) and the same payload over a schema whose
    /// `DerivedTextData` has one more field (so the span is decoded, written in Studio's schema and encoded again): the same
    /// message, with `texts` text nodes of `glyphs` glyphs each.
    type Payload = (Schema, usize, Vec<u8>);

    fn studio_schema_payloads(texts: usize, glyphs: usize) -> (Payload, Payload) {
        use crate::testkit::{figma_like_schema, fl, msg, node, st, vec2};
        let message = |schema: &Schema| {
            let glyph = |i: usize| {
                msg(vec![
                    ("commandsBlob", KiwiValue::Uint(0)),
                    ("position", vec2(i as f32, 8.0)),
                    ("fontSize", fl(12.0)),
                    ("firstCharacter", KiwiValue::Uint(i as u32)),
                ])
            };
            let font = st(vec![
                ("family", KiwiValue::String("Some Face".into())),
                ("style", KiwiValue::String("Regular".into())),
                ("postscript", KiwiValue::String(String::new())),
            ]);
            let nodes = (0..texts)
                .map(|t| {
                    node(
                        (1, t as u32 + 1),
                        (0, 1),
                        "a",
                        "TEXT",
                        &format!("text {t}"),
                        vec![(
                            "derivedTextData",
                            msg(vec![
                                ("layoutSize", vec2(20.0, 10.0)),
                                ("glyphs", KiwiValue::Array((0..glyphs).map(glyph).collect())),
                                ("fontMetaData", KiwiValue::Array(vec![msg(vec![("key", font.clone())])])),
                            ]),
                        )],
                    )
                })
                .collect();
            let blobs = KiwiValue::Array(vec![st(vec![("bytes", KiwiValue::Bytes(vec![0; 2_100_000]))])]);
            let root = schema.find("Message").unwrap();
            let bytes = schema.encode_message(root, &msg(vec![("nodeChanges", KiwiValue::Array(nodes)), ("blobs", blobs)])).unwrap();
            (root, bytes)
        };
        let studio = figma_like_schema();
        let (root, bytes) = message(&studio);
        let mut other = figma_like_schema();
        let derived = other.find("DerivedTextData").unwrap();
        other.defs[derived].fields.push(Field { name: "futureField".into(), ty: FieldType::Native(Native::Uint), is_array: false, value: 99 });
        let (other_root, other_bytes) = message(&other);
        assert_eq!(bytes, other_bytes, "the extra field is unused, so both schemas write the same message");
        ((studio, root, bytes), (other, other_root, other_bytes))
    }

    #[test]
    fn a_span_in_studios_own_definition_is_kept_as_it_is_and_costs_what_a_decode_costs() {
        let ((studio, root, bytes), (other, other_root, _)) = studio_schema_payloads(3, 40);
        let derived = studio.find("DerivedTextData").unwrap();
        assert!(crate::derived_text_wire::is_studio_definition(&studio, derived), "the schema Studio writes is its own definition");
        assert!(!crate::derived_text_wire::is_studio_definition(&other, other.find("DerivedTextData").unwrap()), "one more field is another definition");

        let decode = |s: &Schema, r: usize, cap: usize| s.decode_message_for_import_counting(r, &bytes, cap, 1 << 30);
        assert_eq!(
            fewest_values(|cap| decode(&studio, root, cap)),
            fewest_values(|cap| decode(&other, other_root, cap)),
            "skipping with charges costs the values a decode does"
        );
        let (copied, dropped) = decode(&studio, root, MAX_DECODED_VALUES).unwrap();
        let (rewritten, _) = decode(&other, other_root, MAX_DECODED_VALUES).unwrap();
        assert_eq!(dropped, 0);
        assert_eq!(kept_text_count(&copied), 3);
        assert_eq!(copied.get("nodeChanges"), rewritten.get("nodeChanges"), "copied bytes and re-encoded bytes are the same bytes");
        let KiwiValue::Bytes(kept) = copied.get("nodeChanges").unwrap().as_array().unwrap()[0].get("derivedTextData").unwrap() else { panic!("wire bytes") };
        let value = crate::derived_text_wire::decode_wire(kept).expect("they decode");
        assert_eq!(value.get("glyphs").and_then(KiwiValue::as_array).map(<[_]>::len), Some(40));
    }

    /// A hostile schema: `DerivedTextData` holds a `derivedTextData` of its own, with the `type`, `fontName` and font list
    /// that make a message look like a drawable text. Every nested level would be kept and charged again if a kept field
    /// were decoded with the heavy rules on. Followed by an ordinary text.
    fn self_nesting_payload(depth: usize) -> (Schema, usize, Vec<u8>) {
        let mut b = SchemaBuilder::new();
        let node_type = b.define("NodeType", DefKind::Enum);
        b.variant(node_type, "FRAME", 0);
        b.variant(node_type, "TEXT", 1);
        let font_name = b.define("FontName", DefKind::Struct);
        b.field(font_name, "family", FieldType::Native(Native::String), false, 1);
        let font_meta = b.define("FontMeta", DefKind::Message);
        b.field(font_meta, "key", FieldType::Def(font_name), false, 1);
        let glyph = b.define("Glyph", DefKind::Message);
        b.field(glyph, "commandsBlob", FieldType::Native(Native::Uint), false, 1);
        let derived = b.define("DerivedTextData", DefKind::Message);
        b.field(derived, "type", FieldType::Def(node_type), false, 1);
        b.field(derived, "fontName", FieldType::Def(font_name), false, 2);
        b.field(derived, "fontMetaData", FieldType::Def(font_meta), true, 3);
        b.field(derived, "glyphs", FieldType::Def(glyph), true, 4);
        b.field(derived, "derivedTextData", FieldType::Def(derived), false, 5);
        let vector = b.define("Vector", DefKind::Struct);
        b.field(vector, "x", FieldType::Native(Native::Float), false, 1);
        b.field(vector, "y", FieldType::Native(Native::Float), false, 2);
        b.field(derived, "layoutSize", FieldType::Def(vector), false, 6);
        let node_change = b.define("NodeChange", DefKind::Message);
        b.field(node_change, "type", FieldType::Def(node_type), false, 1);
        b.field(node_change, "fontName", FieldType::Def(font_name), false, 2);
        b.field(node_change, "derivedTextData", FieldType::Def(derived), false, 3);
        b.field(node_change, "name", FieldType::Native(Native::String), false, 4);
        let root = b.define("Message", DefKind::Message);
        b.field(root, "nodeChanges", FieldType::Def(node_change), true, 1);
        b.field(root, "pad", FieldType::Native(Native::Byte), true, 2);
        let schema = b.build();
        let family = "Some Icon Font";
        let key = || KiwiValue::Struct(vec![("family".into(), KiwiValue::String(family.into()))]);
        let level = |glyphs: usize, inner: Option<KiwiValue>| {
            let mut fields = vec![
                ("type".into(), KiwiValue::Enum("TEXT".into())),
                ("fontName".into(), key()),
                ("fontMetaData".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("key".into(), key())])])),
                ("glyphs".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("commandsBlob".into(), KiwiValue::Uint(7))]); glyphs])),
                ("layoutSize".into(), KiwiValue::Struct(vec![("x".into(), KiwiValue::Float(20.0)), ("y".into(), KiwiValue::Float(20.0))])),
            ];
            fields.extend(inner.map(|inner| ("derivedTextData".into(), inner)));
            KiwiValue::Message(fields)
        };
        let mut nested = level(20, None);
        for _ in 0..depth {
            nested = level(20, Some(nested));
        }
        let text = |name: &str, derived: KiwiValue| {
            KiwiValue::Message(vec![
                ("type".into(), KiwiValue::Enum("TEXT".into())),
                ("fontName".into(), key()),
                ("derivedTextData".into(), derived),
                ("name".into(), KiwiValue::String(name.into())),
            ])
        };
        let changes = vec![text("nested", nested), text("later", level(20, None)), text("last", level(20, None))];
        let message = KiwiValue::Message(vec![("nodeChanges".into(), KiwiValue::Array(changes)), ("pad".into(), KiwiValue::Bytes(vec![0; 2_100_000]))]);
        let bytes = schema.encode_message(root, &message).unwrap();
        (schema, root, bytes)
    }

    /// A heavy payload with an instance whose `derivedSymbolData` holds one entry for each of `entries` (the glyph count of the
    /// entry's derived text), followed by a text of `own` glyphs, each followed by a `name` that a desynchronised stream would misread.
    fn nested_payload(entries: &[usize], own: usize) -> (Schema, usize, Vec<u8>) {
        let mut b = SchemaBuilder::new();
        let node_type = b.define("NodeType", DefKind::Enum);
        b.variant(node_type, "FRAME", 0);
        b.variant(node_type, "TEXT", 1);
        b.variant(node_type, "INSTANCE", 2);
        let font_name = b.define("FontName", DefKind::Struct);
        b.field(font_name, "family", FieldType::Native(Native::String), false, 1);
        let font_meta = b.define("FontMeta", DefKind::Message);
        b.field(font_meta, "key", FieldType::Def(font_name), false, 1);
        let glyph = b.define("Glyph", DefKind::Message);
        b.field(glyph, "commandsBlob", FieldType::Native(Native::Uint), false, 1);
        let vector = b.define("Vector", DefKind::Struct);
        b.field(vector, "x", FieldType::Native(Native::Float), false, 1);
        b.field(vector, "y", FieldType::Native(Native::Float), false, 2);
        let derived = b.define("DerivedTextData", DefKind::Message);
        b.field(derived, "fontMetaData", FieldType::Def(font_meta), true, 1);
        b.field(derived, "glyphs", FieldType::Def(glyph), true, 2);
        b.field(derived, "layoutSize", FieldType::Def(vector), false, 3);
        let node_change = b.define("NodeChange", DefKind::Message);
        b.field(node_change, "type", FieldType::Def(node_type), false, 1);
        b.field(node_change, "derivedTextData", FieldType::Def(derived), false, 3);
        b.field(node_change, "name", FieldType::Native(Native::String), false, 4);
        b.field(node_change, "derivedSymbolData", FieldType::Def(node_change), true, 5);
        let root = b.define("Message", DefKind::Message);
        b.field(root, "nodeChanges", FieldType::Def(node_change), true, 1);
        b.field(root, "pad", FieldType::Native(Native::Byte), true, 2);
        let schema = b.build();
        let derived_value = |glyphs: usize| {
            let key = KiwiValue::Struct(vec![("family".into(), KiwiValue::String("Some Icon Font".into()))]);
            KiwiValue::Message(vec![
                ("layoutSize".into(), KiwiValue::Struct(vec![("x".into(), KiwiValue::Float(20.0)), ("y".into(), KiwiValue::Float(20.0))])),
                ("fontMetaData".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("key".into(), key)])])),
                ("glyphs".into(), KiwiValue::Array(vec![KiwiValue::Message(vec![("commandsBlob".into(), KiwiValue::Uint(7))]); glyphs])),
            ])
        };
        let entry = |glyphs: usize, i: usize| {
            KiwiValue::Message(vec![("derivedTextData".into(), derived_value(glyphs)), ("name".into(), KiwiValue::String(format!("entry {i}")))])
        };
        let instance = KiwiValue::Message(vec![
            ("type".into(), KiwiValue::Enum("INSTANCE".into())),
            ("derivedSymbolData".into(), KiwiValue::Array(entries.iter().enumerate().map(|(i, &g)| entry(g, i)).collect())),
            ("name".into(), KiwiValue::String("after the instance".into())),
        ]);
        let text = KiwiValue::Message(vec![
            ("type".into(), KiwiValue::Enum("TEXT".into())),
            ("derivedTextData".into(), derived_value(own)),
            ("name".into(), KiwiValue::String("after the text".into())),
        ]);
        let message =
            KiwiValue::Message(vec![("nodeChanges".into(), KiwiValue::Array(vec![instance, text])), ("pad".into(), KiwiValue::Bytes(vec![0; 2_100_000]))]);
        let bytes = schema.encode_message(root, &message).unwrap();
        (schema, root, bytes)
    }

    /// How many entries of the instance (the first change) kept their derived text, and whether the text (the second) kept its own.
    fn kept_nested(message: &KiwiValue) -> (usize, bool) {
        let changes = message.get("nodeChanges").unwrap().as_array().unwrap();
        let entries = changes[0].get("derivedSymbolData").unwrap().as_array().unwrap();
        for e in entries {
            assert!(e.get("derivedTextData").is_none_or(|d| matches!(d, KiwiValue::Bytes(_))), "a kept entry holds wire bytes");
        }
        (entries.iter().filter(|e| e.get("derivedTextData").is_some()).count(), changes[1].get("derivedTextData").is_some())
    }

    #[test]
    fn the_texts_of_instances_have_an_allowance_of_their_own() {
        let (s, root, bytes) = nested_payload(&[30, 30, 30], 30);
        let decode = |text: usize, nested: Option<usize>| s.decode_message_for_import_counting_nested(root, &bytes, MAX_DECODED_VALUES, text, nested).unwrap();
        let big = 1 << 30;
        let (message, dropped) = decode(big, Some(big));
        assert_eq!((kept_nested(&message), dropped), ((3, true), 0), "everything fits");
        // with none for the instances' texts, all three are counted and the text keeps its own
        let (message, dropped) = decode(big, Some(0));
        assert_eq!((kept_nested(&message), dropped), ((0, true), 3));
        // with none for the texts, the instances' texts are not affected and the text is the one counted
        let (message, dropped) = decode(0, Some(big));
        assert_eq!((kept_nested(&message), dropped), ((3, false), 1));
        // the allowance is on wire bytes, first come first kept, and one byte less leaves the last one out
        let three = (0..4000).find(|&a| kept_nested(&decode(big, Some(a)).0).0 == 3).expect("three entries fit some allowance");
        assert_eq!(kept_nested(&decode(big, Some(three - 1)).0).0, 2, "the third does not fit");
        let (message, dropped) = decode(big, Some(three - 1));
        assert_eq!(dropped, 1);
        let changes = message.get("nodeChanges").unwrap().as_array().unwrap();
        assert_eq!(changes[0].get("name"), Some(&KiwiValue::String("after the instance".into())), "the stream stays in step past a skipped entry");
        assert_eq!(changes[1].get("name"), Some(&KiwiValue::String("after the text".into())));
    }

    #[test]
    fn without_an_allowance_of_their_own_the_texts_of_instances_draw_on_the_texts() {
        let (s, root, bytes) = nested_payload(&[30, 30], 30);
        let decode = |text: usize| s.decode_message_for_import_counting_nested(root, &bytes, MAX_DECODED_VALUES, text, None).unwrap();
        let (message, dropped) = decode(1 << 30);
        assert_eq!((kept_nested(&message), dropped), ((2, true), 0));
        // the entries come first in the file and take what they need; the text, last, is the one that no longer fits
        let all = (0..8000).find(|&a| kept_nested(&decode(a).0) == (2, true)).expect("everything fits some allowance");
        let (message, dropped) = decode(all - 1);
        assert_eq!((kept_nested(&message), dropped), ((2, false), 1), "one byte short of the sum: the text is left out, not the entries");
        let (message, dropped) = decode(0);
        assert_eq!((kept_nested(&message), dropped), ((0, false), 3), "no allowance keeps none of them");
    }

    #[test]
    fn a_paste_keeps_the_texts_of_its_instances_on_its_one_allowance() {
        let (s, root, bytes) = nested_payload(&[30], 30);
        let (message, _) = s.decode_message_for_import_counting(root, &bytes, MAX_DECODED_VALUES, stored_glyphs::ALLOWANCE_PASTE_WIRE_BYTES).unwrap();
        assert_eq!(kept_nested(&message), (1, true));
        let (message, dropped) = s.decode_message_for_import_counting(root, &bytes, MAX_DECODED_VALUES, 0).unwrap();
        assert_eq!((kept_nested(&message), dropped), ((0, false), 2), "the texts of instances are not kept on top of the 4 MiB");
    }

    #[test]
    fn kept_nested_spans_count_toward_the_value_cap_and_a_light_decode_keeps_its_entries_whole() {
        let (s, root, bytes) = nested_payload(&[400, 400], 1);
        let with_nested = |cap: usize| s.decode_message_for_import_counting_nested(root, &bytes, cap, 1 << 30, Some(1 << 30));
        let skipping = fewest_values(|cap| s.decode_message_for_import_counting_nested(root, &bytes, cap, 1 << 30, Some(0)));
        let needed = fewest_values(with_nested);
        assert!(needed > skipping, "{needed} values with the entries' texts kept, {skipping} without");
        assert!(matches!(with_nested(skipping), Err(e) if e.to_string().contains("decoded value count exceeds budget")));
        // under 2 MB the decode is not heavy: the entries' texts stay the decoded values they are
        let mut small = s.decode_message(root, &bytes).unwrap();
        let KiwiValue::Array(changes) = small.get_mut("nodeChanges").unwrap() else { unreachable!() };
        let entries = changes[0].get("derivedSymbolData").unwrap().as_array().unwrap();
        assert!(entries.iter().all(|e| matches!(e.get("derivedTextData"), Some(KiwiValue::Message(_)))));
    }

    #[test]
    fn a_self_nesting_schema_cannot_wrap_the_allowance() {
        let (s, root, bytes) = self_nesting_payload(2);
        let decode = |allowance: usize| s.decode_message_for_import_counting(root, &bytes, MAX_DECODED_VALUES, allowance);
        // the outer field of the first text is its widest; the allowance that just holds it is the one the nested levels
        // would push below zero if they were charged again
        let first_kept = |a: usize| decode(a).unwrap().0.get("nodeChanges").unwrap().as_array().unwrap()[0].get("derivedTextData").is_some();
        let outer = (0..100_000).find(|&a| first_kept(a)).expect("the nested text fits some allowance");
        let (message, dropped) = decode(outer).unwrap();
        assert_eq!(kept_text_count(&message), 1, "only the first text fits an allowance that just holds it");
        assert_eq!(dropped, 2, "the two later texts are left out and counted, however the nested levels were charged");
        let changes = message.get("nodeChanges").unwrap().as_array().unwrap();
        assert!(changes[1].get("derivedTextData").is_none() && changes[2].get("derivedTextData").is_none());
        // a kept field is read without the heavy rules (its nested levels are neither kept nor charged again), and what is
        // retained is its wire bytes in Studio's schema, which has no nested field
        assert!(matches!(changes[0].get("derivedTextData"), Some(KiwiValue::Bytes(_))));
        // and a very large allowance still keeps all three
        assert_eq!(kept_text_count(&decode(1 << 30).unwrap().0), 3);
    }

    #[test]
    fn a_struct_chain_cannot_run_past_the_value_cap_inside_a_kept_span() {
        // 20,000 elements of a 51-level struct chain: about a million values from about 20 KB of wire, under a 65,536 cap
        let (s, root, bytes) = hostile_chain_payload(50, 20_000);
        let result = s.decode_message_for_import_counting(root, &bytes, 1 << 16, 1 << 30);
        assert!(matches!(&result, Err(e) if *e == KiwiError::Bad("decoded value count exceeds budget".into())), "the value cap must hold inside the span");
        // the same payload with the glyph data skipped is far under the cap, so the span is what the cap stopped
        assert!(s.decode_message_for_import_counting(root, &bytes, 1 << 16, 0).is_ok());
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

    #[test]
    fn ac01_u64_max_and_bit63_roundtrip_in_9_bytes() {
        for v in [u64::MAX, 0x8000_0000_0000_0001u64, 0xa3c4_d2ae_ffed_a986u64] {
            let mut w = Writer::default();
            w.var_u64(v);
            assert_eq!(w.buf.len(), 9, "value {v:#x} must encode in 9 bytes");

            let mut trailed = w.buf.clone();
            trailed.push(0x2a);
            let mut r = Reader::new(&trailed);
            assert_eq!(r.var_u64().unwrap(), v);
            assert_eq!(r.pos, 9, "reader must advance 9 bytes for {v:#x}");
            assert_eq!(r.byte().unwrap(), 0x2a);
        }
    }

    #[test]
    fn ac02_var_u64_boundary_byte_lengths() {
        let cases = [(0u64, 1usize), (127, 1), (128, 2), ((1u64 << 56) - 1, 8), (1u64 << 56, 9)];
        for (v, expected_len) in cases {
            let mut w = Writer::default();
            w.var_u64(v);
            assert_eq!(w.buf.len(), expected_len, "value {v:#x}");
            let mut r = Reader::new(&w.buf);
            assert_eq!(r.var_u64().unwrap(), v);
            assert_eq!(r.pos, expected_len);
            assert!(r.at_end());
        }
    }

    #[test]
    fn ac03_field_after_9_byte_varint_decodes_correctly() {
        let mut b = SchemaBuilder::new();
        let nc = b.define("NodeChange", DefKind::Message);
        b.field(nc, "name", FieldType::Native(Native::String), false, 5);
        b.field(nc, "strokeSeed", FieldType::Native(Native::Uint64), false, 482);
        let lib_move = b.define("LibraryMoveInfo", DefKind::Message);
        b.field(lib_move, "oldKey", FieldType::Native(Native::String), false, 1);
        let root = b.define("Message", DefKind::Message);
        b.field(root, "nodeChanges", FieldType::Def(nc), true, 1);
        b.field(root, "libraryMoveInfo", FieldType::Def(lib_move), false, 2);
        let s = b.build();

        // Construct a wire message where strokeSeed (field 482 = 0xe2 0x03) is a 9-byte Kiwi varint
        // with bit 63 set, followed immediately by name (field 5) and libraryMoveInfo (field 2).
        let mut wire = vec![
            1, // Message.nodeChanges
            1, // array len = 1
            0xe2, 0x03, // NodeChange.strokeSeed (id 482)
            0x86, 0xc4, 0xa9, 0xed, 0xff, 0xae, 0xd2, 0xc4, 0xa3, // 9-byte varint with bit 63 set
            5,    // NodeChange.name (id 5)
        ];
        wire.extend_from_slice(b"StrokeNode\0");
        wire.push(0); // end NodeChange
        wire.extend_from_slice(&[2, 1]); // Message.libraryMoveInfo, LibraryMoveInfo.oldKey
        wire.extend_from_slice(b"k1\0");
        wire.extend_from_slice(&[0, 0]); // end LibraryMoveInfo, end Message

        let decoded = s.decode_message(root, &wire).expect("9-byte varint must not shift following fields");
        let changes = decoded.get("nodeChanges").unwrap().as_array().unwrap();
        assert_eq!(changes[0].get("name"), Some(&KiwiValue::String("StrokeNode".into())));
        assert_eq!(decoded.get("libraryMoveInfo").and_then(|m| m.get("oldKey")), Some(&KiwiValue::String("k1".into())));
    }

    #[test]
    fn ac04_fixture_with_bit63_stroke_seed_imports_and_roundtrips() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/stroke-seed-bit63.fig");
        let doc = crate::import_fig(FIXTURE).expect("import stroke-seed-bit63.fig");
        assert_eq!(doc.nodes.len(), 4);
        let re_exported = crate::export_fig(&doc).expect("export_fig round-trip");
        let roundtrip = crate::import_fig(&re_exported).expect("re-import exported .fig");
        assert_eq!(roundtrip.nodes.len(), 4);
    }

    #[test]
    fn ac05_truncated_9_byte_varint_returns_error_without_panic() {
        let full = [0xffu8; 9];
        for len in 0..9 {
            let mut r = Reader::new(&full[..len]);
            assert_eq!(r.var_u64(), Err(KiwiError::Eof(len)));
        }

        let mut b = SchemaBuilder::new();
        let nc = b.define("NodeChange", DefKind::Message);
        b.field(nc, "strokeSeed", FieldType::Native(Native::Uint64), false, 482);
        let s = b.build();
        let msg = KiwiValue::Message(vec![("strokeSeed".into(), KiwiValue::Uint64(u64::MAX))]);
        let bytes = s.encode_message(nc, &msg).unwrap();
        assert_eq!(bytes.len(), 2 + 9 + 1);
        assert_eq!(s.decode_message(nc, &bytes).unwrap(), msg);
        for len in 0..bytes.len() {
            assert!(s.decode_message(nc, &bytes[..len]).is_err(), "truncated len {len} must fail");
        }
        // Truncated 8-byte varint followed by message terminator (eaten as 9th byte) and bogus field id 125.
        let malformed = [0xe2, 0x03, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 125, 0x00];
        let err = s.decode_message(nc, &malformed).unwrap_err();
        assert!(err.to_string().contains("malformed data: unknown field id 125"), "{err}");
    }

    #[test]
    fn edge_case_2_pow_63_high_byte_is_data_not_continuation() {
        let v = 1u64 << 63;
        let mut w = Writer::default();
        w.var_u64(v);
        assert_eq!(w.buf, [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80]);

        let wire = [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x05];
        let mut r = Reader::new(&wire);
        assert_eq!(r.var_u64().unwrap(), v);
        assert_eq!(r.pos, 9);
        assert_eq!(r.byte().unwrap(), 0x05);
    }

    #[test]
    fn edge_case_zigzag_var_i64_min_and_max() {
        let mut w_min = Writer::default();
        w_min.var_i64(i64::MIN);
        assert_eq!(w_min.buf, [0xff; 9]);
        let mut r_min = Reader::new(&w_min.buf);
        assert_eq!(r_min.var_i64().unwrap(), i64::MIN);
        assert!(r_min.at_end());

        let mut w_max = Writer::default();
        w_max.var_i64(i64::MAX);
        assert_eq!(w_max.buf, [0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        let mut r_max = Reader::new(&w_max.buf);
        assert_eq!(r_max.var_i64().unwrap(), i64::MAX);
        assert!(r_max.at_end());
    }

    #[test]
    fn edge_case_10_byte_varint_from_old_build_returns_error_without_panic() {
        let mut b = SchemaBuilder::new();
        let nc = b.define("NodeChange", DefKind::Message);
        b.field(nc, "strokeSeed", FieldType::Native(Native::Uint64), false, 482);
        let s = b.build();

        // Old Studio builds encoded bit-63 u64 values as 10-byte LEB128 (9 continuation bytes + 0x01).
        let old_10_byte_wire = [0xe2, 0x03, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0x00];
        let err = s.decode_message(nc, &old_10_byte_wire).unwrap_err();
        assert!(err.to_string().contains("malformed data: unknown field id 1 in NodeChange"), "{err}");
    }

    #[test]
    fn field_from_json_reconstructs_code_component_id_and_prop_assignments() {
        let s = crate::testkit::figma_like_schema();
        let effect_idx = s.find("Effect").expect("Effect in schema");
        let custom_eff_field = s.defs[effect_idx].fields.iter().find(|f| &*f.name == "customEffectId").expect("customEffectId field");
        let props_field = s.defs[effect_idx].fields.iter().find(|f| &*f.name == "componentPropAssignments").expect("componentPropAssignments field");

        let custom_eff_json = serde_json::json!({
            "guid": {"sessionID": 10, "localID": 25},
            "assetRef": {"key": "shader_key_abc", "version": "1"}
        });
        let custom_eff_kv = s.field_from_json(custom_eff_field, &custom_eff_json, 0).expect("CodeComponentId from json");
        assert_eq!(to_json(&custom_eff_kv), custom_eff_json);

        let props_json = serde_json::json!([
            {
                "defID": {"sessionID": 1, "localID": 2},
                "value": {"floatValue": 0.75}
            },
            {
                "defID": {"sessionID": 1, "localID": 3},
                "value": {"boolValue": true}
            }
        ]);
        let props_kv = s.field_from_json(props_field, &props_json, 0).expect("ComponentPropAssignment[] from json");
        assert_eq!(to_json(&props_kv), props_json);

        let bad_custom_eff = serde_json::json!({
            "guid": {"sessionID": "not-a-uint", "localID": 25},
            "assetRef": {"key": "shader_key_abc", "version": "1"}
        });
        assert!(s.field_from_json_strict(custom_eff_field, &bad_custom_eff, 0).is_none());
        // the lenient conversion keeps what converts, as it does for a passthrough value
        let kept = s.field_from_json(custom_eff_field, &bad_custom_eff, 0).expect("the asset reference converts");
        assert_eq!(to_json(&kept), serde_json::json!({"assetRef": {"key": "shader_key_abc", "version": "1"}}));
    }
    #[test]
    fn first_unknown_json_key_names_the_path_of_a_key_the_schema_does_not_name() {
        use serde_json::json;
        let s = crate::testkit::figma_like_schema();
        let effect = s.find("Effect").expect("Effect in schema");
        let field = |name: &str| s.defs[effect].fields.iter().find(|f| &*f.name == name).unwrap_or_else(|| panic!("{name} field"));
        let unknown = |name: &str, v: serde_json::Value| s.first_unknown_json_key(field(name), &v);

        let id = json!({"guid": {"sessionID": 1, "localID": 2}, "assetRef": {"key": "k", "version": "1"}});
        assert_eq!(unknown("customEffectId", id.clone()), None, "every key is named");
        // a message, a struct inside a message, and a struct on its own
        assert_eq!(
            unknown("customEffectId", json!({"guid": {"sessionID": 1, "localID": 2}, "futureFlag": true})).as_deref(),
            Some("customEffectId.futureFlag")
        );
        assert_eq!(unknown("customEffectId", json!({"guid": {"sessionID": 1, "localID": 2, "epoch": 3}})).as_deref(), Some("customEffectId.guid.epoch"));
        assert_eq!(unknown("offset", json!({"x": 1.0, "y": 2.0, "future": 1})).as_deref(), Some("offset.future"));
        // an array of messages names the index; a message nested in a message is reached
        let props = json!([
            {"defID": {"sessionID": 1, "localID": 2}, "value": {"floatValue": 0.5}},
            {"defID": {"sessionID": 1, "localID": 3}, "value": {"boolValue": true, "future": 1}}
        ]);
        assert_eq!(unknown("componentPropAssignments", props).as_deref(), Some("componentPropAssignments[1].value.future"));
        // a null value carries nothing, a GUID string and a scalar have no keys to read
        assert_eq!(unknown("customEffectId", json!({"guid": "1:2", "futureFlag": null})), None);
        assert_eq!(unknown("radius", json!(4.0)), None);
        assert_eq!(unknown("visible", serde_json::Value::Null), None);
    }
}
