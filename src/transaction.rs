use crate::format::three_way_rollback;
use crate::model::{Action, Plan, Transaction, TransactionEntry};
use crate::security::{ensure_safe_path, hash_bytes, transaction_root};
use crate::verifier::verify_recipe;
use anyhow::{Context, Result, anyhow};
use chacha20poly1305::{
    Key, KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Generate},
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

#[derive(Serialize, Deserialize)]
struct JournalManifest {
    id: String,
    entries: Vec<TransactionEntry>,
    committed: bool,
    backup_key: Vec<u8>,
}

struct Journal {
    entries: Vec<TransactionEntry>,
    backup_key: Vec<u8>,
}

struct RollbackAction {
    path: std::path::PathBuf,
    restored: Option<Vec<u8>>,
    mode: Option<u32>,
    current: crate::model::FileSnapshot,
}

/// Apply prepared bytes as one journaled transaction and verify every written configuration.
///
/// # Errors
///
/// Returns an error for blocking conflicts, concurrent edits, lock contention, filesystem failures,
/// or post-write verification failures. Files already replaced are restored before returning.
pub fn apply_plan(home: &Path, plan: &Plan) -> Result<Transaction> {
    reject_blocking_conflicts(plan)?;
    let root = transaction_root(home);
    create_private_dir(&root)?;
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".lock"))?;
    set_private_file_mode(&root.join(".lock"))?;
    lock.try_lock_exclusive()
        .map_err(|_| anyhow!("another Rewire transaction is active"))?;
    let result = apply_locked(home, plan, &root);
    lock.unlock()?;
    result
}

