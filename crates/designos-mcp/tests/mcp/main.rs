//! One integration-test binary for the crate: one module per former test file.
//! The files that change process-wide state (the library root in the environment, the file-size
//! limit) keep their own process beside this directory, as do the two directory binaries.

#[path = "../support/mod.rs"]
mod support;

mod bound_variant_hug_host;
mod button_label_centre;
mod close_document;
mod color_profile_screenshots;
mod colour_detach;
mod colour_var_refs;
mod component_property_variables;
mod connector_both_ends_deleted;
mod connector_ink_on_image_card;
mod docs_stdio;
mod fill_vector_layout_via_resize;
mod get_node_omits_saved_glyphs;
mod get_variables_subscribed_collection;
mod hug_text_relayout;
mod instance_child_delete;
mod library_variable_writes;
mod list_lint_rules;
mod live_doc_epoch;
mod live_session_guards;
mod mcp_overwrite_metadata;
mod mcp_paths;
mod native_write_pending_pages;
mod nested_bound_variant;
mod resized_line_via_update;
mod resized_vector_via_update;
mod session;
mod set_slot_batch;
mod spec_errors;
mod spec_text_colour;
mod stdio;
mod table_batch;
mod table_read;
mod text_fill;
mod text_style_without_meta;
mod text_truncation_roundtrip;
mod tools_snapshot;
mod version;
mod wrapped_fill_text_height;
