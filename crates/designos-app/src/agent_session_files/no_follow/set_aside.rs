//! Moving an entry of a folder into another open folder, whole. `renameat` takes a file, a link, a FIFO or a folder with everything in
//! it from the descriptor of one folder to the descriptor of another, without opening, walking or following anything.

use super::{c_name, cannot, cannot_write_to, failed, retry, Dir, Fix, ATTEMPTS};
use std::ffi::{CStr, OsStr, OsString};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many names `<name>`, `<name>.2`, `<name>.3`, ... are tried for an entry before the move gives up.
const NAME_TRIES: usize = 64;

/// `YYYYMMDD-HHMMSS` of `secs` since the Unix epoch, in UTC.
fn utc_stamp(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // The day of the Gregorian calendar a number of days after 1970-01-01 (Howard Hinnant's `civil_from_days`).
    let z = days as i64 + 719_468;
    let (era, day_of_era) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", rest / 3_600, rest % 3_600 / 60, rest % 60)
}

impl Dir {
    /// The names in this folder that are not in `keep`, sorted.
    pub(in crate::agent_session_files) fn entries_not_in(&self, keep: &[&str]) -> io::Result<Vec<OsString>> {
        let all = self.entries().map_err(|e| cannot("listed", &self.path, e, Fix::CheckPermissions))?;
        let mut names: Vec<OsString> = all.into_iter().filter(|name| !keep.iter().any(|kept| OsStr::new(kept) == name)).collect();
        names.sort();
        Ok(names)
    }

    /// Whether this folder has an entry `name`, a link included (it is never followed).
    pub(in crate::agent_session_files) fn has_entry(&self, name: &OsStr) -> io::Result<bool> {
        match self.kind_of(&c_name(name)?) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(cannot("checked", &self.path.join(name), e, Fix::CheckPermissions)),
        }
    }

    /// Makes a new folder in this one (mode 0700) named `<UTC date>-<UTC time>-<six random hex digits>`, and opens it. A name that is
    /// taken is never reused: the next one is tried.
    pub(in crate::agent_session_files) fn make_stamp_folder(&self) -> io::Result<(Dir, String)> {
        for _ in 0..ATTEMPTS {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
            let name = format!("{}-{}", utc_stamp(secs), &uuid::Uuid::new_v4().simple().to_string()[..6]);
            let c = c_name(OsStr::new(&name))?;
            // SAFETY: `c` is a NUL-terminated name that outlives the call; the descriptor is this folder's.
            match retry(|| unsafe { libc::mkdirat(self.raw(), c.as_ptr(), 0o700) }) {
                Ok(_) => return Ok((self.open_child(OsStr::new(&name), false)?, name)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(cannot_write_to(&self.path, e)),
            }
        }
        Err(names_exhausted(&self.path))
    }

    /// Moves the entry `name` of this folder into `dest` as `dest_name` (or `dest_name.2`, `dest_name.3`, ... when that is taken)
    /// and says which name it got. An entry that is gone, before or while it is moved, is already out of the way: `None`. The move
    /// never replaces: a name somebody took after it was chosen makes the call fail, and the next name is tried.
    pub(in crate::agent_session_files) fn move_entry(&self, name: &OsStr, dest: &Dir, dest_name: &OsStr) -> io::Result<Option<OsString>> {
        let (from, path) = (c_name(name)?, self.path.join(name));
        #[cfg(test)]
        super::test_hooks::run_before_move(&path);
        let still_there = || !matches!(self.kind_of(&from), Err(e) if e.kind() == io::ErrorKind::NotFound);
        for attempt in 1..=NAME_TRIES {
            let mut candidate = dest_name.to_os_string();
            if attempt > 1 {
                candidate.push(format!(".{attempt}"));
            }
            let to = c_name(&candidate)?;
            #[cfg(test)]
            super::test_hooks::run_before_rename(&path, &dest.path.join(&candidate));
            match self.rename_no_replace(&from, dest, &to) {
                Ok(()) => return Ok(Some(candidate)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound && !still_there() => return Ok(None),
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Err(self.permission_denied(&from, dest, &path, e)),
                Err(e) => {
                    let fix = fix_for_move(&e);
                    return Err(cannot("moved", &path, e, fix));
                }
            }
        }
        Err(too_many_of_that_name(&path))
    }

    /// Renames the entry `from` of this folder to `to` in `dest`, and fails with `AlreadyExists` when `to` is taken, whoever took it.
    #[cfg(target_vendor = "apple")]
    fn rename_no_replace(&self, from: &CStr, dest: &Dir, to: &CStr) -> io::Result<()> {
        // SAFETY: both names are NUL-terminated and outlive the call; each is relative to the descriptor of its own folder.
        retry(|| unsafe { libc::renameatx_np(self.raw(), from.as_ptr(), dest.raw(), to.as_ptr(), libc::RENAME_EXCL) }).map(drop)
    }

    #[cfg(target_os = "linux")]
    fn rename_no_replace(&self, from: &CStr, dest: &Dir, to: &CStr) -> io::Result<()> {
        // SAFETY: both names are NUL-terminated and outlive the call; each is relative to the descriptor of its own folder.
        retry(|| unsafe { libc::renameat2(self.raw(), from.as_ptr(), dest.raw(), to.as_ptr(), libc::RENAME_NOREPLACE) }).map(drop)
    }

    /// A unix without a rename that refuses to replace moves nothing: replacing what a racing agent put there is worse than a refusal.
    #[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
    fn rename_no_replace(&self, _from: &CStr, _dest: &Dir, _to: &CStr) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOTSUP))
    }

    /// The refusal for a move the system answered with "permission denied", naming the folder that refuses: the one the entry is in,
    /// the new folder made for this start, or the entry itself when it is a folder (a folder changes its `..` when it changes parent,
    /// which needs write permission on itself). Modes are read from the descriptors and never by a path.
    fn permission_denied(&self, from: &CStr, dest: &Dir, path: &Path, e: io::Error) -> io::Error {
        if !self.owner_can_write() {
            return cannot_write_to(&self.path, e);
        }
        if !dest.owner_can_write() {
            return cannot_write_to(&dest.path, e);
        }
        if self.entry_is_a_folder_the_owner_cannot_write(from) {
            return cannot("moved", path, e, Fix::MakeWritable);
        }
        cannot_write_to(&self.path, e)
    }

    /// Whether the owner has write permission on this folder (`true` when that cannot be read, so no folder is named by mistake).
    fn owner_can_write(&self) -> bool {
        // SAFETY: an all-zero `stat` is a valid value, and `fstat` fills it from the descriptor this folder holds.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        match retry(|| unsafe { libc::fstat(self.raw(), &mut st) }) {
            Ok(_) => st.st_mode & libc::S_IWUSR != 0,
            Err(_) => true,
        }
    }

    /// Whether the entry `name` is a folder, not a link, that the owner has no write permission on.
    fn entry_is_a_folder_the_owner_cannot_write(&self, name: &CStr) -> bool {
        // SAFETY: an all-zero `stat` is a valid value, and `fstatat` fills it; `name` is NUL-terminated and relative to this folder.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        match retry(|| unsafe { libc::fstatat(self.raw(), name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) }) {
            Ok(_) => st.st_mode & libc::S_IFMT == libc::S_IFDIR && st.st_mode & libc::S_IWUSR == 0,
            Err(_) => false,
        }
    }
}

