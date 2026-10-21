//! Embeds the app icon and version info into `designos.exe` (Windows targets only).
//! On every other target this does nothing, so Linux and macOS builds are unaffected.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon/designos.ico");
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
