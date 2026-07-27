use crate::arena::{Arena, Block, Inst, Value};
use crate::inst::Instruction;
use crate::types::Type;

/// Where an SSA value originated from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueDef {
    /// A function parameter (index into `Function::params`).
    Param(usize),
    /// The result of executing an instruction.
    Inst(Inst),
}

/// Metadata for an SSA value in a function.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ValueData {
    pub ty: Type,
    pub def: ValueDef,
    pub name: Option<String>,
}

/// A basic block containing a sequence of instructions.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockData {
    pub name: String,
    pub insts: Vec<Inst>,
}

impl BlockData {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            insts: Vec::new(),
        }
    }
}

/// Representation of a single function in DQIR.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Function {
    pub name: String,
    /// Parameter list: (parameter name, value ID, type).
    pub params: Vec<(String, Value, Type)>,
    pub return_type: Type,
    pub blocks: Arena<Block, BlockData>,
    pub insts: Arena<Inst, Instruction>,
    pub values: Arena<Value, ValueData>,
    pub entry_block: Option<Block>,
}

impl Function {
    pub fn new(name: impl Into<String>, return_type: Type) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            return_type,
            blocks: Arena::new(),
            insts: Arena::new(),
            values: Arena::new(),
            entry_block: None,
        }
    }

    pub fn add_param(&mut self, name: impl Into<String>, ty: Type) -> Value {
        let name_str = name.into();
        let param_idx = self.params.len();
        let val = self.values.push(ValueData {
            ty,
            def: ValueDef::Param(param_idx),
            name: Some(name_str.clone()),
        });
        self.params.push((name_str, val, ty));
        val
    }

    pub fn create_block(&mut self, name: impl Into<String>) -> Block {
        let block = self.blocks.push(BlockData::new(name));
        if self.entry_block.is_none() {
            self.entry_block = Some(block);
        }
        block
    }

    pub fn create_value(&mut self, ty: Type, inst: Inst) -> Value {
        self.values.push(ValueData {
            ty,
            def: ValueDef::Inst(inst),
            name: None,
        })
    }

    pub fn set_value_name(&mut self, val: Value, name: impl Into<String>) {
        if let Some(vdata) = self.values.get_mut(val) {
            vdata.name = Some(name.into());
        }
    }

    pub fn push_inst(
        &mut self,
        block: Block,
        inst: Instruction,
        result_type: Option<Type>,
    ) -> (Inst, Option<Value>) {
        let inst_id = self.insts.push(inst);
        self.blocks[block].insts.push(inst_id);
        let val_id = result_type.map(|ty| self.create_value(ty, inst_id));
        (inst_id, val_id)
    }

    pub fn block_by_name(&self, name: &str) -> Option<Block> {
        self.blocks
            .iter()
            .find(|(_, b)| b.name == name)
            .map(|(id, _)| id)
    }

    pub fn param_by_name(&self, name: &str) -> Option<Value> {
        self.params
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, val, _)| *val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_creation() {
        let mut func = Function::new("test", Type::I32);
        let p0 = func.add_param("a", Type::I32);
        let p1 = func.add_param("b", Type::I32);
        let b0 = func.create_block("entry");
        assert_eq!(func.entry_block, Some(b0));

        let (_, res) = func.push_inst(
            b0,
            Instruction::Add(p0, p1),
            Some(Type::I32),
        );
        let res = res.expect("should have result value");

        let (_, _) = func.push_inst(b0, Instruction::Ret(Some(res)), None);

        assert_eq!(func.blocks[b0].insts.len(), 2);
        assert_eq!(func.values[res].ty, Type::I32);
    }
}
