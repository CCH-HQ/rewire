use crate::{find_model, redact, validate_base_url};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use ureq::{Agent, Error as UreqError};
use zeroize::Zeroizing;

const API_COUNT: usize = 3;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_ATTEMPTS: usize = 3;
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const KNOWN_COMPAT_SUFFIXES: [&str; 9] = [
    "/api/claudecode",
    "/api/anthropic",
    "/apps/anthropic",
    "/api/coding",
    "/claudecode",
    "/anthropic",
    "/step_plan",
    "/coding",
    "/claude",
];

/// A wire protocol attempted against the compatible gateway's Models endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModelApi {
    OpenAi,
    Anthropic,
    Google,
}

impl ModelApi {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Google => "Google",
        }
    }

    const fn all() -> [Self; API_COUNT] {
        [Self::OpenAi, Self::Anthropic, Self::Google]
    }

    const fn root_version(self) -> &'static str {
        match self {
            Self::OpenAi | Self::Anthropic => "v1",
            Self::Google => "v1beta",
        }
    }
}

impl std::fmt::Display for ModelApi {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// A model returned by at least one compatible Models endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredModel {
    pub id: String,
    pub display_name: Option<String>,
    pub sources: Vec<ModelApi>,
}

/// One isolated protocol failure. Reasons are deliberately credential-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryFailure {
    pub api: ModelApi,
    pub reason: String,
}

/// Credential-free request metadata available through the workflow's explicit debug mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryDiagnostic {
    pub api: ModelApi,
    pub endpoint: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub location: Option<String>,
    pub response_bytes: Option<usize>,
    pub attempts: usize,
}

impl DiscoveryDiagnostic {
    fn new(api: ModelApi, endpoint: String) -> Self {
        Self {
            api,
            endpoint,
            status: None,
            content_type: None,
            location: None,
            response_bytes: None,
            attempts: 0,
        }
    }
}

/// Complete best-effort discovery result across all supported Models APIs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub models: Vec<DiscoveredModel>,
    pub successful_apis: Vec<ModelApi>,
    pub failures: Vec<DiscoveryFailure>,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

impl DiscoveryReport {
    #[must_use]
    pub fn successful_api_count(&self) -> usize {
        self.successful_apis.len()
    }
}

/// Runtime limits for discovery. The workflow uses the conservative defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryOptions {
    pub timeout: Duration,
    pub max_body_bytes: usize,
    /// Total attempts per API, including the first request.
    pub max_attempts: usize,
    /// Delay before the second attempt; subsequent delays double with saturation.
    pub initial_backoff: Duration,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteModel {
    id: String,
    display_name: Option<String>,
}

enum RequestFailure {
    Retryable(String),
    Final(String),
}

impl RequestFailure {
    fn reason(self) -> String {
        match self {
            Self::Retryable(reason) | Self::Final(reason) => reason,
        }
    }
}

/// Build the protocol-standard Models endpoint while preserving an explicit path prefix.
///
/// Root URLs use `/v1/models` for `OpenAI` and `Anthropic`, and `/v1beta/models` for `Google`.
/// A base URL that already includes a path keeps that path and receives one `models` segment.
///
/// # Errors
///
/// Returns an error for an invalid compatible base URL or a URL that cannot accept path segments.
pub fn models_endpoint(base_url: &str, api: ModelApi) -> anyhow::Result<String> {
    let normalized = validate_base_url(base_url)?;
    let mut url = url::Url::parse(&normalized)?;
    let root_path = matches!(url.path(), "" | "/");
    let already_models = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        == Some("models");
    let mut segments = url
        .path_segments_mut()
        .map_err(|()| anyhow::anyhow!("compatible base URL cannot accept path segments"))?;
    segments.pop_if_empty();
    if root_path {
        segments.push(api.root_version());
    }
    if !already_models {
        segments.push("models");
    }
    drop(segments);
    Ok(url.into())
}

