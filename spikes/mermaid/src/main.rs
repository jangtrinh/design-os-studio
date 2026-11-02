//! THROWAWAY spike (#1527). One process = one (engine, fixture) pair so that
//! `ru_maxrss` is a per-diagram peak. Prints one JSON line.
//!
//! usage: spike-mermaid <mmdr|merman|baseline> <fixture.mmd> <out-dir> [warm-runs]

mod engines;
mod front_door;
mod raster;
mod studio_theme;

use serde_json::json;
use std::time::Instant;

fn peak_rss_mb() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    // macOS reports bytes, Linux kilobytes.
    let div = if cfg!(target_os = "macos") { 1024.0 * 1024.0 } else { 1024.0 };
    ru.ru_maxrss as f64 / div
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Everything the UI thread must never do. Runs on a worker thread.
fn work(engine: String, prepared: front_door::Prepared, warm: usize, out_base: String) -> serde_json::Value {
    if let front_door::Kind::Unsupported(k) = &prepared.kind {
        return json!({"ok": false, "path": "unsupported", "kind": k,
            "message": "Diagram type not supported yet · flowchart and sequence work"});
    }
    let t = Instant::now();
    let svg = match engines::render_svg(&engine, &prepared.body) {
        Ok(s) => s,
        Err(e) => return json!({"ok": false, "path": "syntax-error", "message": e.message, "line": e.line,
            "t_fail_ms": ms(t)}),
    };
    let t_cold = ms(t);
    let warm_ms: Vec<f64> = (0..warm)
        .map(|_| {
            let t = Instant::now();
            let _ = engines::render_svg(&engine, &prepared.body);
            ms(t)
        })
        .collect();
    let _ = std::fs::write(format!("{out_base}.svg"), &svg);
    let t = Instant::now();
    let r = match raster::svg_to_png(&svg, studio_theme::BG_SECONDARY, 2.0) {
        Ok(r) => r,
        Err(e) => return json!({"ok": false, "path": "raster-error", "message": e}),
    };
    let t_png = ms(t);
    let _ = std::fs::write(format!("{out_base}.png"), &r.png);
    json!({"ok": true, "t_svg_cold_ms": t_cold, "t_svg_warm_median_ms": if warm_ms.is_empty() { None } else { Some(median(warm_ms)) },
        "t_png_ms": t_png, "svg_bytes": svg.len(), "png_bytes": r.png.len(),
        "px": [r.width, r.height], "text_nodes": r.text_nodes,
        "acc_title": prepared.acc_title, "acc_descr": prepared.acc_descr})
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: spike-mermaid <engine|baseline> <fixture.mmd> <out-dir> [warm-runs]");
        std::process::exit(2);
    }
    let (engine, fixture, out_dir) = (a[1].clone(), &a[2], &a[3]);
    let warm: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
    let src = std::fs::read_to_string(fixture).expect("read fixture");
    let stem = std::path::Path::new(fixture).file_stem().unwrap().to_string_lossy().to_string();
    std::fs::create_dir_all(out_dir).expect("mkdir out");
    if engine == "baseline" {
        // process floor: load the font like the real run does, render nothing
        let _ = raster::svg_to_png("<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>", "#000000", 1.0);
        println!("{}", json!({"engine": "baseline", "peak_rss_mb": peak_rss_mb()}));
        return;
    }
    let prepared = front_door::prepare(&src);
    let out_base = format!("{out_dir}/{engine}-{stem}");
    // Render off the calling thread, as Studio must. Same 8 MiB stack the
    // workspace sets via .cargo/config.toml RUST_MIN_STACK.
    let (tx, rx) = std::sync::mpsc::channel();
    let e2 = engine.clone();
    std::thread::Builder::new()
        .name("mermaid-render".into())
        .stack_size(8 << 20)
        .spawn(move || tx.send(work(e2, prepared, warm, out_base)).ok())
        .expect("spawn");
    // The "UI thread": stays responsive, only polls. 10 s budget = hard cancel point.
    let mut ticks = 0u32;
    let mut res = loop {
        match rx.recv_timeout(std::time::Duration::from_millis(1)) {
            Ok(v) => break v,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if ticks < 10_000 => ticks += 1,
            Err(e) => break json!({"ok": false, "path": "worker", "message": e.to_string()}),
        }
    };
    res["engine"] = json!(engine);
    res["fixture"] = json!(stem);
    res["ui_thread_polls_while_waiting"] = json!(ticks);
    res["peak_rss_mb"] = json!(peak_rss_mb());
    println!("{res}");
}
