#![cfg(unix)]

use designos_mcp::server::Session;
use serde_json::json;
use std::fs;

#[test]
fn partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let ws_path = std::env::temp_dir().join(format!("designos-mcp-paths-efbig-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&ws_path).unwrap();

    let mut s = Session::new(&ws_path).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    for _ in 0..200 {
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap();
    }
    s.call("designos_save_file", json!({"path": "size.designos"})).unwrap();
    let full = fs::metadata(ws_path.join("size.designos")).unwrap().len();
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
    let retry_res = s.call("designos_save_file", json!({"path": "new.designos"}));
    let _ = fs::remove_dir_all(&ws_path);

    assert!(!new_exists, "failed write must not leave a partial new.designos on disk");
    assert_eq!(precious_after, original_bytes, "failed overwrite must preserve precious.designos intact");
    assert!(new_err.safe_to_retry, "new_err must report safe_to_retry: true when nothing changed on disk");
    assert!(ow_err.safe_to_retry, "ow_err must report safe_to_retry: true when original is intact");
    assert!(retry_res.is_ok(), "retrying new.designos without overwrite after failed write must succeed: {retry_res:?}");
}
