use std::collections::HashMap;
use ir::{Block, CmpOp, Function, Inst, Instruction, Type, Value, ValueDef};
use crate::riscv::inst::{MInst, Reg};
use crate::riscv::regalloc::{allocate_registers, Allocation, RegAllocResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredFunction {
    pub name: String,
    pub insts: Vec<MInst>,
    pub frame_size: i32,
}

pub fn lower_function(func: &Function) -> LoweredFunction {
    lower_function_with_regalloc(func, allocate_registers(func))
}

pub fn lower_function_with_regalloc(func: &Function, regalloc: RegAllocResult) -> LoweredFunction {
    let mut lowerer = Lowerer::with_regalloc(func, regalloc);
    lowerer.lower();
    LoweredFunction {
        name: func.name.clone(),
        insts: lowerer.insts,
        frame_size: lowerer.frame_size,
    }
}

pub fn schedule_instructions(lowered: &LoweredFunction) -> LoweredFunction {
    // For our RV64GC target in v0, instruction scheduling is a peephole/identity pass
    // that serves as an explicit query boundary in the Salsa query graph.
    lowered.clone()
}

struct Lowerer<'a> {
    func: &'a Function,
    insts: Vec<MInst>,
    regalloc: RegAllocResult,
    callee_saved_list: Vec<Reg>,
    alloca_offsets: HashMap<Inst, i32>,
    inst_to_val: HashMap<Inst, Value>,
    frame_size: i32,
    backup_offset: i32,
    next_synthetic_block_id: u32,
}

impl<'a> Lowerer<'a> {
    fn with_regalloc(func: &'a Function, regalloc: RegAllocResult) -> Self {
        let mut inst_to_val = HashMap::new();
        for (val_id, vdata) in func.values.iter() {
            if let ValueDef::Inst(inst_id) = vdata.def {
                inst_to_val.insert(inst_id, val_id);
            }
        }

        let mut callee_saved_list: Vec<Reg> = regalloc.used_callee_saved.iter().cloned().collect();
        callee_saved_list.sort_by_key(|r| r.0);

        let mut max_call_args = 0;
        let mut max_phis = 0;
        for (_, bdata) in func.blocks.iter() {
            let mut phis = 0;
            for &inst_id in &bdata.insts {
                match &func.insts[inst_id] {
                    Instruction::Phi(_) => phis += 1,
                    Instruction::Call(_, args) => max_call_args = max_call_args.max(args.len()),
                    _ => {}
                }
            }
            max_phis = max_phis.max(phis);
        }
        let backup_slots = 16.max(func.params.len()).max(max_phis).max(max_call_args) as i32;

        let callee_len = callee_saved_list.len() as i32;
        let mut current_offset = -16 - 8 * callee_len - 8 * regalloc.num_spill_slots;

        let backup_offset = current_offset - 8 * backup_slots;
        current_offset = backup_offset;

        let mut alloca_offsets = HashMap::new();
        for (inst_id, inst) in func.insts.iter() {
            if let Instruction::Alloca(ty) = inst {
                let size = ty.byte_size().unwrap_or(8) as i32;
                let alloc_size = size.max(64);
                let alloc_size = (alloc_size + 7) & !7;
                current_offset -= alloc_size;
                alloca_offsets.insert(inst_id, current_offset);
            }
        }

        let total_needed = -current_offset;
        let frame_size = (total_needed + 15) & !15;

        Self {
            func,
            insts: Vec::new(),
            regalloc,
            callee_saved_list,
            alloca_offsets,
            inst_to_val,
            frame_size,
            backup_offset,
            next_synthetic_block_id: 100_000,
        }
    }

    fn next_synthetic_label(&mut self) -> Block {
        let id = self.next_synthetic_block_id;
        self.next_synthetic_block_id += 1;
        Block(id)
    }

    fn spill_offset(&self, slot: i32) -> i32 {
        -16 - 8 * self.callee_saved_list.len() as i32 - 8 * (slot + 1)
    }

