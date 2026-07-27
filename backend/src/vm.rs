/// An embedded 64-bit RISC-V (RV64GC/IM) virtual machine for executing generated machine code bytes.
pub struct RiscvVm {
    pub regs: [u64; 32],
    pub pc: usize,
    pub memory: Vec<u8>,
    pub exit_pc: usize,
}

impl RiscvVm {
    pub fn new(code: &[u8]) -> Self {
        let mem_size = 4 * 1024 * 1024; // 4 MB initial memory
        let mut memory = vec![0_u8; mem_size];
        memory[..code.len()].copy_from_slice(code);

        let mut regs = [0_u64; 32];
        regs[2] = mem_size as u64 - 16; // sp points to top of memory
        let exit_pc = 0xFFFF_0000;
        regs[1] = exit_pc as u64; // ra is set to magic exit PC

        Self {
            regs,
            pc: 0,
            memory,
            exit_pc,
        }
    }

    pub fn run(&mut self, args: &[u64]) -> u64 {
        for (i, &arg) in args.iter().enumerate() {
            if i < 8 {
                self.regs[10 + i] = arg; // a0..a7
            } else {
                let offset = 8 * (i - 8);
                let sp = self.regs[2] as usize;
                self.write_u64(sp + offset, arg);
            }
        }

        while self.pc != self.exit_pc {
            if self.pc + 4 > self.memory.len() {
                panic!("VM PC out of bounds: 0x{:x}", self.pc);
            }
            let word = u32::from_le_bytes([
                self.memory[self.pc],
                self.memory[self.pc + 1],
                self.memory[self.pc + 2],
                self.memory[self.pc + 3],
            ]);

            let opcode = word & 0x7F;
            let rd = ((word >> 7) & 0x1F) as usize;
            let funct3 = ((word >> 12) & 0x7) as u32;
            let rs1 = ((word >> 15) & 0x1F) as usize;
            let rs2 = ((word >> 20) & 0x1F) as usize;
            let funct7 = ((word >> 25) & 0x7F) as u32;

            let mut next_pc = self.pc + 4;

            match opcode {
                0x33 => self.exec_op_33(rd, funct3, rs1, rs2, funct7),
                0x3B => self.exec_op_3b(rd, funct3, rs1, rs2, funct7),
                0x13 => self.exec_op_13(rd, funct3, rs1, word),
                0x1B => self.exec_op_1b(rd, funct3, rs1, word),
                0x37 => {
                    let imm20 = ((word & 0xFFFFF000) as i32) as i64;
                    self.regs[rd] = imm20 as u64;
                }
                0x17 => {
                    let imm20 = ((word & 0xFFFFF000) as i32) as i64;
                    self.regs[rd] = (self.pc as i64).wrapping_add(imm20) as u64;
                }
                0x03 => self.exec_op_03(rd, funct3, rs1, word),
                0x23 => self.exec_op_23(funct3, rs1, rs2, word),
                0x63 => {
                    let imm12 = ((word >> 31) & 1) << 12;
                    let imm10_5 = ((word >> 25) & 0x3F) << 5;
                    let imm4_1 = ((word >> 8) & 0xF) << 1;
                    let imm11 = ((word >> 7) & 1) << 11;
                    let imm = (imm12 | imm11 | imm10_5 | imm4_1) as i32;
                    let imm = ((imm << 19) >> 19) as i64; // Sign-extend 13-bit offset
                    let v1 = self.regs[rs1];
                    let v2 = self.regs[rs2];
                    let cond = match funct3 {
                        0x0 => v1 == v2,
                        0x1 => v1 != v2,
                        0x4 => (v1 as i64) < (v2 as i64),
                        0x5 => (v1 as i64) >= (v2 as i64),
                        0x6 => v1 < v2,
                        0x7 => v1 >= v2,
                        _ => false,
                    };
                    if cond {
                        next_pc = (self.pc as i64).wrapping_add(imm) as usize;
                    }
                }
                0x6F => {
                    let imm20 = ((word >> 31) & 1) << 20;
                    let imm10_1 = ((word >> 21) & 0x3FF) << 1;
                    let imm11 = ((word >> 20) & 1) << 11;
                    let imm19_12 = ((word >> 12) & 0xFF) << 12;
                    let imm = (imm20 | imm19_12 | imm11 | imm10_1) as i32;
                    let imm = ((imm << 11) >> 11) as i64; // Sign-extend 21-bit offset
                    if rd != 0 {
                        self.regs[rd] = (self.pc + 4) as u64;
                    }
                    next_pc = (self.pc as i64).wrapping_add(imm) as usize;
                }
                0x67 => {
                    let imm = ((word as i32) >> 20) as i64;
                    let target = self.regs[rs1].wrapping_add(imm as u64) & !1;
                    if rd != 0 {
                        self.regs[rd] = (self.pc + 4) as u64;
                    }
                    next_pc = target as usize;
                }
                _ => panic!("Unknown instruction opcode 0x{:02x} at PC 0x{:x}", opcode, self.pc),
            }

            self.pc = next_pc;
            self.regs[0] = 0; // x0 is hardwired zero
        }

        self.regs[10] // return a0
    }

