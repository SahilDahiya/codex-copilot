#!/usr/bin/env python3
"""Personal Copilot login and launcher for the locally built Codex binary.

Device authorization and token exchange follow hk-vk/codexpilot (Apache-2.0):
https://github.com/hk-vk/codexpilot/blob/64a15f525cf054b8ceb37f55ad31a2e432707ede/codex-rs/login/src/github_copilot.rs
"""

import json
import os
from pathlib import Path
import shlex
import shutil
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


CLIENT_ID = "Iv1.b507a08c87ecfe98"
HEADERS = {
    "Accept": "application/json",
    "User-Agent": "GitHubCopilotChat/0.35.0",
    "Editor-Version": "vscode/1.107.0",
    "Editor-Plugin-Version": "copilot-chat/0.35.0",
    "Copilot-Integration-Id": "vscode-chat",
}
APP_HOME = Path.home() / ".codex-copilot"


def request_json(url, *, token=None, form=None):
    headers = dict(HEADERS)
    if token:
        headers["Authorization"] = f"Bearer {token}"
    data = None
    if form is not None:
        data = urllib.parse.urlencode(form).encode()
        headers["Content-Type"] = "application/x-www-form-urlencoded"
    request = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        # Do not echo server bodies, which may contain authentication material.
        raise RuntimeError(
            f"GitHub returned HTTP {error.code} for {urllib.parse.urlsplit(url).path}"
        ) from None


def save_private(path, text):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor, temporary = tempfile.mkstemp(dir=path.parent, prefix=".copilot-")
    try:
        with os.fdopen(descriptor, "w") as output:
            output.write(text)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def api_base_url(token):
    for part in token.split(";"):
        if part.startswith("proxy-ep="):
            host = part.removeprefix("proxy-ep=")
            if host.startswith("proxy."):
                host = "api." + host.removeprefix("proxy.")
            elif not host.startswith("api."):
                host = "api." + host
            # Only send subscription credentials to GitHub's Copilot service.
            if not host.endswith(".githubcopilot.com") or any(
                c in host for c in "/:@?#"
            ):
                raise RuntimeError("GitHub returned an unexpected Copilot API host")
            return "https://" + host
    return "https://api.githubcopilot.com"


def exchange_token(github_token):
    result = request_json(
        "https://api.github.com/copilot_internal/v2/token", token=github_token
    )
    if not isinstance(result.get("token"), str) or not result["token"]:
        raise RuntimeError(
            "GitHub did not return a Copilot token; check your subscription"
        )
    return result["token"], api_base_url(result["token"])


def load_auth(home):
    path = home / "github-copilot-auth.json"
    if not path.exists():
        raise RuntimeError("Sign in first: codex-copilot login")
    return json.loads(path.read_text())


def login(home):
    device = request_json(
        "https://github.com/login/device/code",
        form={"client_id": CLIENT_ID, "scope": "read:user"},
    )
    print(
        f"Open {device['verification_uri']} and enter code: {device['user_code']}",
        flush=True,
    )
    interval = max(device.get("interval", 5), 1)
    deadline = time.monotonic() + min(device.get("expires_in", 900), 900)
    while time.monotonic() < deadline:
        time.sleep(interval)
        result = request_json(
            "https://github.com/login/oauth/access_token",
            form={
                "client_id": CLIENT_ID,
                "device_code": device["device_code"],
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            },
        )
        if result.get("access_token"):
            _, base_url = exchange_token(result["access_token"])
            save_private(
                home / "github-copilot-auth.json",
                json.dumps(
                    {
                        "github_access_token": result["access_token"],
                        "api_base_url": base_url,
                    }
                ),
            )
            print("GitHub Copilot authentication saved.", flush=True)
            return
        error = result.get("error")
        if error == "slow_down":
            interval = max(interval + 5, result.get("interval", 0))
        elif error != "authorization_pending":
            raise RuntimeError(f"GitHub authorization failed: {error}")
    raise RuntimeError("GitHub authorization expired; run login again")


