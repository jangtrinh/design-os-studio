//! Embeds the app icon and version info into `designos.exe` (Windows targets only).
//! On every other target this does nothing, so Linux and macOS builds are unaffected.

fn validate_bundled_skills() {
    let skills_root = std::path::Path::new("../../assets/skills");
    println!("cargo:rerun-if-changed={}", skills_root.display());
    let Ok(entries) = std::fs::read_dir(skills_root) else {
        return;
    };
    for entry in entries.flatten() {
        let skill_file = entry.path().join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", skill_file.display());
        let raw = std::fs::read_to_string(&skill_file).unwrap_or_else(|e| panic!("cannot read bundled skill {}: {e}", skill_file.display()));
        let trimmed = raw.trim_start_matches('\u{feff}');
        let Some(after_open) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n")) else {
            panic!("bundled skill {} is missing YAML front matter (`---`)", skill_file.display());
        };
        let Some(end_idx) = after_open.find("\n---") else {
            panic!("bundled skill {} has unclosed YAML front matter (`---`)", skill_file.display());
        };
        let fm = &after_open[..end_idx];
        for required in ["name", "description", "version", "license"] {
            let found = fm.lines().any(|line| {
                let Some((k, v)) = line.split_once(':') else { return false };
                k.trim() == required && !v.trim().trim_matches('"').trim_matches('\'').is_empty()
            });
            if !found {
                panic!("bundled skill {} is missing required `{required}` in front matter", skill_file.display());
            }
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon/designos.ico");
    validate_bundled_skills();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon/designos.ico");
    res.set("ProductName", "designos");
    res.set("FileDescription", "designos");
    res.set("CompanyName", "designos contributors");
    res.set("LegalCopyright", "MIT OR Apache-2.0");
    res.set("OriginalFilename", "designos.exe");
    res.set("ProductVersion", &version);
    res.set("FileVersion", &version);
    if let Err(e) = res.compile() {
        // CI (and releases) must never ship an icon-less exe; local builds without a Windows SDK
        // just get a warning.
        if std::env::var_os("CI").is_some() {
            panic!("could not embed the Windows icon and version info: {e}");
        }
        println!("cargo:warning=could not embed the Windows icon and version info: {e}");
    }
}
