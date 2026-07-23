#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


TOKEN_PATTERN = re.compile(r"^[0-9a-f]{32}$")
VALID_LOG_LEVELS = {"debug": 10, "info": 20, "warn": 30, "error": 40}
LOG_LEVEL = VALID_LOG_LEVELS.get(os.getenv("LOG_LEVEL", "warn").lower(), 30)
MAX_MESSAGE_LENGTH = 2_000
DEFAULT_TTL_SECONDS = 900
MAX_TTL_SECONDS = 1_800
CHAT_SCAN_LIMIT = 10_000


class ReplyError(RuntimeError):
    pass


def log(level: str, scope: str, message: str) -> None:
    if VALID_LOG_LEVELS[level] >= LOG_LEVEL:
        print(f"[{scope}:{level}] {message}", file=sys.stderr)


def resolve_kakaocli() -> str:
    configured = os.getenv("KAKAOCLI_BIN")
    if configured:
        candidate = Path(configured).expanduser()
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
        raise ReplyError("configured KakaoTalk sender is not executable")

    install_root = Path(
        os.getenv("SHERPA_INSTALL_ROOT", str(Path.home() / ".local"))
    ).expanduser()
    managed = install_root / "bin" / "kakaocli"
    if managed.is_file() and os.access(managed, os.X_OK):
        return str(managed)

    discovered = shutil.which("kakaocli")
    if discovered:
        return discovered
    raise ReplyError("kakaocli is required for KakaoTalk replies")


def state_directory() -> Path:
    configured = os.getenv("SHERPA_KAKAO_REPLY_STATE")
    if configured:
        return Path(configured).expanduser()
    state_root = Path(
        os.getenv("XDG_STATE_HOME", str(Path.home() / ".local" / "state"))
    ).expanduser()
    return state_root / "sherpa" / "kakao-reply"


