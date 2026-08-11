#![deny(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::multiple_unsafe_ops_per_block,
    clippy::undocumented_unsafe_blocks
)]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

mod support;
use support::fixtures::CLIENT_FIXTURES;

fn apply_claude(home: &Path, base_url: &str, token: &str) -> Value {
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            base_url,
            "--token",
            token,
            "--client",
            "claude",
            "--home",
        ])
        .arg(home)
        .args(["--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn dry_run_json_is_structured_and_never_leaks_token() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example/v1/",
            "--token",
            "secret with spaces",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("secret with spaces"));
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["base_url"], "https://gateway.example/v1");
    assert_eq!(value["changes"][0]["action"], "create");
}

#[test]
fn apply_then_rollback_restores_missing_file() {
    let home = tempdir().unwrap();
    let apply = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "http://localhost:8080",
            "--token",
            "rollback-secret",
            "--client",
            "opencode",
            "--model",
            "coder-model",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let apply_json: Value = serde_json::from_slice(&apply).unwrap();
    let id = apply_json["transaction"]["id"].as_str().unwrap();
    let config = home.path().join(".config/opencode/opencode.jsonc");
    assert!(config.exists());
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rewire rollback"))
        .stdout(predicate::str::contains("Restored transaction"));
    assert!(!config.exists());
}

#[test]
fn rollback_without_id_uses_the_latest_transaction_with_yes() {
    let home = tempdir().unwrap();
    let first = apply_claude(home.path(), "https://first-gateway.example", "first-token");
    let first_id = first["transaction"]["id"].as_str().unwrap().to_owned();

    let second = apply_claude(
        home.path(),
        "https://second-gateway.example",
        "second-token",
    );
    let second_id = second["transaction"]["id"].as_str().unwrap().to_owned();

    let rollback = Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rollback: Value = serde_json::from_slice(&rollback).unwrap();
    assert_eq!(rollback["rolled_back"], second_id);
    assert_ne!(rollback["rolled_back"], first_id);

    let config: Value =
        serde_json::from_slice(&fs::read(home.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        config["env"]["ANTHROPIC_BASE_URL"],
        "https://first-gateway.example"
    );

    let backups = Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["backup", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let backups: Value = serde_json::from_slice(&backups).unwrap();
    assert!(
        backups["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == &first_id)
    );
    assert!(
        !backups["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == &second_id)
    );

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", &second_id, "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("was already rolled back"));

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", "../outside", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid transaction identifier"));
}

#[test]
fn rollback_without_id_requires_yes_outside_a_terminal() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "rollback-input",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .success();

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .arg("rollback")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "transaction ID is required outside an interactive terminal",
        ));
}

#[test]
fn rollback_without_id_reports_an_empty_transaction_history() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", "--yes", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no committed transactions are available to roll back",
        ));
}

#[test]
fn rollback_reports_the_owned_field_when_three_way_merge_stops() {
    let home = tempdir().unwrap();
    let applied = apply_claude(
        home.path(),
        "https://gateway.example",
        "rollback-diagnostic",
    );
    let id = applied["transaction"]["id"].as_str().unwrap();
    let config = home.path().join(".claude/settings.json");
    fs::write(
        &config,
        br#"{"env":{"ANTHROPIC_BASE_URL":"https://operator.example","ANTHROPIC_AUTH_TOKEN":"operator-token"}}"#,
    )
    .unwrap();

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", id, "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("adapter-owned field"))
        .stderr(predicate::str::contains("/env/"));
}