    fn load_operand(&mut self, val: Value, scratch: Reg) -> Reg {
        match self.regalloc.allocations.get(&val).copied().unwrap_or(Allocation::SpillSlot(0)) {
            Allocation::Reg(r) => r,
            Allocation::SpillSlot(slot) => {
                let offset = self.spill_offset(slot);
                self.emit_load_reg_offset(scratch, Reg::FP, offset);
                scratch
            }
        }
    }

    fn get_dest(&self, val: Value, scratch: Reg) -> Reg {
        match self.regalloc.allocations.get(&val).copied().unwrap_or(Allocation::SpillSlot(0)) {
            Allocation::Reg(r) => r,
            Allocation::SpillSlot(_) => scratch,
        }
    }

    fn store_result(&mut self, val: Value, from_reg: Reg) {
        match self.regalloc.allocations.get(&val).copied().unwrap_or(Allocation::SpillSlot(0)) {
            Allocation::Reg(r) => {
                if r != from_reg {
                    self.insts.push(MInst::mv(r, from_reg));
                }
            }
            Allocation::SpillSlot(slot) => {
                let offset = self.spill_offset(slot);
                self.emit_store_reg_offset(Reg::FP, from_reg, offset);
            }
        }
    }

    fn lower(&mut self) {
        let entry_block = self.func.entry_block.unwrap_or(Block(0));
        let mut blocks: Vec<_> = self.func.blocks.iter().collect();
        blocks.sort_by_key(|(id, _)| if *id == entry_block { 0 } else { id.0 + 1 });

        for (block_id, block_data) in blocks {
            self.insts.push(MInst::Label(block_id));

            if block_id == entry_block {
                self.emit_prologue();
                self.save_params();
            }

            for &inst_id in &block_data.insts {
                self.lower_inst(block_id, inst_id);
            }
        }
    }

    fn emit_prologue(&mut self) {
        self.emit_add_imm(Reg::SP, Reg::SP, -self.frame_size);
        self.emit_store_reg_offset(Reg::SP, Reg::RA, self.frame_size - 8);
        self.emit_store_reg_offset(Reg::SP, Reg::FP, self.frame_size - 16);
        self.emit_add_imm(Reg::FP, Reg::SP, self.frame_size);

        for i in 0..self.callee_saved_list.len() {
            let reg = self.callee_saved_list[i];
            let offset = -16 - 8 * (i as i32 + 1);
            self.emit_store_reg_offset(Reg::FP, reg, offset);
        }
    }

    fn emit_epilogue(&mut self) {
        for i in 0..self.callee_saved_list.len() {
            let reg = self.callee_saved_list[i];
            let offset = -16 - 8 * (i as i32 + 1);
            self.emit_load_reg_offset(reg, Reg::FP, offset);
        }

        self.emit_load_reg_offset(Reg::RA, Reg::SP, self.frame_size - 8);
        self.emit_load_reg_offset(Reg::FP, Reg::SP, self.frame_size - 16);
        self.emit_add_imm(Reg::SP, Reg::SP, self.frame_size);
        self.insts.push(MInst::Ret);
    }

    fn save_params(&mut self) {
        let num_params = self.func.params.len();
        for i in 0..num_params.min(8) {
            let reg = Reg(10 + i as u8);
            self.emit_store_reg_offset(Reg::FP, reg, self.backup_offset + 8 * i as i32);
        }
        for (i, (_, val, _)) in self.func.params.iter().enumerate() {
            if i < 8 {
                self.emit_load_reg_offset(Reg::T0, Reg::FP, self.backup_offset + 8 * i as i32);
                self.store_result(*val, Reg::T0);
            } else {
                let caller_offset = 8 * (i as i32 - 8);
                self.emit_load_reg_offset(Reg::T0, Reg::FP, caller_offset);
                self.store_result(*val, Reg::T0);
            }
        }
    }

    fn emit_add_imm(&mut self, rd: Reg, rs: Reg, imm: i32) {
        if (-2048..=2047).contains(&imm) {
            self.insts.push(MInst::Addi(rd, rs, imm));
        } else {
            self.insts.push(MInst::Li64(Reg::T0, imm as i64));
            self.insts.push(MInst::Add(rd, rs, Reg::T0));
        }
    }

