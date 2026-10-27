#![cfg(unix)]

use designos_mcp::{engine::Engine, server::Session, tools::errors::ErrorCode};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Barrier,
    },
};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("designos-mcp-paths-{tag}-{}-{nonce}-{seq}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// FIPS 180-4 SHA-256 digest so canary integrity is verified by cryptographic hash
/// without adding a crate to Cargo.lock (`--locked`).
fn sha256_file(path: &Path) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let data = fs::read(path).expect("read canary file");
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data;
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (dst, add) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *dst = dst.wrapping_add(add);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[test]
fn export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched() {
    let ws = TempDir::new("ws-sym-out");
    let outside = TempDir::new("out-sym-out");
    let canary = outside.0.join("canary.txt");
    let canary_bytes = b"canary-outside-workspace-do-not-overwrite\n";
    fs::write(&canary, canary_bytes).unwrap();
    let digest_before = sha256_file(&canary);

    symlink(&canary, ws.0.join("evil.fig")).unwrap();
    let mut s = Session::new(&ws.0).unwrap();

    for payload in [json!({"output_path": "evil.fig"}), json!({"output_path": "evil.fig", "overwrite": true})] {
        let err = s.call("designos_export_fig_file", payload.clone()).expect_err("symlink at last path component must be refused");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{payload}: {err}");
        assert_eq!(err.message, "evil.fig is a link, not a file. Delete the link, then try again.");
        assert_eq!(sha256_file(&canary), digest_before, "canary hash changed after {payload}");
        assert_eq!(fs::read(&canary).unwrap(), canary_bytes);
    }
}

#[test]
fn export_fig_refuses_symlink_inside_workspace() {
    let ws = TempDir::new("ws-sym-in");
    let target = ws.0.join("target.fig");
    let target_bytes = b"inside-workspace-target-bytes\n";
    fs::write(&target, target_bytes).unwrap();
    let digest_before = sha256_file(&target);

    symlink(&target, ws.0.join("link_in.fig")).unwrap();
    let mut s = Session::new(&ws.0).unwrap();

    for payload in [json!({"output_path": "link_in.fig"}), json!({"output_path": "link_in.fig", "overwrite": true})] {
        let err = s.call("designos_export_fig_file", payload.clone()).expect_err("in-workspace symlink must be refused");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{payload}: {err}");
        assert_eq!(err.message, "link_in.fig is a link, not a file. Delete the link, then try again.");
        assert_eq!(sha256_file(&target), digest_before);
    }
}

#[test]
fn export_fig_refuses_dangling_symlink_inside_and_outside_workspace() {
    let ws = TempDir::new("ws-sym-dangling");
    let outside = TempDir::new("out-sym-dangling");
    let missing_out = outside.0.join("created_outside.fig");
    let missing_in = ws.0.join("created_inside.fig");

    symlink(&missing_out, ws.0.join("dangling_out.fig")).unwrap();
    symlink(&missing_in, ws.0.join("dangling_in.fig")).unwrap();
    let mut s = Session::new(&ws.0).unwrap();

    for name in ["dangling_out.fig", "dangling_in.fig"] {
        for payload in [json!({"output_path": name}), json!({"output_path": name, "overwrite": true})] {
            let err = s.call("designos_export_fig_file", payload.clone()).expect_err("dangling symlink must be refused");
            assert_eq!(err.code, ErrorCode::InvalidArguments, "{payload}: {err}");
            assert_eq!(err.message, format!("{name} is a link, not a file. Delete the link, then try again."));
        }
    }
    assert!(!missing_out.exists(), "dangling outside target must not be created");
    assert!(!missing_in.exists(), "dangling inside target must not be created");
}

#[test]
fn export_fig_refuses_existing_file_unless_overwrite_is_true() {
    let ws = TempDir::new("ws-overwrite");
    let existing = ws.0.join("existing.fig");
    let initial = b"do-not-clobber-without-overwrite\n";
    fs::write(&existing, initial).unwrap();
    let digest_before = sha256_file(&existing);

    let mut s = Session::new(&ws.0).unwrap();
    let err = s.call("designos_export_fig_file", json!({"output_path": "existing.fig"})).expect_err("existing .fig without overwrite must fail");
    assert_eq!(err.code, ErrorCode::FileExists, "{err}");
    assert_eq!(err.message, "existing.fig already exists. Pass overwrite: true to replace it.");
    assert_eq!(err.corrected_example, Some(json!({"output_path": "existing.fig", "overwrite": true})));
    assert_eq!(sha256_file(&existing), digest_before);

    let ok = s
        .call("designos_export_fig_file", json!({"output_path": "existing.fig", "overwrite": true}))
        .expect("overwrite: true must replace existing plain .fig file");
    assert_eq!(ok["saved"], true);
    let written = fs::read(&existing).unwrap();
    assert_ne!(written.as_slice(), initial, "overwrite: true must replace existing file bytes");
    assert_eq!(written.len() as u64, ok["bytes"].as_u64().unwrap());
}

#[test]
fn export_fig_refuses_folder() {
    let ws = TempDir::new("ws-folder");
    fs::create_dir_all(ws.0.join("dir.fig")).unwrap();
    let mut s = Session::new(&ws.0).unwrap();

    for payload in [json!({"output_path": "dir.fig"}), json!({"output_path": "dir.fig", "overwrite": true})] {
        let err = s.call("designos_export_fig_file", payload.clone()).expect_err("folder must be refused");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{payload}: {err}");
        assert_eq!(err.message, "dir.fig is a folder, not a file. Choose a file path, then try again.");
    }
}

#[test]
fn export_fig_validates_extension_case_insensitively() {
    let ws = TempDir::new("ws-ext");
    let mut s = Session::new(&ws.0).unwrap();

    for bad in ["out.txt", "out.designos", "out"] {
        let err = s.call("designos_export_fig_file", json!({"output_path": bad})).expect_err("non-.fig extension must be refused");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{bad}: {err}");
    }

    for good in ["UPPER.FIG", "Mixed.Fig"] {
        let ok = s.call("designos_export_fig_file", json!({"output_path": good})).unwrap_or_else(|e| panic!("{good} should be accepted: {e}"));
        assert_eq!(ok["saved"], true);
        assert!(ws.0.join(good).is_file());
    }
}

#[test]
fn bare_engine_export_fig_enforces_all_target_checks() {
    let dir = TempDir::new("engine-direct");
    let outside = TempDir::new("engine-outside");
    let canary = outside.0.join("canary.txt");
    fs::write(&canary, b"engine-canary\n").unwrap();
    let canary_hash = sha256_file(&canary);

    let sym = dir.0.join("sym.fig");
    symlink(&canary, &sym).unwrap();
    let dangling = dir.0.join("dangling.fig");
    symlink(outside.0.join("missing.fig"), &dangling).unwrap();
    let folder = dir.0.join("folder.fig");
    fs::create_dir_all(&folder).unwrap();
    let existing = dir.0.join("existing.fig");
    fs::write(&existing, b"existing-engine\n").unwrap();
    let existing_hash = sha256_file(&existing);

    let mut e = Engine::default();

    let sym_str = sym.to_str().unwrap();
    let err = e.call("designos_export_fig_file", json!({"output_path": sym_str})).expect_err("bare Engine must refuse symlink");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, format!("{sym_str} is a link, not a file. Delete the link, then try again."));
    assert_eq!(sha256_file(&canary), canary_hash);

    let err = e
        .call("designos_export_fig_file", json!({"output_path": sym_str, "overwrite": true}))
        .expect_err("bare Engine with overwrite: true must still refuse symlink");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(sha256_file(&canary), canary_hash);

    let dangling_str = dangling.to_str().unwrap();
    let err = e.call("designos_export_fig_file", json!({"output_path": dangling_str})).expect_err("bare Engine must refuse dangling symlink");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert!(!outside.0.join("missing.fig").exists());

    let folder_str = folder.to_str().unwrap();
    let err = e.call("designos_export_fig_file", json!({"output_path": folder_str})).expect_err("bare Engine must refuse folder");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, format!("{folder_str} is a folder, not a file. Choose a file path, then try again."));

    let existing_str = existing.to_str().unwrap();
    let err = e.call("designos_export_fig_file", json!({"output_path": existing_str})).expect_err("bare Engine must refuse existing .fig without overwrite");
    assert_eq!(err.code, ErrorCode::FileExists);
    assert_eq!(err.message, format!("{existing_str} already exists. Pass overwrite: true to replace it."));
    assert_eq!(sha256_file(&existing), existing_hash);

    let ok = e
        .call("designos_export_fig_file", json!({"output_path": existing_str, "overwrite": true}))
        .expect("bare Engine with overwrite: true must replace existing .fig");
    let written = fs::read(&existing).unwrap();
    assert_ne!(sha256_file(&existing), existing_hash);
    assert_eq!(written.len() as u64, ok["bytes"].as_u64().unwrap());

    let upper = dir.0.join("CAPS.FIG");
    e.call("designos_export_fig_file", json!({"output_path": upper.to_str().unwrap()})).expect("bare Engine must accept .FIG case-insensitively");
}

