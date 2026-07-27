use backend::compile;
use query::Session;

/// Generates a synthetic module with `n` functions to simulate monomorphized/duplicate functions.
pub fn generate_synthetic_module(n: usize) -> String {
    let mut code = String::new();
    for i in 0..n {
        code.push_str(&format!(
            "func @fn_{}(%val: i64) -> i64 {{\nentry:\n    %c: i64 = const.int {}\n    %res: i64 = add %val, %c\n    ret %res\n}}\n\n",
            i, i
        ));
    }
    code
}

/// Runs an incremental compilation benchmark over `n` functions using query::Session.
pub fn bench_session_incremental(n: usize) -> usize {
    let source = generate_synthetic_module(n);
    let mut session = Session::new(&source);
    
    // Initial compile of entire module
    let _ = session.compile_module().expect("initial compilation failed");
    session.clear_logs();

    // Modify only one function in the module
    let mut updated_source = source.clone();
    updated_source = updated_source.replace("const.int 0", "const.int 999");
    session.update_source(&updated_source);

    let _ = session.compile_module().expect("incremental compilation failed");
    session.get_logs().len()
}

/// Runs a one-shot compilation benchmark using backend::compile.
pub fn bench_oneshot_backend(ir_text: &str, fn_name: &str) -> u64 {
    let exec = compile(ir_text, fn_name).expect("one-shot backend compile failed");
    exec.run(&[10])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_session_incremental() {
        let n = 20;
        let logs_count = bench_session_incremental(n);
        // 1 (link) + 6 (fn_0 pipeline) + 19 (untouched fn_ir early cutoff checks) = 26 queries.
        assert_eq!(logs_count, n + 6);
    }

    #[test]
    fn test_bench_oneshot() {
        let ir = "func @add_val(%val: i64) -> i64 {\nentry:\n    %c: i64 = const.int 32\n    %res: i64 = add %val, %c\n    ret %res\n}\n";
        let result = bench_oneshot_backend(ir, "add_val");
        assert_eq!(result, 42);
    }
}
