// src/interpreter/tests/int_api_tests.rs
use crate::interpreter::int_api::{InterpreterApi, ModuleEnvHandle, InterpreterSnapshot, ModuleEnvOps};
use crate::interpreter::environment::Value;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;

/// A mock ModuleEnvOps for testing
struct MockModuleEnv {
    symbols: std::sync::Mutex<HashMap<String, Value>>,
}

impl MockModuleEnv {
    fn new() -> Self {
        Self { symbols: std::sync::Mutex::new(HashMap::new()) }
    }
}

impl ModuleEnvOps for MockModuleEnv {
    fn get_symbol(&self, name: &str) -> Option<Value> {
        self.symbols.lock().unwrap().get(name).cloned()
    }
    fn set_symbol(&self, name: &str, val: Value) {
        self.symbols.lock().unwrap().insert(name.to_string(), val);
    }
    fn execute_top_level(&self, _source: &str) -> Result<()> {
        Ok(())
    }
}

struct MockApi;
impl InterpreterApi for MockApi {
    fn snapshot(&self) -> InterpreterSnapshot {
        InterpreterSnapshot {
            cwd: PathBuf::from("/mock"),
            globals: HashMap::new(),
        }
    }
    fn create_module_env(&self, canonical_path: PathBuf) -> Result<ModuleEnvHandle> {
        Ok(ModuleEnvHandle {
            canonical_path,
            inner: Arc::new(MockModuleEnv::new()),
        })
    }
    fn bind_global(&self, _name: &str, _val: Value) -> Result<()> { Ok(()) }
    fn bind_local(&self, _name: &str, _val: Value) -> Result<()> { Ok(()) }
    fn call_value(&self, _callable: Value, _args: Vec<Value>) -> Result<Value> {
        Ok(Value::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_api_snapshot() {
        let api = MockApi;
        let snap = api.snapshot();
        assert_eq!(snap.cwd, PathBuf::from("/mock"));
    }

    #[test]
    fn test_mock_api_create_module_env() {
        let api = MockApi;
        let handle = api.create_module_env(PathBuf::from("/test/module.pasta")).unwrap();
        assert_eq!(handle.canonical_path, PathBuf::from("/test/module.pasta"));
    }

    #[test]
    fn test_mock_api_call_value() {
        let api = MockApi;
        let result = api.call_value(Value::None, vec![]).unwrap();
        assert!(matches!(result, Value::None));
    }
}
