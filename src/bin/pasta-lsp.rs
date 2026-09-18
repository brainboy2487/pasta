use pasta::lexer::lexer::Lexer;
use pasta::lexer::TokenType;
use pasta::parser::parser::Parser;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};

const KEYWORDS: &[&str] = &[
    "IF", "OTHERWISE", "WHILE", "FOR", "IN", "DEF", "DO", "END", "TRY", "PRINT",
    "RETURN", "RET.NOW", "RET.LATE", "LAMBDA", "FROM", "USE", "AS", "CLASS",
];

const BUILTINS: &[(&str, &str)] = &[
    ("len", "Builtin function: len(value)"),
    ("range", "Builtin function: range(n) / range(start, end[, step])"),
    ("sort", "Builtin function: sort(list|dict[, mode])"),
    ("list_sort", "Builtin function: list_sort(list)"),
    ("dict_get", "Builtin function: dict_get(dict, key)"),
    ("type_of", "Builtin function: type_of(value)"),
];

#[derive(Debug, Clone)]
struct SymbolDef {
    line: u32,
    character: u32,
}

#[derive(Debug, Clone)]
struct DocumentState {
    text: String,
    symbols: HashMap<String, SymbolDef>,
}

fn read_lsp_message(reader: &mut BufReader<io::Stdin>) -> io::Result<Option<String>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let line_trimmed = line.trim_end();
        if let Some(rest) = line_trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }

    let Some(len) = content_length else {
        return Ok(None);
    };
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let msg = String::from_utf8(payload).unwrap_or_default();
    Ok(Some(msg))
}

fn write_lsp_message(stdout: &mut io::StdoutLock<'_>, body: &Value) -> io::Result<()> {
    let text = body.to_string();
    write!(stdout, "Content-Length: {}\r\n\r\n{}", text.len(), text)?;
    stdout.flush()
}

fn to_lsp_pos(line_1: usize, col_1: usize) -> (u32, u32) {
    (
        line_1.saturating_sub(1) as u32,
        col_1.saturating_sub(1) as u32,
    )
}

fn build_symbol_table(text: &str) -> HashMap<String, SymbolDef> {
    let mut table = HashMap::new();
    let tokens = match Lexer::new(text).lex_result() {
        Ok(t) => t,
        Err(_) => return table,
    };
    for i in 0..tokens.len() {
        if tokens[i].kind == TokenType::Def
            && i + 1 < tokens.len()
            && tokens[i + 1].kind == TokenType::Identifier
        {
            let name = tokens[i + 1].value.clone().unwrap_or_default();
            let (line, character) = to_lsp_pos(tokens[i + 1].line, tokens[i + 1].col);
            table.entry(name).or_insert(SymbolDef { line, character });
            continue;
        }
        if tokens[i].kind == TokenType::Identifier
            && i + 1 < tokens.len()
            && tokens[i + 1].kind == TokenType::Eq
        {
            let name = tokens[i].value.clone().unwrap_or_default();
            let (line, character) = to_lsp_pos(tokens[i].line, tokens[i].col);
            table.entry(name).or_insert(SymbolDef { line, character });
        }
    }
    table
}

fn diagnostics_for_text(uri: &str, text: &str) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let tokens = match Lexer::new(text).lex_result() {
        Ok(t) => t,
        Err(e) => {
            let (line, character) = to_lsp_pos(e.line, e.col);
            diagnostics.push(json!({
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character + 1 }
                },
                "severity": 1,
                "code": "L0001",
                "source": "pasta-lsp",
                "message": format!("lex error in {}: {}", uri, e.message)
            }));
            return diagnostics;
        }
    };

    let mut parser = Parser::new(tokens);
    let (_program, parse_diags) = parser.parse_with_diagnostics();
    for d in parse_diags {
        let (sl, sc) = to_lsp_pos(d.span.start_line, d.span.start_col);
        let (el, ec) = to_lsp_pos(d.span.end_line, d.span.end_col);
        diagnostics.push(json!({
            "range": {
                "start": { "line": sl, "character": sc },
                "end": { "line": el, "character": ec.max(sc + 1) }
            },
            "severity": 1,
            "code": "L0002",
            "source": "pasta-lsp",
            "message": d.message
        }));
    }
    diagnostics
}