    fn ensure_mem(&mut self, addr: usize, size: usize) {
        if addr + size > self.memory.len() {
            let new_len = (addr + size).max(self.memory.len() * 2);
            self.memory.resize(new_len, 0);
        }
    }

    fn read_u8(&mut self, addr: usize) -> u8 {
        self.ensure_mem(addr, 1);
        self.memory[addr]
    }
    fn read_u16(&mut self, addr: usize) -> u16 {
        self.ensure_mem(addr, 2);
        u16::from_le_bytes([self.memory[addr], self.memory[addr + 1]])
    }
    fn read_u32(&mut self, addr: usize) -> u32 {
        self.ensure_mem(addr, 4);
        u32::from_le_bytes([
            self.memory[addr],
            self.memory[addr + 1],
            self.memory[addr + 2],
            self.memory[addr + 3],
        ])
    }
    fn read_u64(&mut self, addr: usize) -> u64 {
        self.ensure_mem(addr, 8);
        u64::from_le_bytes([
            self.memory[addr],
            self.memory[addr + 1],
            self.memory[addr + 2],
            self.memory[addr + 3],
            self.memory[addr + 4],
            self.memory[addr + 5],
            self.memory[addr + 6],
            self.memory[addr + 7],
        ])
    }
    fn write_u8(&mut self, addr: usize, val: u8) {
        self.ensure_mem(addr, 1);
        self.memory[addr] = val;
    }
    fn write_u16(&mut self, addr: usize, val: u16) {
        self.ensure_mem(addr, 2);
        self.memory[addr..addr + 2].copy_from_slice(&val.to_le_bytes());
    }
    fn write_u32(&mut self, addr: usize, val: u32) {
        self.ensure_mem(addr, 4);
        self.memory[addr..addr + 4].copy_from_slice(&val.to_le_bytes());
    }
    fn write_u64(&mut self, addr: usize, val: u64) {
        self.ensure_mem(addr, 8);
        self.memory[addr..addr + 8].copy_from_slice(&val.to_le_bytes());
    }

    fn exec_op_33(&mut self, rd: usize, funct3: u32, rs1: usize, rs2: usize, funct7: u32) {
        let v1 = self.regs[rs1];
        let v2 = self.regs[rs2];
        let res = match (funct3, funct7) {
            (0x0, 0x00) => v1.wrapping_add(v2),
            (0x0, 0x20) => v1.wrapping_sub(v2),
            (0x0, 0x01) => v1.wrapping_mul(v2),
            (0x4, 0x01) => {
                if v2 == 0 {
                    u64::MAX // -1
                } else if v1 == 0x8000_0000_0000_0000 && v2 == u64::MAX {
                    0x8000_0000_0000_0000
                } else {
                    ((v1 as i64) / (v2 as i64)) as u64
                }
            }
            (0x5, 0x01) => {
                if v2 == 0 {
                    u64::MAX
                } else {
                    v1 / v2
                }
            }
            (0x6, 0x01) => {
                if v2 == 0 {
                    v1
                } else if v1 == 0x8000_0000_0000_0000 && v2 == u64::MAX {
                    0
                } else {
                    ((v1 as i64) % (v2 as i64)) as u64
                }
            }
            (0x7, 0x01) => {
                if v2 == 0 {
                    v1
                } else {
                    v1 % v2
                }
            }
            (0x1, 0x00) => v1 << (v2 & 0x3F),
            (0x5, 0x00) => v1 >> (v2 & 0x3F),
            (0x5, 0x20) => ((v1 as i64) >> (v2 & 0x3F)) as u64,
            (0x2, 0x00) => if (v1 as i64) < (v2 as i64) { 1 } else { 0 },
            (0x3, 0x00) => if v1 < v2 { 1 } else { 0 },
            (0x4, 0x00) => v1 ^ v2,
            (0x6, 0x00) => v1 | v2,
            (0x7, 0x00) => v1 & v2,
            _ => panic!("Unknown R-type 64-bit opcode 0x33 funct3=0x{:x} funct7=0x{:x}", funct3, funct7),
        };
        self.regs[rd] = res;
    }

