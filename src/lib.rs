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
mod model_discovery;
mod planner;
mod security;
mod transaction;
mod verifier;
mod workflow;

pub use clients::CLIENTS;
pub use diagnostics::diagnose;
pub use model::{
    Action, Client, ClientDiagnostic, Conflict, DoctorReport, Format, Input, ModelConfig,
    OpenCodeSdk, Plan, PlannedChange, Recipe, RewireError, Secret, Transaction, TransactionEntry,
    validate_model_id, validate_model_name,
};
pub use model_catalog::{ModelPreset, POPULAR_MODELS, find_model, popular_models};
pub use model_discovery::{
    DiscoveredModel, DiscoveryDiagnostic, DiscoveryFailure, DiscoveryOptions, DiscoveryReport,
    ModelApi, discover_models, discover_models_with_options, models_endpoint,
    models_endpoint_candidates, parse_models_response,
};
pub use planner::{build_plan, build_plan_with_catalog, build_remove_plan, detect_clients};
pub use security::validate_base_url;
pub use security::{home_from_override, read_token, redact, stable_json, transaction_root};
pub use transaction::available_transactions;
pub use transaction::{apply_plan, rollback};
pub use workflow::{run as run_workflow, run_with_debug as run_workflow_with_debug};