fn reject_blocking_conflicts(plan: &Plan) -> Result<()> {
    if !plan.conflicts.iter().any(|conflict| conflict.blocking) {
        return Ok(());
    }
    let reasons = plan
        .conflicts
        .iter()
        .filter(|conflict| conflict.blocking)
        .map(|conflict| {
            format!(
                "{} ({}): {}",
                conflict.path.display(),
                conflict.client,
                conflict.reason
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(anyhow!("plan contains blocking conflicts: {reasons}"))
}

fn apply_locked(home: &Path, plan: &Plan, root: &Path) -> Result<Transaction> {
    validate_prepared_inputs(home, plan)?;
    let id = transaction_id();
    let dir = root.join(&id);
    create_private_dir(&dir)?;
    let journal = write_journal(plan, &id, &dir)?;

    if let Err(error) = apply_prepared_changes(home, plan, &id, &dir, &journal) {
        if let Err(restore_error) = restore_entries(home, &journal.entries, &journal.backup_key) {
            return Err(anyhow!(
                "{error}; automatic restore also failed: {restore_error}"
            ));
        }
        return Err(error);
    }
    Ok(Transaction {
        id,
        entries: journal.entries,
        committed: true,
    })
}

fn validate_prepared_inputs(home: &Path, plan: &Plan) -> Result<()> {
    // Check every planned input before the first write, so a concurrent edit cannot be overwritten.
    for change in &plan.prepared {
        if matches!(change.action, Action::Noop) {
            continue;
        }
        ensure_safe_path(home, &change.path)?;
        let current = read_snapshot(&change.path)?;
        if current.sha256 != change.before.sha256 || current.exists != change.before.exists {
            return Err(anyhow!("{} changed after planning", change.path.display()));
        }
    }
    Ok(())
}

fn write_journal(plan: &Plan, id: &str, dir: &Path) -> Result<Journal> {
    let mut entries = Vec::new();
    let backup_key = Key::generate().to_vec();
    // Snapshot the whole journal before any target is replaced.
    for (index, change) in plan
        .prepared
        .iter()
        .filter(|change| !matches!(change.action, Action::Noop))
        .enumerate()
    {
        let backup = dir.join(format!("{index:02}-{}.before", change.client.name()));
        let after_backup = dir.join(format!("{index:02}-{}.after", change.client.name()));
        write_private_file(&backup, &encrypt_backup(&change.before.bytes, &backup_key)?)?;
        write_private_file(&after_backup, &encrypt_backup(&change.after, &backup_key)?)?;
        entries.push(TransactionEntry {
            client: change.client,
            path: change.path.clone(),
            before_exists: change.before.exists,
            before_sha256: change.before.sha256.clone(),
            mode: change.before.mode,
            after_sha256: change.after_sha256.clone(),
            after_exists: !matches!(change.action, Action::Delete),
            backup,
            after_backup: Some(after_backup),
            format: Some(change.recipe.format),
        });
    }
    write_private_file(
        dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&JournalManifest {
            id: id.to_owned(),
            entries: entries.clone(),
            committed: false,
            backup_key: backup_key.clone(),
        })?,
    )?;
    Ok(Journal {
        entries,
        backup_key,
    })
}

fn apply_prepared_changes(
    home: &Path,
    plan: &Plan,
    id: &str,
    dir: &Path,
    journal: &Journal,
) -> Result<()> {
    // Apply exact prepared bytes, then verify the bytes that actually reached disk.
    for change in &plan.prepared {
        if matches!(change.action, Action::Noop) {
            continue;
        }
        if matches!(change.action, Action::Delete) {
            fs::remove_file(&change.path)?;
            if change.path.exists() {
                return Err(anyhow!(
                    "deleted target still exists: {}",
                    change.path.display()
                ));
            }
        } else {
            write_atomic(&change.path, &change.after, change.after_mode, home)?;
            verify_recipe(&change.recipe, &fs::read(&change.path)?)?;
        }
    }
    let committed = JournalManifest {
        id: id.to_owned(),
        entries: journal.entries.clone(),
        committed: true,
        backup_key: journal.backup_key.clone(),
    };
    write_private_file(
        dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&committed)?,
    )?;
    Ok(())
}

fn restore_entries(home: &Path, entries: &[TransactionEntry], backup_key: &[u8]) -> Result<()> {
    // A later failure must restore files already replaced in this transaction.
    for entry in entries.iter().rev() {
        let before = decrypt_backup(&fs::read(&entry.backup)?, backup_key)?;
        if entry.before_exists {
            write_atomic(&entry.path, &before, entry.mode, home)?;
        } else {
            let _ = fs::remove_file(&entry.path);
        }
    }
    Ok(())
}

/// Restore a committed transaction with a field-level three-way merge after unrelated edits.
///
/// # Errors
///
/// Returns an error when the journal is invalid, an adapter-owned field changed, or restoration
/// fails. Every target is checked before the first write.
pub fn rollback(home: &Path, id: &str) -> Result<()> {
    let dir = transaction_root(home).join(id);
    let tx: JournalManifest = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
    let mut actions = Vec::new();
    for entry in tx.entries.iter().rev() {
        ensure_safe_path(home, &entry.path)?;
        let current = read_snapshot(&entry.path)?;
        let before = decrypt_backup(&fs::read(&entry.backup)?, &tx.backup_key)?;
        let matches_after = current.exists == entry.after_exists
            && (!entry.after_exists
                || current.sha256.as_deref() == Some(entry.after_sha256.as_str()));
        let restored = if matches_after {
            entry.before_exists.then_some(before)
        } else {
            if !current.exists {
                return Err(anyhow!(
                    "{} was removed after transaction",
                    entry.path.display()
                ));
            }
            let after_backup = entry.after_backup.as_ref().ok_or_else(|| {
                anyhow!(
                    "{} uses a legacy journal without an after snapshot",
                    entry.path.display()
                )
            })?;
            let format = entry.format.ok_or_else(|| {
                anyhow!(
                    "{} uses a legacy journal without format metadata",
                    entry.path.display()
                )
            })?;
            let after = decrypt_backup(&fs::read(after_backup)?, &tx.backup_key)?;
            Some(
                three_way_rollback(
                    format,
                    entry.before_exists.then_some(before.as_slice()),
                    &after,
                    &current.bytes,
                )
                .with_context(|| format!("three-way rollback {}", entry.path.display()))?,
            )
        };
        actions.push(RollbackAction {
            path: entry.path.clone(),
            restored,
            mode: entry.mode,
            current,
        });
    }
    apply_rollback_actions(home, &actions)
}

fn apply_rollback_actions(home: &Path, actions: &[RollbackAction]) -> Result<()> {
    for (index, action) in actions.iter().enumerate() {
        let result = if let Some(bytes) = &action.restored {
            write_atomic(&action.path, bytes, action.mode, home)
        } else {
            fs::remove_file(&action.path).map_err(Into::into)
        };
        if let Err(error) = result {
            let mut restore_error = None;
            for applied in actions[..=index].iter().rev() {
                if let Err(error) = restore_snapshot(home, &applied.path, &applied.current) {
                    restore_error = Some(error);
                    break;
                }
            }
            return if let Some(restore_error) = restore_error {
                Err(anyhow!(
                    "rollback failed: {error}; restoring current files also failed: {restore_error}"
                ))
            } else {
                Err(error)
            };
        }
    }
    Ok(())
}

fn restore_snapshot(home: &Path, path: &Path, snapshot: &crate::model::FileSnapshot) -> Result<()> {
    if snapshot.exists {
        write_atomic(path, &snapshot.bytes, snapshot.mode, home)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn read_snapshot(path: &Path) -> Result<crate::model::FileSnapshot> {
    let metadata = path.metadata().ok();
    let exists = path.exists();
    let bytes = if exists { fs::read(path)? } else { Vec::new() };
    Ok(crate::model::FileSnapshot {
        exists,
        sha256: exists.then(|| hash_bytes(&bytes)),
        bytes,
        mode: file_mode(metadata.as_ref()),
    })
}

fn write_atomic(path: &Path, bytes: &[u8], mode: Option<u32>, home: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temp = NamedTempFile::new_in(path.parent().unwrap_or(home))?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| anyhow!("atomic replace {}: {}", path.display(), e))?;
    if let Some(mode) = mode {
        set_file_mode(path, mode)?;
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<()> {
    create_private_dir_all(path)?;
    set_directory_mode(path)
}

fn write_private_file(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    let path = path.as_ref();
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    set_private_file_mode(path)
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700).create(path)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

fn encrypt_backup(bytes: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow!("invalid transaction backup key"))?;
    let nonce = XNonce::generate();
    let encrypted = cipher
        .encrypt(&nonce, bytes)
        .map_err(|_| anyhow!("encrypt transaction backup"))?;
    let mut output = Vec::with_capacity(nonce.len() + encrypted.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);
    Ok(output)
}

fn decrypt_backup(bytes: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let (nonce, encrypted) = bytes
        .split_at_checked(24)
        .ok_or_else(|| anyhow!("invalid encrypted transaction backup"))?;
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow!("invalid transaction backup key"))?;
    let nonce = XNonce::try_from(nonce).map_err(|_| anyhow!("invalid transaction backup nonce"))?;
    cipher
        .decrypt(&nonce, encrypted)
        .map_err(|_| anyhow!("decrypt transaction backup"))
}

#[cfg(unix)]
fn set_directory_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_directory_mode(_: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_mode(_: &Path) -> Result<()> {
    Ok(())
}

fn file_mode(metadata: Option<&std::fs::Metadata>) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.map(|metadata| metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode(_: &Path, _: u32) -> Result<()> {
    Ok(())
}
fn transaction_id() -> String {
    format!(
        "tx-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
