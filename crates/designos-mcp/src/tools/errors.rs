//! Named MCP error catalogue, structured error envelope, and output-schema builders (E15-5).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Maximum number of entries emitted in `valid_options` before truncating and setting `valid_options_total` (§8.5).
pub const MAX_VALID_OPTIONS: usize = 20;

/// Stable `UPPER_SNAKE_CASE` error codes returned by `designos-mcp` tools (§3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Arguments (6)
    InvalidArguments,
    UnknownField,
    MissingField,
    ValueOutOfRange,
    TextTooLong,
    InvalidColor,

    // Nodes (6)
    NodeNotFound,
    NotAContainer,
    PageRootProtected,
    MoveCycle,
    InstanceCycle,
    OverrideFieldNotApplicable,

    // Grounding (E15-1..3) (22)
    UnknownComponent,
    AmbiguousComponent,
    UnknownVariantProperty,
    UnknownVariantValue,
    NoSuchVariant,
    UnknownOverridePath,
    AmbiguousOverridePath,
    UnknownComponentProperty,
    PropertyTypeMismatch,
    FieldNotAllowedOnInstance,
    UnknownSlot,
    SlotFillNotAllowed,
    SlotChildCount,
    InvalidSlotValue,
    SlotContainerNotFound,
    UnknownVariable,
    AmbiguousVariable,
    VariableTypeMismatch,
    VariableScopeMismatch,
    UnknownStyle,
    StyleTypeMismatch,
    UnknownVariableMode,

    // Batch (E15-4) (4)
    BatchOpFailed,
    UnknownRef,
    DuplicateAs,
    BatchTooLarge,

    // Session (13)
    UnknownTool,
    ToolDisabled,
    UnsavedChanges,
    PathRequired,
    PathOutsideWorkspace,
    FileExists,
    FileTooLarge,
    DocumentInvalid,
    EditorNotNotified,
    EditorPublishFailed,
    PatchTooLarge,
    HistoryDropped,
    InternalError,

    // Active pack codes (3)
    InvalidSelector,
    UnknownLanguage,
    OutsideOwnedFrames,

    // In-progress flag codes (E15-7) (2)
    InProgressTargetNotRoot,
    NodeInProgressByOtherRun,

    // Codes owned by other specs' tools (lint, guidelines, design-system search, screenshot, prompts) (12)
    UnknownPreset,
    UnknownRule,
    ExactlyOneScope,
    InvalidCursor,
    UnknownTopic,
    TopicNotAvailable,
    UnknownPropertyKey,
    ExactlyOneTarget,
    InvalidRegion,
    ImageTooLarge,
    NodesOnDifferentPages,
    UnknownPrompt,

    // Idempotency-key codes (2)
    IdempotencyKeyReused,
    IdempotencyKeyStale,

    // Query codes (E15-17) (2)
    CursorStale,
    RuleUnavailable,

    // Output filename validation codes (#1003 RT-05/RT-06) (2)
    InvalidName,
    NameTooLong,
}

#[allow(non_upper_case_globals)]
impl ErrorCode {
    // Backwards-compatible aliases used across tool modules
    pub const OutOfRange: Self = Self::ValueOutOfRange;
    pub const InvalidGeometry: Self = Self::ValueOutOfRange;
    pub const UnsupportedNodeKind: Self = Self::InvalidArguments;
    pub const InvalidNodeKind: Self = Self::InvalidArguments;
    pub const PageNotFound: Self = Self::NodeNotFound;
    pub const NotATextNode: Self = Self::OverrideFieldNotApplicable;
    pub const CommentNotFound: Self = Self::NodeNotFound;
    pub const CommentThreadNotFound: Self = Self::NodeNotFound;
    pub const IdExhausted: Self = Self::InternalError;
    pub const ComponentNotFound: Self = Self::UnknownComponent;
    pub const NotAComponent: Self = Self::UnknownComponent;
    pub const UnknownComponentProp: Self = Self::UnknownComponentProperty;
    pub const PropTypeMismatch: Self = Self::PropertyTypeMismatch;
    pub const VariableNotFound: Self = Self::UnknownVariable;
    pub const StyleNotFound: Self = Self::UnknownStyle;
    pub const StyleKindMismatch: Self = Self::StyleTypeMismatch;
    pub const UnknownVariableCollection: Self = Self::UnknownVariable;
    pub const BatchTooManyOps: Self = Self::BatchTooLarge;
    pub const UnknownBatchHandle: Self = Self::UnknownRef;
    pub const DuplicateBatchHandle: Self = Self::DuplicateAs;
    pub const NoPreviousNode: Self = Self::UnknownRef;
    pub const OutsideWorkspace: Self = Self::PathOutsideWorkspace;
    pub const FileAlreadyExists: Self = Self::FileExists;
    pub const LivePatchRejected: Self = Self::EditorPublishFailed;
    pub const LivePatchWriteFailed: Self = Self::EditorPublishFailed;
    pub const LivePatchTooLarge: Self = Self::PatchTooLarge;
    pub const MalformedNodeGraph: Self = Self::DocumentInvalid;
    pub const LayerLimitExceeded: Self = Self::FileTooLarge;
    pub const UnsupportedExtension: Self = Self::InvalidArguments;
    pub const NotAFile: Self = Self::InvalidArguments;
    pub const EmptyRender: Self = Self::InvalidArguments;
    pub const IoError: Self = Self::InternalError;
    pub const SessionUnavailable: Self = Self::InternalError;
    pub const IdempotencyKeyConflict: Self = Self::IdempotencyKeyReused;
    pub const InvalidIdempotencyKey: Self = Self::InvalidArguments;

