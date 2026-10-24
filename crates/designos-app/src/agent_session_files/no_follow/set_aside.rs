//! Moving an entry of a folder into another open folder, whole. `renameat` takes a file, a link, a FIFO or a folder with everything in
//! it from the descriptor of one folder to the descriptor of another, without opening, walking or following anything.

use super::{c_name, cannot, cannot_write_to, retry, Dir, Fix, ATTEMPTS};
use std::ffi::{OsStr, OsString};
use std::io;
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
        Err(cannot("created", &self.path, io::Error::other("every name it tried was taken"), Fix::CheckPermissions))
    }

    /// Moves the entry `name` of this folder into `dest` as `dest_name` (or `dest_name.2`, `dest_name.3`, ... when that is taken)
    /// and says which name it got. An entry that is gone, before or while it is moved, is already out of the way: `None`.
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
            match dest.kind_of(&to) {
                Ok(_) => continue,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(cannot("moved", &path, e, Fix::CheckPermissions)),
            }
            // SAFETY: both names are NUL-terminated and outlive the call; each is relative to the descriptor of its own folder.
            match retry(|| unsafe { libc::renameat(self.raw(), from.as_ptr(), dest.raw(), to.as_ptr()) }) {
                Ok(_) => return Ok(Some(candidate)),
                Err(e) if e.kind() == io::ErrorKind::NotFound && !still_there() => return Ok(None),
                // The folder the entry is in is what refuses: it was made read-only.
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Err(cannot_write_to(&self.path, e)),
                Err(e) => return Err(cannot("moved", &path, e, Fix::CheckPermissions)),
            }
        }
        Err(cannot("moved", &path, io::Error::other("the folder it is moved to holds too many entries of that name"), Fix::CheckPermissions))
    }
}

#[cfg(test)]
mod tests {
    use super::utc_stamp;

    #[test]
    fn a_stamp_is_the_utc_date_and_time() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(1_700_000_000), "20231114-221320");
        assert_eq!(utc_stamp(951_782_399), "20000228-235959");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000", "2000 was a leap year");
        assert_eq!(utc_stamp(4_102_444_799), "20991231-235959");
    }
}
