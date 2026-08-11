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
use std::{
    collections::{HashMap, HashSet},
    fs,
};
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
fn model_selection_is_required_and_validated_at_the_core_boundary() {
    for client in [Client::OpenCode, Client::Hermes, Client::OpenClaw] {
        let dir = tempdir().unwrap();
        let error = build_plan(
            dir.path(),
            &Input {
                base_url: "https://gateway.example".into(),
                token: Secret::new("secret").unwrap(),
                clients: vec![client],
                model: None,
                model_name: None,
                sdk: None,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("--model is required"));
        assert!(!dir.path().join(".config/rewire").exists());
    }

    for invalid in [
        "",
        " coder-model",
        "coder-model ",
        "rewire/coder-model",
        "bad\nmodel",
    ] {
        assert!(validate_model_id(invalid).is_err(), "accepted {invalid:?}");
    }
    for valid in ["coder-model", "gpt-4.1-mini", "upstream/model-id"] {
        validate_model_id(valid).unwrap();
    }
    for invalid in ["", " GPT-5.5", "GPT-5.5 ", "GPT\n5.5"] {
        assert!(
            validate_model_name(invalid).is_err(),
            "accepted {invalid:?}"
        );
    }
    assert!(OpenCodeSdk::parse("unknown-sdk").is_err());
}

#[test]
fn local_model_catalog_has_provider_native_ids_and_matching_sdk_families() {
    assert!(popular_models().len() >= 70);
    let mut ids = HashSet::new();
    for preset in popular_models() {
        validate_model_id(preset.id).unwrap();
        assert!(ids.insert(preset.id), "duplicate model ID: {}", preset.id);
        assert!(!preset.display_name.is_empty());
        assert!(!preset.provider.is_empty());
    }
    assert_eq!(find_model("gpt-5.5").unwrap().sdk, OpenCodeSdk::OpenAi);
    assert_eq!(
        find_model("claude-sonnet-5").unwrap().sdk,
        OpenCodeSdk::Anthropic
    );
    assert_eq!(
        find_model("gemini-3.6-flash").unwrap().sdk,
        OpenCodeSdk::Google
    );
    assert_eq!(
        find_model("Qwen3-Coder-Next").unwrap().sdk,
        OpenCodeSdk::OpenAiCompatible
    );
    assert_eq!(
        find_model("kimi-k3").unwrap().sdk,
        OpenCodeSdk::OpenAiCompatible
    );
    assert_eq!(
        find_model("MiniMax-M3").unwrap().sdk,
        OpenCodeSdk::OpenAiCompatible
    );
    assert!(find_model("model-not-in-catalog").is_none());
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
        model_name: None,
        sdk: None,
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
    assert!(codex.pointer("/profiles/rewire/model").is_none());

    let opencode_recipes = Client::OpenCode.recipes(home, endpoint, "TOKEN", Some("coder-model"));
    let opencode_recipe = &opencode_recipes[0];
    assert_eq!(
        opencode_recipe.path,
        home.join(".config/opencode/opencode.jsonc")
    );
    let opencode = &opencode_recipe.values;
    assert_eq!(opencode.pointer("/model").unwrap(), "rewire/coder-model");
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
fn opencode_uses_native_catalogs_for_openai_and_anthropic_only() {
    let home = std::path::Path::new("/fixture-home");
    for (sdk, model, provider) in [
        (OpenCodeSdk::OpenAi, "gpt-5.5", "openai"),
        (OpenCodeSdk::Anthropic, "claude-sonnet-5", "anthropic"),
    ] {
        let recipes = Client::OpenCode.recipes_with_options(
            home,
            "https://gateway.example/v1",
            "TOKEN",
            Some(model),
            Some("ignored display name"),
            Some(sdk),
        );
        let config = &recipes[0].values;
        assert_eq!(config["model"], format!("{provider}/{model}"));
        assert_eq!(
            config["provider"][provider]["options"]["baseURL"],
            "https://gateway.example/v1"
        );
        assert_eq!(
            config["provider"][provider]["options"]["apiKey"],
            "{file:/fixture-home/.config/rewire/secrets/opencode-token}"
        );
        assert!(config["provider"][provider].get("npm").is_none());
        assert!(config["provider"][provider].get("name").is_none());
        assert!(config["provider"][provider].get("models").is_none());
        assert!(config["provider"].get("rewire").is_none());
    }

    let compatible = Client::OpenCode.recipes_with_options(
        home,
        "https://gateway.example/v1",
        "TOKEN",
        Some("custom-model"),
        Some("Custom model"),
        Some(OpenCodeSdk::OpenAiCompatible),
    );
    let config = &compatible[0].values;
    assert_eq!(config["model"], "rewire/custom-model");
    assert_eq!(
        config["provider"]["rewire"]["npm"],
        "@ai-sdk/openai-compatible"
    );
    assert_eq!(
        config["provider"]["rewire"]["models"]["custom-model"]["name"],
        "Custom model"
    );
    assert_eq!(
        OpenCodeSdk::parse("@ai-sdk/anthropic").unwrap(),
        OpenCodeSdk::Anthropic
    );
    assert_eq!(
        OpenCodeSdk::infer(Some("claude-sonnet-4-5")).npm(),
        "@ai-sdk/anthropic"
    );
    assert_eq!(
        OpenCodeSdk::infer(Some("gemini-3-pro")).npm(),
        "@ai-sdk/google"
    );
    assert_eq!(
        OpenCodeSdk::infer(Some("custom-model")).npm(),
        "@ai-sdk/openai-compatible"
    );
}

#[test]
fn opencode_native_provider_removal_is_credential_scoped() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("native-secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("gpt-5.5".into()),
        model_name: None,
        sdk: Some(OpenCodeSdk::OpenAi),
    };
    apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();

    let path = dir.path().join(".config/opencode/opencode.jsonc");
    let mut config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    config["provider"]["openai"]["options"]["timeout"] = serde_json::json!(30_000);
    fs::write(&path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();

    let plan = build_remove_plan(dir.path(), &[Client::OpenCode]).unwrap();
    assert!(plan.conflicts.is_empty());
    apply_plan(dir.path(), &plan).unwrap();

    let config: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert!(config.get("model").is_none());
    assert_eq!(config["provider"]["openai"]["options"]["timeout"], 30_000);
    assert!(config.pointer("/provider/openai/options/baseURL").is_none());
    assert!(config.pointer("/provider/openai/options/apiKey").is_none());
}

#[test]
fn opencode_remove_preserves_operator_owned_native_provider() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = serde_json::json!({
        "model": "openai/gpt-5.5",
        "provider": {
            "openai": {
                "options": {
                    "baseURL": "https://operator.example/v1",
                    "apiKey": "{env:OPENAI_API_KEY}"
                }
            }
        }
    });
    fs::write(&path, serde_json::to_vec_pretty(&original).unwrap()).unwrap();

    let plan = build_remove_plan(dir.path(), &[Client::OpenCode]).unwrap();
    assert!(
        plan.changes
            .iter()
            .all(|change| matches!(change.action, Action::Noop))
    );
    apply_plan(dir.path(), &plan).unwrap();
    let current: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(current, original);
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
    assert_eq!(hermes.pointer("/model/provider").unwrap(), "rewire");
    assert_eq!(hermes.pointer("/model/default").unwrap(), "coder-model");
    assert_eq!(hermes.pointer("/model/base_url").unwrap(), endpoint);
    assert!(
        hermes
            .pointer("/providers/rewire/models/coder-model")
            .unwrap()
            .is_object()
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
        openclaw
            .pointer("/models/providers/rewire/models/0/api")
            .unwrap(),
        "openai-completions"
    );
    assert_eq!(
        openclaw
            .pointer("/models/providers/rewire/models/0/baseUrl")
            .unwrap(),
        endpoint
    );
    assert_eq!(
        openclaw.pointer("/agents/defaults/model/primary").unwrap(),
        "rewire/coder-model"
    );
    assert_eq!(
        openclaw_recipes[1].path,
        home.join(".openclaw/secrets/rewire-token")
    );
    assert!(openclaw.pointer("/providers/rewire").is_none());
}

#[test]
fn hermes_legacy_aliases_migrate_without_losing_operator_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".hermes/config.yaml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r"model:
  provider: rewire
  name: coder-model
  context_length: 131072
providers:
  rewire:
    name: Rewire
    base_url: https://gateway.example/v1
    key_env: REWIRE_TOKEN
    api_mode: chat_completions
    model: coder-model
    models:
      - coder-model
      - stale-model
    request_timeout_seconds: 45
  operator:
    name: Operator
    base_url: https://operator.example/v1
    key_env: OPERATOR_TOKEN
features:
  audit: true
",
    )
    .unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Hermes],
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };

    let migration = build_plan(dir.path(), &input).unwrap();
    assert!(migration.conflicts.is_empty());
    apply_plan(dir.path(), &migration).unwrap();
    let migrated: Value = serde_yaml::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(migrated["model"]["default"], "coder-model");
    assert_eq!(migrated["model"]["provider"], "rewire");
    assert_eq!(migrated["model"]["context_length"], 131_072);
    assert!(migrated["model"].get("name").is_none());
    assert_eq!(
        migrated["providers"]["rewire"]["api"],
        "https://gateway.example/v1"
    );
    assert_eq!(
        migrated["providers"]["rewire"]["transport"],
        "chat_completions"
    );
    assert_eq!(
        migrated["providers"]["rewire"]["default_model"],
        "coder-model"
    );
    assert_eq!(
        migrated["providers"]["rewire"]["request_timeout_seconds"],
        45
    );
    assert!(migrated["providers"]["rewire"].get("base_url").is_none());
    assert!(migrated["providers"]["rewire"].get("api_mode").is_none());
    assert!(migrated["providers"]["rewire"].get("model").is_none());
    assert_eq!(
        migrated["providers"]["rewire"]["models"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["coder-model"]
    );
    assert!(migrated["providers"].get("operator").is_some());
    assert_eq!(migrated["features"]["audit"], true);

    let repeated = build_plan(dir.path(), &input).unwrap();
    assert!(
        repeated
            .changes
            .iter()
            .all(|change| matches!(change.action, Action::Noop))
    );

    apply_plan(
        dir.path(),
        &build_remove_plan(dir.path(), &[Client::Hermes]).unwrap(),
    )
    .unwrap();
    let removed: Value = serde_yaml::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(removed["model"]["context_length"], 131_072);
    assert!(removed["model"].get("default").is_none());
    assert!(removed["model"].get("provider").is_none());
    assert!(removed["model"].get("base_url").is_none());
    assert!(removed["providers"].get("rewire").is_none());
    assert!(removed["providers"].get("operator").is_some());
    assert_eq!(removed["features"]["audit"], true);
}

