use crate::FontRequest;

/// A font embedded in the binary.
#[derive(Debug)]
pub struct FontAsset {
    pub family: &'static str,
    pub italic: bool,
    /// Weight range covered (the file is a variable font when `min != max`).
    pub min_weight: u16,
    pub max_weight: u16,
    pub license: &'static str,
    pub data: &'static [u8],
}

static BUNDLED: [FontAsset; 3] = [
    FontAsset {
        family: "Inter",
        italic: false,
        min_weight: 100,
        max_weight: 900,
        license: "OFL-1.1",
        data: include_bytes!("../assets/inter/Inter[opsz,wght].ttf"),
    },
    FontAsset {
        family: "Inter",
        italic: true,
        min_weight: 100,
        max_weight: 900,
        license: "OFL-1.1",
        data: include_bytes!("../assets/inter/Inter-Italic[opsz,wght].ttf"),
    },
    // The UI monospace role (design/principles.md P5). Upstream moved it from Apache-2.0 to OFL-1.1.
    FontAsset {
        family: "Roboto Mono",
        italic: false,
        min_weight: 100,
        max_weight: 700,
        license: "OFL-1.1",
        data: include_bytes!("../assets/roboto-mono/RobotoMono[wght].ttf"),
    },
];

/// Fonts embedded in the binary; always available, no I/O.
pub fn bundled() -> &'static [FontAsset] {
    &BUNDLED
}

/// The bundled asset serving `req`, if any (family match is case-insensitive).
pub fn bundled_for(req: &FontRequest) -> Option<&'static FontAsset> {
    BUNDLED.iter().find(|a| a.family.eq_ignore_ascii_case(req.family.trim()) && a.italic == req.italic)
}
