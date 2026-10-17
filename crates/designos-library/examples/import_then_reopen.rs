//! Imports a file into a scratch library, then reopens it the way the app does, and prints the
//! outcome. Reproduces "Cannot open the file: json: ..." without starting the editor.
//!
//! ```text
//! cargo run --release -p designos-library --example import_then_reopen -- <file.fig|.json|.designos|.svg> [scratch-dir] [--keep]
//! ```
//!
//! The input is only read. The scratch library is created under `scratch-dir` (default: the
//! system temp directory) and removed afterwards, unless `--keep` is given.

use designos_library::Library;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let keep = args.iter().position(|a| a == "--keep").map(|i| args.remove(i)).is_some();
    let mut args = args.into_iter();
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("usage: import_then_reopen <file.fig|.json|.designos|.svg> [scratch-dir] [--keep]");
        return ExitCode::from(2);
    };
    let base = args.next().map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let root = base.join(format!("import-then-reopen-{}", std::process::id()));
    let outcome = run(&input, &root);
    if keep {
        println!("kept the scratch library at {}", root.display());
    } else {
        let _ = std::fs::remove_dir_all(&root);
    }
    outcome
}

fn run(input: &std::path::Path, root: &std::path::Path) -> ExitCode {
    let lib = Library::open(root).expect("open the scratch library");
    let project = lib.create_project("Scratch").expect("create a project");

    let started = Instant::now();
    let entry = match lib.import_file(input, &project.id, None) {
        Ok(entry) => entry,
        Err(e) => {
            println!("import FAILED: Cannot import the file: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("imported \"{}\" in {:?}", entry.name, started.elapsed());

    let started = Instant::now();
    match lib.load_file(&entry.id) {
        Ok(doc) => {
            println!("reopen OK in {:?}: {} nodes, {} pages", started.elapsed(), doc.nodes.len(), doc.pages.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("reopen FAILED: Cannot open the file: {e}");
            ExitCode::FAILURE
        }
    }
}