#[test]
fn hermes_alias_endpoint_replacement_still_requires_review() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".hermes/config.yaml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        "providers:\n  rewire:\n    base_url: https://old.example/v1\n    key_env: REWIRE_TOKEN\n",
    )
    .unwrap();
    let input = Input {
        base_url: "https://new.example/v1".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Hermes],
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };

    let plan = build_plan(dir.path(), &input).unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert!(!plan.conflicts[0].blocking);
    assert!(plan.conflicts[0].reason.contains("different base URL"));
}

#[test]
fn hermes_canonical_endpoint_wins_over_a_stale_alias() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".hermes/config.yaml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        "providers:\n  rewire:\n    api: https://current.example/v1\n    base_url: https://stale.example/v1\n    key_env: REWIRE_TOKEN\n",
    )
    .unwrap();
    let input = Input {
        base_url: "https://current.example/v1".into(),
        token: Secret::new("secret").unwrap(),
        clients: vec![Client::Hermes],
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };

    let plan = build_plan(dir.path(), &input).unwrap();
    assert!(plan.conflicts.is_empty());
    apply_plan(dir.path(), &plan).unwrap();
    let migrated: Value =
        serde_yaml::from_slice(&fs::read(dir.path().join(".hermes/config.yaml")).unwrap()).unwrap();
    assert_eq!(
        migrated["providers"]["rewire"]["api"],
        "https://current.example/v1"
    );
    assert!(migrated["providers"]["rewire"].get("base_url").is_none());
}