def ensure_private_directory(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = os.lstat(path)
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        raise ReplyError("reply state path must be a real directory")
    if info.st_uid != os.getuid():
        raise ReplyError("reply state directory must be owned by the current user")
    if stat.S_IMODE(info.st_mode) & 0o077:
        raise ReplyError("reply state directory must have mode 0700")


def token_path(directory: Path, token: str) -> Path:
    if not TOKEN_PATTERN.fullmatch(token):
        raise ReplyError("invalid confirmation token")
    return directory / f"{token}.json"


def read_message() -> str:
    message = sys.stdin.read()
    if not message or not message.strip():
        raise ReplyError("reply message must be provided on standard input")
    if "\x00" in message:
        raise ReplyError("reply message must not contain NUL bytes")
    if len(message) > MAX_MESSAGE_LENGTH:
        raise ReplyError(
            f"reply message is too long; maximum={MAX_MESSAGE_LENGTH} characters"
        )
    return message


def load_chats(kakaocli: str) -> list[dict[str, Any]]:
    log("info", "reply:kakao:resolve", "Resolving the exact KakaoTalk chat")
    process = subprocess.run(
        [kakaocli, "chats", "--limit", str(CHAT_SCAN_LIMIT), "--json"],
        check=False,
        capture_output=True,
        text=True,
    )
    if process.returncode != 0:
        log(
            "error",
            "reply:kakao:resolve",
            f"Chat discovery failed; exit_code={process.returncode}",
        )
        raise ReplyError("unable to read KakaoTalk chats")
    try:
        payload = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        log("error", "reply:kakao:resolve", "Chat discovery returned invalid JSON")
        raise ReplyError("unable to parse KakaoTalk chat list") from error
    if not isinstance(payload, list):
        raise ReplyError("KakaoTalk chat list has an unexpected shape")
    return [item for item in payload if isinstance(item, dict)]


def resolve_exact_chat(kakaocli: str, requested_name: str) -> dict[str, Any]:
    if not requested_name or not requested_name.strip():
        raise ReplyError("an exact chat name is required")

    chats = load_chats(kakaocli)
    exact = [chat for chat in chats if chat.get("display_name") == requested_name]
    folded = requested_name.casefold()
    substring = [
        chat
        for chat in chats
        if isinstance(chat.get("display_name"), str)
        and folded in str(chat["display_name"]).casefold()
    ]
    if len(exact) != 1 or len(substring) != 1:
        log(
            "warn",
            "reply:kakao:resolve",
            (
                "Refusing an ambiguous target; "
                f"exact_matches={len(exact)} substring_matches={len(substring)}"
            ),
        )
        raise ReplyError(
            "chat target is missing or ambiguous; use a unique exact display name"
        )
    return exact[0]


def digest_message(message: str) -> str:
    return hashlib.sha256(message.encode("utf-8")).hexdigest()


def cleanup_expired(directory: Path, now: int) -> None:
    for path in directory.glob("*.json"):
        if not TOKEN_PATTERN.fullmatch(path.stem):
            continue
        try:
            info = os.lstat(path)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
                continue
            record = json.loads(path.read_text(encoding="utf-8"))
            expires_at = int(record.get("expires_at", 0))
        except (OSError, ValueError, TypeError, json.JSONDecodeError):
            continue
        if expires_at <= now:
            path.unlink(missing_ok=True)


def write_record(path: Path, record: dict[str, Any]) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(record, handle, ensure_ascii=False, separators=(",", ":"))
            handle.write("\n")
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def load_record(path: Path) -> dict[str, Any]:
    try:
        info = os.lstat(path)
    except FileNotFoundError as error:
        raise ReplyError("confirmation token is missing or already used") from error
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
        raise ReplyError("confirmation record is not a private regular file")
    if stat.S_IMODE(info.st_mode) & 0o077:
        raise ReplyError("confirmation record must have mode 0600")
    try:
        record = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReplyError("confirmation record is unreadable") from error
    if not isinstance(record, dict) or record.get("version") != 1:
        raise ReplyError("confirmation record has an unsupported format")
    return record


def prepare_reply(args: argparse.Namespace) -> int:
    message = read_message()
    kakaocli = resolve_kakaocli()
    chat = resolve_exact_chat(kakaocli, args.chat)
    chat_name = str(chat["display_name"])
    chat_id = str(chat.get("id", ""))
    if not chat_id:
        raise ReplyError("resolved chat is missing its stable identifier")

    now = int(time.time())
    directory = state_directory()
    ensure_private_directory(directory)
    cleanup_expired(directory, now)
    token = secrets.token_hex(16)
    expires_at = now + args.ttl_seconds
    record = {
        "version": 1,
        "chat_name": chat_name,
        "chat_id_sha256": hashlib.sha256(chat_id.encode("utf-8")).hexdigest(),
        "message_sha256": digest_message(message),
        "message_length": len(message),
        "created_at": now,
        "expires_at": expires_at,
    }
    write_record(token_path(directory, token), record)
    log("info", "reply:kakao:prepare", "Prepared a confirmation-bound reply")
    print(
        json.dumps(
            {
                "status": "preview",
                "token": token,
                "chat_name": chat_name,
                "message": message,
                "expires_at": expires_at,
                "requires_user_confirmation": True,
            },
            ensure_ascii=False,
        )
    )
    return 0


def send_reply(args: argparse.Namespace) -> int:
    message = read_message()
    directory = state_directory()
    ensure_private_directory(directory)
    path = token_path(directory, args.token)
    record = load_record(path)
    now = int(time.time())
    if int(record.get("expires_at", 0)) <= now:
        path.unlink(missing_ok=True)
        raise ReplyError("confirmation token has expired")
    if digest_message(message) != record.get("message_sha256"):
        raise ReplyError("reply text differs from the confirmed preview")
    if len(message) != record.get("message_length"):
        raise ReplyError("reply text length differs from the confirmed preview")

    kakaocli = resolve_kakaocli()
    chat = resolve_exact_chat(kakaocli, str(record.get("chat_name", "")))
    chat_id = str(chat.get("id", ""))
    chat_id_hash = hashlib.sha256(chat_id.encode("utf-8")).hexdigest()
    if chat_id_hash != record.get("chat_id_sha256"):
        raise ReplyError("resolved chat changed after preview")

    log("info", "reply:kakao:send", "Dispatching a confirmed KakaoTalk reply")
    process = subprocess.run(
        [kakaocli, "send", str(chat["display_name"]), message],
        check=False,
        capture_output=True,
        text=True,
    )
    if process.returncode != 0:
        log(
            "error",
            "reply:kakao:send",
            f"KakaoTalk UI dispatch failed; exit_code={process.returncode}",
        )
        raise ReplyError("KakaoTalk UI dispatch failed")
    path.unlink(missing_ok=True)
    log("info", "reply:kakao:send", "Confirmed KakaoTalk reply was dispatched")
    print(json.dumps({"status": "dispatched", "token": args.token}))
    return 0


def cancel_reply(args: argparse.Namespace) -> int:
    directory = state_directory()
    ensure_private_directory(directory)
    path = token_path(directory, args.token)
    existed = path.exists()
    path.unlink(missing_ok=True)
    print(json.dumps({"status": "cancelled", "existed": existed}))
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(
        description="Prepare and dispatch confirmation-bound KakaoTalk replies"
    )
    commands = root.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser(
        "prepare",
        help="Resolve an exact chat and create a short-lived preview token",
    )
    prepare.add_argument("--chat", required=True, help="Unique exact chat display name")
    prepare.add_argument(
        "--ttl-seconds",
        type=int,
        default=DEFAULT_TTL_SECONDS,
        choices=range(60, MAX_TTL_SECONDS + 1),
        metavar="60..1800",
    )
    prepare.set_defaults(handler=prepare_reply)

    send = commands.add_parser(
        "send",
        help="Dispatch the same text after the user confirms its preview",
    )
    send.add_argument("--token", required=True, help="Token returned by prepare")
    send.set_defaults(handler=send_reply)

    cancel = commands.add_parser(
        "cancel",
        help="Delete an unused preview token",
    )
    cancel.add_argument("--token", required=True, help="Token returned by prepare")
    cancel.set_defaults(handler=cancel_reply)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        return int(args.handler(args))
    except ReplyError as error:
        log("error", "reply:kakao:failure", str(error))
        print(f"kakao-reply: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