#[test]
fn malformed_json_is_a_blocking_error_without_partial_write() {
    let home = tempdir().unwrap();
    let path = home.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{broken").unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway",
            "--token",
            "TOKEN",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse JSON"));
    assert_eq!(fs::read(&path).unwrap(), b"{broken");
}

#[test]
fn dry_run_reports_malformed_configuration_as_a_structured_conflict() {
    let home = tempdir().unwrap();
    let path = home.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{broken").unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "conflict-secret",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["conflicts"][0]["blocking"], true);
    assert!(
        value["conflicts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("parse JSON")
    );
    assert!(!text.contains("conflict-secret"));
    assert_eq!(fs::read(path).unwrap(), b"{broken");
}

#[test]
fn replacing_an_existing_provider_requires_review_then_explicit_yes() {
    let home = tempdir().unwrap();
    let run = |base_url: &str, yes: bool| {
        let mut command = Command::cargo_bin("rewire").unwrap();
        command
            .args([
                "--baseurl",
                base_url,
                "--token",
                "provider-secret",
                "--client",
                "opencode",
                "--model",
                "coder-model",
                "--home",
            ])
            .arg(home.path())
            .arg("--no-color");
        if yes {
            command.arg("--yes");
        }
        command
    };

    run("https://old-gateway.example/v1", true)
        .assert()
        .success();
    let config = home.path().join(".config/opencode/opencode.jsonc");

    run("https://new-gateway.example/v1", false)
        .assert()
        .success()
        .stdout(predicate::str::contains("[REVIEW] opencode"))
        .stdout(predicate::str::contains("different base URL"));
    let before_confirmation = fs::read_to_string(&config).unwrap();
    assert!(before_confirmation.contains("https://old-gateway.example/v1"));
    assert!(!before_confirmation.contains("https://new-gateway.example/v1"));

    run("https://new-gateway.example/v1", true)
        .assert()
        .success();
    let after_confirmation = fs::read_to_string(config).unwrap();
    assert!(after_confirmation.contains("https://new-gateway.example/v1"));
    assert!(!after_confirmation.contains("https://old-gateway.example/v1"));
}

#[test]
fn remove_requires_client_and_explicit_yes_then_supports_rollback() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example/v1",
            "--token",
            "remove-secret",
            "--client",
            "opencode",
            "--model",
            "coder-model",
            "--home",
        ])
        .arg(home.path())
        .arg("--yes")
        .assert()
        .success();
    let config = home.path().join(".config/opencode/opencode.jsonc");

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["remove", "--client", "opencode", "--no-color"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[DELETE] opencode"));
    assert!(fs::read_to_string(&config).unwrap().contains("\"rewire\""));

    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["remove", "--client", "opencode", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    let id = value["transaction"]["id"].as_str().unwrap();
    assert!(!fs::read_to_string(&config).unwrap().contains("\"rewire\""));
    assert!(
        !home
            .path()
            .join(".config/rewire/secrets/opencode-token")
            .exists()
    );

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", id])
        .assert()
        .success();
    assert!(fs::read_to_string(config).unwrap().contains("\"rewire\""));

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .arg("remove")
        .assert()
        .failure()
        .stderr(predicate::str::contains("remove requires --client"));
}

#[test]
fn doctor_defaults_to_human_output_without_credentials() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Rewire doctor\n"))
        .stdout(predicate::str::contains("Home:"))
        .stdout(predicate::str::contains(
            "No supported client configurations",
        ))
        .stdout(predicate::str::starts_with("{").not());
}

#[test]
fn doctor_emits_json_only_when_requested() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["detected"], serde_json::json!([]));
    assert_eq!(value["clients"].as_array().unwrap().len(), 5);
}

#[test]
fn doctor_reports_environment_names_without_values() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .env("ANTHROPIC_AUTH_TOKEN", "doctor-secret")
        .args(["--home"])
        .arg(home.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("doctor-secret"));
    let value: Value = serde_json::from_str(&text).unwrap();
    let claude = value["clients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|client| client["client"] == "claude")
        .unwrap();
    assert!(
        claude["environment"]
            .as_array()
            .unwrap()
            .contains(&Value::String("ANTHROPIC_AUTH_TOKEN".into()))
    );
}

