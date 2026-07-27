use std::collections::HashMap;
use ir::Block;
use crate::riscv::inst::{MInst, Reg};
use crate::riscv::select::LoweredFunction;

/// Emits RISC-V 64-bit (RV64GC/IM) little-endian machine code bytes from a lowered function.
pub fn emit_machine_code(lowered: &LoweredFunction) -> Vec<u8> {
    let expanded = expand_pseudos(&lowered.insts);

    // Pass 1: Compute instruction offsets and label locations.
    let mut label_offsets: HashMap<Block, i32> = HashMap::new();
    let mut current_pc = 0_i32;
    for inst in &expanded {
        match inst {
            MInst::Label(block) => {
                label_offsets.insert(*block, current_pc);
            }
            _ => {
                current_pc += 4;
            }
        }
    }

    // Pass 2: Encode 32-bit machine words and resolve branch/jump targets.
    let mut bytes = Vec::with_capacity(current_pc as usize);
    current_pc = 0;
    for inst in &expanded {
        if matches!(inst, MInst::Label(_)) {
            continue;
        }

        let word = match *inst {
            // R-type
            MInst::Add(rd, rs1, rs2)  => encode_r(0x33, rd, 0x0, rs1, rs2, 0x00),
            MInst::Sub(rd, rs1, rs2)  => encode_r(0x33, rd, 0x0, rs1, rs2, 0x20),
            MInst::Mul(rd, rs1, rs2)  => encode_r(0x33, rd, 0x0, rs1, rs2, 0x01),
            MInst::Div(rd, rs1, rs2)  => encode_r(0x33, rd, 0x4, rs1, rs2, 0x01),
            MInst::Divu(rd, rs1, rs2) => encode_r(0x33, rd, 0x5, rs1, rs2, 0x01),
            MInst::Rem(rd, rs1, rs2)  => encode_r(0x33, rd, 0x6, rs1, rs2, 0x01),
            MInst::Remu(rd, rs1, rs2) => encode_r(0x33, rd, 0x7, rs1, rs2, 0x01),
            MInst::Sll(rd, rs1, rs2)  => encode_r(0x33, rd, 0x1, rs1, rs2, 0x00),
            MInst::Srl(rd, rs1, rs2)  => encode_r(0x33, rd, 0x5, rs1, rs2, 0x00),
            MInst::Sra(rd, rs1, rs2)  => encode_r(0x33, rd, 0x5, rs1, rs2, 0x20),
            MInst::Slt(rd, rs1, rs2)  => encode_r(0x33, rd, 0x2, rs1, rs2, 0x00),
            MInst::Sltu(rd, rs1, rs2) => encode_r(0x33, rd, 0x3, rs1, rs2, 0x00),
            MInst::Xor(rd, rs1, rs2)  => encode_r(0x33, rd, 0x4, rs1, rs2, 0x00),
            MInst::Or(rd, rs1, rs2)   => encode_r(0x33, rd, 0x6, rs1, rs2, 0x00),
            MInst::And(rd, rs1, rs2)  => encode_r(0x33, rd, 0x7, rs1, rs2, 0x00),

            // R-type word variants
            MInst::Addw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x0, rs1, rs2, 0x00),
            MInst::Subw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x0, rs1, rs2, 0x20),
            MInst::Mulw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x0, rs1, rs2, 0x01),
            MInst::Divw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x4, rs1, rs2, 0x01),
            MInst::Divuw(rd, rs1, rs2)=> encode_r(0x3B, rd, 0x5, rs1, rs2, 0x01),
            MInst::Remw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x6, rs1, rs2, 0x01),
            MInst::Remuw(rd, rs1, rs2)=> encode_r(0x3B, rd, 0x7, rs1, rs2, 0x01),
            MInst::Sllw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x1, rs1, rs2, 0x00),
            MInst::Srlw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x5, rs1, rs2, 0x00),
            MInst::Sraw(rd, rs1, rs2) => encode_r(0x3B, rd, 0x5, rs1, rs2, 0x20),

            // I-type
            MInst::Addi(rd, rs1, imm)  => encode_i(0x13, rd, 0x0, rs1, imm),
            MInst::Addiw(rd, rs1, imm) => encode_i(0x1B, rd, 0x0, rs1, imm),
            MInst::Xori(rd, rs1, imm)  => encode_i(0x13, rd, 0x4, rs1, imm),
            MInst::Ori(rd, rs1, imm)   => encode_i(0x13, rd, 0x6, rs1, imm),
            MInst::Andi(rd, rs1, imm)  => encode_i(0x13, rd, 0x7, rs1, imm),
            MInst::Slti(rd, rs1, imm)  => encode_i(0x13, rd, 0x2, rs1, imm),
            MInst::Sltiu(rd, rs1, imm) => encode_i(0x13, rd, 0x3, rs1, imm),

            // I-type shifts
            MInst::Slli(rd, rs1, shamt)  => encode_i(0x13, rd, 0x1, rs1, (shamt & 0x3F) as i32),
            MInst::Srli(rd, rs1, shamt)  => encode_i(0x13, rd, 0x5, rs1, (shamt & 0x3F) as i32),
            MInst::Srai(rd, rs1, shamt)  => encode_i(0x13, rd, 0x5, rs1, (0x400 | (shamt & 0x3F)) as i32),
            MInst::Slliw(rd, rs1, shamt) => encode_i(0x1B, rd, 0x1, rs1, (shamt & 0x1F) as i32),
            MInst::Srliw(rd, rs1, shamt) => encode_i(0x1B, rd, 0x5, rs1, (shamt & 0x1F) as i32),
            MInst::Sraiw(rd, rs1, shamt) => encode_i(0x1B, rd, 0x5, rs1, (0x400 | (shamt & 0x1F)) as i32),

            // U-type
            MInst::Lui(rd, imm)   => encode_u(0x37, rd, imm),
            MInst::Auipc(rd, imm) => encode_u(0x17, rd, imm),

            // Memory Loads (I-type)
            MInst::Lb(rd, rs1, imm)  => encode_i(0x03, rd, 0x0, rs1, imm),
            MInst::Lh(rd, rs1, imm)  => encode_i(0x03, rd, 0x1, rs1, imm),
            MInst::Lw(rd, rs1, imm)  => encode_i(0x03, rd, 0x2, rs1, imm),
            MInst::Ld(rd, rs1, imm)  => encode_i(0x03, rd, 0x3, rs1, imm),
            MInst::Lbu(rd, rs1, imm) => encode_i(0x03, rd, 0x4, rs1, imm),
            MInst::Lhu(rd, rs1, imm) => encode_i(0x03, rd, 0x5, rs1, imm),
            MInst::Lwu(rd, rs1, imm) => encode_i(0x03, rd, 0x6, rs1, imm),

            // Memory Stores (S-type)
            MInst::Sb(rs1, rs2, imm) => encode_s(0x23, 0x0, rs1, rs2, imm),
            MInst::Sh(rs1, rs2, imm) => encode_s(0x23, 0x1, rs1, rs2, imm),
            MInst::Sw(rs1, rs2, imm) => encode_s(0x23, 0x2, rs1, rs2, imm),
            MInst::Sd(rs1, rs2, imm) => encode_s(0x23, 0x3, rs1, rs2, imm),

            // Branches (B-type)
            MInst::Beq(rs1, rs2, target)  => encode_b(0x63, 0x0, rs1, rs2, label_offset(&label_offsets, target, current_pc)),
            MInst::Bne(rs1, rs2, target)  => encode_b(0x63, 0x1, rs1, rs2, label_offset(&label_offsets, target, current_pc)),
            MInst::Blt(rs1, rs2, target)  => encode_b(0x63, 0x4, rs1, rs2, label_offset(&label_offsets, target, current_pc)),
            MInst::Bge(rs1, rs2, target)  => encode_b(0x63, 0x5, rs1, rs2, label_offset(&label_offsets, target, current_pc)),
            MInst::Bltu(rs1, rs2, target) => encode_b(0x63, 0x6, rs1, rs2, label_offset(&label_offsets, target, current_pc)),
            MInst::Bgeu(rs1, rs2, target) => encode_b(0x63, 0x7, rs1, rs2, label_offset(&label_offsets, target, current_pc)),

            // Jumps
            MInst::Jal(rd, target) => encode_j(0x6F, rd, label_offset(&label_offsets, target, current_pc)),
            MInst::Jalr(rd, rs1, imm) => encode_i(0x67, rd, 0x0, rs1, imm),

            _ => unreachable!("Pseudos should have been expanded in pass 1"),
        };

        bytes.extend_from_slice(&word.to_le_bytes());
        current_pc += 4;
    }

    bytes
}

