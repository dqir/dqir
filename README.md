# DQIR: Demand-Driven Query Intermediate Representation

DQIR is an experimental compiler backend (IR-in, machine-code-out) built around a core architectural principle that neither LLVM nor Cranelift currently address: treating code generation itself as an incremental, cached, dependency-tracked computation rather than a traditional batch pipeline.

## The Motivation and Tradeoff

### The Problem
Traditional compiler backends optimize for either fast cold compiles (such as Cranelift) or high-quality generated machine code (such as LLVM), but neither is architected for fine-grained incremental recomputation. When a developer edits a single function inside a large module, traditional backends redo far more work than necessary because nothing tracks data dependencies at the instruction selection, register allocation, or machine-code emission levels.

This cost is not uniform across languages — it is concentrated specifically wherever a compilation pipeline produces large volumes of *structurally duplicate* backend work. The clearest real-world driver of this is generic monomorphization: when a generic function is instantiated for a concrete type in multiple crates, today's compilers re-instantiate, re-optimize, and re-codegen that function from scratch in every crate that uses it, even when the resulting machine code is byte-for-byte identical. Rust's own compiler team has tracked this exact duplication as an open problem, and real-world profiling of Rust builds has found individual heavily-monomorphized crates responsible for double-digit-second compile costs from this mechanism alone. Macro expansion has a similar duplicating effect. Pipelines without this kind of duplication (e.g. a single C translation unit with no generics) do not exhibit this problem to nearly the same degree — the target here is specifically pipelines where duplicate backend work accumulates, not "large codebases" in general.

Frontend tooling solved an adjacent problem years ago using demand-driven query evaluation (e.g., rust-analyzer and the Salsa incremental computation framework), but compiler backends have lagged behind, and no existing backend deduplicates identical codegen work across separate compilation units at all.

### The DQIR Solution
In DQIR, every stage of the compilation pipeline—parsing, function extraction, register allocation, instruction selection, instruction scheduling, and machine code emission—is modeled as a memoized query in a dependency graph managed by Salsa 0.28. Queries are strictly scoped to individual function boundaries. Editing one function invalidates only the queries that depend on that specific function's intermediate representation; all other functions in the module remain cached and untouched.

This targets two related but distinct costs:
1. **Edit-to-recompile latency within a single build** — solved via Salsa's dependency-tracked query graph and early cutoff, described below.
2. **Duplicate codegen across separate compilation units** (e.g. the same monomorphized function recompiled in multiple crates) — targeted via content-addressed function identity: functions are hashed by their canonicalized IR content (not name or source location), so structurally identical functions across any two compilation units can, in principle, share one cached compiled result rather than being redundantly recompiled. This is the same underlying idea used by content-addressed languages like Unison, applied here one layer lower, at the post-lowering IR rather than at the source-language level.

### Explicit Tradeoff
DQIR does not claim to beat LLVM's generated code quality or Cranelift's cold-compilation speed. The tradeoff is stated up front: enforcing strict per-function query boundaries prevents cross-function and whole-program optimizations (such as interprocedural register allocation or arbitrary cross-function instruction combining). In exchange, DQIR achieves genuine O(1) incremental recompilation speeds for modified functions inside large modules, and — where duplicate monomorphized/macro-expanded code exists — the potential for cache hits across compilation units that no existing backend currently offers.

---

## Workspace Architecture

The project is structured as a Rust workspace with distinct crates for each layer of the compilation stack:

* `ir`: The SSA-based Intermediate Representation. Features arena-allocated, index-based graphs (`Block`, `Inst`, `Value`), type definitions, instruction definitions, a textual IR lexer and parser, and a canonical round-trip printer. Numeric literals (`f64`) are stored as raw IEEE-754 bit patterns (`u64`), not native floats, so that IR values remain `Hash`/`Eq`-safe for both Salsa query keys and content-addressed hashing.
* `backend`: The RISC-V 64-bit (RV64GC/IM) target backend. Includes liveness dataflow analysis, linear-scan register allocation over physical CPU registers, stack frame layout lowering, machine instruction selection, machine code encoding, and an in-memory JIT execution engine.
* `query`: The Salsa 0.28.1 incremental computation graph. Defines the tracked queries, enforces function isolation constraints, implements early cutoff comparisons, and provides query execution instrumentation logging.
* `cli`: A command-line interface driver for compiling textual IR files and inspecting compilation output.
* `bench`: Benchmarking harnesses and stress tests for measuring incremental compilation performance and register allocator throughput, including duplicate-function cache hit rate under synthetic monomorphization-like workloads.

