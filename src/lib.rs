//! Library to interface with the [Pterodactyl API](https://dashflo.net/docs/api/pterodactyl/v1/)

// BEGIN - Embark standard lints v6 for Rust 1.55+
// do not change or add/remove here, but one can add exceptions after this section
// for more info see: <https://github.com/EmbarkStudios/rust-ecosystem/issues/59>
#![deny(unsafe_code)]
#![warn(
    clippy::all,
    clippy::await_holding_lock,
    clippy::char_lit_as_u8,
    clippy::checked_conversions,
    clippy::dbg_macro,
    clippy::debug_assert_with_mut_call,
    clippy::doc_markdown,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::exit,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_deref_methods,
    clippy::explicit_into_iter_loop,
    clippy::fallible_impl_from,
    clippy::filter_map_next,
    clippy::flat_map_option,
    clippy::float_cmp_const,
    clippy::fn_params_excessive_bools,
    clippy::from_iter_instead_of_collect,
    clippy::if_let_mutex,
    clippy::implicit_clone,
    clippy::imprecise_flops,
    clippy::inefficient_to_string,
    clippy::invalid_upcast_comparisons,
    clippy::large_digit_groups,
    clippy::large_stack_arrays,
    clippy::large_types_passed_by_value,
    clippy::let_unit_value,
    clippy::linkedlist,
    clippy::lossy_float_literal,
    clippy::macro_use_imports,
    clippy::manual_ok_or,
    clippy::map_err_ignore,
    clippy::map_flatten,
    clippy::map_unwrap_or,
    clippy::match_on_vec_items,
    clippy::match_same_arms,
    clippy::match_wild_err_arm,
    clippy::match_wildcard_for_single_variants,
    clippy::mem_forget,
    clippy::mismatched_target_os,
    clippy::missing_enforced_import_renames,
    clippy::mut_mut,
    clippy::mutex_integer,
    clippy::needless_borrow,
    clippy::needless_continue,
    clippy::needless_for_each,
    clippy::option_option,
    clippy::path_buf_push_overwrite,
    clippy::ptr_as_ptr,
    clippy::rc_mutex,
    clippy::ref_option_ref,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_functions_in_if_condition,
    clippy::semicolon_if_nothing_returned,
    clippy::single_match_else,
    clippy::string_add_assign,
    clippy::string_add,
    clippy::string_lit_as_bytes,
    clippy::string_to_string,
    clippy::todo,
    clippy::trait_duplication_in_bounds,
    clippy::unimplemented,
    clippy::unnested_or_patterns,
    clippy::unused_self,
    clippy::useless_transmute,
    clippy::verbose_file_reads,
    clippy::zero_sized_map_values,
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms
)]
// END - Embark standard lints v6 for Rust 1.55+
// crate-specific exceptions:
// #![allow()]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

pub mod client;
mod http;
mod structs;

use reqwest::StatusCode;

/// The result type for errors produced by this crate
pub type Result<T> = core::result::Result<T, Error>;

/// Errors produced by this crate
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// HTTP request errors
    #[error("HTTP Request Error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// Json errors
    #[error("Json Error: {0}")]
    Json(#[from] serde_json::Error),

    /// Miscellaneous HTTP status codes
    #[error("Http Status Code: {0}")]
    Http(StatusCode),

    /// Websocket errors
    #[cfg(feature = "websocket")]
    #[error("WebSocket Error: {0}")]
    Websocket(#[from] async_tungstenite::tungstenite::Error),

    /// Received an unexpected message from the websocket
    #[cfg(feature = "websocket")]
    #[error("Unexpected Message")]
    UnexpectedMessage,

    /// The websocket token expired
    #[cfg(feature = "websocket")]
    #[error("WebSocket Token Expired")]
    WebsocketTokenExpired,

    /// Unable to perform operation due to lack of permissions
    #[error("Permission Error")]
    PermissionError,

    /// Rate limit reached
    #[error("Rate Limit")]
    RateLimit,

    /// 2fa token invalid
    #[error("Invalid 2fa Token")]
    Invalid2faToken,

    /// Incorrect password
    #[error("Incorrect Password")]
    IncorrectPassword,

    /// Invalid email
    #[error("Invalid Email")]
    InvalidEmail,

    /// The requested resource was not found
    #[error("Resource Not Found")]
    ResourceNotFound,

    /// Unable to delete the primary network allocation
    #[error("Primary Allocation")]
    PrimaryAllocation,
}