fn label_offset(label_offsets: &HashMap<Block, i32>, target: Block, current_pc: i32) -> i32 {
    let target_pc = *label_offsets
        .get(&target)
        .unwrap_or_else(|| panic!("Target label {:?} not found in assembler pass 1", target));
    target_pc - current_pc
}

fn expand_pseudos(insts: &[MInst]) -> Vec<MInst> {
    let mut res = Vec::new();
    for inst in insts {
        match inst {
            MInst::Li64(rd, imm) => {
                res.extend(expand_li64(*rd, *imm));
            }
            MInst::Ret => {
                res.push(MInst::Jalr(Reg::ZERO, Reg::RA, 0));
            }
            MInst::Call(name) => {
                // Assign a dummy or virtual address for call symbol if needed during tests
                let mut hash = 0x1000_0000_u64;
                for b in name.bytes() {
                    hash = hash.wrapping_add(b as u64).wrapping_mul(31);
                }
                res.extend(expand_li64(Reg::T0, hash as i64));
                res.push(MInst::Jalr(Reg::RA, Reg::T0, 0));
            }
            _ => res.push(inst.clone()),
        }
    }
    res
}

fn expand_li64(rd: Reg, imm: i64) -> Vec<MInst> {
    if (-2048..=2047).contains(&imm) {
        return vec![MInst::Addi(rd, Reg::ZERO, imm as i32)];
    }
    if (-2_147_483_648..=2_147_483_647).contains(&imm) {
        let lo = ((imm & 0xFFF) ^ 0x800) - 0x800;
        let hi = (imm - lo) >> 12;
        if lo == 0 {
            return vec![MInst::Lui(rd, hi as i32)];
        } else {
            return vec![MInst::Lui(rd, hi as i32), MInst::Addiw(rd, rd, lo as i32)];
        }
    }

    // For > 32-bit constants, recursively load top bits then shift and add lowest 12 bits.
    let lo = ((imm & 0xFFF) ^ 0x800) - 0x800;
    let rest = (imm - lo) >> 12;
    let mut insts = expand_li64(rd, rest);
    insts.push(MInst::Slli(rd, rd, 12));
    if lo != 0 {
        insts.push(MInst::Addi(rd, rd, lo as i32));
    }
    insts
}

