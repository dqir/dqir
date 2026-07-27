use std::collections::{HashMap, HashSet};
use crate::arena::{Inst, Value};
use crate::function::{Function, ValueDef};
use crate::inst::{CmpOp, Immediate, Instruction};

/// Run function-local optimization passes: Constant Folding, CSE, and DCE.
pub fn optimize_function(func: &Function) -> Function {
    let mut f = func.clone();
    let mut changed = true;
    while changed {
        changed = false;
        changed |= constant_fold(&mut f);
        changed |= common_subexpression_elimination(&mut f);
        changed |= dead_code_elimination(&mut f);
    }
    f
}

fn has_side_effects(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::Store(_, _, _)
            | Instruction::Call(_, _)
            | Instruction::Jmp(_)
            | Instruction::Br(_, _, _)
            | Instruction::Ret(_)
    )
}

fn get_operands(inst: &Instruction) -> Vec<Value> {
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::Jmp(_) => vec![],
        Instruction::Neg(v) | Instruction::Not(v) | Instruction::Load(_, v, _) | Instruction::Ret(Some(v)) => {
            vec![*v]
        }
        Instruction::Ret(None) => vec![],
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::Div(a, b)
        | Instruction::Rem(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::Shl(a, b)
        | Instruction::Shr(a, b)
        | Instruction::Sar(a, b)
        | Instruction::Cmp(_, a, b)
        | Instruction::Store(a, b, _) => vec![*a, *b],
        Instruction::Br(c, _, _) => vec![*c],
        Instruction::Call(_, args) => args.clone(),
        Instruction::Phi(pairs) => pairs.iter().map(|(_, v)| *v).collect(),
    }
}

fn rewrite_operands(inst: &mut Instruction, remap: &HashMap<Value, Value>) {
    let resolve = |v: &mut Value| {
        let mut curr = *v;
        while let Some(&next) = remap.get(&curr) {
            curr = next;
        }
        *v = curr;
    };

    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::Jmp(_) => {}
        Instruction::Neg(v) | Instruction::Not(v) | Instruction::Load(_, v, _) | Instruction::Ret(Some(v)) => {
            resolve(v);
        }
        Instruction::Ret(None) => {}
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::Div(a, b)
        | Instruction::Rem(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::Shl(a, b)
        | Instruction::Shr(a, b)
        | Instruction::Sar(a, b)
        | Instruction::Cmp(_, a, b)
        | Instruction::Store(a, b, _) => {
            resolve(a);
            resolve(b);
        }
        Instruction::Br(c, _, _) => {
            resolve(c);
        }
        Instruction::Call(_, args) => {
            for arg in args.iter_mut() {
                resolve(arg);
            }
        }
        Instruction::Phi(pairs) => {
            for (_, v) in pairs.iter_mut() {
                resolve(v);
            }
        }
    }
}

