//! Binary module format for PASTA (.phb files)
//!
//! This module provides functionality for compiling PASTA source files (.ph)
//! into binary modules (.phb) and loading them at runtime.
//!
//! Binary format:
//!   - Magic: "PHB\x01" (4 bytes)
//!   - Version: u32 (4 bytes)  
//!   - Checksum: u32 (4 bytes) - CRC32 of serialized AST
//!   - AST length: u64 (8 bytes)
//!   - AST data: bincode-serialized Program

use crate::parser::ast::Program;
use crate::parser::Parser;
use crate::lexer::Lexer;
use std::path::Path;
use std::fs;

/// Magic bytes for .phb files
const PHB_MAGIC: &[u8; 4] = b"PHB\x01";

/// Current binary format version
const PHB_VERSION: u32 = 1;

/// Result type for binary module operations
pub type BinaryResult<T> = Result<T, BinaryError>;

/// Errors that can occur during binary module operations
#[derive(Debug)]
pub enum BinaryError {
    /// File I/O error
    Io(std::io::Error),
    /// Invalid magic bytes
    InvalidMagic,
    /// Unsupported version
    UnsupportedVersion(u32),
    /// Checksum mismatch
    ChecksumMismatch,
    /// Serialization error
    Serialization(String),
    /// Parse error
    Parse(String),
}

impl std::fmt::Display for BinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryError::Io(e) => write!(f, "I/O error: {}", e),
            BinaryError::InvalidMagic => write!(f, "Invalid .phb magic bytes"),
            BinaryError::UnsupportedVersion(v) => write!(f, "Unsupported .phb version: {}", v),
            BinaryError::ChecksumMismatch => write!(f, "Checksum mismatch in .phb file"),
            BinaryError::Serialization(s) => write!(f, "Serialization error: {}", s),
            BinaryError::Parse(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl From<std::io::Error> for BinaryError {
    fn from(e: std::io::Error) -> Self {
        BinaryError::Io(e)
    }
}

/// Simple CRC32 checksum
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Compile a PASTA source file to a binary module
pub fn compile_to_binary(source: &str) -> BinaryResult<Vec<u8>> {
    // Lex
    let tokens = Lexer::new(source).lex();
    
    // Parse
    let mut parser = Parser::new(tokens);
    let (program, errors) = parser.parse_with_diagnostics();
    
    if !errors.is_empty() {
        return Err(BinaryError::Parse(
            errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; ")
        ));
    }
    
    // Serialize using JSON (serde_json is already a dependency)
    let ast_json = serde_json::to_vec(&program)
        .map_err(|e| BinaryError::Serialization(e.to_string()))?;
    
    let checksum = crc32(&ast_json);
    
    // Build binary
    let mut binary = Vec::with_capacity(20 + ast_json.len());
    binary.extend_from_slice(PHB_MAGIC);
    binary.extend_from_slice(&PHB_VERSION.to_le_bytes());
    binary.extend_from_slice(&checksum.to_le_bytes());
    binary.extend_from_slice(&(ast_json.len() as u64).to_le_bytes());
    binary.extend_from_slice(&ast_json);
    
    Ok(binary)
}

/// Compile a .ph file to a .phb file
pub fn compile_file(source_path: &Path, output_path: &Path) -> BinaryResult<()> {
    let source = fs::read_to_string(source_path)?;
    let binary = compile_to_binary(&source)?;
    fs::write(output_path, binary)?;
    Ok(())
}

/// Load a binary module from bytes
pub fn load_from_binary(data: &[u8]) -> BinaryResult<Program> {
    if data.len() < 20 {
        return Err(BinaryError::InvalidMagic);
    }
    
    // Check magic
    if &data[0..4] != PHB_MAGIC {
        return Err(BinaryError::InvalidMagic);
    }
    
    // Check version
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != PHB_VERSION {
        return Err(BinaryError::UnsupportedVersion(version));
    }
    
    // Read checksum
    let stored_checksum = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    
    // Read AST length
    let ast_len = u64::from_le_bytes([
        data[12], data[13], data[14], data[15],
        data[16], data[17], data[18], data[19],
    ]) as usize;
    
    if data.len() < 20 + ast_len {
        return Err(BinaryError::InvalidMagic);
    }
    
    let ast_data = &data[20..20 + ast_len];
    
    // Verify checksum
    let computed_checksum = crc32(ast_data);
    if computed_checksum != stored_checksum {
        return Err(BinaryError::ChecksumMismatch);
    }
    
    // Deserialize
    let program: Program = serde_json::from_slice(ast_data)
        .map_err(|e| BinaryError::Serialization(e.to_string()))?;
    
    Ok(program)
}

/// Load a binary module from a file
pub fn load_file(path: &Path) -> BinaryResult<Program> {
    let data = fs::read(path)?;
    load_from_binary(&data)
}

/// Check if a .phb file is up to date compared to its .ph source
pub fn is_binary_current(source_path: &Path, binary_path: &Path) -> bool {
    if !binary_path.exists() {
        return false;
    }
    
    let source_modified = fs::metadata(source_path)
        .and_then(|m| m.modified())
        .ok();
    let binary_modified = fs::metadata(binary_path)
        .and_then(|m| m.modified())
        .ok();
    
    match (source_modified, binary_modified) {
        (Some(s), Some(b)) => b >= s,
        _ => false,
    }
}

/// Load a module, preferring binary if available and current
pub fn load_module(source_path: &Path) -> BinaryResult<Program> {
    let binary_path = source_path.with_extension("phb");
    
    // Check if binary is current
    if is_binary_current(source_path, &binary_path) {
        // Try to load binary
        match load_file(&binary_path) {
            Ok(program) => return Ok(program),
            Err(_) => {
                // Binary is corrupt, recompile
            }
        }
    }
    
    // Compile from source
    let source = fs::read_to_string(source_path)?;
    let binary = compile_to_binary(&source)?;
    
    // Try to save binary (ignore errors - it's just a cache)
    let _ = fs::write(&binary_path, &binary);
    
    load_from_binary(&binary)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compile_and_load() {
        let source = r#"
            set x = 10
            PRINT x
        "#;
        
        let binary = compile_to_binary(source).unwrap();
        let program = load_from_binary(&binary).unwrap();
        
        assert!(!program.statements.is_empty());
    }
    
    #[test]
    fn test_crc32() {
        let data = b"Hello, World!";
        let crc = crc32(data);
        // Verify it's deterministic
        assert_eq!(crc, crc32(data));
    }
}
