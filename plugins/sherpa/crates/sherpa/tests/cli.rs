use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

const FAKE_KAKAOCLI: &str = r#"#!/bin/sh
case "$1" in
  chats)
    printf '%s\n' '[{"id":101,"display_name":"Example"}]'
    ;;
  send)
    printf '%s\t%s\n' "$2" "$3" >> "$FAKE_KAKAO_LOG"
    ;;
  *)
    exit 2
    ;;
esac
"#;

fn configured_command(
    state: &std::path::Path,
    fake: &std::path::Path,
    log: &std::path::Path,
) -> Command {
    let mut command = Command::cargo_bin("sherpa").unwrap();
    command
        .env("SHERPA_CONTEXT_REPLY_STATE", state)
        .env("KAKAOCLI_BIN", fake)
        .env("FAKE_KAKAO_LOG", log);
    command
}

#[test]
fn context_reply_requires_the_same_confirmed_text_and_is_single_use() {
    let root = tempdir().unwrap();
    let state = root.path().join("state");
    let fake = root.path().join("kakaocli");
    let log = root.path().join("sent.log");
    fs::write(&fake, FAKE_KAKAOCLI).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let output = configured_command(&state, &fake, &log)
        .args([
            "context",
            "reply",
            "prepare",
            "--via",
            "kakaotalk",
            "--conversation",
            "Example",
        ])
        .write_stdin("Confirmed response")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview: Value = serde_json::from_slice(&output).unwrap();
    let token = preview["token"].as_str().unwrap();

    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let approval = state.join(format!("{token}.json"));
    assert_eq!(
        fs::metadata(&approval).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&approval).unwrap().uid(),
        rustix::process::getuid().as_raw()
    );

    configured_command(&state, &fake, &log)
        .args(["context", "reply", "confirm", "--token", token])
        .write_stdin("Changed response")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "reply text differs from the confirmed preview",
        ));
    assert!(!log.exists());

    configured_command(&state, &fake, &log)
        .args(["context", "reply", "confirm", "--token", token])
        .write_stdin("Confirmed response")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"dispatched\""));
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "Example\tConfirmed response\n"
    );
    assert!(!approval.exists());

    configured_command(&state, &fake, &log)
        .args(["context", "reply", "confirm", "--token", token])
        .write_stdin("Confirmed response")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "approval token is missing or already used",
        ));
}

#[test]
fn context_reply_rejects_ambiguous_conversation_names() {
    let root = tempdir().unwrap();
    let state = root.path().join("state");
    let fake = root.path().join("kakaocli");
    let log = root.path().join("sent.log");
    fs::write(
        &fake,
        FAKE_KAKAOCLI.replace(
            r#"[{"id":101,"display_name":"Example"}]"#,
            r#"[{"id":101,"display_name":"Example"},{"id":102,"display_name":"Example Team"}]"#,
        ),
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    configured_command(&state, &fake, &log)
        .args([
            "context",
            "reply",
            "prepare",
            "--via",
            "kakaotalk",
            "--conversation",
            "Example",
        ])
        .write_stdin("Confirmed response")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "conversation is missing or ambiguous",
        ));
}

#[test]
fn planner_forwards_only_to_its_configured_platform_adapters() {
    let root = tempdir().unwrap();
    let adapter = root.path().join("adapter");
    let log = root.path().join("adapter.log");
    fs::write(
        &adapter,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$SHERPA_TEST_ADAPTER_LOG\"\n",
    )
    .unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).unwrap();

    Command::cargo_bin("sherpa")
        .unwrap()
        .env("SHERPA_PLANNER_CALENDAR_BIN", &adapter)
        .env("SHERPA_TEST_ADAPTER_LOG", &log)
        .args(["planner", "calendar", "events", "--json"])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(log).unwrap(), "events --json\n");
}
