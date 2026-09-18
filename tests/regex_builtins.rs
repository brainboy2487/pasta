use assert_cmd::Command;

#[test]
fn regex_builtins_behavior() {
    let src = r#"
IF regex_match("abc123", "[a-z]+[0-9]+") { PRINT "match-true" } ELSE { PRINT "match-false" }
matches = regex_find("foo1bar2", "[a-z]+[0-9]+")
PRINT len(matches)
PRINT matches[0]
PRINT matches[1]
PRINT regex_replace("abc123", "([a-z]+)([0-9]+)", "$1-$2")
caps = regex_captures("abc123", "([a-z]+)([0-9]+)")
PRINT len(caps)
PRINT caps[0]
PRINT caps[1]
PRINT caps[2]
"#;

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .args(["-e", src])
        .assert()
        .success()
        .stdout("match-true\n2\nfoo1\nbar2\nabc-123\n3\nabc123\nabc\n123\n");
}
