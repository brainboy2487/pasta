use assert_cmd::Command;

#[test]
fn bitshift_basic() {
    // Basic left and right shift checks using the REPL -e execution.
    let src = "PRINT 1 << 2\nPRINT 8 >> 1\nPRINT 3 << 0\nPRINT 16 >> 4\n";

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("-e")
        .arg(src)
        .assert()
        .success()
        .stdout("4\n4\n3\n1\n");
}
