#![deny(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::multiple_unsafe_ops_per_block,
    clippy::undocumented_unsafe_blocks
)]

use rewire::*;
use serde_json::Value;
use std::{collections::HashMap, fs};
use tempfile::tempdir;

#[test]
fn validates_and_normalizes_base_url_without_forcing_version_path() {
    assert_eq!(
        validate_base_url("https://api.example.com/v1/").unwrap(),
        "https://api.example.com/v1"
    );
    assert!(validate_base_url("ftp://example.com").is_err());
    assert!(validate_base_url("https://").is_err());
    assert!(validate_base_url("https://user:secret@example.com").is_err());
    assert!(validate_base_url("https://example.com?v=1").is_err());
    assert!(validate_base_url("https://example.com/#fragment").is_err());
    assert!(validate_base_url("https://example.com:invalid").is_err());
    assert_eq!(
        validate_base_url("http://[::1]:9000/v1/").unwrap(),
        "http://[::1]:9000/v1"
    );
}

#[test]
fn merge_preserves_unknown_json_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"permissions":{"allow":["git"]},"custom":42}"#).unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("s3cr3t").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    apply_plan(dir.path(), &plan).unwrap();
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(value["custom"], 42);
    assert_eq!(value["env"]["ANTHROPIC_BASE_URL"], "https://gateway.local");
    assert_eq!(value["env"]["ANTHROPIC_AUTH_TOKEN"], "s3cr3t");
}

#[test]
fn codex_and_opencode_recipes_follow_current_official_provider_shapes() {
    let home = std::path::Path::new("/fixture-home");
    let endpoint = "https://gateway.example/v1";

    let codex = Client::Codex.recipes(home, endpoint, "TOKEN", Some("coder-model"))[0]
        .values
        .clone();
    assert_eq!(
        codex.pointer("/model_providers/rewire/base_url").unwrap(),
        endpoint
    );
    assert_eq!(
        codex
            .pointer("/model_providers/rewire/experimental_bearer_token")
            .unwrap(),
        "TOKEN"
    );
    assert_eq!(
        codex.pointer("/model_providers/rewire/wire_api").unwrap(),
        "responses"
    );
    assert_eq!(
        codex
            .pointer("/model_providers/rewire/requires_openai_auth")
            .unwrap(),
        false
    );
    assert_eq!(
        codex.pointer("/profiles/rewire/model_provider").unwrap(),
        "rewire"
    );
    assert_eq!(
        codex.pointer("/profiles/rewire/model").unwrap(),
        "coder-model"
    );

    let opencode_recipes = Client::OpenCode.recipes(home, endpoint, "TOKEN", Some("coder-model"));
    let opencode_recipe = &opencode_recipes[0];
    assert_eq!(
        opencode_recipe.path,
        home.join(".config/opencode/opencode.jsonc")
    );
    let opencode = &opencode_recipe.values;
    assert_eq!(
        opencode.pointer("/provider/rewire/npm").unwrap(),
        "@ai-sdk/openai-compatible"
    );
    assert_eq!(
        opencode.pointer("/provider/rewire/options/apiKey").unwrap(),
        "{file:/fixture-home/.config/rewire/secrets/opencode-token}"
    );
    assert_eq!(
        opencode_recipes[1].path,
        home.join(".config/rewire/secrets/opencode-token")
    );
    assert_eq!(
        opencode
            .pointer("/provider/rewire/models/coder-model/name")
            .unwrap(),
        "coder-model"
    );
}

