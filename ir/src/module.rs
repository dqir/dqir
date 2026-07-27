use std::collections::HashMap;
use crate::arena::{Arena, FuncId};
use crate::function::Function;
use crate::types::Type;

/// Declaration of an external function (e.g., standard C library functions or host runtime runtime imports).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ExternalDecl {
    pub name: String,
    pub params: Vec<Type>,
    pub return_type: Type,
}

impl ExternalDecl {
    pub fn new(name: impl Into<String>, params: Vec<Type>, return_type: Type) -> Self {
        Self {
            name: name.into(),
            params,
            return_type,
        }
    }
}

/// Representation of a compilation module containing external declarations and defined functions.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Module {
    pub functions: Arena<FuncId, Function>,
    pub func_names: HashMap<String, FuncId>,
    pub externs: Vec<ExternalDecl>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            functions: Arena::new(),
            func_names: HashMap::new(),
            externs: Vec::new(),
        }
    }

    pub fn add_extern(
        &mut self,
        name: impl Into<String>,
        params: Vec<Type>,
        return_type: Type,
    ) {
        self.externs.push(ExternalDecl::new(name, params, return_type));
    }

    pub fn add_function(&mut self, func: Function) -> FuncId {
        let name = func.name.clone();
        let id = self.functions.push(func);
        self.func_names.insert(name, id);
        id
    }

    pub fn get_function(&self, id: FuncId) -> Option<&Function> {
        self.functions.get(id)
    }

    pub fn get_function_mut(&mut self, id: FuncId) -> Option<&mut Function> {
        self.functions.get_mut(id)
    }

    pub fn get_function_by_name(&self, name: &str) -> Option<&Function> {
        self.func_names.get(name).and_then(|id| self.functions.get(*id))
    }

    pub fn get_function_id_by_name(&self, name: &str) -> Option<FuncId> {
        self.func_names.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Type;

    #[test]
    fn test_module() {
        let mut mod_ir = Module::new();
        mod_ir.add_extern("puts", vec![Type::Ptr], Type::I32);
        assert_eq!(mod_ir.externs.len(), 1);

        let func = Function::new("main", Type::I32);
        let fid = mod_ir.add_function(func);
        assert!(mod_ir.get_function(fid).is_some());
        assert_eq!(mod_ir.get_function_id_by_name("main"), Some(fid));
    }
}
