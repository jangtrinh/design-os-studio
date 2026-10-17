//! Comment threads pinned to the canvas.

use crate::{Color, NodeId, Vec2};
use serde::{Deserialize, Serialize};

const fn rgb_const(r: u8, g: u8, b: u8) -> Color {
    Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
}

/// Canonical 6-colour comment/avatar palette.
pub const AUTHOR_PALETTE: [Color; 6] = [
    rgb_const(0xF2, 0x48, 0x22),
    rgb_const(0xFF, 0xA6, 0x29),
    rgb_const(0x14, 0xAE, 0x5C),
    rgb_const(0x0C, 0x8C, 0xE9),
    rgb_const(0x06, 0xB6, 0xD4),
    rgb_const(0xEE, 0x46, 0xBC),
];

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Deterministic avatar/pin colour from [`AUTHOR_PALETTE`] using the djb2 hash.
pub fn author_color_for(seed: &str) -> Color {
    let h = seed.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    AUTHOR_PALETTE[h as usize % AUTHOR_PALETTE.len()]
}

fn os_user_name() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "You".into())
}

fn machine_seed_string() -> String {
    let user = os_user_name();
    let host = ["HOSTNAME", "COMPUTERNAME", "HOME", "USERPROFILE"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into());
    format!("{host}:{user}:{}", std::env::consts::OS)
}

/// Stable per-machine author identifier (`"local-<16 hex>"`).
pub fn machine_author_id() -> String {
    format!("local-{:016x}", fnv1a64(machine_seed_string().as_bytes()))
}

/// Author identity with a stable ID, display name, and deterministic colour.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommentAuthor {
    pub id: String,
    pub display_name: String,
    pub color: Color,
}

impl CommentAuthor {
    /// Migrates a legacy plain-string author name into a stable [`CommentAuthor`].
    pub fn from_legacy(author: &str) -> Self {
        let trimmed = author.trim();
        let display_name = if trimmed.is_empty() { "Anonymous".to_string() } else { trimmed.to_string() };
        let norm = display_name.to_lowercase();
        let id = format!("author-{:016x}", fnv1a64(norm.as_bytes()));
        let color = author_color_for(&display_name);
        Self { id, display_name, color }
    }

    /// Constructs a stable per-machine identity from an explicit machine seed and display name.
    pub fn for_machine(machine_seed: &str, display_name: &str) -> Self {
        let trimmed = display_name.trim();
        let display_name = if trimmed.is_empty() { "You".to_string() } else { trimmed.to_string() };
        let id = format!("local-{:016x}", fnv1a64(machine_seed.trim().as_bytes()));
        let color = author_color_for(&id);
        Self { id, display_name, color }
    }

    /// Returns the local machine's stable [`CommentAuthor`] using the OS user name.
    pub fn local() -> Self {
        Self::for_machine(&machine_seed_string(), &os_user_name())
    }

    /// Returns the local machine's stable [`CommentAuthor`] with a custom display name.
    pub fn local_with_name(display_name: &str) -> Self {
        let name = if display_name.trim().is_empty() { os_user_name() } else { display_name.trim().to_string() };
        Self::for_machine(&machine_seed_string(), &name)
    }
}

impl<'de> Deserialize<'de> for CommentAuthor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawAuthor {
            Legacy(String),
            Structured {
                #[serde(default)]
                id: Option<String>,
                #[serde(default)]
                display_name: Option<String>,
                #[serde(default)]
                name: Option<String>,
                #[serde(default)]
                color: Option<Color>,
            },
        }

        match RawAuthor::deserialize(deserializer)? {
            RawAuthor::Legacy(s) => Ok(CommentAuthor::from_legacy(&s)),
            RawAuthor::Structured { id, display_name, name, color } => {
                let raw_name = display_name.or(name).unwrap_or_default();
                let base = CommentAuthor::from_legacy(&raw_name);
                let id = id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or(base.id);
                let color = color.unwrap_or(base.color);
                Ok(CommentAuthor { id, display_name: base.display_name, color })
            }
        }
    }
}