/// The fix for a move that failed for a reason other than a refused permission: what only the user can move or delete (a mount
/// point, another file system, a file system that cannot move without replacing) gets that sentence, the rest the permissions one.
fn fix_for_move(e: &io::Error) -> Fix {
    match e.raw_os_error() {
        Some(code) if [libc::EXDEV, libc::EBUSY, libc::ENOTSUP, libc::EOPNOTSUPP, libc::ENOSYS, libc::EINVAL].contains(&code) => Fix::MoveOrDeleteIt,
        _ => Fix::CheckPermissions,
    }
}

/// Only a racing agent can take every name of a stamp folder.
fn names_exhausted(folder: &Path) -> io::Error {
    failed(io::ErrorKind::AlreadyExists, format!("A new folder can't be made in {}: every name Studio tried was taken", folder.display()), Fix::CloseOtherTabs)
}

/// Only a racing agent can fill every name `<name>.2` to `<name>.64` of the folder an entry is moved to.
fn too_many_of_that_name(path: &Path) -> io::Error {
    cannot("moved", path, io::Error::other("the folder it is moved to holds too many entries of that name"), Fix::CloseOtherTabs)
}

#[cfg(test)]
mod tests {
    use super::{fix_for_move, names_exhausted, too_many_of_that_name, utc_stamp};
    use crate::agent_session_files::{refusal::cannot, start_failure_message};
    use std::path::Path;

    #[test]
    fn a_move_the_user_has_to_make_gets_the_sentence_that_says_so() {
        let line = |code: i32| {
            let e = std::io::Error::from_raw_os_error(code);
            let (cause, fix) = (e.to_string(), fix_for_move(&e));
            (start_failure_message("Grok Build", &cannot("moved", Path::new("/s/.mcp.json"), e, fix)), cause)
        };
        // Another file system, a mount point, and a file system that cannot move without replacing: only the user can move or delete it.
        for code in [libc::EXDEV, libc::EBUSY, libc::ENOTSUP, libc::EINVAL] {
            let (message, cause) = line(code);
            assert_eq!(message, format!("Couldn't start Grok Build: /s/.mcp.json can't be moved: {cause}. Move or delete it, then try again."));
        }
        let (message, cause) = line(libc::EIO);
        assert_eq!(message, format!("Couldn't start Grok Build: /s/.mcp.json can't be moved: {cause}. Check its permissions and free space, then try again."));
    }

    #[test]
    fn a_racing_agent_that_takes_every_name_stops_the_start_with_the_fix_that_helps() {
        assert_eq!(
            start_failure_message("Grok Build", &names_exhausted(Path::new("/s/.designos-set-aside"))),
            "Couldn't start Grok Build: A new folder can't be made in /s/.designos-set-aside: every name Studio tried was taken. Close this file's other agent tabs, then try again."
        );
        assert_eq!(
            start_failure_message("Grok Build", &too_many_of_that_name(Path::new("/s/.mcp.json"))),
            "Couldn't start Grok Build: /s/.mcp.json can't be moved: the folder it is moved to holds too many entries of that name. Close this file's other agent tabs, then try again."
        );
    }

    #[test]
    fn a_stamp_is_the_utc_date_and_time() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(1_700_000_000), "20231114-221320");
        assert_eq!(utc_stamp(951_782_399), "20000228-235959");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000", "2000 was a leap year");
        assert_eq!(utc_stamp(4_102_444_799), "20991231-235959");
    }
}
