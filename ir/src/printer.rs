use std::collections::HashMap;
use std::fmt;
use crate::arena::{Block, EntityId, Inst, Value};
use crate::function::{Function, ValueDef};
use crate::inst::{Immediate, Instruction};
use crate::module::{ExternalDecl, Module};

/// Format an SSA Value ID as a text representation (e.g. `%foo` or `%0`).
pub fn format_val(func: &Function, val: Value) -> String {
    if let Some(ref name) = func.values[val].name {
        format!("%{}", name)
    } else {
        format!("%{}", val.as_u32())
    }
}

/// Format a Basic Block ID as its name.
pub fn format_block(func: &Function, block: Block) -> String {
    func.blocks[block].name.clone()
}

/// Format a single instruction into its string representation without result value assignment.
pub fn format_inst(func: &Function, inst: &Instruction) -> String {
    match inst {
        Instruction::Const(Immediate::F32(val)) => {
            let mut s = format!("{}", f32::from_bits(*val));
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                s.push_str(".0");
            }
            format!("const.float {}", s)
        }
        Instruction::Const(Immediate::F64(val)) => {
            let mut s = format!("{}", f64::from_bits(*val));
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                s.push_str(".0");
            }
            format!("const.float {}", s)
        }
        Instruction::Const(imm) => format!("const.int {}", imm.as_i64()),
        Instruction::Add(a, b) => format!("add {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Sub(a, b) => format!("sub {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Mul(a, b) => format!("mul {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Div(a, b) => format!("div {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Rem(a, b) => format!("rem {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Neg(a) => format!("neg {}", format_val(func, *a)),
        Instruction::Not(a) => format!("not {}", format_val(func, *a)),
        Instruction::And(a, b) => format!("and {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Or(a, b) => format!("or {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Xor(a, b) => format!("xor {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Shl(a, b) => format!("shl {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Shr(a, b) => format!("shr {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Sar(a, b) => format!("sar {}, {}", format_val(func, *a), format_val(func, *b)),
        Instruction::Cmp(op, a, b) => {
            format!("cmp {} {}, {}", op, format_val(func, *a), format_val(func, *b))
        }
        Instruction::Alloca(ty) => format!("alloca {}", ty),
        Instruction::Load(ty, ptr, offset) => {
            format!("load {}, {}, {}", ty, format_val(func, *ptr), offset)
        }
        Instruction::Store(val, ptr, offset) => {
            format!(
                "store {}, {}, {}",
                format_val(func, *val),
                format_val(func, *ptr),
                offset
            )
        }
        Instruction::Jmp(target) => format!("jmp {}", format_block(func, *target)),
        Instruction::Br(cond, t_block, f_block) => {
            format!(
                "br {}, {}, {}",
                format_val(func, *cond),
                format_block(func, *t_block),
                format_block(func, *f_block)
            )
        }
        Instruction::Ret(opt_val) => {
            if let Some(v) = opt_val {
                format!("ret {}", format_val(func, *v))
            } else {
                "ret".to_string()
            }
        }
        Instruction::Call(target, args) => {
            let args_str = args
                .iter()
                .map(|arg| format_val(func, *arg))
                .collect::<Vec<_>>()
                .join(", ");
            format!("call @{}({})", target, args_str)
        }
        Instruction::Phi(incoming) => {
            let pairs = incoming
                .iter()
                .map(|(blk, val)| format!("[ {}, {} ]", format_val(func, *val), format_block(func, *blk)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("phi {}", pairs)
        }
    }
}

impl fmt::Display for ExternalDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params_str = self
            .params
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "extern func @{}({}) -> {}",
            self.name, params_str, self.return_type
        )
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params_str = self
            .params
            .iter()
            .map(|(_, val, ty)| format!("{}: {}", format_val(self, *val), ty))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(f, "func @{}({}) -> {} {{", self.name, params_str, self.return_type)?;

        // Map instructions to their result values if they exist.
        let mut inst_to_val: HashMap<Inst, Value> = HashMap::new();
        for (val_id, val_data) in self.values.iter() {
            if let ValueDef::Inst(inst_id) = val_data.def {
                inst_to_val.insert(inst_id, val_id);
            }
        }

        for (i, (block_id, block_data)) in self.blocks.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            writeln!(f, "{}:", block_data.name)?;
            for &inst_id in &block_data.insts {
                let inst = &self.insts[inst_id];
                let inst_str = format_inst(self, inst);
                if let Some(&val_id) = inst_to_val.get(&inst_id) {
                    let ty = self.values[val_id].ty;
                    writeln!(f, "    {}: {} = {}", format_val(self, val_id), ty, inst_str)?;
                } else {
                    writeln!(f, "    {}", inst_str)?;
                }
            }
            // avoid warning if block_id is unused when debugging
            let _ = block_id;
        }

        write!(f, "}}")
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, ext) in self.externs.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", ext)?;
        }

        if !self.externs.is_empty() && !self.functions.is_empty() {
            writeln!(f)?;
            writeln!(f)?;
        }

        for (i, (_, func)) in self.functions.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", func)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst::Instruction;
    use crate::types::Type;

    #[test]
    fn test_print_function() {
        let mut func = Function::new("add_two", Type::I32);
        let p0 = func.add_param("a", Type::I32);
        let p1 = func.add_param("b", Type::I32);
        let b0 = func.create_block("entry");
        let (_, res) = func.push_inst(b0, Instruction::Add(p0, p1), Some(Type::I32));
        let res = res.unwrap();
        func.set_value_name(res, "sum");
        let _ = func.push_inst(b0, Instruction::Ret(Some(res)), None);

        let printed = func.to_string();
        let expected = "func @add_two(%a: i32, %b: i32) -> i32 {\nentry:\n    %sum: i32 = add %a, %b\n    ret %sum\n}";
        assert_eq!(printed, expected);
    }
}
