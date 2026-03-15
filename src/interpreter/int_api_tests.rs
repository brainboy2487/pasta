// src/interpreter/tests/int_api_tests.rs
use crate::interpreter::int_api::{InterpreterApi, ModuleEnvHandle, InterpreterSnapshot};
use crate::interpreter::environment::Value;
use anyhow::Result;
use std::path::PathBuf;

struct MockApi;
impl InterpreterApi for MockApi {
    fn snapshot(&self) -> InterpreterSnapshot { /* return minimal snapshot */ }
    fn create_module_env(&self, canonical_path: PathBuf) -> Result<ModuleEnvHandle> { unimplemented!() }
    fn bind_global(&self, _name: &str, _val: Value) -> Result<()> { Ok(()) }
    fn bind_local(&self, _name: &str, _val: Value) -> Result<()> { Ok(()) }
    fn call_value(&self, _callable: Value, _args: Vec<Value>) -> Result<Value> { unimplemented!() }
}
