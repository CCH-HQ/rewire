use crate::cli::Output;
use anyhow::Result;
use rewire::transaction_root;
use std::fs;
use std::path::Path;

/// List transaction directories without exposing journal internals.
pub(super) fn run(home: &Path, output: Output) -> Result<()> {
    let root = transaction_root(home);
    let mut ids = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                ids.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    ids.sort();
    output.backups(&ids)
}
