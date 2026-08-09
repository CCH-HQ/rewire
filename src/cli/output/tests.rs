use super::{Palette, render_plan};
use rewire::{Client, Input, Secret, apply_plan, build_plan};
use std::fs;
use tempfile::tempdir;

#[test]
fn plan_colors_create_and_summary_semantics() {
    let home = tempdir().unwrap();
    let plan = build_plan(
        home.path(),
        &Input {
            base_url: "https://gateway.example".into(),
            token: Secret::new("color-secret").unwrap(),
            clients: vec![Client::Claude],
            model: None,
            model_name: None,
            sdk: None,
        },
    )
    .unwrap();

    let colored = render_plan(&plan, Palette { enabled: true }).unwrap();
    let uncolored = render_plan(&plan, Palette { enabled: false }).unwrap();
    assert!(colored.contains("\u{1b}["));
    assert!(colored.contains("[CREATE]"));
    assert!(!uncolored.contains("\u{1b}["));
    assert!(uncolored.contains("1. [CREATE] claude"));
}

#[test]
fn blocking_plan_reasons_use_danger_color() {
    let home = tempdir().unwrap();
    let path = home.path().join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "{broken").unwrap();
    let plan = build_plan(
        home.path(),
        &Input {
            base_url: "https://gateway.example".into(),
            token: Secret::new("color-secret").unwrap(),
            clients: vec![Client::Claude],
            model: None,
            model_name: None,
            sdk: None,
        },
    )
    .unwrap();

    let colored = render_plan(&plan, Palette { enabled: true }).unwrap();
    assert!(colored.contains("[BLOCKED]"));
    assert!(colored.contains("Reason:"));
    assert!(colored.contains("\u{1b}[31m") || colored.contains("\u{1b}[1;31m"));
}

#[test]
fn provider_overwrite_reasons_use_review_color() {
    let home = tempdir().unwrap();
    let initial = Input {
        base_url: "https://old-gateway.example".into(),
        token: Secret::new("color-secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };
    apply_plan(home.path(), &build_plan(home.path(), &initial).unwrap()).unwrap();
    let replacement = Input {
        base_url: "https://new-gateway.example".into(),
        token: Secret::new("color-secret").unwrap(),
        clients: vec![Client::OpenCode],
        model: Some("coder-model".into()),
        model_name: None,
        sdk: None,
    };
    let plan = build_plan(home.path(), &replacement).unwrap();

    let colored = render_plan(&plan, Palette { enabled: true }).unwrap();
    assert!(colored.contains("[REVIEW]"));
    assert!(colored.contains("\u{1b}[33mReason:"));
}
