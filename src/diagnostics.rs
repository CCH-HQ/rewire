use crate::{Client, ClientDiagnostic, DoctorReport, clients::CLIENTS};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const VERSION_TIMEOUT: Duration = Duration::from_secs(2);

/// Inspect configuration, executable, version, and environment state without exposing values.
#[must_use]
pub fn diagnose(home: &Path) -> DoctorReport {
    let clients = CLIENTS
        .iter()
        .copied()
        .map(|client| diagnose_client(home, client))
        .collect::<Vec<_>>();
    let detected = clients
        .iter()
        .filter(|diagnostic| diagnostic.configuration_detected)
        .map(|diagnostic| diagnostic.client)
        .collect();
    DoctorReport {
        ok: true,
        home: home.to_path_buf(),
        detected,
        clients,
    }
}

fn diagnose_client(home: &Path, client: Client) -> ClientDiagnostic {
    let config_path = client
        .recipes(home, "https://example.invalid", "TOKEN", None)
        .first()
        .expect("every supported client has a primary configuration recipe")
        .path
        .clone();
    let executable = find_executable(home, client);
    let version = executable
        .as_deref()
        .and_then(|path| read_version(path, VERSION_TIMEOUT));
    ClientDiagnostic {
        client,
        configuration_detected: config_path.exists(),
        config_path,
        installed: executable.is_some(),
        executable,
        version,
        environment: environment_names(client),
    }
}

fn environment_names(client: Client) -> Vec<String> {
    client_environment_names(client)
        .iter()
        .filter(|name| env::var_os(name).is_some_and(|value| !value.is_empty()))
        .map(|name| (*name).to_owned())
        .collect()
}

fn client_environment_names(client: Client) -> &'static [&'static str] {
    match client {
        Client::Claude => &[
            "CLAUDE_CONFIG_DIR",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ],
        Client::Codex => &["CODEX_HOME", "OPENAI_API_KEY"],
        Client::OpenCode => &["OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR", "XDG_CONFIG_HOME"],
        Client::Hermes => &["HERMES_HOME", "REWIRE_TOKEN"],
        Client::OpenClaw => &["OPENCLAW_CONFIG_PATH", "OPENCLAW_STATE_DIR"],
    }
}

fn find_executable(home: &Path, client: Client) -> Option<PathBuf> {
    let name = client.name();
    let from_path = env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .flat_map(|directory| executable_names(name).map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file());
    from_path.or_else(|| {
        fallback_executables(home, client)
            .into_iter()
            .find(|candidate| candidate.is_file())
    })
}

fn executable_names(name: &str) -> impl Iterator<Item = String> + '_ {
    std::iter::once(name.to_owned()).chain(cfg!(windows).then(|| format!("{name}.exe")))
}

fn fallback_executables(home: &Path, client: Client) -> Vec<PathBuf> {
    let mut paths = vec![home.join(".local/bin").join(client.name())];
    match client {
        Client::OpenCode => paths.push(home.join(".opencode/bin/opencode")),
        Client::Hermes => paths.push(home.join(".hermes/bin/hermes")),
        Client::Claude | Client::Codex | Client::OpenClaw => {}
    }
    paths
}

fn read_version(executable: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        if child.try_wait().ok().flatten().is_some() {
            let output = child.wait_with_output().ok()?;
            let text = if output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr)
            } else {
                String::from_utf8_lossy(&output.stdout)
            };
            return text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| {
                    // Version commands should be one line; cap hostile or accidental output.
                    line.chars().take(200).collect()
                });
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}
