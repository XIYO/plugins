use std::{fs, os::unix::fs::PermissionsExt};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn context_command() -> Command {
    let mut command = Command::cargo_bin("sherpa").unwrap();
    command.arg("context");
    command
}

#[test]
fn cct_spec_is_available_without_reading_any_source() {
    let mut command = context_command();
    command
        .arg("cct-spec")
        .assert()
        .success()
        .stdout(predicate::str::contains("CCT3"))
        .stdout(predicate::str::contains("A=self"));
}

#[test]
fn reversed_range_fails_before_source_execution() {
    let mut command = context_command();
    command
        .args([
            "benchmark",
            "kakao",
            "--start",
            "2026-07-02",
            "--end",
            "2026-07-01",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("start must be earlier than end"));
}

#[test]
fn purge_requires_explicit_force() {
    let directory = tempdir().unwrap();
    let state = directory.path().join("private").join("state.sqlite3");
    let mut command = context_command();
    command
        .args(["purge", "--state", state.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("purge requires --force"));
}

#[test]
fn sync_pending_summary_and_status_form_an_incremental_cycle() {
    let directory = tempdir().unwrap();
    let reader = directory.path().join("kakaocli-fixture");
    fs::write(
        &reader,
        "#!/bin/sh\nprintf '%s\\n' '[[12,34,1782864000,\"Room\",56,\"Sender\",1,\"hello\",\"\",\"\",\"\",0]]'\n",
    )
    .unwrap();
    fs::set_permissions(&reader, fs::Permissions::from_mode(0o755)).unwrap();
    let state = directory.path().join("private").join("state.sqlite3");

    context_command()
        .args([
            "sync",
            "kakao",
            "--start",
            "2026-07-01",
            "--end",
            "2026-07-02",
            "--binary",
            reader.to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"inserted_messages\": 1"));

    let pipeline_args = [
        "kakao",
        "--start",
        "2026-07-01",
        "--end",
        "2026-07-02",
        "--thread",
        "K001",
        "--state",
        state.to_str().unwrap(),
    ];
    context_command()
        .arg("pending")
        .args(pipeline_args)
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));

    context_command()
        .args([
            "context",
            "put",
            "session",
            "--thread",
            "K001",
            "--start",
            "2026-07-01",
            "--end",
            "2026-07-02",
            "--state",
            state.to_str().unwrap(),
        ])
        .write_stdin("important session summary")
        .assert()
        .success();

    context_command()
        .args([
            "context",
            "put",
            "thread",
            "--thread",
            "K001",
            "--start",
            "2026-07-01",
            "--end",
            "2026-07-02",
            "--state",
            state.to_str().unwrap(),
        ])
        .write_stdin("must not commit without a watermark")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--through-context-id is required"));

    context_command()
        .args([
            "context",
            "inputs",
            "thread",
            "--thread",
            "K001",
            "--state",
            state.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "!CTX1|scope=session|through=1|count=1",
        ))
        .stdout(predicate::str::contains("important session summary"));

    context_command()
        .args([
            "context",
            "put",
            "thread",
            "--thread",
            "K001",
            "--through-context-id",
            "1",
            "--start",
            "2026-07-01",
            "--end",
            "2026-07-02",
            "--state",
            state.to_str().unwrap(),
        ])
        .write_stdin("cumulative thread summary")
        .assert()
        .success();

    context_command()
        .args([
            "context",
            "inputs",
            "global",
            "--state",
            state.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "!CTX1|scope=thread|through=2|count=1",
        ))
        .stdout(predicate::str::contains("cumulative thread summary"));

    context_command()
        .args([
            "context",
            "put",
            "global",
            "--through-context-id",
            "2",
            "--start",
            "2026-07-01",
            "--end",
            "2026-07-02",
            "--state",
            state.to_str().unwrap(),
        ])
        .write_stdin("cumulative global summary")
        .assert()
        .success();

    context_command()
        .arg("pending")
        .args(pipeline_args)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    context_command()
        .args([
            "status",
            "kakao",
            "--thread",
            "K001",
            "--state",
            state.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"pending_messages\": 0"))
        .stdout(predicate::str::contains("\"last_analyzed_at_utc\""));
}
