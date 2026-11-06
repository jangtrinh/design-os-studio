#![cfg(unix)]

//! A save of a document larger than the 1 MiB write buffer that runs out of room half way must still tell the
//! agent to stop (`needs_human`), exactly as a small document does (see `mcp_paths_full_disk.rs`).

use designos_mcp::server::Session;
use serde_json::json;
use std::fs;

#[test]
fn large_document_hitting_the_size_limit_mid_stream_reports_needs_human_and_keeps_the_original() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let ws_path = std::env::temp_dir().join(format!("designos-mcp-large-efbig-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&ws_path).unwrap();

    let mut s = Session::new(&ws_path).unwrap();
    s.engine.doc.images.insert("blob".into(), (0..3_000_000u32).map(|i| (i % 251) as u8).collect());
    s.call("designos_save_file", json!({"path": "size.designos"})).unwrap();
    let full = fs::metadata(ws_path.join("size.designos")).unwrap().len();
    assert!(full > 2 * 1024 * 1024, "the document must be larger than the write buffer: {full}");
    let original_bytes = b"PRECIOUS-ORIGINAL";
    fs::write(ws_path.join("precious.designos"), original_bytes).unwrap();

    let mut old = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: ignoring SIGXFSZ and lowering the soft RLIMIT_FSIZE limit in this dedicated
    // single-test integration binary affects only this process and is immediately restored.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        libc::getrlimit(libc::RLIMIT_FSIZE, &mut old);
        let lim = libc::rlimit { rlim_cur: full / 2, rlim_max: old.rlim_max };
        libc::setrlimit(libc::RLIMIT_FSIZE, &lim);
    }
    let new_res = s.call("designos_save_file", json!({"path": "new.designos"}));
    let ow_res = s.call("designos_save_file", json!({"path": "precious.designos", "overwrite": true}));
    unsafe {
        libc::setrlimit(libc::RLIMIT_FSIZE, &old);
    }

    let new_err = new_res.expect_err("partial write to new.designos must fail");
    let ow_err = ow_res.expect_err("partial overwrite of precious.designos must fail");
    let new_exists = ws_path.join("new.designos").exists();
    let precious_after = fs::read(ws_path.join("precious.designos")).unwrap_or_default();
    let _ = fs::remove_dir_all(&ws_path);

    assert!(!new_exists, "failed write must not leave a partial new.designos on disk");
    assert_eq!(precious_after, original_bytes, "failed overwrite must preserve precious.designos intact");
    for err in [&new_err, &ow_err] {
        let wire = err.to_json_value();
        assert_eq!(wire["error_kind"], "needs_human", "{wire}");
        assert_eq!(wire["safe_to_retry"], false, "{wire}");
        assert!(wire["next_step"].as_str().unwrap().starts_with("Stop and tell the user"), "{wire}");
        assert!(wire["next_step"].as_str().unwrap().contains("Do not retry"), "{wire}");
    }
}