#[test]
fn hermes_and_openclaw_recipes_follow_current_official_provider_shapes() {
    let home = std::path::Path::new("/fixture-home");
    let endpoint = "https://gateway.example/v1";
    let hermes_recipes = Client::Hermes.recipes(home, endpoint, "TOKEN", Some("coder-model"));
    let hermes = &hermes_recipes[0].values;
    assert_eq!(hermes.pointer("/providers/rewire/api").unwrap(), endpoint);
    assert_eq!(
        hermes.pointer("/providers/rewire/key_env").unwrap(),
        "REWIRE_TOKEN"
    );
    assert_eq!(
        hermes.pointer("/providers/rewire/transport").unwrap(),
        "chat_completions"
    );
    assert_eq!(
        hermes.pointer("/providers/rewire/default_model").unwrap(),
        "coder-model"
    );
    assert_eq!(hermes_recipes[1].path, home.join(".hermes/.env"));

    let openclaw_recipes = Client::OpenClaw.recipes(home, endpoint, "TOKEN", Some("coder-model"));
    let openclaw = &openclaw_recipes[0].values;
    assert_eq!(
        openclaw
            .pointer("/models/providers/rewire/baseUrl")
            .unwrap(),
        endpoint
    );
    assert_eq!(
        openclaw
            .pointer("/models/providers/rewire/apiKey/source")
            .unwrap(),
        "file"
    );
    assert_eq!(
        openclaw.pointer("/secrets/providers/rewire/mode").unwrap(),
        "singleValue"
    );
    assert_eq!(
        openclaw
            .pointer("/models/providers/rewire/models/0/id")
            .unwrap(),
        "coder-model"
    );
    assert_eq!(
        openclaw_recipes[1].path,
        home.join(".openclaw/secrets/rewire-token")
    );
    assert!(openclaw.pointer("/providers/rewire").is_none());
}

#[test]
fn opencode_recipe_matches_native_global_config_precedence() {
    let home = tempdir().unwrap();
    let config_dir = home.path().join(".config/opencode");
    fs::create_dir_all(&config_dir).unwrap();

    let fresh = Client::OpenCode.recipes(home.path(), "https://gateway", "TOKEN", None);
    assert_eq!(fresh[0].path, config_dir.join("opencode.jsonc"));

    fs::write(config_dir.join("opencode.json"), "{}").unwrap();
    let json = Client::OpenCode.recipes(home.path(), "https://gateway", "TOKEN", None);
    assert_eq!(json[0].path, config_dir.join("opencode.json"));

    fs::write(config_dir.join("opencode.jsonc"), "{}").unwrap();
    let jsonc = Client::OpenCode.recipes(home.path(), "https://gateway", "TOKEN", None);
    assert_eq!(jsonc[0].path, config_dir.join("opencode.jsonc"));
}

#[test]
fn rollback_refuses_when_user_changed_owned_file() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "http://localhost:9000".into(),
        token: Secret::new("token").unwrap(),
        clients: vec![Client::OpenCode],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    let path = Client::OpenCode.recipes(dir.path(), &plan.base_url, "token", None)[0]
        .path
        .clone();
    fs::write(&path, br#"{"manual":true}"#).unwrap();
    assert!(rollback(dir.path(), &tx.id).is_err());
    assert_eq!(fs::read(&path).unwrap(), br#"{"manual":true}"#);
    assert!(
        dir.path()
            .join(".config/rewire/secrets/opencode-token")
            .exists()
    );
}

#[test]
fn three_way_jsonc_rollback_preserves_later_unrelated_fields_and_comments() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "{\n  // original operator comment\n  \"theme\": \"dark\",\n}\n",
    )
    .unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("coder-model".into()),
    };
    let tx = apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();
    let applied = fs::read_to_string(&config).unwrap();
    let edited = applied.replacen('{', "{\n  // added after apply\n  \"manual\": true,", 1);
    fs::write(&config, edited).unwrap();

    rollback(dir.path(), &tx.id).unwrap();
    let restored = fs::read_to_string(&config).unwrap();
    assert!(restored.contains("// original operator comment"));
    assert!(restored.contains("// added after apply"));
    assert!(restored.contains("\"manual\": true"));
    assert!(restored.contains("\"theme\": \"dark\""));
    assert!(!restored.contains("https://gateway.example/v1"));
    assert!(!restored.contains("{file:"));
    assert!(
        !dir.path()
            .join(".config/rewire/secrets/opencode-token")
            .exists()
    );
}

#[test]
fn three_way_toml_rollback_preserves_later_unrelated_table() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "[history]\nsave_history = true\n").unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Codex],
        model: Some("coder-model".into()),
    };
    let tx = apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();
    let mut edited = fs::read_to_string(&config).unwrap();
    edited.push_str("\n[manual]\nkeep = true\n");
    fs::write(&config, edited).unwrap();

    rollback(dir.path(), &tx.id).unwrap();
    let restored = fs::read_to_string(config).unwrap();
    let parsed: Value = toml_edit::de::from_str(&restored).unwrap();
    assert_eq!(parsed.pointer("/history/save_history").unwrap(), true);
    assert_eq!(parsed.pointer("/manual/keep").unwrap(), true);
    assert!(parsed.pointer("/model_providers/rewire/base_url").is_none());
    assert!(parsed.pointer("/profiles/rewire/model_provider").is_none());
    assert!(!restored.contains("secret"));
}