    fn emit_store_reg_offset(&mut self, base: Reg, src: Reg, offset: i32) {
        if (-2048..=2047).contains(&offset) {
            self.insts.push(MInst::Sd(base, src, offset));
        } else {
            let addr_reg = if src == Reg::T0 { Reg::T1 } else { Reg::T0 };
            self.insts.push(MInst::Li64(addr_reg, offset as i64));
            self.insts.push(MInst::Add(addr_reg, base, addr_reg));
            self.insts.push(MInst::Sd(addr_reg, src, 0));
        }
    }

    fn emit_load_reg_offset(&mut self, dest: Reg, base: Reg, offset: i32) {
        if (-2048..=2047).contains(&offset) {
            self.insts.push(MInst::Ld(dest, base, offset));
        } else {
            let addr_reg = if dest == Reg::T0 { Reg::T1 } else { Reg::T0 };
            self.insts.push(MInst::Li64(addr_reg, offset as i64));
            self.insts.push(MInst::Add(addr_reg, base, addr_reg));
            self.insts.push(MInst::Ld(dest, addr_reg, 0));
        }
    }

    fn val_type(&self, val: Value) -> Type {
        self.func.values[val].ty
    }

    fn has_phis_from(&self, target_block: Block, pred_block: Block) -> bool {
        for &inst_id in &self.func.blocks[target_block].insts {
            if let Instruction::Phi(incoming) = &self.func.insts[inst_id] {
                if incoming.iter().any(|(b, _)| *b == pred_block) {
                    return true;
                }
            }
        }
        false
    }

    fn lower_edge_phis(&mut self, pred_block: Block, target_block: Block) {
        let mut phi_pairs = Vec::new();
        for &inst_id in &self.func.blocks[target_block].insts {
            if let Instruction::Phi(incoming) = &self.func.insts[inst_id] {
                if let Some((_, incoming_val)) = incoming.iter().find(|(b, _)| *b == pred_block) {
                    if let Some(&res_val) = self.inst_to_val.get(&inst_id) {
                        phi_pairs.push((*incoming_val, res_val));
                    }
                }
            }
        }

        for (i, &(in_val, _)) in phi_pairs.iter().enumerate() {
            let r = self.load_operand(in_val, Reg::T0);
            self.emit_store_reg_offset(Reg::FP, r, self.backup_offset + 8 * i as i32);
        }

        for (i, &(_, res_val)) in phi_pairs.iter().enumerate() {
            self.emit_load_reg_offset(Reg::T0, Reg::FP, self.backup_offset + 8 * i as i32);
            self.store_result(res_val, Reg::T0);
        }
    }

