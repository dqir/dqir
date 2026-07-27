use backend::compile_function;
use ir::parse;

fn compile_and_run(ir_code: &str, args: &[u64]) -> u64 {
    let module = parse(ir_code.trim()).expect("IR parsing failed");
    let (_, func) = module.functions.iter().next().expect("No functions found in module");
    let exec = compile_function(func).expect("Backend compilation failed");
    exec.run(args)
}

#[test]
fn test_hand_written_simple() {
    let ir = r#"
func @simple() -> i64 {
entry:
    %c: i64 = const.int 42
    ret %c
}
"#;
    assert_eq!(compile_and_run(ir, &[]), 42);
}

#[test]
fn test_arithmetic_and_comparisons() {
    let ir = r#"
func @arith(%a: i64, %b: i64) -> i64 {
entry:
    %sum: i64 = add %a, %b
    %diff: i64 = sub %a, %b
    %prod: i64 = mul %sum, %diff
    %cmp: i1 = cmp gt %prod, %a
    %cmp_i64: i64 = add %cmp, %cmp
    %res: i64 = add %prod, %cmp_i64
    ret %res
}
"#;
    // a=10, b=3 -> sum=13, diff=7, prod=91. cmp=(91 > 10)=1. cmp_i64=2. res=93.
    assert_eq!(compile_and_run(ir, &[10, 3]), 93);
}

#[test]
fn test_factorial_iterative() {
    let ir = r#"
func @factorial(%n: i64) -> i64 {
entry:
    %one: i64 = const.int 1
    jmp loop_head

loop_head:
    %i: i64 = phi [ %n, entry ], [ %next_i, loop_body ]
    %res: i64 = phi [ %one, entry ], [ %next_res, loop_body ]
    %cond: i1 = cmp ugt %i, %one
    br %cond, loop_body, loop_end

loop_body:
    %next_res: i64 = mul %res, %i
    %next_i: i64 = sub %i, %one
    jmp loop_head

loop_end:
    ret %res
}
"#;
    assert_eq!(compile_and_run(ir, &[0]), 1);
    assert_eq!(compile_and_run(ir, &[1]), 1);
    assert_eq!(compile_and_run(ir, &[5]), 120);
    assert_eq!(compile_and_run(ir, &[10]), 3628800);
}

#[test]
fn test_fibonacci() {
    let ir = r#"
func @fib(%n: i64) -> i64 {
entry:
    %zero: i64 = const.int 0
    %one: i64 = const.int 1
    %cond0: i1 = cmp le %n, %zero
    br %cond0, end_zero, check_one

end_zero:
    ret %zero

check_one:
    %cond1: i1 = cmp eq %n, %one
    br %cond1, end_one, loop_head

end_one:
    ret %one

loop_head:
    %i: i64 = phi [ %one, check_one ], [ %next_i, loop_body ]
    %prev: i64 = phi [ %zero, check_one ], [ %curr, loop_body ]
    %curr: i64 = phi [ %one, check_one ], [ %next_val, loop_body ]
    %next_val: i64 = add %prev, %curr
    %next_i: i64 = add %i, %one
    %done: i1 = cmp ge %next_i, %n
    br %done, end_loop, loop_body

loop_body:
    jmp loop_head

end_loop:
    ret %next_val
}
"#;
    assert_eq!(compile_and_run(ir, &[0]), 0);
    assert_eq!(compile_and_run(ir, &[1]), 1);
    assert_eq!(compile_and_run(ir, &[2]), 1);
    assert_eq!(compile_and_run(ir, &[10]), 55);
    assert_eq!(compile_and_run(ir, &[20]), 6765);
}

#[test]
fn test_loop_with_branch_gcd() {
    let ir = r#"
func @gcd(%a: i64, %b: i64) -> i64 {
entry:
    jmp loop_head

loop_head:
    %cur_a: i64 = phi [ %a, entry ], [ %cur_b, loop_body ]
    %cur_b: i64 = phi [ %b, entry ], [ %rem_val, loop_body ]
    %zero: i64 = const.int 0
    %cond: i1 = cmp eq %cur_b, %zero
    br %cond, end, loop_body

loop_body:
    %rem_val: i64 = rem %cur_a, %cur_b
    jmp loop_head

end:
    ret %cur_a
}
"#;
    assert_eq!(compile_and_run(ir, &[48, 18]), 6);
    assert_eq!(compile_and_run(ir, &[101, 103]), 1);
    assert_eq!(compile_and_run(ir, &[54, 24]), 6);
}

#[test]
fn test_memory_operations() {
    let ir = r#"
func @mem_ops(%val: i64) -> i64 {
entry:
    %ptr: ptr = alloca i64
    store %val, %ptr, 0
    %loaded: i64 = load i64, %ptr, 0
    store %loaded, %ptr, 8
    %offset_load: i64 = load i64, %ptr, 8
    ret %offset_load
}
"#;
    assert_eq!(compile_and_run(ir, &[12345]), 12345);
    assert_eq!(compile_and_run(ir, &[999999]), 999999);
}