#[test]
fn discovered_catalog_is_written_for_opencode_hermes_and_openclaw() {
    let dir = tempdir().unwrap();
    let models = vec![
        ModelConfig {
            id: "gpt-5.5".into(),
            display_name: Some("GPT-5.5".into()),
            sdk: OpenCodeSdk::OpenAi,
        },
        ModelConfig {
            id: "claude-sonnet-5".into(),
            display_name: Some("Claude Sonnet 5".into()),
            sdk: OpenCodeSdk::Anthropic,
        },
        ModelConfig {
            id: "gemini-3-pro".into(),
            display_name: Some("Gemini 3 Pro".into()),
            sdk: OpenCodeSdk::Google,
        },
        ModelConfig {
            id: "custom-model".into(),
            display_name: None,
            sdk: OpenCodeSdk::OpenAiCompatible,
        },
    ];
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: Secret::new("catalog-secret").unwrap(),
        clients: vec![Client::OpenCode, Client::Hermes, Client::OpenClaw],
        model: Some("claude-sonnet-5".into()),
        model_name: Some("Claude Sonnet 5".into()),
        sdk: Some(OpenCodeSdk::Anthropic),
    };
    let plan = build_plan_with_catalog(dir.path(), &input, &models).unwrap();
    assert_eq!(plan.models, models);
    assert_eq!(plan.sdk.as_deref(), Some("@ai-sdk/anthropic"));
    apply_plan(dir.path(), &plan).unwrap();

    let opencode: Value = serde_json::from_slice(
        &fs::read(dir.path().join(".config/opencode/opencode.jsonc")).unwrap(),
    )
    .unwrap();
    assert_opencode_catalog(&opencode);

    let hermes: Value =
        serde_yaml::from_slice(&fs::read(dir.path().join(".hermes/config.yaml")).unwrap()).unwrap();
    assert_hermes_catalog(&hermes);

    let openclaw: Value =
        serde_json::from_slice(&fs::read(dir.path().join(".openclaw/openclaw.json")).unwrap())
            .unwrap();
    assert_openclaw_catalog(&openclaw);
}

