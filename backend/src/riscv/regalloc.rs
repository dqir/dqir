use std::collections::{HashMap, HashSet};
use ir::{Block, Function, Inst, Instruction, Value, ValueDef};
use crate::riscv::inst::Reg;

// Caller-saved allocatable registers (12 regs)
pub const CALLER_SAVED_POOL: [Reg; 12] = [
    Reg::T3, Reg::T4, Reg::T5, Reg::T6,
    Reg::A0, Reg::A1, Reg::A2, Reg::A3, Reg::A4, Reg::A5, Reg::A6, Reg::A7,
];

// Callee-saved allocatable registers (11 regs)
pub const CALLEE_SAVED_POOL: [Reg; 11] = [
    Reg::S1, Reg::S2, Reg::S3, Reg::S4, Reg::S5,
    Reg::S6, Reg::S7, Reg::S8, Reg::S9, Reg::S10, Reg::S11,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allocation {
    Reg(Reg),
    SpillSlot(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegAllocResult {
    pub allocations: HashMap<Value, Allocation>,
    pub used_callee_saved: HashSet<Reg>,
    pub num_spill_slots: i32,
}

pub fn operands_of(inst: &Instruction) -> Vec<Value> {
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) => vec![],
        Instruction::Add(v1, v2)
        | Instruction::Sub(v1, v2)
        | Instruction::Mul(v1, v2)
        | Instruction::Div(v1, v2)
        | Instruction::Rem(v1, v2)
        | Instruction::And(v1, v2)
        | Instruction::Or(v1, v2)
        | Instruction::Xor(v1, v2)
        | Instruction::Shl(v1, v2)
        | Instruction::Shr(v1, v2)
        | Instruction::Sar(v1, v2)
        | Instruction::Cmp(_, v1, v2)
        | Instruction::Store(v1, v2, _) => vec![*v1, *v2],
        Instruction::Neg(v)
        | Instruction::Not(v)
        | Instruction::Load(_, v, _)
        | Instruction::Br(v, _, _)
        | Instruction::Ret(Some(v)) => vec![*v],
        Instruction::Ret(None) | Instruction::Jmp(_) => vec![],
        Instruction::Call(_, args) => args.clone(),
        Instruction::Phi(incoming) => incoming.iter().map(|(_, v)| *v).collect(),
    }
}

fn successors_of(func: &Function, block: Block) -> Vec<Block> {
    if let Some(&last_id) = func.blocks[block].insts.last() {
        match &func.insts[last_id] {
            Instruction::Jmp(target) => vec![*target],
            Instruction::Br(_, true_b, false_b) => vec![*true_b, *false_b],
            _ => vec![],
        }
    } else {
        vec![]
    }
}

pub fn allocate_registers(func: &Function) -> RegAllocResult {
    let entry_block = func.entry_block.unwrap_or(Block(0));
    let mut blocks: Vec<_> = func.blocks.iter().collect();
    blocks.sort_by_key(|(id, _)| if *id == entry_block { 0 } else { id.0 + 1 });

    let mut inst_indices: HashMap<Inst, usize> = HashMap::new();
    let mut block_range: HashMap<Block, (usize, usize)> = HashMap::new();
    let mut call_indices: HashSet<usize> = HashSet::new();
    let mut current_idx = 1_usize; // 0 is reserved for function parameter definitions

    for (block_id, block_data) in &blocks {
        let start_idx = current_idx;
        for &inst_id in &block_data.insts {
            inst_indices.insert(inst_id, current_idx);
            if matches!(&func.insts[inst_id], Instruction::Call(_, _)) {
                call_indices.insert(current_idx);
            }
            current_idx += 1;
        }
        let end_idx = if current_idx > start_idx { current_idx - 1 } else { start_idx };
        block_range.insert(*block_id, (start_idx, end_idx));
    }

    // Liveness analysis (Live-in / Live-out)
    let mut def_set: HashMap<Block, HashSet<Value>> = HashMap::new();
    let mut use_set: HashMap<Block, HashSet<Value>> = HashMap::new();
    for (block_id, block_data) in &blocks {
        let mut b_def = HashSet::new();
        let mut b_use = HashSet::new();
        for &inst_id in &block_data.insts {
            let inst = &func.insts[inst_id];
            if let Instruction::Phi(_) = inst {
                // Phi operands are evaluated on predecessor edges.
                // We only define the result value here.
            } else {
                for v in operands_of(inst) {
                    if !b_def.contains(&v) {
                        b_use.insert(v);
                    }
                }
            }
            // Check if instruction defines a result value
            for (val_id, vdata) in func.values.iter() {
                if vdata.def == ValueDef::Inst(inst_id) {
                    b_def.insert(val_id);
                }
            }
        }
        def_set.insert(*block_id, b_def);
        use_set.insert(*block_id, b_use);
    }

    // Add Phi incoming operands to predecessor use_set
    for (_block_id, block_data) in &blocks {
        for &inst_id in &block_data.insts {
            if let Instruction::Phi(incoming) = &func.insts[inst_id] {
                for (pred_block, val) in incoming {
                    if let Some(pred_def) = def_set.get(pred_block) {
                        if !pred_def.contains(val) {
                            if let Some(pred_use) = use_set.get_mut(pred_block) {
                                pred_use.insert(*val);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut live_in: HashMap<Block, HashSet<Value>> = HashMap::new();
    let mut live_out: HashMap<Block, HashSet<Value>> = HashMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (block_id, _) in blocks.iter().rev() {
            let mut new_out = HashSet::new();
            for succ in successors_of(func, *block_id) {
                if let Some(succ_in) = live_in.get(&succ) {
                    new_out.extend(succ_in.iter().cloned());
                }
            }
            let old_out = live_out.entry(*block_id).or_default();
            if new_out != *old_out {
                *old_out = new_out.clone();
                changed = true;
            }

            let mut new_in = use_set[block_id].clone();
            let b_def = &def_set[block_id];
            for v in &new_out {
                if !b_def.contains(v) {
                    new_in.insert(*v);
                }
            }
            let old_in = live_in.entry(*block_id).or_default();
            if new_in != *old_in {
                *old_in = new_in;
                changed = true;
            }
        }
    }

    // Build Live Intervals [start, end]
    let mut intervals: HashMap<Value, (usize, usize)> = HashMap::new();
    for (val_id, vdata) in func.values.iter() {
        let def_idx = match vdata.def {
            ValueDef::Inst(inst_id) => *inst_indices.get(&inst_id).unwrap_or(&0),
            ValueDef::Param(_) => 0,
        };
        intervals.insert(val_id, (def_idx, def_idx));
    }

    for (inst_id, &idx) in &inst_indices {
        let inst = &func.insts[*inst_id];
        if let Instruction::Phi(incoming) = inst {
            for (pred_block, val) in incoming {
                let pred_end = block_range[pred_block].1;
                if let Some(entry) = intervals.get_mut(val) {
                    entry.1 = entry.1.max(pred_end);
                }
            }
        } else {
            for val in operands_of(inst) {
                if let Some(entry) = intervals.get_mut(&val) {
                    entry.1 = entry.1.max(idx);
                }
            }
        }
    }

    for (block, &(b_start, b_end)) in &block_range {
        if let Some(in_set) = live_in.get(block) {
            for val in in_set {
                if let Some(entry) = intervals.get_mut(val) {
                    entry.0 = entry.0.min(b_start);
                    entry.1 = entry.1.max(b_start);
                }
            }
        }
        if let Some(out_set) = live_out.get(block) {
            for val in out_set {
                if let Some(entry) = intervals.get_mut(val) {
                    entry.1 = entry.1.max(b_end);
                }
            }
        }
    }

    // Linear Scan Allocation
    let mut sorted_intervals: Vec<(Value, usize, usize)> = intervals
        .iter()
        .map(|(&v, &(s, e))| (v, s, e))
        .collect();
    sorted_intervals.sort_by_key(|(_, s, _)| *s);

    let mut active: Vec<(Value, usize, usize, Allocation)> = Vec::new();
    let mut free_caller_saved: Vec<Reg> = CALLER_SAVED_POOL.to_vec();
    free_caller_saved.reverse();
    let mut free_callee_saved: Vec<Reg> = CALLEE_SAVED_POOL.to_vec();
    free_callee_saved.reverse();

    let mut used_callee_saved: HashSet<Reg> = HashSet::new();
    let mut allocations: HashMap<Value, Allocation> = HashMap::new();
    let mut next_spill_slot = 0_i32;

    for (val, start, end) in sorted_intervals {
        // Expire intervals ending at or before current start
        let mut i = 0;
        while i < active.len() {
            if active[i].2 <= start {
                let (_, _, _, alloc) = active.remove(i);
                if let Allocation::Reg(r) = alloc {
                    if CALLER_SAVED_POOL.contains(&r) {
                        free_caller_saved.push(r);
                    } else if CALLEE_SAVED_POOL.contains(&r) {
                        free_callee_saved.push(r);
                    }
                }
            } else {
                i += 1;
            }
        }

        let crosses_call = call_indices.iter().any(|&idx| start <= idx && idx <= end);

        let chosen_reg = if crosses_call {
            free_callee_saved.pop()
        } else {
            if let Some(r) = free_caller_saved.pop() {
                Some(r)
            } else {
                free_callee_saved.pop()
            }
        };

        if let Some(reg) = chosen_reg {
            if CALLEE_SAVED_POOL.contains(&reg) {
                used_callee_saved.insert(reg);
            }
            allocations.insert(val, Allocation::Reg(reg));
            let pos = active.partition_point(|item| item.2 <= end);
            active.insert(pos, (val, start, end, Allocation::Reg(reg)));
        } else {
            // Spill candidate search
            let cand_idx = active.iter().rposition(|item| {
                if let Allocation::Reg(r) = item.3 {
                    if crosses_call {
                        CALLEE_SAVED_POOL.contains(&r)
                    } else {
                        true
                    }
                } else {
                    false
                }
            });

            if let Some(idx) = cand_idx {
                if active[idx].2 > end {
                    let (spilled_val, _, _, alloc) = active.remove(idx);
                    let reg = match alloc { Allocation::Reg(r) => r, _ => unreachable!() };
                    let slot = next_spill_slot;
                    next_spill_slot += 1;
                    allocations.insert(spilled_val, Allocation::SpillSlot(slot));

                    allocations.insert(val, Allocation::Reg(reg));
                    let pos = active.partition_point(|item| item.2 <= end);
                    active.insert(pos, (val, start, end, Allocation::Reg(reg)));
                } else {
                    let slot = next_spill_slot;
                    next_spill_slot += 1;
                    allocations.insert(val, Allocation::SpillSlot(slot));
                }
            } else {
                let slot = next_spill_slot;
                next_spill_slot += 1;
                allocations.insert(val, Allocation::SpillSlot(slot));
            }
        }
    }

    RegAllocResult {
        allocations,
        used_callee_saved,
        num_spill_slots: next_spill_slot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::parse;

    #[test]
    fn test_regalloc_spilling_on_high_pressure() {
        let ir = r#"
func @high_pressure() -> i64 {
entry:
    %v0: i64 = const.int 1
    %v1: i64 = const.int 2
    %v2: i64 = const.int 3
    %v3: i64 = const.int 4
    %v4: i64 = const.int 5
    %v5: i64 = const.int 6
    %v6: i64 = const.int 7
    %v7: i64 = const.int 8
    %v8: i64 = const.int 9
    %v9: i64 = const.int 10
    %v10: i64 = const.int 11
    %v11: i64 = const.int 12
    %v12: i64 = const.int 13
    %v13: i64 = const.int 14
    %v14: i64 = const.int 15
    %v15: i64 = const.int 16
    %v16: i64 = const.int 17
    %v17: i64 = const.int 18
    %v18: i64 = const.int 19
    %v19: i64 = const.int 20
    %v20: i64 = const.int 21
    %v21: i64 = const.int 22
    %v22: i64 = const.int 23
    %v23: i64 = const.int 24
    %v24: i64 = const.int 25
    %v25: i64 = const.int 26
    %v26: i64 = const.int 27
    %v27: i64 = const.int 28
    %v28: i64 = const.int 29
    %v29: i64 = const.int 30

    %s1: i64 = add %v0, %v1
    %s2: i64 = add %s1, %v2
    %s3: i64 = add %s2, %v3
    %s4: i64 = add %s3, %v4
    %s5: i64 = add %s4, %v5
    %s6: i64 = add %s5, %v6
    %s7: i64 = add %s6, %v7
    %s8: i64 = add %s7, %v8
    %s9: i64 = add %s8, %v9
    %s10: i64 = add %s9, %v10
    %s11: i64 = add %s10, %v11
    %s12: i64 = add %s11, %v12
    %s13: i64 = add %s12, %v13
    %s14: i64 = add %s13, %v14
    %s15: i64 = add %s14, %v15
    %s16: i64 = add %s15, %v16
    %s17: i64 = add %s16, %v17
    %s18: i64 = add %s17, %v18
    %s19: i64 = add %s18, %v19
    %s20: i64 = add %s19, %v20
    %s21: i64 = add %s20, %v21
    %s22: i64 = add %s21, %v22
    %s23: i64 = add %s22, %v23
    %s24: i64 = add %s23, %v24
    %s25: i64 = add %s24, %v25
    %s26: i64 = add %s25, %v26
    %s27: i64 = add %s26, %v27
    %s28: i64 = add %s27, %v28
    %s29: i64 = add %s28, %v29

    %d0: i64 = sub %s29, %v0
    %d1: i64 = sub %d0, %v1
    %d2: i64 = sub %d1, %v2
    %d3: i64 = sub %d2, %v3
    %d4: i64 = sub %d3, %v4
    %d5: i64 = sub %d4, %v5
    %d6: i64 = sub %d5, %v6
    %d7: i64 = sub %d6, %v7
    %d8: i64 = sub %d7, %v8
    %d9: i64 = sub %d8, %v9
    %d10: i64 = sub %d9, %v10
    %d11: i64 = sub %d10, %v11
    %d12: i64 = sub %d11, %v12
    %d13: i64 = sub %d12, %v13
    %d14: i64 = sub %d13, %v14
    %d15: i64 = sub %d14, %v15
    %d16: i64 = sub %d15, %v16
    %d17: i64 = sub %d16, %v17
    %d18: i64 = sub %d17, %v18
    %d19: i64 = sub %d18, %v19
    %d20: i64 = sub %d19, %v20
    %d21: i64 = sub %d20, %v21
    %d22: i64 = sub %d21, %v22
    %d23: i64 = sub %d22, %v23
    %d24: i64 = sub %d23, %v24
    %d25: i64 = sub %d24, %v25
    %d26: i64 = sub %d25, %v26
    %d27: i64 = sub %d26, %v27
    %d28: i64 = sub %d27, %v28
    %res: i64 = sub %d28, %v29

    ret %res
}
"#;
        let module = parse(ir.trim()).unwrap();
        let (_, func) = module.functions.iter().next().unwrap();
        let res = allocate_registers(func);
        // With 30 simultaneously live variables and 23 registers, we MUST spill to memory!
        assert!(res.num_spill_slots >= 7, "Expected at least 7 spill slots, got {}", res.num_spill_slots);
        assert!(!res.used_callee_saved.is_empty(), "Should utilize callee-saved registers under high pressure");
    }
}
