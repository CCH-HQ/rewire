use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=REWIRE_GIT_COMMIT");
    println!("cargo:rerun-if-changed=.git/HEAD");
    let commit = env::var("REWIRE_GIT_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(git_commit);
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    println!("cargo:rustc-env=REWIRE_GIT_COMMIT={}", one_line(&commit));
    println!("cargo:rustc-env=REWIRE_BUILD_TARGET={}", one_line(&target));
}

fn git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}
