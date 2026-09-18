use assert_cmd::Command;

#[test]
fn sort_list_numbers_default_stable() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort([3, 1, 2, 1])"#])
        .assert()
        .success()
        .stdout("1 1 2 3\n");
}

#[test]
fn sort_list_strings_fast() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort(["pear", "apple", "banana"], "fast")"#])
        .assert()
        .success()
        .stdout("apple banana pear\n");
}

#[test]
fn sort_list_mixed_uses_string_fallback() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort([2, "10", 1])"#])
        .assert()
        .success()
        .stdout("1 10 2\n");
}

#[test]
fn sort_dict_by_keys() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort({"z": 26, "a": 1, "m": 13})"#])
        .assert()
        .success()
        .stdout("{a: 1, m: 13, z: 26}\n");
}

#[test]
fn sort_rejects_invalid_mode() {
    let output = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort([3, 2, 1], "quick")"#])
        .output()
        .expect("script should run");

    assert!(!output.status.success(), "script unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("sort: mode must be fast, stable, or slow"),
        "stderr was: {stderr}"
    );
}

#[test]
fn sort_rejects_unsupported_type() {
    let output = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT sort(42)"#])
        .output()
        .expect("script should run");

    assert!(!output.status.success(), "script unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("sort: expected list or dict"),
        "stderr was: {stderr}"
    );
}

#[test]
fn list_sort_still_works() {
    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", r#"PRINT list_sort([9, 4, 7, 1])"#])
        .assert()
        .success()
        .stdout("1 4 7 9\n");
}
