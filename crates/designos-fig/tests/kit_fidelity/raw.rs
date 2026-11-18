//! Raw decode of a `.fig`: every `NodeChange` record with the heavy derived fields kept (`derivedTextData`,
//! `derivedSymbolData`, `fillGeometry`, `strokeGeometry`), and the blob table as bytes. This is the decode
//! `decode_fig_to_json` runs, kept as kiwi values so geometry blobs stay bytes instead of hex strings.

use designos_fig::container;
use designos_fig::kiwi::{KiwiValue, Schema};

pub struct Raw {
    pub records: Vec<KiwiValue>,
    pub blobs: Vec<Vec<u8>>,
}

pub fn decode(bytes: &[u8]) -> Raw {
    let mut archive = container::open(bytes).expect("open the container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read the chunks");
    archive.canvas = Vec::new();
    let schema = Schema::decode(&schema_bin).expect("decode the schema");
    drop(schema_bin);
    let root = schema.root_message().expect("root message");
    // Not `decode_message_for_import`: that one drops `derivedTextData` above 2 MB.
    let mut message = schema.decode_message(root, &msg_bin).expect("decode the message");
    drop(msg_bin);
    let blobs = match message.take_field("blobs") {
        Some(KiwiValue::Array(arr)) => arr
            .into_iter()
            .map(|mut b| match b.take_field("bytes") {
                Some(KiwiValue::Bytes(bytes)) => bytes,
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let records = match message.take_field("nodeChanges") {
        Some(KiwiValue::Array(arr)) => arr,
        _ => Vec::new(),
    };
    Raw { records, blobs }
}

impl Raw {
    /// Records that describe a live node (not tombstones).
    pub fn live(&self) -> impl Iterator<Item = &KiwiValue> {
        self.records.iter().filter(|r| !is_removed(r))
    }
}

pub fn is_removed(r: &KiwiValue) -> bool {
    field_str(r, "phase") == Some("REMOVED")
}

pub fn str_of(v: &KiwiValue) -> Option<&str> {
    match v {
        KiwiValue::String(s) => Some(s.as_str()),
        KiwiValue::Enum(s) => Some(s),
        _ => None,
    }
}

pub fn field_str<'a>(v: &'a KiwiValue, k: &str) -> Option<&'a str> {
    v.get(k).and_then(str_of)
}

/// How many live records carry each of the heavy derived fields, printed first so a part whose truth is absent
/// is known before anything is built on it.
pub fn presence(raw: &Raw) -> Vec<(&'static str, u64)> {
    let keys = ["derivedTextData", "derivedSymbolData", "fillGeometry", "strokeGeometry", "symbolData", "textData"];
    let mut counts = vec![0u64; keys.len()];
    let mut in_symbol_data = 0u64;
    for r in raw.live() {
        for (i, k) in keys.iter().enumerate() {
            if r.get(k).is_some() {
                counts[i] += 1;
            }
        }
        if r.get("symbolData").and_then(|s| s.get("derivedSymbolData")).is_some() {
            in_symbol_data += 1;
        }
    }
    let mut out: Vec<(&'static str, u64)> = keys.iter().copied().zip(counts).collect();
    out.push(("symbolData.derivedSymbolData", in_symbol_data));
    out.push(("live records", raw.live().count() as u64));
    out.push(("all records", raw.records.len() as u64));
    out.push(("blobs", raw.blobs.len() as u64));
    out
}
