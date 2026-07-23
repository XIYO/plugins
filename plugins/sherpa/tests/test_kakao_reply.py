from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path


PLUGIN_ROOT = Path(__file__).resolve().parents[1]
REPLY_SCRIPT = PLUGIN_ROOT / "scripts" / "kakao-reply.py"


FAKE_KAKAOCLI = """#!/usr/bin/env python3
import json
import os
import sys

chats = json.loads(os.environ["FAKE_KAKAO_CHATS"])
if sys.argv[1] == "chats":
    print(json.dumps(chats))
    raise SystemExit(0)
if sys.argv[1] == "send":
    with open(os.environ["FAKE_KAKAO_LOG"], "a", encoding="utf-8") as handle:
        handle.write(json.dumps(sys.argv[1:], ensure_ascii=False) + "\\n")
    print("sent")
    raise SystemExit(0)
raise SystemExit(2)
"""


class KakaoReplyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        root = Path(self.temporary.name)
        self.fake = root / "kakaocli"
        self.fake.write_text(FAKE_KAKAOCLI, encoding="utf-8")
        self.fake.chmod(stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
        self.state = root / "state"
        self.log = root / "send.log"
        self.environment = {
            **os.environ,
            "KAKAOCLI_BIN": str(self.fake),
            "SHERPA_KAKAO_REPLY_STATE": str(self.state),
            "FAKE_KAKAO_LOG": str(self.log),
            "FAKE_KAKAO_CHATS": json.dumps(
                [
                    {
                        "id": "chat-1",
                        "display_name": "Project Alpha",
                        "member_count": 2,
                    },
                    {
                        "id": "chat-2",
                        "display_name": "General",
                        "member_count": 4,
                    },
                ]
            ),
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_reply(
        self, *arguments: str, message: str = ""
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(REPLY_SCRIPT), *arguments],
            input=message,
            text=True,
            capture_output=True,
            env=self.environment,
            check=False,
        )

    def prepare(self, message: str = "Approved response") -> dict[str, object]:
        process = self.run_reply(
            "prepare",
            "--chat",
            "Project Alpha",
            message=message,
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        return json.loads(process.stdout)

    def test_prepare_and_send_are_bound_to_exact_preview(self) -> None:
        preview = self.prepare()
        self.assertEqual(preview["status"], "preview")
        self.assertEqual(preview["chat_name"], "Project Alpha")
        self.assertEqual(preview["message"], "Approved response")
        state_info = os.stat(self.state)
        token_info = os.stat(self.state / f"{preview['token']}.json")
        self.assertEqual(stat.S_IMODE(state_info.st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(token_info.st_mode), 0o600)

        send = self.run_reply(
            "send",
            "--token",
            str(preview["token"]),
            message="Approved response",
        )
        self.assertEqual(send.returncode, 0, send.stderr)
        self.assertEqual(json.loads(send.stdout)["status"], "dispatched")
        invocation = json.loads(self.log.read_text(encoding="utf-8").strip())
        self.assertEqual(invocation, ["send", "Project Alpha", "Approved response"])
        self.assertFalse((self.state / f"{preview['token']}.json").exists())

    def test_message_change_is_rejected_without_dispatch(self) -> None:
        preview = self.prepare()
        send = self.run_reply(
            "send",
            "--token",
            str(preview["token"]),
            message="Changed response",
        )
        self.assertNotEqual(send.returncode, 0)
        self.assertIn("differs from the confirmed preview", send.stderr)
        self.assertFalse(self.log.exists())

    def test_substring_collision_is_rejected(self) -> None:
        self.environment["FAKE_KAKAO_CHATS"] = json.dumps(
            [
                {"id": "chat-1", "display_name": "Project Alpha"},
                {"id": "chat-2", "display_name": "Project Alpha Archive"},
            ]
        )
        process = self.run_reply(
            "prepare",
            "--chat",
            "Project Alpha",
            message="Approved response",
        )
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("missing or ambiguous", process.stderr)
        self.assertFalse(self.log.exists())

    def test_token_is_single_use(self) -> None:
        preview = self.prepare()
        first = self.run_reply(
            "send",
            "--token",
            str(preview["token"]),
            message="Approved response",
        )
        second = self.run_reply(
            "send",
            "--token",
            str(preview["token"]),
            message="Approved response",
        )
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertNotEqual(second.returncode, 0)
        self.assertIn("missing or already used", second.stderr)

    def test_expired_token_is_rejected_and_removed(self) -> None:
        preview = self.prepare()
        path = self.state / f"{preview['token']}.json"
        record = json.loads(path.read_text(encoding="utf-8"))
        record["expires_at"] = int(time.time()) - 1
        path.write_text(json.dumps(record), encoding="utf-8")
        path.chmod(0o600)

        send = self.run_reply(
            "send",
            "--token",
            str(preview["token"]),
            message="Approved response",
        )
        self.assertNotEqual(send.returncode, 0)
        self.assertIn("expired", send.stderr)
        self.assertFalse(path.exists())
        self.assertFalse(self.log.exists())

    def test_cancel_removes_unused_token(self) -> None:
        preview = self.prepare()
        cancel = self.run_reply(
            "cancel",
            "--token",
            str(preview["token"]),
        )
        self.assertEqual(cancel.returncode, 0, cancel.stderr)
        self.assertTrue(json.loads(cancel.stdout)["existed"])
        self.assertFalse((self.state / f"{preview['token']}.json").exists())


if __name__ == "__main__":
    unittest.main()
