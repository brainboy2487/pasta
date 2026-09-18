use std::fs;

use pasta::{lexer::lexer::Lexer, Parser};

fn assert_example_parses(path: &str) {
    let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let tokens = Lexer::new(&src).lex();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(
        !program.statements.is_empty(),
        "{path} should produce at least one statement"
    );
}

#[test]
fn snake_demo_parses() {
    assert_example_parses("examples/game_demos/snake.ps");
}

#[test]
fn conway_demo_parses() {
    assert_example_parses("examples/game_demos/conways_gol.ps");
}

#[test]
fn pong_demo_parses() {
    assert_example_parses("examples/game_demos/pong.ps");
}

#[test]
fn tetris_demo_parses() {
    assert_example_parses("examples/game_demos/tetris.ps");
}

#[test]
fn text_rpg_objects_demo_parses() {
    assert_example_parses("examples/game_demos/text_rpg_objects.ps");
}

#[test]
fn text_rpg_dungeon_demo_parses() {
    assert_example_parses("examples/game_demos/text_rpg_dungeon.ps");
}
