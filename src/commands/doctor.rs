use crate::cli::Output;
use anyhow::Result;
use rewire::diagnose;
use std::path::Path;

/// Detect client files and delegate representation choice to the shared output layer.
pub(super) fn run(home: &Path, output: Output) -> Result<()> {
    output.doctor(&diagnose(home))
}
