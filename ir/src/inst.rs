use std::fmt;
use std::str::FromStr;
use crate::arena::{Block, Value};
use crate::types::Type;

/// Comparison operators for integer and floating-point comparison instructions.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Ult,
    Ule,
    Ugt,
    Uge,
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CmpOp::Eq => "eq",
            CmpOp::Ne => "ne",
            CmpOp::Lt => "lt",
            CmpOp::Le => "le",
            CmpOp::Gt => "gt",
            CmpOp::Ge => "ge",
            CmpOp::Ult => "ult",
            CmpOp::Ule => "ule",
            CmpOp::Ugt => "ugt",
            CmpOp::Uge => "uge",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for CmpOp {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eq" => Ok(CmpOp::Eq),
            "ne" => Ok(CmpOp::Ne),
            "lt" => Ok(CmpOp::Lt),
            "le" => Ok(CmpOp::Le),
            "gt" => Ok(CmpOp::Gt),
            "ge" => Ok(CmpOp::Ge),
            "ult" => Ok(CmpOp::Ult),
            "ule" => Ok(CmpOp::Ule),
            "ugt" => Ok(CmpOp::Ugt),
            "uge" => Ok(CmpOp::Uge),
            _ => Err(format!("Unknown comparison operator: '{}'", s)),
        }
    }
}

/// Immediate value representing integer or floating-point bit patterns.
/// The IR never holds a native Rust f32/f64 anywhere in its stored form, ensuring it is safely Hash/Eq-able.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Immediate {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    /// IEEE-754 single-precision bit pattern
    F32(u32),
    /// IEEE-754 double-precision bit pattern
    F64(u64),
}

impl Immediate {
    pub fn as_i64(&self) -> i64 {
        match *self {
            Immediate::I8(v) => v as i64,
            Immediate::I16(v) => v as i64,
            Immediate::I32(v) => v as i64,
            Immediate::I64(v) => v,
            Immediate::U8(v) => v as i64,
            Immediate::U16(v) => v as i64,
            Immediate::U32(v) => v as i64,
            Immediate::U64(v) => v as i64,
            Immediate::F32(v) => v as i64,
            Immediate::F64(v) => v as i64,
        }
    }
}

/// An SSA instruction in the DQIR intermediate representation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Instruction {
    // Constants
    /// Constant immediate value (integer or floating-point bit pattern).
    Const(Immediate),

    // Arithmetic & Bitwise
    Add(Value, Value),
    Sub(Value, Value),
    Mul(Value, Value),
    Div(Value, Value),
    Rem(Value, Value),
    Neg(Value),
    Not(Value),
    And(Value, Value),
    Or(Value, Value),
    Xor(Value, Value),
    Shl(Value, Value),
    Shr(Value, Value),
    Sar(Value, Value),

    // Comparison
    Cmp(CmpOp, Value, Value),

    // Memory
    Alloca(Type),
    Load(Type, Value, i32),
    Store(Value, Value, i32),

    // Control Flow
    Jmp(Block),
    Br(Value, Block, Block),
    Ret(Option<Value>),

    // Function Calls
    Call(String, Vec<Value>),

    // SSA Join / Phi
    Phi(Vec<(Block, Value)>),
}

impl Instruction {
    /// Returns true if this instruction terminates a basic block (jmp, br, ret).
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Instruction::Jmp(_) | Instruction::Br(_, _, _) | Instruction::Ret(_)
        )
    }

    /// Returns the short opcode name of the instruction.
    pub fn opcode_name(&self) -> &'static str {
        match self {
            Instruction::Const(Immediate::F32(_) | Immediate::F64(_)) => "const.float",
            Instruction::Const(_) => "const.int",
            Instruction::Add(_, _) => "add",
            Instruction::Sub(_, _) => "sub",
            Instruction::Mul(_, _) => "mul",
            Instruction::Div(_, _) => "div",
            Instruction::Rem(_, _) => "rem",
            Instruction::Neg(_) => "neg",
            Instruction::Not(_) => "not",
            Instruction::And(_, _) => "and",
            Instruction::Or(_, _) => "or",
            Instruction::Xor(_, _) => "xor",
            Instruction::Shl(_, _) => "shl",
            Instruction::Shr(_, _) => "shr",
            Instruction::Sar(_, _) => "sar",
            Instruction::Cmp(_, _, _) => "cmp",
            Instruction::Alloca(_) => "alloca",
            Instruction::Load(_, _, _) => "load",
            Instruction::Store(_, _, _) => "store",
            Instruction::Jmp(_) => "jmp",
            Instruction::Br(_, _, _) => "br",
            Instruction::Ret(_) => "ret",
            Instruction::Call(_, _) => "call",
            Instruction::Phi(_) => "phi",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmp_op() {
        assert_eq!(CmpOp::from_str("eq"), Ok(CmpOp::Eq));
        assert_eq!(CmpOp::from_str("ugt"), Ok(CmpOp::Ugt));
        assert!(CmpOp::from_str("foo").is_err());
    }

    #[test]
    fn test_terminator() {
        let jmp = Instruction::Jmp(Block(0));
        let add = Instruction::Add(Value(0), Value(1));
        assert!(jmp.is_terminator());
        assert!(!add.is_terminator());
    }

    #[test]
    fn test_immediate_and_instruction_hash() {
        use std::collections::HashSet;

        let imm1 = Immediate::F64(3.14159f64.to_bits());
        let imm2 = Immediate::F64(3.14159f64.to_bits());
        let imm3 = Immediate::F64((-0.0f64).to_bits());
        let imm4 = Immediate::F64(0.0f64.to_bits());

        let mut set = HashSet::new();
        set.insert(imm1);
        assert!(set.contains(&imm2));
        set.insert(imm3);
        set.insert(imm4);
        // -0.0 and +0.0 have different bit patterns in IEEE 754, so both should be preserved in set!
        assert_eq!(set.len(), 3);

        let inst1 = Instruction::Const(Immediate::F64(3.14159f64.to_bits()));
        let inst2 = Instruction::Const(Immediate::F64(3.14159f64.to_bits()));
        let mut inst_set = HashSet::new();
        inst_set.insert(inst1.clone());
        assert!(inst_set.contains(&inst2));
    }
}
