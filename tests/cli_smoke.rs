use assert_cmd::Command;

#[test]
fn cli_eval_handles_brace_blocks_and_try() {
    let src = r#"
total = 0
FOR n IN [1, 2, 3] {
    IF n == 2 {
        total = total + 10
    } OTHERWISE {
        total = total + n
    }
}
TRY {
    PRINT total
} OTHERWISE {
    PRINT "unreachable"
} END
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("14\n");
}

#[test]
fn cli_runs_function_fixture_file() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("tests/fixtures/golden/functions_and_collections.ps")
        .assert()
        .success()
        .stdout("neg:pos\n");
}

#[test]
fn cli_runs_headless_canvas_fixture_file() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("tests/fixtures/golden/headless_grid_runs.ps")
        .assert()
        .success()
        .stdout("4294967295\n");
}