#[test]
fn file_creation_tools_refuse_control_characters_in_names() {
    let ws = TempDir::new("ws-rt05-ctrl");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for stem in ["line\nbreak", "carriage\rreturn", "bell\x07char", "del\x7fchar"] {
        let save_name = format!("{stem}.designos");
        let err = s.call("designos_save_file", json!({"path": save_name})).expect_err("save_file must refuse control char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{save_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{save_name}: {err}");
        assert_eq!(err.message, format!("{save_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&save_name).exists());

        let png_name = format!("{stem}.png");
        let err = s.call("designos_render_png", json!({"path": png_name, "node_id": rect_id})).expect_err("render_png must refuse control char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{png_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{png_name}: {err}");
        assert_eq!(err.message, format!("{png_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&png_name).exists());

        let fig_name = format!("{stem}.fig");
        let err = s.call("designos_export_fig_file", json!({"output_path": fig_name})).expect_err("export_fig_file must refuse control char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{fig_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{fig_name}: {err}");
        assert_eq!(err.message, format!("{fig_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&fig_name).exists());
    }
}

#[test]
fn file_creation_tools_refuse_bidi_override_characters_in_names() {
    let ws = TempDir::new("ws-rt05-bidi");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for stem in [
        "rlo\u{202E}evil",
        "lre\u{202A}evil",
        "rle\u{202B}evil",
        "pdf\u{202C}evil",
        "lro\u{202D}evil",
        "lri\u{2066}evil",
        "rli\u{2067}evil",
        "fsi\u{2068}evil",
        "pdi\u{2069}evil",
    ] {
        let save_name = format!("{stem}.designos");
        let err = s.call("designos_save_file", json!({"path": save_name})).expect_err("save_file must refuse bidi char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{save_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{save_name}: {err}");
        assert_eq!(err.message, format!("{save_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&save_name).exists());

        let png_name = format!("{stem}.png");
        let err = s.call("designos_render_png", json!({"path": png_name, "node_id": rect_id})).expect_err("render_png must refuse bidi char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{png_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{png_name}: {err}");
        assert_eq!(err.message, format!("{png_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&png_name).exists());

        let fig_name = format!("{stem}.fig");
        let err = s.call("designos_export_fig_file", json!({"output_path": fig_name})).expect_err("export_fig_file must refuse bidi char");
        assert_eq!(err.code, ErrorCode::InvalidName, "{fig_name}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{fig_name}: {err}");
        assert_eq!(err.message, format!("{fig_name} has a name with a line break or a direction-control character. Rename it, then try again."));
        assert!(!ws.0.join(&fig_name).exists());
    }
}

#[test]
fn file_creation_tools_refuse_whitespace_only_names_and_accept_vietnamese_and_cjk() {
    let ws = TempDir::new("ws-rt05-ws-unicode");
    let mut s = Session::new(&ws.0).unwrap();
    fs::create_dir_all(ws.0.join("dir")).unwrap();

    for bad_name in ["", "   ", "\t", "   .designos", " \u{00A0} .designos", "dir/   .designos", "   /file.designos"] {
        let err = s.call("designos_save_file", json!({"path": bad_name})).expect_err("save_file must refuse whitespace-only name");
        assert_eq!(err.code, ErrorCode::InvalidName, "{bad_name:?}: {err}");
        assert_eq!(err.code.as_str(), "INVALID_NAME", "{bad_name:?}: {err}");
        assert!(err.corrected_example.is_none(), "{bad_name:?} must not suggest overwrite: {:?}", err.corrected_example);
        if !bad_name.is_empty() {
            assert!(!ws.0.join(bad_name).exists(), "{bad_name:?} must not be created on disk");
        }
    }

    for valid_name in
        ["Thiết kế hệ thống.designos", "Bảng điều khiển — Phiên bản 2.designos", "デザインシステム.designos", "设计系统.designos", "디자인시스템.designos"]
    {
        let res =
            s.call("designos_save_file", json!({"path": valid_name})).unwrap_or_else(|e| panic!("valid Unicode name {valid_name:?} must save cleanly: {e}"));
        assert_eq!(res["saved"], json!(true), "{valid_name:?}: {res}");
        assert!(ws.0.join(valid_name).is_file(), "{valid_name:?} must exist on disk after save");
    }
}

#[test]
fn file_creation_tools_report_name_too_long_instead_of_file_exists() {
    let ws = TempDir::new("ws-rt06");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for stem_len in [260usize, 291usize, 300usize] {
        let stem = "a".repeat(stem_len);
        let save_name = format!("{stem}.designos");
        let err = s.call("designos_save_file", json!({"path": save_name})).expect_err("overlong save_file name must fail");
        assert_ne!(err.code, ErrorCode::FileExists, "must not report FILE_EXISTS");
        assert_eq!(err.code, ErrorCode::NameTooLong, "{err}");
        assert_eq!(err.code.as_str(), "NAME_TOO_LONG", "{err}");
        assert_eq!(err.message, format!("{save_name} has a name longer than 255 bytes. Use a shorter name."));
        assert!(err.corrected_example.is_none(), "NAME_TOO_LONG must not include an overwrite hint: {:?}", err.corrected_example);
        assert!(!err.message.contains("overwrite"), "NAME_TOO_LONG message must not mention overwrite: {}", err.message);
        assert!(!ws.0.join(&save_name).exists(), "overlong save_file must not create a file");
    }

    let raw_300 = "a".repeat(300);
    let err = s.call("designos_save_file", json!({"path": raw_300})).expect_err("300-byte raw name must fail with NAME_TOO_LONG");
    assert_eq!(err.code, ErrorCode::NameTooLong, "{err}");
    assert_eq!(err.code.as_str(), "NAME_TOO_LONG", "{err}");
    assert!(err.corrected_example.is_none());
    assert!(!err.message.contains("overwrite"));

    // Multi-byte UTF-8 boundary: 82 * 3 bytes ("đ" = 2 bytes, "ệ" = 3 bytes) -> exact 255 bytes vs 256 bytes.
    // "ệ" is 3 UTF-8 bytes; 82 * 3 = 246 bytes + ".designos" (9 bytes) = 255 bytes.
    let utf8_255 = format!("{}.designos", "ệ".repeat(82));
    assert_eq!(utf8_255.len(), 255);
    s.call("designos_save_file", json!({"path": utf8_255})).expect("255-byte UTF-8 filename must succeed");
    assert!(ws.0.join(&utf8_255).is_file());

    // 247 bytes stem (82 * 3 + 1 ASCII byte) + ".designos" (9 bytes) = 256 bytes -> NAME_TOO_LONG.
    let utf8_256 = format!("{}a.designos", "ệ".repeat(82));
    assert_eq!(utf8_256.len(), 256);
    let err = s.call("designos_save_file", json!({"path": utf8_256})).expect_err("256-byte UTF-8 filename must fail with NAME_TOO_LONG");
    assert_eq!(err.code, ErrorCode::NameTooLong, "{err}");
    assert_eq!(err.code.as_str(), "NAME_TOO_LONG", "{err}");
    assert!(err.corrected_example.is_none());
    assert!(!ws.0.join(&utf8_256).exists());

    let stem = "a".repeat(260);
    let png_name = format!("{stem}.png");
    let err = s.call("designos_render_png", json!({"path": png_name, "node_id": rect_id})).expect_err("overlong render_png name must fail");
    assert_ne!(err.code, ErrorCode::FileExists, "must not report FILE_EXISTS");
    assert_eq!(err.code, ErrorCode::NameTooLong, "{err}");
    assert_eq!(err.code.as_str(), "NAME_TOO_LONG", "{err}");
    assert_eq!(err.message, format!("{png_name} has a name longer than 255 bytes. Use a shorter name."));
    assert!(err.corrected_example.is_none());

    let fig_name = format!("{stem}.fig");
    let err = s.call("designos_export_fig_file", json!({"output_path": fig_name})).expect_err("overlong export_fig_file name must fail");
    assert_ne!(err.code, ErrorCode::FileExists, "must not report FILE_EXISTS");
    assert_eq!(err.code, ErrorCode::NameTooLong, "{err}");
    assert_eq!(err.code.as_str(), "NAME_TOO_LONG", "{err}");
    assert_eq!(err.message, format!("{fig_name} has a name longer than 255 bytes. Use a shorter name."));
    assert!(err.corrected_example.is_none());
}

#[test]
fn outside_workspace_refusal_never_echoes_path_and_preserves_canary() {
    let ws = TempDir::new("ws-v02");
    let outside = TempDir::new("out-v02-secret");
    let canary = outside.0.join("secret_canary_file.designos");
    let canary_fig = outside.0.join("secret_canary_file.fig");
    let canary_png = outside.0.join("secret_canary_file.png");
    let canary_cap = outside.0.join("secret_canary_file.webcapture.json");
    for p in [&canary, &canary_fig, &canary_png, &canary_cap] {
        fs::write(p, b"top-secret-canary-payload\n").unwrap();
    }
    let hashes_before: Vec<[u8; 32]> = [&canary, &canary_fig, &canary_png, &canary_cap].iter().map(|p| sha256_file(p)).collect();

    symlink(&outside.0, ws.0.join("escape_dir")).unwrap();

    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        ("designos_open_file", json!({"path": "/etc/passwd"}), "/etc/passwd"),
        ("designos_open_file", json!({"path": "../secret_canary_file.designos"}), "secret_canary_file"),
        ("designos_open_file", json!({"path": canary.to_str().unwrap()}), "secret_canary_file"),
        ("designos_open_file", json!({"path": "escape_dir/secret_canary_file.designos"}), "secret_canary_file"),
        ("designos_save_file", json!({"path": "../secret_canary_file.designos", "overwrite": true}), "secret_canary_file"),
        ("designos_save_file", json!({"path": canary.to_str().unwrap(), "overwrite": true}), "secret_canary_file"),
        ("designos_save_file", json!({"path": "escape_dir/secret_canary_file.designos", "overwrite": true}), "secret_canary_file"),
        ("designos_save_file", json!({"path": "/etc/passwd"}), "/etc/passwd"),
        ("designos_render_png", json!({"path": "../secret_canary_file.png", "node_id": rect_id}), "secret_canary_file"),
        ("designos_render_png", json!({"path": canary_png.to_str().unwrap(), "node_id": rect_id}), "secret_canary_file"),
        ("designos_render_png", json!({"path": "escape_dir/secret_canary_file.png", "node_id": rect_id}), "secret_canary_file"),
        ("designos_render_png", json!({"path": "/etc/passwd", "node_id": rect_id}), "/etc/passwd"),
        ("designos_export_fig_file", json!({"output_path": "../secret_canary_file.fig", "overwrite": true}), "secret_canary_file"),
        ("designos_export_fig_file", json!({"output_path": canary_fig.to_str().unwrap(), "overwrite": true}), "secret_canary_file"),
        ("designos_export_fig_file", json!({"output_path": "escape_dir/secret_canary_file.fig", "overwrite": true}), "secret_canary_file"),
        ("designos_export_fig_file", json!({"output_path": "/etc/passwd"}), "/etc/passwd"),
        ("designos_import_web_capture", json!({"path": "../secret_canary_file.webcapture.json"}), "secret_canary_file"),
        ("designos_import_web_capture", json!({"path": canary_cap.to_str().unwrap()}), "secret_canary_file"),
        ("designos_import_web_capture", json!({"path": "escape_dir/secret_canary_file.webcapture.json"}), "secret_canary_file"),
    ];

    for (tool, args, forbidden_substring) in cases {
        let err = s.call(tool, args.clone()).expect_err("outside-workspace path must be refused");
        assert_eq!(err.code, ErrorCode::PathOutsideWorkspace, "{tool} {args}: {err}");
        assert!(err.received.is_none(), "{tool} {args} must not set received on PATH_OUTSIDE_WORKSPACE: {err}");
        let serialized = err.to_json_value().to_string();
        assert!(!serialized.contains(forbidden_substring), "{tool} {args} echoed outside path substring `{forbidden_substring}` in reply: {serialized}");
        let hashes_after: Vec<[u8; 32]> = [&canary, &canary_fig, &canary_png, &canary_cap].iter().map(|p| sha256_file(p)).collect();
        assert_eq!(hashes_after, hashes_before, "canary file modified after refused call {tool} {args}");
    }
}

#[test]
fn save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary() {
    let ws = TempDir::new("ws-save-png-sym");
    let outside = TempDir::new("out-save-png-sym");
    let canary = outside.0.join("canary.txt");
    fs::write(&canary, b"save-png-canary\n").unwrap();
    let canary_hash = sha256_file(&canary);
    let missing_out = outside.0.join("dangling_created.txt");

    symlink(&canary, ws.0.join("live.designos")).unwrap();
    symlink(&missing_out, ws.0.join("dangling.designos")).unwrap();
    symlink(&canary, ws.0.join("live.png")).unwrap();
    symlink(&missing_out, ws.0.join("dangling.png")).unwrap();

    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for name in ["live.designos", "dangling.designos"] {
        for overwrite in [false, true] {
            let err = s.call("designos_save_file", json!({"path": name, "overwrite": overwrite})).expect_err("save_file must refuse symlink");
            assert_eq!(err.code, ErrorCode::InvalidArguments, "{name} overwrite={overwrite}: {err}");
            assert_eq!(err.message, format!("{name} is a link, not a file. Delete the link, then try again."));
            assert_eq!(sha256_file(&canary), canary_hash);
            assert!(!missing_out.exists(), "dangling symlink target must not be created");
        }
    }

    for name in ["live.png", "dangling.png"] {
        let err = s.call("designos_render_png", json!({"path": name, "node_id": rect_id})).expect_err("render_png must refuse symlink");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{name}: {err}");
        assert_eq!(err.message, format!("{name} is a link, not a file. Delete the link, then try again."));
        assert_eq!(sha256_file(&canary), canary_hash);
        assert!(!missing_out.exists());
    }
}

#[test]
fn concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json() {
    let ws = TempDir::new("ws-r2-race");
    let rounds = 300;
    let mut both_ok = 0usize;
    let mut one_exists = 0usize;
    let mut other = 0usize;
    let mut corrupt = 0usize;

    for i in 0..rounds {
        let name = format!("race-{i}.designos");
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let ws_path = ws.0.clone();
                let b = barrier.clone();
                let name = name.clone();
                std::thread::spawn(move || {
                    let mut s = Session::new(&ws_path).unwrap();
                    b.wait();
                    s.call("designos_save_file", json!({"path": name})).map(|_| ()).map_err(|e| e.code)
                })
            })
            .collect();
        let res: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let oks = res.iter().filter(|r| r.is_ok()).count();
        let exists = res.iter().filter(|r| matches!(r, Err(ErrorCode::FileExists))).count();
        match (oks, exists) {
            (2, _) => both_ok += 1,
            (1, 1) => one_exists += 1,
            _ => other += 1,
        }
        let bytes = fs::read(ws.0.join(&name)).unwrap_or_default();
        if serde_json::from_slice::<Value>(&bytes).is_err() {
            corrupt += 1;
        }
    }

    assert_eq!(both_ok, 0, "two concurrent no-overwrite saves must never both succeed (both_ok={both_ok}/{rounds})");
    assert_eq!(corrupt, 0, "concurrent saves must never write invalid/concatenated JSON (corrupt={corrupt}/{rounds})");
    assert_eq!(other, 0, "unexpected error codes in concurrent saves (other={other}/{rounds})");
    assert_eq!(one_exists, rounds, "each round must have one winner and one FILE_EXISTS");
}

#[test]
fn file_planted_during_no_overwrite_export_is_never_overwritten() {
    let ws = TempDir::new("ws-r2-plant");
    let rounds = 300;
    let stop = Arc::new(AtomicBool::new(false));
    let mut lost = 0usize;
    let mut s = Session::new(&ws.0).unwrap();

    for i in 0..rounds {
        let name = format!("plant-{i}.fig");
        let target = ws.0.join(&name);
        stop.store(false, Ordering::SeqCst);
        let st = stop.clone();
        let t2 = target.clone();
        let planter = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                if fs::OpenOptions::new().write(true).create_new(true).open(&t2).and_then(|mut f| std::io::Write::write_all(&mut f, b"PLANTED")).is_ok() {
                    return true;
                }
            }
            false
        });
        let r = s.call("designos_export_fig_file", json!({"output_path": name}));
        stop.store(true, Ordering::SeqCst);
        let planted = planter.join().unwrap();
        let bytes = fs::read(&target).unwrap_or_default();
        if planted && (r.is_ok() || bytes != b"PLANTED") {
            lost += 1;
        }
        let _ = fs::remove_file(&target);
    }

    assert_eq!(lost, 0, "a file planted between check and write must never be overwritten without overwrite: true (lost={lost}/{rounds})");
}

#[test]
fn render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint() {
    let ws = TempDir::new("ws-r3-png-race");
    let rounds = 100;
    let mut both_ok = 0usize;
    let mut one_exists = 0usize;

    for i in 0..rounds {
        let name = format!("png-race-{i}.png");
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let ws_path = ws.0.clone();
                let b = barrier.clone();
                let name = name.clone();
                std::thread::spawn(move || {
                    let mut s = Session::new(&ws_path).unwrap();
                    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
                    let rect_id = s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()
                        ["node"]["id"]
                        .as_u64()
                        .unwrap();
                    b.wait();
                    s.call("designos_render_png", json!({"path": name, "node_id": rect_id}))
                })
            })
            .collect();
        let res: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let oks = res.iter().filter(|r| r.is_ok()).count();
        let errs: Vec<_> = res.into_iter().filter_map(|r| r.err()).collect();
        if oks == 2 {
            both_ok += 1;
        } else if oks == 1 && errs.len() == 1 {
            let err = &errs[0];
            assert_eq!(err.code, ErrorCode::FileExists, "round {i}: {err}");
            assert!(!err.message.contains("overwrite"), "render_png has no overwrite option and must never tell the caller to pass overwrite: {}", err.message);
            assert_eq!(
                err.message,
                format!("{name} already exists. Choose a new path, then try again."),
                "render_png FILE_EXISTS message must use ruling-1 wording, got: {}",
                err.message
            );
            assert_eq!(err.received, Some(json!(name)), "render_png FILE_EXISTS must include received path");
            assert!(err.corrected_example.is_none(), "render_png FILE_EXISTS must not emit an overwrite corrected_example: {:?}", err.corrected_example);
            one_exists += 1;
        } else {
            panic!("round {i}: unexpected outcomes oks={oks} errs={errs:?}");
        }
    }

    assert_eq!(both_ok, 0, "concurrent render_png calls to the same path must never both succeed");
    assert_eq!(one_exists, rounds, "every round must have one winner and one FILE_EXISTS loser");

    // Tools that DO have the overwrite option (save_file, export_fig_file) keep the overwrite hint on race / existing file.
    let mut s = Session::new(&ws.0).unwrap();
    fs::write(ws.0.join("taken.designos"), b"x").unwrap();
    let save_err = s.call("designos_save_file", json!({"path": "taken.designos"})).expect_err("existing .designos without overwrite must fail");
    assert_eq!(save_err.code, ErrorCode::FileExists);
    assert_eq!(save_err.message, "taken.designos already exists. Pass overwrite: true to replace it.");
    assert_eq!(save_err.corrected_example, Some(json!({"path": "taken.designos", "overwrite": true})));

    fs::write(ws.0.join("taken.fig"), b"x").unwrap();
    let fig_err = s.call("designos_export_fig_file", json!({"output_path": "taken.fig"})).expect_err("existing .fig without overwrite must fail");
    assert_eq!(fig_err.code, ErrorCode::FileExists);
    assert_eq!(fig_err.message, "taken.fig already exists. Pass overwrite: true to replace it.");
    assert_eq!(fig_err.corrected_example, Some(json!({"output_path": "taken.fig", "overwrite": true})));
}

#[test]
fn render_png_refuses_existing_file_early_and_preserves_bytes() {
    let ws = TempDir::new("ws-r4-existing-png");
    let existing = ws.0.join("existing.png");
    let initial = b"existing-png-bytes-do-not-overwrite\n";
    fs::write(&existing, initial).unwrap();
    let digest_before = sha256_file(&existing);

    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    let err = s
        .call("designos_render_png", json!({"path": "existing.png", "node_id": rect_id}))
        .expect_err("render_png to an existing .png must fail with FILE_EXISTS");
    assert_eq!(err.code, ErrorCode::FileExists, "{err}");
    assert_eq!(err.path, "/path");
    assert_eq!(err.message, "existing.png already exists. Choose a new path, then try again.");
    assert_eq!(err.received, Some(json!("existing.png")));
    assert!(err.corrected_example.is_none(), "render_png FILE_EXISTS must not set corrected_example: {:?}", err.corrected_example);
    assert_eq!(sha256_file(&existing), digest_before, "existing.png hash must be unchanged");
    assert_eq!(fs::read(&existing).unwrap().as_slice(), initial, "existing.png bytes must be unchanged");

    // Early refusal in validate_existing_output_target happens before node lookup / rendering.
    let early_err = s
        .call("designos_render_png", json!({"path": "existing.png", "node_id": 999_999}))
        .expect_err("render_png to an existing .png must refuse early in validate_existing_output_target before node lookup/render");
    assert_eq!(early_err.code, ErrorCode::FileExists, "{early_err}");
    assert_eq!(early_err.message, "existing.png already exists. Choose a new path, then try again.");
    assert_eq!(early_err.received, Some(json!("existing.png")));
    assert!(early_err.corrected_example.is_none());
    assert_eq!(sha256_file(&existing), digest_before);
    assert_eq!(fs::read(&existing).unwrap().as_slice(), initial);
}

#[test]
fn new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes() {
    let ws = TempDir::new("ws-r2-modes");
    let outside = TempDir::new("out-r2-modes");
    // SAFETY: reads the process umask and immediately restores it.
    let umask = unsafe {
        let old = libc::umask(0o022);
        libc::umask(old);
        old as u32
    };
    let expected_mode = 0o666 & !umask;

    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    s.call("designos_save_file", json!({"path": "a.designos"})).unwrap();
    s.call("designos_save_file", json!({"path": "b.designos", "overwrite": true})).unwrap();
    s.call("designos_render_png", json!({"path": "a.png", "node_id": rect_id})).unwrap();
    s.call("designos_export_fig_file", json!({"output_path": "a.fig"})).unwrap();

    for name in ["a.designos", "b.designos", "a.png", "a.fig"] {
        let mode = fs::metadata(ws.0.join(name)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, expected_mode, "{name} mode was {mode:o}, expected {expected_mode:o} (not 0600)");
    }

    // Overwriting an existing file preserves its mode.
    fs::set_permissions(ws.0.join("a.designos"), fs::Permissions::from_mode(0o640)).unwrap();
    s.call("designos_save_file", json!({"path": "a.designos", "overwrite": true})).unwrap();
    let preserved = fs::metadata(ws.0.join("a.designos")).unwrap().permissions().mode() & 0o777;
    assert_eq!(preserved, 0o640, "overwrite: true must preserve existing file mode");

    // 255-byte filenames work in both overwrite: false and overwrite: true modes.
    let name_255_fig = format!("{}.fig", "a".repeat(251));
    let name_255_dos = format!("{}.designos", "b".repeat(246));
    assert_eq!(name_255_fig.len(), 255);
    assert_eq!(name_255_dos.len(), 255);
    for ow in [false, true] {
        s.call("designos_export_fig_file", json!({"output_path": name_255_fig, "overwrite": ow}))
            .unwrap_or_else(|e| panic!("255-byte .fig with overwrite={ow} must succeed: {e}"));
        s.call("designos_save_file", json!({"path": name_255_dos, "overwrite": ow}))
            .unwrap_or_else(|e| panic!("255-byte .designos with overwrite={ow} must succeed: {e}"));
    }

    // Hard links, FIFOs, and read-only targets remain refused in both modes.
    let canary = outside.0.join("canary.bin");
    fs::write(&canary, b"CANARY-BYTES").unwrap();
    let canary_hash = sha256_file(&canary);
    fs::hard_link(&canary, ws.0.join("hl.fig")).unwrap();
    fs::hard_link(&canary, ws.0.join("hl.designos")).unwrap();
    let err = s.call("designos_export_fig_file", json!({"output_path": "hl.fig", "overwrite": true})).expect_err("hard link must be refused on overwrite");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, "hl.fig has a second name (a hard link). Delete it, then try again.");
    let err = s.call("designos_save_file", json!({"path": "hl.designos", "overwrite": true})).expect_err("hard link must be refused on overwrite");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, "hl.designos has a second name (a hard link). Delete it, then try again.");
    assert_eq!(sha256_file(&canary), canary_hash);
}