def available_models(home):
    token, base_url = exchange_token(load_auth(home)["github_access_token"])
    result = request_json(base_url + "/models", token=token)
    return [
        model["id"]
        for model in result.get("data", [])
        if "/responses" in model.get("supported_endpoints", [])
    ]


def configure(home, model):
    models = available_models(home)
    if model not in models:
        raise RuntimeError(
            f"Choose a Responses-compatible model from: {', '.join(models)}"
        )
    auth = load_auth(home)
    catalog = json.loads((home / "bundled-models.json").read_text())
    catalog["models"] = [
        entry for entry in catalog["models"] if entry["slug"] in models
    ]
    if not any(entry["slug"] == model for entry in catalog["models"]):
        raise RuntimeError(
            "This build has no metadata for that model; choose another model"
        )
    save_private(home / "models.json", json.dumps(catalog))
    # This file is managed separately so normal Codex settings remain editable.
    helper = str(Path(__file__).resolve())
    settings = f"""model = {json.dumps(model)}
model_provider = "github_copilot"
model_catalog_json = {json.dumps(str(home / "models.json"))}
approval_policy = "on-request"
approvals_reviewer = "user"
web_search = "disabled"

[features]
image_generation = false

[model_providers.github_copilot]
name = "GitHub Copilot"
base_url = {json.dumps(auth["api_base_url"])}
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false

[model_providers.github_copilot.http_headers]
"User-Agent" = "GitHubCopilotChat/0.35.0"
"Editor-Version" = "vscode/1.107.0"
"Editor-Plugin-Version" = "copilot-chat/0.35.0"
"Copilot-Integration-Id" = "vscode-chat"

[model_providers.github_copilot.auth]
command = {json.dumps(sys.executable)}
args = [{json.dumps(helper)}, "token"]
cwd = {json.dumps(str(home))}
timeout_ms = 45000
refresh_interval_ms = 300000
"""
    config = home / "config.toml"
    if config.exists():
        save_private(home / "config.toml.backup", config.read_text())
    save_private(config, settings)
    print(f"Configured {model}. Previous config, if any, saved as config.toml.backup.")


def copilot_route(home):
    """Build the only inference provider permitted by this launcher."""
    base_url = load_auth(home)["api_base_url"]
    url = urllib.parse.urlsplit(base_url)
    host = url.hostname or ""
    if not (
        url.scheme == "https"
        and url.netloc == host
        and (host == "api.githubcopilot.com" or host.endswith(".githubcopilot.com"))
        and url.path in ("", "/")
        and not url.query
        and not url.fragment
    ):
        raise RuntimeError("Refusing a non-Copilot inference endpoint; sign in again")
    return {
        "name": "GitHub Copilot",
        "base_url": base_url,
        "wire_api": "responses",
        "requires_openai_auth": False,
        "supports_websockets": False,
        "http_headers": HEADERS,
        "auth": {
            "command": sys.executable,
            "args": [str(Path(__file__).resolve()), "token"],
            "cwd": str(home),
            "timeout_ms": 45000,
            "refresh_interval_ms": 300000,
        },
    }


def toml_value(value):
    if isinstance(value, dict):
        return (
            "{ "
            + ", ".join(
                f"{json.dumps(key)} = {toml_value(item)}" for key, item in value.items()
            )
            + " }"
        )
    return json.dumps(value)


