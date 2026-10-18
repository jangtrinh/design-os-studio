//! Native plugin bundle storage. Installation never evaluates downloaded code.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

const REPOSITORY: &str = "jangtrinh/design-os-plugins";
const ALLOWED_REPOS: &[(&str, &str)] = &[("jangtrinh", "design-os-plugins"), ("jangtrinh", "designos-plugins")];
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_FIGMA_VERSION: &str = "1.0.0";
#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
const IMPORT_RETRY: &str = "Correct the plugin folder, then import again.";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub license: String,
    pub source_url: String,
    pub manifest_url: String,
    pub main: String,
    pub permissions: Vec<String>,
    pub runtime_supported: bool,
    pub installed_at: u64,
    #[serde(default = "schema_one")]
    pub schema_version: u32,
    #[serde(default)]
    pub panel: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub network: Vec<String>,
}

fn schema_one() -> u32 {
    1
}

#[derive(Debug)]
pub struct Bundle {
    pub plugin: InstalledPlugin,
    pub panel: String,
    pub files: BTreeMap<String, Vec<u8>>,
    pub network: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    id: String,
    name: String,
    version: String,
    license: String,
    main: String,
    permissions: Vec<String>,
    #[serde(default)]
    panel: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    network: Vec<String>,
}

/// Canonical raw manifest URL. This intentionally accepts only the native registry.
pub fn validate_url(input: &str) -> Result<String, String> {
    let url = Url::parse(input.trim()).map_err(|_| "Paste a valid HTTPS plugin manifest URL.".to_string())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use an HTTPS manifest URL without credentials, a port, query, or fragment.".into());
    }
    // Reject encoded separators and dot segments before URL normalization can hide them.
    if input.contains('%') || input.contains('\\') || input.split('/').any(|p| p == "." || p == "..") {
        return Err("Plugin URLs cannot contain escaped paths or parent-directory segments.".into());
    }
    let parts: Vec<_> = url.path().trim_start_matches('/').split('/').collect();
    let native = match url.host_str() {
        Some("github.com") if parts.len() == 7 && parts[2] == "blob" => vec![parts[0], parts[1], parts[3], parts[4], parts[5], parts[6]],
        Some("raw.githubusercontent.com") if parts.len() == 6 => parts,
        _ => return Err(format!("Install a native manifest from https://github.com/{REPOSITORY}. Figma Community plugins are not supported.")),
    };
    if !ALLOWED_REPOS.iter().any(|(owner, repo)| native[0] == *owner && native[1] == *repo)
        || native[3] != "plugins"
        || native[5] != "manifest.json"
        || !safe_part(native[2])
        || !safe_part(native[4])
    {
        return Err(format!("Use a manifest at {REPOSITORY}/blob/<ref>/plugins/<name>/manifest.json."));
    }
    Ok(format!("https://raw.githubusercontent.com/{}", native.join("/")))
}

fn safe_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn validate_asset_path(path: &str) -> Result<(), String> {
    if path.len() > 240 || !path.split('/').all(safe_part) || path.contains('\\') {
        return Err("Plugin files must use safe relative paths.".into());
    }
    for part in path.split('/') {
        let stem = part.split('.').next().unwrap_or_default().to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) || part.ends_with('.')
        {
            return Err("Plugin file contains a reserved name.".into());
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if !matches!(manifest.schema_version, 1 | 2) {
        return Err("Unsupported plugin schema_version; this build supports versions 1 and 2.".into());
    }
    if !safe_part(&manifest.id) || !safe_part(&manifest.version) || manifest.id.len() + manifest.version.len() > 100 {
        return Err("Plugin id and version must contain only letters, digits, dots, dashes, or underscores (up to 100 characters combined).".into());
    }
    if manifest.name.trim().is_empty() || manifest.name.len() > 200 || manifest.license.trim().is_empty() || manifest.license.len() > 200 {
        return Err("Plugin name and license are required and must be at most 200 characters.".into());
    }
    validate_asset_path(&manifest.main)?;
    if !manifest.main.ends_with(".js") {
        return Err("Plugin main must be a relative .js path.".into());
    }
    let assets = if manifest.schema_version == 1 {
        if !manifest.permissions.is_empty() || manifest.panel.is_some() || !manifest.files.is_empty() || !manifest.network.is_empty() {
            return Err("Schema 1 plugins cannot declare permissions, panel, files, or network.".into());
        }
        vec![manifest.main.clone()]
    } else {
        let panel = manifest.panel.as_ref().ok_or("Schema 2 requires a panel HTML file.")?;
        validate_asset_path(panel)?;
        if panel.contains('/')
            || !panel.ends_with(".html")
            || manifest.files.is_empty()
            || manifest.files.len() > 16
            || !manifest.files.contains(panel)
            || !manifest.files.contains(&manifest.main)
        {
            return Err("Schema 2 must list its HTML panel and main script in files (at most 16 assets).".into());
        }
        let mut permissions = BTreeSet::new();
        if manifest.permissions.iter().any(|p| !matches!(p.as_str(), "document:insert-svg" | "storage:local") || !permissions.insert(p)) {
            return Err("Unsupported or duplicate plugin permission.".into());
        }
        if manifest.network.len() > 1 || manifest.network.iter().any(|origin| origin != "https://api.iconify.design") {
            return Err("Only the https://api.iconify.design network origin is supported.".into());
        }
        manifest.files.clone()
    };
    let mut names =
        BTreeSet::from(["manifest.json".to_string(), "installed.json".to_string(), "license".to_string(), "host.json".to_string(), "insert-svg".to_string()]);
    let mut path_spellings = BTreeMap::new();
    for asset in assets {
        validate_asset_path(&asset)?;
        let mut prefix = String::new();
        for part in asset.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if let Some(previous) = path_spellings.insert(prefix.to_ascii_lowercase(), prefix.clone()) {
                if previous != prefix {
                    return Err("Plugin paths differ only by letter case.".into());
                }
            }
        }
        let key = asset.to_ascii_lowercase();
        if !names.insert(key) {
            return Err("Plugin files collide with another asset or reserved metadata.".into());
        }
    }
    for name in &names {
        let mut parent = name.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if names.contains(prefix) {
                return Err("Plugin asset path has a file/directory collision.".into());
            }
            parent = prefix;
        }
    }
    Ok(())
}

fn bundle_directory(root: &Path, id: &str, version: &str) -> PathBuf {
    let encode = |s: &str| s.bytes().map(|b| format!("{b:02x}")).collect::<String>();
    root.join("plugins").join(format!("{}--{}", encode(id), encode(version)))
}

fn read_bundle_file(directory: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(directory).map_err(|e| format!("Cannot locate plugin bundle: {e}"))?;
    if metadata.file_type().is_symlink() {
        return Err("Plugin bundles cannot be symbolic links.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Plugin bundles cannot be reparse points.".into());
        }
    }
    let canonical = fs::canonicalize(directory).map_err(|e| format!("Cannot locate plugin bundle: {e}"))?;
    let mut path = directory.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        let metadata = fs::symlink_metadata(&path).map_err(|e| format!("Cannot read plugin asset {relative}: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err("Plugin assets cannot be symbolic links.".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Plugin assets cannot be reparse points.".into());
            }
        }
        if !fs::canonicalize(&path).map_err(|e| e.to_string())?.starts_with(&canonical) {
            return Err("Plugin asset escapes its bundle.".into());
        }
    }
    let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err("Invalid or oversized plugin asset.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path).map_err(|e| e.to_string())?.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    check_size(bytes)
}

/// Loads only declared assets, validating the manifest again before execution.
pub fn load_bundle(root: &Path, plugin: &InstalledPlugin) -> Result<Bundle, String> {
    let directory = bundle_directory(root, &plugin.id, &plugin.version);
    let bytes = read_bundle_file(&directory, "manifest.json")?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid stored manifest: {e}"))?;
    validate_manifest(&manifest)?;
    if manifest.id != plugin.id || manifest.version != plugin.version {
        return Err("Stored manifest identity does not match the installed plugin.".into());
    }
    if manifest.schema_version != 2 {
        return Err("Schema 1 bundles do not support execution.".into());
    }
    let license = read_bundle_file(&directory, "LICENSE")?;
    if std::str::from_utf8(&license).map_or(true, |s| s.trim().is_empty()) {
        return Err("Missing or invalid plugin license.".into());
    }
    let mut total = bytes.len() + license.len();
    let mut files = BTreeMap::new();
    for path in &manifest.files {
        let bytes = read_bundle_file(&directory, path)?;
        total += bytes.len();
        if total > MAX_TOTAL_BYTES {
            return Err("Plugin bundle exceeds 8 MiB.".into());
        }
        files.insert(path.clone(), bytes);
    }
    let mut plugin = plugin.clone();
    plugin.name = manifest.name;
    plugin.license = manifest.license;
    plugin.main = manifest.main;
    plugin.schema_version = manifest.schema_version;
    plugin.panel = manifest.panel.clone();
    plugin.files = manifest.files;
    plugin.permissions = manifest.permissions;
    plugin.network = manifest.network.clone();
    plugin.runtime_supported = true;
    Ok(Bundle { plugin, panel: manifest.panel.expect("validated panel"), files, network: manifest.network })
}