    /// Every error code variant in `ErrorCode` (the 50 catalogue codes from §3.3, the 3 active pack codes, the 2 in-progress flag codes, the 12 codes owned by other specs' tools, the 2 idempotency codes and the 2 query codes).
    pub const ALL: &'static [ErrorCode] = &[
        Self::InvalidArguments,
        Self::UnknownField,
        Self::MissingField,
        Self::ValueOutOfRange,
        Self::TextTooLong,
        Self::InvalidColor,
        Self::NodeNotFound,
        Self::NotAContainer,
        Self::PageRootProtected,
        Self::MoveCycle,
        Self::InstanceCycle,
        Self::OverrideFieldNotApplicable,
        Self::UnknownComponent,
        Self::AmbiguousComponent,
        Self::UnknownVariantProperty,
        Self::UnknownVariantValue,
        Self::NoSuchVariant,
        Self::UnknownOverridePath,
        Self::AmbiguousOverridePath,
        Self::UnknownComponentProperty,
        Self::PropertyTypeMismatch,
        Self::FieldNotAllowedOnInstance,
        Self::UnknownSlot,
        Self::SlotFillNotAllowed,
        Self::SlotChildCount,
        Self::InvalidSlotValue,
        Self::SlotContainerNotFound,
        Self::UnknownVariable,
        Self::AmbiguousVariable,
        Self::VariableTypeMismatch,
        Self::VariableScopeMismatch,
        Self::UnknownStyle,
        Self::StyleTypeMismatch,
        Self::UnknownVariableMode,
        Self::BatchOpFailed,
        Self::UnknownRef,
        Self::DuplicateAs,
        Self::BatchTooLarge,
        Self::UnknownTool,
        Self::ToolDisabled,
        Self::UnsavedChanges,
        Self::PathRequired,
        Self::PathOutsideWorkspace,
        Self::FileExists,
        Self::FileTooLarge,
        Self::DocumentInvalid,
        Self::EditorNotNotified,
        Self::EditorPublishFailed,
        Self::PatchTooLarge,
        Self::HistoryDropped,
        Self::InternalError,
        Self::InvalidSelector,
        Self::UnknownLanguage,
        Self::OutsideOwnedFrames,
        Self::InProgressTargetNotRoot,
        Self::NodeInProgressByOtherRun,
        Self::UnknownPreset,
        Self::UnknownRule,
        Self::ExactlyOneScope,
        Self::InvalidCursor,
        Self::UnknownTopic,
        Self::TopicNotAvailable,
        Self::UnknownPropertyKey,
        Self::ExactlyOneTarget,
        Self::InvalidRegion,
        Self::ImageTooLarge,
        Self::NodesOnDifferentPages,
        Self::UnknownPrompt,
        Self::IdempotencyKeyReused,
        Self::IdempotencyKeyStale,
        Self::CursorStale,
        Self::RuleUnavailable,
        Self::InvalidName,
        Self::NameTooLong,
    ];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidArguments => "INVALID_ARGUMENTS",
            Self::UnknownField => "UNKNOWN_FIELD",
            Self::MissingField => "MISSING_FIELD",
            Self::ValueOutOfRange => "VALUE_OUT_OF_RANGE",
            Self::TextTooLong => "TEXT_TOO_LONG",
            Self::InvalidColor => "INVALID_COLOR",
            Self::NodeNotFound => "NODE_NOT_FOUND",
            Self::NotAContainer => "NOT_A_CONTAINER",
            Self::PageRootProtected => "PAGE_ROOT_PROTECTED",
            Self::MoveCycle => "MOVE_CYCLE",
            Self::InstanceCycle => "INSTANCE_CYCLE",
            Self::OverrideFieldNotApplicable => "OVERRIDE_FIELD_NOT_APPLICABLE",
            Self::UnknownComponent => "UNKNOWN_COMPONENT",
            Self::AmbiguousComponent => "AMBIGUOUS_COMPONENT",
            Self::UnknownVariantProperty => "UNKNOWN_VARIANT_PROPERTY",
            Self::UnknownVariantValue => "UNKNOWN_VARIANT_VALUE",
            Self::NoSuchVariant => "NO_SUCH_VARIANT",
            Self::UnknownOverridePath => "UNKNOWN_OVERRIDE_PATH",
            Self::AmbiguousOverridePath => "AMBIGUOUS_OVERRIDE_PATH",
            Self::UnknownComponentProperty => "UNKNOWN_COMPONENT_PROPERTY",
            Self::PropertyTypeMismatch => "PROPERTY_TYPE_MISMATCH",
            Self::FieldNotAllowedOnInstance => "FIELD_NOT_ALLOWED_ON_INSTANCE",
            Self::UnknownSlot => "UNKNOWN_SLOT",
            Self::SlotFillNotAllowed => "SLOT_FILL_NOT_ALLOWED",
            Self::SlotChildCount => "SLOT_CHILD_COUNT",
            Self::InvalidSlotValue => "INVALID_SLOT_VALUE",
            Self::SlotContainerNotFound => "SLOT_CONTAINER_NOT_FOUND",
            Self::UnknownVariable => "UNKNOWN_VARIABLE",
            Self::AmbiguousVariable => "AMBIGUOUS_VARIABLE",
            Self::VariableTypeMismatch => "VARIABLE_TYPE_MISMATCH",
            Self::VariableScopeMismatch => "VARIABLE_SCOPE_MISMATCH",
            Self::UnknownStyle => "UNKNOWN_STYLE",
            Self::StyleTypeMismatch => "STYLE_TYPE_MISMATCH",
            Self::UnknownVariableMode => "UNKNOWN_VARIABLE_MODE",
            Self::BatchOpFailed => "BATCH_OP_FAILED",
            Self::UnknownRef => "UNKNOWN_REF",
            Self::DuplicateAs => "DUPLICATE_AS",
            Self::BatchTooLarge => "BATCH_TOO_LARGE",
            Self::UnknownTool => "UNKNOWN_TOOL",
            Self::ToolDisabled => "TOOL_DISABLED",
            Self::UnsavedChanges => "UNSAVED_CHANGES",
            Self::PathRequired => "PATH_REQUIRED",
            Self::PathOutsideWorkspace => "PATH_OUTSIDE_WORKSPACE",
            Self::FileExists => "FILE_EXISTS",
            Self::FileTooLarge => "FILE_TOO_LARGE",
            Self::DocumentInvalid => "DOCUMENT_INVALID",
            Self::EditorNotNotified => "EDITOR_NOT_NOTIFIED",
            Self::EditorPublishFailed => "EDITOR_PUBLISH_FAILED",
            Self::PatchTooLarge => "PATCH_TOO_LARGE",
            Self::HistoryDropped => "HISTORY_DROPPED",
            Self::InternalError => "INTERNAL_ERROR",
            Self::InvalidSelector => "INVALID_SELECTOR",
            Self::UnknownLanguage => "UNKNOWN_LANGUAGE",
            Self::OutsideOwnedFrames => "OUTSIDE_OWNED_FRAMES",
            Self::InProgressTargetNotRoot => "IN_PROGRESS_TARGET_NOT_ROOT",
            Self::NodeInProgressByOtherRun => "NODE_IN_PROGRESS_BY_OTHER_RUN",
            Self::UnknownPreset => "UNKNOWN_PRESET",
            Self::UnknownRule => "UNKNOWN_RULE",
            Self::ExactlyOneScope => "EXACTLY_ONE_SCOPE",
            Self::InvalidCursor => "INVALID_CURSOR",
            Self::UnknownTopic => "UNKNOWN_TOPIC",
            Self::TopicNotAvailable => "TOPIC_NOT_AVAILABLE",
            Self::UnknownPropertyKey => "UNKNOWN_PROPERTY_KEY",
            Self::ExactlyOneTarget => "EXACTLY_ONE_TARGET",
            Self::InvalidRegion => "INVALID_REGION",
            Self::ImageTooLarge => "IMAGE_TOO_LARGE",
            Self::NodesOnDifferentPages => "NODES_ON_DIFFERENT_PAGES",
            Self::UnknownPrompt => "UNKNOWN_PROMPT",
            Self::IdempotencyKeyReused => "IDEMPOTENCY_KEY_REUSED",
            Self::IdempotencyKeyStale => "IDEMPOTENCY_KEY_STALE",
            Self::CursorStale => "CURSOR_STALE",
            Self::RuleUnavailable => "RULE_UNAVAILABLE",
            Self::InvalidName => "INVALID_NAME",
            Self::NameTooLong => "NAME_TOO_LONG",
        }
    }

    pub const fn default_next_step(&self) -> &'static str {
        match self {
            Self::EditorNotNotified => "Call designos_get_document before editing.",
            Self::EditorPublishFailed => "Save the file or reopen Studio before continuing.",
            Self::PatchTooLarge => "Split the edit into smaller calls or save and reopen the document.",
            Self::HistoryDropped => "Call designos_get_document to verify state; undo is unavailable for the last edit.",
            Self::UnsavedChanges => "Call designos_save_file first, or pass discard_changes=true to discard the unsaved changes.",
            Self::PathRequired => "Pass path to a .designos file inside the workspace, then retry.",
            Self::FileExists => "Pass overwrite=true or choose a new path.",
            Self::FileTooLarge => "This file is over the limit in `expected`. Open it in DESIGN:OS Studio, or use a smaller file.",
            Self::PathOutsideWorkspace => "Pass a relative path inside the active workspace.",
            Self::InvalidName => "Choose a valid file name without control or direction-control characters, then retry.",
            Self::NameTooLong => "Choose a file name of at most 255 UTF-8 bytes, then retry.",
            Self::NodeNotFound => "Call designos_list_nodes or designos_get_document to inspect valid node IDs.",
            Self::UnknownComponent | Self::AmbiguousComponent => "Use one of valid_options or prefix with the library name (<Library>/<Component>).",
            Self::UnknownVariable | Self::AmbiguousVariable => "Call designos_get_variables or pick one of valid_options.",
            Self::UnknownStyle | Self::StyleTypeMismatch => "Call designos_get_document to inspect shared styles or use one of valid_options.",
            Self::BatchTooLarge => "Split the batch into chunks of at most 200 operations.",
            Self::ExactlyOneScope | Self::ExactlyOneTarget => "Remove the extra scope or target arguments so exactly one remains, then retry.",
            Self::TopicNotAvailable => "Call designos_get_guidelines without a topic to list the available topics.",
            Self::InternalError => "Call designos_get_document to inspect current state before retrying.",
            Self::InProgressTargetNotRoot => "Flag the root frame named in valid_options instead of a nested layer.",
            Self::NodeInProgressByOtherRun => "Wait for the other run to finish, or pass force:true only when that run is known dead.",
            Self::CursorStale => "Run the query again without a cursor.",
            Self::RuleUnavailable => "Remove the rule from the selector, or run it where its engine is available.",
            Self::IdempotencyKeyReused => "Send this payload with a new idempotency_key.",
            Self::IdempotencyKeyStale => {
                "Read the target nodes; this write was undone or is not in the open document. Do not send it again unless you have verified the target nodes are absent and you still want the write."
            }
            _ => "Adjust the value at `path` using `valid_options` or `corrected_example` and retry.",
        }
    }

    pub fn from_code_str(s: &str) -> Option<Self> {
        match s {
            "INVALID_ARGUMENTS" => Some(Self::InvalidArguments),
            "UNKNOWN_FIELD" => Some(Self::UnknownField),
            "MISSING_FIELD" => Some(Self::MissingField),
            "VALUE_OUT_OF_RANGE" | "INVALID_GEOMETRY" => Some(Self::ValueOutOfRange),
            "TEXT_TOO_LONG" => Some(Self::TextTooLong),
            "INVALID_COLOR" | "INVALID_COLOUR_SPEC" => Some(Self::InvalidColor),
            "NODE_NOT_FOUND" | "PAGE_NOT_FOUND" | "COMMENT_THREAD_NOT_FOUND" => Some(Self::NodeNotFound),
            "NOT_A_CONTAINER" => Some(Self::NotAContainer),
            "PAGE_ROOT_PROTECTED" => Some(Self::PageRootProtected),
            "MOVE_CYCLE" => Some(Self::MoveCycle),
            "INSTANCE_CYCLE" => Some(Self::InstanceCycle),
            "OVERRIDE_FIELD_NOT_APPLICABLE" | "NOT_A_TEXT_NODE" => Some(Self::OverrideFieldNotApplicable),
            "UNKNOWN_COMPONENT" | "COMPONENT_NOT_FOUND" | "NOT_A_COMPONENT" => Some(Self::UnknownComponent),
            "AMBIGUOUS_COMPONENT" => Some(Self::AmbiguousComponent),
            "UNKNOWN_VARIANT_PROPERTY" => Some(Self::UnknownVariantProperty),
            "UNKNOWN_VARIANT_VALUE" => Some(Self::UnknownVariantValue),
            "NO_SUCH_VARIANT" => Some(Self::NoSuchVariant),
            "UNKNOWN_OVERRIDE_PATH" => Some(Self::UnknownOverridePath),
            "AMBIGUOUS_OVERRIDE_PATH" => Some(Self::AmbiguousOverridePath),
            "UNKNOWN_COMPONENT_PROPERTY" | "UNKNOWN_COMPONENT_PROP" => Some(Self::UnknownComponentProperty),
            "PROPERTY_TYPE_MISMATCH" | "PROP_TYPE_MISMATCH" => Some(Self::PropertyTypeMismatch),
            "FIELD_NOT_ALLOWED_ON_INSTANCE" => Some(Self::FieldNotAllowedOnInstance),
            "UNKNOWN_SLOT" => Some(Self::UnknownSlot),
            "SLOT_FILL_NOT_ALLOWED" => Some(Self::SlotFillNotAllowed),
            "SLOT_CHILD_COUNT" => Some(Self::SlotChildCount),
            "INVALID_SLOT_VALUE" => Some(Self::InvalidSlotValue),
            "SLOT_CONTAINER_NOT_FOUND" => Some(Self::SlotContainerNotFound),
            "UNKNOWN_VARIABLE" | "VARIABLE_NOT_FOUND" | "UNKNOWN_VARIABLE_COLLECTION" | "UNKNOWN_COLLECTION" => Some(Self::UnknownVariable),
            "AMBIGUOUS_VARIABLE" => Some(Self::AmbiguousVariable),
            "VARIABLE_TYPE_MISMATCH" => Some(Self::VariableTypeMismatch),
            "VARIABLE_SCOPE_MISMATCH" => Some(Self::VariableScopeMismatch),
            "UNKNOWN_STYLE" | "STYLE_NOT_FOUND" => Some(Self::UnknownStyle),
            "STYLE_TYPE_MISMATCH" | "STYLE_KIND_MISMATCH" | "STYLE_PROPERTY_MISMATCH" => Some(Self::StyleTypeMismatch),
            "UNKNOWN_VARIABLE_MODE" | "UNKNOWN_MODE" => Some(Self::UnknownVariableMode),
            "BATCH_OP_FAILED" => Some(Self::BatchOpFailed),
            "UNKNOWN_REF" | "UNKNOWN_BATCH_HANDLE" | "NO_PREVIOUS_NODE" => Some(Self::UnknownRef),
            "DUPLICATE_AS" | "DUPLICATE_BATCH_HANDLE" => Some(Self::DuplicateAs),
            "BATCH_TOO_LARGE" | "BATCH_TOO_MANY_OPS" => Some(Self::BatchTooLarge),
            "UNKNOWN_TOOL" => Some(Self::UnknownTool),
            "TOOL_DISABLED" => Some(Self::ToolDisabled),
            "UNSAVED_CHANGES" => Some(Self::UnsavedChanges),
            "PATH_REQUIRED" => Some(Self::PathRequired),
            "PATH_OUTSIDE_WORKSPACE" => Some(Self::PathOutsideWorkspace),
            "FILE_EXISTS" => Some(Self::FileExists),
            "FILE_TOO_LARGE" => Some(Self::FileTooLarge),
            "DOCUMENT_INVALID" => Some(Self::DocumentInvalid),
            "EDITOR_NOT_NOTIFIED" => Some(Self::EditorNotNotified),
            "EDITOR_PUBLISH_FAILED" | "LIVE_PATCH_REJECTED" => Some(Self::EditorPublishFailed),
            "PATCH_TOO_LARGE" => Some(Self::PatchTooLarge),
            "HISTORY_DROPPED" => Some(Self::HistoryDropped),
            "INTERNAL_ERROR" | "ID_EXHAUSTED" | "INSERT_FAILED" => Some(Self::InternalError),
            "INVALID_SELECTOR" => Some(Self::InvalidSelector),
            "UNKNOWN_LANGUAGE" => Some(Self::UnknownLanguage),
            "OUTSIDE_OWNED_FRAMES" => Some(Self::OutsideOwnedFrames),
            "IN_PROGRESS_TARGET_NOT_ROOT" => Some(Self::InProgressTargetNotRoot),
            "NODE_IN_PROGRESS_BY_OTHER_RUN" => Some(Self::NodeInProgressByOtherRun),
            "UNKNOWN_PRESET" => Some(Self::UnknownPreset),
            "UNKNOWN_RULE" => Some(Self::UnknownRule),
            "EXACTLY_ONE_SCOPE" => Some(Self::ExactlyOneScope),
            "INVALID_CURSOR" => Some(Self::InvalidCursor),
            "UNKNOWN_TOPIC" => Some(Self::UnknownTopic),
            "TOPIC_NOT_AVAILABLE" => Some(Self::TopicNotAvailable),
            "UNKNOWN_PROPERTY_KEY" => Some(Self::UnknownPropertyKey),
            "EXACTLY_ONE_TARGET" => Some(Self::ExactlyOneTarget),
            "INVALID_REGION" => Some(Self::InvalidRegion),
            "IMAGE_TOO_LARGE" => Some(Self::ImageTooLarge),
            "NODES_ON_DIFFERENT_PAGES" => Some(Self::NodesOnDifferentPages),
            "UNKNOWN_PROMPT" => Some(Self::UnknownPrompt),
            "IDEMPOTENCY_KEY_REUSED" | "IDEMPOTENCY_KEY_CONFLICT" | "E_IDEMPOTENCY_CONFLICT" => Some(Self::IdempotencyKeyReused),
            "IDEMPOTENCY_KEY_STALE" => Some(Self::IdempotencyKeyStale),
            "CURSOR_STALE" => Some(Self::CursorStale),
            "RULE_UNAVAILABLE" => Some(Self::RuleUnavailable),
            "INVALID_NAME" => Some(Self::InvalidName),
            "NAME_TOO_LONG" => Some(Self::NameTooLong),
            _ => None,
        }
    }
}

