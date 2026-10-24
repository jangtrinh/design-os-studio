//! A scratch session folder, a victim file outside it, and the hazards an agent can plant in the folder.
use designos_mcp::server::Session;
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::{symlink, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// What the victim file holds before a test: a user's file that nothing here may change.
pub const VICTIM: &[u8] = b"USER CONTENT, KEEP ME\n";

static SEQ: AtomicU64 = AtomicU64::new(0);

pub struct Scratch {
    base: PathBuf,
    pub workspace: PathBuf,
    pub session: PathBuf,
    victims: PathBuf,
}

impl Scratch {
    pub fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("designos-mcp-session-files-{}-{nonce}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        let (workspace, session, victims) = (base.join("ws"), base.join("session"), base.join("victims"));
        for dir in [&workspace, &session, &victims] {
            fs::create_dir_all(dir).unwrap();
        }
        Self { base, workspace, session, victims }
    }

    /// The user's file outside the session folder, holding `bytes`.
    pub fn victim(&self, bytes: &[u8]) -> PathBuf {
        let path = self.victims.join("user-file.txt");
        fs::write(&path, bytes).unwrap();
        path
    }

    /// Where the victim would be, with nothing there yet.
    pub fn missing_victim(&self) -> PathBuf {
        self.victims.join("not-yet-there.txt")
    }

    /// A symbolic link at `name` in the session folder, pointing at `target`.
    pub fn link(&self, name: &str, target: &Path) -> PathBuf {
        let path = self.session.join(name);
        symlink(target, &path).unwrap();
        path
    }

    /// A second name in the session folder for the file `target`.
    pub fn hard_link(&self, name: &str, target: &Path) -> PathBuf {
        let path = self.session.join(name);
        fs::hard_link(target, &path).unwrap();
        path
    }

    pub fn fifo(&self, name: &str) -> PathBuf {
        let path = self.session.join(name);
        assert!(Command::new("mkfifo").arg(&path).status().unwrap().success(), "mkfifo {}", path.display());
        path
    }

    pub fn session(&self) -> Session {
        Session::with_session_dir(&self.workspace, &self.session).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// The name a write to `stem.<ext>` used for its temporary file before temporary names were random.
pub fn old_temp_name(stem: &str) -> String {
    format!("{stem}.tmp-{}", std::process::id())
}

pub fn link_sentence(path: &Path) -> String {
    format!("{} is a link, not a file. Delete the link and try again.", path.display())
}

pub fn not_plain_sentence(path: &Path) -> String {
    format!("{} is not a plain file. Delete it and try again.", path.display())
}

/// The warnings a tool reply carries about the session folder.
pub fn warnings(reply: &Value) -> Vec<String> {
    reply["session_file_warnings"].as_array().map(|all| all.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

pub fn assert_told(reply: &Value, sentence: &str) {
    assert!(warnings(reply).iter().any(|w| w == sentence), "the reply does not carry {sentence:?}; session_file_warnings = {:?}", warnings(reply));
}

pub fn assert_untouched(victim: &Path, expected: &[u8], what: &str) {
    assert_eq!(fs::read(victim).unwrap(), expected, "{what}: the user's file changed");
}

/// Runs `work` and returns its result, which must arrive within a second. When it does not, the FIFO at `fifo` is opened from
/// both ends to free the stuck thread, and the test fails.
pub fn within_one_second<T: Send + 'static>(fifo: &Path, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(work());
    });
    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(result) => result,
        Err(_) => {
            let reader = fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(fifo);
            drop(fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(fifo));
            let freed = rx.recv_timeout(Duration::from_secs(10)).is_ok();
            drop(reader);
            panic!("designos-mcp did not return within 1 s on the FIFO at {} (freed afterwards: {freed})", fifo.display());
        }
    }
}