#[test]
fn three_way_hermes_rollback_preserves_yaml_and_dotenv_edits() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".hermes/config.yaml");
    let dotenv = dir.path().join(".hermes/.env");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "features:\n  audit: true\n").unwrap();
    fs::write(&dotenv, "EXISTING=before\nREWIRE_TOKEN=old-token\n").unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("new-token").unwrap(),
        clients: vec![Client::Hermes],
        model: Some("coder-model".into()),
    };
    let tx = apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();
    let mut yaml_edit = fs::read_to_string(&config).unwrap();
    yaml_edit.push_str("manual: true\n");
    fs::write(&config, yaml_edit).unwrap();
    let mut dotenv_edit = fs::read_to_string(&dotenv).unwrap();
    dotenv_edit.push_str("LATER=value\n");
    fs::write(&dotenv, dotenv_edit).unwrap();

    rollback(dir.path(), &tx.id).unwrap();
    let restored_yaml: Value = serde_yaml::from_slice(&fs::read(config).unwrap()).unwrap();
    assert_eq!(restored_yaml.pointer("/features/audit").unwrap(), true);
    assert_eq!(restored_yaml.pointer("/manual").unwrap(), true);
    assert!(restored_yaml.pointer("/providers/rewire/api").is_none());
    let restored_env = fs::read_to_string(dotenv).unwrap();
    assert!(restored_env.contains("EXISTING=before\n"));
    assert!(restored_env.contains("REWIRE_TOKEN=old-token\n"));
    assert!(restored_env.contains("LATER=value\n"));
    assert!(!restored_env.contains("new-token"));
}

#[test]
fn token_never_appears_in_redacted_output() {
    assert_eq!(redact("url=TOKEN", "TOKEN"), "url=[REDACTED_TOKEN]");
}

#[test]
fn json5_comments_and_unknown_fields_survive_structured_merge() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{\n  // keep this operator note\n  custom: 7,\n}").unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    apply_plan(dir.path(), &plan).unwrap();
    let output = fs::read_to_string(path).unwrap();
    assert!(output.contains("  // keep this operator note\n"));
    assert!(output.contains("custom: 7"));
    assert!(output.contains("\"rewire\""));
}

#[cfg(unix)]
#[test]
fn symlinked_configuration_target_is_rejected_before_write() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let external = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(external.path().join("settings.json"), b"{}").unwrap();
    symlink(external.path().join("settings.json"), &path).unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert!(plan.conflicts[0].blocking);
    assert!(apply_plan(dir.path(), &plan).is_err());
    assert_eq!(
        fs::read(external.path().join("settings.json")).unwrap(),
        b"{}"
    );
}

#[test]
fn malformed_configuration_is_reported_as_a_blocking_plan_conflict() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{broken").unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert!(plan.changes.is_empty());
    assert_eq!(plan.conflicts.len(), 1);
    assert!(plan.conflicts[0].blocking);
    assert!(plan.conflicts[0].reason.contains("parse JSON"));
    assert_eq!(fs::read(path).unwrap(), b"{broken");
}

#[test]
fn read_only_configuration_is_reported_before_apply() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{}").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].reason, "target is read-only");
}

#[test]
fn all_client_recipes_write_parseable_configurations() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "http://[::1]:9000/gateway".into(),
        token: Secret::new("secret").unwrap(),
        clients: CLIENTS.to_vec(),
        model: Some("coder-model".into()),
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    assert_eq!(tx.entries.len(), 8);
    for client in CLIENTS {
        for recipe in client.recipes(dir.path(), &plan.base_url, "secret", Some("coder-model")) {
            let bytes = fs::read(&recipe.path).unwrap();
            match recipe.format {
                Format::Json => {
                    serde_json::from_slice::<Value>(&bytes).unwrap();
                }
                Format::Toml => {
                    String::from_utf8(bytes)
                        .unwrap()
                        .parse::<toml_edit::DocumentMut>()
                        .unwrap();
                }
                Format::Yaml => {
                    serde_yaml::from_slice::<Value>(&bytes).unwrap();
                }
                Format::Dotenv => assert!(
                    String::from_utf8(bytes)
                        .unwrap()
                        .contains("REWIRE_TOKEN='secret'")
                ),
                Format::Plain => assert_eq!(bytes, b"secret"),
            }
        }
    }
}

