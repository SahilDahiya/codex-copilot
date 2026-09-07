import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import copilot_local as copilot


class CopilotLocalTests(unittest.TestCase):
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
