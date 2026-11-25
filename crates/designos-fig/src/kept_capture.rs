//! What a `.fig` open keeps of Figma's own records while the decoder reads them.
//!
//! The decoder reports, for each record of `nodeChanges`, where each field's value sits in the inflated message payload,
//! and for each entry of the blob table, where its bytes sit. The payload itself is kept whole (see
//! [`designos_doc::KeptArena`]), so a kept record is a list of ranges and nothing is copied.
//!
//! A field is kept only when the file's definition of it is the one Studio writes with ([`Schema::same_definition`]): its
//! bytes can then be written back as they are. A field of any other definition is left out and counted, so a short or
//! renumbered copy never stands in for Figma's value.

use crate::fig_export::export_schema;
use crate::kiwi::{FieldType, KiwiValue, RawSpan, Schema};
use designos_doc::{BlobSlot, FigKeptStore, KeptArena, KeptCounts, KeptData, KeptField, KeptRecord, KeptRecordKind};
use std::path::PathBuf;

/// The kill switch: with it off the open keeps nothing and the document saves exactly as it did before the store existed.
pub const KEPT_CAPTURE_ENABLED: bool = true;

/// Most wire bytes of kept fields in one record. The average record of the largest file measured is about 1.2 KB; one past
/// this is hostile or exotic, and is left out and counted.
pub const MAX_KEPT_RECORD_BYTES: usize = 4 << 20;

/// Most fields kept for one record. `NodeChange` has 616 fields, so a record past this repeats fields.
pub const MAX_KEPT_FIELDS_PER_RECORD: usize = 1024;

/// The disk budget of one open file: the largest inflated payload kept. The kit's payload is 193 MB for 119,748 records
/// (1.6 KB a record), so the largest file measured, 1,058,895 records, is about 1.7 GB; this leaves it 1.75 times as much. A
/// payload past it keeps nothing and every record of it is counted. Offsets are 32 bits, which bounds the budget below 4 GiB.
pub const MAX_KEPT_ARENA_BYTES: usize = 3 << 30;

/// The payload size from which the arena is written to a file and mapped instead of held in memory: 512 MiB is 2.8 times
/// the kit's payload, so the kit stays in memory, and the largest file measured (about 1.7 GB) is mapped.
pub const SPILL_THRESHOLD_BYTES: usize = 512 << 20;

/// Wire bytes that pay for one kept field range. A file cannot make the ranges outweigh the payload by more than
/// `12 / 8` (a range is 12 bytes): a record that would pass it is left out and counted.
const WIRE_BYTES_PER_FIELD_RANGE: usize = 8;

/// Fields of a layer record the tree writer owns, and the stored glyphs, which have a rule of their own.
const LAYER_FIELDS_NOT_KEPT: [&str; 4] = ["guid", "phase", "parentIndex", "derivedTextData"];

/// Where the arena goes when it spills, and from which size.
#[derive(Clone, Debug)]
pub struct KeptOptions {
    pub spill_threshold_bytes: usize,
    pub spill_dir: PathBuf,
}

impl Default for KeptOptions {
    fn default() -> Self {
        Self { spill_threshold_bytes: SPILL_THRESHOLD_BYTES, spill_dir: default_spill_dir() }
    }
}

static SPILL_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Sets the directory an arena that passes [`SPILL_THRESHOLD_BYTES`] is written to (the session directory of the app). Until
/// it is set, the spill goes to a folder of the system's temporary directory.
pub fn set_kept_spill_dir(dir: PathBuf) {
    *SPILL_DIR.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
}

fn default_spill_dir() -> PathBuf {
    SPILL_DIR.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone().unwrap_or_else(|| std::env::temp_dir().join("designos-kept"))
}

/// Which field of the root message a captured array is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    NodeChanges,
    Blobs,
}

/// The state of a capture while the decoder runs.
pub(crate) struct Capture {
    enabled: bool,
    /// Ranges the fields of every record may add up to.
    field_budget: usize,
    records: Vec<KeptRecord>,
    fields: Vec<KeptField>,
    blobs: Vec<BlobSlot>,
    counts: KeptCounts,
    /// Per field number of the file's `NodeChange`: 0 not worked out, 1 the definition is Studio's, 2 it is not.
    conformity: Vec<u8>,
    /// Studio's `NodeChange`, in its own export schema.
    export_node_change: Option<usize>,
    /// The caps the decoder had left when it finished, and what keeping the heavy fields would charge beyond a plain decode.
    values_left: usize,
    zero_wire_left: usize,
    extra_values: usize,
    extra_zero_wire: usize,
    /// Positions in `fields` of the heavy fields kept.
    heavy: Vec<usize>,
}

/// What the capture holds once the decode is done.
pub(crate) struct KeptCapture {
    pub records: Vec<KeptRecord>,
    pub fields: Vec<KeptField>,
    pub blobs: Vec<BlobSlot>,
    pub counts: KeptCounts,
}

