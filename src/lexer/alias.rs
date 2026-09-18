 // src/lexer/alias.rs
 //! Alias table and normalization for PASTA lexer.
 //!
 //! Case-insensitive alias mapping with optional JSON override.
 //! - Canonical names are stored uppercase (e.g., "DO").
 //! - Aliases are stored lowercase for case-insensitive lookup.
 //! - Includes explicit aliases for OBJ and SPAWN used by the grammar.

 use std::collections::HashMap;
 use std::fs;
 use std::path::Path;

 /// AliasTable maps lowercase alias -> canonical token name (uppercase string).
 #[derive(Debug, Clone)]
 pub struct AliasTable {
     map: HashMap<String, String>,
 }

 impl AliasTable {
     /// Create a new AliasTable. Attempts to load `alias_table.json` from a few
     /// common locations; falls back to built-in defaults on failure.
     pub fn new() -> Self {
         if let Some(table) = Self::load_from_json() {
             return table;
         }
         Self::with_defaults()
     }

     /// Construct the default alias table (hard-coded).
     pub fn with_defaults() -> Self {
         let mut map = HashMap::new();

         macro_rules! add_aliases {
             ($canonical:expr, [$($alias:expr),* $(,)?]) => {
                 $(
                     map.insert($alias.to_lowercase(), $canonical.to_uppercase());
                 )*
             };
         }

         // Core control keywords and common synonyms
         add_aliases!("DEF", ["def", "define", "function", "func"]);
         add_aliases!("DO", ["do", "run", "start", "begin"]);
         add_aliases!("AND", ["and"]);
         add_aliases!("OR", ["or"]);
         add_aliases!("NOT", ["not", "negate"]);
         add_aliases!("FOR", ["for", "times", "repeat", "using"]);
         add_aliases!("IN",  ["in"]);
         add_aliases!("AS", ["as", "named", "called"]);
         add_aliases!("OVER", ["over", "above", "before"]);
         add_aliases!("LIMIT", ["limit", "bounded_by", "under"]);
         add_aliases!("END", ["end", "stop", "finish", "terminate"]);
         add_aliases!("PAUSE", ["pause", "sleep", "hold", "suspend"]);
         add_aliases!("UNPAUSE", ["unpause", "resume"]);
         add_aliases!("RESTART", ["restart", "reset", "rerun"]);
         add_aliases!("WAIT", ["wait", "hold_for"]);
         add_aliases!("SET", ["set", "assign", "let", "make", "MAKE", "LET", "ASSIGN"]);
         add_aliases!("IF", ["if", "provided"]);
         // NOTE: "do" must NOT alias to THEN — it is the DO block keyword.
         // The parser's IF handler already accepts both THEN and DO via match_token.
         add_aliases!("THEN", ["then"]);
         add_aliases!("TRY", ["try", "attempt"]);
         add_aliases!("OTHERWISE", ["otherwise", "else", "catch"]);
         add_aliases!("GROUP", ["group", "bundle"]);
         add_aliases!("CLASS", ["class", "type", "kind"]);
         add_aliases!("LEARN", ["learn", "build_model", "make_net", "define_net"]);
         add_aliases!("BUILD", ["build", "construct"]);
         add_aliases!("TENSOR", ["array", "matrix"]);
         add_aliases!("PRINT", ["print", "echo", "println"]);
         add_aliases!("WHILE", ["while"]);
         add_aliases!("TRUE", ["true", "True", "TRUE"]);
         add_aliases!("FALSE", ["false", "False", "FALSE"]);

         // Grammar-specific explicit tokens used by parser/lexer
         add_aliases!("OBJ", ["obj"]);
         add_aliases!("SPAWN", ["spawn"]);
         // New control keywords
         add_aliases!("UNLESS", ["unless"]);
         add_aliases!("UNTIL",  ["until"]);
         add_aliases!("PASS",   ["pass", "noop"]);
         add_aliases!("NONE",   ["none", "null", "nil", "nothing", "None", "NULL", "Null", "NIL", "Nil", "empty", "Empty", "EMPTY"]);
         add_aliases!("BREAK",  ["break"]);
         add_aliases!("CONTINUE", ["continue"]);
         add_aliases!("ASSERT", ["assert", "check"]);
         add_aliases!("TYPEOF", ["typeof", "type_of", "kindof"]);
         add_aliases!("YIELD",  ["yield", "emit"]);
         add_aliases!("RETURN", ["return"]);
         add_aliases!("MATCH",  ["match"]);
         add_aliases!("WHEN",   ["when"]);
         add_aliases!("WITH",   ["with"]);
         add_aliases!("FROM",   ["from"]);
         add_aliases!("CONST",  ["const", "constant"]);
         add_aliases!("EXPORT", ["export"]);
         add_aliases!("AWAIT",  ["await"]);
         // Graphics keywords
         add_aliases!("DRAW",   ["draw", "render"]);
         add_aliases!("COLOR",  ["color", "colour"]);
         add_aliases!("FRAME",  ["frame"]);
         add_aliases!("STEP",   ["step"]);

         add_aliases!("MUT", ["mut"]); // keep MUT available as canonical if needed
         // import is lexed as Identifier and dispatched through call_builtin;
         // the alias entry lets a future ImportStatement token be added cleanly.
         add_aliases!("IMPORT", ["import", "use", "require", "include"]);

         // v1.4.4 pointer system keywords
         add_aliases!("GOTO",  ["goto"]);
         add_aliases!("LOOP",  ["loop"]);
         add_aliases!("PULL",  ["pull"]);
         add_aliases!("PUSH",  ["push"]);
         add_aliases!("ALLOC", ["alloc"]);
         add_aliases!("FREE",  ["free"]);
         add_aliases!("INFO",  ["info"]);
         add_aliases!("REF",   ["ref"]);
         add_aliases!("SEEK",  ["seek"]);
         add_aliases!("SWAP",  ["swap", "Swap", "SWITCH", "Switch", "switch"]);
         add_aliases!("DOES_PARENT_EXIST", ["does_parent_exist", "DoesParentExist"]);

         // Pipeline punctuation aliases (canonical uppercase names)
         map.insert("|".to_string(), "PIPE".to_string());
         map.insert("||".to_string(), "PIPEOR".to_string());
         map.insert("|&|".to_string(), "PIPEBOTH".to_string());
         map.insert("|:|".to_string(), "PIPEMAP".to_string());
         // Spaced forms
         map.insert("| |".to_string(), "PIPE".to_string());
         map.insert("| & |".to_string(), "PIPEBOTH".to_string());
         map.insert("| : |".to_string(), "PIPEMAP".to_string());

         AliasTable { map }
     }

     /// Attempt to load alias_table.json from common locations.
     /// Expected JSON format: { "DO": ["do","run"], "SET": ["set","let"], ... }
     fn load_from_json() -> Option<Self> {
         let candidates = [
             Path::new("alias_table.json"),
             Path::new("src/lexer/alias_table.json"),
             Path::new("resources/alias_table.json"),
         ];

         for p in &candidates {
             if p.exists() {
                 if let Ok(s) = fs::read_to_string(p) {
                     if let Ok(map) = Self::parse_json(&s) {
                         return Some(AliasTable { map });
                     }
                 }
             }
         }
         None
     }

     /// Parse JSON content into the internal map.
     /// Returns a map alias(lowercase) -> canonical(UPPERCASE).
     fn parse_json(s: &str) -> Result<HashMap<String, String>, serde_json::Error> {
         let raw: serde_json::Value = serde_json::from_str(s)?;
         let mut map = HashMap::new();

         if let serde_json::Value::Object(obj) = raw {
             for (canonical, v) in obj.into_iter() {
                 let canonical_up = canonical.to_uppercase();
                 if let serde_json::Value::Array(arr) = v {
                     for item in arr {
                         if let serde_json::Value::String(alias) = item {
                             map.insert(alias.to_lowercase(), canonical_up.clone());
                         }
                     }
                 }
             }
         }
         Ok(map)
     }

     /// Normalize a single word to its canonical token name (uppercase string).
     ///
     /// `word` is matched case-insensitively. `is_do_line` enables contextual
     /// aliasing rules (currently `"before"` -> `OVER` only on DO lines).
     pub fn normalize(&self, word: &str, is_do_line: bool) -> Option<String> {
         let lower = word.to_lowercase();

         // Contextual rule: "before" maps to OVER only on DO lines.
         if lower == "before" && !is_do_line {
             return None;
         }

         self.map.get(&lower).cloned()
     }

     /// Add or override an alias mapping at runtime.
     /// `canonical` may be provided in any case; it will be stored uppercase.
     /// `alias` is stored lowercased for case-insensitive matching.
     pub fn add_alias(&mut self, canonical: &str, alias: &str) {
         self.map.insert(alias.to_lowercase(), canonical.to_uppercase());
     }

     /// Remove an alias (returns true if removed).
     pub fn remove_alias(&mut self, alias: &str) -> bool {
         self.map.remove(&alias.to_lowercase()).is_some()
     }

     /// Inspect the internal map (for debugging/tests).
     /// Returns iterator of (alias_lowercase, canonical_uppercase).
     pub fn entries(&self) -> impl Iterator<Item = (&String, &String)> {
         self.map.iter()
     }

     /// Convenience: check whether a canonical token exists in the table.
     pub fn contains_canonical(&self, canonical: &str) -> bool {
         let up = canonical.to_uppercase();
         self.map.values().any(|v| v == &up)
     }
 }
