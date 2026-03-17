use pasta::lexer::lexer::Lexer;
use pasta::lexer::tokens::TokenType;

#[test]
fn lex_range_operator() {
    let src = "0..10";
    let tokens = Lexer::new(src).lex();
    assert_eq!(tokens[0].kind, TokenType::Number);
    assert_eq!(tokens[1].kind, TokenType::DotDot);
    assert_eq!(tokens[2].kind, TokenType::Number);
}