impl Capture {
    /// A capture of a payload of `arena_len` bytes. One past [`MAX_KEPT_ARENA_BYTES`] (or with the kill switch off) keeps
    /// nothing but still counts the records it sees.
    pub(crate) fn new(arena_len: usize) -> Self {
        Self {
            enabled: KEPT_CAPTURE_ENABLED && arena_len <= MAX_KEPT_ARENA_BYTES,
            field_budget: arena_len / WIRE_BYTES_PER_FIELD_RANGE + 1,
            records: Vec::new(),
            fields: Vec::new(),
            blobs: Vec::new(),
            counts: KeptCounts::default(),
            conformity: Vec::new(),
            export_node_change: export_schema().find("NodeChange"),
            values_left: 0,
            zero_wire_left: 0,
            extra_values: 0,
            extra_zero_wire: 0,
            heavy: Vec::new(),
        }
    }

    /// The decode is done: `values_left` and `zero_wire_left` are what its caps have left, `extra_values` and `extra_zero_wire`
    /// what keeping the heavy fields costs beyond a decode without a capture. Kept spans charge the caps as a decode of them
    /// would, once the rest has been charged: when the sum fits they are charged, and when it does not they are all left out
    /// and counted, so the capture never makes a file that opens fail.
    pub(crate) fn decode_finished(&mut self, values_left: usize, zero_wire_left: usize, extra_values: usize, extra_zero_wire: usize) {
        self.values_left = values_left;
        self.zero_wire_left = zero_wire_left;
        self.extra_values = extra_values;
        self.extra_zero_wire = extra_zero_wire;
    }

    pub(crate) fn role(name: &str) -> Option<Role> {
        match name {
            "nodeChanges" => Some(Role::NodeChanges),
            "blobs" => Some(Role::Blobs),
            _ => None,
        }
    }

    pub(crate) fn finish(mut self) -> KeptCapture {
        let fits = self.extra_values <= self.values_left && self.extra_zero_wire <= self.zero_wire_left;
        if !fits && !self.heavy.is_empty() {
            self.withdraw_heavy_fields();
        }
        let mut counts = self.counts;
        counts.decode_values_left = self.values_left;
        counts.kept_extra_values_charged = if fits { self.extra_values } else { 0 };
        counts.records_kept = self.records.len();
        counts.fields_kept = self.fields.len();
        KeptCapture { records: self.records, fields: self.fields, blobs: self.blobs, counts }
    }

    /// Takes the heavy fields out of the field table and counts them.
    fn withdraw_heavy_fields(&mut self) {
        let heavy: std::collections::HashSet<usize> = self.heavy.drain(..).collect();
        self.counts.kept_fields_over_decode_cap += heavy.len();
        let old = std::mem::take(&mut self.fields);
        let mut fields = Vec::with_capacity(old.len() - heavy.len());
        for rec in &mut self.records {
            let first = rec.first_index();
            let count = rec.field_count();
            let new_first = fields.len();
            fields.extend(old[first..first + count].iter().enumerate().filter(|(i, _)| !heavy.contains(&(first + i))).map(|(_, f)| *f));
            *rec = KeptRecord::new(rec.guid, rec.kind, new_first as u32, (fields.len() - new_first) as u32);
        }
        self.fields = fields;
    }

    /// Reports one record of `nodeChanges`: `val` as decoded, `spans` its fields as the decoder read them, `def` the
    /// file's `NodeChange`.
    pub(crate) fn node_change(&mut self, file: &Schema, def: usize, val: &KiwiValue, spans: &[RawSpan], heavy_ids: &[u32]) {
        self.counts.records_seen += 1;
        let Some(guid) = guid_of(val) else {
            self.counts.dropped_kept_records += 1;
            return;
        };
        if !self.enabled {
            self.counts.dropped_kept_records += 1;
            return;
        }
        let kind = kind_of(val);
        let first = self.fields.len();
        let mut bytes = 0usize;
        for span in spans {
            let Some(name) = field_name(file, def, span.id) else { continue };
            if kind == KeptRecordKind::Layer && LAYER_FIELDS_NOT_KEPT.contains(&name) {
                continue;
            }
            if !self.conforms(file, def, span.id) {
                self.counts.kept_fields_not_conformed += 1;
                continue;
            }
            let len = span.end.saturating_sub(span.start);
            bytes = bytes.saturating_add(len as usize);
            if heavy_ids.contains(&span.id) {
                self.heavy.push(self.fields.len());
            }
            self.fields.push(KeptField { no: span.id, offset: span.start, len });
            if bytes > MAX_KEPT_RECORD_BYTES || self.fields.len() - first > MAX_KEPT_FIELDS_PER_RECORD {
                break;
            }
        }
        let count = self.fields.len() - first;
        let over = bytes > MAX_KEPT_RECORD_BYTES || count > MAX_KEPT_FIELDS_PER_RECORD || self.fields.len() > self.field_budget;
        let (Ok(first32), Ok(count32), false) = (u32::try_from(first), u32::try_from(count), over) else {
            self.fields.truncate(first);
            self.heavy.retain(|&i| i < first);
            self.counts.dropped_kept_records += 1;
            return;
        };
        self.records.push(KeptRecord::new(guid, kind, first32, count32));
    }