    fn lower_inst(&mut self, current_block: Block, inst_id: Inst) {
        let inst = &self.func.insts[inst_id];
        let res_val = self.inst_to_val.get(&inst_id).copied();

        match inst {
            Instruction::Const(imm) => {
                let res = res_val.unwrap();
                let dest = self.get_dest(res, Reg::T0);
                self.insts.push(MInst::Li64(dest, imm.as_i64()));
                self.store_result(res, dest);
            }
            Instruction::Add(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Add, MInst::Addw),
            Instruction::Sub(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Sub, MInst::Subw),
            Instruction::Mul(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Mul, MInst::Mulw),
            Instruction::Div(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Div, MInst::Divw),
            Instruction::Rem(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Rem, MInst::Remw),
            Instruction::And(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::And, MInst::And),
            Instruction::Or(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Or, MInst::Or),
            Instruction::Xor(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Xor, MInst::Xor),
            Instruction::Shl(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Sll, MInst::Sllw),
            Instruction::Shr(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Srl, MInst::Srlw),
            Instruction::Sar(v1, v2) => self.lower_binary(*v1, *v2, res_val.unwrap(), MInst::Sra, MInst::Sraw),
            Instruction::Neg(val) => {
                let res = res_val.unwrap();
                let r1 = self.load_operand(*val, Reg::T0);
                let dest = self.get_dest(res, Reg::T1);
                if self.val_type(res).bit_width() == Some(32) {
                    self.insts.push(MInst::Subw(dest, Reg::ZERO, r1));
                } else {
                    self.insts.push(MInst::Sub(dest, Reg::ZERO, r1));
                }
                self.store_result(res, dest);
            }
            Instruction::Not(val) => {
                let res = res_val.unwrap();
                let r1 = self.load_operand(*val, Reg::T0);
                let dest = self.get_dest(res, Reg::T1);
                self.insts.push(MInst::Xori(dest, r1, -1));
                self.store_result(res, dest);
            }
            Instruction::Cmp(op, v1, v2) => {
                let res = res_val.unwrap();
                let r1 = self.load_operand(*v1, Reg::T0);
                let r2 = self.load_operand(*v2, Reg::T1);
                let dest = self.get_dest(res, Reg::T0);
                match op {
                    CmpOp::Eq => {
                        self.insts.push(MInst::Xor(dest, r1, r2));
                        self.insts.push(MInst::Sltiu(dest, dest, 1));
                    }
                    CmpOp::Ne => {
                        self.insts.push(MInst::Xor(dest, r1, r2));
                        self.insts.push(MInst::Sltu(dest, Reg::ZERO, dest));
                    }
                    CmpOp::Lt => {
                        self.insts.push(MInst::Slt(dest, r1, r2));
                    }
                    CmpOp::Gt => {
                        self.insts.push(MInst::Slt(dest, r2, r1));
                    }
                    CmpOp::Le => {
                        self.insts.push(MInst::Slt(dest, r2, r1));
                        self.insts.push(MInst::Xori(dest, dest, 1));
                    }
                    CmpOp::Ge => {
                        self.insts.push(MInst::Slt(dest, r1, r2));
                        self.insts.push(MInst::Xori(dest, dest, 1));
                    }
                    CmpOp::Ult => {
                        self.insts.push(MInst::Sltu(dest, r1, r2));
                    }
                    CmpOp::Ugt => {
                        self.insts.push(MInst::Sltu(dest, r2, r1));
                    }
                    CmpOp::Ule => {
                        self.insts.push(MInst::Sltu(dest, r2, r1));
                        self.insts.push(MInst::Xori(dest, dest, 1));
                    }
                    CmpOp::Uge => {
                        self.insts.push(MInst::Sltu(dest, r1, r2));
                        self.insts.push(MInst::Xori(dest, dest, 1));
                    }
                }
                self.store_result(res, dest);
            }
            Instruction::Alloca(_) => {
                let res = res_val.unwrap();
                let buf_offset = *self.alloca_offsets.get(&inst_id).expect("Alloca offset not found");
                let dest = self.get_dest(res, Reg::T0);
                self.emit_add_imm(dest, Reg::FP, buf_offset);
                self.store_result(res, dest);
            }
            Instruction::Load(ty, ptr_val, offset) => {
                let res = res_val.unwrap();
                let r_ptr = self.load_operand(*ptr_val, Reg::T0);
                let dest = self.get_dest(res, Reg::T1);
                if (-2048..=2047).contains(offset) {
                    match ty {
                        Type::I1 | Type::I8 => self.insts.push(MInst::Lb(dest, r_ptr, *offset)),
                        Type::I16 => self.insts.push(MInst::Lh(dest, r_ptr, *offset)),
                        Type::I32 | Type::F32 => self.insts.push(MInst::Lw(dest, r_ptr, *offset)),
                        _ => self.insts.push(MInst::Ld(dest, r_ptr, *offset)),
                    }
                } else {
                    self.insts.push(MInst::Li64(Reg::T2, *offset as i64));
                    self.insts.push(MInst::Add(Reg::T2, r_ptr, Reg::T2));
                    match ty {
                        Type::I1 | Type::I8 => self.insts.push(MInst::Lb(dest, Reg::T2, 0)),
                        Type::I16 => self.insts.push(MInst::Lh(dest, Reg::T2, 0)),
                        Type::I32 | Type::F32 => self.insts.push(MInst::Lw(dest, Reg::T2, 0)),
                        _ => self.insts.push(MInst::Ld(dest, Reg::T2, 0)),
                    }
                }
                self.store_result(res, dest);
            }
            Instruction::Store(val, ptr_val, offset) => {
                let r_val = self.load_operand(*val, Reg::T0);
                let r_ptr = self.load_operand(*ptr_val, Reg::T1);
                let ty = self.val_type(*val);
                if (-2048..=2047).contains(offset) {
                    match ty {
                        Type::I1 | Type::I8 => self.insts.push(MInst::Sb(r_ptr, r_val, *offset)),
                        Type::I16 => self.insts.push(MInst::Sh(r_ptr, r_val, *offset)),
                        Type::I32 | Type::F32 => self.insts.push(MInst::Sw(r_ptr, r_val, *offset)),
                        _ => self.insts.push(MInst::Sd(r_ptr, r_val, *offset)),
                    }
                } else {
                    self.insts.push(MInst::Li64(Reg::T2, *offset as i64));
                    self.insts.push(MInst::Add(Reg::T2, r_ptr, Reg::T2));
                    match ty {
                        Type::I1 | Type::I8 => self.insts.push(MInst::Sb(Reg::T2, r_val, 0)),
                        Type::I16 => self.insts.push(MInst::Sh(Reg::T2, r_val, 0)),
                        Type::I32 | Type::F32 => self.insts.push(MInst::Sw(Reg::T2, r_val, 0)),
                        _ => self.insts.push(MInst::Sd(Reg::T2, r_val, 0)),
                    }
                }
            }
            Instruction::Jmp(target) => {
                self.lower_edge_phis(current_block, *target);
                self.insts.push(MInst::Jal(Reg::ZERO, *target));
            }
            Instruction::Br(cond_val, true_block, false_block) => {
                let r_cond = self.load_operand(*cond_val, Reg::T0);
                let has_true_phis = self.has_phis_from(*true_block, current_block);
                let has_false_phis = self.has_phis_from(*false_block, current_block);

                if !has_true_phis && !has_false_phis {
                    self.insts.push(MInst::Bne(r_cond, Reg::ZERO, *true_block));
                    self.insts.push(MInst::Jal(Reg::ZERO, *false_block));
                } else {
                    let true_label = self.next_synthetic_label();
                    self.insts.push(MInst::Bne(r_cond, Reg::ZERO, true_label));

                    self.lower_edge_phis(current_block, *false_block);
                    self.insts.push(MInst::Jal(Reg::ZERO, *false_block));

                    self.insts.push(MInst::Label(true_label));
                    self.lower_edge_phis(current_block, *true_block);
                    self.insts.push(MInst::Jal(Reg::ZERO, *true_block));
                }
            }
            Instruction::Ret(opt_val) => {
                if let Some(val) = opt_val {
                    let r = self.load_operand(*val, Reg::T0);
                    if r != Reg::A0 {
                        self.insts.push(MInst::mv(Reg::A0, r));
                    }
                }
                self.emit_epilogue();
            }
            Instruction::Call(name, args) => {
                for (i, &arg) in args.iter().enumerate() {
                    let r = self.load_operand(arg, Reg::T0);
                    self.emit_store_reg_offset(Reg::FP, r, self.backup_offset + 8 * i as i32);
                }
                for i in 0..args.len() {
                    if i < 8 {
                        let reg = Reg(10 + i as u8);
                        self.emit_load_reg_offset(reg, Reg::FP, self.backup_offset + 8 * i as i32);
                    } else {
                        self.emit_load_reg_offset(Reg::T0, Reg::FP, self.backup_offset + 8 * i as i32);
                        let offset = 8 * (i as i32 - 8);
                        self.emit_store_reg_offset(Reg::SP, Reg::T0, offset);
                    }
                }
                self.insts.push(MInst::Call(name.clone()));
                if let Some(res) = res_val {
                    self.store_result(res, Reg::A0);
                }
            }
            Instruction::Phi(_) => {}
        }
    }

    fn lower_binary(
        &mut self,
        v1: Value,
        v2: Value,
        res: Value,
        op64: fn(Reg, Reg, Reg) -> MInst,
        op32: fn(Reg, Reg, Reg) -> MInst,
    ) {
        let r1 = self.load_operand(v1, Reg::T0);
        let r2 = self.load_operand(v2, Reg::T1);
        let dest = self.get_dest(res, Reg::T0);
        if self.val_type(res).bit_width() == Some(32) {
            self.insts.push(op32(dest, r1, r2));
        } else {
            self.insts.push(op64(dest, r1, r2));
        }
        self.store_result(res, dest);
    }
}
