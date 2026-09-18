use std::fs;

#[test]
fn parser_ps_has_expected_constructors_and_variants() {
    let parser_ps = fs::read_to_string("src/parser/parser.ps").expect("read parser.ps");
    let ast_ps = fs::read_to_string("src/parser/ast.ps").expect("read ast.ps");

    let expr_variants = vec![
        "Number", "String", "Bool", "None", "Identifier", "ConstructorCall", "Combine",
        "Reassign", "Binary", "Call", "List", "Raw", "Lambda", "TensorBuilder",
        "Index", "Ref", "ObjFamNew", "DoesParentExist", "Dict",
    ];
    let stmt_variants = vec![
        "Assignment", "ConstAssignment", "MultiAssignment", "IndexAssignment", "ObjDecl",
        "SpawnBlock", "DefDoUntil", "DoBlock", "FunctionDef", "WhileBlock", "ForIn",
        "ModuleDecl", "Break", "Continue", "PriorityOverride", "Constraint", "ExprStmt",
        "FromBlock", "Print", "If", "End", "RetNow", "RetLate", "AttemptBlock", "TryBlock",
        "LoopBlock", "GotoLabel", "GotoBlock", "Pull", "Push", "Alloc", "Free", "Info", "Seek", "Swap",
        "Other", "UseUnsafe",
    ];

    for v in expr_variants.iter() {
        let def_name = format!("DEF Expr_{}", v);
        let alt = format!("\"type\": \"{}\"", v);
        assert!(ast_ps.contains(&def_name) || parser_ps.contains(&alt) || ast_ps.contains(&alt),
            "Missing expr variant {} in ast.ps or parser.ps", v);
    }

    for v in stmt_variants.iter() {
        let def_name = format!("DEF St_{}", v);
        let alt = format!("\"type\": \"{}\"", v);
        assert!(ast_ps.contains(&def_name) || parser_ps.contains(&alt) || ast_ps.contains(&alt),
            "Missing stmt variant {} in ast.ps or parser.ps", v);
    }
}