#[test]
fn applying_same_plan_again_is_a_noop() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Codex],
        model: None,
    };
    let first = build_plan(dir.path(), &input).unwrap();
    apply_plan(dir.path(), &first).unwrap();
    let second = build_plan(dir.path(), &input).unwrap();
    assert!(matches!(second.changes[0].action, Action::Noop));
    assert!(apply_plan(dir.path(), &second).unwrap().entries.is_empty());
}

#[test]
fn existing_rewire_providers_with_the_same_url_are_idempotent_for_every_adapter() {
    for client in [
        Client::Codex,
        Client::OpenCode,
        Client::Hermes,
        Client::OpenClaw,
    ] {
        let dir = tempdir().unwrap();
        let initial = Input {
            base_url: "https://gateway.example/v1".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
        };
        apply_plan(dir.path(), &build_plan(dir.path(), &initial).unwrap()).unwrap();

        // A cosmetic trailing slash normalizes to the same endpoint and must not request review.
        let repeated = Input {
            base_url: "https://gateway.example/v1/".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
        };
        let plan = build_plan(dir.path(), &repeated).unwrap();
        assert!(
            plan.conflicts.is_empty(),
            "{client} reported a false conflict"
        );
        assert!(
            plan.changes
                .iter()
                .all(|change| matches!(change.action, Action::Noop)),
            "{client} was not idempotent"
        );
    }
}

#[test]
fn existing_rewire_providers_with_a_different_url_require_review_for_every_adapter() {
    for client in [
        Client::Codex,
        Client::OpenCode,
        Client::Hermes,
        Client::OpenClaw,
    ] {
        let dir = tempdir().unwrap();
        let initial = Input {
            base_url: "https://old-gateway.example/v1".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
        };
        apply_plan(dir.path(), &build_plan(dir.path(), &initial).unwrap()).unwrap();

        let replacement = Input {
            base_url: "https://new-gateway.example/v1".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
        };
        let plan = build_plan(dir.path(), &replacement).unwrap();
        assert_eq!(plan.conflicts.len(), 1, "{client} review count");
        assert!(!plan.conflicts[0].blocking, "{client} review was blocking");
        assert!(
            plan.conflicts[0].reason.contains("different base URL"),
            "{client} review reason was not actionable"
        );
        assert!(
            plan.changes
                .iter()
                .any(|change| matches!(change.action, Action::Merge)),
            "{client} did not retain the prepared replacement"
        );
    }
}

#[test]
fn apply_rejects_a_file_changed_after_planning() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"operator":true}"#).unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    fs::write(&path, br#"{"operator":false,"edited":true}"#).unwrap();
    assert!(apply_plan(dir.path(), &plan).is_err());
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        r#"{"operator":false,"edited":true}"#
    );
}

#[cfg(unix)]
#[test]
fn apply_and_rollback_preserve_unix_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{}\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    rollback(dir.path(), &tx.id).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn later_write_failure_restores_earlier_replacements() {
    let dir = tempdir().unwrap();
    let blocker = dir.path().join(".codex");
    fs::write(&blocker, b"directory blocker").unwrap();
    let claude_path = dir.path().join(".claude/settings.json");
    let input = Input {
        base_url: "https://gateway.local".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Claude, Client::Codex],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert!(apply_plan(dir.path(), &plan).is_err());
    assert!(!claude_path.exists());
    assert_eq!(fs::read(&blocker).unwrap(), b"directory blocker");
}

