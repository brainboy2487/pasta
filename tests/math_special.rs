use assert_cmd::Command;

#[test]
#[ignore]
fn math_special_functions_skeleton() {
    // Ignored skeleton test for special math functions (gamma, erf, log1p, expm1)
    // Remove #[ignore] after implementing builtins.
    let src = r#"
PRINT "gamma:" + str(gamma(5))
PRINT "erf:" + str(erf(0.5))
PRINT "erfc:" + str(erfc(0.5))
PRINT "log1p:" + str(log1p(1e-9))
PRINT "expm1:" + str(expm1(1.0))
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success();
}
