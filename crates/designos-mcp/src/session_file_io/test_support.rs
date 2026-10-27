//! A scratch folder and a victim file for the session-file tests.
#![cfg(unix)]

use super::*;
use std::fs;
use std::process::Command;

pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn new(tag: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("designos-mcp-io-{tag}-{}-{nonce}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A file outside the session folder that nothing here may change.
    pub(super) fn victim(&self) -> PathBuf {
        let path = self.path("victim.txt");
        fs::write(&path, b"USER CONTENT\n").unwrap();
        path
    }

    pub(super) fn fifo(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        assert!(Command::new("mkfifo").arg(&path).status().unwrap().success());
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn refusal(result: io::Result<impl std::fmt::Debug>) -> Refusal {
    refusal_of(&result.expect_err("the path must be refused")).expect("the error must be a refusal")
}

pub(super) fn victim_is_intact(victim: &Path) {
    assert_eq!(fs::read(victim).unwrap(), b"USER CONTENT\n");
}
