#![deny(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::multiple_unsafe_ops_per_block,
    clippy::undocumented_unsafe_blocks
)]
//! Rewire core: client recipes, planning, structured formats, and transactions.

mod clients;
mod diagnostics;
mod format;
mod model;
mod model_catalog;
mod planner;
mod security;
mod transaction;
mod verifier;
mod workflow;

pub use clients::CLIENTS;
pub use diagnostics::diagnose;
pub use model::{
    Action, Client, ClientDiagnostic, Conflict, DoctorReport, Format, Input, OpenCodeSdk, Plan,
    PlannedChange, Recipe, RewireError, Secret, Transaction, TransactionEntry, validate_model_id,
    validate_model_name,
};
pub use model_catalog::{ModelPreset, POPULAR_MODELS, find_model, popular_models};
pub use planner::{build_plan, build_remove_plan, detect_clients};
pub use security::validate_base_url;
pub use security::{home_from_override, read_token, redact, stable_json, transaction_root};
pub use transaction::available_transactions;
pub use transaction::{apply_plan, rollback};
pub use workflow::run as run_workflow;
