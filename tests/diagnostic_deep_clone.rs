use assert_cmd::Command;

#[test]
fn small_reproducer() {
    let src = r#"
# small nested-def reproducer
DEF make(i):
    DEF inner():
        RETURN i
    END
    RETURN inner
END

f1 = make(1)
f2 = make(2)
PRINT f1()
PRINT f2()
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("1\n2\n");
}

#[test]
#[ignore]
fn heavy_reproducer() {
    // Ignored heavy test: generates many top-level DEFs capturing the same list
    let mut prog = String::from("x = [1,2,3]\n");
    for i in 0..300 {
        prog.push_str(&format!("DEF f{}():\n    RETURN x\nEND\n", i));
    }
    prog.push_str("PRINT \"done\"\n");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", &prog])
        .assert()
        .success()
        .stdout("done\n");
}
