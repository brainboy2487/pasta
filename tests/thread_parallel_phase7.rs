use std::collections::HashSet;
use std::time::{Duration, Instant};

use pasta::lexer::lexer::Lexer;
use pasta::parser::ast::Statement;
use pasta::{Executor, Parser, Value};

fn parse_program(src: &str) -> pasta::parser::ast::Program {
    let tokens = Lexer::new(src).lex();
    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[test]
fn parses_thread_assignment_header() {
    let program = parse_program(
        "thid = THREAD worker:\n    x = 1\n",
    );
    assert_eq!(program.statements.len(), 1);
    match &program.statements[0] {
        Statement::ThreadHeader(header) => {
            assert_eq!(header.name.name, "worker");
            assert_eq!(header.assign_target.as_ref().map(|i| i.name.as_str()), Some("thid"));
            assert_eq!(header.body.len(), 1);
        }
        other => panic!("expected ThreadHeader, got {other:?}"),
    }
}

#[test]
fn parses_parallel_block_with_threads() {
    let program = parse_program(
        "PARALLEL:\n    THREAD left:\n        x = 1\n    THREAD right:\n        y = 2\n",
    );
    assert_eq!(program.statements.len(), 1);
    match &program.statements[0] {
        Statement::ParallelBlock(block) => {
            assert_eq!(block.threads.len(), 2);
            assert_eq!(block.threads[0].name.name, "left");
            assert_eq!(block.threads[1].name.name, "right");
        }
        other => panic!("expected ParallelBlock, got {other:?}"),
    }
}

#[test]
fn executes_thread_header_and_returns_thread_id() {
    let env = Executor::run("tid = THREAD worker_id_case:\n    x = 1\n").expect("thread header should run");
    let tid = env.get_symbol("tid").expect("tid should be assigned");
    match tid {
        Value::Number(n) => assert!(n >= 1.0, "thread id should be a positive number"),
        other => panic!("expected numeric tid, got {other:?}"),
    }
}

#[test]
fn executes_parallel_block_spawns_named_threads() {
    let before: HashSet<u64> = pasta::threading::threads::list_threads()
        .into_iter()
        .map(|t| t.id)
        .collect();

    let _ = Executor::run(
        "PARALLEL:\n    THREAD alpha:\n        a = 1\n    THREAD beta:\n        b = 2\n",
    )
    .expect("parallel block should run");

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_alpha = false;
    let mut saw_beta = false;
    while Instant::now() < deadline {
        let rows = pasta::threading::threads::list_threads();
        for row in rows {
            if before.contains(&row.id) {
                continue;
            }
            if row.name == "alpha" {
                saw_alpha = true;
            }
            if row.name == "beta" {
                saw_beta = true;
            }
        }
        if saw_alpha && saw_beta {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_alpha, "expected alpha thread entry in registry");
    assert!(saw_beta, "expected beta thread entry in registry");
}

#[test]
fn wait_for_thread_blocks_until_completion() {
    let env = Executor::run(
        "done = 0\n\
         THREAD worker_wait_case:\n\
             sleep(20)\n\
         WAIT FOR worker_wait_case\n\
         done = 1\n\
         status = thread.status(\"worker_wait_case\")\n",
    )
    .expect("wait for thread should run");
    assert_eq!(env.get_symbol("done"), Some(Value::Number(1.0)));
    assert_eq!(
        env.get_symbol("status"),
        Some(Value::String("finished".to_string()))
    );
}

#[test]
fn if_thread_status_condition_parses_and_runs() {
    let env = Executor::run(
        "flag = 0\n\
         THREAD worker_if_case:\n\
             sleep(30)\n\
         IF thread worker_if_case is running:\n\
             flag = 1\n\
         END\n",
    )
    .expect("thread status IF condition should run");
    assert_eq!(env.get_symbol("flag"), Some(Value::Number(1.0)));
}