#[test]
fn plan_warns_about_conflicting_claude_environment_without_leaking_values() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .env("ANTHROPIC_BASE_URL", "https://shell-gateway.example")
        .env("ANTHROPIC_AUTH_TOKEN", "shell-secret")
        .env("ANTHROPIC_API_KEY", "shell-api-key")
        .args([
            "--baseurl",
            "https://planned-gateway.example",
            "--token",
            "planned-secret",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    for secret in ["shell-secret", "shell-api-key", "planned-secret"] {
        assert!(!text.contains(secret));
    }
    let value: Value = serde_json::from_str(&text).unwrap();
    let warnings = value["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 3);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("ANTHROPIC_BASE_URL"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("ANTHROPIC_AUTH_TOKEN"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("ANTHROPIC_API_KEY"))
    );
}

#[test]
fn client_location_environment_selects_the_effective_primary_config() {
    let cases = [
        (
            "claude",
            "CLAUDE_CONFIG_DIR",
            "custom/claude",
            "custom/claude/settings.json",
        ),
        (
            "codex",
            "CODEX_HOME",
            "custom/codex",
            "custom/codex/config.toml",
        ),
        (
            "opencode",
            "OPENCODE_CONFIG",
            "custom/opencode.jsonc",
            "custom/opencode.jsonc",
        ),
        (
            "opencode",
            "OPENCODE_CONFIG_DIR",
            "custom/opencode-dir",
            "custom/opencode-dir/opencode.jsonc",
        ),
        (
            "opencode",
            "XDG_CONFIG_HOME",
            "custom/xdg",
            "custom/xdg/opencode/opencode.jsonc",
        ),
        (
            "hermes",
            "HERMES_HOME",
            "custom/hermes",
            "custom/hermes/config.yaml",
        ),
        (
            "openclaw",
            "OPENCLAW_CONFIG_PATH",
            "custom/openclaw.json",
            "custom/openclaw.json",
        ),
        (
            "openclaw",
            "OPENCLAW_STATE_DIR",
            "custom/openclaw-state",
            "custom/openclaw-state/openclaw.json",
        ),
    ];
    for (client, variable, relative_value, relative_expected) in cases {
        let home = tempdir().unwrap();
        let value = home.path().join(relative_value);
        let mut command = Command::cargo_bin("rewire").unwrap();
        for name in [
            "CLAUDE_CONFIG_DIR",
            "CODEX_HOME",
            "OPENCODE_CONFIG",
            "OPENCODE_CONFIG_DIR",
            "XDG_CONFIG_HOME",
            "HERMES_HOME",
            "OPENCLAW_CONFIG_PATH",
            "OPENCLAW_STATE_DIR",
        ] {
            command.env_remove(name);
        }
        command
            .env("HOME", home.path())
            .env(variable, &value)
            .args([
                "--baseurl",
                "https://gateway.example/v1",
                "--token",
                "location-secret",
                "--client",
                client,
                "--home",
            ])
            .arg(home.path());
        if matches!(client, "opencode" | "hermes" | "openclaw") {
            command.args(["--model", "coder-model"]);
        }
        let output = command
            .args(["--dry-run", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let plan: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            plan["changes"][0]["path"],
            home.path()
                .join(relative_expected)
                .to_string_lossy()
                .as_ref(),
            "{client} ignored {variable}"
        );
    }
}

#[test]
fn client_location_environment_cannot_escape_the_selected_home() {
    let home = tempdir().unwrap();
    let external = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .env("HOME", home.path())
        .env("OPENCODE_CONFIG", external.path().join("opencode.jsonc"))
        .args([
            "--baseurl",
            "https://gateway.example/v1",
            "--token",
            "location-secret",
            "--client",
            "opencode",
            "--model",
            "coder-model",
            "--home",
        ])
        .arg(home.path())
        .args(["--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plan: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(plan["conflicts"][0]["blocking"], true);
    assert!(
        plan["conflicts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("outside home")
    );
    assert!(!external.path().join("opencode.jsonc").exists());
}

#[test]
fn plan_defaults_to_numbered_human_output() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "plan-secret",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .arg("plan")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Rewire plan\n"))
        .stdout(predicate::str::contains("1. [CREATE] claude"))
        .stdout(predicate::str::contains("Summary: 1 modification(s)"))
        .stdout(predicate::str::contains("plan-secret").not())
        .stdout(predicate::str::starts_with("{").not());
}

#[test]
fn plan_json_preserves_the_machine_contract() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "plan-json-secret",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["plan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["base_url"], "https://gateway.example");
    assert_eq!(value["changes"][0]["action"], "create");
    assert!(!String::from_utf8_lossy(&output).contains("plan-json-secret"));
}

#[test]
fn backup_list_defaults_to_human_output_and_supports_json() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["backup", "list"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Rewire backups\n"))
        .stdout(predicate::str::contains(
            "No transaction backups were found",
        ));

    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["backup", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["transactions"], serde_json::json!([]));
}

#[test]
fn apply_defaults_to_human_output() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "apply-secret",
            "--client",
            "opencode",
            "--model",
            "coder-model",
            "--home",
        ])
        .arg(home.path())
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Rewire apply\n"))
        .stdout(predicate::str::contains("[WRITTEN]"))
        .stdout(predicate::str::contains("apply-secret").not());
}

#[test]
fn json_mode_formats_runtime_errors_for_automation() {
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args(["plan", "--json"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"], "--baseurl is required");
}

#[test]
fn token_stdin_and_baseurl_environment_are_supported() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .env("REWIRE_BASEURL", "https://env-gateway.example/v1/")
        .args(["--token-stdin", "--client", "claude", "--home"])
        .arg(home.path())
        .args(["--dry-run", "--json"])
        .write_stdin("stdin-secret\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("https://env-gateway.example/v1"))
        .stdout(predicate::str::contains("stdin-secret").not());
}

#[test]
fn model_selection_is_part_of_the_stable_plan_contract() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "model-selection-secret",
            "--client",
            "opencode,openclaw",
            "--model",
            "coder-model",
            "--home",
        ])
        .arg(home.path())
        .args(["plan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["model"], "coder-model");
    assert_eq!(value["sdk"], "@ai-sdk/openai-compatible");
    assert_eq!(value["changes"].as_array().unwrap().len(), 4);
    assert!(!String::from_utf8_lossy(&output).contains("model-selection-secret"));
}

#[test]
fn opencode_cli_accepts_explicit_sdk_and_display_name() {
    let home = tempdir().unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example/v1",
            "--token",
            "sdk-secret",
            "--client",
            "opencode",
            "--model",
            "gpt-5.5",
            "--model-name",
            "GPT-5.5",
            "--sdk",
            "openai",
            "--home",
        ])
        .arg(home.path())
        .args(["plan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("model_name").is_none());
    assert_eq!(value["sdk"], "@ai-sdk/openai");
    assert!(text.contains("@ai-sdk/openai"));
    assert!(value["warnings"].as_array().unwrap().iter().any(|warning| {
        warning
            .as_str()
            .unwrap()
            .contains("native provider manages model display names")
    }));
    assert!(!text.contains("sdk-secret"));
}

#[test]
fn required_model_clients_fail_before_planning_or_writing() {
    for client in ["opencode", "hermes", "openclaw"] {
        let home = tempdir().unwrap();
        Command::cargo_bin("rewire")
            .unwrap()
            .args([
                "--baseurl",
                "https://gateway.example",
                "--token",
                "TOKEN",
                "--client",
                client,
                "--home",
            ])
            .arg(home.path())
            .args(["plan", "--json"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--model is required"));
        assert!(fs::read_dir(home.path()).unwrap().next().is_none());
    }
}

#[test]
fn claude_and_codex_preserve_model_selection_even_when_model_is_supplied() {
    let home = tempdir().unwrap();
    let codex = home.path().join(".codex/config.toml");
    fs::create_dir_all(codex.parent().unwrap()).unwrap();
    fs::write(&codex, "model = \"existing-model\"\n").unwrap();
    let output = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "TOKEN",
            "--client",
            "claude,codex",
            "--model",
            "ignored-model",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert!(value["plan"].get("model").is_none());
    let codex: Value = toml_edit::de::from_slice(&fs::read(codex).unwrap()).unwrap();
    assert_eq!(codex["model"], "existing-model");
    assert!(codex.pointer("/profiles/rewire/model").is_none());
}

#[test]
fn existing_configuration_is_restored_byte_for_byte_after_rollback() {
    let home = tempdir().unwrap();
    let config = home.path().join(".claude/settings.json");
    let original = br#"{
  "custom": {"keep": true},
  "permissions": {"allow": ["git"]}
}
"#;
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, original).unwrap();

    let apply = Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "https://gateway.example",
            "--token",
            "TOKEN",
            "--client",
            "claude",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let transaction_id = serde_json::from_slice::<Value>(&apply).unwrap()["transaction"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--home"])
        .arg(home.path())
        .args(["rollback", &transaction_id, "--json"])
        .assert()
        .success();
    assert_eq!(fs::read(config).unwrap(), original);
}

#[test]
fn invalid_client_and_url_fail_before_creating_transaction_state() {
    let home = tempdir().unwrap();
    Command::cargo_bin("rewire")
        .unwrap()
        .args([
            "--baseurl",
            "ftp://not-supported",
            "--token",
            "TOKEN",
            "--client",
            "unknown",
            "--home",
        ])
        .arg(home.path())
        .args(["--yes", "--json"])
        .assert()
        .failure();
    let transaction_root = rewire::transaction_root(home.path());
    assert!(
        !transaction_root.exists()
            || fs::read_dir(transaction_root)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_type().unwrap().is_dir())
    );
}

#[test]
fn real_client_fixtures_keep_unrelated_configuration() {
    for case in CLIENT_FIXTURES {
        let home = tempdir().unwrap();
        let target_path = home.path().join(case.target);
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(case.fixture),
            &target_path,
        )
        .unwrap();
        let mut command = Command::cargo_bin("rewire").unwrap();
        command
            .args([
                "--baseurl",
                "https://fixture-gateway.example",
                "--token",
                "fixture-secret",
                "--client",
                case.client,
                "--home",
            ])
            .arg(home.path());
        if matches!(case.client, "opencode" | "hermes" | "openclaw") {
            command.args(["--model", "fixture-model"]);
        }
        let output = command
            .args(["--yes", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(!String::from_utf8_lossy(&output).contains("fixture-secret"));
        let content = fs::read_to_string(target_path).unwrap();
        for preserved in case.preserved {
            assert!(
                content.contains(preserved),
                "{} fixture lost {preserved}",
                case.name
            );
        }
        for adapter_marker in case.adapter {
            assert!(
                content.contains(adapter_marker),
                "{} fixture did not receive {adapter_marker}",
                case.name
            );
        }
    }
}

#[test]
fn no_arguments_on_non_tty_fails_fast_instead_of_starting_workflow() {
    Command::cargo_bin("rewire")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("--baseurl is required"));
}

#[test]
fn non_interactive_mode_reports_missing_required_input() {
    Command::cargo_bin("rewire")
        .unwrap()
        .arg("--non-interactive")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--baseurl is required"));
}

#[test]
fn explicit_workflow_rejects_piped_terminal_boundaries() {
    Command::cargo_bin("rewire")
        .unwrap()
        .arg("configure")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "interactive workflow requires terminal stdin and stdout",
        ));
}