/// Build ordered Models endpoint candidates for gateway URLs with compatibility subpaths.
///
/// The explicit path remains first. When it ends in a known Anthropic-compatible routing suffix,
/// a suffix-stripped protocol endpoint is added as a fallback for gateways that expose model
/// discovery outside their request-routing prefix.
///
/// # Errors
///
/// Returns an error under the same conditions as [`models_endpoint`].
pub fn models_endpoint_candidates(base_url: &str, api: ModelApi) -> anyhow::Result<Vec<String>> {
    let normalized = validate_base_url(base_url)?;
    let mut candidates = vec![models_endpoint(&normalized, api)?];
    let mut url = url::Url::parse(&normalized)?;
    let path = url.path().trim_end_matches('/').to_owned();
    if let Some(suffix) = KNOWN_COMPAT_SUFFIXES
        .iter()
        .find(|suffix| path.ends_with(**suffix))
    {
        let stripped = &path[..path.len() - suffix.len()];
        url.set_path(if stripped.is_empty() { "/" } else { stripped });
        let fallback = protocol_models_endpoint(url, api)?;
        if !candidates.contains(&fallback) {
            candidates.push(fallback);
        }
    }
    Ok(candidates)
}

fn protocol_models_endpoint(mut url: url::Url, api: ModelApi) -> anyhow::Result<String> {
    let has_version = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        == Some(api.root_version());
    let mut segments = url
        .path_segments_mut()
        .map_err(|()| anyhow::anyhow!("compatible base URL cannot accept path segments"))?;
    segments.pop_if_empty();
    if !has_version {
        segments.push(api.root_version());
    }
    segments.push("models");
    drop(segments);
    Ok(url.into())
}

/// Probe `OpenAI`, `Anthropic`, and `Google` response shapes in parallel.
///
/// Every protocol is isolated: one failed request becomes a warning in the returned report and
/// does not discard models found by the other protocols.
#[must_use]
pub fn discover_models(base_url: &str, token: &str) -> DiscoveryReport {
    discover_models_with_options(base_url, token, DiscoveryOptions::default())
}

/// Probe all supported Models APIs with explicit limits, primarily for deterministic callers.
#[must_use]
pub fn discover_models_with_options(
    base_url: &str,
    token: &str,
    options: DiscoveryOptions,
) -> DiscoveryReport {
    // Scoped threads borrow the credential, so discovery never creates a long-lived token copy.
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(API_COUNT);
        for api in ModelApi::all() {
            let Ok(endpoints) = models_endpoint_candidates(base_url, api) else {
                handles.push((api, None, None));
                continue;
            };
            let diagnostic_endpoint = endpoints[0].clone();
            handles.push((
                api,
                Some(diagnostic_endpoint),
                Some(scope.spawn(move || request_models(api, &endpoints, token, options))),
            ));
        }
        handles
            .into_iter()
            .map(|(api, endpoint, handle)| {
                let Some(handle) = handle else {
                    let diagnostic =
                        DiscoveryDiagnostic::new(api, "[invalid Models endpoint]".to_owned());
                    return (Err("invalid Models endpoint".to_owned()), diagnostic);
                };
                handle.join().unwrap_or_else(|_| {
                    let diagnostic = DiscoveryDiagnostic::new(
                        api,
                        endpoint.unwrap_or_else(|| "[unknown Models endpoint]".to_owned()),
                    );
                    (
                        Err("model scan worker stopped unexpectedly".to_owned()),
                        diagnostic,
                    )
                })
            })
            .collect::<Vec<_>>()
    });

    merge_results(results)
}

fn request_models(
    api: ModelApi,
    endpoints: &[String],
    token: &str,
    options: DiscoveryOptions,
) -> (Result<Vec<RemoteModel>, String>, DiscoveryDiagnostic) {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(options.timeout))
        .timeout_connect(Some(options.timeout))
        // Authentication headers must never follow a redirect to a different origin.
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into();
    let mut preceding_attempts = 0;
    for (index, endpoint) in endpoints.iter().enumerate() {
        let (result, mut diagnostic) =
            request_models_at_endpoint(api, endpoint, token, options, &agent);
        diagnostic.attempts += preceding_attempts;
        if result
            .as_ref()
            .is_err_and(|reason| matches!(reason.as_str(), "HTTP 404" | "HTTP 405"))
            && index + 1 < endpoints.len()
        {
            preceding_attempts = diagnostic.attempts;
            continue;
        }
        return (result, diagnostic);
    }
    unreachable!("endpoint candidate lists always contain a primary endpoint")
}