/// Structured error envelope returned by `Engine::call` and `Session::call` (§3.1, §4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_options_total: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_example: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ToolError>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_op: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    pub safe_to_retry: bool,
    pub next_step: String,
}

impl ToolError {
    pub fn new(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: path.into(),
            received: None,
            expected: None,
            valid_options: Vec::new(),
            valid_options_total: None,
            corrected_example: None,
            cause: None,
            failed_op: None,
            op: None,
            safe_to_retry: !matches!(code, ErrorCode::FileTooLarge),
            next_step: code.default_next_step().to_string(),
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    pub fn with_received(mut self, received: impl Into<Value>) -> Self {
        if self.code != ErrorCode::PathOutsideWorkspace {
            self.received = Some(received.into());
        }
        self
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        let e = expected.into();
        if !e.is_empty() {
            self.expected = Some(e);
        }
        self
    }

    pub fn with_valid_options(mut self, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut opts: Vec<String> = options.into_iter().map(Into::into).collect();
        let total = opts.len();
        if total > MAX_VALID_OPTIONS {
            opts.truncate(MAX_VALID_OPTIONS);
            self.valid_options_total = Some(total);
        } else {
            self.valid_options_total = None;
        }
        self.valid_options = opts;
        self
    }

    pub fn with_corrected_example(mut self, example: Value) -> Self {
        self.corrected_example = Some(example);
        self
    }

    pub fn with_cause(mut self, cause: ToolError) -> Self {
        if self.path.is_empty() {
            self.path = cause.path.clone();
        }
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn with_failed_op(mut self, failed_op: usize, op: impl Into<String>) -> Self {
        self.failed_op = Some(failed_op);
        self.op = Some(op.into());
        self
    }

    pub fn with_safe_to_retry(mut self, safe: bool) -> Self {
        self.safe_to_retry = safe;
        if let Some(cause) = &mut self.cause {
            cause.safe_to_retry = safe;
        }
        self
    }

    pub fn with_next_step(mut self, next_step: impl Into<String>) -> Self {
        self.next_step = next_step.into();
        self
    }

    /// Wraps an inner operation failure in a `BATCH_OP_FAILED` envelope (§3.1, §3.3).
    pub fn batch_op_failed(failed_op: usize, op: impl Into<String>, cause: ToolError) -> Self {
        let op_str = op.into();
        let path = if cause.path.is_empty() { format!("/ops/{failed_op}") } else { cause.path.clone() };
        let msg = format!("op #{failed_op} ({op_str}) failed: {}", cause.message);
        let safe = cause.safe_to_retry;
        Self::new(ErrorCode::BatchOpFailed, path, msg).with_failed_op(failed_op, op_str).with_cause(cause).with_safe_to_retry(safe)
    }

    fn build_fields_map(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("code".into(), json!(self.code.as_str()));
        map.insert("message".into(), json!(self.message));
        map.insert("path".into(), json!(self.path));
        if self.code != ErrorCode::PathOutsideWorkspace {
            if let Some(received) = &self.received {
                map.insert("received".into(), received.clone());
            }
        }
        if let Some(expected) = &self.expected {
            map.insert("expected".into(), json!(expected));
        }
        if !self.valid_options.is_empty() {
            let total = self.valid_options_total.unwrap_or(self.valid_options.len());
            let capped: Vec<String> = self.valid_options.iter().take(MAX_VALID_OPTIONS).cloned().collect();
            map.insert("valid_options".into(), json!(capped));
            if total > MAX_VALID_OPTIONS {
                map.insert("valid_options_total".into(), json!(total));
            }
        } else if let Some(total) = self.valid_options_total {
            if total > MAX_VALID_OPTIONS {
                map.insert("valid_options_total".into(), json!(total));
            }
        }
        if let Some(ex) = &self.corrected_example {
            map.insert("corrected_example".into(), ex.clone());
        }
        if let Some(cause) = &self.cause {
            map.insert("cause".into(), Value::Object(cause.build_fields_map()));
        }
        if let Some(failed_op) = self.failed_op {
            map.insert("failed_op".into(), json!(failed_op));
        }
        if let Some(op) = &self.op {
            map.insert("op".into(), json!(op));
        }
        map.insert("safe_to_retry".into(), json!(self.safe_to_retry));
        map.insert("next_step".into(), json!(self.next_step));
        map
    }

    /// Converts this `ToolError` into the JSON object used in `CallToolResult::structured_error` (§3.1, §4).
    /// Includes both `"error": { ... }` (for `outputSchema` validation) and top-level envelope fields.
    pub fn to_json_value(&self) -> Value {
        let inner = self.build_fields_map();
        let mut top = inner.clone();
        top.insert("error".into(), Value::Object(inner));
        Value::Object(top)
    }

    /// Substring check across `message`, `code`, and the serialized JSON envelope.
    pub fn contains(&self, pat: &str) -> bool {
        self.message.contains(pat) || self.code.as_str().contains(pat) || self.to_string().contains(pat)
    }

    /// Prefix check across `message` and the serialized JSON envelope.
    pub fn starts_with(&self, pat: &str) -> bool {
        self.message.starts_with(pat) || self.to_string().starts_with(pat)
    }
}

impl ToolError {
    /// Wraps a failure from a helper that still returns a plain string.
    ///
    /// Three shapes are understood, so no tool loses its catalogue code or structured fields:
    /// - a JSON error envelope (`{"code": ..., "message": ..., "valid_options": [...], ...}`, as the
    ///   screenshot and design-context helpers return), parsed by [`ToolError::from_json_envelope`];
    /// - a leading `CODE:` token naming a known [`ErrorCode`];
    /// - a trailing `(safe_to_retry: ...)` marker, and a trailing `valid_options: [...]` JSON array,
    ///   which becomes the `valid_options` field and is removed from the message.
    ///
    /// Anything else becomes `INVALID_ARGUMENTS`.
    pub fn from_plain_message(msg: impl Into<String>) -> Self {
        let msg: String = msg.into();
        if let Some(parsed) = serde_json::from_str::<Value>(&msg).ok().and_then(|v| Self::from_json_envelope(&v)) {
            return parsed;
        }
        let (head, rest) = match msg.split_once(':') {
            Some((h, r)) if !h.is_empty() && h.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') => (Some(h), r.trim_start()),
            _ => (None, msg.as_str()),
        };
        let known = head.and_then(ErrorCode::from_code_str);
        let body = if known.is_some() { rest } else { msg.as_str() };
        let (text, safe) = match body.rsplit_once(" (safe_to_retry: ") {
            Some((t, tail)) if tail.ends_with(')') => (t, !tail.starts_with("false")),
            _ => (body, true),
        };
        let (text, options) = split_trailing_valid_options(text);
        let err = ToolError::new(known.unwrap_or(ErrorCode::InvalidArguments), "", text).with_safe_to_retry(safe);
        if options.is_empty() {
            err
        } else {
            err.with_valid_options(options)
        }
    }

    /// Builds a [`ToolError`] from a JSON error envelope with a string `code`. Returns `None` when
    /// `value` is not such an object. An unknown `code` becomes `INVALID_ARGUMENTS`; `valid_options`
    /// entries that are not strings (for example `{"max_long_edge": 1024}`) keep their compact JSON text.
    pub fn from_json_envelope(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let code_str = obj.get("code")?.as_str()?;
        let code = ErrorCode::from_code_str(code_str).unwrap_or(ErrorCode::InvalidArguments);
        let text = |key: &str| obj.get(key).and_then(Value::as_str);
        // Helpers repeat the code at the start of the message ("NODE_NOT_FOUND: ..."); the code has its own field.
        let message = text("message").map(|m| m.strip_prefix(code_str).and_then(|rest| rest.strip_prefix(": ")).unwrap_or(m)).unwrap_or(code_str);
        let mut err =
            ToolError::new(code, text("path").unwrap_or(""), message).with_safe_to_retry(obj.get("safe_to_retry").and_then(Value::as_bool).unwrap_or(true));
        if let Some(next_step) = text("next_step") {
            err = err.with_next_step(next_step);
        }
        if let Some(expected) = text("expected") {
            err = err.with_expected(expected);
        }
        if let Some(received) = obj.get("received") {
            err = err.with_received(received.clone());
        }
        if let Some(example) = obj.get("corrected_example") {
            err = err.with_corrected_example(example.clone());
        }
        if let Some(options) = obj.get("valid_options").and_then(Value::as_array) {
            err = err.with_valid_options(options.iter().map(|o| o.as_str().map_or_else(|| o.to_string(), str::to_string)));
        }
        Some(err)
    }
}

/// Splits a trailing `valid_options: ["a", "b"]` JSON array off `text` (with the `; ` or ` (` that
/// introduced it). Returns `text` unchanged and no options when the tail is not a string array.
fn split_trailing_valid_options(text: &str) -> (&str, Vec<String>) {
    const KEY: &str = "valid_options: ";
    let Some(at) = text.rfind(KEY) else { return (text, Vec::new()) };
    let tail = text[at + KEY.len()..].trim_end().trim_end_matches(')');
    let Ok(options) = serde_json::from_str::<Vec<String>>(tail) else { return (text, Vec::new()) };
    let head = text[..at].trim_end().trim_end_matches(['(', ';', ',']).trim_end();
    (head, options)
}

impl From<String> for ToolError {
    fn from(msg: String) -> Self {
        Self::from_plain_message(msg)
    }
}

impl From<&str> for ToolError {
    fn from(msg: &str) -> Self {
        Self::from_plain_message(msg)
    }
}

impl From<ToolError> for Value {
    fn from(e: ToolError) -> Self {
        e.to_json_value()
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string(&self.to_json_value()) {
            Ok(s) => write!(f, "{s}"),
            Err(_) => write!(f, "{}: {}", self.code.as_str(), self.message),
        }
    }
}

impl std::error::Error for ToolError {}

impl std::ops::Deref for ToolError {
    type Target = str;
    fn deref(&self) -> &str {
        &self.message
    }
}

/// Normalizes a `designos-doc` dotted/bracketed spec path (e.g. `"spec.children[0].fill"` or
/// `"overrides[0].path"`) into an RFC 6901 JSON Pointer rooted at `prefix` (e.g. `"/spec"` or `"/ops/2/spec"`).
pub fn spec_path_to_json_pointer(raw: &str, prefix: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('/') {
        if prefix.is_empty() || prefix == "/spec" || trimmed.starts_with(prefix) {
            return trimmed.to_string();
        }
        let rel = trimmed.strip_prefix("/spec").unwrap_or(trimmed);
        return format!("{prefix}{rel}");
    }

    let rel = if trimmed == "spec" {
        ""
    } else if let Some(rest) = trimmed.strip_prefix("spec.") {
        rest
    } else {
        trimmed
    };

    let base = if prefix.is_empty() { "/spec" } else { prefix };
    if rel.is_empty() {
        return base.to_string();
    }

    let mut out = String::from(base);
    for dot_seg in rel.split('.') {
        if dot_seg.is_empty() {
            continue;
        }
        let mut rem = dot_seg;
        while let Some((before_br, after_open)) = rem.split_once('[') {
            if !before_br.is_empty() {
                out.push('/');
                out.push_str(&escape_json_pointer_token(before_br));
            }
            if let Some((idx_str, tail)) = after_open.split_once(']') {
                out.push('/');
                out.push_str(idx_str);
                rem = tail;
            } else {
                out.push('/');
                out.push_str(&escape_json_pointer_token(after_open));
                rem = "";
                break;
            }
        }
        if !rem.is_empty() {
            out.push('/');
            out.push_str(&escape_json_pointer_token(rem));
        }
    }
    out
}

fn escape_json_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Converts a [`designos_doc::SpecError`] into a [`ToolError`] with an RFC 6901 JSON Pointer
/// rooted at `pointer_prefix` (e.g. `"/spec"` for `designos_apply_spec` or `"/ops/2/spec"` for batch).
pub fn from_spec_error(err: designos_doc::SpecError, pointer_prefix: &str) -> ToolError {
    let code = ErrorCode::from_code_str(&err.code).unwrap_or_else(|| {
        if err.message.contains("exhausted") {
            ErrorCode::InternalError
        } else if err.message.contains("slots can only be set on instance nodes") {
            ErrorCode::OverrideFieldNotApplicable
        } else {
            ErrorCode::InvalidArguments
        }
    });

    let raw_path = if !err.path.is_empty() { err.path.as_str() } else { err.field_path.as_deref().unwrap_or("") };
    let pointer = spec_path_to_json_pointer(raw_path, pointer_prefix);

    let mut tool_err = ToolError::new(code, pointer, err.message).with_valid_options(err.valid_options).with_safe_to_retry(err.safe_to_retry);

    if let Some(rec) = err.received {
        tool_err = tool_err.with_received(Value::String(rec));
    }
    if let Some(exp) = err.expected {
        tool_err = tool_err.with_expected(exp);
    }
    if let Some(ex) = err.corrected_example {
        tool_err = tool_err.with_corrected_example(ex);
    }
    tool_err
}

/// Classifies a `serde_json::Error` produced when deserializing tool arguments or a nested `NodeSpec`
/// into `UNKNOWN_FIELD`, `MISSING_FIELD`, `INVALID_SLOT_VALUE`, or `INVALID_ARGUMENTS` (§3.3, §8.1).
pub fn from_serde_error(err: &serde_json::Error, pointer_prefix: &str, is_spec: bool) -> ToolError {
    from_serde_error_with_args(err, pointer_prefix, is_spec, None)
}

/// Like [`from_serde_error`], but inspects `raw_args` when available to compute the exact nested
/// JSON Pointer (e.g. `/spec/children/0/flex_direction` or `/spec/overrides/0/fill_color`) and a
/// complete `corrected_example` object that passes validation when resent.
pub fn from_serde_error_with_args(err: &serde_json::Error, pointer_prefix: &str, is_spec: bool, raw_args: Option<&Value>) -> ToolError {
    let msg = err.to_string();

    if msg.contains("INVALID_SLOT_VALUE") {
        let ptr = if pointer_prefix.is_empty() {
            "/spec/slots".to_string()
        } else if pointer_prefix.ends_with("/spec") {
            format!("{pointer_prefix}/slots")
        } else {
            format!("{pointer_prefix}/spec/slots")
        };
        let corrected = raw_args.and_then(fix_invalid_slot_in_args).unwrap_or_else(|| json!({ "slots": { "header": false } }));
        let clean_msg = msg.strip_prefix("INVALID_SLOT_VALUE: ").unwrap_or(&msg).to_string();
        return ToolError::new(ErrorCode::InvalidSlotValue, ptr, clean_msg)
            .with_valid_options(vec!["false", "[NodeSpec...]"])
            .with_corrected_example(corrected);
    }

    if let Some((bad_field, valid_options)) = designos_doc::extract_unknown_field_options(&msg) {
        let is_layout_field =
            msg.contains("`main_align`") || msg.contains("`counter_align`") || valid_options.iter().any(|v| v == "direction" || v == "main_align");
        let is_override_field = msg.contains("`swap`") && msg.contains("`path`") && !msg.contains("`children`");
        let is_node_spec_field = msg.contains("`width_sizing`")
            || msg.contains("`children`")
            || valid_options.iter().any(|v| v == "children" || v == "width_sizing" || v == "preset");

        if let Some(root) = raw_args {
            if let Some(rel_ptr) = find_unknown_field_in_args(root, pointer_prefix, is_spec, &bad_field, is_layout_field, is_override_field, is_node_spec_field)
            {
                let ptr = format!("{pointer_prefix}{rel_ptr}/{}", escape_json_pointer_token(&bad_field));
                let corrected = build_unknown_field_corrected_example(root, &rel_ptr, &bad_field, is_layout_field, is_override_field, is_node_spec_field);
                let mut te = ToolError::new(ErrorCode::UnknownField, ptr, format!("Unknown field `{bad_field}`"))
                    .with_received(Value::String(bad_field))
                    .with_valid_options(valid_options);
                if let Some(ex) = corrected {
                    te = te.with_corrected_example(ex);
                }
                return te;
            }
        }

        let ptr = if is_layout_field {
            if pointer_prefix.is_empty() {
                format!("/spec/layout/{}", escape_json_pointer_token(&bad_field))
            } else if pointer_prefix.ends_with("/spec") {
                format!("{pointer_prefix}/layout/{}", escape_json_pointer_token(&bad_field))
            } else {
                format!("{pointer_prefix}/spec/layout/{}", escape_json_pointer_token(&bad_field))
            }
        } else if is_override_field {
            if pointer_prefix.is_empty() {
                format!("/spec/overrides/0/{}", escape_json_pointer_token(&bad_field))
            } else if pointer_prefix.ends_with("/spec") {
                format!("{pointer_prefix}/overrides/0/{}", escape_json_pointer_token(&bad_field))
            } else {
                format!("{pointer_prefix}/spec/overrides/0/{}", escape_json_pointer_token(&bad_field))
            }
        } else if is_spec && is_node_spec_field && !pointer_prefix.ends_with("/spec") {
            if pointer_prefix.is_empty() {
                format!("/spec/{}", escape_json_pointer_token(&bad_field))
            } else {
                format!("{pointer_prefix}/spec/{}", escape_json_pointer_token(&bad_field))
            }
        } else if pointer_prefix.is_empty() {
            format!("/{}", escape_json_pointer_token(&bad_field))
        } else {
            format!("{pointer_prefix}/{}", escape_json_pointer_token(&bad_field))
        };

        let corrected = valid_options.first().map(|first_valid| {
            let mut m = Map::new();
            m.insert(first_valid.clone(), json!("..."));
            Value::Object(m)
        });
        let mut te = ToolError::new(ErrorCode::UnknownField, ptr, format!("Unknown field `{bad_field}`"))
            .with_received(Value::String(bad_field))
            .with_valid_options(valid_options);
        if let Some(ex) = corrected {
            te = te.with_corrected_example(ex);
        }
        return te;
    }

    if let Some((_, after)) = msg.split_once("missing field `") {
        if let Some((missing, _)) = after.split_once('`') {
            let ptr = if pointer_prefix.is_empty() {
                format!("/{}", escape_json_pointer_token(missing))
            } else {
                format!("{pointer_prefix}/{}", escape_json_pointer_token(missing))
            };
            return ToolError::new(ErrorCode::MissingField, ptr, format!("Missing required field `{missing}`"))
                .with_expected(format!("required field `{missing}`"));
        }
    }

    if is_spec || raw_args.is_some_and(|r| r.get("spec").is_some()) {
        if let Some(spec_msg) = raw_args.and_then(describe_spec_args_error).or_else(|| humanize_spec_serde_message(&msg)) {
            return ToolError::new(ErrorCode::InvalidArguments, pointer_prefix, format!("Invalid arguments: {spec_msg}"));
        }
    }

    ToolError::new(ErrorCode::InvalidArguments, pointer_prefix, format!("Invalid arguments: {msg}"))
}

fn join_spec_path(prefix: &str, seg: &str) -> String {
    if prefix.is_empty() {
        seg.to_string()
    } else {
        format!("{prefix}/{seg}")
    }
}

fn is_finite_f32(v: &Value) -> bool {
    v.as_f64().is_some_and(|n| (n as f32).is_finite())
}

fn is_non_finite_f32_number(v: &Value) -> bool {
    v.is_number() && !is_finite_f32(v)
}

fn is_num_or_var(v: &Value) -> bool {
    is_finite_f32(v) || v.is_string()
}

fn is_u16(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| u16::try_from(n).is_ok())
}

fn is_u32(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| u32::try_from(n).is_ok())
}

