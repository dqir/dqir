pub mod db;

use std::sync::Arc;
use ir::{Function, Module};
use backend::riscv::{
    emit_machine_code, lower_function_with_regalloc, schedule_instructions,
    allocate_registers as riscv_regalloc, LoweredFunction, RegAllocResult,
};
pub use db::{CompilerDatabase, Db, QueryLogEntry};

#[salsa::input]
pub struct SourceInput {
    pub text: String,
}

#[salsa::interned]
pub struct FunctionName<'db> {
    pub name: String,
}

#[salsa::tracked]
pub fn parse_module(db: &dyn Db, input: SourceInput) -> Arc<Module> {
    let text = input.text(db);
    Arc::new(ir::parse(&text).expect("Failed to parse IR module"))
}

#[salsa::tracked]
pub fn fn_ir(db: &dyn Db, input: SourceInput, name: FunctionName<'_>) -> Arc<Function> {
    db.log_query("fn_ir", &name.name(db));
    let module = parse_module(db, input);
    for (_, func) in module.functions.iter() {
        if func.name == *name.name(db) {
            return Arc::new(func.clone());
        }
    }
    panic!("Function not found: {}", name.name(db));
}

#[salsa::tracked]
pub fn optimize_ir(db: &dyn Db, input: SourceInput, name: FunctionName<'_>) -> Arc<Function> {
    db.log_query("optimize_ir", &name.name(db));
    let func = fn_ir(db, input, name);
    Arc::new(ir::optimize_function(func.as_ref()))
}

#[salsa::tracked]
pub fn allocate_registers(
    db: &dyn Db,
    input: SourceInput,
    name: FunctionName<'_>,
) -> Arc<RegAllocResult> {
    db.log_query("allocate_registers", &name.name(db));
    let func = optimize_ir(db, input, name);
    Arc::new(riscv_regalloc(func.as_ref()))
}

#[salsa::tracked]
pub fn select_instructions(
    db: &dyn Db,
    input: SourceInput,
    name: FunctionName<'_>,
) -> Arc<LoweredFunction> {
    db.log_query("select_instructions", &name.name(db));
    let func = optimize_ir(db, input, name);
    let regalloc = allocate_registers(db, input, name);
    Arc::new(lower_function_with_regalloc(func.as_ref(), regalloc.as_ref().clone()))
}

#[salsa::tracked]
pub fn schedule(
    db: &dyn Db,
    input: SourceInput,
    name: FunctionName<'_>,
) -> Arc<LoweredFunction> {
    db.log_query("schedule", &name.name(db));
    let lowered = select_instructions(db, input, name);
    Arc::new(schedule_instructions(lowered.as_ref()))
}

#[salsa::tracked]
pub fn emit(db: &dyn Db, input: SourceInput, name: FunctionName<'_>) -> Arc<Vec<u8>> {
    db.log_query("emit", &name.name(db));
    let scheduled = schedule(db, input, name);
    Arc::new(emit_machine_code(scheduled.as_ref()))
}

#[salsa::tracked]
pub fn link(db: &dyn Db, input: SourceInput) -> Arc<Vec<u8>> {
    db.log_query("link", "module");
    let module = parse_module(db, input);
    let mut all_bytes = Vec::new();
    for (_, func) in module.functions.iter() {
        let fn_name = FunctionName::new(db, func.name.clone());
        let fn_bytes = emit(db, input, fn_name);
        all_bytes.extend_from_slice(fn_bytes.as_ref());
    }
    Arc::new(all_bytes)
}

use salsa::Setter;

/// High-level session driver for incremental compilation.
/// Encapsulates the Salsa database, input source tracking, and query execution logs behind a clean facade.
pub struct Session {
    db: CompilerDatabase,
    input: SourceInput,
}

impl Session {
    /// Creates a new incremental compilation session from source IR text.
    pub fn new(source: &str) -> Self {
        let db = CompilerDatabase::default();
        let input = SourceInput::new(&db, source.to_string());
        Self { db, input }
    }

    /// Updates the source IR text in the session, invalidating only affected query nodes.
    pub fn update_source(&mut self, new_source: &str) {
        self.input.set_text(&mut self.db).to(new_source.to_string());
    }

    /// Incrementally compiles a specific function by name and wraps it in executable JIT memory.
    pub fn compile_fn(&self, fn_name: &str) -> Result<backend::Executable, String> {
        let name = FunctionName::new(&self.db, fn_name.to_string());
        let bytes = emit(&self.db, self.input, name);
        backend::Executable::new(fn_name, bytes.as_ref())
    }

    /// Incrementally compiles and links the entire module into a single machine code binary.
    pub fn compile_module(&self) -> Result<Vec<u8>, String> {
        let bytes = link(&self.db, self.input);
        Ok(bytes.as_ref().clone())
    }

    /// Retrieves all query execution logs recorded since initialization or the last clear.
    pub fn get_logs(&self) -> Vec<QueryLogEntry> {
        self.db.get_logs()
    }

    /// Clears the query execution instrumentation logs.
    pub fn clear_logs(&self) {
        self.db.clear_logs();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_incremental_compilation() {
        let mut session = Session::new("func @foo() -> i64 {\nentry:\n    %c: i64 = const.int 10\n    ret %c\n}\n");
        let exec = session.compile_fn("foo").expect("compile_fn should succeed");
        assert_eq!(exec.run(&[]), 10);
        assert_eq!(session.get_logs().len(), 6);

        session.clear_logs();
        let _ = session.compile_fn("foo").expect("compile_fn should succeed");
        assert!(session.get_logs().is_empty());

        session.update_source("func @foo() -> i64 {\nentry:\n    %c: i64 = const.int 20\n    ret %c\n}\n");
        let exec2 = session.compile_fn("foo").expect("compile_fn should succeed");
        assert_eq!(exec2.run(&[]), 20);
        assert!(!session.get_logs().is_empty());
    }
}

