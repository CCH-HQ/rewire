use super::recipes;
use std::path::Path;

#[test]
fn bare_origin_gets_responses_version_prefix() {
    let recipe = &recipes(
        Path::new("/fixture-home"),
        "https://gateway.example",
        "TOKEN",
    )[0];
    assert_eq!(
        recipe.values["model_providers"]["rewire"]["base_url"],
        "https://gateway.example/v1"
    );
}

#[test]
fn explicit_gateway_path_is_not_rewritten() {
    let recipe = &recipes(
        Path::new("/fixture-home"),
        "https://gateway.example/api/codex",
        "TOKEN",
    )[0];
    assert_eq!(
        recipe.values["model_providers"]["rewire"]["base_url"],
        "https://gateway.example/api/codex"
    );
}
