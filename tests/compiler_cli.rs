use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::tempdir;

fn compiler_toolchain_available() -> bool {
    StdCommand::new("llc").arg("--version").output().is_ok()
        && StdCommand::new("clang").arg("--version").output().is_ok()
}

#[test]
fn cli_emits_llvm_for_bootstrap_hello_world() {
    let assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["--emit-llvm", "-e", r#"PRINT "Hello from LLVM""#])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(stdout.contains("define i32 @main()"), "{stdout}");
    assert!(stdout.contains("declare i32 @puts"), "{stdout}");
    assert!(stdout.contains("Hello from LLVM"), "{stdout}");
}

#[test]
fn cli_compiles_bootstrap_hello_world_binary() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("hello.ps");
    let output_path = temp.path().join("hello_bin");
    fs::write(&script_path, "PRINT \"Hello from compiled Pasta\"\n").expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    StdCommand::new(&output_path)
        .output()
        .map(|output| {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                "Hello from compiled Pasta\n"
            );
            assert!(output.status.success());
        })
        .expect("compiled binary should run");
}

#[test]
fn cli_compiles_numeric_bool_and_variable_subset() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("subset.ps");
    let output_path = temp.path().join("subset_bin");
    fs::write(
        &script_path,
        r#"
message = "subset ok"
x = 5
y = x * 2 + 1
flag = y >= 11
same = y == 11
other = false
PRINT message
PRINT y
PRINT flag
PRINT same
PRINT other
PRINT other == false
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "subset ok\n11\ntrue\ntrue\nfalse\ntrue\n"
    );
}

#[test]
fn cli_compiles_color_builtin() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("color_builtin.ps");
    let output_path = temp.path().join("color_builtin_bin");
    fs::write(
        &script_path,
        r#"
shade = color(1, 2, 3)
PRINT shade
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "4278256131\n");
}

#[test]
fn cli_compiles_basic_runtime_graphics_bridge() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("graphics_bridge.ps");
    let output_path = temp.path().join("graphics_bridge_bin");
    fs::write(
        &script_path,
        r#"
win = WINDOW("Compiled Graphics", 16, 16)
SET_DRAW_TARGET(win)
SET_COLOR(color(1, 2, 3))
canvas_fill_rect(win, 0, 0, 4, 4)
key = WINDOW_KEY(win)
IF key == "Up":
    PRINT "up"
OTHERWISE:
    PRINT "idle"
END
PRINT window_poll(win)
SWAP_BUFFER(win)
fps_init(10)
fps_begin(10)
fps_end()
fps_tick()
WINDOW_CLOSE(win)
PRINT window_poll(win)
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "idle\ntrue\nfalse\n");
}

#[test]
fn cli_compiles_collection_runtime_bridge() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("collections.ps");
    let output_path = temp.path().join("collections_bin");
    fs::write(
        &script_path,
        r#"
items = [4, 5, 6]
PRINT list_len(items)
PRINT items[1]
head = [1] + items
PRINT list_len(head)
tail = list_slice(head, 1, 4)
PRINT tail[2]
point = {"x": 9, "y": 3}
PRINT dict_get(point, "x")
PRINT rand.int(0, 1) >= 0
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "3\n5\n4\n6\n9\ntrue\n"
    );
}

#[test]
fn cli_compiles_functions_calls_and_returns() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("functions.ps");
    let output_path = temp.path().join("functions_bin");
    fs::write(
        &script_path,
        r#"
DEF inc(n):
    next = n + 1
    RET.NOW(next)
END

DEF double_inc(n):
    RET.NOW(inc(n) + inc(n))
END

DEF echo_name(name):
    PRINT name
    RET.NOW(name)
END

DEF is_big(n):
    RET.NOW(n >= 10)
END

value = double_inc(4)
spoken = echo_name("Ada")
flag = is_big(value)
PRINT value
PRINT spoken
PRINT flag
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Ada\n10\nAda\ntrue\n"
    );
}

