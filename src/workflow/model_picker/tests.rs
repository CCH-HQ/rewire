use super::*;

fn discovered(id: &str, display_name: Option<&str>) -> DiscoveredModel {
    DiscoveredModel {
        id: id.to_owned(),
        display_name: display_name.map(str::to_owned),
        sources: vec![ModelApi::OpenAi],
    }
}

#[test]
fn compact_picker_contains_only_discovered_models_and_actions() {
    let models = vec![discovered("gpt-5.5", Some("Remote GPT"))];
    let choices = build_choices(&models, false, Palette { enabled: false });
    assert_eq!(choices.len(), 4);
    assert!(matches!(choices[0].kind, ModelChoiceKind::AddAll));
    assert_eq!(choices[0].label, "Add all 1 available model");
    assert!(matches!(choices[1].kind, ModelChoiceKind::Discovered(_)));
    assert_eq!(choices[1].label, "Remote GPT [AVAILABLE] (gpt-5.5)");
    assert!(matches!(choices[2].kind, ModelChoiceKind::ShowAll));
    assert!(matches!(choices[3].kind, ModelChoiceKind::Custom));
    assert!(!choices.iter().any(|choice| choice.label.contains("Claude")));
}

#[test]
fn empty_discovery_keeps_catalog_explicitly_opt_in() {
    let choices = build_choices(&[], false, Palette { enabled: false });
    assert_eq!(choices.len(), 2);
    assert!(matches!(choices[0].kind, ModelChoiceKind::ShowAll));
    assert!(matches!(choices[1].kind, ModelChoiceKind::Custom));
}

#[test]
fn expanded_picker_adds_catalog_without_duplicate_ids() {
    let models = vec![discovered("gpt-5.5", None)];
    let choices = build_choices(&models, true, Palette { enabled: false });
    assert!(choices.iter().any(|choice| choice.label.contains("Claude")));
    assert_eq!(
        choices
            .iter()
            .filter(|choice| choice.label.contains("(gpt-5.5)"))
            .count(),
        1
    );
    assert!(
        !choices
            .iter()
            .any(|choice| matches!(choice.kind, ModelChoiceKind::ShowAll))
    );
    assert!(matches!(
        choices.last().map(|choice| &choice.kind),
        Some(ModelChoiceKind::Custom)
    ));
}

#[test]
fn available_marker_honors_color_policy() {
    let models = vec![discovered("remote-model", None)];
    let colored = build_choices(&models, false, Palette { enabled: true });
    assert!(colored[1].label.contains("\u{1b}[32m[AVAILABLE]"));
    let plain = build_choices(&models, false, Palette { enabled: false });
    assert_eq!(plain[1].label, "remote-model [AVAILABLE] (remote-model)");
}

#[test]
fn discovered_catalog_model_uses_catalog_sdk_instead_of_endpoint_source() {
    let model = DiscoveredModel {
        id: "kimi-k3".to_owned(),
        display_name: None,
        sources: vec![ModelApi::OpenAi],
    };
    let selected = select_discovered(model);
    assert_eq!(selected.sdk, OpenCodeSdk::OpenAiCompatible);
    assert_eq!(selected.display_name.as_deref(), Some("Kimi K3"));
    assert!(selected.models.is_empty());
}

#[test]
fn discovered_models_convert_to_named_catalog_entries() {
    let remote = discovered("gpt-5.5", None);
    let configured = model_config(&remote);
    assert_eq!(configured.id, "gpt-5.5");
    assert_eq!(configured.display_name.as_deref(), Some("GPT-5.5"));
    assert_eq!(configured.sdk, OpenCodeSdk::OpenAi);
}

#[test]
fn catalog_sdk_inference_handles_qualified_and_unknown_model_ids() {
    let qualified = discovered("openai/gpt-5.5", Some("Qualified GPT"));
    assert_eq!(model_config(&qualified).sdk, OpenCodeSdk::OpenAi);

    let unknown = discovered("vendor/new-model", None);
    assert_eq!(model_config(&unknown).sdk, OpenCodeSdk::OpenAiCompatible);
}

#[test]
fn add_all_keeps_the_complete_catalog_and_a_separate_default() {
    let models = vec![
        discovered("alpha-model", Some("Alpha")),
        discovered("beta-model", Some("Beta")),
    ];
    let selected = select_all_models(&models, models[1].clone());

    assert_eq!(selected.id, "beta-model");
    assert_eq!(selected.sdk, OpenCodeSdk::OpenAiCompatible);
    assert_eq!(
        selected
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha-model", "beta-model"]
    );
}

#[test]
fn model_prompt_names_only_clients_that_consume_the_selection() {
    assert_eq!(model_target_label(&[Client::OpenCode]), "OpenCode");
    assert_eq!(
        model_target_label(&[Client::Claude, Client::Codex, Client::Hermes]),
        "Hermes"
    );
    assert_eq!(
        model_target_label(&[Client::Codex, Client::Hermes, Client::OpenClaw]),
        "Hermes and OpenClaw"
    );
    assert_eq!(
        model_target_label(&[
            Client::Claude,
            Client::OpenCode,
            Client::Hermes,
            Client::OpenClaw,
        ]),
        "OpenCode, Hermes, and OpenClaw"
    );
}

#[test]
fn spinner_and_control_labels_remain_ascii() {
    assert!(CUSTOM_MODEL_PROMPT.is_ascii());
    assert!(SHOW_ALL_MODELS.is_ascii());
    assert!(model_target_label(&[Client::OpenCode, Client::Hermes]).is_ascii());
    assert!(SCAN_MESSAGE.is_ascii());
    assert!(SPINNER_FRAMES.iter().all(char::is_ascii));
}

#[test]
fn debug_line_contains_response_metadata_without_body_or_credentials() {
    let diagnostic = crate::DiscoveryDiagnostic {
        api: ModelApi::OpenAi,
        endpoint: "https://gateway.example/v1/models".to_owned(),
        status: Some(307),
        content_type: Some("text/html".to_owned()),
        location: Some("/login".to_owned()),
        response_bytes: Some(42),
        attempts: 1,
    };
    assert_eq!(
        format_discovery_diagnostic(&diagnostic),
        "OpenAI GET https://gateway.example/v1/models -> HTTP 307; content-type=text/html; bytes=42; attempts=1; location=/login"
    );
}