    fn exec_op_3b(&mut self, rd: usize, funct3: u32, rs1: usize, rs2: usize, funct7: u32) {
        let v1 = self.regs[rs1] as i32;
        let v2 = self.regs[rs2] as i32;
        let res = match (funct3, funct7) {
            (0x0, 0x00) => v1.wrapping_add(v2) as i64 as u64,
            (0x0, 0x20) => v1.wrapping_sub(v2) as i64 as u64,
            (0x0, 0x01) => v1.wrapping_mul(v2) as i64 as u64,
            (0x4, 0x01) => {
                if v2 == 0 {
                    u64::MAX // -1
                } else if v1 == i32::MIN && v2 == -1 {
                    i32::MIN as i64 as u64
                } else {
                    (v1 / v2) as i64 as u64
                }
            }
            (0x5, 0x01) => {
                let u1 = v1 as u32;
                let u2 = v2 as u32;
                if u2 == 0 {
                    u64::MAX
                } else {
                    (u1 / u2) as i32 as i64 as u64
                }
            }
            (0x6, 0x01) => {
                if v2 == 0 {
                    v1 as i64 as u64
                } else if v1 == i32::MIN && v2 == -1 {
                    0
                } else {
                    (v1 % v2) as i64 as u64
                }
            }
            (0x7, 0x01) => {
                let u1 = v1 as u32;
                let u2 = v2 as u32;
                if u2 == 0 {
                    u1 as i32 as i64 as u64
                } else {
                    (u1 % u2) as i32 as i64 as u64
                }
            }
            (0x1, 0x00) => (v1 << (v2 & 0x1F)) as i64 as u64,
            (0x5, 0x00) => ((v1 as u32) >> (v2 & 0x1F)) as i32 as i64 as u64,
            (0x5, 0x20) => (v1 >> (v2 & 0x1F)) as i64 as u64,
            _ => panic!("Unknown R-type 32-bit opcode 0x3B funct3=0x{:x} funct7=0x{:x}", funct3, funct7),
        };
        self.regs[rd] = res;
    }

    fn exec_op_13(&mut self, rd: usize, funct3: u32, rs1: usize, word: u32) {
        let imm = ((word as i32) >> 20) as i64;
        let v1 = self.regs[rs1];
        let res = match funct3 {
            0x0 => v1.wrapping_add(imm as u64),
            0x2 => if (v1 as i64) < imm { 1 } else { 0 },
            0x3 => if v1 < (imm as u64) { 1 } else { 0 },
            0x4 => v1 ^ (imm as u64),
            0x6 => v1 | (imm as u64),
            0x7 => v1 & (imm as u64),
            0x1 => v1 << ((word >> 20) & 0x3F),
            0x5 => {
                let shamt = (word >> 20) & 0x3F;
                if (word >> 26) & 0x3F == 0x10 {
                    ((v1 as i64) >> shamt) as u64
                } else {
                    v1 >> shamt
                }
            }
            _ => panic!("Unknown I-type opcode 0x13 funct3=0x{:x}", funct3),
        };
        self.regs[rd] = res;
    }

    fn exec_op_1b(&mut self, rd: usize, funct3: u32, rs1: usize, word: u32) {
        let imm = ((word as i32) >> 20) as i32;
        let v1 = self.regs[rs1] as i32;
        let res = match funct3 {
            0x0 => v1.wrapping_add(imm) as i64 as u64,
            0x1 => (v1 << ((word >> 20) & 0x1F)) as i64 as u64,
            0x5 => {
                let shamt = (word >> 20) & 0x1F;
                if (word >> 30) & 1 == 1 {
                    (v1 >> shamt) as i64 as u64
                } else {
                    ((v1 as u32) >> shamt) as i32 as i64 as u64
                }
            }
            _ => panic!("Unknown I-type word opcode 0x1B funct3=0x{:x}", funct3),
        };
        self.regs[rd] = res;
    }

    fn exec_op_03(&mut self, rd: usize, funct3: u32, rs1: usize, word: u32) {
        let imm = ((word as i32) >> 20) as i64;
        let addr = self.regs[rs1].wrapping_add(imm as u64) as usize;
        let res = match funct3 {
            0x0 => (self.read_u8(addr) as i8) as i64 as u64,
            0x1 => (self.read_u16(addr) as i16) as i64 as u64,
            0x2 => (self.read_u32(addr) as i32) as i64 as u64,
            0x3 => self.read_u64(addr),
            0x4 => self.read_u8(addr) as u64,
            0x5 => self.read_u16(addr) as u64,
            0x6 => self.read_u32(addr) as u64,
            _ => panic!("Unknown load funct3=0x{:x}", funct3),
        };
        self.regs[rd] = res;
    }

    fn exec_op_23(&mut self, funct3: u32, rs1: usize, rs2: usize, word: u32) {
        let imm_0_4 = (word >> 7) & 0x1F;
        let imm_5_11 = (word >> 25) & 0x7F;
        let imm = (imm_0_4 | (imm_5_11 << 5)) as i32;
        let imm = ((imm << 20) >> 20) as i64;
        let addr = self.regs[rs1].wrapping_add(imm as u64) as usize;
        let val = self.regs[rs2];
        match funct3 {
            0x0 => self.write_u8(addr, val as u8),
            0x1 => self.write_u16(addr, val as u16),
            0x2 => self.write_u32(addr, val as u32),
            0x3 => self.write_u64(addr, val),
            _ => panic!("Unknown store funct3=0x{:x}", funct3),
        }
    }
}