fn assert_opencode_catalog(opencode: &Value) {
    assert_eq!(opencode["model"], "rewire-anthropic/claude-sonnet-5");
    assert!(opencode["provider"].get("rewire").is_none());
    assert_eq!(
        opencode["provider"]["rewire-oairesp"]["npm"],
        "@ai-sdk/openai"
    );
    assert_eq!(
        opencode["provider"]["rewire-anthropic"]["npm"],
        "@ai-sdk/anthropic"
    );
    assert_eq!(
        opencode["provider"]["rewire-google"]["npm"],
        "@ai-sdk/google"
    );
    assert_eq!(
        opencode["provider"]["rewire-oaicomp"]["npm"],
        "@ai-sdk/openai-compatible"
    );
    for provider in [
        "rewire-oairesp",
        "rewire-anthropic",
        "rewire-google",
        "rewire-oaicomp",
    ] {
        assert_eq!(
            opencode["provider"][provider]["models"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
    }
    assert_eq!(
        opencode["provider"]["rewire-oairesp"]["options"]["baseURL"],
        "https://gateway.example/v1"
    );
    assert_eq!(
        opencode["provider"]["rewire-anthropic"]["options"]["baseURL"],
        "https://gateway.example/v1"
    );
    assert_eq!(
        opencode["provider"]["rewire-google"]["options"]["baseURL"],
        "https://gateway.example/v1beta"
    );
    assert_eq!(
        opencode["provider"]["rewire-oaicomp"]["options"]["baseURL"],
        "https://gateway.example/v1"
    );
}

fn assert_hermes_catalog(hermes: &Value) {
    assert_eq!(hermes["model"]["default"], "claude-sonnet-5");
    assert_eq!(
        hermes["providers"]["rewire"]["models"]
            .as_object()
            .unwrap()
            .len(),
        4
    );
}

fn assert_openclaw_catalog(openclaw: &Value) {
    assert_eq!(
        openclaw["agents"]["defaults"]["model"]["primary"],
        "rewire/claude-sonnet-5"
    );
    assert_eq!(
        openclaw["models"]["providers"]["rewire"]["models"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let openclaw_models = openclaw["models"]["providers"]["rewire"]["models"]
        .as_array()
        .unwrap();
    let openclaw_model = |id: &str| {
        openclaw_models
            .iter()
            .find(|model| model["id"] == id)
            .unwrap()
    };
    assert_eq!(openclaw_model("gpt-5.5")["api"], "openai-responses");
    assert_eq!(
        openclaw_model("gpt-5.5")["baseUrl"],
        "https://gateway.example/v1"
    );
    assert_eq!(
        openclaw_model("claude-sonnet-5")["api"],
        "anthropic-messages"
    );
    assert_eq!(
        openclaw_model("claude-sonnet-5")["baseUrl"],
        "https://gateway.example"
    );
    assert_eq!(
        openclaw_model("gemini-3-pro")["api"],
        "google-generative-ai"
    );
    assert_eq!(
        openclaw_model("gemini-3-pro")["baseUrl"],
        "https://gateway.example/v1beta"
    );
    assert_eq!(openclaw_model("custom-model")["api"], "openai-completions");
    assert_eq!(
        openclaw_model("custom-model")["baseUrl"],
        "https://gateway.example/v1"
    );
}

#[test]
fn opencode_catalog_migrates_and_reconciles_rewire_managed_provider_groups() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join(".config/opencode/opencode.jsonc");
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let token_reference = format!(
        "{{file:{}}}",
        dir.path()
            .join(".config/rewire/secrets/opencode-token")
            .to_string_lossy()
    );
    fs::write(
        &config_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "model": "rewire/claude-sonnet-5",
            "theme": "operator-theme",
            "provider": {
                "rewire": {
                    "name": "Rewire",
                    "npm": "@ai-sdk/openai-compatible",
                    "options": {
                        "baseURL": "https://gateway.example/v1",
                        "apiKey": token_reference,
                    },
                    "models": {
                        "gpt-5.5": {"name": "GPT-5.5"},
                        "claude-sonnet-5": {"name": "Claude Sonnet 5"},
                    },
                },
                "operator": {
                    "npm": "@ai-sdk/openai-compatible",
                    "options": {"baseURL": "https://operator.example/v1"},
                    "models": {"keep-model": {"name": "Keep Model"}},
                },
            },
        }))
        .unwrap(),
    )
    .unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("catalog-secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("claude-sonnet-5".into()),
        model_name: Some("Claude Sonnet 5".into()),
        sdk: Some(OpenCodeSdk::Anthropic),
    };
    let full_catalog = vec![
        ModelConfig {
            id: "gpt-5.5".into(),
            display_name: Some("GPT-5.5".into()),
            sdk: OpenCodeSdk::OpenAi,
        },
        ModelConfig {
            id: "claude-sonnet-5".into(),
            display_name: Some("Claude Sonnet 5".into()),
            sdk: OpenCodeSdk::Anthropic,
        },
    ];
    apply_plan(
        dir.path(),
        &build_plan_with_catalog(dir.path(), &input, &full_catalog).unwrap(),
    )
    .unwrap();

    let migrated: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    assert_eq!(migrated["theme"], "operator-theme");
    assert!(migrated["provider"].get("rewire").is_none());
    assert!(migrated["provider"].get("operator").is_some());
    assert!(migrated["provider"].get("rewire-oairesp").is_some());
    assert!(migrated["provider"].get("rewire-anthropic").is_some());
    assert_eq!(migrated["model"], "rewire-anthropic/claude-sonnet-5");

    let mut migrated = migrated;
    migrated["provider"]["rewire-anthropic"]["options"]["timeout"] = serde_json::json!(30_000);
    fs::write(&config_path, serde_json::to_vec_pretty(&migrated).unwrap()).unwrap();

    let anthropic_only = vec![full_catalog[1].clone()];
    apply_plan(
        dir.path(),
        &build_plan_with_catalog(dir.path(), &input, &anthropic_only).unwrap(),
    )
    .unwrap();
    let reconciled: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    assert!(reconciled["provider"].get("rewire-oairesp").is_none());
    assert_eq!(
        reconciled["provider"]["rewire-anthropic"]["models"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reconciled["provider"]["rewire-anthropic"]["options"]["timeout"],
        30_000
    );
    assert!(reconciled["provider"].get("operator").is_some());
}

#[test]
fn discovered_catalog_requires_unique_ids_and_an_included_default() {
    let dir = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("catalog-secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("default-model".into()),
        model_name: None,
        sdk: None,
    };
    let duplicate = vec![
        ModelConfig {
            id: "default-model".into(),
            display_name: None,
            sdk: OpenCodeSdk::OpenAiCompatible,
        },
        ModelConfig {
            id: "default-model".into(),
            display_name: None,
            sdk: OpenCodeSdk::OpenAiCompatible,
        },
    ];
    assert!(build_plan_with_catalog(dir.path(), &input, &duplicate).is_err());
    let missing_default = vec![ModelConfig {
        id: "other-model".into(),
        display_name: None,
        sdk: OpenCodeSdk::OpenAiCompatible,
    }];
    assert!(build_plan_with_catalog(dir.path(), &input, &missing_default).is_err());
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
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };
    let plan = build_plan(dir.path(), &input).unwrap();
    let tx = apply_plan(dir.path(), &plan).unwrap();
    let path = Client::OpenCode.recipes(dir.path(), &plan.base_url, "token", Some("coder-model"))
        [0]
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
            model_name: None,
            sdk: None,
        };
        apply_plan(dir.path(), &build_plan(dir.path(), &initial).unwrap()).unwrap();

        // A cosmetic trailing slash normalizes to the same endpoint and must not request review.
        let repeated = Input {
            base_url: "https://gateway.example/v1/".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
            model_name: None,
            sdk: None,
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
            model_name: None,
            sdk: None,
        };
        apply_plan(dir.path(), &build_plan(dir.path(), &initial).unwrap()).unwrap();

        let replacement = Input {
            base_url: "https://new-gateway.example/v1".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("coder-model".into()),
            model_name: None,
            sdk: None,
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
fn replacing_a_client_selected_model_requires_review_in_each_native_format() {
    for client in [Client::OpenCode, Client::Hermes, Client::OpenClaw] {
        let dir = tempdir().unwrap();
        let initial = Input {
            base_url: "https://gateway.example/v1".into(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("old-model".into()),
            model_name: None,
            sdk: None,
        };
        apply_plan(dir.path(), &build_plan(dir.path(), &initial).unwrap()).unwrap();

        let replacement = Input {
            base_url: initial.base_url.clone(),
            token: Secret::new("secret").unwrap(),
            clients: vec![client],
            model: Some("new-model".into()),
            model_name: None,
            sdk: None,
        };
        let plan = build_plan(dir.path(), &replacement).unwrap();
        assert_eq!(plan.conflicts.len(), 1, "{client} review count");
        assert!(!plan.conflicts[0].blocking);
        assert!(plan.conflicts[0].reason.contains("selected model"));
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
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
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
        model_name: None,
        sdk: None,
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
    assert!(opencode.pointer("/model").is_none());
    let hermes: Value =
        serde_yaml::from_slice(&fs::read(dir.path().join(".hermes/config.yaml")).unwrap()).unwrap();
    assert!(hermes.pointer("/providers/rewire").is_none());
    assert!(hermes.pointer("/model").is_none());
    assert!(
        !fs::read_to_string(dir.path().join(".hermes/.env"))
            .unwrap()
            .contains("REWIRE_TOKEN=")
    );
    let openclaw: Value =
        serde_json::from_slice(&fs::read(dir.path().join(".openclaw/openclaw.json")).unwrap())
            .unwrap();
    assert!(openclaw.pointer("/models/providers/rewire").is_none());
    assert!(openclaw.pointer("/agents/defaults/model/primary").is_none());
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
fn remove_preserves_model_selections_changed_to_another_provider() {
    let dir = tempdir().unwrap();
    let clients = [Client::OpenCode, Client::Hermes, Client::OpenClaw];
    let input = Input {
        base_url: "https://gateway.example/v1".into(),
        token: Secret::new("remove-secret").unwrap(),
        clients: clients.to_vec(),
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };
    apply_plan(dir.path(), &build_plan(dir.path(), &input).unwrap()).unwrap();

    let opencode_path = dir.path().join(".config/opencode/opencode.jsonc");
    let mut opencode: Value = serde_json::from_slice(&fs::read(&opencode_path).unwrap()).unwrap();
    opencode["model"] = Value::String("other/coder-model".into());
    fs::write(
        &opencode_path,
        serde_json::to_vec_pretty(&opencode).unwrap(),
    )
    .unwrap();

    let hermes_path = dir.path().join(".hermes/config.yaml");
    let mut hermes: Value = serde_yaml::from_slice(&fs::read(&hermes_path).unwrap()).unwrap();
    hermes["model"] = serde_json::json!({"provider": "other", "name": "coder-model"});
    fs::write(&hermes_path, serde_yaml::to_string(&hermes).unwrap()).unwrap();

    let openclaw_path = dir.path().join(".openclaw/openclaw.json");
    let mut openclaw: Value = serde_json::from_slice(&fs::read(&openclaw_path).unwrap()).unwrap();
    openclaw["agents"]["defaults"]["model"]["primary"] = Value::String("other/coder-model".into());
    fs::write(
        &openclaw_path,
        serde_json::to_vec_pretty(&openclaw).unwrap(),
    )
    .unwrap();

    apply_plan(
        dir.path(),
        &build_remove_plan(dir.path(), &clients).unwrap(),
    )
    .unwrap();

    let opencode: Value = serde_json::from_slice(&fs::read(opencode_path).unwrap()).unwrap();
    assert_eq!(opencode["model"], "other/coder-model");
    assert!(opencode.pointer("/provider/rewire").is_none());
    let hermes: Value = serde_yaml::from_slice(&fs::read(hermes_path).unwrap()).unwrap();
    assert_eq!(hermes["model"]["provider"], "other");
    assert!(hermes.pointer("/providers/rewire").is_none());
    let openclaw: Value = serde_json::from_slice(&fs::read(openclaw_path).unwrap()).unwrap();
    assert_eq!(
        openclaw["agents"]["defaults"]["model"]["primary"],
        "other/coder-model"
    );
    assert!(openclaw.pointer("/models/providers/rewire").is_none());
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
        model_name: None,
        sdk: None,
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
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
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