#[test]
fn cli_compiles_if_while_break_and_continue() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("control_flow.ps");
    let output_path = temp.path().join("control_flow_bin");
    fs::write(
        &script_path,
        r#"
DEF classify(total):
    IF total >= 8:
        RET.NOW(true)
    OTHERWISE:
        RET.NOW(false)
    END
END

i = 0
sum_total = 0
WHILE i < 5:
    i = i + 1
    IF i == 2:
        CONTINUE
    END
    IF i == 5:
        BREAK
    END
    sum_total = sum_total + i
END

label = "small"
IF classify(sum_total):
    label = "big"
OTHERWISE:
    label = "small"
END

PRINT sum_total
PRINT classify(sum_total)
PRINT label
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    let output = StdCommand::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "8\ntrue\nbig\n");
}

#[test]
fn cli_compiles_native_shared_module() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("mathfast.pm");
    let output_path = temp.path().join("libmathfast.so");
    fs::write(
        &script_path,
        r#"
MOD mathfast:
    export add

    DEF add(a, b):
        RET.NOW(a + b)
    END
END
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--shared")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    assert!(output_path.exists(), "shared library should be created");
}

#[test]
fn cli_compiles_native_shared_module_with_opaque_value_bridge() {
    if !compiler_toolchain_available() {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("bridge.pm");
    let output_path = temp.path().join("libbridge.so");
    fs::write(
        &script_path,
        r#"
MOD bridge:
    export bounce

    DEF forward(v):
        RET.NOW(v)
    END

    DEF bounce(v):
        local_value = v
        RET.NOW(forward(local_value))
    END
END
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--compile")
        .arg("--shared")
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();

    assert!(output_path.exists(), "shared library should be created");
}

#[test]
fn cli_rejects_compiled_function_wrong_arity() {
    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("arity.ps");
    fs::write(
        &script_path,
        r#"
DEF add(a, b):
    RET.NOW(a + b)
END

PRINT add(1)
"#,
    )
    .expect("write script");

    let assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--emit-llvm")
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("expects exactly 2 argument(s), got 1"), "{stderr}");
}

#[test]
fn cli_rejects_compiled_function_missing_return_path() {
    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("missing_return_path.ps");
    fs::write(
        &script_path,
        r#"
DEF maybe_value(n):
    IF n > 0:
        RET.NOW(n)
    END
END

PRINT maybe_value(1)
"#,
    )
    .expect("write script");

    let assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--emit-llvm")
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(stderr.contains("every control-flow path"), "{stderr}");
}

#[test]
fn cli_allows_compiled_function_without_explicit_return_when_used_as_statement() {
    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("missing_return.ps");
    fs::write(
        &script_path,
        r#"
DEF greet(name):
    PRINT name
END

greet("Ada")
"#,
    )
    .expect("write script");

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&script_path)
        .arg("--emit-llvm")
        .assert()
        .success();
}

#[test]
fn cli_rejects_compiled_scope_modifiers_and_break_outside_loop() {
    let temp = tempdir().expect("tempdir");
    let scope_script = temp.path().join("scope_modifier.ps");
    fs::write(
        &scope_script,
        r#"
value = 0
IF(BIND_SCOPE) true:
    value = 1
END
PRINT value
"#,
    )
    .expect("write script");

    let scope_assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&scope_script)
        .arg("--emit-llvm")
        .assert()
        .failure();
    let scope_stderr =
        String::from_utf8(scope_assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(scope_stderr.contains("does not yet support BIND_SCOPE / UNBIND_SCOPE"), "{scope_stderr}");

    let while_script = temp.path().join("break_outside_loop.ps");
    fs::write(
        &while_script,
        r#"
BREAK
"#,
    )
    .expect("write script");

    let while_assert = Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg(&while_script)
        .arg("--emit-llvm")
        .assert()
        .failure();
    let while_stderr =
        String::from_utf8(while_assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(while_stderr.contains("only supported inside compiled WHILE loops"), "{while_stderr}");
}
