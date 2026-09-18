use assert_cmd::Command;

#[test]
fn math_library_basic() {
    let math = include_str!("../src/stdlib/math.ph").replace("**", "^");
    let src = format!("{}\n\nPRINT m_pow(2,3)\nPRINT m_mod(10,3)\nPRINT m_gcd(48,18)\nPRINT m_lcm(4,6)\nPRINT m_factorial(5)\nPRINT m_binomial(5,2)\nPRINT m_vec2_dot([1,2],[3,4])\nPRINT m_vec2_len([3,4])\n", math);

    Command::cargo_bin("pasta")
        .expect("binary should build")
        .arg("-e")
        .arg(&src)
        .assert()
        .success()
        .stdout("8\n1\n6\n12\n120\n10\n11\n5\n");
}
