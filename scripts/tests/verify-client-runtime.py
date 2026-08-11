#!/usr/bin/env python3
"""Verify real client calls and reject credentials in every captured runtime artifact."""

from __future__ import annotations

import json
from pathlib import Path


MARKER = "REWIRE_E2E_OK"
MODEL = "claude-sonnet-4-6"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path.name} must contain a JSON object")
    return value


def nested(value: object, *keys: str) -> object:
    for key in keys:
        require(isinstance(value, dict) and key in value, f"missing {'/'.join(keys)}")
        value = value[key]
    return value


def verify_versions(logs: Path) -> None:
    expected = {
        "claude.version": "2.1.186",
        "codex.version": "codex-cli 0.147.0",
        "opencode.version": "1.18.16",
        "hermes.version": "Hermes Agent v0.19.0",
        "openclaw.version": "OpenClaw 2026.7.1-2",
    }
    for name, fragment in expected.items():
        require(fragment in (logs / name).read_text(encoding="utf-8"), f"unexpected {name}")


def verify_claude(logs: Path) -> None:
    result = read_json(logs / "claude.stdout")
    require(result.get("subtype") == "success", "Claude did not report success")
    require(result.get("is_error") is False, "Claude reported a model error")
    require(result.get("result") == MARKER, "Claude returned an unexpected response")


def verify_codex(logs: Path) -> None:
    require(
        (logs / "codex.last").read_text(encoding="utf-8").strip() == MARKER,
        "Codex returned an unexpected response",
    )


def verify_opencode(logs: Path) -> None:
    texts: list[str] = []
    for line in (logs / "opencode.stdout").read_text(encoding="utf-8").splitlines():
        if not line.strip().startswith("{"):
            continue
        event = json.loads(line)
        if isinstance(event, dict) and event.get("type") == "text":
            text = nested(event, "part", "text")
            if isinstance(text, str):
                texts.append(text)
    require(MARKER in texts, "OpenCode returned an unexpected response")


def verify_hermes(logs: Path) -> None:
    require(
        (logs / "hermes.stdout").read_text(encoding="utf-8").strip() == MARKER,
        "Hermes returned an unexpected response",
    )
    usage = read_json(logs / "hermes-usage.json")
    require(usage.get("completed") is True, "Hermes did not complete its model call")
    require(usage.get("failed") is False, "Hermes marked its model call as failed")
    require(usage.get("model") == MODEL, "Hermes used an unexpected model")


def verify_openclaw(logs: Path) -> None:
    result = read_json(logs / "openclaw.stdout")
    require(result.get("status") == "ok", "OpenClaw did not report success")
    payloads = nested(result, "result", "payloads")
    require(isinstance(payloads, list), "OpenClaw omitted response payloads")
    require(
        any(isinstance(payload, dict) and payload.get("text") == MARKER for payload in payloads),
        "OpenClaw returned an unexpected response",
    )
    agent = nested(result, "result", "meta", "agentMeta")
    require(nested(agent, "provider") == "rewire", "OpenClaw bypassed the Rewire provider")
    require(nested(agent, "model") == MODEL, "OpenClaw used an unexpected model")


def main() -> None:
    logs = Path("/e2e/logs")
    token = Path("/run/rewire/key").read_bytes().rstrip(b"\r\n")
    require(token != b"", "API token is empty")
    for path in logs.rglob("*"):
        if path.is_file():
            require(token not in path.read_bytes(), f"API token leaked into {path.name}")

    verify_versions(logs)
    verify_claude(logs)
    verify_codex(logs)
    verify_opencode(logs)
    verify_hermes(logs)
    verify_openclaw(logs)
    print("Verified real Claude, Codex, OpenCode, Hermes, and OpenClaw model calls.")


if __name__ == "__main__":
    main()
