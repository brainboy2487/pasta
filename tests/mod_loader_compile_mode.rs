use std::fs;
use tempfile::tempdir;

use pasta::{mod_loader, Value};

#[test]
fn compile_mode_returns_exports_not_values() {
    let temp = tempdir().unwrap();
    let module_path = temp.path().join("values.pm");
    fs::write(&module_path,
r#"MOD values:
    export answer, greeter
    answer = 42

    DEF greeter(name):
        RETURN "hi " + name
    END
END
"#,
    )
    .unwrap();

    let module_key = module_path.to_string_lossy().to_string();

    let mut loader = mod_loader::default_loader_with_config(None);
    loader.set_operation_mode(mod_loader::OperationMode::Compile);
    loader.register_use_block(vec![module_key.clone()]).unwrap();
    let module = loader.ensure_loaded(&module_key).unwrap();

    let keys: Vec<String> = module.exports.keys().cloned().collect();
    assert!(keys.contains(&"answer".to_string()));
    assert!(keys.contains(&"greeter".to_string()));

    assert_eq!(module.exports.get("answer").unwrap(), &Value::None);
}
