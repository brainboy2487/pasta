use pasta::{Executor, Value};

const COLOR_BLACK: u32 = 0xFF000000;
const COLOR_BLUE: u32 = 0xFF0000FF;
const COLOR_GREEN: u32 = 0xFF00FF00;
const COLOR_RED: u32 = 0xFFFF0000;

fn run_script(src: &str) -> Executor {
    let mut exe = Executor::new();
    let program = Executor::parse(src);
    exe.execute_program(&program)
        .expect("script should execute");
    exe
}

fn handle_from_env(exe: &Executor, name: &str) -> String {
    match exe.env.get(name) {
        Some(Value::String(handle)) => handle,
        other => panic!("expected string handle in {name}, got {other:?}"),
    }
}

#[test]
fn draw_to_grid_uses_active_draw_target_and_current_color() {
    let exe = run_script(
        r#"
win = WINDOW("grid-test", 6, 4)
SET_DRAW_TARGET(win)
SET_COLOR(4278255360)
DRAW_GRID(2, 2)
DRAW_TO_GRID(1, 0)
"#,
    );

    let handle = handle_from_env(&exe, "win");
    let canvas = exe.back_buffers.get(&handle).expect("window back buffer");

    assert_eq!(canvas.get_pixel(0, 0), COLOR_BLACK);
    assert_eq!(canvas.get_pixel(2, 0), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(3, 1), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(4, 0), COLOR_BLACK);
}

#[test]
fn draw_grid_batch_supports_current_syntax_lists_dicts_and_list_append() {
    let exe = run_script(
        r#"
c = CANVAS(6, 4)
DRAW_GRID(c, 2, 2)
cells = []
list_append(cells, [0, 0, 4278190335])
SET_COLOR(4294901760)
list_append(cells, [1, 0])
list_append(cells, {"x": 2, "y": 0, "color": 4278255360})
DRAW_GRID_BATCH(c, cells)
"#,
    );

    let handle = handle_from_env(&exe, "c");
    let canvas = exe.canvases.get(&handle).expect("canvas handle");

    assert_eq!(canvas.get_pixel(0, 0), COLOR_BLUE);
    assert_eq!(canvas.get_pixel(2, 0), COLOR_RED);
    assert_eq!(canvas.get_pixel(4, 0), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(5, 1), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(0, 3), COLOR_BLACK);
}

#[test]
fn draw_grid_runs_fill_horizontal_spans() {
    let exe = run_script(
        r#"
c = CANVAS(10, 4)
DRAW_GRID(c, 2, 2)
DRAW_GRID_RUNS(c, [
    [1, 0, 3, 4278255360],
    {"x": 0, "y": 1, "len": 2, "color": 4278190335}
])
"#,
    );

    let handle = handle_from_env(&exe, "c");
    let canvas = exe.canvases.get(&handle).expect("canvas handle");

    assert_eq!(canvas.get_pixel(2, 0), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(7, 1), COLOR_GREEN);
    assert_eq!(canvas.get_pixel(0, 2), COLOR_BLUE);
    assert_eq!(canvas.get_pixel(3, 3), COLOR_BLUE);
    assert_eq!(canvas.get_pixel(8, 0), COLOR_BLACK);
}

#[test]
fn draw_to_grid_requires_draw_grid_first() {
    let mut exe = Executor::new();
    let program = Executor::parse(
        r#"
win = WINDOW("grid-test", 4, 4)
SET_DRAW_TARGET(win)
DRAW_TO_GRID(0, 0, 4278255360)
"#,
    );

    let err = exe
        .execute_program(&program)
        .expect_err("missing DRAW_GRID should fail");
    assert!(
        err.to_string().contains("DRAW_GRID must be called"),
        "unexpected error: {err}"
    );
}

#[test]
#[ignore]
fn benchmark_grid_batch_vs_individual_draws() {
    let mut exe = Executor::new();
    let handle = match exe
        .call_builtin("canvas", vec![Value::Number(256.0), Value::Number(256.0)])
        .expect("canvas should be created")
    {
        Value::String(handle) => handle,
        other => panic!("expected canvas handle, got {other:?}"),
    };
    exe.call_builtin(
        "draw_grid",
        vec![
            Value::String(handle.clone()),
            Value::Number(4.0),
            Value::Number(4.0),
        ],
    )
    .expect("grid config should succeed");

    let cells: Vec<Value> = (0..64)
        .flat_map(|y| {
            (0..64).map(move |x| {
                Value::List(vec![
                    Value::Number(x as f64),
                    Value::Number(y as f64),
                    Value::Number(COLOR_GREEN as f64),
                ])
            })
        })
        .collect();
    let runs: Vec<Value> = (0..64)
        .map(|y| {
            Value::List(vec![
                Value::Number(0.0),
                Value::Number(y as f64),
                Value::Number(64.0),
                Value::Number(COLOR_GREEN as f64),
            ])
        })
        .collect();

    let start_individual = std::time::Instant::now();
    for _ in 0..20 {
        for y in 0..64 {
            for x in 0..64 {
                exe.call_builtin(
                    "draw_to_grid",
                    vec![
                        Value::String(handle.clone()),
                        Value::Number(x as f64),
                        Value::Number(y as f64),
                        Value::Number(COLOR_GREEN as f64),
                    ],
                )
                .expect("individual draw should succeed");
            }
        }
    }
    let individual = start_individual.elapsed();

    let start_batch = std::time::Instant::now();
    for _ in 0..20 {
        exe.call_builtin(
            "draw_grid_batch",
            vec![Value::String(handle.clone()), Value::List(cells.clone())],
        )
        .expect("batch draw should succeed");
    }
    let batch = start_batch.elapsed();

    let start_runs = std::time::Instant::now();
    for _ in 0..20 {
        exe.call_builtin(
            "draw_grid_runs",
            vec![Value::String(handle.clone()), Value::List(runs.clone())],
        )
        .expect("run draw should succeed");
    }
    let run_batch = start_runs.elapsed();

    println!("individual={individual:?} batch={batch:?} runs={run_batch:?}");
    assert!(
        batch < individual,
        "batch path should beat individual draws"
    );
    assert!(
        run_batch < batch,
        "run path should beat per-cell batch draws"
    );
}
