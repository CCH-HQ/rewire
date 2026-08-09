use crate::model::{Format, Recipe};
use anyhow::{Context, Result, anyhow};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::{Map, Value};

pub fn merge_recipe(recipe: &Recipe, existing: Option<&[u8]>) -> Result<Vec<u8>> {
    // Parsers and writers stay behind this boundary so planners never edit syntax strings.
    if recipe.removal {
        return remove_recipe(recipe, existing);
    }
    match recipe.format {
        Format::Json => merge_json(recipe, existing),
        Format::Yaml => {
            let mut root = existing
                .map(serde_yaml::from_slice)
                .transpose()?
                .unwrap_or(Value::Object(Map::new()));
            deep_merge(&mut root, &recipe.values);
            Ok(serde_yaml::to_string(&root)?.into_bytes())
        }
        Format::Toml => {
            let mut doc = existing
                .map(|b| String::from_utf8(b.to_vec()))
                .transpose()?
                .map(|s| s.parse::<toml_edit::DocumentMut>())
                .transpose()?
                .unwrap_or_default();
            for (section, val) in recipe.values.as_object().expect("recipe object") {
                let item = doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                merge_toml_item(item, val)?;
            }
            Ok(doc.to_string().into_bytes())
        }
        Format::Dotenv => merge_dotenv(recipe, existing),
        Format::Plain => Ok(recipe
            .values
            .as_str()
            .expect("plain recipes contain a string")
            .as_bytes()
            .to_vec()),
    }
}

fn remove_recipe(recipe: &Recipe, existing: Option<&[u8]>) -> Result<Vec<u8>> {
    let existing = existing.unwrap_or_default();
    match recipe.format {
        Format::Json => {
            let source = std::str::from_utf8(if existing.is_empty() { b"{}" } else { existing })?;
            let root = CstRootNode::parse(source, &ParseOptions::default())
                .context("parse JSON/JSONC/JSON5 configuration")?;
            remove_cst_fields(
                &root.object_value_or_set(),
                recipe.values.as_object().expect("removal recipe object"),
            )?;
            Ok(root.to_string().into_bytes())
        }
        Format::Toml => {
            let mut document = std::str::from_utf8(existing)?
                .parse::<toml_edit::DocumentMut>()
                .context("parse TOML configuration")?;
            remove_toml_fields(
                document.as_table_mut(),
                recipe.values.as_object().expect("removal recipe object"),
            )?;
            Ok(document.to_string().into_bytes())
        }
        Format::Yaml => {
            let mut root = if existing.is_empty() {
                Value::Object(Map::new())
            } else {
                serde_yaml::from_slice(existing).context("parse YAML configuration")?
            };
            remove_value_fields(
                root.as_object_mut()
                    .ok_or_else(|| anyhow!("YAML configuration root is not an object"))?,
                recipe.values.as_object().expect("removal recipe object"),
            )?;
            Ok(serde_yaml::to_string(&root)?.into_bytes())
        }
        Format::Dotenv => remove_dotenv_keys(recipe, existing),
        Format::Plain => Ok(Vec::new()),
    }
}

fn remove_cst_fields(target: &CstObject, patch: &Map<String, Value>) -> Result<()> {
    for (key, value) in patch {
        let Some(property) = target.get(key) else {
            continue;
        };
        if let Some(nested) = value.as_object() {
            let object = property
                .value()
                .and_then(|node| node.as_object())
                .ok_or_else(|| anyhow!("JSON removal parent {key} is not an object"))?;
            remove_cst_fields(&object, nested)?;
        } else {
            property.remove();
        }
    }
    Ok(())
}

fn remove_toml_fields(
    target: &mut dyn toml_edit::TableLike,
    patch: &Map<String, Value>,
) -> Result<()> {
    for (key, value) in patch {
        if let Some(nested) = value.as_object() {
            let Some(item) = target.get_mut(key) else {
                continue;
            };
            let table = item
                .as_table_like_mut()
                .ok_or_else(|| anyhow!("TOML removal parent {key} is not a table"))?;
            remove_toml_fields(table, nested)?;
        } else {
            target.remove(key);
        }
    }
    Ok(())
}

