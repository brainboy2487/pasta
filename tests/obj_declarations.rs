use pasta::{lexer::lexer::Lexer, Executor, Parser, Value};

fn run_script(src: &str) -> anyhow::Result<Executor> {
    let tokens = Lexer::new(src).lex();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let mut exec = Executor::new();
    exec.execute_program(&program)?;
    Ok(exec)
}

#[test]
fn obj_decl_conflicts_with_existing_def() {
    let err = match run_script(
        r#"
DEF Monster(x):
    RETURN x
END

OBJ.NRML Monster(health):
    FIELD hp = health
END
"#,
    ) {
        Ok(_) => panic!("expected OBJ/DEF name conflict"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("OBJ declaration 'Monster' conflicts with existing DEF"),
        "{err}"
    );
}

#[test]
fn obj_decl_constructor_creates_node_and_initializes_fields() {
    let exec = run_script(
        r#"
log = []

OBJ.NRML Monster(health, size):
    FIELD hp = health
    FIELD dims = size
    CONSTRUCTOR(ignored):
        list_append(log, TYPEOF(Monster))
        list_append(log, health)
        list_append(log, size)
    END
END

parent_a = OBJ.NRML.MUT(1, 2)
parent_b = OBJ.NRML.MUT(1, 2)
child = Monster(parent_a, parent_b, 9)
"#,
    )
    .unwrap();

    let child = exec.env.get("child").expect("child binding should exist");
    let child_id = match child {
        Value::FamilyNode { id, .. } => id,
        other => panic!("expected family node, got {:?}", other),
    };

    let child_node = exec
        .family_registry
        .get(child_id)
        .expect("family registry should contain constructed child");
    let parent_a = match exec.env.get("parent_a").unwrap() {
        Value::FamilyNode { id, .. } => id,
        other => panic!("expected family node parent_a, got {:?}", other),
    };
    let parent_b = match exec.env.get("parent_b").unwrap() {
        Value::FamilyNode { id, .. } => id,
        other => panic!("expected family node parent_b, got {:?}", other),
    };
    assert_eq!(child_node.parent_a_id, parent_a);
    assert_eq!(child_node.parent_b_id, parent_b);

    let object = exec
        .objects
        .get(&child_id)
        .expect("internal object storage should exist for constructed family node");
    assert_eq!(object.fields.get("hp"), Some(&Value::Number(9.0)));
    assert_eq!(object.fields.get("dims"), Some(&Value::None));

    let log = exec
        .env
        .get("log")
        .map(|value| exec.deref(value))
        .expect("log should exist");
    assert_eq!(
        log,
        Value::List(vec![
            Value::String("family_node".to_string()),
            Value::Number(9.0),
            Value::None,
        ])
    );
}

#[test]
fn obj_decl_still_accepts_legacy_mut_marker() {
    let exec = run_script(
        r#"
OBJ.NRML.MUT Monster(health):
    FIELD hp = health
END

parent_a = OBJ.NRML.MUT(1, 2)
parent_b = OBJ.NRML.MUT(1, 2)
child = Monster(parent_a, parent_b, 9)
value = child["hp"]
"#,
    )
    .unwrap();

    assert_eq!(exec.env.get("value"), Some(Value::Number(9.0)));
}

#[test]
fn obj_decl_rejects_extra_declared_arguments() {
    let err = match run_script(
        r#"
OBJ.NRML Monster(health):
    FIELD hp = health
END

child = Monster(1, 2, 9, 10)
"#,
    ) {
        Ok(_) => panic!("expected extra-argument error"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("expected at most 1 declared args after parents"),
        "{err}"
    );
}

#[test]
fn obj_decl_fields_can_be_read_and_written_via_brackets() {
    let exec = run_script(
        r#"
OBJ.NRML Monster(health):
    FIELD hp = health
END

parent_a = OBJ.NRML.MUT(1, 2)
parent_b = OBJ.NRML.MUT(1, 2)
child = Monster(parent_a, parent_b, 9)
before = child["hp"]
child["hp"] = 12
after = child["hp"]
"#,
    )
    .unwrap();

    assert_eq!(exec.env.get("before"), Some(Value::Number(9.0)));
    assert_eq!(exec.env.get("after"), Some(Value::Number(12.0)));

    let child_id = match exec.env.get("child").unwrap() {
        Value::FamilyNode { id, .. } => id,
        other => panic!("expected family node child, got {:?}", other),
    };
    let object = exec.objects.get(&child_id).unwrap();
    assert_eq!(object.fields.get("hp"), Some(&Value::Number(12.0)));
}

#[test]
fn obj_decl_rejects_undeclared_field_assignment() {
    let err = match run_script(
        r#"
OBJ.NRML Monster(health):
    FIELD hp = health
END

parent_a = OBJ.NRML.MUT(1, 2)
parent_b = OBJ.NRML.MUT(1, 2)
child = Monster(parent_a, parent_b, 9)
child["mana"] = 5
"#,
    ) {
        Ok(_) => panic!("expected undeclared field assignment error"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("field 'mana' is not declared on object family 'Monster'"),
        "{err}"
    );
}
