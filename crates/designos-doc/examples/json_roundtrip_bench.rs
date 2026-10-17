//! Times native document JSON load and save and prints what the load repaired.
//!
//! ```text
//! cargo run --release -p designos-doc --example json_roundtrip_bench -- <file.designos> [load|save|compare]
//! ```
//!
//! `load` (default) parses the file; `save` also serialises the parsed document again; `compare`
//! runs three iterations each of plain `serde_json::from_str::<Document>` and
//! `finite_json::from_str_repairing::<Document>` (`Document::from_json_with_report`) to measure the
//! strict-pass wrapper overhead on large documents. Wrap it in `/usr/bin/time -l` for the peak
//! resident set size. The input is only read.

use designos_doc::{finite_json, Document};
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: json_roundtrip_bench <file.designos> [load|save|compare]");
    let mode = args.next().unwrap_or_else(|| "load".into());

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    println!("file: {} bytes", text.len());

    if mode == "compare" {
        let runs = 3usize;
        let mut plain_times = Vec::with_capacity(runs);
        let mut strict_times = Vec::with_capacity(runs);
        let mut node_count = 0usize;
        for run in 1..=runs {
            let t0 = Instant::now();
            let plain: Document = serde_json::from_str(&text).unwrap_or_else(|e| panic!("serde_json cannot parse {path}: {e}"));
            let dt_plain = t0.elapsed();
            node_count = plain.nodes.len();
            drop(plain);

            let t1 = Instant::now();
            let (strict, replaced): (Document, usize) =
                finite_json::from_str_repairing(&text).unwrap_or_else(|e| panic!("finite_json cannot parse {path}: {e}"));
            let dt_strict = t1.elapsed();
            assert_eq!(strict.nodes.len(), node_count);
            assert_eq!(replaced, 0);
            drop(strict);

            println!(
                "run {run}: serde_json={:.2}ms, finite_json::from_str_strict={:.2}ms ({:+.2}%)",
                dt_plain.as_secs_f64() * 1000.0,
                dt_strict.as_secs_f64() * 1000.0,
                (dt_strict.as_secs_f64() / dt_plain.as_secs_f64() - 1.0) * 100.0,
            );
            plain_times.push(dt_plain);
            strict_times.push(dt_strict);
        }
        plain_times.sort();
        strict_times.sort();
        let med_plain: Duration = plain_times[runs / 2];
        let med_strict: Duration = strict_times[runs / 2];
        println!(
            "median ({node_count} nodes): serde_json={:.2}ms, finite_json::from_str_strict={:.2}ms ({:+.2}%)",
            med_plain.as_secs_f64() * 1000.0,
            med_strict.as_secs_f64() * 1000.0,
            (med_strict.as_secs_f64() / med_plain.as_secs_f64() - 1.0) * 100.0,
        );
        return;
    }

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