fn remove_value_fields(target: &mut Map<String, Value>, patch: &Map<String, Value>) -> Result<()> {
    for (key, value) in patch {
        if let Some(nested) = value.as_object() {
            let Some(value) = target.get_mut(key) else {
                continue;
            };
            let object = value
                .as_object_mut()
                .ok_or_else(|| anyhow!("YAML removal parent {key} is not an object"))?;
            remove_value_fields(object, nested)?;
        } else {
            target.remove(key);
        }
    }
    Ok(())
}

fn remove_dotenv_keys(recipe: &Recipe, existing: &[u8]) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(existing)?;
    let keys = recipe
        .values
        .as_object()
        .expect("dotenv removal recipe object");
    Ok(source
        .split_inclusive('\n')
        .filter(|line| dotenv_key(line).is_none_or(|key| !keys.contains_key(key)))
        .collect::<String>()
        .into_bytes())
}

/// Parse a structured client configuration into a format-neutral value for semantic inspection.
///
/// Dotenv and plain secret targets intentionally return `None`: they do not contain providers.
pub(crate) fn parse_structured(format: Format, bytes: &[u8]) -> Result<Option<Value>> {
    let value = match format {
        Format::Json => {
            let source = std::str::from_utf8(bytes)?;
            let root = CstRootNode::parse(source, &ParseOptions::default())
                .context("parse JSON/JSONC/JSON5 configuration")?;
            root.value().and_then(|node| node.to_serde_value())
        }
        Format::Toml => Some(toml_edit::de::from_slice(bytes).context("parse TOML configuration")?),
        Format::Yaml => Some(serde_yaml::from_slice(bytes).context("parse YAML configuration")?),
        Format::Dotenv | Format::Plain => None,
    };
    Ok(value)
}

/// Reverse only fields changed by a transaction after unrelated user edits.
///
/// The caller has already established that `current` is not byte-identical to `after`.
pub(crate) fn three_way_rollback(
    format: Format,
    before: Option<&[u8]>,
    after: &[u8],
    current: &[u8],
) -> Result<Vec<u8>> {
    match format {
        Format::Dotenv => rollback_dotenv(before.unwrap_or_default(), after, current),
        Format::Plain => Err(anyhow!(
            "adapter-owned secret value changed after transaction"
        )),
        Format::Json | Format::Toml | Format::Yaml => {
            rollback_structured(format, before, after, current)
        }
    }
}

#[derive(Debug)]
struct FieldChange {
    path: Vec<String>,
    before: Option<Value>,
    after: Option<Value>,
}

fn rollback_structured(
    format: Format,
    before: Option<&[u8]>,
    after: &[u8],
    current: &[u8],
) -> Result<Vec<u8>> {
    let empty = Value::Object(Map::new());
    let before_value = before
        .map(|bytes| parse_structured(format, bytes))
        .transpose()?
        .flatten()
        .unwrap_or_else(|| empty.clone());
    let after_value = parse_structured(format, after)?
        .ok_or_else(|| anyhow!("transaction format is not structured"))?;
    let current_value = parse_structured(format, current)?
        .ok_or_else(|| anyhow!("current format is not structured"))?;
    let mut changes = Vec::new();
    collect_field_changes(
        Some(&before_value),
        Some(&after_value),
        &mut Vec::new(),
        &mut changes,
    );
    for change in &changes {
        if value_at(&current_value, &change.path) != change.after.as_ref() {
            return Err(anyhow!(
                "adapter-owned field {} changed after transaction",
                display_path(&change.path)
            ));
        }
    }
    match format {
        Format::Json => rollback_json(current, &changes),
        Format::Toml => rollback_toml(current, &changes),
        Format::Yaml => {
            let mut restored = current_value;
            for change in &changes {
                restore_value_at(&mut restored, &change.path, change.before.as_ref())?;
            }
            Ok(serde_yaml::to_string(&restored)?.into_bytes())
        }
        Format::Dotenv | Format::Plain => unreachable!("handled before structured rollback"),
    }
}

