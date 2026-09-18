use assert_cmd::Command;

#[test]
fn closure_capture_behavior_module_and_nested() {
    let src = r#"
x = 1

DEF f():
    RETURN x
END

x = 2
PRINT f()

DEF outer():
    base = 10
    DEF add_base(x):
        RET.NOW(x + base)
    END
    RET.NOW(add_base)
END

f2 = outer()
PRINT f2(5)
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("2\n15\n");
}
