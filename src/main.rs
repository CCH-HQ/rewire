#![deny(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::multiple_unsafe_ops_per_block,
    clippy::undocumented_unsafe_blocks
)]

mod cli;
mod commands;

use crate::cli::{Output, input};
use std::process::ExitCode;

fn main() -> ExitCode {
    let json = input::json_requested();
    let color = input::color_requested();
    match commands::run(input::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            Output::error(&error, json, color);
            ExitCode::FAILURE
        }
    }
}
