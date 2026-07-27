use std::fmt;
use ir::Block;

/// A RISC-V 64-bit general purpose register.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Reg(pub u8);

impl Reg {
    pub const ZERO: Reg = Reg(0);  // x0: Hardwired zero
    pub const RA: Reg   = Reg(1);  // x1: Return address
    pub const SP: Reg   = Reg(2);  // x2: Stack pointer
    pub const GP: Reg   = Reg(3);  // x3: Global pointer
    pub const TP: Reg   = Reg(4);  // x4: Thread pointer
    pub const T0: Reg   = Reg(5);  // x5: Temporary / alternate link register
    pub const T1: Reg   = Reg(6);  // x6: Temporary
    pub const T2: Reg   = Reg(7);  // x7: Temporary
    pub const FP: Reg   = Reg(8);  // x8: Saved register / Frame pointer (s0)
    pub const S1: Reg   = Reg(9);  // x9: Saved register
    pub const A0: Reg   = Reg(10); // x10: Function argument 0 / return value 0
    pub const A1: Reg   = Reg(11); // x11: Function argument 1 / return value 1
    pub const A2: Reg   = Reg(12); // x12: Function argument 2
    pub const A3: Reg   = Reg(13); // x13: Function argument 3
    pub const A4: Reg   = Reg(14); // x14: Function argument 4
    pub const A5: Reg   = Reg(15); // x15: Function argument 5
    pub const A6: Reg   = Reg(16); // x16: Function argument 6
    pub const A7: Reg   = Reg(17); // x17: Function argument 7
    pub const S2: Reg   = Reg(18); // x18: Saved register
    pub const S3: Reg   = Reg(19); // x19: Saved register
    pub const S4: Reg   = Reg(20); // x20: Saved register
    pub const S5: Reg   = Reg(21); // x21: Saved register
    pub const S6: Reg   = Reg(22); // x22: Saved register
    pub const S7: Reg   = Reg(23); // x23: Saved register
    pub const S8: Reg   = Reg(24); // x24: Saved register
    pub const S9: Reg   = Reg(25); // x25: Saved register
    pub const S10: Reg  = Reg(26); // x26: Saved register
    pub const S11: Reg  = Reg(27); // x27: Saved register
    pub const T3: Reg   = Reg(28); // x28: Temporary
    pub const T4: Reg   = Reg(29); // x29: Temporary
    pub const T5: Reg   = Reg(30); // x30: Temporary
    pub const T6: Reg   = Reg(31); // x31: Temporary

    /// Returns the standard ABI name of the register.
    pub fn abi_name(&self) -> &'static str {
        match self.0 {
            0 => "zero",
            1 => "ra",
            2 => "sp",
            3 => "gp",
            4 => "tp",
            5 => "t0",
            6 => "t1",
            7 => "t2",
            8 => "fp",
            9 => "s1",
            10 => "a0",
            11 => "a1",
            12 => "a2",
            13 => "a3",
            14 => "a4",
            15 => "a5",
            16 => "a6",
            17 => "a7",
            18 => "s2",
            19 => "s3",
            20 => "s4",
            21 => "s5",
            22 => "s6",
            23 => "s7",
            24 => "s8",
            25 => "s9",
            26 => "s10",
            27 => "s11",
            28 => "t3",
            29 => "t4",
            30 => "t5",
            31 => "t6",
            _ => "unknown",
        }
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.abi_name())
    }
}

/// A machine instruction in the RISC-V 64-bit (RV64GC/IM) target architecture.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MInst {
    // Basic Block Label (pseudo instruction used for branch targets)
    Label(Block),

    // ALU R-Type (64-bit)
    Add(Reg, Reg, Reg),
    Sub(Reg, Reg, Reg),
    Mul(Reg, Reg, Reg),
    Div(Reg, Reg, Reg),
    Divu(Reg, Reg, Reg),
    Rem(Reg, Reg, Reg),
    Remu(Reg, Reg, Reg),
    And(Reg, Reg, Reg),
    Or(Reg, Reg, Reg),
    Xor(Reg, Reg, Reg),
    Sll(Reg, Reg, Reg),
    Srl(Reg, Reg, Reg),
    Sra(Reg, Reg, Reg),
    Slt(Reg, Reg, Reg),
    Sltu(Reg, Reg, Reg),

    // ALU R-Type (32-bit Word variants)
    Addw(Reg, Reg, Reg),
    Subw(Reg, Reg, Reg),
    Mulw(Reg, Reg, Reg),
    Divw(Reg, Reg, Reg),
    Divuw(Reg, Reg, Reg),
    Remw(Reg, Reg, Reg),
    Remuw(Reg, Reg, Reg),
    Sllw(Reg, Reg, Reg),
    Srlw(Reg, Reg, Reg),
    Sraw(Reg, Reg, Reg),

    // ALU I-Type
    Addi(Reg, Reg, i32),
    Addiw(Reg, Reg, i32),
    Xori(Reg, Reg, i32),
    Ori(Reg, Reg, i32),
    Andi(Reg, Reg, i32),
    Slli(Reg, Reg, u32),
    Srli(Reg, Reg, u32),
    Srai(Reg, Reg, u32),
    Slliw(Reg, Reg, u32),
    Srliw(Reg, Reg, u32),
    Sraiw(Reg, Reg, u32),
    Slti(Reg, Reg, i32),
    Sltiu(Reg, Reg, i32),

    // Upper Immediates
    Lui(Reg, i32),
    Auipc(Reg, i32),

    // Pseudo Immediate Loader (loads arbitrary 64-bit integer into reg)
    Li64(Reg, i64),

    // Memory Loads
    Ld(Reg, Reg, i32),
    Lw(Reg, Reg, i32),
    Lwu(Reg, Reg, i32),
    Lh(Reg, Reg, i32),
    Lhu(Reg, Reg, i32),
    Lb(Reg, Reg, i32),
    Lbu(Reg, Reg, i32),

    // Memory Stores
    Sd(Reg, Reg, i32),
    Sw(Reg, Reg, i32),
    Sh(Reg, Reg, i32),
    Sb(Reg, Reg, i32),

    // Conditional Branches
    Beq(Reg, Reg, Block),
    Bne(Reg, Reg, Block),
    Blt(Reg, Reg, Block),
    Bge(Reg, Reg, Block),
    Bltu(Reg, Reg, Block),
    Bgeu(Reg, Reg, Block),

    // Jumps and Calls
    Jal(Reg, Block),
    Jalr(Reg, Reg, i32),
    Call(String),
    Ret,
}

impl MInst {
    /// Convenience constructor for register move: `mv rd, rs` -> `addi rd, rs, 0`.
    pub fn mv(rd: Reg, rs: Reg) -> Self {
        MInst::Addi(rd, rs, 0)
    }

    /// Convenience constructor for nop: `addi zero, zero, 0`.
    pub fn nop() -> Self {
        MInst::Addi(Reg::ZERO, Reg::ZERO, 0)
    }
}
