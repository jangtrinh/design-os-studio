//! Debug helper: dump a `.fig` file as JSON.
//!
//!   cargo run -p designos-fig --example fig2json -- file.fig            # raw kiwi message
//!   cargo run -p designos-fig --example fig2json -- file.fig --doc      # imported Document
//!   cargo run -p designos-fig --example fig2json -- file.fig --rest     # re-exported REST JSON

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: fig2json <file.fig> [--doc | --rest]");
        std::process::exit(2);
    };
    let mode = args.next();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(1);
        }
    };
    let out = match mode.as_deref() {
        Some("--doc") => designos_fig::import_fig(&bytes).map(|mut d| {
            // A large file defers its inactive pages; the serializer refuses a document until they are loaded.
            if let Err(e) = d.load_pending_pages() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
            d.to_json().unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
        }),
        Some("--rest") => designos_fig::import_fig(&bytes).map(|d| designos_fig::export_rest_json(&d).to_string()),
        _ => designos_fig::decode_fig_to_json(&bytes).map(|v| serde_json::to_string_pretty(&v).unwrap_or_default()),
    };
    match out {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
