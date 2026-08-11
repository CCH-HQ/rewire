#!/usr/bin/env python3
"""Assert Docker installer/configuration E2E results without exposing credentials."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import tomllib
from urllib.error import HTTPError, URLError
from urllib.parse import urljoin
from urllib.request import Request, urlopen


MODEL_ID = "claude-sonnet-4-6"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def read_json(path: Path, *, allow_installer_prefix: bool = False) -> dict[str, object]:
    content = path.read_text(encoding="utf-8")
    if allow_installer_prefix:
        object_start = content.find("{")
        require(object_start >= 0, f"{path} omitted its JSON result")
        content = content[object_start:]
    value = json.loads(content)
    require(isinstance(value, dict), f"{path} must contain a JSON object")
    return value


def nested(value: object, *keys: str) -> object:
    for key in keys:
        require(
            isinstance(value, dict) and key in value,
            f"missing configuration field: {'/'.join(keys)}",
        )
        value = value[key]
    return value


def assert_private(path: Path) -> None:
    mode = stat.S_IMODE(path.stat().st_mode)
    require(mode & 0o077 == 0, f"credential file is accessible outside its owner: {path}")


def assert_no_token(paths: list[Path], token: bytes) -> None:
    for path in paths:
        if path.is_file():
            require(token not in path.read_bytes(), f"API token leaked into {path}")
        elif path.is_dir():
            for child in path.rglob("*"):
                if child.is_file():
                    require(token not in child.read_bytes(), f"API token leaked into {child}")


def verify_live_models(domain: str, token: str) -> None:
    endpoint = urljoin(f"{domain.rstrip('/')}/", "v1/models")
    request = Request(
        endpoint,
        headers={
            "Accept": "application/json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "rewire-docker-e2e",
        },
    )
    try:
        with urlopen(request, timeout=15) as response:
            require(response.status == 200, "Models endpoint did not return HTTP 200")
            payload = json.load(response)
    except (HTTPError, URLError, TimeoutError, json.JSONDecodeError) as error:
        raise AssertionError("authenticated /v1/models compatibility probe failed") from error

    entries = payload.get("data", payload.get("models", [])) if isinstance(payload, dict) else []
    require(isinstance(entries, list), "Models endpoint did not return a model list")
    model_ids = {
        entry.get("id", entry.get("name"))
        for entry in entries
        if isinstance(entry, dict)
    }
    require(MODEL_ID in model_ids, f"Models endpoint did not advertise {MODEL_ID}")
    print(f"Authenticated model probe found {len(model_ids)} models, including {MODEL_ID}.")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-api-probe", action="store_true")
    args = parser.parse_args()

    home = Path("/e2e/home")
    install = Path("/e2e/install")
    assets = Path("/e2e/assets")
    logs = Path("/e2e/logs")
    token_bytes = Path("/run/rewire/key").read_bytes().rstrip(b"\r\n")
    token = token_bytes.decode("utf-8")
    domain = Path("/run/rewire/domain").read_text(encoding="utf-8").strip().rstrip("/")
    require(token != "", "API token is empty")

    binary = install / "rewire"
    require(
        binary.is_file() and os.access(binary, os.X_OK),
        "installed Rewire binary is not executable",
    )

    apply_output = read_json(logs / "apply.stdout", allow_installer_prefix=True)
    require(apply_output.get("ok") is True, "apply output did not report success")
    transaction = apply_output.get("transaction")
    require(
        isinstance(transaction, dict) and transaction.get("id"),
        "apply output omitted its transaction ID",
    )

    plan = read_json(logs / "idempotent.stdout")
    changes = plan.get("changes")
    require(
        isinstance(changes, list) and len(changes) == 8,
        "idempotency plan must cover all eight client files",
    )
    require(
        all(isinstance(change, dict) and change.get("action") == "noop" for change in changes),
        "second plan was not idempotent",
    )

    claude = read_json(home / ".claude/settings.json")
    require(
        nested(claude, "env", "ANTHROPIC_BASE_URL") == domain,
        "Claude base URL is incorrect",
    )
    require(
        nested(claude, "env", "ANTHROPIC_AUTH_TOKEN") == token,
        "Claude token is incorrect",
    )

    with (home / ".codex/config.toml").open("rb") as stream:
        codex = tomllib.load(stream)
    codex_provider = nested(codex, "model_providers", "rewire")
    require(nested(codex_provider, "base_url") == f"{domain}/v1", "Codex base URL is incorrect")
    require(
        nested(codex_provider, "experimental_bearer_token") == token,
        "Codex token is incorrect",
    )
    require(nested(codex_provider, "wire_api") == "responses", "Codex wire API is incorrect")

    opencode = read_json(home / ".config/opencode/opencode.jsonc")
    require(
        opencode.get("model") == f"anthropic/{MODEL_ID}",
        "OpenCode selected model is incorrect",
    )
    opencode_options = nested(opencode, "provider", "anthropic", "options")
    require(
        nested(opencode_options, "baseURL") == f"{domain}/v1",
        "OpenCode Anthropic base URL is incorrect",
    )
    require(
        str(nested(opencode_options, "apiKey")).startswith("{file:"),
        "OpenCode did not use a token file reference",
    )

    opencode_secret = home / ".config/rewire/secrets/opencode-token"
    require(opencode_secret.read_bytes() == token_bytes, "OpenCode token file is incorrect")
    assert_private(opencode_secret)

    hermes = (home / ".hermes/config.yaml").read_text(encoding="utf-8")
    for fragment in (f"default: {MODEL_ID}", f"api: {domain}", "key_env: REWIRE_TOKEN"):
        require(fragment in hermes, f"Hermes config omitted {fragment.split(':', 1)[0]}")
    hermes_env = home / ".hermes/.env"
    require(token_bytes in hermes_env.read_bytes(), "Hermes environment file omitted the API token")
    assert_private(hermes_env)

    openclaw = read_json(home / ".openclaw/openclaw.json")
    require(
        nested(openclaw, "agents", "defaults", "model", "primary")
        == f"rewire/{MODEL_ID}",
        "OpenClaw selected model is incorrect",
    )
    openclaw_provider = nested(openclaw, "models", "providers", "rewire")
    require(
        nested(openclaw_provider, "api") == "anthropic-messages",
        "OpenClaw API transport is incorrect",
    )
    require(
        nested(openclaw_provider, "baseUrl") == domain,
        "OpenClaw base URL is incorrect",
    )
    openclaw_secret = home / ".openclaw/secrets/rewire-token"
    require(openclaw_secret.read_bytes() == token_bytes, "OpenClaw token file is incorrect")
    assert_private(openclaw_secret)

    http_log = (logs / "http.log").read_text(encoding="utf-8")
    target = (assets / "TARGET").read_text(encoding="utf-8")
    archive = f"rewire-{target}.tar.gz"
    require(f"GET /{archive} " in http_log, "installer did not download the platform archive over HTTP")
    require("GET /SHA256SUMS " in http_log, "installer did not download SHA256SUMS over HTTP")

    transaction_root = home / ".local/state/rewire/transactions"
    # Client credential files intentionally contain the token. Public logs, distributable assets,
    # the installed binary, and encrypted transaction state must never contain it in plaintext.
    assert_no_token(
        [logs, assets, install, transaction_root],
        token_bytes,
    )

    if not args.skip_api_probe:
        verify_live_models(domain, token)
    print("Verified download, checksum, five client configs, idempotency, and credential boundaries.")


if __name__ == "__main__":
    main()
