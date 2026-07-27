use ir::parser::parse;

fn assert_round_trip(input: &str) {
    let input_trimmed = input.trim();
    let mod1 = parse(input_trimmed).expect("Failed to parse initial input");
    let printed1 = mod1.to_string();
    assert_eq!(
        input_trimmed,
        printed1.trim(),
        "Initial print does not match canonical input:\n--- EXPECTED ---\n{}\n--- ACTUAL ---\n{}\n",
        input_trimmed,
        printed1.trim()
    );

    let mod2 = parse(&printed1).expect("Failed to re-parse printed output");
    let printed2 = mod2.to_string();
    assert_eq!(
        printed1, printed2,
        "Second print does not match first print:\n--- FIRST ---\n{}\n--- SECOND ---\n{}\n",
        printed1, printed2
    );
    assert_eq!(mod1, mod2, "Parsed AST structures differ after round-trip");
}

#[test]
fn test_01_simple_arithmetic() {
    let ir = r#"
func @arithmetic(%a: i64, %b: i64) -> i64 {
entry:
    %sum: i64 = add %a, %b
    %diff: i64 = sub %sum, %b
    %prod: i64 = mul %diff, %a
    %quot: i64 = div %prod, %b
    %rem: i64 = rem %quot, %a
    ret %rem
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_02_bitwise_operations() {
    let ir = r#"
func @bitwise(%x: i32, %y: i32) -> i32 {
entry:
    %and_val: i32 = and %x, %y
    %or_val: i32 = or %and_val, %x
    %xor_val: i32 = xor %or_val, %y
    %shl_val: i32 = shl %xor_val, %x
    %shr_val: i32 = shr %shl_val, %y
    %sar_val: i32 = sar %shr_val, %x
    %not_val: i32 = not %sar_val
    ret %not_val
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_03_comparisons() {
    let ir = r#"
func @compare(%a: i64, %b: i64) -> i1 {
entry:
    %eq: i1 = cmp eq %a, %b
    %ne: i1 = cmp ne %a, %b
    %lt: i1 = cmp lt %a, %b
    %le: i1 = cmp le %a, %b
    %gt: i1 = cmp gt %a, %b
    %ge: i1 = cmp ge %a, %b
    %ult: i1 = cmp ult %a, %b
    %ule: i1 = cmp ule %a, %b
    %ugt: i1 = cmp ugt %a, %b
    %uge: i1 = cmp uge %a, %b
    ret %uge
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_04_constants() {
    let ir = r#"
func @constants() -> f64 {
entry:
    %i: i64 = const.int -42
    %f1: f64 = const.float 3.14159
    %f2: f64 = const.float 1.0
    %neg_i: i64 = neg %i
    ret %f1
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_05_memory_operations() {
    let ir = r#"
func @memory_ops(%val: i64) -> i64 {
entry:
    %ptr: ptr = alloca i64
    store %val, %ptr, 0
    %loaded: i64 = load i64, %ptr, 0
    store %loaded, %ptr, 8
    %offset_load: i64 = load i64, %ptr, 8
    ret %offset_load
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_06_unconditional_branch() {
    let ir = r#"
func @unconditional() -> i32 {
entry:
    jmp block1

block1:
    %val: i32 = const.int 100
    jmp block2

block2:
    ret %val
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_07_conditional_branch() {
    let ir = r#"
func @conditional(%n: i64) -> i64 {
entry:
    %zero: i64 = const.int 0
    %cond: i1 = cmp eq %n, %zero
    br %cond, then_block, else_block

then_block:
    %one: i64 = const.int 1
    ret %one

else_block:
    ret %n
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_08_ssa_join_phi() {
    let ir = r#"
func @abs_value(%x: i64) -> i64 {
entry:
    %zero: i64 = const.int 0
    %is_neg: i1 = cmp lt %x, %zero
    br %is_neg, neg_block, pos_block

neg_block:
    %neg_x: i64 = neg %x
    jmp merge_block

pos_block:
    jmp merge_block

merge_block:
    %res: i64 = phi [ %neg_x, neg_block ], [ %x, pos_block ]
    ret %res
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_09_function_calls() {
    let ir = r#"
extern func @malloc(i64) -> ptr
extern func @free(ptr) -> void
extern func @puts(ptr) -> i32

func @test_calls(%size: i64) -> i32 {
entry:
    %buf: ptr = call @malloc(%size)
    %status: i32 = call @puts(%buf)
    call @free(%buf)
    ret %status
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_10_loop_with_phi() {
    let ir = r#"
func @sum_n(%n: i64) -> i64 {
entry:
    %zero: i64 = const.int 0
    %one: i64 = const.int 1
    jmp loop_header

loop_header:
    %i: i64 = phi [ %zero, entry ], [ %next_i, loop_body ]
    %acc: i64 = phi [ %zero, entry ], [ %next_acc, loop_body ]
    %cond: i1 = cmp lt %i, %n
    br %cond, loop_body, loop_exit

loop_body:
    %next_i: i64 = add %i, %one
    %next_acc: i64 = add %acc, %i
    jmp loop_header

loop_exit:
    ret %acc
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_11_factorial_iterative() {
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
    assert_round_trip(ir);
}

#[test]
fn test_12_nested_control_flow() {
    let ir = r#"
func @nested(%a: i32, %b: i32, %c: i32) -> i32 {
entry:
    %cond1: i1 = cmp gt %a, %b
    br %cond1, outer_then, outer_else

outer_then:
    %cond2: i1 = cmp gt %a, %c
    br %cond2, inner_then1, inner_else1

inner_then1:
    ret %a

inner_else1:
    ret %c

outer_else:
    %cond3: i1 = cmp gt %b, %c
    br %cond3, inner_then2, inner_else2

inner_then2:
    ret %b

inner_else2:
    ret %c
}
"#;
    assert_round_trip(ir);
}

#[test]
fn test_13_gcd_euclidean() {
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
    assert_round_trip(ir);
}

#[test]
fn test_14_multiple_externs_and_functions() {
    let ir = r#"
extern func @read() -> i64
extern func @write(i64) -> void

func @compute(%val: i64) -> i64 {
entry:
    %two: i64 = const.int 2
    %res: i64 = mul %val, %two
    ret %res
}

func @main() -> void {
entry:
    %in_val: i64 = call @read()
    %out_val: i64 = call @compute(%in_val)
    call @write(%out_val)
    ret
}
"#;
    assert_round_trip(ir);
}