---

## The DQIR Textual IR Language

DQIR defines a human-readable textual Intermediate Representation similar in role and structure to LLVM's `.ll` format. These textual IR files use the `.dqir` file extension. The `ir` crate provides a lossless canonical round-trip parser and printer, ensuring that `parse(text).to_string() == text`.

### Syntax Overview

* **Functions**: Declared with `func @name(%arg: type, ...) -> return_type { ... }`.
* **External Symbols**: Declared with `extern func @name(arg_type, ...) -> return_type`.
* **Basic Blocks**: Labeled destinations ending with a colon (e.g., `entry:`, `loop_block:`).
* **SSA Variables**: Values and instructions are assigned to temporary registers prefixed with `%` (e.g., `%sum`, `%1`).
* **Supported Types**: `i1` (booleans/conditions), `i32`, `i64` (64-bit integers), `f64` (double-precision floating point, stored internally as IEEE-754 bit patterns), `ptr` (memory pointers), and `void`.

### Comparison with LLVM IR

| Feature | LLVM `.ll` Format | DQIR Textual IR (`.dqir`) Format |
| :--- | :--- | :--- |
| **Function Definition** | `define i64 @add(i64 %x, i64 %y) { ... }` | `func @add(%x: i64, %y: i64) -> i64 { ... }` |
| **Constant Values** | `i64 42` (inline operand) | `%c: i64 = const.int 42` |
| **Arithmetic** | `%sum = add i64 %x, %y` | `%sum: i64 = add %x, %y` |
| **Comparisons** | `%cond = icmp eq i64 %x, %y` | `%cond: i1 = cmp eq %x, %y` |
| **Conditional Branch** | `br i1 %cond, label %true, label %false` | `br %cond, true_block, false_block` |
| **Unconditional Jump** | `br label %target` | `jmp target_block` |
| **SSA Phi Nodes** | `%res = phi i64 [ %x, %entry ], [ %y, %loop ]` | `%res: i64 = phi [ %x, entry ], [ %y, loop ]` |
| **Stack Allocation** | `%ptr = alloca i64, align 8` | `%ptr: ptr = alloca i64` |
| **Memory Load** | `%val = load i64, ptr %ptr, align 8` | `%val: i64 = load i64, %ptr, 0` |
| **Memory Store** | `store i64 %val, ptr %ptr, align 8` | `store %val, %ptr, 0` |
| **Function Call** | `%res = call i64 @foo(i64 %x)` | `%res: i64 = call @foo(%x)` |

### Textual IR Examples

#### 1. Arithmetic and Memory Operations (`memory.dqir`)
```dqir
func @memory_example(%val: i64) -> i64 {
entry:
    %ptr: ptr = alloca i64
    store %val, %ptr, 0
    %loaded: i64 = load i64, %ptr, 0
    %c10: i64 = const.int 10
    %res: i64 = add %loaded, %c10
    ret %res
}
```

#### 2. Iterative Control Flow and SSA Phi Nodes (`factorial.dqir`)
```dqir
func @factorial(%n: i64) -> i64 {
entry:
    %c0: i64 = const.int 0
    %c1: i64 = const.int 1
    %is_zero: i1 = cmp le %n, %c0
    br %is_zero, return_one, loop_init

return_one:
    ret %c1

loop_init:
    jmp loop_block

loop_block:
    %i: i64 = phi [ %c1, loop_init ], [ %next_i, loop_block ]
    %acc: i64 = phi [ %c1, loop_init ], [ %next_acc, loop_block ]
    %next_acc: i64 = mul %acc, %i
    %next_i: i64 = add %i, %c1
    %done: i1 = cmp gt %i, %n
    br %done, return_res, loop_block

return_res:
    ret %acc
}
```