fn is_valid_color(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    ["r", "g", "b", "a"].into_iter().all(|ch| obj.get(ch).is_some_and(is_finite_f32))
}

fn is_valid_comment_author(v: &Value) -> bool {
    if v.is_string() {
        return true;
    }
    let Some(obj) = v.as_object() else {
        return false;
    };
    obj.get("id").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("display_name").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("name").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("color").is_none_or(|x| x.is_null() || is_valid_color(x))
}

fn describe_comments_error(val: &Value, path: &str) -> Option<String> {
    let Some(arr) = val.as_array() else {
        return Some(format!("{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."));
    };
    for (i, item) in arr.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            return Some(format!(
                "{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."
            ));
        };
        let thread_path = format!("{path}/{i}");
        if !obj.get("id").is_some_and(Value::is_string)
            || !obj.get("resolved").is_some_and(Value::is_boolean)
            || obj.get("page_id").and_then(Value::as_u64).is_none()
            || !obj.get("page_name").is_some_and(Value::is_string)
            || !obj.get("world_x").is_some_and(is_finite_f32)
            || !obj.get("world_y").is_some_and(is_finite_f32)
            || !obj.get("target_node_id").is_none_or(|x| x.is_null() || x.as_u64().is_some())
            || !obj.get("target_node_name").is_none_or(|x| x.is_null() || x.is_string())
            || !obj.get("target_node_kind").is_none_or(|x| x.is_null() || x.is_string())
            || !obj.get("target_frame_id").is_none_or(|x| x.is_null() || x.as_u64().is_some())
            || !obj.get("target_frame_name").is_none_or(|x| x.is_null() || x.is_string())
        {
            return Some(format!(
                "{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."
            ));
        }
        let Some(msgs) = obj.get("messages").and_then(Value::as_array) else {
            return Some(format!(
                "{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."
            ));
        };
        for (j, msg) in msgs.iter().enumerate() {
            let Some(mo) = msg.as_object() else {
                return Some(format!(
                    "{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."
                ));
            };
            let msg_path = format!("{thread_path}/messages/{j}");
            if !mo.get("id").is_some_and(Value::is_string) {
                return Some(format!("{msg_path}/id must be a string. Pass a string value."));
            }
            if !mo.get("author").is_some_and(is_valid_comment_author) {
                return Some(format!(
                    "{msg_path}/author must be a string or an author object. Pass a display name string or an object such as {{\"id\": \"...\", \"name\": \"...\"}}."
                ));
            }
            if !mo.get("author_id").is_none_or(|x| x.is_null() || x.is_string()) {
                return Some(format!("{msg_path}/author_id must be a string. Pass a string value."));
            }
            if !mo.get("author_color").is_none_or(|x| x.is_null() || is_valid_color(x)) {
                return Some(format!("{msg_path}/author_color must be an RGBA color object with numeric r, g, b, and a fields between 0 and 1."));
            }
            if !mo.get("text").is_some_and(Value::is_string) {
                return Some(format!("{msg_path}/text must be a string. Pass a string value."));
            }
            if !mo.get("created").is_some_and(Value::is_string) {
                return Some(format!("{msg_path}/created must be a string. Pass an RFC 3339 timestamp string."));
            }
            if !mo.get("edited").is_none_or(|x| x.is_null() || x.is_string()) {
                return Some(format!("{msg_path}/edited must be a string. Pass an RFC 3339 timestamp string or null."));
            }
        }
    }
    None
}

