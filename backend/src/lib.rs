pub mod loader;
pub mod riscv;
pub mod vm;

pub use loader::{Executable, JitMemory};
pub use riscv::{
    emit_machine_code, lower_function, lower_function_with_regalloc, schedule_instructions,
    LoweredFunction, MInst, Reg,
};
pub use vm::RiscvVm;

/// Compiles an IR function to an executable RISC-V 64-bit machine code wrapper.
/// Automatically applies function-local optimizations before lowering.
pub fn compile_function(func: &ir::Function) -> Result<Executable, String> {
    let optimized = ir::optimize_function(func);
    let lowered = lower_function(&optimized);
    let bytes = emit_machine_code(&lowered);
    Executable::new(&optimized.name, &bytes)
}

/// Compiles a textual IR string targeting a specific function by name into an executable RISC-V 64-bit JIT wrapper.
pub fn compile(ir_text: &str, fn_name: &str) -> Result<Executable, String> {
    let module = ir::parse(ir_text).map_err(|e| format!("Parse error: {}", e))?;
    let (_, func) = module
        .functions
        .iter()
        .find(|(_, f)| f.name == fn_name)
        .ok_or_else(|| format!("Function '@{}' not found in IR module", fn_name))?;
    compile_function(func)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::Type;

    #[test]
    fn test_empty_function() {
        let mut func = ir::Function::new("test", Type::Void);
        let b = func.create_block("entry");
        func.push_inst(b, ir::Instruction::Ret(None), None);
        let exec = compile_function(&func).expect("compilation should succeed");
        assert!(!exec.jit_mem.as_slice().is_empty());
    }

    #[test]
    fn test_compile_textual_ir() {
        let ir = "func @add_ten(%x: i64) -> i64 {\nentry:\n    %c: i64 = const.int 10\n    %res: i64 = add %x, %c\n    ret %res\n}\n";
        let exec = compile(ir, "add_ten").expect("should compile cleanly");
        let res = exec.run(&[32]);
        assert_eq!(res, 42);
    }
}

