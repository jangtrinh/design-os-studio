use crate::fontsource::{font_id, metadata_url, FontMeta};
use crate::{bundled_for, FontError, FontRequest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Source of bytes for a URL. The default implementation (feature `net`) is a
/// blocking `ureq` client; tests inject their own.
pub trait Fetch: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, FontError>;
}

#[cfg(feature = "net")]
struct UreqFetch {
    agent: ureq::Agent,
}

#[cfg(feature = "net")]
impl UreqFetch {
    fn new() -> Self {
        use std::time::Duration;
        let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout(Duration::from_secs(60)).user_agent("designos-fonts").build();
        Self { agent }
    }
}

#[cfg(feature = "net")]
impl Fetch for UreqFetch {
    fn get(&self, url: &str) -> Result<Vec<u8>, FontError> {
        use std::io::Read;
        const MAX: u64 = 64 * 1024 * 1024;
        match self.agent.get(url).call() {
            Ok(resp) => {
                let mut buf = Vec::new();
                resp.into_reader().take(MAX).read_to_end(&mut buf).map_err(|e| FontError::Network(e.to_string()))?;
                Ok(buf)
            }
            Err(ureq::Error::Status(404, _)) => Err(FontError::NotFound(url.to_string())),
            Err(e) => Err(FontError::Network(e.to_string())),
        }
    }
}

/// License record stored next to each cached font (`<name>.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub family: String,
    pub license: String,
    /// Weight that was requested.
    pub requested_weight: u16,
    /// Weight of the file actually stored (nearest available).
    pub weight: u16,
    pub italic: bool,
    pub source_url: String,
}

/// On-disk font cache with optional network fallback.
///
/// Layout: `<dir>/<fontsource-id>-<weight>-<normal|italic>.ttf` plus a
/// `.json` sidecar holding the license id.
pub struct FontCache {
    dir: PathBuf,
    fetcher: Option<Box<dyn Fetch>>,
}

impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FontCache {
    /// `dirs::cache_dir()/designos/fonts` (falls back to the temp dir), with the
    /// built-in HTTP fetcher when the `net` feature is on.
    pub fn new() -> Self {
        Self::with_dir(Self::default_dir())
    }

    pub fn default_dir() -> PathBuf {
        dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("designos").join("fonts")
    }

    /// Cache rooted at `dir` (used by tests).
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        #[cfg(feature = "net")]
        let fetcher: Option<Box<dyn Fetch>> = Some(Box::new(UreqFetch::new()));
        #[cfg(not(feature = "net"))]
        let fetcher: Option<Box<dyn Fetch>> = None;
        Self { dir: dir.into(), fetcher }
    }

    /// Replace the network layer.
    pub fn with_fetcher(mut self, f: Box<dyn Fetch>) -> Self {
        self.fetcher = Some(f);
        self
    }

    /// Never touch the network; only bundled fonts and cached files resolve.
    pub fn offline(mut self) -> Self {
        self.fetcher = None;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn stem(req: &FontRequest) -> String {
        format!("{}-{}-{}", font_id(&req.family), req.weight, if req.italic { "italic" } else { "normal" })
    }

    pub fn font_path(&self, req: &FontRequest) -> PathBuf {
        self.dir.join(format!("{}.ttf", Self::stem(req)))
    }

    pub fn sidecar_path(&self, req: &FontRequest) -> PathBuf {
        self.dir.join(format!("{}.json", Self::stem(req)))
    }

    /// License record for a cached font, if present.
    pub fn sidecar(&self, req: &FontRequest) -> Option<Sidecar> {
        serde_json::from_slice(&std::fs::read(self.sidecar_path(req)).ok()?).ok()
    }

    /// Cached bytes only (no bundled lookup, no network).
    pub fn cached(&self, req: &FontRequest) -> Option<Vec<u8>> {
        std::fs::read(self.font_path(req)).ok().filter(|b| !b.is_empty())
    }

    /// Bundled font, else cached file, else fetch from Fontsource and cache it.
    /// Only open-licensed fonts are accepted.
    pub fn resolve(&self, req: &FontRequest) -> Result<Vec<u8>, FontError> {
        if let Some(a) = bundled_for(req) {
            return Ok(a.data.to_vec());
        }
        if let Some(bytes) = self.cached(req) {
            return Ok(bytes);
        }
        let fetcher = self.fetcher.as_deref().ok_or_else(|| FontError::Offline(format!("'{}' is not cached and fetching is disabled", req.family)))?;
        let meta_bytes = fetcher.get(&metadata_url(&req.family)).map_err(|e| match e {
            FontError::NotFound(_) => FontError::NotFound(req.family.clone()),
            other => other,
        })?;
        let meta = FontMeta::parse(&String::from_utf8_lossy(&meta_bytes))?;
        if !meta.is_open() {
            return Err(FontError::NotOpenSource { family: meta.family, license: meta.license });
        }
        let pick = meta.pick(req.weight, req.italic)?;
        let bytes = fetcher.get(&pick.url)?;
        if bytes.len() < 12 || !matches!(&bytes[..4], [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf") {
            return Err(FontError::Metadata(format!("{} is not a TTF/OTF file", pick.url)));
        }
        self.store(
            req,
            &bytes,
            &Sidecar {
                family: meta.family,
                license: meta.license,
                requested_weight: req.weight,
                weight: pick.weight,
                italic: pick.italic,
                source_url: pick.url,
            },
        )?;
        Ok(bytes)
    }

    fn store(&self, req: &FontRequest, bytes: &[u8], side: &Sidecar) -> Result<(), FontError> {
        std::fs::create_dir_all(&self.dir)?;
        // Sidecar first so a font file never exists without its license record.
        write_atomic(&self.sidecar_path(req), &serde_json::to_vec_pretty(side).map_err(|e| FontError::Metadata(e.to_string()))?)?;
        write_atomic(&self.font_path(req), bytes)?;
        Ok(())
    }
}

/// Distinguishes the temporary files of concurrent saves within one process.
static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Writes `bytes` to `path` via a temporary file next to it, so a crash mid-write never leaves a
/// truncated file behind.
///
/// Behaviour a plain `std::fs::write` has and this keeps:
/// - a symlink (including a dangling symlink) is saved *through*: the target file receives the
///   bytes (and is created along with its parent directories if missing) while the link stays a link;
/// - the existing file's permission mode is applied at temp-file creation time (never briefly wider);
/// - a read-only or non-writable target (including a `0644` file owned by another user or denied by
///   ACL/flags in a writable folder) is refused with `PermissionDenied` instead of being replaced;
/// - filenames of 249–255 bytes (e.g. ~80+ Vietnamese UTF-8 characters) truncate the temp prefix on
///   a char boundary so the temp filename never hits `ENAMETOOLONG`;
/// - leftover `<prefix>.save-<pid>-<seq>.tmp` files from crashed saves (dead PIDs) are swept on save;
///   only names this function writes are touched, never a user file that merely ends in `.tmp`;
/// - a target that is not a regular file (named pipe, socket, device) is refused with `InvalidInput`
///   before it is opened, so saving onto a pipe does not wait for a reader.
///
/// Limitation: the rename installs a new file, so ACLs, extended attributes, ownership and hard
/// links of the replaced file are not carried over (the permission mode is).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (target, existing) = resolve_save_target(path)?;
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let prefix = tmp_prefix(&name);
    sweep_stale_temps(dir, &name, prefix);
    write_via_tmp(&target, dir, prefix, bytes, existing.as_ref())
}

/// Writes atomically while refusing a symbolic link at the final path component.
///
/// This is for an agent supplied output path. The ordinary [`write_atomic`] contract follows
/// links because library and editor saves intentionally preserve that behaviour.
pub fn write_atomic_no_follow(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let existing = inspect_no_follow_target(path)?;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let prefix = tmp_prefix(&name);
    sweep_stale_temps(dir, &name, prefix);
    write_via_tmp(path, dir, prefix, bytes, existing.as_ref())
}

