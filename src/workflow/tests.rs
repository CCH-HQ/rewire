use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn detected_clients_are_marked_without_changing_stable_order() {
    let labels = client_labels(&[Client::OpenCode, Client::Claude]);
    assert_eq!(labels[0], "claude (detected)");
    assert_eq!(labels[1], "codex");
    assert_eq!(labels[2], "opencode (detected)");
}

#[test]
fn final_review_numbers_only_modifications_and_conflicts() {
    let home = tempdir().unwrap();
    let input = Input {
        base_url: "https://gateway.example".into(),
        token: crate::Secret::new("number-secret").unwrap(),
        clients: vec![Client::Claude, Client::Codex],
        model: None,
        model_name: None,
        sdk: None,
    };
    let plan = build_plan(home.path(), &input).unwrap();
    let items = numbered_plan_items(&plan);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].number, 1);
    assert_eq!(items[0].marker, PlanMarker::Create);
    assert_eq!(items[0].client, Client::Claude);
    assert_eq!(items[1].number, 2);
    assert_eq!(items[1].marker, PlanMarker::Create);
    assert_eq!(items[1].client, Client::Codex);
    assert!(!format!("{items:?}").contains("number-secret"));
}

#[test]
fn blocking_conflict_removes_apply_from_final_confirmation() {
    let actions = review_actions(ReviewSummary {
        updates: 1,
        conflicts: 1,
        blocking: 1,
        ..ReviewSummary::default()
    });
    assert_eq!(actions[0].0, ReviewAction::Edit);
    assert!(
        actions
            .iter()
            .all(|(action, _)| *action != ReviewAction::Apply)
    );
}

#[test]
fn reviewed_overwrite_is_named_in_final_confirmation() {
    let actions = review_actions(ReviewSummary {
        updates: 1,
        conflicts: 1,
        blocking: 0,
        ..ReviewSummary::default()
    });
    assert_eq!(actions[0].0, ReviewAction::Apply);
    assert_eq!(
        actions[0].1,
        "Apply 1 numbered modification(s), including 1 reviewed overwrite(s)"
    );
}

#[test]
fn malformed_target_gets_a_number_and_reason() {
    let home = tempdir().unwrap();
    let path = home.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "{broken").unwrap();
    let plan = build_plan(
        home.path(),
        &Input {
            base_url: "https://gateway.example".into(),
            token: crate::Secret::new("secret").unwrap(),
            clients: vec![Client::Claude],
            model: None,
            model_name: None,
            sdk: None,
        },
    )
    .unwrap();
    let items = numbered_plan_items(&plan);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].number, 1);
    assert_eq!(items[0].marker, PlanMarker::Blocked);
    assert_eq!(items[0].client, Client::Claude);
    assert!(items[0].reason.as_deref().unwrap().contains("parse JSON"));
}

#[test]
fn workflow_control_text_remains_ascii_for_legacy_windows_code_pages() {
    let actions = review_actions(ReviewSummary {
        updates: 2,
        ..ReviewSummary::default()
    });
    let fixed = [
        WORKFLOW_TITLE,
        WORKFLOW_HELP,
        CLIENT_PROMPT,
        BASE_URL_PROMPT,
        TOKEN_PROMPT,
        MODEL_PROMPT,
        CUSTOM_MODEL_PROMPT,
        MODEL_NAME_PROMPT,
        SDK_PROMPT,
        REVIEW_PROMPT,
        BLOCKED_PROMPT,
    ];
    assert!(fixed.iter().all(|text| text.is_ascii()));
    assert!(client_labels(&[]).iter().all(|text| text.is_ascii()));
    assert!(actions.iter().all(|(_, label)| label.is_ascii()));
    assert_eq!(TOKEN_DISPLAY_MODE, PasswordDisplayMode::Masked);
    assert!(render_config(true).password_mask.is_ascii());
}

#[test]
fn opencode_sdk_choices_are_typed_and_model_aware() {
    let choices = OpenCodeSdk::choices();
    assert!(choices.iter().all(|choice| choice.to_string().is_ascii()));
    for (model, expected, cursor) in [
        ("gpt-5.5", OpenCodeSdk::OpenAi, 0),
        ("claude-sonnet-4-5", OpenCodeSdk::Anthropic, 1),
        ("gemini-3-pro", OpenCodeSdk::Google, 2),
        ("custom-model", OpenCodeSdk::OpenAiCompatible, 3),
    ] {
        let inferred = OpenCodeSdk::infer(Some(model));
        assert_eq!(inferred, expected);
        assert_eq!(
            choices.iter().position(|choice| *choice == inferred),
            Some(cursor)
        );
    }
}

#[test]
fn model_catalog_labels_keep_ids_separate_from_display_names() {
    let choices = popular_models()
        .iter()
        .copied()
        .map(ModelChoice::Preset)
        .chain(std::iter::once(ModelChoice::Custom))
        .collect::<Vec<_>>();
    assert!(choices.iter().all(|choice| choice.to_string().is_ascii()));
    let first = choices.first().unwrap().to_string();
    assert!(first.contains("gpt-5.5"));
    assert!(first.contains("GPT-5.5"));
}

#[test]
fn semantic_palette_colors_status_without_changing_plain_copy() {
    let enabled = Palette { enabled: true };
    assert!(enabled.success("Applied").contains("\u{1b}[32m"));
    assert!(enabled.warning("Cancelled").contains("\u{1b}[33m"));
    assert!(enabled.danger("Blocked").contains("\u{1b}[31m"));
    assert!(enabled.accent("Plan").contains("\u{1b}[36m"));
    assert!(enabled.muted("Summary").contains("\u{1b}[90m"));
    assert!(enabled.warning("Cancelled").ends_with("\u{1b}[0m"));

    let disabled = Palette { enabled: false };
    assert_eq!(disabled.success("Applied"), "Applied");
    assert_eq!(disabled.warning("Cancelled"), "Cancelled");
    assert_eq!(disabled.danger("Blocked"), "Blocked");
}
