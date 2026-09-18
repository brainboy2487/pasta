use assert_cmd::Command;

#[test]
fn block_scope_defaults_and_modifiers_match_current_rules() {
    let src = r#"
IF true:
    escaped = 1
END

IF(BIND_SCOPE) true:
    hoisted = 2
END

IF(UNBIND_SCOPE) true:
    hidden = 3
END

PRINT escaped
PRINT hoisted
PRINT hidden
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("1\n2\n3\n");
}

#[test]
fn plain_def_assignments_do_not_mutate_caller_scope() {
    let src = r#"
x = 1

DEF touch():
    x = 9
END

touch()
PRINT x
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn ret_late_zero_ms_is_currently_ret_now() {
    let src = r#"
DEF immediate():
    RET.LATE(0ms): 7
END

PRINT immediate()
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("7\n");
}

#[test]
fn operators_follow_current_precedence_and_juxtaposition_rules() {
    let src = r#"
PRINT 1 + 2 * 3
PRINT 2 ^ 3 ^ 2
name = "Ada"
PRINT "Hello, " name
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("7\n512\nHello, Ada\n");
}

#[test]
fn for_in_number_iteration_keeps_assignments_but_drops_loop_variable() {
    let src = r#"
total = 0
FOR i IN 3:
    total = total + i
END

PRINT total
TRY:
    PRINT i
OTHERWISE:
    PRINT "missing"
END
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("3\nmissing\n");
}

#[test]
fn function_without_ret_now_returns_last_statement_value() {
    let src = r#"
DEF last_value():
    "first"
    42
END

PRINT last_value()
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn bare_ret_now_is_currently_zero() {
    let src = r#"
DEF zeroish():
    RET.NOW()
END

PRINT zeroish()
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn attempt_binds_current_error_text() {
    let src = r#"
ATTEMPT(err):
    PRINT missing_name
OTHERWISE:
    PRINT "caught"
    PRINT err
END
"#;

    let output = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .output()
        .expect("script should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("caught\n"), "stdout was: {stdout}");
    assert!(stdout.contains("undefined variable"), "stdout was: {stdout}");
}

#[test]
fn nested_defs_capture_enclosing_function_scope_when_called_as_values() {
    let src = r#"
DEF outer():
    base = 10
    DEF add_base(x):
        RET.NOW(x + base)
    END
    RET.NOW(add_base)
END

f = outer()
PRINT f(5)
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("15\n");
}

#[test]
fn numeric_string_coercions_follow_current_runtime_rules() {
    let src = r#"
PRINT "2" + 3
PRINT "" + 4
PRINT "abc" + 4
PRINT "5" == 5
PRINT [] < 1
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("5\n4\nabc4\ntrue\ntrue\n");
}

#[test]
fn try_still_leaks_plain_assignments_but_attempt_exposes_error_binding() {
    let src = r#"
TRY:
    hidden = 1
    PRINT missing_try_name
OTHERWISE:
END

ATTEMPT(err):
    PRINT missing_attempt_name
OTHERWISE:
    PRINT err
END

PRINT hidden
PRINT err
"#;

    let output = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .output()
        .expect("script should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\n1\n"), "stdout was: {stdout}");
    assert!(stdout.matches("undefined variable").count() >= 2, "stdout was: {stdout}");
}