pub fn install(root: &Path, input: &str) -> Result<InstalledPlugin, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirects(0)
        .user_agent("designos-plugin-installer")
        .build();
    let token = std::env::var("DESIGNOS_PLUGINS_TOKEN").or_else(|_| std::env::var("DESIGNOS_PLUGINS_TOKEN")).ok().filter(|value| !value.trim().is_empty());
    install_with(root, input, |url| {
        // All supplied URLs were constructed from the validated native repository path.
        // Send credentials only to GitHub's API, never to downloaded plugin code.
        let source = Url::parse(url).map_err(|_| "Invalid plugin download URL.".to_string())?;
        let segments: Vec<_> = source.path().trim_start_matches('/').split('/').collect();
        let mut api_url = Url::parse(&format!("https://api.github.com/repos/{}/{}/contents/{}", segments[0], segments[1], segments[3..].join("/")))
            .map_err(|_| "Invalid plugin download path.".to_string())?;
        api_url.query_pairs_mut().append_pair("ref", segments[2]);
        let mut request = agent.get(api_url.as_str()).set("Accept", "application/vnd.github.raw+json").set("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &token {
            request = request.set("Authorization", &format!("Bearer {}", token.trim()));
        }
        let response = request.call().map_err(|err| -> String { match err {
            ureq::Error::Status(401 | 403 | 404, _) => format!("The plugin repository or file is unavailable. For a private registry, set DESIGNOS_PLUGINS_TOKEN (or DESIGNOS_PLUGINS_TOKEN) to a GitHub token with read access to {REPOSITORY}, then restart design:os studio."),
            _ => "Plugin download failed. Check your connection and the manifest URL.".into(),
        } })?;
        if response.status() != 200 {
            return Err("The plugin URL must return a file directly; redirects are not supported.".into());
        }
        let mut bytes = Vec::new();
        response.into_reader().take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(|e| format!("Cannot read plugin download: {e}"))?;
        check_size(bytes)
    })
}

fn check_size(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_BYTES {
        Err("Plugin files must be no larger than 2 MiB each.".into())
    } else {
        Ok(bytes)
    }
}

