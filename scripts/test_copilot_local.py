import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import copilot_local as copilot


class CopilotLocalTests(unittest.TestCase):
    def test_install_includes_code_mode_host(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / "scripts" / "copilot_local.py"
            helper.parent.mkdir()
            helper.write_text("# helper\n")
            build = root / "codex-rs" / "target" / "dev-small"
            build.mkdir(parents=True)
            binaries = {"codex": b"main", "codex-code-mode-host": b"host"}
            for name, contents in binaries.items():
                (build / name).write_bytes(contents)
                (build / name).chmod(0o755)
            catalog = root / "codex-rs" / "models-manager" / "models.json"
            catalog.parent.mkdir()
            catalog.write_text('{"models": []}')
            home = root / "installation"
            with (
                patch.object(copilot, "__file__", str(helper)),
                patch.object(Path, "home", return_value=root),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                copilot.install(home)
            self.assertEqual(
                {path.name: path.read_bytes() for path in (home / "bin").iterdir()},
                binaries,
            )
            self.assertTrue(
                (home / "bin" / "codex-code-mode-host").stat().st_mode & 0o100
            )

    def test_install_missing_host_preserves_existing_installation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build = root / "codex-rs" / "target" / "dev-small"
            build.mkdir(parents=True)
            (build / "codex").write_bytes(b"new")
            home = root / "installation"
            (home / "bin").mkdir(parents=True)
            (home / "bin" / "codex").write_bytes(b"existing")
            with (
                patch.object(
                    copilot, "__file__", str(root / "scripts" / "copilot_local.py")
                ),
                self.assertRaisesRegex(RuntimeError, "codex-code-mode-host"),
            ):
                copilot.install(home)
            self.assertEqual((home / "bin" / "codex").read_bytes(), b"existing")

    def test_login_handles_pending_and_slow_down_before_saving_credentials(self):
        responses = [
            {
                "verification_uri": "https://github.com/login/device",
                "user_code": "EXAMPLE",
                "device_code": "device",
                "interval": 1,
            },
            {"error": "authorization_pending"},
            {"error": "slow_down"},
            {"access_token": "github-secret"},
            {"token": "copilot-secret;proxy-ep=proxy.individual.githubcopilot.com"},
        ]
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            output = io.StringIO()
            with (
                patch.object(copilot, "request_json", side_effect=responses),
                patch.object(copilot.time, "sleep") as sleep,
                contextlib.redirect_stdout(output),
            ):
                copilot.login(home)
            self.assertEqual(
                [call.args for call in sleep.call_args_list], [(1,), (1,), (6,)]
            )
            self.assertEqual(
                copilot.load_auth(home),
                {
                    "github_access_token": "github-secret",
                    "api_base_url": "https://api.individual.githubcopilot.com",
                },
            )
            self.assertNotIn("github-secret", output.getvalue())
            self.assertNotIn("copilot-secret", output.getvalue())

    def test_unexpected_token_endpoint_is_rejected(self):
        for host in [
            "proxy.githubcopilot.com.evil.test",
            "api.githubcopilot.com@evil.test",
            "evil.test/api.githubcopilot.com",
        ]:
            with self.subTest(host=host), self.assertRaises(RuntimeError):
                copilot.api_base_url("proxy-ep=" + host)

    def test_models_excludes_backends_without_responses_support(self):
        with (
            patch.object(
                copilot, "load_auth", return_value={"github_access_token": "secret"}
            ),
            patch.object(
                copilot,
                "exchange_token",
                return_value=("token", "https://api.githubcopilot.com"),
            ),
            patch.object(
                copilot,
                "request_json",
                return_value={
                    "data": [
                        {
                            "id": "chat-only",
                            "supported_endpoints": ["/chat/completions"],
                        },
                        {
                            "id": "responses-model",
                            "supported_endpoints": ["/responses"],
                        },
                        {"id": "unknown"},
                    ]
                },
            ),
        ):
            self.assertEqual(
                copilot.available_models(Path("unused")), ["responses-model"]
            )

    def test_launcher_pins_copilot_and_preserves_model_selection(self):
        with patch.object(
            copilot,
            "load_auth",
            return_value={"api_base_url": "https://api.business.githubcopilot.com"},
        ):
            args = copilot.locked_launch_args(
                Path("private-home"), ["-m", "gpt-6-astra"]
            )
        self.assertEqual(args[:3], ["-c", 'model_provider="github_copilot"', "-c"])
        self.assertEqual(args[-2:], ["-m", "gpt-6-astra"])
        self.assertIn('"base_url" = "https://api.business.githubcopilot.com"', args[3])
        self.assertIn('"requires_openai_auth" = false', args[3])
        self.assertIn('"token"', args[3])
        self.assertNotIn("github_access_token", args[3])

    def test_launcher_rejects_provider_switches_before_loading_credentials(self):
        for args in [
            ["--oss"],
            ["--local-provider=ollama"],
            ["--remote", "ws://localhost:1234"],
            ["-c", 'model_provider="openai"'],
            ["--config=model_provider='openai'"],
            ["-c", r'"model_provi\u0064er"="openai"'],
            ["-cmodel_providers.github_copilot.base_url='https://api.openai.com'"],
            ["--config", '"model_providers".github_copilot.auth.command="other"'],
        ]:
            with self.subTest(args=args), patch.object(copilot, "load_auth") as auth:
                with self.assertRaises(RuntimeError):
                    copilot.locked_launch_args(Path("unused"), args)
                auth.assert_not_called()

    def test_launcher_does_not_parse_prompt_as_configuration(self):
        with patch.object(
            copilot,
            "load_auth",
            return_value={"api_base_url": "https://api.githubcopilot.com"},
        ):
            args = ["exec", "--", "--oss", "-c", "model_provider=openai"]
            self.assertEqual(copilot.locked_launch_args(Path("home"), args)[4:], args)

    def test_launcher_rejects_unexpected_saved_endpoints(self):
        for url in [
            "https://api.openai.com/v1",
            "http://api.githubcopilot.com",
            "https://api.githubcopilot.com.evil.test",
            "https://user@api.githubcopilot.com",
            "https://api.githubcopilot.com/proxy",
            "https://api.githubcopilot.com?redirect=evil",
        ]:
            with (
                self.subTest(url=url),
                patch.object(copilot, "load_auth", return_value={"api_base_url": url}),
            ):
                with self.assertRaises(RuntimeError):
                    copilot.copilot_route(Path("unused"))

    def test_private_write_replaces_file_and_restricts_permissions(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "auth.json"
            path.write_text("old secret")
            path.chmod(0o644)
            copilot.save_private(path, json.dumps({"token": "new secret"}))
            self.assertEqual(json.loads(path.read_text()), {"token": "new secret"})
            if copilot.os.name != "nt":
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(list(Path(directory).iterdir()), [path])


if __name__ == "__main__":
    unittest.main()