#[test]
fn in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted() {
    let base = TempDir::new("ws-r2-alias");
    let real = base.0.join("real");
    fs::create_dir_all(&real).unwrap();
    let alias = base.0.join("alias");
    symlink(&real, &alias).unwrap();

    let mut s = Session::new(&alias).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for (tag, root) in [("alias", alias.clone()), ("tmpdir-raw", real.clone()), ("canonical", real.canonicalize().unwrap())] {
        let save_path = root.join(format!("{tag}.designos"));
        let png_path = root.join(format!("{tag}.png"));
        let fig_path = root.join(format!("{tag}.fig"));
        s.call("designos_save_file", json!({"path": save_path.to_str().unwrap()}))
            .unwrap_or_else(|e| panic!("save_file via {tag} ({}) must succeed: {e}", save_path.display()));
        s.call("designos_render_png", json!({"path": png_path.to_str().unwrap(), "node_id": rect_id}))
            .unwrap_or_else(|e| panic!("render_png via {tag} ({}) must succeed: {e}", png_path.display()));
        s.call("designos_export_fig_file", json!({"output_path": fig_path.to_str().unwrap()}))
            .unwrap_or_else(|e| panic!("export_fig_file via {tag} ({}) must succeed: {e}", fig_path.display()));
        assert!(real.join(format!("{tag}.designos")).is_file());
        assert!(real.join(format!("{tag}.png")).is_file());
        assert!(real.join(format!("{tag}.fig")).is_file());
    }
}