fn install_with(root: &Path, input: &str, mut fetch: impl FnMut(&str) -> Result<Vec<u8>, String>) -> Result<InstalledPlugin, String> {
    let manifest_url = validate_url(input)?;
    let bytes = check_size(fetch(&manifest_url)?)?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid native plugin manifest: {e}"))?;
    validate_manifest(&manifest)?;
    let plugins = root.join("plugins");
    let destination = bundle_directory(root, &manifest.id, &manifest.version);
    if destination.exists() {
        return Err(format!("{} {} is already installed.", manifest.name, manifest.version));
    }
    let base = manifest_url.strip_suffix("manifest.json").expect("validated manifest URL");
    let paths = if manifest.schema_version == 2 { manifest.files.clone() } else { vec![manifest.main.clone()] };
    let mut assets = BTreeMap::new();
    let mut total = bytes.len();
    for path in paths {
        let content = check_size(fetch(&format!("{base}{path}"))?)?;
        total += content.len();
        if total > MAX_TOTAL_BYTES {
            return Err("Plugin bundle exceeds 8 MiB.".into());
        }
        assets.insert(path, content);
    }
    let license = check_size(fetch(&format!("{base}LICENSE"))?)?;
    if total + license.len() > MAX_TOTAL_BYTES {
        return Err("Plugin bundle exceeds 8 MiB.".into());
    }
    if std::str::from_utf8(&license).map_or(true, |text| text.trim().is_empty()) {
        return Err("The plugin must include a nonempty UTF-8 LICENSE file beside its manifest.".into());
    }
    let installed = InstalledPlugin {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        license: manifest.license.clone(),
        source_url: input.trim().into(),
        manifest_url,
        main: manifest.main.clone(),
        permissions: manifest.permissions.clone(),
        runtime_supported: manifest.schema_version == 2,
        schema_version: manifest.schema_version,
        panel: manifest.panel.clone(),
        files: manifest.files.clone(),
        network: manifest.network.clone(),
        installed_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
    };
    fs::create_dir_all(&plugins).map_err(|e| format!("Cannot create plugin storage: {e}"))?;
    let temporary = plugins.join(format!(".install-{}-{}-{}", std::process::id(), installed.installed_at, NEXT_TEMP.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&temporary).map_err(|e| format!("Cannot create temporary plugin directory: {e}"))?;
    let result = (|| -> Result<(), String> {
        for (path, content) in &assets {
            let path = temporary.join(path);
            fs::create_dir_all(path.parent().expect("asset has parent")).map_err(|e| e.to_string())?;
            write_synced(&path, content)?;
        }
        write_synced(&temporary.join("LICENSE"), &license)?;
        write_synced(&temporary.join("manifest.json"), &bytes)?;
        write_synced(&temporary.join("installed.json"), &serde_json::to_vec_pretty(&installed).map_err(|e| e.to_string())?)?;
        // Destination bundles are always nonempty; rename cannot replace an existing install.
        if destination.exists() {
            return Err("This plugin version is already installed.".into());
        }
        fs::rename(&temporary, &destination).map_err(|e| format!("Cannot finish plugin installation: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result?;
    Ok(installed)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| format!("Cannot write plugin file: {e}"))?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| format!("Cannot save plugin file: {e}"))
}

pub fn list(root: &Path) -> Result<Vec<InstalledPlugin>, String> {
    let plugins = root.join("plugins");
    if !plugins.exists() {
        return Ok(Vec::new());
    }
    let mut installed = Vec::new();
    for entry in fs::read_dir(&plugins).map_err(|e| format!("Cannot read installed plugins: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let bytes = read_bundle_file(&entry.path(), "installed.json")?;
        let mut plugin: InstalledPlugin = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid installed plugin metadata: {e}"))?;
        // installed.json is unverified, URL text included: it never decides how a bundle is read and
        // its capability fields are never shown. Only a manifest that validates on disk sets them.
        plugin.runtime_supported = false;
        plugin.permissions = Vec::new();
        plugin.network = Vec::new();
        if entry.path() == bundle_directory(root, &plugin.id, &plugin.version) {
            if let Ok(bundle) = load_bundle(root, &plugin) {
                plugin = bundle.plugin;
            } else if let Ok(Some(figma)) = load_figma_manifest(root, &plugin) {
                // An imported Figma bundle has no native manifest and no runner yet, so it is never runnable.
                plugin.name = figma.name.clone();
                plugin.main = figma.main.clone();
                plugin.panel = figma.ui.as_ref().and_then(FigmaUi::primary_panel).map(str::to_string);
                plugin.network = figma.declared_network();
            }
        }
        installed.push(plugin);
    }
    installed.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    const URL: &str = "https://github.com/jangtrinh/design-os-plugins/blob/main/plugins/hello/manifest.json";
    fn manifest() -> Vec<u8> {
        br#"{"schema_version":1,"id":"hello","name":"Hello","version":"1.0.0","license":"MIT","main":"dist/main.js","permissions":[]}"#.to_vec()
    }
    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("designos-plugins-test-{}-{}", std::process::id(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed)))
    }
    #[test]
    fn only_native_repository_manifests_are_accepted() {
        let raw = validate_url(URL).unwrap();
        assert_eq!(raw, "https://raw.githubusercontent.com/jangtrinh/design-os-plugins/main/plugins/hello/manifest.json");
        assert_eq!(validate_url(&raw).unwrap(), raw);
        for bad in [
            "https://www.figma.com/community/plugin/735098390272716381/iconify",
            "https://github.com/other/designos-plugins/blob/main/plugins/hello/manifest.json",
            "https://raw.githubusercontent.com/jangtrinh/design-os-plugins/main/plugins/../manifest.json",
            "https://raw.githubusercontent.com/jangtrinh/design-os-plugins/main/plugins/%2e%2e/manifest.json",
            "https://user@github.com/jangtrinh/design-os-plugins/blob/main/plugins/hello/manifest.json",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn install_roundtrip_duplicate_and_inert_script() {
        let root = root();
        let plugin = install_with(&root, URL, |url| {
            Ok(if url.ends_with("manifest.json") {
                manifest()
            } else if url.ends_with("/LICENSE") {
                b"MIT license test notice".to_vec()
            } else {
                assert!(url.ends_with("/plugins/hello/dist/main.js"));
                b"throw new Error('never execute');".to_vec()
            })
        })
        .unwrap();
        assert!(!plugin.runtime_supported);
        assert_eq!(list(&root).unwrap()[0].id, "hello");
        assert!(install_with(&root, URL, |_| Ok(manifest())).unwrap_err().contains("already installed"));
        let entry = fs::read_dir(root.join("plugins")).unwrap().next().unwrap().unwrap();
        assert_eq!(fs::read(entry.path().join("dist/main.js")).unwrap(), b"throw new Error('never execute');");
        assert_eq!(fs::read(entry.path().join("LICENSE")).unwrap(), b"MIT license test notice");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failures_leave_no_install_and_reject_capabilities_and_traversal() {
        let root = root();
        assert!(install_with(&root, URL, |url| if url.ends_with("manifest.json") { Ok(manifest()) } else { Err("offline".into()) }).is_err());
        assert!(list(&root).unwrap().is_empty());
        for (key, value) in
            [("main", serde_json::json!("../escape.js")), ("permissions", serde_json::json!(["network"])), ("schema_version", serde_json::json!(2))]
        {
            let mut m: serde_json::Value = serde_json::from_slice(&manifest()).unwrap();
            m[key] = value;
            assert!(install_with(&root, URL, |_| Ok(serde_json::to_vec(&m).unwrap())).is_err());
        }
        assert!(check_size(vec![0; MAX_BYTES + 1]).is_err());
        assert!(!root.exists());
    }
    #[test]
    fn incomplete_storage_is_removed() {
        let root = root();
        let mut m: serde_json::Value = serde_json::from_slice(&manifest()).unwrap();
        // A directory/file collision forces failure after writing the downloaded script.
        m["main"] = serde_json::json!("manifest.json/main.js");
        assert!(install_with(&root, URL, |url| Ok(if url.ends_with("manifest.json") { serde_json::to_vec(&m).unwrap() } else { b"void 0;".to_vec() })).is_err());
        assert!(!root.join("plugins").exists());
        assert!(list(&root).unwrap().is_empty());
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
    }
    fn manifest_v2() -> serde_json::Value {
        serde_json::json!({"schema_version":2,"id":"icons","name":"Icons","version":"1.0.0","license":"MIT","main":"main.js","panel":"index.html","files":["index.html","main.js","style.css"],"permissions":["document:insert-svg","storage:local"],"network":["https://api.iconify.design"]})
    }
    fn install_v2(root: &Path) -> InstalledPlugin {
        install_with(root, URL, |url| Ok(if url.ends_with("manifest.json") { serde_json::to_vec(&manifest_v2()).unwrap() } else { b"test asset".to_vec() }))
            .unwrap()
    }
    #[test]
    fn browser_bundle_roundtrip_and_incomplete_assets() {
        let root = root();
        let plugin = install_v2(&root);
        assert!(plugin.runtime_supported);
        let bundle = load_bundle(&root, &plugin).unwrap();
        assert_eq!(bundle.panel, "index.html");
        assert_eq!(bundle.files.len(), 3);
        assert_eq!(bundle.network, ["https://api.iconify.design"]);
        assert!(list(&root).unwrap()[0].runtime_supported);
        let directory = bundle_directory(&root, &plugin.id, &plugin.version);
        fs::write(directory.join("undeclared.js"), b"never served").unwrap();
        assert!(!load_bundle(&root, &plugin).unwrap().files.contains_key("undeclared.js"));
        fs::remove_file(directory.join("style.css")).unwrap();
        assert!(load_bundle(&root, &plugin).is_err());
        assert!(!list(&root).unwrap()[0].runtime_supported);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn browser_manifest_rejects_capability_and_path_escapes() {
        for (key, value) in [
            ("permissions", serde_json::json!(["network"])),
            ("permissions", serde_json::json!(["storage:local", "storage:local"])),
            ("network", serde_json::json!(["https://api.iconify.design/evil"])),
            ("network", serde_json::json!(["https://other.example"])),
            ("files", serde_json::json!(["index.html", "main.js", "../escape"])),
            ("files", serde_json::json!(["index.html", "main.js", "MAIN.js"])),
            ("files", serde_json::json!(["index.html", "main.js", "manifest.json"])),
            ("files", serde_json::json!(["index.html", "main.js", "HOST.json"])),
            ("files", serde_json::json!(["index.html", "main.js", "insert-svg"])),
            ("files", serde_json::json!(["index.html", "main.js", "LICENSE/notice"])),
            ("files", serde_json::json!(["index.html", "main.js", "foo", "Foo/bar"])),
            ("files", serde_json::json!(["index.html", "main.js", "con.txt"])),
            ("files", serde_json::json!(["index.html", "main.js", "Foo/a", "foo/b"])),
            ("files", serde_json::json!(["index.html", "main.js", "assets./x"])),
            ("files", serde_json::json!(["main.js"])),
        ] {
            let mut value_manifest = manifest_v2();
            value_manifest[key] = value;
            let manifest: Manifest = serde_json::from_value(value_manifest).unwrap();
            assert!(validate_manifest(&manifest).is_err(), "{key}: {manifest:?}");
        }
    }
    #[test]
    fn browser_panel_must_be_at_bundle_root() {
        let mut value = manifest_v2();
        value["panel"] = serde_json::json!("ui/index.html");
        value["files"] = serde_json::json!(["ui/index.html", "main.js"]);
        let manifest: Manifest = serde_json::from_value(value).unwrap();
        assert!(validate_manifest(&manifest).is_err());
    }
    #[test]
    fn installed_metadata_cannot_grant_runtime_capabilities() {
        let root = root();
        let plugin = install_v2(&root);
        let directory = bundle_directory(&root, &plugin.id, &plugin.version);
        let mut fake = plugin.clone();
        fake.permissions = vec!["arbitrary".into()];
        fake.network = vec!["https://other.example".into()];
        fake.main = "undeclared.js".into();
        fs::write(directory.join("installed.json"), serde_json::to_vec(&fake).unwrap()).unwrap();
        let actual = list(&root).unwrap().remove(0);
        assert_eq!(actual.permissions, plugin.permissions);
        assert_eq!(actual.network, plugin.network);
        assert_eq!(actual.main, plugin.main);
        let mut manifest = manifest_v2();
        manifest["id"] = serde_json::json!("imposter");
        fs::write(directory.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(load_bundle(&root, &plugin).is_err());
        assert!(!list(&root).unwrap()[0].runtime_supported);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn schema_one_metadata_without_new_fields_stays_inert() {
        let root = root();
        let plugin = install_with(&root, URL, |url| Ok(if url.ends_with("manifest.json") { manifest() } else { b"test".to_vec() })).unwrap();
        let directory = bundle_directory(&root, &plugin.id, &plugin.version);
        let mut metadata = serde_json::to_value(&plugin).unwrap();
        for field in ["schema_version", "panel", "files", "network"] {
            metadata.as_object_mut().unwrap().remove(field);
        }
        metadata["runtime_supported"] = serde_json::json!(true);
        fs::write(directory.join("installed.json"), serde_json::to_vec(&metadata).unwrap()).unwrap();
        assert!(!list(&root).unwrap()[0].runtime_supported);
        assert_eq!(list(&root).unwrap()[0].schema_version, 1);
        assert!(load_bundle(&root, &plugin).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bundle_total_limit_applies_before_storage() {
        let root = root();
        let mut manifest = manifest_v2();
        manifest["files"] = serde_json::json!(["index.html", "main.js", "a", "b"]);
        let result =
            install_with(&root, URL, |url| Ok(if url.ends_with("manifest.json") { serde_json::to_vec(&manifest).unwrap() } else { vec![b'a'; MAX_BYTES] }));
        assert!(result.unwrap_err().contains("8 MiB"));
        assert!(!root.exists());
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_link_assets_are_not_loaded() {
        use std::os::unix::fs::symlink;
        let root = root();
        let plugin = install_v2(&root);
        let directory = bundle_directory(&root, &plugin.id, &plugin.version);
        fs::remove_file(directory.join("main.js")).unwrap();
        symlink(directory.join("style.css"), directory.join("main.js")).unwrap();
        assert!(load_bundle(&root, &plugin).unwrap_err().contains("symbolic links"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "Requires explicit DESIGNOS_PLUGINS_TOKEN and access to the private registry"]
    fn real_registry_install() {
        assert!(std::env::var("DESIGNOS_PLUGINS_TOKEN").is_ok(), "Set DESIGNOS_PLUGINS_TOKEN explicitly to run this test");
        let root = root();
        let url = "https://github.com/jangtrinh/design-os-plugins/blob/main/plugins/install-demo/manifest.json";
        let plugin = install(&root, url).expect("Install the published native sample");
        let reloaded = list(&root).unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].id, plugin.id);
        assert_eq!(reloaded[0].version, plugin.version);
        assert!(!reloaded[0].runtime_supported);
        assert!(install(&root, url).unwrap_err().contains("already installed"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn figma_manifest_parses_main_ui_editor_type_menu_parameters_and_network_access() {
        let raw = br#"{
            "name": "Design Token Sync",
            "id": "token-sync-local",
            "api": "1.0.0",
            "main": "dist/code.js",
            "ui": {
                "default": "ui/index.html",
                "settings": "ui/settings.html"
            },
            "editorType": ["figma", "dev"],
            "parameters": [
                {
                    "name": "Collection",
                    "key": "collection",
                    "description": "Target variable collection",
                    "allowFreeform": false,
                    "optional": false
                }
            ],
            "parameterOnly": false,
            "menu": [
                {
                    "name": "Sync Tokens",
                    "command": "sync-tokens",
                    "parameters": [
                        {
                            "name": "Mode",
                            "key": "mode",
                            "description": "Mode to export",
                            "allowFreeform": true,
                            "optional": true
                        }
                    ],
                    "parameterOnly": true
                },
                { "separator": true },
                {
                    "name": "Advanced",
                    "menu": [
                        {
                            "name": "Purge Unused",
                            "command": "purge-unused"
                        }
                    ]
                }
            ],
            "networkAccess": {
                "allowedDomains": ["https://api.example.com"],
                "reasoning": "Fetch token definitions",
                "devAllowedDomains": ["http://localhost:4100"]
            }
        }"#;
        let parsed = parse_figma_manifest(raw).expect("valid Figma manifest should parse");
        assert_eq!(parsed.name, "Design Token Sync");
        assert_eq!(parsed.id.as_deref(), Some("token-sync-local"));
        assert_eq!(parsed.api.as_deref(), Some("1.0.0"));
        assert_eq!(parsed.main, "dist/code.js");
        assert_eq!(parsed.ui.as_ref().and_then(FigmaUi::primary_panel), Some("ui/index.html"));
        assert_eq!(parsed.editor_type, vec!["figma".to_string(), "dev".to_string()]);
        assert_eq!(parsed.parameters.len(), 1);
        assert_eq!(parsed.parameters[0].key, "collection");
        let net = parsed.network_access.as_ref().expect("networkAccess parsed");
        assert_eq!(net.allowed_domains, vec!["https://api.example.com".to_string()]);
        assert_eq!(net.dev_allowed_domains, vec!["http://localhost:4100".to_string()]);
        assert_eq!(net.reasoning.as_deref(), Some("Fetch token definitions"));

        let entries = parsed.menu_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Sync Tokens");
        assert_eq!(entries[0].command, "sync-tokens");
        assert!(entries[0].parameter_only);
        assert_eq!(entries[0].parameters.len(), 1);
        assert_eq!(entries[0].parameters[0].key, "mode");
        assert_eq!(entries[1].label, "Advanced / Purge Unused");
        assert_eq!(entries[1].command, "purge-unused");
        assert_eq!(entries[1].submenu_path, vec!["Advanced".to_string()]);
    }

    #[test]
    fn figma_manifest_invalid_fields_are_named_in_error() {
        let base = serde_json::json!({
            "name": "Local Plugin",
            "id": "local-plugin",
            "api": "1.0.0",
            "main": "code.js",
            "ui": "ui.html",
            "editorType": ["figma"],
            "menu": [{"name": "Run", "command": "run"}],
            "parameters": [{"name": "Query", "key": "query"}],
            "networkAccess": {"allowedDomains": ["https://api.iconify.design"]}
        });

        for (field, bad_value) in [
            ("name", serde_json::json!("   ")),
            ("id", serde_json::json!("../bad-id")),
            ("api", serde_json::json!("")),
            ("main", serde_json::json!("code.ts")),
            ("main", serde_json::json!("../escape.js")),
            ("ui", serde_json::json!("panel.txt")),
            ("ui", serde_json::json!({"default": "../escape.html"})),
            ("editorType", serde_json::json!([])),
            ("editorType", serde_json::json!(["unknown_editor"])),
            ("menu", serde_json::json!([])),
            ("menu", serde_json::json!([{"name": "", "command": "run"}])),
            ("parameters", serde_json::json!([{"name": "Query", "key": "bad key!"}])),
            ("networkAccess", serde_json::json!({"allowedDomains": []})),
            ("networkAccess", serde_json::json!({"allowedDomains": ["none", "https://example.com"]})),
            ("unexpectedField", serde_json::json!(123)),
        ] {
            let mut candidate = base.clone();
            candidate[field] = bad_value;
            let bytes = serde_json::to_vec(&candidate).unwrap();
            let err = parse_figma_manifest(&bytes).expect_err(&format!("field {field} should fail"));
            assert!(err.contains(field), "error for field '{field}' did not name it: {err}");
        }
    }

    #[test]
    fn import_figma_manifest_from_local_folder_persists_bundle_and_exposes_menu_entries() {
        let root = root();
        let repo_dir = root.join("sample-plugin-repo");
        fs::create_dir_all(repo_dir.join("dist")).unwrap();
        fs::create_dir_all(repo_dir.join("ui")).unwrap();
        fs::write(repo_dir.join("dist/code.js"), b"figma.closePlugin();").unwrap();
        fs::write(repo_dir.join("ui/index.html"), b"<!doctype html><title>UI</title>").unwrap();
        let manifest_json = serde_json::json!({
            "name": "Repo Formatter",
            "id": "repo-formatter",
            "api": "1.0.0",
            "main": "dist/code.js",
            "ui": "ui/index.html",
            "editorType": ["figma", "figjam"],
            "menu": [
                {"name": "Format Selection", "command": "format-selection"},
                {"separator": true},
                {"name": "Lint", "menu": [{"name": "Check Contrast", "command": "check-contrast"}]}
            ],
            "parameters": [
                {"name": "Strict", "key": "strict", "optional": true}
            ],
            "networkAccess": {
                "allowedDomains": ["none"]
            }
        });
        fs::write(repo_dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest_json).unwrap()).unwrap();

        let installed = import_figma_manifest(&root, &repo_dir).expect("import from local repo folder");
        assert_eq!(installed.source_kind(), PluginSourceKind::LocalManifest);
        assert_eq!(installed.id, "repo-formatter");
        assert_eq!(installed.name, "Repo Formatter");
        assert_eq!(installed.main, "dist/code.js");
        assert_eq!(installed.panel.as_deref(), Some("ui/index.html"));

        let listed = list(&root).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].source_kind(), PluginSourceKind::LocalManifest);

        let entries = menu_entries(&root).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].plugin_id, "repo-formatter");
        assert_eq!(entries[0].label, "Format Selection");
        assert_eq!(entries[0].command, "format-selection");
        assert_eq!(entries[1].label, "Lint / Check Contrast");
        assert_eq!(entries[1].command, "check-contrast");

        fs::remove_dir_all(root).unwrap();
    }

    fn figma_manifest_value() -> serde_json::Value {
        serde_json::json!({
            "name": "Repo Formatter",
            "id": "repo-formatter",
            "api": "1.0.0",
            "main": "dist/code.js",
            "ui": "ui/index.html",
            "editorType": ["figma"],
            "menu": [{"name": "Format Selection", "command": "format-selection"}],
            "networkAccess": {"allowedDomains": ["https://api.example.com"]}
        })
    }

    fn write_figma_repo(parent: &Path, manifest: &serde_json::Value) -> PathBuf {
        let repo = parent.join("figma-repo");
        fs::create_dir_all(repo.join("dist")).unwrap();
        fs::create_dir_all(repo.join("ui")).unwrap();
        fs::write(repo.join("dist/code.js"), b"figma.closePlugin();").unwrap();
        fs::write(repo.join("ui/index.html"), b"<!doctype html><title>UI</title>").unwrap();
        fs::write(repo.join("manifest.json"), serde_json::to_vec_pretty(manifest).unwrap()).unwrap();
        repo
    }

    #[test]
    fn imported_figma_plugin_is_not_runnable_and_installed_metadata_cannot_grant_capabilities() {
        let root = root();
        let repo = write_figma_repo(&root, &figma_manifest_value());
        let installed = import_figma_manifest(&root, &repo).unwrap();
        assert!(!installed.runtime_supported, "the import result must not advertise a runner that does not exist");
        assert!(!list(&root).unwrap()[0].runtime_supported, "the library must not enable Open for a bundle load_bundle cannot open");
        assert!(load_bundle(&root, &installed).is_err());

        let directory = bundle_directory(&root, &installed.id, &installed.version);
        let mut fake = installed.clone();
        fake.permissions = vec!["document:insert-svg".into()];
        fake.network = vec!["https://other.example".into()];
        fake.runtime_supported = true;
        fs::write(directory.join("installed.json"), serde_json::to_vec(&fake).unwrap()).unwrap();
        let actual = list(&root).unwrap().remove(0);
        assert!(actual.permissions.is_empty(), "installed.json must not grant document permissions: {:?}", actual.permissions);
        assert_eq!(actual.network, ["https://api.example.com"]);
        assert!(!actual.runtime_supported);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn valid_native_bundle_is_never_reclassified_by_url_text_in_installed_metadata() {
        let root = root();
        let plugin = install_v2(&root);
        let directory = bundle_directory(&root, &plugin.id, &plugin.version);
        let mut fake = plugin.clone();
        fake.source_url = "file:///tmp/elsewhere".into();
        fake.manifest_url = "file:///tmp/elsewhere/manifest.json".into();
        fake.name = "Imposter".into();
        fake.main = "undeclared.js".into();
        fake.permissions = vec!["arbitrary".into()];
        fake.network = vec!["https://other.example".into()];
        fs::write(directory.join("installed.json"), serde_json::to_vec(&fake).unwrap()).unwrap();
        // A planted Figma manifest with a matching identity must not replace the validated native record.
        let mut planted = figma_manifest_value();
        planted["id"] = serde_json::json!(plugin.id);
        planted["api"] = serde_json::json!(plugin.version);
        planted["networkAccess"] = serde_json::json!({"allowedDomains": ["https://planted.example"]});
        fs::write(directory.join("figma-manifest.json"), serde_json::to_vec(&planted).unwrap()).unwrap();

        assert!(load_bundle(&root, &plugin).is_ok());
        let actual = list(&root).unwrap().remove(0);
        assert_eq!(actual.name, plugin.name);
        assert_eq!(actual.main, plugin.main);
        assert_eq!(actual.permissions, plugin.permissions);
        assert_eq!(actual.network, plugin.network);
        assert!(actual.runtime_supported);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unverified_installed_metadata_never_advertises_capabilities() {
        for tamper_url in [false, true] {
            let root = root();
            let plugin = install_v2(&root);
            let directory = bundle_directory(&root, &plugin.id, &plugin.version);
            fs::remove_file(directory.join("style.css")).unwrap();
            assert!(load_bundle(&root, &plugin).is_err());
            let mut fake = plugin.clone();
            fake.permissions = vec!["arbitrary".into()];
            fake.network = vec!["https://other.example".into()];
            if tamper_url {
                fake.source_url = "file:///tmp/elsewhere".into();
                fake.manifest_url = "file:///tmp/elsewhere/manifest.json".into();
            }
            fs::write(directory.join("installed.json"), serde_json::to_vec(&fake).unwrap()).unwrap();
            let actual = list(&root).unwrap().remove(0);
            assert!(!actual.runtime_supported, "tamper_url={tamper_url}");
            assert!(actual.permissions.is_empty(), "tamper_url={tamper_url}: {:?}", actual.permissions);
            assert!(actual.network.is_empty(), "tamper_url={tamper_url}: {:?}", actual.network);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn figma_editor_type_accepts_one_string_or_a_list_and_rejects_the_rest_with_a_next_step() {
        let single = br#"{"name":"Contrast","id":"contrast","api":"1.0.0","main":"code.js","editorType":"figma"}"#;
        assert_eq!(parse_figma_manifest(single).unwrap().editor_type, ["figma"]);
        let root = root();
        let repo = root.join("single-editor-type");
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("code.js"), b"figma.closePlugin();").unwrap();
        fs::write(repo.join("manifest.json"), single).unwrap();
        assert_eq!(import_figma_manifest(&root, &repo).unwrap().id, "contrast");
        fs::remove_dir_all(root).unwrap();

        let with = |value: serde_json::Value| {
            let mut manifest = figma_manifest_value();
            manifest["editorType"] = value;
            parse_figma_manifest(&serde_json::to_vec(&manifest).unwrap())
        };
        assert_eq!(with(serde_json::json!(["figma", "figjam"])).unwrap().editor_type, ["figma", "figjam"]);
        for bad in [
            serde_json::json!(""),
            serde_json::json!([]),
            serde_json::json!([""]),
            serde_json::json!(5),
            serde_json::json!([1]),
            serde_json::json!(null),
            serde_json::json!("unknown_editor"),
            serde_json::json!(["figma", "figma"]),
        ] {
            let err = with(bad.clone()).expect_err(&format!("{bad} must be rejected"));
            assert!(err.contains("editorType") && err.contains("Use "), "{bad}: {err}");
        }
    }

    #[test]
    fn figma_menu_entries_refuse_a_manifest_that_names_a_different_plugin() {
        for (field, value) in [("id", serde_json::json!("other-plugin")), ("api", serde_json::json!("9.9.9"))] {
            let root = root();
            let repo = write_figma_repo(&root, &figma_manifest_value());
            let installed = import_figma_manifest(&root, &repo).unwrap();
            assert_eq!(menu_entries(&root).unwrap().len(), 1);

            let directory = bundle_directory(&root, &installed.id, &installed.version);
            let mut swapped = figma_manifest_value();
            swapped[field] = value;
            swapped["name"] = serde_json::json!("Imposter");
            fs::write(directory.join("figma-manifest.json"), serde_json::to_vec(&swapped).unwrap()).unwrap();

            let err = menu_entries(&root).expect_err("a manifest for another plugin must not produce menu rows");
            assert!(err.contains("does not match") && err.contains("import"), "{field}: {err}");
            assert!(installed.menu_entries(&root).is_empty());
            let listed = list(&root).unwrap();
            assert_eq!(listed[0].name, "Repo Formatter", "{field}: the tampered name must not be applied");
            assert_eq!(listed[0].id, "repo-formatter");
            assert!(!listed[0].runtime_supported);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn figma_multi_panel_ui_prefers_default_then_main_then_a_stable_fallback() {
        let primary = |ui: serde_json::Value| {
            let mut manifest = figma_manifest_value();
            manifest["ui"] = ui;
            parse_figma_manifest(&serde_json::to_vec(&manifest).unwrap()).unwrap().ui.unwrap().primary_panel().map(str::to_string)
        };
        assert_eq!(primary(serde_json::json!({"about": "ui/about.html", "main": "ui/main.html"})).as_deref(), Some("ui/main.html"));
        assert_eq!(
            primary(serde_json::json!({"about": "ui/about.html", "main": "ui/main.html", "default": "ui/start.html"})).as_deref(),
            Some("ui/start.html")
        );
        assert_eq!(primary(serde_json::json!({"zeta": "ui/z.html", "alpha": "ui/a.html"})).as_deref(), Some("ui/a.html"));
    }

    #[test]
    fn figma_import_requires_an_identity_the_bundle_store_can_hold() {
        let with = |id: Option<String>, api: &str| {
            let mut m = figma_manifest_value();
            match id {
                Some(id) => m["id"] = serde_json::json!(id),
                None => {
                    m.as_object_mut().unwrap().remove("id");
                }
            }
            m["api"] = serde_json::json!(api);
            m
        };
        // A missing id is never guessed from the name; an id/api pair longer than the native limit does not fit the directory name.
        let cases = [("id", with(None, "1.0.0")), ("id", with(Some("i".repeat(100)), &"1".repeat(100))), ("id", with(Some("i".repeat(60)), &"1".repeat(61)))];
        for (field, manifest) in cases {
            let root = root();
            let repo = write_figma_repo(&root, &manifest);
            let err = import_figma_manifest(&root, &repo).expect_err("identity must be rejected before anything is stored");
            assert!(err.contains(&format!("'{field}'")) && err.contains("import again"), "{err}");
            assert!(visible_plugin_entries(&root).is_empty());
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn visible_plugin_entries(root: &Path) -> Vec<String> {
        match fs::read_dir(root.join("plugins")) {
            Ok(entries) => entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect(),
            Err(_) => Vec::new(),
        }
    }

    #[test]
    fn figma_import_rejects_hostile_inputs_with_a_next_step_and_leaves_the_store_untouched() {
        let root = root();
        let sentinel = root.join("outside-sentinel.js");
        fs::create_dir_all(&root).unwrap();
        fs::write(&sentinel, b"sentinel-secret").unwrap();
        let mut cases: Vec<(&str, PathBuf)> = Vec::new();
        let fresh = |name: &str, manifest: &serde_json::Value| {
            let parent = root.join(name);
            fs::create_dir_all(&parent).unwrap();
            write_figma_repo(&parent, manifest)
        };

        let escape = {
            let mut m = figma_manifest_value();
            m["main"] = serde_json::json!("../outside-sentinel.js");
            fresh("traversal-main", &m)
        };
        cases.push(("traversal in main", escape));
        let escape_ui = {
            let mut m = figma_manifest_value();
            m["ui"] = serde_json::json!({"default": "../../outside.html"});
            fresh("traversal-ui", &m)
        };
        cases.push(("traversal in ui", escape_ui));

        let oversized = fresh("oversized", &figma_manifest_value());
        let mut padded = serde_json::to_vec(&figma_manifest_value()).unwrap();
        padded.extend(std::iter::repeat_n(b' ', MAX_BYTES));
        fs::write(oversized.join("manifest.json"), padded).unwrap();
        cases.push(("oversized manifest", oversized));

        let bad_manifest = fresh("non-utf8-manifest", &figma_manifest_value());
        fs::write(bad_manifest.join("manifest.json"), b"{\"name\":\"\xff\xfe\",\"main\":\"dist/code.js\"}").unwrap();
        cases.push(("non-UTF-8 manifest", bad_manifest));

        let bad_main = fresh("non-utf8-main", &figma_manifest_value());
        fs::write(bad_main.join("dist/code.js"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
        cases.push(("non-UTF-8 main", bad_main));

        let bad_ui = fresh("non-utf8-ui", &figma_manifest_value());
        fs::write(bad_ui.join("ui/index.html"), [0xc3, 0x28]).unwrap();
        cases.push(("non-UTF-8 ui", bad_ui));

        let missing = fresh("missing-main", &figma_manifest_value());
        fs::remove_file(missing.join("dist/code.js")).unwrap();
        cases.push(("missing main", missing));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let linked_file = fresh("symlinked-main", &figma_manifest_value());
            fs::remove_file(linked_file.join("dist/code.js")).unwrap();
            symlink(&sentinel, linked_file.join("dist/code.js")).unwrap();
            cases.push(("symlinked main", linked_file));

            let linked_dir = fresh("symlinked-dir", &figma_manifest_value());
            fs::remove_dir_all(linked_dir.join("dist")).unwrap();
            let target = root.join("elsewhere");
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("code.js"), b"sentinel-secret").unwrap();
            symlink(&target, linked_dir.join("dist")).unwrap();
            cases.push(("symlinked directory", linked_dir));

            let linked_manifest = fresh("symlinked-manifest", &figma_manifest_value());
            fs::remove_file(linked_manifest.join("manifest.json")).unwrap();
            symlink(root.join("figma-repo-source.json"), linked_manifest.join("manifest.json")).unwrap();
            fs::write(root.join("figma-repo-source.json"), serde_json::to_vec(&figma_manifest_value()).unwrap()).unwrap();
            cases.push(("symlinked manifest", linked_manifest.clone()));
            cases.push(("symlinked manifest given as a file", linked_manifest.join("manifest.json")));
        }

        for (label, path) in &cases {
            let before: Vec<_> = fs::read_dir(path).map(|d| d.map(|e| e.unwrap().file_name()).collect()).unwrap_or_default();
            let err = std::panic::catch_unwind(|| import_figma_manifest(&root, path)).unwrap_or_else(|_| panic!("{label}: import panicked")).expect_err(label);
            assert!(err.contains("import again"), "{label}: the error must name the next step: {err}");
            assert!(visible_plugin_entries(&root).is_empty(), "{label}: nothing may be stored: {:?}", visible_plugin_entries(&root));
            let after: Vec<_> = fs::read_dir(path).map(|d| d.map(|e| e.unwrap().file_name()).collect()).unwrap_or_default();
            assert_eq!(before, after, "{label}: the source folder must not change");
        }
        assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel-secret");
        fs::remove_dir_all(root).unwrap();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
pub enum PluginSourceKind {
    #[default]
    NativeRegistry,
    LocalManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FigmaUi {
    Single(String),
    Multiple(BTreeMap<String, String>),
}

impl FigmaUi {
    pub fn primary_panel(&self) -> Option<&str> {
        match self {
            FigmaUi::Single(path) => Some(path.as_str()),
            // The map is key-sorted, so source order is unavailable: prefer `default`, then `main`,
            // then the alphabetically first entry.
            FigmaUi::Multiple(map) => map.get("default").or_else(|| map.get("main")).or_else(|| map.values().next()).map(String::as_str),
        }
    }

    #[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
    pub fn all_paths(&self) -> Vec<&str> {
        match self {
            FigmaUi::Single(path) => vec![path.as_str()],
            FigmaUi::Multiple(map) => map.values().map(String::as_str).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigmaParameter {
    pub name: String,
    pub key: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "allowFreeform", default)]
    pub allow_freeform: bool,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FigmaMenuItem {
    Separator,
    Submenu {
        name: String,
        menu: Vec<FigmaMenuItem>,
    },
    Command {
        name: String,
        command: String,
        #[serde(default)]
        parameters: Vec<FigmaParameter>,
        #[serde(rename = "parameterOnly", default)]
        parameter_only: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigmaNetworkAccess {
    #[serde(rename = "allowedDomains")]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(rename = "devAllowedDomains", default)]
    pub dev_allowed_domains: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigmaManifest {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    pub main: String,
    #[serde(default)]
    pub ui: Option<FigmaUi>,
    #[serde(rename = "editorType", default)]
    pub editor_type: Vec<String>,
    #[serde(default)]
    pub menu: Vec<FigmaMenuItem>,
    #[serde(default)]
    pub parameters: Vec<FigmaParameter>,
    #[serde(rename = "parameterOnly", default)]
    pub parameter_only: bool,
    #[serde(rename = "networkAccess", default)]
    pub network_access: Option<FigmaNetworkAccess>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
pub struct PluginMenuEntry {
    pub plugin_id: String,
    pub plugin_name: String,
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub submenu_path: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<FigmaParameter>,
    #[serde(default)]
    pub parameter_only: bool,
}

impl FigmaManifest {
    pub fn resolved_id(&self) -> String {
        if let Some(id) = &self.id {
            if safe_part(id) {
                return id.clone();
            }
        }
        slugify_plugin_id(&self.name)
    }

    pub fn version(&self) -> &str {
        self.api.as_deref().unwrap_or(DEFAULT_FIGMA_VERSION)
    }

    /// Network origins the manifest asks for. `none` means no access.
    pub fn declared_network(&self) -> Vec<String> {
        self.network_access.as_ref().map(|n| n.allowed_domains.iter().filter(|d| d.as_str() != "none").cloned().collect()).unwrap_or_default()
    }

    #[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
    pub fn menu_entries(&self) -> Vec<PluginMenuEntry> {
        let plugin_id = self.resolved_id();
        let mut out = Vec::new();
        if self.menu.is_empty() {
            out.push(PluginMenuEntry {
                plugin_id,
                plugin_name: self.name.clone(),
                label: self.name.clone(),
                command: "default".into(),
                submenu_path: Vec::new(),
                parameters: self.parameters.clone(),
                parameter_only: self.parameter_only,
            });
            return out;
        }
        collect_menu_entries(&plugin_id, &self.name, &self.menu, &mut Vec::new(), &mut out);
        out
    }
}

#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
fn collect_menu_entries(plugin_id: &str, plugin_name: &str, items: &[FigmaMenuItem], path: &mut Vec<String>, out: &mut Vec<PluginMenuEntry>) {
    for item in items {
        match item {
            FigmaMenuItem::Separator => {}
            FigmaMenuItem::Command { name, command, parameters, parameter_only } => {
                let label = if path.is_empty() { name.clone() } else { format!("{} / {name}", path.join(" / ")) };
                out.push(PluginMenuEntry {
                    plugin_id: plugin_id.to_string(),
                    plugin_name: plugin_name.to_string(),
                    label,
                    command: command.clone(),
                    submenu_path: path.clone(),
                    parameters: parameters.clone(),
                    parameter_only: *parameter_only,
                });
            }
            FigmaMenuItem::Submenu { name, menu } => {
                path.push(name.clone());
                collect_menu_entries(plugin_id, plugin_name, menu, path, out);
                path.pop();
            }
        }
    }
}

fn slugify_plugin_id(name: &str) -> String {
    let mut slug = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() {
            slug.push(b.to_ascii_lowercase() as char);
        } else if (b == b' ' || b == b'-' || b == b'_') && !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 64 {
            break;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "local-plugin".into()
    } else {
        trimmed.to_string()
    }
}

impl InstalledPlugin {
    #[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
    pub fn source_kind(&self) -> PluginSourceKind {
        if self.manifest_url.starts_with("file://") || self.source_url.starts_with("file://") {
            PluginSourceKind::LocalManifest
        } else {
            PluginSourceKind::NativeRegistry
        }
    }

    #[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
    pub fn menu_entries(&self, root: &Path) -> Vec<PluginMenuEntry> {
        if let Ok(Some(figma)) = load_figma_manifest(root, self) {
            figma.menu_entries()
        } else {
            Vec::new()
        }
    }
}

const KNOWN_FIGMA_MANIFEST_FIELDS: &[&str] = &[
    "name",
    "id",
    "api",
    "main",
    "ui",
    "editorType",
    "menu",
    "parameters",
    "parameterOnly",
    "networkAccess",
    "permissions",
    "capabilities",
    "documentAccess",
    "enableProposedApi",
    "enablePrivatePluginApi",
    "build",
    "relaunchButtons",
    "codegenLanguages",
    "codegenPreferences",
    "widgetApi",
    "containsWidget",
];

const ALLOWED_EDITOR_TYPES: &[&str] = &["figma", "figjam", "dev", "slides", "buzz"];

fn parse_parameters(field_path: &str, value: &serde_json::Value) -> Result<Vec<FigmaParameter>, String> {
    let arr = value.as_array().ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': expected an array of parameter objects."))?;
    let mut seen_keys = BTreeSet::new();
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item.as_object().ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': each parameter must be an object."))?;
        for k in obj.keys() {
            if !matches!(k.as_str(), "name" | "key" | "description" | "allowFreeform" | "optional") {
                return Err(format!("Invalid plugin manifest field '{field_path}.{k}': unknown parameter property."));
            }
        }
        let name = obj
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 200)
            .ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': parameter 'name' must be a non-empty string."))?
            .to_string();
        let key = obj
            .get("key")
            .and_then(serde_json::Value::as_str)
            .filter(|k| safe_part(k))
            .ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': parameter 'key' must contain only safe identifier characters."))?
            .to_string();
        if !seen_keys.insert(key.clone()) {
            return Err(format!("Invalid plugin manifest field '{field_path}': duplicate parameter key '{key}'."));
        }
        let description = match obj.get("description") {
            Some(v) => Some(v.as_str().ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': 'description' must be a string."))?.to_string()),
            None => None,
        };
        let allow_freeform = match obj.get("allowFreeform") {
            Some(v) => v.as_bool().ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': 'allowFreeform' must be a boolean."))?,
            None => false,
        };
        let optional = match obj.get("optional") {
            Some(v) => v.as_bool().ok_or_else(|| format!("Invalid plugin manifest field '{field_path}': 'optional' must be a boolean."))?,
            None => false,
        };
        out.push(FigmaParameter { name, key, description, allow_freeform, optional });
    }
    Ok(out)
}

fn parse_menu_items(value: &serde_json::Value) -> Result<Vec<FigmaMenuItem>, String> {
    let arr = value.as_array().ok_or_else(|| "Invalid plugin manifest field 'menu': expected a non-empty array.".to_string())?;
    if arr.is_empty() {
        return Err("Invalid plugin manifest field 'menu': menu array cannot be empty.".into());
    }
    let mut items = Vec::with_capacity(arr.len());
    for entry in arr {
        let obj = entry.as_object().ok_or_else(|| "Invalid plugin manifest field 'menu': each menu entry must be an object.".to_string())?;
        if let Some(sep) = obj.get("separator") {
            if obj.len() != 1 || sep.as_bool() != Some(true) {
                return Err("Invalid plugin manifest field 'menu': separator entry must be {\"separator\": true}.".into());
            }
            items.push(FigmaMenuItem::Separator);
            continue;
        }
        let name = obj
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 200)
            .ok_or_else(|| "Invalid plugin manifest field 'menu': menu item 'name' must be a non-empty string.".to_string())?
            .to_string();
        if let Some(sub) = obj.get("menu") {
            for k in obj.keys() {
                if !matches!(k.as_str(), "name" | "menu") {
                    return Err(format!("Invalid plugin manifest field 'menu.{k}': submenu item only allows 'name' and 'menu'."));
                }
            }
            let children = parse_menu_items(sub)?;
            items.push(FigmaMenuItem::Submenu { name, menu: children });
        } else {
            for k in obj.keys() {
                if !matches!(k.as_str(), "name" | "command" | "parameters" | "parameterOnly") {
                    return Err(format!("Invalid plugin manifest field 'menu.{k}': unknown menu command property."));
                }
            }
            let command = obj
                .get("command")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty() && safe_part(s))
                .ok_or_else(|| "Invalid plugin manifest field 'menu': command entry requires a safe non-empty 'command' string.".to_string())?
                .to_string();
            let parameters = match obj.get("parameters") {
                Some(v) => parse_parameters("menu.parameters", v)?,
                None => Vec::new(),
            };
            let parameter_only = match obj.get("parameterOnly") {
                Some(v) => v.as_bool().ok_or_else(|| "Invalid plugin manifest field 'menu.parameterOnly': expected a boolean.".to_string())?,
                None => false,
            };
            items.push(FigmaMenuItem::Command { name, command, parameters, parameter_only });
        }
    }
    Ok(items)
}

fn validate_domain_list(field_name: &str, list: &[String], allow_empty: bool) -> Result<(), String> {
    if !allow_empty && list.is_empty() {
        return Err(format!("Invalid plugin manifest field '{field_name}': domain list cannot be empty."));
    }
    if list.iter().any(|d| d == "none") && list.len() > 1 {
        return Err(format!("Invalid plugin manifest field '{field_name}': 'none' cannot be combined with other domains."));
    }
    for domain in list {
        let trimmed = domain.trim();
        if trimmed.is_empty() || trimmed != domain || domain.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!("Invalid plugin manifest field '{field_name}': invalid domain '{domain}'."));
        }
    }
    Ok(())
}

/// Figma accepts one editor type or a list of them; both normalise to a list.
fn parse_editor_types(value: &serde_json::Value) -> Result<Vec<String>, String> {
    const HELP: &str = "Use \"figma\", \"figjam\", \"dev\", \"slides\" or \"buzz\", or a non-empty list of them.";
    let names: Vec<&serde_json::Value> = match value {
        serde_json::Value::Array(items) => items.iter().collect(),
        single => vec![single],
    };
    if names.is_empty() {
        return Err(format!("Invalid plugin manifest field 'editorType': it lists no editor type. {HELP}"));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(names.len());
    for item in names {
        let s = item.as_str().ok_or_else(|| format!("Invalid plugin manifest field 'editorType': every editor type must be a string. {HELP}"))?;
        if !ALLOWED_EDITOR_TYPES.contains(&s) || !seen.insert(s) {
            return Err(format!("Invalid plugin manifest field 'editorType': unsupported or duplicate editor type '{s}'. {HELP}"));
        }
        out.push(s.to_string());
    }
    Ok(out)
}

pub fn parse_figma_manifest(bytes: &[u8]) -> Result<FigmaManifest, String> {
    let root_val: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| format!("Invalid plugin manifest JSON: {e}"))?;
    let obj = root_val.as_object().ok_or_else(|| "Invalid plugin manifest: root value must be a JSON object.".to_string())?;

    for key in obj.keys() {
        if !KNOWN_FIGMA_MANIFEST_FIELDS.contains(&key.as_str()) {
            return Err(format!("Invalid plugin manifest field '{key}': unknown field."));
        }
    }

    let name = obj
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 200 && !s.chars().any(char::is_control))
        .ok_or_else(|| "Invalid plugin manifest field 'name': required non-empty string (at most 200 characters).".to_string())?
        .to_string();

    let id = match obj.get("id") {
        Some(v) => {
            let s = v.as_str().ok_or_else(|| "Invalid plugin manifest field 'id': must be a string.".to_string())?;
            if !safe_part(s) {
                return Err("Invalid plugin manifest field 'id': must contain only letters, digits, dots, dashes, or underscores.".into());
            }
            Some(s.to_string())
        }
        None => None,
    };

    let api = match obj.get("api") {
        Some(v) => {
            let s = v.as_str().ok_or_else(|| "Invalid plugin manifest field 'api': must be a string.".to_string())?;
            if !safe_part(s) {
                return Err("Invalid plugin manifest field 'api': must be a valid version string.".into());
            }
            Some(s.to_string())
        }
        None => None,
    };

    let main = obj
        .get("main")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Invalid plugin manifest field 'main': required relative .js path.".to_string())?
        .to_string();
    validate_asset_path(&main).map_err(|e| format!("Invalid plugin manifest field 'main': {e}"))?;
    if !main.ends_with(".js") {
        return Err("Invalid plugin manifest field 'main': must end with .js.".into());
    }

    let ui = match obj.get("ui") {
        Some(serde_json::Value::String(path)) => {
            validate_asset_path(path).map_err(|e| format!("Invalid plugin manifest field 'ui': {e}"))?;
            if !path.ends_with(".html") {
                return Err("Invalid plugin manifest field 'ui': UI file must end with .html.".into());
            }
            Some(FigmaUi::Single(path.clone()))
        }
        Some(serde_json::Value::Object(map)) => {
            if map.is_empty() {
                return Err("Invalid plugin manifest field 'ui': UI map cannot be empty.".into());
            }
            let mut ui_map = BTreeMap::new();
            for (k, v) in map {
                if !safe_part(k) {
                    return Err(format!("Invalid plugin manifest field 'ui': invalid UI key '{k}'."));
                }
                let path = v.as_str().ok_or_else(|| format!("Invalid plugin manifest field 'ui': UI entry '{k}' must be a string path."))?;
                validate_asset_path(path).map_err(|e| format!("Invalid plugin manifest field 'ui': {e}"))?;
                if !path.ends_with(".html") {
                    return Err(format!("Invalid plugin manifest field 'ui': UI entry '{k}' must end with .html."));
                }
                ui_map.insert(k.clone(), path.to_string());
            }
            Some(FigmaUi::Multiple(ui_map))
        }
        Some(_) => return Err("Invalid plugin manifest field 'ui': must be a string path or an object map of HTML paths.".into()),
        None => None,
    };

    let editor_type = match obj.get("editorType") {
        Some(v) => parse_editor_types(v)?,
        None => vec!["figma".to_string()],
    };

    let menu = match obj.get("menu") {
        Some(v) => parse_menu_items(v)?,
        None => Vec::new(),
    };

    let parameters = match obj.get("parameters") {
        Some(v) => parse_parameters("parameters", v)?,
        None => Vec::new(),
    };

    let parameter_only = match obj.get("parameterOnly") {
        Some(v) => v.as_bool().ok_or_else(|| "Invalid plugin manifest field 'parameterOnly': must be a boolean.".to_string())?,
        None => false,
    };

    let network_access = match obj.get("networkAccess") {
        Some(v) => {
            let net_obj = v.as_object().ok_or_else(|| "Invalid plugin manifest field 'networkAccess': must be an object.".to_string())?;
            for k in net_obj.keys() {
                if !matches!(k.as_str(), "allowedDomains" | "reasoning" | "devAllowedDomains") {
                    return Err(format!("Invalid plugin manifest field 'networkAccess.{k}': unknown property."));
                }
            }
            let allowed_val = net_obj
                .get("allowedDomains")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "Invalid plugin manifest field 'networkAccess': 'allowedDomains' must be a non-empty array of strings.".to_string())?;
            let mut allowed_domains = Vec::with_capacity(allowed_val.len());
            for item in allowed_val {
                let s = item.as_str().ok_or_else(|| "Invalid plugin manifest field 'networkAccess': each allowedDomain must be a string.".to_string())?;
                allowed_domains.push(s.to_string());
            }
            validate_domain_list("networkAccess", &allowed_domains, false)?;

            let dev_allowed_domains = match net_obj.get("devAllowedDomains") {
                Some(dev_val) => {
                    let arr = dev_val
                        .as_array()
                        .ok_or_else(|| "Invalid plugin manifest field 'networkAccess': 'devAllowedDomains' must be an array of strings.".to_string())?;
                    let mut list = Vec::with_capacity(arr.len());
                    for item in arr {
                        let s = item
                            .as_str()
                            .ok_or_else(|| "Invalid plugin manifest field 'networkAccess': each devAllowedDomain must be a string.".to_string())?;
                        list.push(s.to_string());
                    }
                    validate_domain_list("networkAccess", &list, true)?;
                    list
                }
                None => Vec::new(),
            };

            let reasoning = match net_obj.get("reasoning") {
                Some(r) => {
                    Some(r.as_str().ok_or_else(|| "Invalid plugin manifest field 'networkAccess': 'reasoning' must be a string.".to_string())?.to_string())
                }
                None => None,
            };

            Some(FigmaNetworkAccess { allowed_domains, reasoning, dev_allowed_domains })
        }
        None => None,
    };

    Ok(FigmaManifest { name, id, api, main, ui, editor_type, menu, parameters, parameter_only, network_access })
}

pub fn load_figma_manifest(root: &Path, plugin: &InstalledPlugin) -> Result<Option<FigmaManifest>, String> {
    let directory = bundle_directory(root, &plugin.id, &plugin.version);
    if !directory.join("figma-manifest.json").exists() {
        return Ok(None);
    }
    let bytes = read_bundle_file(&directory, "figma-manifest.json")?;
    let parsed = parse_figma_manifest(&bytes)?;
    if parsed.resolved_id() != plugin.id || parsed.version() != plugin.version {
        return Err(format!(
            "The stored Figma manifest for {} does not match the installed plugin. Delete this plugin from your plugins folder, then import it again.",
            plugin.id
        ));
    }
    Ok(Some(parsed))
}

#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
pub fn import_figma_manifest(root: &Path, manifest_or_dir: &Path) -> Result<InstalledPlugin, String> {
    let meta = fs::symlink_metadata(manifest_or_dir).map_err(|e| format!("Cannot locate plugin manifest path: {e} Check the path, then import again."))?;
    if meta.file_type().is_symlink() {
        return Err("Plugin manifest path cannot be a symbolic link. Choose the real folder or file, then import again.".into());
    }
    let (plugin_dir, manifest_rel) = if meta.is_dir() {
        (manifest_or_dir.to_path_buf(), "manifest.json".to_string())
    } else {
        let file_name = manifest_or_dir.file_name().and_then(|n| n.to_str()).ok_or_else(|| "Invalid plugin manifest filename.".to_string())?;
        let parent = manifest_or_dir.parent().ok_or_else(|| "Cannot determine plugin directory.".to_string())?;
        (parent.to_path_buf(), file_name.to_string())
    };

    let manifest_bytes =
        read_bundle_file(&plugin_dir, &manifest_rel).map_err(|e| format!("Cannot read the plugin manifest {manifest_rel}: {e} {IMPORT_RETRY}"))?;
    let parsed = parse_figma_manifest(&manifest_bytes).map_err(|e| format!("{e} {IMPORT_RETRY}"))?;
    // The bundle is stored under its id, so the import needs an explicit one rather than a name-derived guess.
    let id = parsed
        .id
        .clone()
        .ok_or_else(|| format!("Invalid plugin manifest field 'id': add an id of letters, digits, dots, dashes or underscores. {IMPORT_RETRY}"))?;
    let version = parsed.version().to_string();
    if id.len() + version.len() > 100 {
        return Err(format!("Invalid plugin manifest fields 'id' and 'api': together they must be at most 100 characters. {IMPORT_RETRY}"));
    }

    let destination = bundle_directory(root, &id, &version);
    if destination.exists() {
        return Err(format!("{} {} is already installed.", parsed.name, version));
    }

    let mut asset_paths = vec![parsed.main.clone()];
    if let Some(ui) = &parsed.ui {
        for p in ui.all_paths() {
            if !asset_paths.iter().any(|existing| existing == p) {
                asset_paths.push(p.to_string());
            }
        }
    }

    let mut assets = BTreeMap::new();
    let mut total = manifest_bytes.len();
    for rel in &asset_paths {
        let field = if rel == &parsed.main { "main" } else { "ui" };
        let content = read_bundle_file(&plugin_dir, rel).map_err(|e| format!("Invalid plugin manifest field '{field}': {e} {IMPORT_RETRY}"))?;
        if std::str::from_utf8(&content).is_err() {
            return Err(format!("Invalid plugin manifest field '{field}': {rel} is not valid UTF-8 text. Re-save it as UTF-8, then import again."));
        }
        total += content.len();
        if total > MAX_TOTAL_BYTES {
            return Err("Plugin bundle exceeds 8 MiB. Remove large files from the plugin folder, then import again.".into());
        }
        assets.insert(rel.clone(), content);
    }

    let canonical_dir = fs::canonicalize(&plugin_dir).map_err(|e| format!("Cannot canonicalize plugin folder: {e}"))?;
    let manifest_uri = format!("file://{}", canonical_dir.join(&manifest_rel).display());
    let network = parsed.declared_network();

    let installed = InstalledPlugin {
        id: id.clone(),
        name: parsed.name.clone(),
        version: version.clone(),
        license: "LOCAL".into(),
        source_url: manifest_uri.clone(),
        manifest_url: manifest_uri,
        main: parsed.main.clone(),
        permissions: Vec::new(),
        // No runner executes a Figma bundle yet; it has no native manifest or LICENSE for load_bundle.
        runtime_supported: false,
        installed_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        schema_version: 2,
        panel: parsed.ui.as_ref().and_then(FigmaUi::primary_panel).map(str::to_string),
        files: asset_paths,
        network,
    };

    let plugins = root.join("plugins");
    fs::create_dir_all(&plugins).map_err(|e| format!("Cannot create plugin storage: {e}"))?;
    let temporary = plugins.join(format!(".install-{}-{}-{}", std::process::id(), installed.installed_at, NEXT_TEMP.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&temporary).map_err(|e| format!("Cannot create temporary plugin directory: {e}"))?;
    let result = (|| -> Result<(), String> {
        for (path, content) in &assets {
            let target = temporary.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_synced(&target, content)?;
        }
        write_synced(&temporary.join("figma-manifest.json"), &manifest_bytes)?;
        write_synced(&temporary.join("installed.json"), &serde_json::to_vec_pretty(&installed).map_err(|e| e.to_string())?)?;
        if destination.exists() {
            return Err("This plugin version is already installed.".into());
        }
        fs::rename(&temporary, &destination).map_err(|e| format!("Cannot finish plugin installation: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result?;
    Ok(installed)
}

#[allow(dead_code)] // Figma manifest import and plugin menu entries: wired into the Plugins menu by #482.
pub fn menu_entries(root: &Path) -> Result<Vec<PluginMenuEntry>, String> {
    let plugins = list(root)?;
    let mut entries = Vec::new();
    for plugin in &plugins {
        if let Some(figma) = load_figma_manifest(root, plugin)? {
            entries.extend(figma.menu_entries());
        }
    }
    Ok(entries)
}