fn describe_layout_spec_error(val: &Value, prefix: &str) -> Option<String> {
    let Some(obj) = val.as_object() else {
        return Some(format!("{prefix} must be a LayoutSpec object. Pass a JSON object such as {{\"direction\": \"row\", \"gap\": 8}}."));
    };
    for (k, v) in obj {
        let field_path = join_spec_path(prefix, k);
        match k.as_str() {
            "direction" | "dir" | "primary_align" | "justify" | "counter_align" | "align" => {
                if !v.is_string() {
                    return Some(format!("{field_path} must be a string. Pass a string value."));
                }
            }
            "wrap" => {
                if !v.is_boolean() {
                    return Some(format!("{field_path} must be a boolean. Pass true or false."));
                }
            }
            "gap" | "counter_gap" => {
                if is_non_finite_f32_number(v) {
                    return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                }
                if !is_num_or_var(v) {
                    return Some(format!("{field_path} must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."));
                }
            }
            "padding" => {
                if v.as_array().is_some_and(|a| a.len() == 4 && a.iter().any(is_non_finite_f32_number)) {
                    return Some(format!("{field_path} must contain finite numbers. Pass smaller numbers."));
                }
                if !v.as_array().is_some_and(|a| a.len() == 4 && a.iter().all(is_num_or_var)) {
                    return Some(format!(
                        "{field_path} must be an array of 4 numbers or \"var:...\" strings ([top, right, bottom, left]). Pass 4 values such as [16, 16, 16, 16]."
                    ));
                }
            }
            "grid_columns" | "grid_rows" => {
                if !v.is_null() && !is_u32(v) {
                    return Some(format!("{field_path} must be a positive integer. Pass a whole number of grid tracks."));
                }
            }
            _ => {}
        }
    }
    None
}

