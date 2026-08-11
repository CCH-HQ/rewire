use super::opencode_base_url;
use crate::model::OpenCodeSdk;

#[test]
fn bare_origins_receive_ai_sdk_version_prefixes() {
    assert_eq!(
        opencode_base_url("https://gateway.example", OpenCodeSdk::OpenAi),
        "https://gateway.example/v1"
    );
    assert_eq!(
        opencode_base_url("https://gateway.example", OpenCodeSdk::Anthropic),
        "https://gateway.example/v1"
    );
    assert_eq!(
        opencode_base_url("https://gateway.example", OpenCodeSdk::Google),
        "https://gateway.example/v1beta"
    );
    assert_eq!(
        opencode_base_url("https://gateway.example", OpenCodeSdk::OpenAiCompatible,),
        "https://gateway.example/v1"
    );
}

#[test]
fn explicit_compatibility_paths_are_preserved() {
    assert_eq!(
        opencode_base_url(
            "https://gateway.example/api/anthropic",
            OpenCodeSdk::Anthropic,
        ),
        "https://gateway.example/api/anthropic"
    );
    assert_eq!(
        opencode_base_url("http://[::1]:9000/custom/google", OpenCodeSdk::Google,),
        "http://[::1]:9000/custom/google"
    );
}