#### 3. Complex Control Flow and Multi-Phi Nodes (`fibonacci.dqir`)
```dqir
func @fibonacci(%n: i64) -> i64 {
entry:
    %c0: i64 = const.int 0
    %c1: i64 = const.int 1
    %is_zero: i1 = cmp eq %n, %c0
    br %is_zero, return_block, loop_init

loop_init:
    jmp loop_block

loop_block:
    %i: i64 = phi [ %c1, loop_init ], [ %next_i, loop_block ]
    %a: i64 = phi [ %c0, loop_init ], [ %b, loop_block ]
    %b: i64 = phi [ %c1, loop_init ], [ %sum, loop_block ]
    %sum: i64 = add %a, %b
    %next_i: i64 = add %i, %c1
    %done: i1 = cmp ge %i, %n
    br %done, return_block, loop_block

return_block:
    %res: i64 = phi [ %c0, entry ], [ %sum, loop_block ]
    ret %res
}
```

#### 4. Floating-Point Constant Folding & IEEE-754 Bit Pattern (`floating_point.dqir`)
```dqir
func @float_bit_pattern(%x: f64) -> f64 {
entry:
    %c1: f64 = const.float 3.5
    %c2: f64 = const.float 2.5
    %folded_sum: f64 = add %c1, %c2
    ret %folded_sum
}
```

---

### Target Architecture: RISC-V 64-bit (RV64GC)

The backend emits native 64-bit RISC-V instructions (`RV64IM`/`RV64GC`). Key implementation details include:

1. **Register Allocation**:
   * Uses a linear-scan register allocator over physical CPU registers.
   * Allocates from 23 general-purpose registers: Caller-Saved (`t3`–`t6`, `a0`–`a7`) and Callee-Saved (`s1`–`s11`).
   * Registers `t0`, `t1`, and `t2` are reserved as unmanaged scratch registers for memory address generation and stack frame offset calculations.
   * Automatically assigns Callee-Saved registers to SSA values whose live intervals cross function call instructions (`Instruction::Call`).
   * When register pressure exceeds physical register capacity, live ranges with the furthest end points are spilled to stack memory slots.
2. **Stack Frame and ABI Lowering**:
   * Adheres to the standard RISC-V calling convention.
   * Dynamically calculates frame sizes to accommodate saved return addresses (`ra`), saved frame pointers (`fp`), callee-saved register backups, SSA spill slots, parameter backup areas, and local `alloca` memory buffers.
   * Supports passing arguments via registers (`a0`–`a7`) as well as memory-staged stack arguments for functions with more than 8 parameters.
3. **In-Memory JIT Execution Engine**:
   * Provides an executable memory wrapper (`JitMemory` and `Executable`) utilizing Unix POSIX `mmap` (`PROT_READ | PROT_WRITE | PROT_EXEC`).
   * Allows compiled RISC-V machine code bytes to be executed directly in memory on compatible hardware or emulated environments without requiring external linkers or temporary disk files.

x86-64 is a deliberately deferred, separately-scoped target (see project roadmap), chosen second rather than first because RISC-V's simpler, orthogonal, fixed-width encoding minimizes target-specific complexity while the core query-graph architecture is being proven out.

---

## The Incremental Query Graph

The `query` crate structures compilation into a directed acyclic graph of memoized Salsa queries:

```
[SourceInput]
     |
     v
parse_module (returns Arc<Module>)
     |
     v
   fn_ir (per-function extraction; returns Arc<Function>)
     |
     v
 optimize_ir (function-local constant folding, CSE, DCE; returns Arc<Function>)
     |
     +---------------------------+
     |                           |
     v                           v
allocate_registers          select_instructions
     |                           |
     +---------------------------+
     |
     v
  schedule (instruction scheduling boundary)
     |
     v
    emit (machine code byte emission; returns Arc<Vec<u8>>)
     |
     v
    link (module-level assembly; returns Arc<Vec<u8>>)
```

### Early Cutoff and Function Isolation

The architectural superpower of DQIR relies on two mechanisms enforcing incremental execution:

1. **Strict Query Boundaries**:
   Per-function queries (`optimize_ir`, `allocate_registers`, `select_instructions`, `schedule`, and `emit`) take only the database handle, the source input, and an interned `FunctionName`. Inside these queries, they are strictly prohibited from inspecting the IR or register allocations of any other function.