    /// Reports one entry of the blob table: `val` is the `Blob` as decoded (definition `def` of the file's schema; a struct in
    /// Figma's schema, a message in another), `spans` its fields when it is a message, `after` where the decoder stood when
    /// it finished the entry. The slot is where the bytes sit, after the length that precedes them.
    pub(crate) fn blob(&mut self, file: &Schema, def: usize, val: &KiwiValue, spans: &[RawSpan], after: usize) {
        let end = file.defs[def].field_named("bytes").and_then(|f| spans.iter().find(|s| s.id == f.value)).map_or(after, |s| s.end as usize);
        let slot = match (self.enabled, val.get("bytes").and_then(KiwiValue::as_bytes)) {
            (true, Some(bytes)) if bytes.len() <= end => BlobSlot { offset: (end - bytes.len()) as u32, len: bytes.len() as u32 },
            _ => BlobSlot { offset: 0, len: 0 },
        };
        self.blobs.push(slot);
    }

    /// Whether the file's definition of field `no` of `NodeChange` is the one Studio writes with, so its bytes are Studio's
    /// bytes. Worked out once per field number.
    fn conforms(&mut self, file: &Schema, def: usize, no: u32) -> bool {
        let at = no as usize;
        if at >= file.defs[def].field_by_value.len() {
            return false;
        }
        if self.conformity.is_empty() {
            self.conformity = vec![0; file.defs[def].field_by_value.len()];
        }
        if self.conformity[at] == 0 {
            self.conformity[at] = if self.work_out_conformity(file, def, no) { 1 } else { 2 };
        }
        self.conformity[at] == 1
    }

    fn work_out_conformity(&self, file: &Schema, def: usize, no: u32) -> bool {
        let export = export_schema();
        let (Some(nc), Some(field)) =
            (self.export_node_change, file.defs[def].field_by_value.get(no as usize).copied().flatten().map(|i| &file.defs[def].fields[i]))
        else {
            return false;
        };
        let Some(ours) = export.defs[nc].field_named(&field.name) else { return false };
        ours.value == field.value
            && ours.is_array == field.is_array
            && match (field.ty, ours.ty) {
                (FieldType::Native(a), FieldType::Native(b)) => a == b,
                (FieldType::Def(a), FieldType::Def(b)) => file.same_definition(a, export, b),
                _ => false,
            }
    }
}

fn field_name(file: &Schema, def: usize, no: u32) -> Option<&str> {
    let d = &file.defs[def];
    let idx = d.field_by_value.get(no as usize).copied().flatten().or_else(|| d.fields.iter().position(|f| f.value == no))?;
    Some(&d.fields[idx].name)
}

/// The record's guid as `(session, local)`.
fn guid_of(val: &KiwiValue) -> Option<(u32, u32)> {
    let g = val.get("guid")?;
    let part = |name: &str| match g.get(name)? {
        KiwiValue::Uint(n) => Some(*n),
        _ => None,
    };
    Some((part("sessionID")?, part("localID")?))
}

/// A layer, or any other record: a page, the document, a style, a variable, or one Figma soft-deleted.
fn kind_of(val: &KiwiValue) -> KeptRecordKind {
    let ty = match val.get("type") {
        Some(KiwiValue::Enum(name)) => &**name,
        _ => "",
    };
    let soft_deleted = |key: &str| matches!(val.get(key), Some(KiwiValue::Bool(true)));
    let other = matches!(ty, "DOCUMENT" | "CANVAS" | "VARIABLE" | "VARIABLE_SET" | "VARIABLE_OVERRIDE")
        || val.get("styleType").is_some()
        || soft_deleted("isSoftDeleted")
        || soft_deleted("isSoftDeletedStyle");
    if other {
        KeptRecordKind::Other
    } else {
        KeptRecordKind::Layer
    }
}

/// The store of a file import: the payload as the arena (written to a file and mapped past `opts.spill_threshold_bytes`) and
/// the ranges that were captured in it. A payload that cannot be spilled keeps nothing, and its records are counted.
pub(crate) fn build_store(mut payload: Vec<u8>, capture: KeptCapture, opts: &KeptOptions) -> FigKeptStore {
    let KeptCapture { records, fields, blobs, mut counts, .. } = capture;
    if records.is_empty() {
        return FigKeptStore::new(KeptData::new(KeptArena::default(), Vec::new(), Vec::new(), Vec::new(), counts));
    }
    let arena = if payload.len() > opts.spill_threshold_bytes {
        match KeptArena::on_disk(&payload, &opts.spill_dir) {
            Ok(arena) => arena,
            Err(_) => {
                counts.dropped_kept_records += counts.records_kept;
                counts.records_kept = 0;
                counts.fields_kept = 0;
                return FigKeptStore::new(KeptData::new(KeptArena::default(), Vec::new(), Vec::new(), Vec::new(), counts));
            }
        }
    } else {
        payload.shrink_to_fit();
        KeptArena::in_memory(std::mem::take(&mut payload))
    };
    drop(payload);
    FigKeptStore::new(KeptData::new(arena, records, fields, blobs, counts))
}