fn collect_field_changes(
    before: Option<&Value>,
    after: Option<&Value>,
    path: &mut Vec<String>,
    changes: &mut Vec<FieldChange>,
) {
    if before == after {
        return;
    }
    match (
        before.and_then(Value::as_object),
        after.and_then(Value::as_object),
    ) {
        (Some(before), Some(after)) => {
            let mut keys = before.keys().chain(after.keys()).collect::<Vec<_>>();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                path.push(key.clone());
                collect_field_changes(before.get(key), after.get(key), path, changes);
                path.pop();
            }
        }
        (None, Some(after)) if before.is_none() && !after.is_empty() => {
            for (key, value) in after {
                path.push(key.clone());
                collect_field_changes(None, Some(value), path, changes);
                path.pop();
            }
        }
        (Some(before), None) if after.is_none() && !before.is_empty() => {
            for (key, value) in before {
                path.push(key.clone());
                collect_field_changes(Some(value), None, path, changes);
                path.pop();
            }
        }
        _ => changes.push(FieldChange {
            path: path.clone(),
            before: before.cloned(),
            after: after.cloned(),
        }),
    }
}

fn value_at<'a>(root: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(root, |value, key| value.get(key))
}

fn display_path(path: &[String]) -> String {
    format!("/{}", path.join("/"))
}

fn restore_value_at(root: &mut Value, path: &[String], before: Option<&Value>) -> Result<()> {
    let (key, parents) = path
        .split_last()
        .ok_or_else(|| anyhow!("transaction attempted to replace the configuration root"))?;
    let mut target = root;
    for parent in parents {
        target = target
            .get_mut(parent)
            .ok_or_else(|| anyhow!("missing rollback parent {parent}"))?;
    }
    let object = target
        .as_object_mut()
        .ok_or_else(|| anyhow!("rollback parent is not an object"))?;
    if let Some(value) = before {
        object.insert(key.clone(), value.clone());
    } else {
        object.remove(key);
    }
    Ok(())
}

fn rollback_json(current: &[u8], changes: &[FieldChange]) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(current)?;
    let root = CstRootNode::parse(source, &ParseOptions::default())
        .context("parse current JSON/JSONC/JSON5 configuration")?;
    for change in changes {
        let (key, parents) = change
            .path
            .split_last()
            .ok_or_else(|| anyhow!("transaction attempted to replace the JSON root"))?;
        let mut object = root
            .object_value()
            .ok_or_else(|| anyhow!("JSON configuration root is not an object"))?;
        for parent in parents {
            object = object
                .object_value(parent)
                .ok_or_else(|| anyhow!("missing JSON rollback parent {parent}"))?;
        }
        let property = object
            .get(key)
            .ok_or_else(|| anyhow!("missing JSON rollback field {key}"))?;
        if let Some(value) = &change.before {
            property.set_value(cst_value(value));
        } else {
            property.remove();
        }
    }
    Ok(root.to_string().into_bytes())
}

fn rollback_toml(current: &[u8], changes: &[FieldChange]) -> Result<Vec<u8>> {
    let mut document = std::str::from_utf8(current)?
        .parse::<toml_edit::DocumentMut>()
        .context("parse current TOML configuration")?;
    for change in changes {
        let (key, parents) = change
            .path
            .split_last()
            .ok_or_else(|| anyhow!("transaction attempted to replace the TOML root"))?;
        let mut table = document.as_table_mut();
        for parent in parents {
            table = table
                .get_mut(parent)
                .and_then(toml_edit::Item::as_table_mut)
                .ok_or_else(|| anyhow!("missing TOML rollback parent {parent}"))?;
        }
        if let Some(value) = &change.before {
            table[key] = toml_value(value)?;
        } else {
            table.remove(key);
        }
    }
    Ok(document.to_string().into_bytes())
}

fn rollback_dotenv(before: &[u8], after: &[u8], current: &[u8]) -> Result<Vec<u8>> {
    let before = std::str::from_utf8(before)?;
    let after = std::str::from_utf8(after)?;
    let current = std::str::from_utf8(current)?;
    let before_lines = dotenv_lines(before, "REWIRE_TOKEN");
    let after_lines = dotenv_lines(after, "REWIRE_TOKEN");
    let current_lines = dotenv_lines(current, "REWIRE_TOKEN");
    if current_lines != after_lines {
        return Err(anyhow!(
            "adapter-owned field /REWIRE_TOKEN changed after transaction"
        ));
    }
    let mut output = String::new();
    let mut restored = false;
    for line in current.split_inclusive('\n') {
        if dotenv_key(line) == Some("REWIRE_TOKEN") {
            if !restored {
                for before_line in &before_lines {
                    output.push_str(before_line);
                }
                restored = true;
            }
        } else {
            output.push_str(line);
        }
    }
    Ok(output.into_bytes())
}