#[test]
fn export_fig_rules_list_both_file_exists_and_internal_error() {
    let codes = designos_mcp::rules::tool_error_codes("designos_export_fig_file");
    assert!(codes.contains(&"`FILE_EXISTS`"), "designos_export_fig_file rules must list FILE_EXISTS: {codes:?}");
    assert!(codes.contains(&"`INTERNAL_ERROR`"), "designos_export_fig_file rules must list INTERNAL_ERROR: {codes:?}");
}

#[test]
fn argument_errors_never_echo_outside_paths_in_corrected_example_or_received() {
    let ws = TempDir::new("ws-r2-arg-echo");
    let mut s = Session::new(&ws.0).unwrap();
    let cases: Vec<(&str, Value)> = vec![
        ("designos_render_png", json!({"path": "/etc/SECRETNAME.png", "node_id": 1, "bogus": 1})),
        ("designos_save_file", json!({"path": "/etc/SECRETNAME.designos", "overwrite": true, "bogus": 1})),
        ("designos_export_fig_file", json!({"output_path": "/etc/SECRETNAME.fig", "bogus": true})),
        ("designos_open_file", json!({"path": "/etc/SECRETNAME.designos", "bogus": 1})),
        ("designos_import_web_capture", json!({"path": "/etc/SECRETNAME.json", "bogus": 1})),
        ("designos_save_file", json!({"path": "/etc/SECRETNAME.designos", "overwrite": "yes"})),
        ("designos_open_file", json!({"path": "/etc/SECRETNAME", "discard_changes": "no"})),
    ];
    for (tool, args) in cases {
        let reply = serde_json::to_string(&s.call_tool_result(tool, args.clone())).unwrap();
        assert!(!reply.contains("SECRETNAME"), "{tool} {args} leaked outside path in argument error reply: {reply}");
    }
}

