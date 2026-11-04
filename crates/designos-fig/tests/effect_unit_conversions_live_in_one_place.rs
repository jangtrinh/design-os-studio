//! The noise size and the specular angle of an effect are converted between Studio's values and the
//! stored ones in one pair of functions each. No other code does arithmetic on them, so a later change
//! of either unit is made in one place and a round trip stays lossless under any unit.

use std::fs;
use std::path::{Path, PathBuf};

const CONVERSIONS: &str = "effect_units.rs";
const FUNCTIONS: [&str; 4] = ["noise_size_from_kiwi", "noise_size_to_kiwi", "specular_angle_from_kiwi", "specular_angle_to_kiwi"];
const KEYS: [&str; 2] = ["\"noiseSize\"", "\"specularAngle\""];
/// The files that move values between Studio and the stored shapes, and the conversions each must call.
const BOUNDARIES: [(&str, [&str; 2]); 3] = [
    ("rest_import.rs", ["noise_size_from_kiwi", "specular_angle_from_kiwi"]),
    ("fig_export.rs", ["noise_size_to_kiwi", "specular_angle_to_kiwi"]),
    ("export.rs", ["noise_size_to_kiwi", "specular_angle_to_kiwi"]),
];
const ARITHMETIC: [&str; 9] = [" * ", " / ", " + ", " - ", ".to_radians", ".to_degrees", ".max(", ".min(", ".clamp("];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The code of `text` before its test module, without comment lines.
fn code_lines(text: &str) -> Vec<&str> {
    let end = text.find("#[cfg(test)]").unwrap_or(text.len());
    text[..end].lines().filter(|l| !l.trim_start().starts_with("//")).collect()
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn size_and_angle_conversions_live_in_one_pair_of_functions() {
    let mut files = Vec::new();
    sources(&src_dir(), &mut files);

    let home = fs::read_to_string(src_dir().join(CONVERSIONS)).unwrap_or_else(|_| panic!("src/{CONVERSIONS} holds the conversions"));
    for f in FUNCTIONS {
        assert!(home.contains(&format!("fn {f}(")), "{CONVERSIONS} defines {f}");
    }

    for path in files.iter().filter(|p| p.file_name().is_some_and(|n| n != CONVERSIONS)) {
        let text = fs::read_to_string(path).unwrap();
        for line in code_lines(&text).into_iter().filter(|l| KEYS.iter().any(|k| l.contains(k))) {
            let op = ARITHMETIC.iter().find(|op| line.contains(**op));
            assert!(op.is_none(), "{}: arithmetic on a converted value outside {CONVERSIONS}: {}", path.display(), line.trim());
        }
    }

    for (name, calls) in BOUNDARIES {
        let text = fs::read_to_string(src_dir().join(name)).unwrap();
        let code = code_lines(&text).join("\n");
        for f in calls {
            assert!(code.contains(f), "{name} converts through {f}");
        }
    }
}
