use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cct_spec_is_available_without_reading_any_source() {
    let mut command = Command::cargo_bin("msgpipe").unwrap();
    command
        .arg("cct-spec")
        .assert()
        .success()
        .stdout(predicate::str::contains("CCT3"))
        .stdout(predicate::str::contains("A=self"));
}

#[test]
fn reversed_range_fails_before_source_execution() {
    let mut command = Command::cargo_bin("msgpipe").unwrap();
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