/// Pass 1: Constant Folding
fn constant_fold(func: &mut Function) -> bool {
    let mut changed = false;

    // First build a map of SSA value -> constant immediate
    let mut const_vals: HashMap<Value, Immediate> = HashMap::new();
    for (val_id, vdata) in func.values.iter() {
        if let ValueDef::Inst(inst_id) = vdata.def {
            if let Instruction::Const(imm) = &func.insts[inst_id] {
                const_vals.insert(val_id, *imm);
            }
        }
    }

    for (_inst_id, inst) in func.insts.iter_mut() {
        let folded = match inst {
            Instruction::Add(a, b) => {
                if let (Some(&imm_a), Some(&imm_b)) = (const_vals.get(a), const_vals.get(b)) {
                    match (imm_a, imm_b) {
                        (Immediate::F64(bits_a), Immediate::F64(bits_b)) => {
                            let fa = f64::from_bits(bits_a);
                            let fb = f64::from_bits(bits_b);
                            Some(Immediate::F64((fa + fb).to_bits()))
                        }
                        (Immediate::F32(bits_a), Immediate::F32(bits_b)) => {
                            let fa = f32::from_bits(bits_a);
                            let fb = f32::from_bits(bits_b);
                            Some(Immediate::F32((fa + fb).to_bits()))
                        }
                        _ => Some(Immediate::I64(imm_a.as_i64().wrapping_add(imm_b.as_i64()))),
                    }
                } else {
                    None
                }
            }
            Instruction::Sub(a, b) => {
                if let (Some(&imm_a), Some(&imm_b)) = (const_vals.get(a), const_vals.get(b)) {
                    match (imm_a, imm_b) {
                        (Immediate::F64(bits_a), Immediate::F64(bits_b)) => {
                            let fa = f64::from_bits(bits_a);
                            let fb = f64::from_bits(bits_b);
                            Some(Immediate::F64((fa - fb).to_bits()))
                        }
                        (Immediate::F32(bits_a), Immediate::F32(bits_b)) => {
                            let fa = f32::from_bits(bits_a);
                            let fb = f32::from_bits(bits_b);
                            Some(Immediate::F32((fa - fb).to_bits()))
                        }
                        _ => Some(Immediate::I64(imm_a.as_i64().wrapping_sub(imm_b.as_i64()))),
                    }
                } else {
                    None
                }
            }
            Instruction::Mul(a, b) => {
                if let (Some(&imm_a), Some(&imm_b)) = (const_vals.get(a), const_vals.get(b)) {
                    match (imm_a, imm_b) {
                        (Immediate::F64(bits_a), Immediate::F64(bits_b)) => {
                            let fa = f64::from_bits(bits_a);
                            let fb = f64::from_bits(bits_b);
                            Some(Immediate::F64((fa * fb).to_bits()))
                        }
                        (Immediate::F32(bits_a), Immediate::F32(bits_b)) => {
                            let fa = f32::from_bits(bits_a);
                            let fb = f32::from_bits(bits_b);
                            Some(Immediate::F32((fa * fb).to_bits()))
                        }
                        _ => Some(Immediate::I64(imm_a.as_i64().wrapping_mul(imm_b.as_i64()))),
                    }
                } else {
                    None
                }
            }
            Instruction::Cmp(op, a, b) => {
                if let (Some(&imm_a), Some(&imm_b)) = (const_vals.get(a), const_vals.get(b)) {
                    let va = imm_a.as_i64();
                    let vb = imm_b.as_i64();
                    let res = match op {
                        CmpOp::Eq => va == vb,
                        CmpOp::Ne => va != vb,
                        CmpOp::Lt => va < vb,
                        CmpOp::Gt => va > vb,
                        CmpOp::Le => va <= vb,
                        CmpOp::Ge => va >= vb,
                        CmpOp::Ult => (va as u64) < (vb as u64),
                        CmpOp::Ugt => (va as u64) > (vb as u64),
                        CmpOp::Ule => (va as u64) <= (vb as u64),
                        CmpOp::Uge => (va as u64) >= (vb as u64),
                    };
                    Some(Immediate::I64(if res { 1 } else { 0 }))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(imm) = folded {
            *inst = Instruction::Const(imm);
            changed = true;
        }
    }

    changed
}

/// Pass 2: Common Subexpression Elimination (CSE) within basic blocks
fn common_subexpression_elimination(func: &mut Function) -> bool {
    let mut changed = false;
    let mut remap: HashMap<Value, Value> = HashMap::new();

    // First rewrite all operands in all instructions if we already have remappings from previous blocks/iterations
    // For local CSE, we process block by block
    for (_blk_id, bdata) in func.blocks.iter() {
        let mut seen_insts: HashMap<Instruction, Value> = HashMap::new();
        for &inst_id in &bdata.insts {
            let inst = &mut func.insts[inst_id];
            rewrite_operands(inst, &remap);

            if !has_side_effects(inst) {
                // Find which value this instruction defines, if any
                let mut def_val = None;
                for (val_id, vdata) in func.values.iter() {
                    if vdata.def == ValueDef::Inst(inst_id) {
                        def_val = Some(val_id);
                        break;
                    }
                }

                if let Some(val_id) = def_val {
                    if let Some(&existing_val) = seen_insts.get(inst) {
                        // This instruction is identical to an earlier one in the same block!
                        remap.insert(val_id, existing_val);
                        changed = true;
                    } else {
                        seen_insts.insert(inst.clone(), val_id);
                    }
                }
            }
        }
    }

    // Apply any remappings found to the entire function
    if changed {
        for (_inst_id, inst) in func.insts.iter_mut() {
            rewrite_operands(inst, &remap);
        }
    }

    changed
}

/// Pass 3: Dead Code Elimination (DCE)
fn dead_code_elimination(func: &mut Function) -> bool {
    let mut changed = false;

    // Collect all referenced SSA values
    let mut used_vals: HashSet<Value> = HashSet::new();
    for (_inst_id, inst) in func.insts.iter() {
        for v in get_operands(inst) {
            used_vals.insert(v);
        }
    }

    // Identify which instructions define values
    let mut inst_to_val: HashMap<Inst, Value> = HashMap::new();
    for (val_id, vdata) in func.values.iter() {
        if let ValueDef::Inst(inst_id) = vdata.def {
            inst_to_val.insert(inst_id, val_id);
        }
    }

    // Remove side-effect-free instructions whose result value is never used
    for (_blk_id, bdata) in func.blocks.iter_mut() {
        let original_len = bdata.insts.len();
        bdata.insts.retain(|&inst_id| {
            let inst = &func.insts[inst_id];
            if has_side_effects(inst) {
                true
            } else if let Some(&val_id) = inst_to_val.get(&inst_id) {
                used_vals.contains(&val_id)
            } else {
                // Instruction defines no value and has no side effects -> dead
                false
            }
        });
        if bdata.insts.len() < original_len {
            changed = true;
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Type;

    #[test]
    fn test_constant_folding_add() {
        let mut func = Function::new("test", Type::I32);
        let b0 = func.create_block("entry");
        let (_, c10) = func.push_inst(b0, Instruction::Const(Immediate::I64(10)), Some(Type::I32));
        let (_, c20) = func.push_inst(b0, Instruction::Const(Immediate::I64(20)), Some(Type::I32));
        let (_, sum) = func.push_inst(b0, Instruction::Add(c10.unwrap(), c20.unwrap()), Some(Type::I32));
        func.push_inst(b0, Instruction::Ret(Some(sum.unwrap())), None);

        let optimized = optimize_function(&func);
        let printed = optimized.to_string();
        // The add should be folded into const.int 30, and c10 and c20 should be eliminated by DCE!
        assert!(printed.contains("const.int 30"));
        assert!(!printed.contains("add"));
    }

    #[test]
    fn test_cse() {
        let mut func = Function::new("test_cse", Type::I32);
        let p0 = func.add_param("a", Type::I32);
        let p1 = func.add_param("b", Type::I32);
        let b0 = func.create_block("entry");
        let (_, sum1) = func.push_inst(b0, Instruction::Add(p0, p1), Some(Type::I32));
        let (_, sum2) = func.push_inst(b0, Instruction::Add(p0, p1), Some(Type::I32));
        let (_, final_add) = func.push_inst(b0, Instruction::Add(sum1.unwrap(), sum2.unwrap()), Some(Type::I32));
        func.push_inst(b0, Instruction::Ret(Some(final_add.unwrap())), None);

        let optimized = optimize_function(&func);
        let printed = optimized.to_string();
        // sum2 should be deduplicated to sum1 via CSE
        assert_eq!(printed.matches("add %a, %b").count(), 1);
    }
}
