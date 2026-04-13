//! Binary module format for precompiled PASTA headers (.phb)
//!
//! Format:
//!   Magic: "PHB1" (4 bytes)
//!   Version: u32
//!   Module name length: u32
//!   Module name: [u8; name_len]
//!   Exports count: u32
//!   Exports: [Export; count]
//!   AST blob length: u32
//!   AST blob: [u8; blob_len] (bincode serialized)

use std::path::Path;
use std::fs;
use std::io::{Read, Write, Cursor};
use anyhow::{Result, anyhow};

use crate::parser::ast::{Statement, Identifier, Span};
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Magic bytes for PHB format
const PHB_MAGIC: &[u8; 4] = b"PHB1";
const PHB_VERSION: u32 = 1;

/// A compiled binary module
#[derive(Debug, Clone)]
pub struct BinaryModule {
    pub name: String,
    pub version: u32,
    pub exports: Vec<String>,
    pub statements: Vec<Statement>,
}

impl BinaryModule {
    /// Compile a .ph source file to a BinaryModule
    pub fn compile_source(name: &str, source: &str) -> Result<Self> {
        let tokens = Lexer::new(source).lex();
        let mut parser = Parser::new(tokens);
        let (program, _diags) = parser.parse_with_diagnostics();
        
        // Extract exports (all top-level function defs and assignments)
        let mut exports = Vec::new();
        for stmt in &program {
            match stmt {
                Statement::FunctionDef { name, .. } => {
                    exports.push(name.name.clone());
                }
                Statement::Assignment { target, .. } => {
                    // Export top-level assignments (constants, etc)
                    if !target.name.starts_with("__") {
                        exports.push(target.name.clone());
                    }
                }
                _ => {}
            }
        }
        
        Ok(BinaryModule {
            name: name.to_string(),
            version: PHB_VERSION,
            exports,
            statements: program,
        })
    }
    
    /// Serialize to binary format
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        
        // Magic
        buf.extend_from_slice(PHB_MAGIC);
        
        // Version
        buf.extend_from_slice(&self.version.to_le_bytes());
        
        // Module name
        let name_bytes = self.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        
        // Exports
        buf.extend_from_slice(&(self.exports.len() as u32).to_le_bytes());
        for exp in &self.exports {
            let exp_bytes = exp.as_bytes();
            buf.extend_from_slice(&(exp_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(exp_bytes);
        }
        
        // AST blob - use JSON for now (could use bincode for more speed)
        let ast_json = serde_json::to_vec(&self.statements)
            .map_err(|e| anyhow!("Failed to serialize AST: {}", e))?;
        buf.extend_from_slice(&(ast_json.len() as u32).to_le_bytes());
        buf.extend_from_slice(&ast_json);
        
        Ok(buf)
    }
    
    /// Deserialize from binary format
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        let mut buf4 = [0u8; 4];
        
        // Magic
        cursor.read_exact(&mut buf4)?;
        if &buf4 != PHB_MAGIC {
            return Err(anyhow!("Invalid PHB magic bytes"));
        }
        
        // Version
        cursor.read_exact(&mut buf4)?;
        let version = u32::from_le_bytes(buf4);
        if version > PHB_VERSION {
            return Err(anyhow!("PHB version {} not supported (max {})", version, PHB_VERSION));
        }
        
        // Module name
        cursor.read_exact(&mut buf4)?;
        let name_len = u32::from_le_bytes(buf4) as usize;
        let mut name_buf = vec![0u8; name_len];
        cursor.read_exact(&mut name_buf)?;
        let name = String::from_utf8(name_buf)?;
        
        // Exports
        cursor.read_exact(&mut buf4)?;
        let export_count = u32::from_le_bytes(buf4) as usize;
        let mut exports = Vec::with_capacity(export_count);
        for _ in 0..export_count {
            cursor.read_exact(&mut buf4)?;
            let exp_len = u32::from_le_bytes(buf4) as usize;
            let mut exp_buf = vec![0u8; exp_len];
            cursor.read_exact(&mut exp_buf)?;
            exports.push(String::from_utf8(exp_buf)?);
        }
        
        // AST blob
        cursor.read_exact(&mut buf4)?;
        let ast_len = u32::from_le_bytes(buf4) as usize;
        let mut ast_buf = vec![0u8; ast_len];
        cursor.read_exact(&mut ast_buf)?;
        let statements: Vec<Statement> = serde_json::from_slice(&ast_buf)
            .map_err(|e| anyhow!("Failed to deserialize AST: {}", e))?;
        
        Ok(BinaryModule {
            name,
            version,
            exports,
            statements,
        })
    }
    
    /// Compile a .ph file and write to .phb
    pub fn compile_file(ph_path: &Path) -> Result<()> {
        let source = fs::read_to_string(ph_path)?;
        let name = ph_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        
        let module = Self::compile_source(name, &source)?;
        let bytes = module.to_bytes()?;
        
        let phb_path = ph_path.with_extension("phb");
        fs::write(&phb_path, bytes)?;
        
        Ok(())
    }
    
    /// Load a module - tries .phb first, falls back to .ph
    pub fn load(base_path: &Path) -> Result<Self> {
        let phb_path = base_path.with_extension("phb");
        let ph_path = base_path.with_extension("ph");
        
        // Try binary first
        if phb_path.exists() {
            let data = fs::read(&phb_path)?;
            return Self::from_bytes(&data);
        }
        
        // Fall back to source
        if ph_path.exists() {
            let source = fs::read_to_string(&ph_path)?;
            let name = base_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            return Self::compile_source(name, &source);
        }
        
        Err(anyhow!("Module not found: {:?}", base_path))
    }
}

/// Compile all .ph files in a directory to .phb
pub fn compile_stdlib(stdlib_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(stdlib_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "ph").unwrap_or(false) {
            println!("Compiling {:?}...", path);
            if let Err(e) = BinaryModule::compile_file(&path) {
                eprintln!("  Warning: Failed to compile {:?}: {}", path, e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_roundtrip() {
        let source = r#"
set VERSION = "1.0"
DEF add(a, b):
    RETURN a + b
END
"#;
        let module = BinaryModule::compile_source("test", source).unwrap();
        let bytes = module.to_bytes().unwrap();
        let restored = BinaryModule::from_bytes(&bytes).unwrap();
        
        assert_eq!(module.name, restored.name);
        assert_eq!(module.exports.len(), restored.exports.len());
    }
}