fn request_models_at_endpoint(
    api: ModelApi,
    endpoint: &str,
    token: &str,
    options: DiscoveryOptions,
    agent: &Agent,
) -> (Result<Vec<RemoteModel>, String>, DiscoveryDiagnostic) {
    let max_attempts = options.max_attempts.max(1);
    let mut backoff = options.initial_backoff;
    for attempt in 1..=max_attempts {
        let (result, mut diagnostic) = request_models_once(api, endpoint, token, options, agent);
        diagnostic.attempts = attempt;
        match result {
            Ok(models) => return (Ok(models), diagnostic),
            Err(RequestFailure::Retryable(_)) if attempt < max_attempts => {
                if !backoff.is_zero() {
                    std::thread::sleep(backoff);
                    backoff = backoff.saturating_mul(2);
                }
            }
            Err(error) => {
                let mut reason = error.reason();
                if attempt > 1 {
                    reason = format!("{reason} after {attempt} attempts");
                }
                return (Err(reason), diagnostic);
            }
        }
    }
    unreachable!("at least one discovery attempt is always made")
}

fn request_models_once(
    api: ModelApi,
    endpoint: &str,
    token: &str,
    options: DiscoveryOptions,
    agent: &Agent,
) -> (
    Result<Vec<RemoteModel>, RequestFailure>,
    DiscoveryDiagnostic,
) {
    let mut diagnostic = DiscoveryDiagnostic::new(api, endpoint.to_owned());
    let request = agent.get(endpoint).header("Accept", "application/json");
    let response = match api {
        ModelApi::OpenAi => {
            let bearer = Zeroizing::new(format!("Bearer {token}"));
            request.header("Authorization", bearer.as_str()).call()
        }
        ModelApi::Anthropic => request
            .header("x-api-key", token)
            .header("anthropic-version", "2023-06-01")
            .call(),
        ModelApi::Google => request.header("x-goog-api-key", token).call(),
    };
    let mut response = match response {
        Ok(response) => response,
        Err(error) => return (Err(request_error(&error)), diagnostic),
    };
    let status = response.status();
    diagnostic.status = Some(status.as_u16());
    diagnostic.content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(|value| sanitize_header(value, token));
    diagnostic.location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .map(|value| sanitize_location(value, token));
    diagnostic.response_bytes = response
        .body()
        .content_length()
        .and_then(|length| usize::try_from(length).ok());
    if !status.is_success() {
        let reason = if status.is_redirection() {
            format!("HTTP {} redirect", status.as_u16())
        } else {
            format!("HTTP {}", status.as_u16())
        };
        let error = if status.as_u16() == 429 || status.is_server_error() {
            RequestFailure::Retryable(reason)
        } else {
            RequestFailure::Final(reason)
        };
        return (Err(error), diagnostic);
    }

    if response
        .body()
        .content_length()
        .is_some_and(|length| length > u64::try_from(options.max_body_bytes).unwrap_or(u64::MAX))
    {
        return (
            Err(RequestFailure::Final(body_limit_reason(
                options.max_body_bytes,
            ))),
            diagnostic,
        );
    }
    // Read one extra byte so close-delimited and chunked bodies cannot silently truncate at limit.
    let body = match response
        .body_mut()
        .with_config()
        .limit(u64::try_from(options.max_body_bytes.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_vec()
    {
        Ok(body) => body,
        Err(error) => {
            return (
                Err(body_read_error(&error, options.max_body_bytes)),
                diagnostic,
            );
        }
    };
    diagnostic.response_bytes = Some(body.len());
    if body.len() > options.max_body_bytes {
        return (
            Err(RequestFailure::Final(body_limit_reason(
                options.max_body_bytes,
            ))),
            diagnostic,
        );
    }
    (
        parse_remote_models(api, &body).map_err(RequestFailure::Final),
        diagnostic,
    )
}

fn request_error(error: &UreqError) -> RequestFailure {
    match error {
        UreqError::StatusCode(status) if *status == 429 || *status >= 500 => {
            RequestFailure::Retryable(format!("HTTP {status}"))
        }
        UreqError::StatusCode(status) => RequestFailure::Final(format!("HTTP {status}")),
        UreqError::Timeout(_) => RequestFailure::Retryable("request timed out".to_owned()),
        UreqError::HostNotFound => RequestFailure::Retryable("host was not found".to_owned()),
        UreqError::TooManyRedirects | UreqError::RedirectFailed => {
            RequestFailure::Final("redirected Models endpoint".to_owned())
        }
        _ => RequestFailure::Retryable("request failed".to_owned()),
    }
}

fn sanitize_header(value: &str, token: &str) -> String {
    redact(value, token)
        .chars()
        .filter(|character| !character.is_control())
        .take(160)
        .collect()
}

fn sanitize_location(value: &str, token: &str) -> String {
    let without_query = value.split(['?', '#']).next().unwrap_or_default();
    sanitize_header(without_query, token)
}

fn body_limit_reason(limit: usize) -> String {
    if limit == DEFAULT_MAX_BODY_BYTES {
        "response exceeded the 1 MiB limit".to_owned()
    } else {
        format!("response exceeded the {limit}-byte limit")
    }
}

fn body_read_error(error: &UreqError, limit: usize) -> RequestFailure {
    if matches!(error, UreqError::BodyExceedsLimit(_)) {
        RequestFailure::Final(body_limit_reason(limit))
    } else {
        RequestFailure::Retryable("could not read model response".to_owned())
    }
}

/// Parse one provider's response schema without consulting the network.
///
/// # Errors
///
/// Returns a credential-free compatibility error for malformed JSON or a mismatched schema.
pub fn parse_models_response(api: ModelApi, body: &[u8]) -> Result<Vec<DiscoveredModel>, String> {
    parse_remote_models(api, body).map(|models| {
        models
            .into_iter()
            .map(|model| DiscoveredModel {
                id: model.id,
                display_name: model.display_name,
                sources: vec![api],
            })
            .collect()
    })
}

fn parse_remote_models(api: ModelApi, body: &[u8]) -> Result<Vec<RemoteModel>, String> {
    let root: ModelResponse =
        serde_json::from_slice(body).map_err(|_| "response was not valid JSON".to_owned())?;
    let records = match api {
        ModelApi::OpenAi | ModelApi::Anthropic => root.data,
        ModelApi::Google => root.models,
    }
    .ok_or_else(|| format!("response did not match the {api} Models schema"))?;

    let mut models = Vec::new();
    for record in records {
        let Some(raw_id) = record.id.or(record.name) else {
            continue;
        };
        let id = if matches!(api, ModelApi::Google) {
            raw_id.strip_prefix("models/").unwrap_or(&raw_id)
        } else {
            &raw_id
        };
        let id = id.trim();
        if id.is_empty() || id.chars().any(char::is_control) {
            continue;
        }
        let display_name = record
            .display_name
            .filter(|name| !name.trim().is_empty() && !name.chars().any(char::is_control));
        models.push(RemoteModel {
            id: id.to_owned(),
            display_name,
        });
    }
    Ok(models)
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    data: Option<Vec<ModelRecord>>,
    models: Option<Vec<ModelRecord>>,
}

#[derive(Debug, Deserialize)]
struct ModelRecord {
    id: Option<String>,
    name: Option<String>,
    #[serde(alias = "displayName")]
    display_name: Option<String>,
}

fn merge_results(
    results: Vec<(Result<Vec<RemoteModel>, String>, DiscoveryDiagnostic)>,
) -> DiscoveryReport {
    let mut merged = BTreeMap::<String, DiscoveredModel>::new();
    let mut successful_apis = Vec::new();
    let mut failures = Vec::new();
    let mut diagnostics = Vec::with_capacity(results.len());
    for (result, diagnostic) in results {
        let api = diagnostic.api;
        diagnostics.push(diagnostic);
        match result {
            Ok(models) => {
                successful_apis.push(api);
                for model in models {
                    let entry = merged.entry(model.id.clone()).or_insert(DiscoveredModel {
                        id: model.id,
                        display_name: model.display_name.clone(),
                        sources: Vec::new(),
                    });
                    if entry.display_name.is_none() {
                        entry.display_name = model.display_name;
                    }
                    if !entry.sources.contains(&api) {
                        entry.sources.push(api);
                    }
                }
            }
            Err(reason) => failures.push(DiscoveryFailure { api, reason }),
        }
    }
    let mut models = merged.into_values().collect::<Vec<_>>();
    for model in &mut models {
        model.sources.sort_unstable();
    }
    // Catalog order keeps familiar models stable; unknown remote IDs follow alphabetically.
    models.sort_by(|left, right| {
        let left_known = find_model(&left.id).is_some();
        let right_known = find_model(&right.id).is_some();
        right_known
            .cmp(&left_known)
            .then_with(|| {
                left.id
                    .to_ascii_lowercase()
                    .cmp(&right.id.to_ascii_lowercase())
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    successful_apis.sort_unstable();
    failures.sort_by_key(|failure| failure.api);
    diagnostics.sort_by_key(|diagnostic| diagnostic.api);
    DiscoveryReport {
        models,
        successful_apis,
        failures,
        diagnostics,
    }
}
