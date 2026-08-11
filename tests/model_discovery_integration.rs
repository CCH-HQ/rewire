use rewire::{
    DiscoveryOptions, ModelApi, discover_models_with_options, models_endpoint,
    parse_models_response,
};
use std::fmt::Write as _;
use std::io::{Read, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn parses_all_provider_schemas_and_normalizes_google_names() {
    let openai = parse_models_response(
        ModelApi::OpenAi,
        br#"{"data":[{"id":"gpt-5.5"},{"id":""},{"id":"bad\nmodel"}]}"#,
    )
    .unwrap();
    assert_eq!(openai[0].id, "gpt-5.5");
    assert_eq!(openai[0].sources, vec![ModelApi::OpenAi]);

    let anthropic = parse_models_response(
        ModelApi::Anthropic,
        br#"{"data":[{"id":"claude-sonnet-5","display_name":"Claude Sonnet 5"}]}"#,
    )
    .unwrap();
    assert_eq!(
        anthropic[0].display_name.as_deref(),
        Some("Claude Sonnet 5")
    );

    let google = parse_models_response(
        ModelApi::Google,
        br#"{"models":[{"name":"models/gemini-3-pro","displayName":"Gemini 3 Pro"}]}"#,
    )
    .unwrap();
    assert_eq!(google[0].id, "gemini-3-pro");
    assert_eq!(google[0].display_name.as_deref(), Some("Gemini 3 Pro"));
}

#[test]
fn models_endpoint_preserves_existing_gateway_path() {
    assert_eq!(
        models_endpoint("https://gateway.example", ModelApi::OpenAi).unwrap(),
        "https://gateway.example/v1/models"
    );
    assert_eq!(
        models_endpoint("https://gateway.example", ModelApi::Anthropic).unwrap(),
        "https://gateway.example/v1/models"
    );
    assert_eq!(
        models_endpoint("https://gateway.example", ModelApi::Google).unwrap(),
        "https://gateway.example/v1beta/models"
    );
    assert_eq!(
        models_endpoint("https://gateway.example/custom/v1/", ModelApi::Google).unwrap(),
        "https://gateway.example/custom/v1/models"
    );
    assert_eq!(
        models_endpoint("https://gateway.example/v1/models", ModelApi::OpenAi).unwrap(),
        "https://gateway.example/v1/models"
    );
}

#[test]
fn discovery_sends_protocol_headers_in_parallel_and_merges_duplicate_models() {
    let (base_url, server) = start_server(|request| {
        let request = request.to_ascii_lowercase();
        assert!(request.contains("accept: application/json"));
        if request.contains("authorization: bearer test-token") {
            assert!(request.starts_with("get /v1/models http/1.1"));
            response(200, r#"{"data":[{"id":"shared-model"},{"id":"gpt-5.5"}]}"#)
        } else if request.contains("x-api-key: test-token") {
            assert!(request.starts_with("get /v1/models http/1.1"));
            assert!(request.contains("anthropic-version: 2023-06-01"));
            response(
                200,
                r#"{"data":[{"id":"shared-model","display_name":"Shared"}]}"#,
            )
        } else {
            assert!(request.starts_with("get /v1beta/models http/1.1"));
            assert!(request.contains("x-goog-api-key: test-token"), "{request}");
            response(
                200,
                r#"{"models":[{"name":"models/shared-model"},{"name":"models/gemini-3-pro"}]}"#,
            )
        }
    });

    let started = Instant::now();
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        test_options(Duration::from_secs(2), 1024),
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    server.join().unwrap();

    assert_eq!(report.successful_api_count(), 3);
    assert!(report.failures.is_empty());
    assert_eq!(report.diagnostics.len(), 3);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.status == Some(200))
    );
    let shared = report
        .models
        .iter()
        .find(|model| model.id == "shared-model")
        .unwrap();
    assert_eq!(
        shared.sources,
        vec![ModelApi::OpenAi, ModelApi::Anthropic, ModelApi::Google]
    );
    assert_eq!(shared.display_name.as_deref(), Some("Shared"));
    assert!(report.models.iter().any(|model| model.id == "gemini-3-pro"));
}

#[test]
fn redirect_diagnostics_explain_login_pages_without_leaking_tokens() {
    let (base_url, server) = start_server(|_| {
        response_with_headers(
            307,
            &[("Location", "/login/TOP_SECRET?token=TOP_SECRET")],
            "redirect",
        )
    });
    let report = discover_models_with_options(
        &base_url,
        "TOP_SECRET",
        test_options(Duration::from_secs(2), 1024),
    );
    server.join().unwrap();
    assert!(report.models.is_empty());
    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason == "HTTP 307 redirect")
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            diagnostic.location.as_deref() == Some("/login/[REDACTED_TOKEN]")
        })
    );
    assert!(!format!("{report:?}").contains("TOP_SECRET"));
}

#[test]
fn http_statuses_and_schema_mismatches_are_credential_free_failures() {
    let (base_url, server) = start_server(|request| {
        let request = request.to_ascii_lowercase();
        if request.contains("authorization:") {
            response(401, "unauthorized")
        } else if request.contains("x-api-key:") {
            response(404, "missing")
        } else {
            response(200, r#"{"data":[]}"#)
        }
    });
    let report = discover_models_with_options(
        &base_url,
        "TOP_SECRET",
        test_options(Duration::from_secs(2), 1024),
    );
    server.join().unwrap();
    assert!(report.models.is_empty());
    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.reason == "HTTP 401")
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.reason == "HTTP 404")
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.reason.contains("Google Models schema"))
    );
    assert!(!format!("{report:?}").contains("TOP_SECRET"));
}