fn describe_slot_fill_error(slot_val: &Value, slot_path: &str) -> Option<String> {
    match slot_val {
        Value::Bool(false) => None,
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let item_path = format!("{slot_path}/{i}");
                if !item.is_object() {
                    return Some(format!("{item_path} must be a NodeSpec object. Pass each slot child as a JSON object."));
                }
                if let Some(err) = describe_spec_value_error_at(item, &item_path) {
                    return Some(err);
                }
            }
            None
        }
        _ => Some(format!("{slot_path} must be false or an array of NodeSpec objects. Pass false or [NodeSpec] for the slot.")),
    }
}

fn describe_slots_error(val: &Value, path: &str) -> Option<String> {
    let Some(map) = val.as_object() else {
        return Some(format!("{path} must be an object mapping slot names to false or an array of NodeSpec objects. Pass false or [NodeSpec] for each slot."));
    };
    for (slot_name, slot_val) in map {
        let slot_path = format!("{path}/{slot_name}");
        if let Some(err) = describe_slot_fill_error(slot_val, &slot_path) {
            return Some(err);
        }
    }
    None
}

fn describe_override_spec_error(val: &Value, prefix: &str) -> Option<String> {
    let Some(obj) = val.as_object() else {
        return Some(format!("{prefix} must be an OverrideSpec object. Pass each override as a JSON object."));
    };
    for (k, v) in obj {
        let field_path = join_spec_path(prefix, k);
        match k.as_str() {
            "path" | "text" | "fill" | "stroke" | "swap" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{field_path} must be a string. Pass a string value."));
                }
            }
            "id" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some(format!("{field_path} must be a non-negative integer. Pass a numeric node ID."));
                }
            }
            "visible" => {
                if !v.is_null() && !v.is_boolean() {
                    return Some(format!("{field_path} must be a boolean. Pass true or false."));
                }
            }
            "opacity" => {
                if !v.is_null() {
                    if is_non_finite_f32_number(v) {
                        return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                    }
                    if !is_finite_f32(v) {
                        return Some(format!("{field_path} must be a number between 0 and 1. Pass a numeric opacity such as 0.8."));
                    }
                }
            }
            "variant" => {
                if !v.as_object().is_some_and(|m| m.values().all(Value::is_string)) {
                    return Some(format!(
                        "{field_path} must be an object mapping variant property names to string values. Pass an object such as {{\"Size\": \"md\"}}."
                    ));
                }
            }
            "props" => {
                if !v.is_object() {
                    return Some(format!("{field_path} must be an object mapping component property names to values. Pass a JSON object."));
                }
            }
            "slots" => {
                if let Some(err) = describe_slots_error(v, &field_path) {
                    return Some(err);
                }
            }
            _ => {}
        }
    }
    None
}

fn describe_in_progress_error(val: &Value, prefix: &str) -> Option<String> {
    if val.is_null() || val.is_boolean() {
        return None;
    }
    let Some(obj) = val.as_object() else {
        return Some(format!("{prefix} must be a boolean or an in_progress object. Pass true, false, or an object such as {{\"agent\": \"...\"}}."));
    };
    for (k, v) in obj {
        let field_path = join_spec_path(prefix, k);
        match k.as_str() {
            "run_id" | "agent" | "agent_name" | "status" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{field_path} must be a string. Pass a string value."));
                }
            }
            "started_at_ms" | "since_ms" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some(format!("{field_path} must be a non-negative integer. Pass a whole number."));
                }
            }
            "stale_after_s" => {
                if !v.is_null() && !is_u32(v) {
                    return Some(format!("{field_path} must be a non-negative integer. Pass a whole number."));
                }
            }
            _ => {}
        }
    }
    None
}

/// Inspects a raw `NodeSpec` JSON value and returns a plain-words error message naming the first
/// field whose value has the wrong JSON type.
pub fn describe_spec_value_error(val: &Value) -> Option<String> {
    describe_spec_value_error_at(val, "")
}

