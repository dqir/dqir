# dqir
DQIR (Demand-driven Query Intermediate Representation) Project

A compiler backend (IR-in, machine-code-out) built around one idea neither LLVM nor Cranelift currently address: treating codegen itself as an incremental, cached, dependency-tracked computation rather than a batch pipeline.

The problem it targets: Compiler backends optimize for either fast cold compiles (Cranelift) or high-quality generated code (LLVM) - but neither is architected to be incremental. Change one function in a large module, and both backends redo far more work than necessary, because nothing tracks what actually depends on what at the codegen level. Frontend tooling solved this years ago (Salsa, rust-analyzer's incremental analysis) - backends haven't.

What it is: Instruction selection, register allocation, scheduling, and emission are each modeled as memoized queries in a dependency graph (via Salsa), keyed per-function. Editing one function invalidates only the queries that depend on it - everything else stays cached.

What it explicitly does not claim: to beat LLVM's generated-code quality, or beat Cranelift's cold-compile speed. The tradeoff is stated up front - per-function query boundaries mean no cross-function/whole-program optimization, in exchange for genuine incremental recompile speed neither competitor offers.

Stack: Rust, arena/index-based IR (not pointer-graph), x86-64 target for v0, Salsa as the incremental engine, a minimal toy language used only as a test harness -never intended to become a real production language.