#[test]
fn trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first() {
    let ws = TempDir::new("ws-r2-nits");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    let rect_id =
        s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle", "width": 20, "height": 20})).unwrap()["node"]["id"].as_u64().unwrap();

    for (tool, key, bad_dir, stem) in [
        ("designos_export_fig_file", "output_path", "x.fig/", "x.fig"),
        ("designos_save_file", "path", "x.designos/", "x.designos"),
        ("designos_render_png", "path", "x.png/", "x.png"),
    ] {
        let mut payload = json!({ key: bad_dir });
        if tool == "designos_render_png" {
            payload["node_id"] = json!(rect_id);
        }
        let err = s.call(tool, payload).expect_err("trailing slash must be refused as folder");
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{tool} {bad_dir}: {err}");
        assert_eq!(err.message, format!("{bad_dir} is a folder, not a file. Choose a file path, then try again."));
        assert!(!ws.0.join(stem).exists(), "{stem} must not be created when {bad_dir} is passed");
    }

    let mut e = Engine::default();
    let trailing_engine = ws.0.join("engine.fig/");
    let trailing_str = format!("{}/", ws.0.join("engine.fig").display());
    let err = e.call("designos_export_fig_file", json!({"output_path": trailing_str})).expect_err("bare Engine must refuse trailing slash");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, format!("{trailing_str} is a folder, not a file. Choose a file path, then try again."));
    assert!(!trailing_engine.with_file_name("engine.fig").exists());

    let save_ext = s.call("designos_save_file", json!({"path": "wrong.txt"})).expect_err("save_file must refuse wrong extension");
    assert_eq!(save_ext.code, ErrorCode::InvalidArguments);
    assert_eq!(save_ext.message, "wrong.txt does not end with .designos. Choose a path ending with .designos, then try again.");

    let png_ext = s.call("designos_render_png", json!({"path": "wrong.txt", "node_id": rect_id})).expect_err("render_png must refuse wrong extension");
    assert_eq!(png_ext.code, ErrorCode::InvalidArguments);
    assert_eq!(png_ext.message, "wrong.txt does not end with .png. Choose a path ending with .png, then try again.");

    for bidi_mark in ["lrm\u{200E}x.fig", "rlm\u{200F}x.fig", "alm\u{061C}x.fig"] {
        let err = s.call("designos_export_fig_file", json!({"output_path": bidi_mark})).expect_err("bidi mark must be refused");
        assert_eq!(err.code.as_str(), "INVALID_NAME");
        assert_eq!(err.message, format!("{bidi_mark} has a name with a line break or a direction-control character. Rename it, then try again."));
    }
}