fn word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

fn identifier_at_position(text: &str, line: u32, character: u32) -> Option<String> {
    let line_text = text.lines().nth(line as usize)?;
    let chars: Vec<char> = line_text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut idx = (character as usize).min(chars.len());
    if idx == chars.len() && idx > 0 {
        idx -= 1;
    }
    if !word_char(chars[idx]) {
        if idx > 0 && word_char(chars[idx - 1]) {
            idx -= 1;
        } else {
            return None;
        }
    }

    let mut start = idx;
    while start > 0 && word_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = idx;
    while end + 1 < chars.len() && word_char(chars[end + 1]) {
        end += 1;
    }
    let ident: String = chars[start..=end].iter().collect();
    if ident.is_empty() {
        None
    } else {
        Some(ident)
    }
}

fn prefix_at_position(text: &str, line: u32, character: u32) -> String {
    let Some(line_text) = text.lines().nth(line as usize) else {
        return String::new();
    };
    let chars: Vec<char> = line_text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut idx = (character as usize).min(chars.len());
    if idx > 0 && (idx == chars.len() || !word_char(chars[idx])) {
        idx -= 1;
    }
    if !word_char(chars[idx]) {
        return String::new();
    }
    let mut start = idx;
    while start > 0 && word_char(chars[start - 1]) {
        start -= 1;
    }
    chars[start..=idx].iter().collect()
}

fn completion_items(prefix: &str, symbols: &HashMap<String, SymbolDef>) -> Vec<Value> {
    let p = prefix.to_ascii_lowercase();
    let mut items = Vec::new();
    for kw in KEYWORDS {
        if kw.to_ascii_lowercase().starts_with(&p) {
            items.push(json!({
                "label": kw,
                "kind": 14,
                "detail": "keyword",
            }));
        }
    }
    for (name, detail) in BUILTINS {
        if name.to_ascii_lowercase().starts_with(&p) {
            items.push(json!({
                "label": name,
                "kind": 3,
                "detail": detail,
            }));
        }
    }
    for name in symbols.keys() {
        if name.to_ascii_lowercase().starts_with(&p) {
            items.push(json!({
                "label": name,
                "kind": 6,
                "detail": "document symbol",
            }));
        }
    }
    items
}

fn hover_text_for_symbol(symbol: &str) -> Option<String> {
    for (name, detail) in BUILTINS {
        if symbol.eq_ignore_ascii_case(name) {
            return Some((*detail).to_string());
        }
    }
    if KEYWORDS.iter().any(|k| symbol.eq_ignore_ascii_case(k)) {
        return Some(format!("Keyword: {}", symbol.to_ascii_uppercase()));
    }
    None
}