def locked_launch_args(home, args):
    """Pin the provider above config/profile settings and reject CLI route changes."""
    if args in (["--help"], ["--version"]):
        return args
    pending_config = False
    for arg in args:
        if arg == "--" and not pending_config:
            break
        override = None
        if pending_config:
            override = arg
            pending_config = False
        elif arg in ("-c", "--config"):
            pending_config = True
        elif arg.startswith("--config="):
            override = arg.removeprefix("--config=")
        elif arg.startswith("-c"):
            override = arg[2:].lstrip("=")
        elif arg.split("=", 1)[0] in ("--oss", "--local-provider", "--remote"):
            raise RuntimeError(
                "codex-copilot only supports local sessions using GitHub Copilot"
            )
        if override is not None:
            key = override.split("=", 1)[0].replace('"', "").replace("'", "").strip()
            if "\\" in key or key.split(".", 1)[0].strip() in (
                "model_provider",
                "model_providers",
            ):
                raise RuntimeError("The model provider is locked to GitHub Copilot")
    return [
        "-c",
        'model_provider="github_copilot"',
        "-c",
        "model_providers.github_copilot=" + toml_value(copilot_route(home)),
        *args,
    ]


def install(home):
    root = Path(__file__).resolve().parents[1]
    source = root / "codex-rs" / "target" / "dev-small" / "codex"
    if not source.is_file():
        raise RuntimeError(
            "Build first: cd codex-rs && cargo build --profile dev-small -p codex-cli --bin codex"
        )
    (home / "bin").mkdir(parents=True, exist_ok=True, mode=0o700)
    staged = home / "bin" / "codex.new"
    shutil.copy2(source, staged)
    os.replace(staged, home / "bin" / "codex")
    helper = home / "copilot_local.py"
    save_private(helper, Path(__file__).read_text())
    shutil.copy2(
        root / "codex-rs" / "models-manager" / "models.json",
        home / "bundled-models.json",
    )
    command = Path.home() / ".local" / "bin" / "codex-copilot"
    save_private(
        command,
        f'#!/bin/sh\nexec {shlex.quote(sys.executable)} {shlex.quote(str(helper))} "$@"\n',
    )
    command.chmod(0o755)
    print(f"Installed {command}")


def main():
    args = sys.argv[1:]
    home = APP_HOME
    if args == ["install"]:
        install(home)
    elif args == ["login"]:
        login(home)
        if (
            not (home / "config.toml").exists()
            and (home / "bundled-models.json").exists()
        ):
            available = available_models(home)
            catalog = json.loads((home / "bundled-models.json").read_text())
            candidates = [
                entry["slug"]
                for entry in catalog["models"]
                if entry["slug"] in available
            ]
            if not candidates:
                raise RuntimeError(
                    "No compatible models are enabled for this subscription"
                )
            configure(home, "gpt-5.4" if "gpt-5.4" in candidates else candidates[0])
    elif args == ["token"]:
        # Always exchange here: Codex caches for five minutes and invokes this
        # again after a 401. Reusing our own cache would defeat that recovery.
        auth = load_auth(home)
        token, base_url = exchange_token(auth["github_access_token"])
        if base_url != auth["api_base_url"]:
            raise RuntimeError(
                "Copilot endpoint changed; run login and configure again"
            )
        print(token)
    elif args == ["status"]:
        route = copilot_route(home)
        print("Inference provider: GitHub Copilot (locked)")
        print("Inference endpoint: " + route["base_url"])
        print("Authentication: GitHub token exchange; no OpenAI subscription fallback")
    elif args == ["models"]:
        print("\n".join(available_models(home)))
    elif len(args) == 2 and args[0] == "configure":
        configure(home, args[1])
    elif args == ["logout"]:
        (home / "github-copilot-auth.json").unlink(missing_ok=True)
        print("Local Copilot credentials removed.")
    else:
        binary = home / "bin" / "codex"
        if not binary.is_file():
            raise RuntimeError(f"Local Codex binary has not been installed at {binary}")
        if not (home / "config.toml").is_file() and args not in (
            ["--help"],
            ["--version"],
        ):
            raise RuntimeError(
                "Run login, models, then configure MODEL before starting Codex"
            )
        env = dict(os.environ, CODEX_HOME=str(home))
        os.execve(str(binary), [str(binary), *locked_launch_args(home, args)], env)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        print(f"Copilot: {error}", file=sys.stderr)
        sys.exit(1)
