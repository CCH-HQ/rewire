use super::*;

#[test]
fn selected_sdk_controls_runtime_transport_and_base_url() {
    let home = Path::new("/tmp/fixture");
    let cases = [
        (
            OpenCodeSdk::Anthropic,
            "anthropic_messages",
            "https://gateway.example/",
        ),
        (
            OpenCodeSdk::OpenAi,
            "codex_responses",
            "https://gateway.example/v1",
        ),
        (
            OpenCodeSdk::OpenAiCompatible,
            "chat_completions",
            "https://gateway.example/v1",
        ),
        (
            OpenCodeSdk::Google,
            "chat_completions",
            "https://gateway.example/v1",
        ),
    ];

    for (sdk, transport, base_url) in cases {
        let recipe = &recipes(
            home,
            "https://gateway.example/",
            "TOKEN",
            Some("fixture-model"),
            sdk,
            &[],
        )[0];
        assert_eq!(
            recipe.values.pointer("/providers/rewire/transport"),
            Some(&Value::String(transport.into()))
        );
        assert_eq!(
            recipe.values.pointer("/providers/rewire/api"),
            Some(&Value::String(base_url.into()))
        );
        assert_eq!(
            recipe.values.pointer("/model/base_url"),
            Some(&Value::String(base_url.into()))
        );
    }
}

#[test]
fn explicit_gateway_paths_are_preserved() {
    let recipe = &recipes(
        Path::new("/tmp/fixture"),
        "https://gateway.example/tenant/openai",
        "TOKEN",
        Some("fixture-model"),
        OpenCodeSdk::OpenAiCompatible,
        &[],
    )[0];
    assert_eq!(
        recipe.values.pointer("/providers/rewire/api"),
        Some(&Value::String(
            "https://gateway.example/tenant/openai".into()
        ))
    );
}

#[test]
fn windows_default_directory_uses_local_app_data_then_home_fallback() {
    let home = Path::new("C:/Users/fixture");
    assert_eq!(
        windows_default_directory(Some(std::ffi::OsStr::new("D:/HermesData")), home),
        Path::new("D:/HermesData/hermes")
    );
    assert_eq!(
        windows_default_directory(None, home),
        Path::new("C:/Users/fixture/AppData/Local/hermes")
    );
}