#[test]
fn credential_files_make_environment_references_persistent() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("persistent-token").unwrap(),
        clients: vec![Client::OpenCode, Client::Hermes, Client::OpenClaw],
        model: Some("coder-model".into()),
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    assert_eq!(tx.entries.len(), 6);

    let opencode_secret = dir.path().join(".config/rewire/secrets/opencode-token");
    assert_eq!(fs::read(&opencode_secret).unwrap(), b"persistent-token");
    let opencode = fs::read_to_string(dir.path().join(".config/opencode/opencode.jsonc")).unwrap();
    assert!(opencode.contains(&format!("{{file:{}}}", opencode_secret.display())));

    let hermes = fs::read_to_string(dir.path().join(".hermes/.env")).unwrap();
    assert!(hermes.contains("REWIRE_TOKEN='persistent-token'"));

    let openclaw_secret = dir.path().join(".openclaw/secrets/rewire-token");
    assert_eq!(fs::read(&openclaw_secret).unwrap(), b"persistent-token");
    let openclaw = fs::read_to_string(dir.path().join(".openclaw/openclaw.json")).unwrap();
    assert!(openclaw.contains("singleValue"));
    assert!(openclaw.contains(&openclaw_secret.to_string_lossy().to_string()));
}

#[test]
fn hermes_dotenv_merge_preserves_unrelated_values_and_special_token_bytes() {
    let dir = tempdir().unwrap();
    let dotenv = dir.path().join(".hermes/.env");
    fs::create_dir_all(dotenv.parent().unwrap()).unwrap();
    fs::write(
        &dotenv,
        "# operator settings\nEXISTING=value\nexport REWIRE_TOKEN=old\nREWIRE_TOKEN=duplicate\n",
    )
    .unwrap();
    let token = "sp ace#'\\$中文";
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: Secret::new(token).unwrap(),
        clients: vec![Client::Hermes],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    apply_plan(dir.path(), &plan).unwrap();

    let content = fs::read_to_string(&dotenv).unwrap();
    assert!(content.contains("# operator settings\nEXISTING=value\n"));
    assert_eq!(content.matches("REWIRE_TOKEN=").count(), 1);
    let parsed = dotenvy::from_read_iter(content.as_bytes())
        .collect::<std::result::Result<HashMap<_, _>, _>>()
        .unwrap();
    assert_eq!(parsed["REWIRE_TOKEN"], token);
}

#[test]
fn a_secret_file_write_failure_restores_the_primary_config() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "{ custom: true }\n").unwrap();
    fs::write(dir.path().join(".config/rewire"), b"parent blocker").unwrap();
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert!(apply_plan(dir.path(), &plan).is_err());
    assert_eq!(fs::read_to_string(config).unwrap(), "{ custom: true }\n");
    assert_eq!(
        fs::read(dir.path().join(".config/rewire")).unwrap(),
        b"parent blocker"
    );
}

#[test]
fn rollback_removes_primary_and_secret_files_created_together() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("coder-model".into()),
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    rollback(dir.path(), &tx.id).unwrap();
    for recipe in Client::OpenCode.recipes(
        dir.path(),
        &input.base_url,
        input.token.expose(),
        input.model.as_deref(),
    ) {
        assert!(
            !recipe.path.exists(),
            "{} still exists",
            recipe.path.display()
        );
    }
}

