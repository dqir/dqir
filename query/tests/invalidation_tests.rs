use query::{emit, link, CompilerDatabase, Db, FunctionName, SourceInput};
use salsa::Setter;

fn generate_50_function_module(modify_idx: Option<usize>) -> String {
    let mut ir = String::new();
    for i in 0..50 {
        let val = if Some(i) == modify_idx { 9999 } else { i };
        ir.push_str(&format!(
            "func @fn_{}(%x: i64) -> i64 {{\nentry:\n    %c: i64 = const.int {}\n    %res: i64 = add %x, %c\n    ret %res\n}}\n\n",
            i, val
        ));
    }
    ir
}

#[test]
fn test_50_function_incremental_compilation_granularity() {
    let mut db = CompilerDatabase::default();
    let initial_ir = generate_50_function_module(None);
    let input = SourceInput::new(&db, initial_ir);

    // Initial full compilation
    let bytes_v1 = link(&db, input).clone();
    assert!(!bytes_v1.is_empty(), "Initial compilation should produce machine code");

    let initial_logs = db.get_logs();
    let initial_emit_count = initial_logs.iter().filter(|e| e.query == "emit").count();
    assert_eq!(
        initial_emit_count, 50,
        "Initial compilation must emit machine code for all 50 functions, got {}",
        initial_emit_count
    );

    // Clear logs and modify ONLY function @fn_25
    db.clear_logs();
    let modified_ir = generate_50_function_module(Some(25));
    input.set_text(&mut db).to(modified_ir);

    // Re-run compilation
    let bytes_v2 = link(&db, input).clone();
    assert_ne!(
        bytes_v1, bytes_v2,
        "Modified compilation must produce different machine code bytes"
    );

    let logs = db.get_logs();
    let pipeline_queries: Vec<_> = logs
        .iter()
        .filter(|e| {
            matches!(
                e.query.as_str(),
                "optimize_ir" | "allocate_registers" | "select_instructions" | "schedule" | "emit"
            )
        })
        .collect();

    // Verify that NO function other than fn_25 recomputed any pipeline query
    for entry in &pipeline_queries {
        assert_eq!(
            entry.fn_name, "fn_25",
            "CRITICAL FAILURE: Function @{} recomputed query '{}' when only @fn_25 was modified!",
            entry.fn_name, entry.query
        );
    }

    // Verify that fn_25 recomputed exactly its 5 pipeline queries once
    assert_eq!(
        pipeline_queries.len(),
        5,
        "Expected exactly 5 pipeline query recomputations (optimize, regalloc, select, schedule, emit) for @fn_25, got {}: {:?}",
        pipeline_queries.len(),
        pipeline_queries
    );
}

#[test]
fn test_individual_query_caching() {
    let db = CompilerDatabase::default();
    let ir = "func @foo() -> i64 {\nentry:\n    %c: i64 = const.int 42\n    ret %c\n}\n";
    let input = SourceInput::new(&db, ir.to_string());
    let name = FunctionName::new(&db, "foo".to_string());

    let _ = emit(&db, input, name);
    assert_eq!(db.get_logs().len(), 6); // fn_ir, optimize_ir, allocate_registers, select_instructions, schedule, emit

    db.clear_logs();
    // Re-request emit without any input changes
    let _ = emit(&db, input, name);
    assert!(
        db.get_logs().is_empty(),
        "Second request without changes must hit cache 100%, but executed: {:?}",
        db.get_logs()
    );
}

#[test]
fn test_salsa_jit_end_to_end_execution() {
    let mut db = CompilerDatabase::default();
    let ir_v1 = r#"
func @add_val(%x: i64) -> i64 {
entry:
    %c: i64 = const.int 10
    %res: i64 = add %x, %c
    ret %res
}
"#;
    let input = SourceInput::new(&db, ir_v1.to_string());
    let name = FunctionName::new(&db, "add_val".to_string());

    let bytes_v1 = emit(&db, input, name).clone();
    let exec_v1 = backend::Executable::new("add_val", &bytes_v1).expect("JIT memory allocation should succeed");
    let result_v1 = exec_v1.run(&[5]);
    assert_eq!(result_v1, 15, "5 + 10 should be 15");

    // Modify the function to add 100 instead of 10
    let ir_v2 = r#"
func @add_val(%x: i64) -> i64 {
entry:
    %c: i64 = const.int 100
    %res: i64 = add %x, %c
    ret %res
}
"#;
    input.set_text(&mut db).to(ir_v2.to_string());

    let name_v2 = FunctionName::new(&db, "add_val".to_string());
    let bytes_v2 = emit(&db, input, name_v2).clone();
    let exec_v2 = backend::Executable::new("add_val", &bytes_v2).expect("JIT memory allocation should succeed");
    let result_v2 = exec_v2.run(&[5]);
    assert_eq!(result_v2, 105, "5 + 100 should be 105");
}
