use pasta::interpreter::{Executor, Environment, Value};

/// Run the given Pasta source and return the Environment.
fn run_env(src: &str) -> Environment {
    Executor::run(src).expect("Executor::run failed")
}

#[test]
fn range_two_args_half_open() {
    // Sum 0..3 -> 0 + 1 + 2 + 3 = 6
    let src = r#"out = 0
FOR i IN range(0, 4):
    out = out + i
END"#;
    let env = run_env(src);
    let out = env.get_symbol("out").expect("expected 'out' to be defined");
    assert_eq!(out, Value::Number(6.0));
}

#[test]
fn range_empty_when_start_eq_end() {
    // No iterations when start == end
    let src = r#"out = 0
FOR i IN range(5, 5):
    out = out + 1
END"#;
    let env = run_env(src);
    let out = env.get_symbol("out").expect("expected 'out' to be defined");
    assert_eq!(out, Value::Number(0.0));
}