#[test]
fn remove_plan_deletes_only_adapter_owned_fields_and_can_be_rolled_back() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("remove-secret").unwrap(),
        clients: CLIENTS.to_vec(),
        model: Some("coder-model".into()),
    };
    let configured = apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();
    let remove_plan = build_remove_plan(dir.path(), CLIENTS).unwrap();
    assert!(remove_plan.conflicts.is_empty());
    assert_eq!(
        remove_plan
            .changes
            .iter()
            .filter(|change| matches!(change.action, Action::Delete))
            .count(),
        2
    );
    let removed = apply_plan(dir.path(), &remove_plan).unwrap();

    let claude: Value =
        serde_json::from_slice(&fs::read(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    assert!(claude.pointer("/env/ANTHROPIC_BASE_URL").is_none());
    let codex: Value =
        toml_edit::de::from_slice(&fs::read(dir.path().join(".codex/config.toml")).unwrap())
            .unwrap();
    assert!(codex.pointer("/model_providers/rewire").is_none());
    let opencode: Value = serde_json::from_slice(
        &fs::read(dir.path().join(".config/opencode/opencode.jsonc")).unwrap(),
    )
    .unwrap();
    assert!(opencode.pointer("/provider/rewire").is_none());
    let hermes: Value =
        serde_yaml::from_slice(&fs::read(dir.path().join(".hermes/config.yaml")).unwrap()).unwrap();
    assert!(hermes.pointer("/providers/rewire").is_none());
    assert!(
        !fs::read_to_string(dir.path().join(".hermes/.env"))
            .unwrap()
            .contains("REWIRE_TOKEN=")
    );
    let openclaw: Value =
        serde_json::from_slice(&fs::read(dir.path().join(".openclaw/openclaw.json")).unwrap())
            .unwrap();
    assert!(openclaw.pointer("/models/providers/rewire").is_none());
    assert!(
        !dir.path()
            .join(".config/rewire/secrets/opencode-token")
            .exists()
    );
    assert!(!dir.path().join(".openclaw/secrets/rewire-token").exists());

    rollback(dir.path(), &removed.id).unwrap();
    assert_eq!(
        fs::read(dir.path().join(".config/rewire/secrets/opencode-token")).unwrap(),
        b"remove-secret"
    );
    assert_eq!(
        fs::read(dir.path().join(".openclaw/secrets/rewire-token")).unwrap(),
        b"remove-secret"
    );
    rollback(dir.path(), &configured.id).unwrap();
}

#[test]
fn remove_supports_toml_inline_tables_without_deleting_other_providers() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "model_providers = { rewire = { base_url = \"https://gateway.example\" }, keep = { base_url = \"https://keep.example\" } }\nprofiles = { rewire = { model_provider = \"rewire\" }, keep = { model_provider = \"keep\" } }\n",
    )
    .unwrap();
    let plan = build_remove_plan(dir.path(), &[Client::Codex]).unwrap();
    assert!(plan.conflicts.is_empty());
    apply_plan(dir.path(), &plan).unwrap();
    let value: Value = toml_edit::de::from_slice(&fs::read(config).unwrap()).unwrap();
    assert!(value.pointer("/model_providers/rewire").is_none());
    assert_eq!(
        value.pointer("/model_providers/keep/base_url").unwrap(),
        "https://keep.example"
    );
    assert!(value.pointer("/profiles/rewire").is_none());
    assert_eq!(
        value.pointer("/profiles/keep/model_provider").unwrap(),
        "keep"
    );
}

#[test]
fn remove_blocks_incompatible_parent_types_without_writing() {
    let dir = tempdir().unwrap();
    let config = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let original = b"{ provider: \"operator-owned-scalar\" }\n";
    fs::write(&config, original).unwrap();
    let plan = build_remove_plan(dir.path(), &[Client::OpenCode]).unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert!(plan.conflicts[0].blocking);
    assert!(plan.conflicts[0].reason.contains("not an object"));
    assert!(apply_plan(dir.path(), &plan).is_err());
    assert_eq!(fs::read(config).unwrap(), original);
}

#[test]
fn plans_debug_output_and_encrypted_backups_do_not_contain_tokens() {
    let dir = tempdir().unwrap();
    let token = "backup-scan-token";
    let config = dir.path().join(".claude/settings.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let original = format!(
        "{{\"env\":{{\"ANTHROPIC_BASE_URL\":\"https://old.example\",\"ANTHROPIC_AUTH_TOKEN\":\"{token}\"}}}}"
    );
    fs::write(&config, &original).unwrap();
    let input = Input {
        base_url: "https://new.example".into(),
        token: Secret::new(token).unwrap(),
        clients: vec![Client::Claude],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    assert!(!format!("{plan:?}").contains(token));
    assert!(!stable_json(&plan).unwrap().contains(token));
    let tx = apply_plan(dir.path(), &plan).unwrap();
    assert!(!stable_json(&tx).unwrap().contains(token));

    let transaction_dir = transaction_root(dir.path()).join(&tx.id);
    for entry in fs::read_dir(transaction_dir).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(token.len())
                .any(|window| window == token.as_bytes())
        );
    }
    rollback(dir.path(), &tx.id).unwrap();
    assert_eq!(fs::read_to_string(config).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn secret_targets_and_transaction_artifacts_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    let secret = dir.path().join(".config/rewire/secrets/opencode-token");
    assert_eq!(
        fs::metadata(secret).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let root = transaction_root(dir.path());
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let transaction_dir = root.join(tx.id);
    assert_eq!(
        fs::metadata(&transaction_dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for entry in fs::read_dir(transaction_dir).unwrap() {
        assert_eq!(
            entry.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