fn dotenv_lines<'a>(source: &'a str, key: &str) -> Vec<&'a str> {
    source
        .split_inclusive('\n')
        .filter(|line| dotenv_key(line) == Some(key))
        .collect()
}

fn merge_dotenv(recipe: &Recipe, existing: Option<&[u8]>) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(existing.unwrap_or_default())?;
    let values = recipe.values.as_object().expect("dotenv recipe object");
    let mut output = source.to_owned();
    for (key, value) in values {
        output = merge_dotenv_key(
            &output,
            key,
            value.as_str().expect("dotenv recipe value is a string"),
        );
    }
    Ok(output.into_bytes())
}

fn merge_dotenv_key(source: &str, key: &str, value: &str) -> String {
    let assignment = format!("{key}={}", quote_dotenv(value));
    let mut output = String::with_capacity(source.len() + assignment.len() + 1);
    let mut replaced = false;
    for line in source.split_inclusive('\n') {
        if dotenv_key(line) == Some(key) {
            if !replaced {
                output.push_str(&assignment);
                output.push_str(if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                });
                replaced = true;
            }
        } else {
            output.push_str(line);
        }
    }
    if !replaced {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&assignment);
        output.push('\n');
    }
    output
}

fn dotenv_key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with('#') || line.is_empty() {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty()
        && key
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric()))
    .then_some(key)
}

fn quote_dotenv(value: &str) -> String {
    // dotenv single quotes keep shell metacharacters literal; close/reopen only for an apostrophe.
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn merge_json(recipe: &Recipe, existing: Option<&[u8]>) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(existing.unwrap_or(b"{}"))?;
    let root = CstRootNode::parse(source, &ParseOptions::default())
        .context("parse JSON/JSONC/JSON5 configuration")?;
    let object = root.object_value_or_set();
    merge_cst_object(&object, recipe.values.as_object().expect("recipe object"));
    Ok(root.to_string().into_bytes())
}

fn merge_cst_object(target: &CstObject, patch: &Map<String, Value>) {
    for (key, value) in patch {
        if let Some(object) = value.as_object() {
            if let Some(property) = target.get(key) {
                if let Some(existing) = property.value().and_then(|node| node.as_object()) {
                    merge_cst_object(&existing, object);
                    continue;
                }
                property.set_value(cst_value(value));
            } else {
                target.append(key, cst_value(value));
            }
        } else if let Some(property) = target.get(key) {
            property.set_value(cst_value(value));
        } else {
            target.append(key, cst_value(value));
        }
    }
}

fn cst_value(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => CstInputValue::Bool(*value),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(value.clone()),
        Value::Array(values) => CstInputValue::Array(values.iter().map(cst_value).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), cst_value(value)))
                .collect(),
        ),
    }
}
fn merge_toml_item(item: &mut toml_edit::Item, value: &Value) -> Result<()> {
    // toml_edit preserves untouched comments and formatting while replacing only owned keys.
    let table = item
        .as_table_mut()
        .ok_or_else(|| anyhow!("TOML target is not a table"))?;
    for (key, val) in value
        .as_object()
        .ok_or_else(|| anyhow!("TOML recipe must be object"))?
    {
        table[key] = toml_value(val)?;
    }
    Ok(())
}
fn toml_value(value: &Value) -> Result<toml_edit::Item> {
    Ok(match value {
        Value::String(v) => toml_edit::value(v.clone()),
        Value::Bool(v) => toml_edit::value(*v),
        Value::Number(v) if v.is_i64() => toml_edit::value(v.as_i64().unwrap()),
        Value::Number(v) if v.is_u64() => toml_edit::value(
            i64::try_from(v.as_u64().expect("number was checked as u64"))
                .context("unsigned integer does not fit in TOML's signed integer range")?,
        ),
        Value::Number(v) => toml_edit::value(v.as_f64().unwrap()),
        Value::Object(map) => {
            let mut table = toml_edit::Table::new();
            for (k, v) in map {
                table[k] = toml_value(v)?;
            }
            toml_edit::Item::Table(table)
        }
        _ => return Err(anyhow!("unsupported TOML value")),
    })
}
fn deep_merge(target: &mut Value, patch: &Value) {
    // Objects merge recursively; scalar and array values are intentionally replaced atomically.
    if let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            match target.get_mut(key) {
                Some(existing) if existing.is_object() && value.is_object() => {
                    deep_merge(existing, value);
                }
                _ => {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }
}
