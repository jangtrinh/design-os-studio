//! Times native document JSON load and save and prints what the load repaired.
//!
//! ```text
//! cargo run --release -p designos-doc --example json_roundtrip_bench -- <file.designos> [load|save]
//! ```
//!
//! `load` (default) parses the file; `save` also serialises the parsed document again. Wrap it in
//! `/usr/bin/time -l` for the peak resident set size. The input is only read.

use designos_doc::Document;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: json_roundtrip_bench <file.designos> [load|save]");
    let mode = args.next().unwrap_or_else(|| "load".into());

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    println!("file: {} bytes", text.len());

    let started = Instant::now();
    let (doc, report) = Document::from_json_with_report(&text).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"));
    println!("load: {:?}, {} nodes, repair report: {report:?}", started.elapsed(), doc.nodes.len());
    drop(text);

    if mode == "save" {
        let started = Instant::now();
        let json = doc.to_json().expect("serialise");
        println!("save: {:?}, {} bytes", started.elapsed(), json.len());
    }
}
