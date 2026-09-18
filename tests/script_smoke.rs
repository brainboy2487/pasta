use assert_cmd::Command;

#[test]
fn cli_invocation_smoke_eval() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT "phase6-smoke""#])
        .assert()
        .success()
        .stdout("phase6-smoke\n");
}

#[test]
fn cli_runs_phase5_sort_smoke_script() {
    let assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("tests/phase5_sort_smoke.ps")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf-8");
    assert!(
        stdout.contains("=== PHASE5 SORT SMOKE PASS ==="),
        "stdout was:\n{stdout}"
    );
}

#[test]
fn cli_runs_phase7_thread_parallel_smoke_script() {
    let assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("tests/phase7_thread_parallel_smoke.ps")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf-8");
    assert!(
        stdout.contains("=== PHASE7 THREAD PARALLEL SMOKE PASS ==="),
        "stdout was:\n{stdout}"
    );
}