2. **Salsa Early Cutoff**:
   When a developer modifies a source file (e.g., editing function `@fn_25` inside a 50-function module), `link` re-invokes `parse_module`, producing a new AST. When downstream queries request `fn_ir` for function `@fn_0`, the query extracts the AST for `@fn_0` and compares it against the previously cached AST. Because `@fn_0` was not modified, its AST is structurally identical (`PartialEq` evaluates to true). Salsa triggers an Early Cutoff, marking the output of `fn_ir(db, input, "fn_0")` as unchanged (green). Furthermore, even if `@fn_0` was modified in a way that simplifies to the exact same optimized IR via `optimize_ir`, another Early Cutoff occurs at `optimize_ir`. Consequently, `allocate_registers`, `select_instructions`, `schedule`, and `emit` for untouched or equivalently optimized functions are bypassed entirely, resulting in 100% cache hits.

Note: Salsa's early cutoff, as described above, operates *within a single compilation database/session* — it is what makes edit-to-recompile latency fast. It is a distinct mechanism from the content-addressed cross-compilation-unit caching described in "The DQIR Solution" above, which targets duplicate work *across separate builds or compilation units* (e.g. the same monomorphized function appearing in two different crates). Both are part of the project's roadmap; the content-addressed layer is planned as an extension on top of the per-function IR hashing already required for Salsa query keys.

---

## Embedding DQIR: Simplified API Usage

In accordance with our core design tenet—*geniuses admire simplicity and idiots admire complexity*—DQIR shields embedding applications from compiler internals, Salsa database handles, interned keys, and manual AST manipulation. Whether you need a simple one-shot JIT compiler or a demand-driven incremental compilation session, the API is effortless.

### 1. One-Shot Backend Compilation (`backend::compile`)
For applications that simply want to execute textual IR without session management, `backend::compile` parses, automatically applies Phase 5 optimizations (`optimize_function`), allocates registers, emits RISC-V machine code, and wraps it in executable JIT memory:

```rust
use backend::compile;

let ir_text = r#"
func @add_ten(%val: i64) -> i64 {
entry:
    %c: i64 = const.int 10
    %res: i64 = add %val, %c
    ret %res
}
"#;

let exec = compile(ir_text, "add_ten").expect("compilation should succeed");
let result = exec.run(&[32]);
assert_eq!(result, 42);
```

### 2. Demand-Driven Incremental Compilation (`query::Session`)
For long-running JIT environments, language servers, or interactive tools, `query::Session` manages the incremental Salsa database and instrumentation logs behind an ergonomic facade:

```rust
use query::Session;

let mut session = Session::new(ir_text);

// Compile and execute a function by name
let exec = session.compile_fn("add_ten").expect("should compile cleanly");
assert_eq!(exec.run(&[32]), 42);

// Modify the source IR (e.g., editing a single function in a 50-function module)
session.update_source(new_ir_text);

// Re-compile: Salsa automatically triggers Early Cutoff on all untouched functions!
let exec_updated = session.compile_fn("add_ten").expect("incremental compilation should succeed");
```

---

## Building, Testing, and Verification

### Prerequisites
* Rust toolchain (Edition 2024 compatible, Rust 1.85+)
* Standard Linux/Unix build environment

### Running the Workspace Test Suite
To compile and execute all 44 unit and integration tests across all crates (IR parsing, round-trip verification, register allocator pressure stress tests, Salsa invalidation tests, and monomorphization deduplication benchmarks), run:

```bash
cargo test --workspace
```

### Running Specific Test Suites

* **IR Round-Trip and Parser Tests**:
  ```bash
  cargo test -p ir
  ```
* **Backend Codegen and Register Allocator Stress Tests**:
  ```bash
  cargo test -p backend
  ```
* **Salsa Query Granularity and JIT Execution Tests**:
  ```bash
  cargo test -p query --test invalidation_tests
  ```
* **Monomorphization Deduplication & Session Scaling Benchmarks**:
  ```bash
  cargo test -p bench
  ```

### Verification Highlights
The test suite includes dedicated verification tests confirming architectural claims:
* `test_regalloc_spilling_on_high_pressure`: Asserts correct stack spilling and callee-saved register utilization when 30 SSA variables are simultaneously live across 23 physical registers.
* `test_50_function_incremental_compilation_granularity`: Generates a 50-function module, performs an initial compilation, modifies only `@fn_25`, and asserts via instrumentation logs that exactly 4 codegen queries execute for `@fn_25` while 0 codegen queries execute for the other 49 functions.
* `test_salsa_jit_end_to_end_execution`: Proves end-to-end integration by compiling textual IR to machine code via Salsa, executing it in JIT memory, mutating the IR constants, re-compiling incrementally, and executing the updated JIT binary to verify the new return value.