/// Truncates `name` on a UTF-8 char boundary so appending `.{pid}-{seq}.tmp` never exceeds
/// the 255-byte / 255-char filesystem filename limit (`ENAMETOOLONG`).
fn tmp_prefix(name: &str) -> &str {
    const MAX_PREFIX_BYTES: usize = 200;
    if name.len() <= MAX_PREFIX_BYTES {
        return name;
    }
    let mut end = MAX_PREFIX_BYTES;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

#[cfg(unix)]
fn is_process_dead(pid: u32) -> bool {
    const ESRCH: i32 = 3;
    if pid == std::process::id() || (pid as i32) <= 0 {
        return false;
    }
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if unsafe { kill(pid as i32, 0) } == 0 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(ESRCH)
}

#[cfg(not(unix))]
fn is_process_dead(_pid: u32) -> bool {
    false
}

/// Fixed word between the target name and the `<pid>-<seq>` fields of every temporary file this
/// writer creates. A user file that only looks like `<name>.<digits>-<digits>.tmp` lacks it, so the
/// sweep can tell a crashed save from a user's file.
const TMP_MARKER: &str = "save";

/// The temporary file name of save number `seq` by process `pid`: `<prefix>.save-<pid>-<seq>.tmp`.
fn tmp_file_name(prefix: &str, pid: u32, seq: u64) -> String {
    format!("{prefix}.{TMP_MARKER}-{pid}-{seq}.tmp")
}

/// The pid inside a name produced by [`tmp_file_name`] for `prefix`, or `None` for any other name.
fn tmp_file_pid(file_name: &str, prefix: &str) -> Option<u32> {
    let stem = file_name.strip_suffix(".tmp")?;
    let fields = stem.strip_prefix(prefix)?.strip_prefix('.')?.strip_prefix(TMP_MARKER)?.strip_prefix('-')?;
    let (pid, seq) = fields.split_once('-')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(pid) || !digits(seq) {
        return None;
    }
    pid.parse().ok()
}

/// Best-effort sweep of leftover `.tmp` files for `target` left behind by crashed processes.
fn sweep_stale_temps(dir: &Path, target_name: &str, prefix: &str) {
    let legacy = dir.join(format!("{target_name}.tmp"));
    let _ = std::fs::remove_file(legacy);

    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let Some(pid) = fname.to_str().and_then(|s| tmp_file_pid(s, prefix)) else { continue };
        if is_process_dead(pid) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Follows symlinks (including dangling chains) to the file that must really change. Returns the
/// metadata of that file when it exists, and refuses a directory, a non-regular file or a
/// non-writable file.
fn resolve_save_target(path: &Path) -> std::io::Result<(PathBuf, Option<std::fs::Metadata>)> {
    use std::io::{Error, ErrorKind};
    let mut cur = path.to_path_buf();
    for _ in 0..40 {
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let link_target = std::fs::read_link(&cur)?;
                let parent = cur.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
                cur = parent.join(link_target);
            }
            Ok(meta) => {
                if meta.is_dir() {
                    return Err(Error::new(ErrorKind::InvalidInput, format!("{} is a directory", cur.display())));
                }
                // Opening a named pipe for writing waits for a reader, so anything that is not a
                // regular file is refused before the writability probe below. (A file swapped for
                // a pipe between this check and that open is not covered.)
                if !meta.is_file() {
                    return Err(Error::new(ErrorKind::InvalidInput, format!("{} is not a regular file", cur.display())));
                }
                // The mode bits are checked on their own, before the open probe: a file with no
                // write bit that is writable only through an ACL would be saved once, and the
                // renamed-in replacement (which carries no ACL) would then refuse every later save.
                if meta.permissions().readonly() {
                    return Err(Error::new(ErrorKind::PermissionDenied, format!("{} is read-only", cur.display())));
                }
                if let Err(e) = std::fs::OpenOptions::new().write(true).open(&cur) {
                    return Err(Error::new(ErrorKind::PermissionDenied, format!("{} is read-only or not writable: {e}", cur.display())));
                }
                let real = std::fs::canonicalize(&cur).unwrap_or(cur);
                return Ok((real, Some(meta)));
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Ok((cur, None));
            }
            Err(e) => return Err(e),
        }
    }
    Err(Error::new(ErrorKind::InvalidInput, format!("too many levels of symbolic links: {}", path.display())))
}