fn describe_spec_value_error_at(val: &Value, prefix: &str) -> Option<String> {
    let Some(obj) = val.as_object() else {
        let label = if prefix.is_empty() { "spec" } else { prefix };
        return Some(format!("{label} must be a NodeSpec object. Pass a JSON object with node fields such as kind, name, and children."));
    };
    for (k, v) in obj {
        let field_path = join_spec_path(prefix, k);
        match k.as_str() {
            "id" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some(format!("{field_path} must be a non-negative integer. Pass a numeric node ID."));
                }
            }
            "kind" | "name" => {
                if !v.is_string() {
                    return Some(format!("{field_path} must be a string. Pass a string value."));
                }
            }
            "component" | "preset" | "width_sizing" | "height_sizing" | "fill" | "stroke" | "text" | "text_color" | "color" | "text_style" | "effect_style"
            | "grid_style" | "dev_status" | "annotation" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{field_path} must be a string. Pass a string value."));
                }
            }
            "variant" => {
                if !v.as_object().is_some_and(|m| m.values().all(Value::is_string)) {
                    return Some(format!(
                        "{field_path} must be an object mapping variant property names to string values. Pass an object such as {{\"Size\": \"md\"}}."
                    ));
                }
            }
            "mode" => {
                if !v.as_object().is_some_and(|m| m.values().all(Value::is_string)) {
                    return Some(format!(
                        "{field_path} must be an object mapping variable collection names to mode names. Pass an object such as {{\"Theme\": \"Dark\"}}."
                    ));
                }
            }
            "props" => {
                if !v.is_object() {
                    return Some(format!("{field_path} must be an object mapping component property names to values. Pass a JSON object."));
                }
            }
            "overrides" => {
                let Some(arr) = v.as_array() else {
                    return Some(format!("{field_path} must be an array of OverrideSpec objects. Pass each override as a JSON object."));
                };
                for (i, item) in arr.iter().enumerate() {
                    if !item.is_object() {
                        return Some(format!("{field_path} must be an array of OverrideSpec objects. Pass each override as a JSON object."));
                    }
                    let item_path = format!("{field_path}/{i}");
                    if let Some(err) = describe_override_spec_error(item, &item_path) {
                        return Some(err);
                    }
                }
            }
            "slots" => {
                if let Some(err) = describe_slots_error(v, &field_path) {
                    return Some(err);
                }
            }
            "x" | "y" => {
                if is_non_finite_f32_number(v) {
                    return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                }
                if !is_finite_f32(v) {
                    return Some(format!("{field_path} must be a number. Pass a numeric coordinate in pixels."));
                }
            }
            "min_width" | "max_width" | "min_height" | "max_height" => {
                if !v.is_null() {
                    if is_non_finite_f32_number(v) {
                        return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                    }
                    if !is_finite_f32(v) {
                        return Some(format!("{field_path} must be a number. Pass a numeric size in pixels."));
                    }
                }
            }
            "width" | "height" | "stroke_weight" | "stroke_width" | "corner_radius" | "radius" | "font_size" | "line_height" => {
                if !v.is_null() {
                    if is_non_finite_f32_number(v) {
                        return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                    }
                    if !is_num_or_var(v) {
                        return Some(format!("{field_path} must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."));
                    }
                }
            }
            "opacity" => {
                if is_non_finite_f32_number(v) {
                    return Some(format!("{field_path} must be a finite number. Pass a smaller number."));
                }
                if !is_num_or_var(v) {
                    return Some(format!(
                        "{field_path} must be a number between 0 and 1 or a \"var:...\" string. Pass a number such as 0.8 or a variable reference."
                    ));
                }
            }
            "corner_radii" => {
                if !v.is_null() {
                    if v.as_array().is_some_and(|a| a.len() == 4 && a.iter().any(is_non_finite_f32_number)) {
                        return Some(format!("{field_path} must contain finite numbers. Pass smaller numbers."));
                    }
                    if !v.as_array().is_some_and(|a| a.len() == 4 && a.iter().all(is_num_or_var)) {
                        return Some(format!(
                            "{field_path} must be an array of 4 numbers or \"var:...\" strings ([tl, tr, br, bl]). Pass 4 values such as [8, 8, 8, 8]."
                        ));
                    }
                }
            }
            "col_span" | "row_span" => {
                if !v.is_null() && !is_u16(v) {
                    return Some(format!("{field_path} must be a positive integer. Pass a whole number of grid tracks."));
                }
            }
            "font_weight" => {
                if !v.is_null() && !is_u16(v) {
                    return Some(format!("{field_path} must be a number between 100 and 900 (for example 400, 500, 600, or 700). Pass an integer weight."));
                }
            }
            "clips_content" => {
                if !v.is_null() && !v.is_boolean() {
                    return Some(format!("{field_path} must be a boolean. Pass true or false."));
                }
            }
            "visible" => {
                if !v.is_null() && !v.is_boolean() && !v.is_string() {
                    return Some(format!("{field_path} must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference."));
                }
            }
            "layout" => {
                if !v.is_null() {
                    if let Some(err) = describe_layout_spec_error(v, &field_path) {
                        return Some(err);
                    }
                }
            }
            "annotations" => {
                if !v.as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
                    return Some(format!("{field_path} must be an array of strings. Pass an array such as [\"Note\"]."));
                }
            }
            "in_progress" => {
                if let Some(err) = describe_in_progress_error(v, &field_path) {
                    return Some(err);
                }
            }
            "comments" => {
                if let Some(err) = describe_comments_error(v, &field_path) {
                    return Some(err);
                }
            }
            "children" => {
                let Some(arr) = v.as_array() else {
                    return Some(format!("{field_path} must be an array of NodeSpec objects. Pass each child as a JSON object."));
                };
                for (i, child) in arr.iter().enumerate() {
                    if !child.is_object() {
                        return Some(format!("{field_path} must be an array of NodeSpec objects. Pass each child as a JSON object."));
                    }
                    let child_path = format!("{field_path}/{i}");
                    if let Some(err) = describe_spec_value_error_at(child, &child_path) {
                        return Some(err);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Inspects raw `designos_apply_spec`, batch `apply_spec`, or batch `set_slot` arguments for spec field type errors.
pub fn describe_spec_args_error(raw_args: &Value) -> Option<String> {
    if let Some(obj) = raw_args.as_object() {
        let op = obj.get("op").and_then(Value::as_str);
        if op == Some("set_slot") {
            for key in ["node", "node_id", "instance"] {
                if let Some(node) = obj.get(key) {
                    if node.as_u64().is_none() && !node.is_string() {
                        return Some(format!("{key} must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference."));
                    }
                }
            }
            if let Some(slot) = obj.get("slot") {
                if !slot.is_string() {
                    return Some("slot must be a string. Pass a slot name string.".into());
                }
            }
            if let Some(alias) = obj.get("as") {
                if !alias.is_null() && !alias.is_string() {
                    return Some("as must be a string. Pass an alias name string.".into());
                }
            }
            for key in ["fill", "value"] {
                if let Some(slot_val) = obj.get(key) {
                    return describe_slot_fill_error(slot_val, key);
                }
            }
            return None;
        }
        if let Some(pid) = obj.get("parent_id") {
            if op == Some("apply_spec") {
                if !pid.is_null() && pid.as_u64().is_none() && !pid.is_string() {
                    return Some("parent_id must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference.".into());
                }
            } else if !pid.is_null() && pid.as_u64().is_none() {
                return Some("parent_id must be a non-negative integer. Pass a numeric node ID.".into());
            }
        }
        if let Some(p) = obj.get("parent") {
            if !p.is_null() && p.as_u64().is_none() && !p.is_string() {
                return Some("parent must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference.".into());
            }
        }
        if let Some(idx) = obj.get("index") {
            if !idx.is_null() && idx.as_u64().is_none() {
                return Some("index must be a non-negative integer. Pass a zero-based child index.".into());
            }
        }
        if let Some(alias) = obj.get("as") {
            if !alias.is_null() && !alias.is_string() {
                return Some("as must be a string. Pass an alias name string.".into());
            }
        }
        if let Some(di) = obj.get("diff_image") {
            if !di.is_boolean() {
                return Some("diff_image must be a boolean. Pass true or false.".into());
            }
        }
        if let Some(ik) = obj.get("idempotency_key") {
            if !ik.is_null() && !ik.is_string() {
                return Some("idempotency_key must be a string. Pass a string key.".into());
            }
        }
        if let Some(spec) = obj.get("spec") {
            return describe_spec_value_error(spec);
        }
    }
    None
}

/// Parses a raw `NodeSpec` JSON string for CLI `--apply-spec`, returning a plain-words error
/// message (with nested field path and no internal error-code prefix) on invalid input.
pub fn parse_spec_json(raw: &str) -> Result<designos_doc::NodeSpec, String> {
    let val: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    match serde_json::from_str::<designos_doc::NodeSpec>(raw) {
        Ok(spec) => {
            if let Some(msg) = describe_spec_value_error(&val) {
                return Err(msg);
            }
            Ok(spec)
        }
        Err(e) => {
            let msg = e.to_string();
            if let Some((bad, valid)) = designos_doc::extract_unknown_field_options(&msg) {
                if valid.is_empty() {
                    return Err(format!("Unknown field `{bad}`."));
                }
                return Err(format!("Unknown field `{bad}` (valid options: {})", valid.join(", ")));
            }
            if let Some(clean) = msg.strip_prefix("INVALID_SLOT_VALUE: ") {
                if let Some(spec_msg) = describe_spec_value_error(&val) {
                    return Err(spec_msg);
                }
                return Err(clean.to_string());
            }
            Err(describe_spec_value_error(&val).or_else(|| humanize_spec_serde_message(&msg)).unwrap_or(msg))
        }
    }
}

/// Replaces Rust type names in a raw `serde_json` error string with plain spec wording.
pub fn humanize_spec_serde_message(msg: &str) -> Option<String> {
    if msg.contains("untagged enum NumberOrVar") {
        return Some("value must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference.".into());
    }
    if msg.contains("untagged enum BoolOrVar") {
        return Some("visible must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference.".into());
    }
    if msg.contains("untagged enum InProgressSpec") {
        return Some("in_progress must be a boolean or an in_progress object. Pass true, false, or an object such as {\"agent\": \"...\"}.".into());
    }
    let replaced = msg
        .replace("struct NodeSpec", "a NodeSpec object")
        .replace("struct LayoutSpec", "a LayoutSpec object")
        .replace("struct OverrideSpec", "an OverrideSpec object")
        .replace("struct ResolvedCommentThread", "a comment thread object")
        .replace("struct Color", "an RGBA color object")
        .replace("a tuple of size 4", "an array of 4 numbers or \"var:...\" strings")
        .replace("expected u64", "expected a non-negative integer")
        .replace("expected u32", "expected a non-negative integer")
        .replace("expected u16", "expected a non-negative integer")
        .replace("expected f32", "expected a number");
    (replaced != msg).then_some(replaced)
}

fn fix_invalid_slot_in_args(root: &Value) -> Option<Value> {
    let mut fixed = root.clone();
    let slots_val = if fixed.get("spec").and_then(|s| s.get("slots")).is_some() {
        fixed.pointer_mut("/spec/slots")?
    } else if fixed.get("slots").is_some() {
        fixed.pointer_mut("/slots")?
    } else {
        return None;
    };
    if let Value::Object(map) = slots_val {
        for v in map.values_mut() {
            if *v != Value::Bool(false) && !v.is_array() {
                *v = Value::Bool(false);
            }
        }
        return Some(fixed);
    }
    None
}

fn find_unknown_field_in_args(
    root: &Value,
    pointer_prefix: &str,
    is_spec: bool,
    bad_field: &str,
    is_layout: bool,
    is_override: bool,
    is_node_spec: bool,
) -> Option<String> {
    if !is_layout && !is_override && !is_node_spec && root.as_object().is_some_and(|m| m.contains_key(bad_field)) {
        return Some(String::new());
    }
    if is_spec || is_layout || is_override || is_node_spec {
        if !pointer_prefix.ends_with("/spec") {
            if let Some(spec_val) = root.get("spec") {
                if let Some(found) = find_in_node_spec(spec_val, "/spec", bad_field, is_layout, is_override, is_node_spec) {
                    return Some(found);
                }
            }
            for slot_key in ["value", "fill"] {
                if let Some(arr) = root.get(slot_key).and_then(Value::as_array) {
                    for (i, item) in arr.iter().enumerate() {
                        let item_ptr = format!("/{slot_key}/{i}");
                        if let Some(found) = find_in_node_spec(item, &item_ptr, bad_field, is_layout, is_override, is_node_spec) {
                            return Some(found);
                        }
                    }
                }
            }
        }
        if let Some(found) = find_in_node_spec(root, "", bad_field, is_layout, is_override, is_node_spec) {
            return Some(found);
        }
    }
    if root.as_object().is_some_and(|m| m.contains_key(bad_field)) {
        return Some(String::new());
    }
    None
}

fn find_in_node_spec(spec: &Value, spec_ptr: &str, bad_field: &str, is_layout: bool, is_override: bool, is_node_spec: bool) -> Option<String> {
    let obj = spec.as_object()?;
    if is_node_spec && obj.contains_key(bad_field) {
        return Some(spec_ptr.to_string());
    }
    if is_layout && obj.get("layout").and_then(Value::as_object).is_some_and(|m| m.contains_key(bad_field)) {
        return Some(format!("{spec_ptr}/layout"));
    }
    if is_override {
        if let Some(arr) = obj.get("overrides").and_then(Value::as_array) {
            for (i, ov) in arr.iter().enumerate() {
                if ov.as_object().is_some_and(|m| m.contains_key(bad_field)) {
                    return Some(format!("{spec_ptr}/overrides/{i}"));
                }
            }
        }
    }
    if let Some(arr) = obj.get("children").and_then(Value::as_array) {
        for (i, ch) in arr.iter().enumerate() {
            let child_ptr = format!("{spec_ptr}/children/{i}");
            if let Some(found) = find_in_node_spec(ch, &child_ptr, bad_field, is_layout, is_override, is_node_spec) {
                return Some(found);
            }
        }
    }
    if let Some(slots) = obj.get("slots").and_then(Value::as_object) {
        for (slot_name, slot_val) in slots {
            if let Some(arr) = slot_val.as_array() {
                let esc = escape_json_pointer_token(slot_name);
                for (i, ch) in arr.iter().enumerate() {
                    let child_ptr = format!("{spec_ptr}/slots/{esc}/{i}");
                    if let Some(found) = find_in_node_spec(ch, &child_ptr, bad_field, is_layout, is_override, is_node_spec) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn build_unknown_field_corrected_example(
    root: &Value,
    rel_ptr: &str,
    bad_field: &str,
    is_layout: bool,
    is_override: bool,
    is_node_spec: bool,
) -> Option<Value> {
    let mut fixed = root.clone();
    let target = if rel_ptr.is_empty() { Some(&mut fixed) } else { fixed.pointer_mut(rel_ptr) }?;
    let obj = target.as_object_mut()?;
    let removed = obj.remove(bad_field);
    if is_layout && (bad_field == "dir" || bad_field == "flex_direction") && !obj.contains_key("direction") {
        if let Some(v) = removed {
            obj.insert("direction".into(), v);
        }
    } else if (is_override || is_node_spec) && bad_field == "fill_color" && !obj.contains_key("fill") {
        if let Some(v) = removed {
            obj.insert("fill".into(), v);
        }
    } else if is_node_spec && bad_field == "component_name" && !obj.contains_key("component") {
        if let Some(v) = removed {
            obj.insert("component".into(), v);
        }
    }
    if let Some(top) = fixed.as_object_mut() {
        top.remove("path");
        top.remove("output_path");
        if top.is_empty() {
            return None;
        }
    }
    Some(fixed)
}

/// Ensures that any partial `corrected_example` on `err` (such as a single-field patch or a
/// bare `NodeSpec` fragment) is embedded into a clone of `original_args` so resending
/// `err.corrected_example` directly to the same tool succeeds.
pub fn contextualize_corrected_example(err: &mut ToolError, original_args: &Value) {
    if let Some(ref mut cause) = err.cause {
        contextualize_corrected_example(cause, original_args);
    }
    let (Some(Value::Object(ex_map)), Value::Object(orig_map)) = (&err.corrected_example, original_args) else {
        return;
    };
    let ex_map = ex_map.clone();
    if err.path == "/idempotency_key" {
        // A top-level argument: it replaces the key in the call whatever the tool's shape.
        let mut full = original_args.clone();
        if let Value::Object(ref mut m) = full {
            for (k, v) in ex_map {
                m.insert(k, v);
            }
        }
        err.corrected_example = Some(full);
    } else if orig_map.contains_key("spec") {
        if ex_map.contains_key("spec") || ex_map.contains_key("parent_id") {
            let mut full = original_args.clone();
            if let Value::Object(ref mut m) = full {
                for (k, v) in ex_map {
                    m.insert(k, v);
                }
            }
            err.corrected_example = Some(full);
        } else if ex_map.contains_key("kind") {
            let mut full = original_args.clone();
            if (err.path.starts_with("/spec/slots/") || err.path.starts_with("/spec/children/")) && full.pointer(&err.path).is_some() {
                if let Some(slot_item) = full.pointer_mut(&err.path) {
                    *slot_item = Value::Object(ex_map);
                }
            } else {
                full["spec"] = Value::Object(ex_map);
            }
            err.corrected_example = Some(full);
        } else {
            let mut full = original_args.clone();
            if let Some(Value::Object(ref mut spec_obj)) = full.get_mut("spec") {
                for (k, v) in ex_map {
                    spec_obj.insert(k, v);
                }
            }
            err.corrected_example = Some(full);
        }
    } else if orig_map.contains_key("ops") && !ex_map.contains_key("ops") {
        let mut full = original_args.clone();
        let op_idx = err.failed_op.or_else(|| {
            let rest = err.path.strip_prefix("/ops/")?;
            let idx_str = rest.split('/').next()?;
            idx_str.parse::<usize>().ok()
        });
        if let Some(idx) = op_idx {
            if let Some(Value::Object(ref mut op_obj)) = full.get_mut("ops").and_then(Value::as_array_mut).and_then(|a| a.get_mut(idx)) {
                for (k, v) in ex_map {
                    op_obj.insert(k, v);
                }
                err.corrected_example = Some(full);
            }
        }
    } else if err.code != ErrorCode::UnknownField {
        let mut full = original_args.clone();
        if let Value::Object(ref mut m) = full {
            for (k, v) in ex_map {
                m.insert(k, v);
            }
        }
        err.corrected_example = Some(full);
    }
    if err.code != ErrorCode::FileExists && err.code != ErrorCode::UnsavedChanges {
        if let Some(Value::Object(ref mut m)) = err.corrected_example {
            m.remove("path");
            m.remove("output_path");
            if m.is_empty() {
                err.corrected_example = None;
            }
        }
        if let Some(Value::Object(ref mut m)) = err.received {
            m.remove("path");
            m.remove("output_path");
        }
    }
}

/// Shared JSON Schema fragment for the structured error envelope (§3.1, §4).
pub fn error_envelope_schema() -> Value {
    json!({
        "type": "object",
        "required": ["code", "message", "path", "safe_to_retry", "next_step"],
        "properties": {
            "code": { "type": "string" },
            "message": { "type": "string" },
            "path": { "type": "string" },
            "received": {},
            "expected": { "type": "string" },
            "valid_options": {
                "type": "array",
                "items": { "type": "string" },
                "maxItems": MAX_VALID_OPTIONS
            },
            "valid_options_total": { "type": "integer", "minimum": 0 },
            "corrected_example": { "type": "object" },
            "cause": { "type": "object" },
            "failed_op": { "type": "integer", "minimum": 0 },
            "op": { "type": "string" },
            "safe_to_retry": { "type": "boolean" },
            "next_step": { "type": "string" }
        }
    })
}

/// Builds a tool `outputSchema` from `properties` and `required` arrays (§4).
/// Top-level `type` is `"object"` per MCP `2025-11-25` `Tool.outputSchema`, with `properties.error`
/// referencing the shared error envelope and `anyOf` covering success and error shapes.
pub fn tool_output_schema(mut properties: Value, required: &[&str]) -> Value {
    let err_schema = error_envelope_schema();
    if let Value::Object(ref mut props_map) = properties {
        props_map.insert("error".into(), err_schema.clone());
    }
    json!({
        "type": "object",
        "properties": properties,
        "anyOf": [
            { "required": required },
            { "required": ["error"] }
        ]
    })
}

/// Builds a tool `outputSchema` from an existing success object schema (used by `designos_apply_batch`).
pub fn tool_output_schema_from_object(success_schema: Value) -> Value {
    let props = success_schema.get("properties").cloned().unwrap_or_else(|| json!({}));
    let req_vec: Vec<String> = success_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    let req_refs: Vec<&str> = req_vec.iter().map(String::as_str).collect();
    tool_output_schema(props, &req_refs)
}

#[cfg(test)]
mod plain_message_tests {
    use super::*;

    #[test]
    fn a_known_code_prefix_and_retry_marker_become_fields() {
        let e = ToolError::from("NODE_NOT_FOUND: node 7 not found (safe_to_retry: false)".to_string());
        assert_eq!(e.code, ErrorCode::NodeNotFound);
        assert_eq!(e.message, "node 7 not found");
        assert!(!e.safe_to_retry);
    }

    #[test]
    fn an_unknown_prefix_keeps_the_whole_message_as_invalid_arguments() {
        let e = ToolError::from("NOT_A_CATALOGUE_CODE: provide at most one of page_id or node_ids (safe_to_retry: true)");
        assert_eq!(e.code, ErrorCode::InvalidArguments);
        assert_eq!(e.message, "NOT_A_CATALOGUE_CODE: provide at most one of page_id or node_ids");
        assert!(e.safe_to_retry);
    }

    #[test]
    fn codes_owned_by_other_tools_keep_their_own_code() {
        let e = ToolError::from("EXACTLY_ONE_SCOPE: provide at most one of page_id or node_ids (safe_to_retry: true)");
        assert_eq!(e.code, ErrorCode::ExactlyOneScope);
        assert_eq!(e.message, "provide at most one of page_id or node_ids");
        for code in ErrorCode::ALL {
            assert_eq!(ErrorCode::from_code_str(code.as_str()), Some(*code), "{} must round-trip", code.as_str());
        }
    }

    #[test]
    fn a_trailing_valid_options_array_becomes_the_field() {
        let e = ToolError::from("UNKNOWN_PRESET: unknown lint preset 'x'; valid_options: [\"agent\", \"strict\"] (safe_to_retry: true)".to_string());
        assert_eq!(e.code, ErrorCode::UnknownPreset);
        assert_eq!(e.message, "unknown lint preset 'x'");
        assert_eq!(e.valid_options, vec!["agent", "strict"]);
        assert!(e.safe_to_retry);

        let loose = ToolError::from("INVALID_ARGUMENTS: odd valid_options: not an array");
        assert!(loose.valid_options.is_empty());
        assert!(loose.message.contains("valid_options: not an array"));
    }

    #[test]
    fn a_json_envelope_is_parsed_instead_of_embedded() {
        let raw = json!({
            "code": "IMAGE_TOO_LARGE",
            "message": "IMAGE_TOO_LARGE: too big",
            "path": "/scale",
            "valid_options": [{"largest_allowed_scale": 1.5}, "x"],
            "safe_to_retry": true,
            "next_step": "Lower scale."
        })
        .to_string();
        let e = ToolError::from(raw);
        assert_eq!(e.code, ErrorCode::ImageTooLarge);
        assert_eq!(e.message, "too big");
        assert_eq!(e.path, "/scale");
        assert_eq!(e.valid_options, vec!["{\"largest_allowed_scale\":1.5}".to_string(), "x".to_string()]);
        assert_eq!(e.next_step, "Lower scale.");
    }

    #[test]
    fn plain_text_without_a_marker_is_retryable_invalid_arguments() {
        let e = ToolError::from_plain_message("limit must be between 1 and 500");
        assert_eq!(e.code, ErrorCode::InvalidArguments);
        assert_eq!(e.message, "limit must be between 1 and 500");
        assert!(e.safe_to_retry);
    }
}