#[test]
fn test_many_params() {
    let ir = r#"
func @many_params(%p0: i64, %p1: i64, %p2: i64, %p3: i64, %p4: i64, %p5: i64, %p6: i64, %p7: i64, %p8: i64, %p9: i64) -> i64 {
entry:
    %s1: i64 = add %p0, %p1
    %s2: i64 = add %s1, %p2
    %s3: i64 = add %s2, %p3
    %s4: i64 = add %s3, %p4
    %s5: i64 = add %s4, %p5
    %s6: i64 = add %s5, %p6
    %s7: i64 = add %s6, %p7
    %s8: i64 = add %s7, %p8
    %s9: i64 = add %s8, %p9
    ret %s9
}
"#;
    assert_eq!(compile_and_run(ir, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), 55);
    assert_eq!(compile_and_run(ir, &[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 550);
}

#[test]
fn test_high_register_pressure_30_values() {
    let ir = r#"
func @high_pressure() -> i64 {
entry:
    %v0: i64 = const.int 1
    %v1: i64 = const.int 2
    %v2: i64 = const.int 3
    %v3: i64 = const.int 4
    %v4: i64 = const.int 5
    %v5: i64 = const.int 6
    %v6: i64 = const.int 7
    %v7: i64 = const.int 8
    %v8: i64 = const.int 9
    %v9: i64 = const.int 10
    %v10: i64 = const.int 11
    %v11: i64 = const.int 12
    %v12: i64 = const.int 13
    %v13: i64 = const.int 14
    %v14: i64 = const.int 15
    %v15: i64 = const.int 16
    %v16: i64 = const.int 17
    %v17: i64 = const.int 18
    %v18: i64 = const.int 19
    %v19: i64 = const.int 20
    %v20: i64 = const.int 21
    %v21: i64 = const.int 22
    %v22: i64 = const.int 23
    %v23: i64 = const.int 24
    %v24: i64 = const.int 25
    %v25: i64 = const.int 26
    %v26: i64 = const.int 27
    %v27: i64 = const.int 28
    %v28: i64 = const.int 29
    %v29: i64 = const.int 30

    %s1: i64 = add %v0, %v1
    %s2: i64 = add %s1, %v2
    %s3: i64 = add %s2, %v3
    %s4: i64 = add %s3, %v4
    %s5: i64 = add %s4, %v5
    %s6: i64 = add %s5, %v6
    %s7: i64 = add %s6, %v7
    %s8: i64 = add %s7, %v8
    %s9: i64 = add %s8, %v9
    %s10: i64 = add %s9, %v10
    %s11: i64 = add %s10, %v11
    %s12: i64 = add %s11, %v12
    %s13: i64 = add %s12, %v13
    %s14: i64 = add %s13, %v14
    %s15: i64 = add %s14, %v15
    %s16: i64 = add %s15, %v16
    %s17: i64 = add %s16, %v17
    %s18: i64 = add %s17, %v18
    %s19: i64 = add %s18, %v19
    %s20: i64 = add %s19, %v20
    %s21: i64 = add %s20, %v21
    %s22: i64 = add %s21, %v22
    %s23: i64 = add %s22, %v23
    %s24: i64 = add %s23, %v24
    %s25: i64 = add %s24, %v25
    %s26: i64 = add %s25, %v26
    %s27: i64 = add %s26, %v27
    %s28: i64 = add %s27, %v28
    %s29: i64 = add %s28, %v29

    %d0: i64 = sub %s29, %v0
    %d1: i64 = sub %d0, %v1
    %d2: i64 = sub %d1, %v2
    %d3: i64 = sub %d2, %v3
    %d4: i64 = sub %d3, %v4
    %d5: i64 = sub %d4, %v5
    %d6: i64 = sub %d5, %v6
    %d7: i64 = sub %d6, %v7
    %d8: i64 = sub %d7, %v8
    %d9: i64 = sub %d8, %v9
    %d10: i64 = sub %d9, %v10
    %d11: i64 = sub %d10, %v11
    %d12: i64 = sub %d11, %v12
    %d13: i64 = sub %d12, %v13
    %d14: i64 = sub %d13, %v14
    %d15: i64 = sub %d14, %v15
    %d16: i64 = sub %d15, %v16
    %d17: i64 = sub %d16, %v17
    %d18: i64 = sub %d17, %v18
    %d19: i64 = sub %d18, %v19
    %d20: i64 = sub %d19, %v20
    %d21: i64 = sub %d20, %v21
    %d22: i64 = sub %d21, %v22
    %d23: i64 = sub %d22, %v23
    %d24: i64 = sub %d23, %v24
    %d25: i64 = sub %d24, %v25
    %d26: i64 = sub %d25, %v26
    %d27: i64 = sub %d26, %v27
    %d28: i64 = sub %d27, %v28
    %res: i64 = sub %d28, %v29

    ret %res
}
"#;
    assert_eq!(compile_and_run(ir, &[]), 0);
}

#[test]
fn test_high_register_pressure_loop() {
    let ir = r#"
func @pressure_loop(%n: i64) -> i64 {
entry:
    %zero: i64 = const.int 0
    %one: i64 = const.int 1
    %c2: i64 = const.int 2
    %c3: i64 = const.int 3
    %c4: i64 = const.int 4
    %c5: i64 = const.int 5
    %c6: i64 = const.int 6
    %c7: i64 = const.int 7
    %c8: i64 = const.int 8
    %c9: i64 = const.int 9
    %c10: i64 = const.int 10
    %c11: i64 = const.int 11
    %c12: i64 = const.int 12
    %c13: i64 = const.int 13
    %c14: i64 = const.int 14
    %c15: i64 = const.int 15
    %c16: i64 = const.int 16
    %c17: i64 = const.int 17
    %c18: i64 = const.int 18
    %c19: i64 = const.int 19
    %c20: i64 = const.int 20

    jmp loop_head

loop_head:
    %i: i64 = phi [ %zero, entry ], [ %next_i, loop_body ]
    %a0: i64 = phi [ %one, entry ], [ %n0, loop_body ]
    %a1: i64 = phi [ %c2, entry ], [ %n1, loop_body ]
    %a2: i64 = phi [ %c3, entry ], [ %n2, loop_body ]
    %a3: i64 = phi [ %c4, entry ], [ %n3, loop_body ]
    %a4: i64 = phi [ %c5, entry ], [ %n4, loop_body ]
    %a5: i64 = phi [ %c6, entry ], [ %n5, loop_body ]
    %a6: i64 = phi [ %c7, entry ], [ %n6, loop_body ]
    %a7: i64 = phi [ %c8, entry ], [ %n7, loop_body ]
    %a8: i64 = phi [ %c9, entry ], [ %n8, loop_body ]
    %a9: i64 = phi [ %c10, entry ], [ %n9, loop_body ]
    %a10: i64 = phi [ %c11, entry ], [ %n10, loop_body ]
    %a11: i64 = phi [ %c12, entry ], [ %n11, loop_body ]
    %a12: i64 = phi [ %c13, entry ], [ %n12, loop_body ]
    %a13: i64 = phi [ %c14, entry ], [ %n13, loop_body ]
    %a14: i64 = phi [ %c15, entry ], [ %n14, loop_body ]
    %a15: i64 = phi [ %c16, entry ], [ %n15, loop_body ]
    %a16: i64 = phi [ %c17, entry ], [ %n16, loop_body ]
    %a17: i64 = phi [ %c18, entry ], [ %n17, loop_body ]
    %a18: i64 = phi [ %c19, entry ], [ %n18, loop_body ]
    %a19: i64 = phi [ %c20, entry ], [ %n19, loop_body ]

    %cond: i1 = cmp ge %i, %n
    br %cond, end_loop, loop_body

loop_body:
    %n0: i64 = add %a0, %one
    %n1: i64 = add %a1, %one
    %n2: i64 = add %a2, %one
    %n3: i64 = add %a3, %one
    %n4: i64 = add %a4, %one
    %n5: i64 = add %a5, %one
    %n6: i64 = add %a6, %one
    %n7: i64 = add %a7, %one
    %n8: i64 = add %a8, %one
    %n9: i64 = add %a9, %one
    %n10: i64 = add %a10, %one
    %n11: i64 = add %a11, %one
    %n12: i64 = add %a12, %one
    %n13: i64 = add %a13, %one
    %n14: i64 = add %a14, %one
    %n15: i64 = add %a15, %one
    %n16: i64 = add %a16, %one
    %n17: i64 = add %a17, %one
    %n18: i64 = add %a18, %one
    %n19: i64 = add %a19, %one
    %next_i: i64 = add %i, %one
    jmp loop_head

end_loop:
    %sum0: i64 = add %a0, %a1
    %sum1: i64 = add %sum0, %a2
    %sum2: i64 = add %sum1, %a3
    %sum3: i64 = add %sum2, %a4
    %sum4: i64 = add %sum3, %a5
    %sum5: i64 = add %sum4, %a6
    %sum6: i64 = add %sum5, %a7
    %sum7: i64 = add %sum6, %a8
    %sum8: i64 = add %sum7, %a9
    %sum9: i64 = add %sum8, %a10
    %sum10: i64 = add %sum9, %a11
    %sum11: i64 = add %sum10, %a12
    %sum12: i64 = add %sum11, %a13
    %sum13: i64 = add %sum12, %a14
    %sum14: i64 = add %sum13, %a15
    %sum15: i64 = add %sum14, %a16
    %sum16: i64 = add %sum15, %a17
    %sum17: i64 = add %sum16, %a18
    %res: i64 = add %sum17, %a19
    ret %res
}
"#;
    // Initial sum of 1..20 is 20*21/2 = 210.
    // Each iteration adds 1 to each of the 20 variables -> adds 20 to the sum.
    // For n=0: sum is 210.
    // For n=5: sum is 210 + 5*20 = 310.
    // For n=10: sum is 210 + 10*20 = 410.
    assert_eq!(compile_and_run(ir, &[0]), 210);
    assert_eq!(compile_and_run(ir, &[5]), 310);
    assert_eq!(compile_and_run(ir, &[10]), 410);
}