#[test]
fn legacy_tui_spelling_routes_to_the_guided_workflow() {
    Command::cargo_bin("rewire")
        .unwrap()
        .arg("tui")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "interactive workflow requires terminal stdin and stdout",
        ));
}

#[test]
fn piped_help_is_readable_without_terminal_color_sequences() {
    Command::cargo_bin("rewire")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(format!(
            "Rewire {}\n",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("configure"))
        .stdout(predicate::str::contains("--model <MODEL>"))
        .stdout(predicate::str::contains("--debug"))
        .stdout(predicate::str::contains(
            "Run the guided client-selection and numbered-review workflow",
        ))
        .stdout(predicate::str::contains("\u{1b}[").not());
}

#[test]
fn long_version_includes_package_commit_and_build_target() {
    Command::cargo_bin("rewire")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(format!(
            "rewire {}\n",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicate::str::contains("commit: "))
        .stdout(predicate::str::contains("target: "));
}

#[test]
fn completion_scripts_come_from_the_current_command_schema() {
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_rewire"))
        .stdout(predicate::str::contains("--baseurl"))
        .stdout(predicate::str::contains("remove"));
}

#[test]
fn no_color_flag_keeps_early_help_output_plain() {
    Command::cargo_bin("rewire")
        .unwrap()
        .args(["--no-color", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("\u{1b}[").not());
}
