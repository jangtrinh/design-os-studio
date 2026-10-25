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

/// The one environment variable the machine seed reads. The OS defines it for every process of an
/// account whichever way the app was started (shell, Dock, Explorer). Variables such as `HOSTNAME`
/// are exported by some launchers only, so reading them made one account's id depend on how the
/// app was opened.
const MACHINE_SEED_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn machine_seed_from(var: impl Fn(&str) -> Option<String>) -> String {
    let home = var(MACHINE_SEED_VAR).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "localhost".into());
    format!("{home}:{}", std::env::consts::OS)
}

fn machine_seed_string() -> String {
    machine_seed_from(|k| std::env::var(k).ok())
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

fn is_finite_color(c: &Color) -> bool {
    [c.r, c.g, c.b, c.a].iter().all(|v| v.is_finite())
}

/// An id read from a file, or `None` when it is absent or blank.
fn clean_id(id: Option<&str>) -> Option<&str> {
    id.map(str::trim).filter(|s| !s.is_empty())
}

/// A display name without surrounding spaces, or `fallback` when nothing is left.
fn display_name_or(name: &str, fallback: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

impl CommentAuthor {
    /// Migrates a legacy plain-string author name into a stable [`CommentAuthor`].
    pub fn from_legacy(author: &str) -> Self {
        Self::from_parts(None, author, None)
    }

    /// Builds an identity from what a file recorded. A missing id is derived from the lowercased
    /// name; a missing (or non-finite) colour is derived from the id, so one id always has one colour
    /// and renaming an author with a recorded id changes neither.
    fn from_parts(id: Option<&str>, name: &str, color: Option<Color>) -> Self {
        let display_name = display_name_or(name, "Anonymous");
        let id = match clean_id(id) {
            Some(id) => id.to_string(),
            None => format!("author-{:016x}", fnv1a64(display_name.to_lowercase().as_bytes())),
        };
        let color = color.filter(is_finite_color).unwrap_or_else(|| author_color_for(&id));
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

/// What a file may hold under an `author` key: the plain display name every older file used, or a
/// structured identity.
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

impl RawAuthor {
    /// The recorded `(id, display name, colour)`; a legacy name records no id or colour.
    fn into_parts(self) -> (Option<String>, String, Option<Color>) {
        match self {
            RawAuthor::Legacy(name) => (None, name, None),
            RawAuthor::Structured { id, display_name, name, color } => (id, display_name.or(name).unwrap_or_default(), color),
        }
    }
}

impl<'de> Deserialize<'de> for CommentAuthor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (id, name, color) = RawAuthor::deserialize(deserializer)?.into_parts();
        Ok(CommentAuthor::from_parts(id.as_deref(), &name, color))
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Comment {
    pub id: String,
    /// Display name chosen in the app's settings. Saved as a plain string, and editing it never
    /// changes `author_id` or `author_color`.
    pub author: String,
    /// Stable id of the author. `None` for comments saved before authors had one; their id is then
    /// derived from `author` (see [`Comment::author_identity`]). Saved next to `author`, which an
    /// older build reads as it always did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    /// The author's recorded colour; `None` derives it from the id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_color: Option<Color>,
    pub text: String,
    /// RFC 3339 timestamp.
    pub created: String,
    pub edited: Option<String>,
}

impl Comment {
    /// The [`CommentAuthor`] of this comment: the recorded id and colour when the file has them,
    /// otherwise the ones derived from the display name.
    pub fn author_identity(&self) -> CommentAuthor {
        CommentAuthor::from_parts(self.author_id.as_deref(), &self.author, self.author_color)
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
            author: RawAuthor,
            #[serde(default)]
            author_id: Option<String>,
            #[serde(default)]
            author_color: Option<Color>,
            text: String,
            created: String,
            #[serde(default)]
            edited: Option<String>,
        }

        let raw = RawComment::deserialize(deserializer)?;
        let (id, name, color) = raw.author.into_parts();
        let author = display_name_or(&name, "Anonymous");
        let author_id = clean_id(id.as_deref()).or(clean_id(raw.author_id.as_deref())).map(str::to_string);
        let author_color = color.filter(is_finite_color).or(raw.author_color.filter(is_finite_color));
        Ok(Comment { id: raw.id, author, author_id, author_color, text: raw.text, created: raw.created, edited: raw.edited })
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
    use std::sync::Mutex;

    /// Serialises the tests that read or change process environment variables.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Puts an environment variable back the way it was when the test ends, even on a panic.
    struct EnvRestore {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn capture(key: &'static str) -> Self {
            Self { key, old: std::env::var_os(key) }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// A colour that is not in the palette, so a test can tell "kept" from "derived".
    const SAVED_COLOR: Color = rgb_const(0x12, 0x34, 0x56);

    fn structured_comment_json(author: &str) -> String {
        format!(r#"{{"id":"c2","author":{author},"text":"Reply","created":"2026-10-17T00:01:00Z"}}"#)
    }

    fn explicit_author_json() -> String {
        let color = serde_json::to_string(&SAVED_COLOR).expect("colour json");
        structured_comment_json(&format!(r#"{{"id":"local-deadbeefcafebabe","display_name":"Kir","color":{color}}}"#))
    }

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
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

    #[test]
    fn explicit_author_identity_survives_load_and_save() {
        let loaded: Comment = serde_json::from_str(&explicit_author_json()).expect("structured comment json");
        let identity = loaded.author_identity();
        assert_eq!(identity.id, "local-deadbeefcafebabe", "the id written in the file must be kept on load");
        assert_eq!(identity.color, SAVED_COLOR, "the colour written in the file must be kept on load");
        assert_eq!(identity.display_name, "Kir");

        let saved = serde_json::to_string(&loaded).expect("comment json");
        let reloaded: Comment = serde_json::from_str(&saved).expect("saved comment json");
        assert_eq!(reloaded.author_identity(), identity, "saved json lost the author identity: {saved}");
        assert_eq!(reloaded, loaded);
    }

    #[test]
    fn display_name_edit_keeps_id_and_colour() {
        let mut comment: Comment = serde_json::from_str(&explicit_author_json()).expect("structured comment json");
        comment.author = "Kirill".into();

        let identity = comment.author_identity();
        assert_eq!(identity.display_name, "Kirill");
        assert_eq!(identity.id, "local-deadbeefcafebabe", "renaming must not change the author id");
        assert_eq!(identity.color, SAVED_COLOR, "renaming must not change the author colour");

        let saved = serde_json::to_string(&comment).expect("comment json");
        let reloaded: Comment = serde_json::from_str(&saved).expect("saved comment json");
        assert_eq!(reloaded.author_identity(), identity, "saved json lost the author identity: {saved}");
    }

    #[test]
    fn document_round_trip_keeps_comment_author_identity() {
        let mut doc = crate::Document::new("t");
        let page = doc.add_page("Page 1");
        let message: Comment = serde_json::from_str(&explicit_author_json()).expect("structured comment json");
        let legacy: Comment = serde_json::from_str(r#"{"id":"c1","author":"Kir","text":"Hi","created":"2026-10-17T00:00:00Z"}"#).expect("legacy comment json");
        doc.comments.push(CommentThread {
            id: "t1".into(),
            anchor: CommentAnchor { page, node: None, offset: Vec2::new(1.0, 2.0) },
            messages: vec![legacy.clone(), message.clone()],
            resolved: false,
        });

        let json = doc.to_json().expect("document json");
        let back = crate::Document::from_json(&json).expect("document loads again");
        let messages = &back.comments[0].messages;
        assert_eq!(messages[0].author_identity(), legacy.author_identity());
        assert_eq!(messages[1].author_identity().id, "local-deadbeefcafebabe", "document save dropped the author id");
        assert_eq!(messages[1].author_identity().color, SAVED_COLOR, "document save dropped the author colour");
        assert_eq!(messages[1].author_identity(), message.author_identity());
    }

    #[test]
    fn legacy_comment_saves_back_as_plain_author_string() {
        let json = r#"{"id":"c1","author":"Kir","text":"Check contrast","created":"2026-10-17T00:00:00Z","edited":null}"#;
        let legacy: Comment = serde_json::from_str(json).expect("legacy comment json");
        let saved = serde_json::to_string(&legacy).expect("comment json");
        assert!(saved.contains(r#""author":"Kir""#), "a legacy author must stay a plain string: {saved}");
        assert!(!saved.contains("author_id") && !saved.contains("author_color"), "a legacy comment gains no identity keys: {saved}");
        assert_eq!(serde_json::from_str::<Comment>(&saved).expect("saved comment json"), legacy);
    }

    #[test]
    fn hostile_author_payloads_never_panic_or_yield_non_finite_colour() {
        let payloads = [
            r#"{}"#,
            r#"{"id":"   ","display_name":"   "}"#,
            r#"{"display_name":"Kir","color":{"r":1e39,"g":0.5,"b":0.5,"a":1.0}}"#,
            r#"{"display_name":"Kir","color":{"r":-1e39,"g":0.5,"b":0.5,"a":1.0}}"#,
            r#"{"id":"x","name":"Legacy Name"}"#,
            r#""""#,
        ];
        for payload in payloads {
            let comment: Comment = serde_json::from_str(&structured_comment_json(payload)).unwrap_or_else(|e| panic!("{payload}: {e}"));
            let identity = comment.author_identity();
            let c = identity.color;
            assert!([c.r, c.g, c.b, c.a].iter().all(|v| v.is_finite()), "{payload}: non-finite colour {c:?}");
            assert!(!identity.id.trim().is_empty() && !identity.display_name.trim().is_empty(), "{payload}: blank identity {identity:?}");
            let saved = serde_json::to_string(&comment).expect("comment json");
            let reloaded: Comment = serde_json::from_str(&saved).unwrap_or_else(|e| panic!("{payload}: saved json {saved} does not reload: {e}"));
            assert_eq!(reloaded.author_identity(), identity, "{payload}: identity changed across save: {saved}");
        }
    }

    #[test]
    fn non_finite_saved_colour_falls_back_to_the_derived_colour() {
        // 1e39 does not fit an f32: it parses as infinity, which would save as `null` and break the reload.
        let json = structured_comment_json(r#"{"id":"local-0123456789abcdef","display_name":"Kir","color":{"r":1e39,"g":0.5,"b":0.5,"a":1.0}}"#);
        let comment: Comment = serde_json::from_str(&json).expect("structured comment json");
        assert_eq!(comment.author_color, None, "a colour with an infinite channel must not be kept");
        assert_eq!(comment.author_id.as_deref(), Some("local-0123456789abcdef"));
        assert_eq!(comment.author_identity().color, author_color_for("local-0123456789abcdef"));
    }

    #[test]
    fn saved_identity_is_ignored_by_a_reader_that_knows_only_the_original_fields() {
        #[derive(Deserialize)]
        struct OriginalComment {
            id: String,
            author: String,
            text: String,
            created: String,
            edited: Option<String>,
        }

        let loaded: Comment = serde_json::from_str(&explicit_author_json()).expect("structured comment json");
        let saved = serde_json::to_string(&loaded).expect("comment json");
        let original: OriginalComment = serde_json::from_str(&saved).expect("an older build reads the saved comment");
        assert_eq!((original.id.as_str(), original.author.as_str(), original.text.as_str()), ("c2", "Kir", "Reply"));
        assert_eq!((original.created.as_str(), original.edited), ("2026-10-17T00:01:00Z", None));
    }

    #[test]
    fn identity_keys_next_to_a_plain_author_string_are_read() {
        let color = serde_json::to_string(&SAVED_COLOR).expect("colour json");
        let json = format!(
            r#"{{"id":"c3","author":"Kir","author_id":" local-feedface00000001 ","author_color":{color},"text":"Hi","created":"2026-10-17T00:02:00Z","edited":null}}"#
        );
        let comment: Comment = serde_json::from_str(&json).expect("comment json");
        assert_eq!(comment.author_id.as_deref(), Some("local-feedface00000001"));
        assert_eq!(comment.author_identity().color, SAVED_COLOR);
        let saved = serde_json::to_string(&comment).expect("comment json");
        assert!(saved.contains(r#""author":"Kir""#) && saved.contains(r#""author_id":"local-feedface00000001""#), "{saved}");
    }

    #[test]
    fn colour_follows_the_id_not_the_letter_case_of_the_name() {
        let upper = CommentAuthor::from_legacy("Kir");
        let lower = CommentAuthor::from_legacy("kir");
        assert_eq!(upper.id, lower.id);
        assert_eq!(upper.color, lower.color, "one id must map to one colour");
        assert_eq!(upper.color, author_color_for(&upper.id));

        let no_colour: CommentAuthor = serde_json::from_str(r#"{"id":"local-0123456789abcdef","display_name":"Kir"}"#).expect("structured author");
        assert_eq!(no_colour.color, author_color_for("local-0123456789abcdef"), "a structured author without a colour is coloured from its id");

        let machine = CommentAuthor::for_machine("box:jang:macos", "Jang");
        assert_eq!(machine.color, author_color_for(&machine.id));
    }

    #[test]
    fn machine_seed_reads_one_variable_and_the_os() {
        let os = std::env::consts::OS;
        let noisy = |k: &str| match k {
            "HOME" | "USERPROFILE" => Some(" /Users/jang ".to_string()),
            "HOSTNAME" | "COMPUTERNAME" | "USER" | "USERNAME" | "LOGNAME" => Some("other-value".to_string()),
            _ => None,
        };
        assert_eq!(machine_seed_from(noisy), format!("/Users/jang:{os}"));
        assert_eq!(machine_seed_from(|_| None), format!("localhost:{os}"));
        assert_eq!(machine_seed_from(|_| Some("   ".to_string())), format!("localhost:{os}"));
    }

    #[test]
    fn machine_id_ignores_hostname_that_only_some_launches_export() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvRestore::capture("HOSTNAME");

        std::env::remove_var("HOSTNAME");
        let without_hostname = machine_author_id();
        std::env::set_var("HOSTNAME", "build-box-17");
        let with_hostname = machine_author_id();
        assert_eq!(without_hostname, with_hostname, "the same account must keep one machine id whether or not HOSTNAME is exported");
    }
}