fn inspect_no_follow_target(path: &Path) -> std::io::Result<Option<std::fs::Metadata>> {
    use std::io::{Error, ErrorKind};
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if meta.file_type().is_symlink() {
        return Err(Error::new(ErrorKind::InvalidInput, format!("{} is a symbolic link", path.display())));
    }
    if !meta.is_file() {
        return Err(Error::new(ErrorKind::InvalidInput, format!("{} is not a regular file", path.display())));
    }
    if meta.permissions().readonly() {
        return Err(Error::new(ErrorKind::PermissionDenied, format!("{} is read-only", path.display())));
    }
    if let Err(e) = std::fs::OpenOptions::new().write(true).open(path) {
        return Err(Error::new(ErrorKind::PermissionDenied, format!("{} is read-only or not writable: {e}", path.display())));
    }
    Ok(Some(meta))
}

fn write_via_tmp(target: &Path, dir: &Path, prefix: &str, bytes: &[u8], existing: Option<&std::fs::Metadata>) -> std::io::Result<()> {
    use std::io::{ErrorKind, Write};
    use std::sync::atomic::Ordering;

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let initial_mode = existing.map(|m| m.permissions().mode() & 0o7777).unwrap_or(0o666);
        opts.mode(initial_mode);
    }

    let (tmp, mut file) = loop {
        let tmp = dir.join(tmp_file_name(prefix, std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed)));
        match opts.open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let res = (|| -> std::io::Result<()> {
        if let Some(meta) = existing {
            file.set_permissions(meta.permissions())?;
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, target)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return res;
    }
    sync_dir(dir)
}