#[test]
fn failed_endpoints_do_not_discard_successful_results() {
    let (base_url, server) = start_server(|request| {
        let request = request.to_ascii_lowercase();
        if request.contains("authorization:") {
            response(200, r#"{"data":[{"id":"gpt-5.5"}]}"#)
        } else if request.contains("x-api-key:") {
            response(401, "unauthorized")
                .replace("Content-Type: application/json", "Content-Type: text/plain")
        } else {
            response(200, "not-json")
        }
    });
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        test_options(Duration::from_secs(2), 1024),
    );
    server.join().unwrap();
    assert_eq!(report.successful_api_count(), 1);
    assert_eq!(report.models.len(), 1);
    assert_eq!(report.failures.len(), 2);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.reason == "HTTP 401")
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.reason == "response was not valid JSON")
    );
}

#[test]
fn response_limit_timeout_and_error_text_do_not_leak_credentials() {
    let (base_url, server) = start_server(|_| response(200, &"x".repeat(256)));
    let report = discover_models_with_options(
        &base_url,
        "TOP_SECRET",
        test_options(Duration::from_secs(2), 64),
    );
    server.join().unwrap();
    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason.contains("64-byte limit"))
    );
    assert!(!format!("{report:?}").contains("TOP_SECRET"));
}

#[test]
fn response_limit_handles_close_delimited_bodies() {
    let (base_url, server) = start_server(|_| response_without_length(200, &"x".repeat(256)));
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        test_options(Duration::from_secs(2), 64),
    );
    server.join().unwrap();
    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason.contains("64-byte limit"))
    );
}

#[test]
fn timeout_is_applied_to_each_parallel_request() {
    let (base_url, server) = start_server(|_| {
        thread::sleep(Duration::from_millis(250));
        response(200, r#"{"data":[]}"#)
    });
    let started = Instant::now();
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        test_options(Duration::from_millis(40), 1024),
    );
    let elapsed = started.elapsed();
    server.join().unwrap();
    assert_eq!(report.failures.len(), 3);
    assert!(elapsed < Duration::from_millis(220));
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason == "request timed out"
                || failure.reason == "request failed")
    );
}

#[test]
fn response_read_failures_retry_per_api_then_recover() {
    let attempts = Arc::new(Mutex::new(std::collections::BTreeMap::<&str, usize>::new()));
    let handler_attempts = Arc::clone(&attempts);
    let (base_url, server) = start_server_requests(6, move |request| {
        let request = request.to_ascii_lowercase();
        let api = if request.contains("authorization:") {
            "openai"
        } else if request.contains("x-api-key:") {
            "anthropic"
        } else {
            "google"
        };
        let attempt = {
            let mut attempts = handler_attempts.lock().unwrap();
            let attempt = attempts.entry(api).or_default();
            *attempt += 1;
            *attempt
        };
        if attempt == 1 {
            truncated_response()
        } else if api == "google" {
            response(200, r#"{"models":[{"name":"models/recovered-google"}]}"#)
        } else {
            response(200, &format!(r#"{{"data":[{{"id":"recovered-{api}"}}]}}"#))
        }
    });
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        DiscoveryOptions {
            timeout: Duration::from_secs(1),
            max_body_bytes: 1024,
            max_attempts: 3,
            initial_backoff: Duration::from_millis(5),
        },
    );
    server.join().unwrap();

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.models.len(), 3);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.attempts == 2)
    );
}

#[test]
fn retryable_statuses_stop_at_the_configured_attempt_limit() {
    let (base_url, server) = start_server_requests(9, |_| response(503, "busy"));
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        DiscoveryOptions {
            timeout: Duration::from_secs(1),
            max_body_bytes: 1024,
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
        },
    );
    server.join().unwrap();

    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason == "HTTP 503 after 3 attempts")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.attempts == 3)
    );
}

#[test]
fn zero_attempt_limit_still_makes_one_bounded_request() {
    let (base_url, server) = start_server(|_| response(503, "busy"));
    let report = discover_models_with_options(
        &base_url,
        "test-token",
        DiscoveryOptions {
            timeout: Duration::from_secs(1),
            max_body_bytes: 1024,
            max_attempts: 0,
            initial_backoff: Duration::ZERO,
        },
    );
    server.join().unwrap();

    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.reason == "HTTP 503")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.attempts == 1)
    );
}

fn test_options(timeout: Duration, max_body_bytes: usize) -> DiscoveryOptions {
    DiscoveryOptions {
        timeout,
        max_body_bytes,
        max_attempts: 1,
        initial_backoff: Duration::ZERO,
    }
}

fn start_server<F>(handler: F) -> (String, thread::JoinHandle<()>)
where
    F: Fn(String) -> String + Send + Sync + 'static,
{
    start_server_requests(3, handler)
}

fn start_server_requests<F>(request_count: usize, handler: F) -> (String, thread::JoinHandle<()>)
where
    F: Fn(String) -> String + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handler = std::sync::Arc::new(handler);
    let server = thread::spawn(move || {
        for _ in 0..request_count {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let body = handler(request);
            stream.write_all(body.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}"), server)
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn response(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn response_with_headers(status: u16, headers: &[(&str, &str)], body: &str) -> String {
    let mut response = format!(
        "HTTP/1.1 {status} Response\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        write!(response, "{name}: {value}\r\n").unwrap();
    }
    response.push_str("\r\n");
    response.push_str(body);
    response
}

fn response_without_length(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}"
    )
}

fn truncated_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 64\r\nConnection: close\r\n\r\n{".to_owned()
}
