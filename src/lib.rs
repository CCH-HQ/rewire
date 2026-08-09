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
mod planner;
mod security;
mod transaction;
mod verifier;
mod workflow;

pub use clients::CLIENTS;
pub use diagnostics::diagnose;
pub use model::{
    Action, Client, ClientDiagnostic, Conflict, DoctorReport, Format, Input, Plan, PlannedChange,
    Recipe, RewireError, Secret, Transaction, TransactionEntry,
};
pub use planner::{build_plan, build_remove_plan, detect_clients};
pub use security::validate_base_url;
pub use security::{home_from_override, read_token, redact, stable_json, transaction_root};
pub use transaction::{apply_plan, rollback};
pub use workflow::run as run_workflow;