fn encode_r(opcode: u32, rd: Reg, funct3: u32, rs1: Reg, rs2: Reg, funct7: u32) -> u32 {
    let rd = rd.0 as u32;
    let rs1 = rs1.0 as u32;
    let rs2 = rs2.0 as u32;
    opcode | (rd << 7) | (funct3 << 12) | (rs1 << 15) | (rs2 << 20) | (funct7 << 25)
}

fn encode_i(opcode: u32, rd: Reg, funct3: u32, rs1: Reg, imm: i32) -> u32 {
    let rd = rd.0 as u32;
    let rs1 = rs1.0 as u32;
    let imm = (imm as u32) & 0xFFF;
    opcode | (rd << 7) | (funct3 << 12) | (rs1 << 15) | (imm << 20)
}

fn encode_s(opcode: u32, funct3: u32, rs1: Reg, rs2: Reg, imm: i32) -> u32 {
    let rs1 = rs1.0 as u32;
    let rs2 = rs2.0 as u32;
    let imm = imm as u32;
    let imm_0_4 = (imm & 0x1F) << 7;
    let imm_5_11 = ((imm >> 5) & 0x7F) << 25;
    opcode | imm_0_4 | (funct3 << 12) | (rs1 << 15) | (rs2 << 20) | imm_5_11
}

fn encode_u(opcode: u32, rd: Reg, imm: i32) -> u32 {
    let rd = rd.0 as u32;
    let imm20 = (imm as u32) & 0xFFFFF;
    opcode | (rd << 7) | (imm20 << 12)
}

fn encode_b(opcode: u32, funct3: u32, rs1: Reg, rs2: Reg, imm: i32) -> u32 {
    let rs1 = rs1.0 as u32;
    let rs2 = rs2.0 as u32;
    let imm = imm as u32;
    let imm12 = ((imm >> 12) & 1) << 31;
    let imm10_5 = ((imm >> 5) & 0x3F) << 25;
    let imm4_1 = ((imm >> 1) & 0xF) << 8;
    let imm11 = ((imm >> 11) & 1) << 7;
    opcode | imm11 | imm4_1 | (funct3 << 12) | (rs1 << 15) | (rs2 << 20) | imm10_5 | imm12
}

fn encode_j(opcode: u32, rd: Reg, imm: i32) -> u32 {
    let rd = rd.0 as u32;
    let imm = imm as u32;
    let imm20 = ((imm >> 20) & 1) << 31;
    let imm10_1 = ((imm >> 1) & 0x3FF) << 21;
    let imm11 = ((imm >> 11) & 1) << 20;
    let imm19_12 = ((imm >> 12) & 0xFF) << 12;
    opcode | (rd << 7) | imm19_12 | imm11 | imm10_1 | imm20
}
