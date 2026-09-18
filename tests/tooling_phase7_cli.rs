use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn fmt_check_reports_unformatted_file() {
    let td = tempdir().expect("tempdir");
    let file = td.path().join("sample.ps");
    fs::write(&file, "PRINT 1   \nPRINT 2").expect("write sample");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["fmt", "--check", file.to_string_lossy().as_ref()])
        .assert()
        .failure();
}

#[test]
fn fmt_rewrites_file_to_normalized_output() {
    let td = tempdir().expect("tempdir");
    let file = td.path().join("sample.ps");
    fs::write(&file, "PRINT 1   \nPRINT 2").expect("write sample");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["fmt", file.to_string_lossy().as_ref()])
        .assert()
        .success();

    let content = fs::read_to_string(&file).expect("read formatted file");
    assert_eq!(content, "PRINT 1\nPRINT 2\n");
}

#[test]
fn fmt_check_supports_directory_targets() {
    let td = tempdir().expect("tempdir");
    let sub = td.path().join("src");
    fs::create_dir_all(&sub).expect("mkdir");
    let file = sub.join("sample.ps");
    fs::write(&file, "PRINT 1   \nPRINT 2").expect("write sample");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["fmt", "--check", td.path().to_string_lossy().as_ref()])
        .assert()
        .failure();
}

#[test]
fn lint_strict_fails_on_unused_assignment() {
    let td = tempdir().expect("tempdir");
    let file = td.path().join("unused.ps");
    fs::write(&file, "x = 1\nPRINT 2\n").expect("write sample");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["lint", "--strict", file.to_string_lossy().as_ref()])
        .assert()
        .failure();
}

#[test]
fn lint_non_strict_allows_warnings() {
    let td = tempdir().expect("tempdir");
    let file = td.path().join("warn_only.ps");
    fs::write(&file, "unused = 1\nPRINT 2\n").expect("write sample");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["lint", file.to_string_lossy().as_ref()])
        .assert()
        .success();
}

#[test]
fn lint_json_output_returns_diagnostics_array() {
    let td = tempdir().expect("tempdir");
    let file = td.path().join("warn_only.ps");
    fs::write(&file, "unused = 1\nPRINT 2\n").expect("write sample");

    let output = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["lint", "--json", file.to_string_lossy().as_ref()])
        .output()
        .expect("lint should run");

    assert!(output.status.success(), "lint --json failed");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(stdout.starts_with('['), "stdout was: {stdout}");
    assert!(stdout.contains("\"code\": \"L1001\""), "stdout was: {stdout}");
}