fn publish_diagnostics(
    out: &mut io::StdoutLock<'_>,
    uri: &str,
    text: &str,
) -> io::Result<()> {
    let diags = diagnostics_for_text(uri, text);
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diags
        }
    });
    write_lsp_message(out, &notif)
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut shutdown_requested = false;
    let mut docs: HashMap<String, DocumentState> = HashMap::new();

    while let Some(raw) = read_lsp_message(&mut reader)? {
        let msg: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
        let id = msg.get("id").cloned();
        let method = msg
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or_default();

        match method {
            "initialize" => {
                if let Some(id_val) = id {
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": {
                            "capabilities": {
                                "textDocumentSync": 1,
                                "hoverProvider": true,
                                "definitionProvider": true,
                                "completionProvider": {
                                    "resolveProvider": false
                                }
                            },
                            "serverInfo": {
                                "name": "pasta-lsp",
                                "version": pasta::PASTA_VERSION
                            }
                        }
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
            "shutdown" => {
                shutdown_requested = true;
                if let Some(id_val) = id {
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": Value::Null
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
            "exit" => {
                break;
            }
            "textDocument/didOpen" => {
                let uri = msg["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let text = msg["params"]["textDocument"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let symbols = build_symbol_table(&text);
                docs.insert(uri.clone(), DocumentState { text: text.clone(), symbols });
                publish_diagnostics(&mut out, &uri, &text)?;
            }
            "textDocument/didChange" => {
                let uri = msg["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let text = msg["params"]["contentChanges"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let symbols = build_symbol_table(&text);
                docs.insert(uri.clone(), DocumentState { text: text.clone(), symbols });
                publish_diagnostics(&mut out, &uri, &text)?;
            }
            "textDocument/didClose" => {
                let uri = msg["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                docs.remove(&uri);
                let notif = json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": []
                    }
                });
                write_lsp_message(&mut out, &notif)?;
            }
            "textDocument/completion" => {
                if let Some(id_val) = id {
                    let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or_default();
                    let line = msg["params"]["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = msg["params"]["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let (prefix, symbols) = if let Some(doc) = docs.get(uri) {
                        (prefix_at_position(&doc.text, line, character), doc.symbols.clone())
                    } else {
                        (String::new(), HashMap::new())
                    };
                    let items = completion_items(&prefix, &symbols);
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": {
                            "isIncomplete": false,
                            "items": items
                        }
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
            "textDocument/definition" => {
                if let Some(id_val) = id {
                    let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or_default();
                    let line = msg["params"]["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = msg["params"]["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let result = if let Some(doc) = docs.get(uri) {
                        if let Some(symbol) = identifier_at_position(&doc.text, line, character) {
                            if let Some(def) = doc.symbols.get(&symbol) {
                                json!([{
                                    "uri": uri,
                                    "range": {
                                        "start": { "line": def.line, "character": def.character },
                                        "end": { "line": def.line, "character": def.character + 1 }
                                    }
                                }])
                            } else {
                                Value::Null
                            }
                        } else {
                            Value::Null
                        }
                    } else {
                        Value::Null
                    };
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": result
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
            "textDocument/hover" => {
                if let Some(id_val) = id {
                    let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or_default();
                    let line = msg["params"]["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = msg["params"]["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let result = if let Some(doc) = docs.get(uri) {
                        if let Some(symbol) = identifier_at_position(&doc.text, line, character) {
                            let msg = hover_text_for_symbol(&symbol)
                                .unwrap_or_else(|| format!("Symbol: {symbol}"));
                            json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": msg
                                }
                            })
                        } else {
                            Value::Null
                        }
                    } else {
                        Value::Null
                    };
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": result
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
            _ => {
                if id.is_some() {
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": Value::Null
                    });
                    write_lsp_message(&mut out, &response)?;
                }
            }
        }
    }

    if shutdown_requested {
        Ok(())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table_tracks_defs_and_assignments() {
        let text = r#"
DEF add(a, b):
    out = a + b
END
"#;
        let table = build_symbol_table(text);
        assert!(table.contains_key("add"));
        assert!(table.contains_key("out"));
    }

    #[test]
    fn identifier_extracts_at_cursor() {
        let text = "value = sort(items)\n";
        let id = identifier_at_position(text, 0, 10).expect("identifier");
        assert_eq!(id, "sort");
    }

    #[test]
    fn completion_filters_by_prefix() {
        let mut symbols = HashMap::new();
        symbols.insert("my_value".to_string(), SymbolDef { line: 0, character: 0 });
        let items = completion_items("so", &symbols);
        let labels: Vec<String> = items
            .iter()
            .filter_map(|v| v.get("label").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .collect();
        assert!(labels.iter().any(|s| s == "sort"));
        assert!(!labels.iter().any(|s| s == "my_value"));
    }
}
