use super::{openclaw_api, openclaw_base_url};
use crate::model::OpenCodeSdk;

#[test]
fn maps_each_catalog_family_to_openclaw_transport() {
    assert_eq!(openclaw_api(OpenCodeSdk::OpenAi), "openai-responses");
    assert_eq!(openclaw_api(OpenCodeSdk::Anthropic), "anthropic-messages");
    assert_eq!(openclaw_api(OpenCodeSdk::Google), "google-generative-ai");
    assert_eq!(
        openclaw_api(OpenCodeSdk::OpenAiCompatible),
        "openai-completions"
    );
}

#[test]
fn applies_transport_versions_only_to_bare_origins() {
    assert_eq!(
        openclaw_base_url("https://gateway.example", OpenCodeSdk::OpenAi),
        "https://gateway.example/v1"
    );
    assert_eq!(
        openclaw_base_url("https://gateway.example", OpenCodeSdk::Google),
        "https://gateway.example/v1beta"
    );
    assert_eq!(
        openclaw_base_url("https://gateway.example", OpenCodeSdk::Anthropic),
        "https://gateway.example"
    );
    assert_eq!(
        openclaw_base_url("https://gateway.example/custom/google", OpenCodeSdk::Google,),
        "https://gateway.example/custom/google"
    );
}