/// Where a thread is pinned. With `node`, `offset` is relative to the node's
/// top-left so the pin follows it; otherwise `offset` is in page space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommentAnchor {
    pub page: NodeId,
    pub node: Option<NodeId>,
    pub offset: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Comment {
    pub id: String,
    /// Display name chosen in the app's settings.
    pub author: String,
    pub text: String,
    /// RFC 3339 timestamp.
    pub created: String,
    pub edited: Option<String>,
}

impl Comment {
    /// Returns the migrated [`CommentAuthor`] (stable ID, display name, and colour) for this comment.
    pub fn author_identity(&self) -> CommentAuthor {
        CommentAuthor::from_legacy(&self.author)
    }
}

impl<'de> Deserialize<'de> for Comment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawComment {
            id: String,
            author: CommentAuthor,
            text: String,
            created: String,
            #[serde(default)]
            edited: Option<String>,
        }

        let raw = RawComment::deserialize(deserializer)?;
        Ok(Comment { id: raw.id, author: raw.author.display_name, text: raw.text, created: raw.created, edited: raw.edited })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommentThread {
    pub id: String,
    pub anchor: CommentAnchor,
    /// First message is the thread's opening comment.
    pub messages: Vec<Comment>,
    pub resolved: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_author_strings_migrate_to_stable_identity_and_palette_colour() {
        let kir = CommentAuthor::from_legacy("  Kir  ");
        assert_eq!(kir.display_name, "Kir");
        assert!(kir.id.starts_with("author-"), "expected stable author id, got {}", kir.id);
        assert!(AUTHOR_PALETTE.contains(&kir.color), "expected palette colour, got {:?}", kir.color);
        assert_eq!(kir, CommentAuthor::from_legacy("Kir"));

        // Deserializing CommentAuthor directly from a legacy JSON string migrates it
        let from_str: CommentAuthor = serde_json::from_str("\"Ada Lovelace\"").expect("legacy string migrates");
        assert_eq!(from_str.display_name, "Ada Lovelace");
        assert!(from_str.id.starts_with("author-"));
        assert!(AUTHOR_PALETTE.contains(&from_str.color));

        // Deserializing a Comment with either a legacy author string or a structured author object works
        let legacy_comment: Comment =
            serde_json::from_str(r#"{"id":"c1","author":"Kir","text":"Check contrast","created":"2026-10-17T00:00:00Z","edited":null}"#)
                .expect("legacy comment json");
        assert_eq!(legacy_comment.author, "Kir");
        assert_eq!(legacy_comment.author_identity(), kir);

        let structured_comment: Comment = serde_json::from_str(
            r#"{"id":"c2","author":{"id":"local-deadbeefcafebabe","display_name":"Kir","color":{"r":0.95,"g":0.28,"b":0.13,"a":1.0}},"text":"Reply","created":"2026-10-17T00:01:00Z","edited":null}"#,
        )
        .expect("structured comment json");
        assert_eq!(structured_comment.author, "Kir");
        assert_eq!(structured_comment.author_identity().display_name, "Kir");
    }

    #[test]
    fn machine_identity_is_stable_across_calls_and_display_name_changes() {
        let a = CommentAuthor::local();
        let b = CommentAuthor::local();
        assert!(!a.id.is_empty());
        assert!(a.id.starts_with("local-"), "expected local- prefix, got {}", a.id);
        assert_eq!(a.id, b.id, "local machine identity must be stable");
        assert!(!a.display_name.is_empty());
        assert!(AUTHOR_PALETTE.contains(&a.color));

        let renamed = CommentAuthor::local_with_name("Ada Lovelace");
        assert_eq!(renamed.id, a.id, "changing display name keeps the same per-machine id");
        assert_eq!(renamed.display_name, "Ada Lovelace");

        let m1 = CommentAuthor::for_machine("macbook-pro:jang", "Jang");
        let m2 = CommentAuthor::for_machine("macbook-pro:jang", "Jang Trinh");
        let other = CommentAuthor::for_machine("other-host:bob", "Jang");
        assert_eq!(m1.id, m2.id, "same machine seed produces identical id");
        assert_ne!(m1.id, other.id, "different machine seed produces distinct id");
    }
}
