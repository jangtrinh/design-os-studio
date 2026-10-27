//! What went wrong with a session file, kept until a tool reply can carry it to the agent. A session file that cannot be read
//! or written is not a reason to stop a tool, but Studio then stops seeing the agent's edits or its settings, and the agent
//! must hear which path to clean up.
use crate::session_file_io;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct SessionFileProblems {
    /// Sentences not yet handed to a reply.
    pending: Vec<String>,
    /// The last sentence kept for each path while it fails, so a problem that stays is told once and one that returns is told again.
    kept: BTreeMap<PathBuf, String>,
}

impl SessionFileProblems {
    /// The text of session file `path`. A file that is not there is `None` without a word (the editor has not written it yet);
    /// any other failure is `None` and is kept for the agent.
    pub fn read_to_string(&mut self, path: &Path) -> Option<String> {
        let read = session_file_io::read_to_string(path);
        self.settle_read(path, read)
    }

    /// The bytes of session file `path`; see [`Self::read_to_string`].
    pub fn read(&mut self, path: &Path) -> Option<Vec<u8>> {
        let read = session_file_io::read(path);
        self.settle_read(path, read)
    }

    /// When session file `path` was last changed; see [`Self::read_to_string`].
    pub fn modified(&mut self, path: &Path) -> Option<SystemTime> {
        let modified = session_file_io::stat(path).and_then(|meta| meta.modified());
        self.settle_read(path, modified)
    }

    /// Records how a write to session file `path` ended: an error is kept for the agent, a success clears the path.
    pub fn wrote(&mut self, path: &Path, written: io::Result<()>) {
        match written {
            Ok(()) => {
                self.kept.remove(path);
            }
            Err(e) => self.keep(path, session_file_io::explain("write", path, &e)),
        }
    }

    /// Keeps `sentence` about `path`, unless that very sentence is the one kept for it.
    pub fn keep(&mut self, path: &Path, sentence: String) {
        if self.kept.get(path) != Some(&sentence) {
            self.kept.insert(path.to_path_buf(), sentence.clone());
            self.pending.push(sentence);
        }
    }

    /// The sentences kept since the last call.
    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }

    fn settle_read<T>(&mut self, path: &Path, read: io::Result<T>) -> Option<T> {
        match read {
            Ok(value) => {
                self.kept.remove(path);
                Some(value)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.kept.remove(path);
                None
            }
            Err(e) => {
                self.keep(path, session_file_io::explain("read", path, &e));
                None
            }
        }
    }
}
