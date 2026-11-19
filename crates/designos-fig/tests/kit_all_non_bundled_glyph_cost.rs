//! Worst case for the stored glyphs a heavy `.fig` import keeps: a real design kit whose texts are all set in a font
//! that is not bundled. Both tests are ignored measurements; they need a copy of the kit and print only counts, never
//! its content.
//!
//! ```text
//! KIT_CLONE=<kit .fig> KIT_OUT=<copy .fig> cargo test --release -p designos-fig --test kit_all_non_bundled_glyph_cost -- --ignored write_copy
//! KIT_OUT=<copy .fig> /usr/bin/time -l <test binary> --ignored --nocapture import_copy
//! ```
//!
//! The first rewrites every text's `fontName.family` and the family in its derived font list to a made-up name, so the
//! importer sees the glyphs as drawable. `write_rejected_copy` is the other worst case: the layer's family is made up
//! and the family stored in the derived font list is a different one, so the decoder reads every text's glyph data and
//! then rejects it. The last test imports a copy and prints open time, how many texts kept their glyphs and how many
//! the allowance left out. Run it with the test binary built from the branch and from its base, and compare the peak
//! resident size `time -l` reports.

use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{KiwiValue as V, Schema};

const MADE_UP_FAMILY: &str = "Zzz Not Installed Sans";

fn rewrite_family(holder: &mut V, name: &str) {
    if let Some(family) = holder.get_mut("family") {
        *family = V::String(name.into());
    }
}

/// Writes a copy of the kit whose texts all carry the made-up layer family and `stored_family` in their derived font list.
fn write_family_copy(stored_family: &str) {
    let src = std::env::var_os("KIT_CLONE").expect("KIT_CLONE");
    let out = std::env::var_os("KIT_OUT").expect("KIT_OUT");
    let bytes = std::fs::read(src).expect("read the kit clone");
    let archive = container::open(&bytes).expect("open");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("chunks");
    let schema = Schema::decode(&schema_bin).expect("schema");
    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message(root, &msg_bin).expect("decode");
    drop(msg_bin);
    let mut rewritten = 0usize;
    if let Some(V::Array(changes)) = message.get_mut("nodeChanges") {
        for change in changes {
            if !matches!(change.get("type"), Some(V::Enum(t)) if &**t == "TEXT") {
                continue;
            }
            if let Some(font) = change.get_mut("fontName") {
                rewrite_family(font, MADE_UP_FAMILY);
            }
            if let Some(V::Array(fonts)) = change.get_mut("derivedTextData").and_then(|d| d.get_mut("fontMetaData")) {
                for font in fonts {
                    if let Some(key) = font.get_mut("key") {
                        rewrite_family(key, stored_family);
                    }
                }
            }
            rewritten += 1;
        }
    }
    let body = schema.encode_message(root, &message).expect("encode");
    std::fs::write(out, container::write_container(&schema.encode(), &body, Compression::Zstd, 161)).expect("write the copy");
    eprintln!("MEASURE rewrote the family of {rewritten} texts");
}

#[test]
#[ignore = "measurement: needs KIT_CLONE and KIT_OUT"]
fn write_copy() {
    write_family_copy(MADE_UP_FAMILY);
}

#[test]
#[ignore = "measurement: needs KIT_CLONE and KIT_OUT"]
fn write_rejected_copy() {
    write_family_copy("Zzz Another Stored Face");
}

#[test]
#[ignore = "measurement: needs KIT_OUT"]
fn import_copy() {
    let bytes = std::fs::read(std::env::var_os("KIT_OUT").expect("KIT_OUT")).expect("read the copy");
    let started = std::time::Instant::now();
    let (doc, report) = designos_fig::import_fig_with_report(&bytes).expect("import");
    let secs = started.elapsed().as_secs_f32();
    let with_geometry = doc.nodes.values().filter(|n| n.fig_text_geometry.is_some()).count();
    let with_json = doc.nodes.values().filter(|n| n.fig_metadata.contains_key("derivedTextData")).count();
    let json_bytes: usize = doc.nodes.values().filter_map(|n| n.fig_metadata.get("derivedTextData")).map(String::len).sum();
    let glyphs: usize = doc.nodes.values().filter_map(|n| n.fig_text_geometry.as_deref()).map(|g| g.glyphs.len()).sum();
    let retained: usize = doc.nodes.values().filter_map(|n| n.fig_text_geometry.as_deref()).map(designos_doc::FigTextGeometry::retained_bytes).sum();
    eprintln!(
        "MEASURE import secs={secs:.2} nodes={} nodes_with_geometry={with_geometry} nodes_with_derived_json={with_json} json_bytes={json_bytes} glyphs={glyphs} retained_bytes={retained}",
        doc.nodes.len(),
    );
    eprintln!("MEASURE report: {report:?}");
}