/// Makes the rename itself durable. Platforms that cannot open or sync a directory are skipped.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    match std::fs::File::open(dir).and_then(|d| d.sync_all()) {
        Err(e) if matches!(e.kind(), std::io::ErrorKind::InvalidInput | std::io::ErrorKind::Unsupported) => {}
        other => return other,
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    const CARLITO: &str = include_str!("../tests/fixtures/fontsource_carlito.json");

    struct Mock {
        routes: HashMap<String, Vec<u8>>,
        hits: Arc<Mutex<Vec<String>>>,
    }

    impl Fetch for Mock {
        fn get(&self, url: &str) -> Result<Vec<u8>, FontError> {
            self.hits.lock().unwrap().push(url.to_string());
            self.routes.get(url).cloned().ok_or_else(|| FontError::NotFound(url.to_string()))
        }
    }

    fn fake_ttf() -> Vec<u8> {
        let mut v = vec![0, 1, 0, 0];
        v.extend_from_slice(&[7u8; 64]);
        v
    }

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("designos-fonts-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn mock_cache(dir: &Path, meta: &str, hits: Arc<Mutex<Vec<String>>>) -> FontCache {
        let m = FontMeta::parse(meta).unwrap();
        let mut routes = HashMap::new();
        routes.insert(metadata_url(&m.family), meta.as_bytes().to_vec());
        for subs in m.ttf.values().flat_map(|s| s.values()) {
            for url in subs.values() {
                routes.insert(url.clone(), fake_ttf());
            }
        }
        FontCache::with_dir(dir).with_fetcher(Box::new(Mock { routes, hits }))
    }

    #[test]
    fn fetches_stores_then_serves_from_cache() {
        let dir = tempdir("roundtrip");
        let hits = Arc::new(Mutex::new(Vec::new()));
        let cache = mock_cache(&dir, CARLITO, hits.clone());
        let req = FontRequest::new("Carlito", 600, false);
        assert!(cache.cached(&req).is_none());

        let bytes = cache.resolve(&req).unwrap();
        assert_eq!(bytes, fake_ttf());
        assert_eq!(hits.lock().unwrap().len(), 2); // metadata + ttf
        assert!(cache.font_path(&req).ends_with("carlito-600-normal.ttf"));
        let side = cache.sidecar(&req).unwrap();
        assert_eq!(side.license, "OFL-1.1");
        assert_eq!((side.requested_weight, side.weight, side.italic), (600, 700, false));
        assert!(side.source_url.ends_with("latin-700-normal.ttf"));

        // Second resolve, even through an offline cache, uses the disk copy.
        let offline = FontCache::with_dir(&dir).offline();
        assert_eq!(offline.resolve(&req).unwrap(), fake_ttf());
        assert_eq!(hits.lock().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn offline_miss_is_an_error_not_a_panic() {
        let dir = tempdir("offline");
        let cache = FontCache::with_dir(&dir).offline();
        let err = cache.resolve(&FontRequest::new("Roboto", 400, false)).unwrap_err();
        assert!(matches!(err, FontError::Offline(_)));
        assert!(!dir.exists());
    }

    #[test]
    fn bundled_inter_needs_no_cache_or_network() {
        let cache = FontCache::with_dir(tempdir("bundled")).offline();
        let r = cache.resolve(&FontRequest::new("inter", 700, true)).unwrap();
        assert_eq!(r.as_slice(), crate::bundled().iter().find(|a| a.italic).unwrap().data);
    }

    #[test]
    fn closed_license_is_rejected_and_nothing_is_cached() {
        let dir = tempdir("closed");
        let meta = CARLITO.replace("\"license\":\"OFL-1.1\"", "\"license\":\"Proprietary\"");
        assert_ne!(meta, CARLITO);
        let cache = mock_cache(&dir, &meta, Arc::default());
        let err = cache.resolve(&FontRequest::new("Carlito", 400, false)).unwrap_err();
        assert!(matches!(err, FontError::NotOpenSource { ref license, .. } if license == "Proprietary"), "{err:?}");
        assert!(!dir.exists());
    }

    #[test]
    fn non_font_payload_is_rejected() {
        let dir = tempdir("junk");
        let m = FontMeta::parse(CARLITO).unwrap();
        let url = m.pick(400, false).unwrap().url;
        let mut routes = HashMap::new();
        routes.insert(metadata_url("Carlito"), CARLITO.as_bytes().to_vec());
        routes.insert(url, b"<html>oops</html>".to_vec());
        let cache = FontCache::with_dir(&dir).with_fetcher(Box::new(Mock { routes, hits: Arc::default() }));
        assert!(cache.resolve(&FontRequest::new("Carlito", 400, false)).is_err());
        assert!(!cache.font_path(&FontRequest::new("Carlito", 400, false)).exists());
    }

    #[test]
    fn unknown_family_maps_to_not_found() {
        let cache = FontCache::with_dir(tempdir("nf")).with_fetcher(Box::new(Mock { routes: HashMap::new(), hits: Arc::default() }));
        let err = cache.resolve(&FontRequest::new("No Such Font", 400, false)).unwrap_err();
        assert!(matches!(err, FontError::NotFound(ref f) if f == "No Such Font"), "{err:?}");
    }

    #[test]
    fn temp_names_round_trip_and_nothing_else_is_taken_for_a_crashed_save() {
        let name = tmp_file_name("doc.designos", 4242, 17);
        assert_eq!(tmp_file_pid(&name, "doc.designos"), Some(4242));
        for other in [
            "doc.designos.4242-17.tmp",            // digit pair without the marker: a user's file
            "doc.designos.save-4242.tmp",          // missing seq
            "doc.designos.save--17.tmp",           // empty pid
            "doc.designos.save-42a-17.tmp",        // non-digit pid
            "doc.designos.save-4242-1x.tmp",       // non-digit seq
            "doc.designos.save-4242-17",           // no .tmp suffix
            "other.designos.save-4242-17.tmp",     // another target
            "doc.designos.save-99999999999-1.tmp", // pid beyond u32
        ] {
            assert_eq!(tmp_file_pid(other, "doc.designos"), None, "{other}");
        }
    }

    /// The pid of a process that has already exited and been reaped.
    #[cfg(unix)]
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().expect("`true` is available on unix");
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[cfg(unix)]
    #[test]
    fn leftover_of_a_dead_process_is_swept_but_a_look_alike_user_file_is_kept() {
        let dir = tempdir("sweep");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("doc.designos");
        std::fs::write(&target, b"v1").unwrap();
        let pid = dead_pid();
        let leftover = dir.join(tmp_file_name("doc.designos", pid, 0));
        let user_file = dir.join(format!("doc.designos.{pid}-0.tmp"));
        std::fs::write(&leftover, b"partial").unwrap();
        std::fs::write(&user_file, b"mine").unwrap();

        write_atomic(&target, b"v2").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"v2");
        assert!(!leftover.exists(), "the leftover of a crashed save is swept");
        assert_eq!(std::fs::read(&user_file).unwrap(), b"mine", "a user file with the same digits is not a leftover");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_dir_is_under_designos_fonts() {
        let d = FontCache::default_dir();
        assert!(d.ends_with(Path::new("designos").join("fonts")));
    }
}
