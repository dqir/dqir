use std::collections::HashMap;
use std::str::FromStr;
use crate::arena::{Block, Value};
use crate::function::{Function, ValueData, ValueDef};
use crate::inst::{CmpOp, Immediate, Instruction};
use crate::lexer::{LexError, Spanned, Token};
use crate::module::Module;
use crate::types::Type;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        Self {
            message: e.message,
            line: e.line,
            col: e.col,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at line {}, col {}: {}", self.line, self.col, self.message)
    }
}

pub struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned<Token>]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&'a Spanned<Token>> {
        self.tokens.get(self.pos)
    }

    fn next_token(&mut self) -> Option<&'a Spanned<Token>> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn current_span(&self) -> (usize, usize) {
        if let Some(tok) = self.peek() {
            (tok.line, tok.col)
        } else if let Some(last) = self.tokens.last() {
            (last.line, last.col)
        } else {
            (1, 1)
        }
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let (line, col) = self.current_span();
        ParseError {
            message: message.into(),
            line,
            col,
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<&'a Spanned<Token>, ParseError> {
        let (line, col) = self.current_span();
        if let Some(tok) = self.next_token() {
            if tok.value == expected {
                Ok(tok)
            } else {
                Err(ParseError {
                    message: format!("Expected '{}', found '{}'", expected, tok.value),
                    line: tok.line,
                    col: tok.col,
                })
            }
        } else {
            Err(ParseError {
                message: format!("Expected '{}', found end of file", expected),
                line,
                col,
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let (line, col) = self.current_span();
        if let Some(tok) = self.next_token() {
            if let Token::Ident(ref s) = tok.value {
                Ok(s.clone())
            } else {
                Err(ParseError {
                    message: format!("Expected identifier, found '{}'", tok.value),
                    line: tok.line,
                    col: tok.col,
                })
            }
        } else {
            Err(ParseError {
                message: "Expected identifier, found end of file".to_string(),
                line,
                col,
            })
        }
    }

    fn expect_func_name(&mut self) -> Result<String, ParseError> {
        let (line, col) = self.current_span();
        if let Some(tok) = self.next_token() {
            if let Token::FuncName(ref s) = tok.value {
                Ok(s.clone())
            } else {
                Err(ParseError {
                    message: format!("Expected @func_name, found '{}'", tok.value),
                    line: tok.line,
                    col: tok.col,
                })
            }
        } else {
            Err(ParseError {
                message: "Expected @func_name, found end of file".to_string(),
                line,
                col,
            })
        }
    }

    fn expect_value_name(&mut self) -> Result<String, ParseError> {
        let (line, col) = self.current_span();
        if let Some(tok) = self.next_token() {
            if let Token::ValueName(ref s) = tok.value {
                Ok(s.clone())
            } else {
                Err(ParseError {
                    message: format!("Expected %value_name, found '{}'", tok.value),
                    line: tok.line,
                    col: tok.col,
                })
            }
        } else {
            Err(ParseError {
                message: "Expected %value_name, found end of file".to_string(),
                line,
                col,
            })
        }
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let ident = self.expect_ident()?;
        Type::from_str(&ident).map_err(|e| self.error(e))
    }

    fn parse_cmp_op(&mut self) -> Result<CmpOp, ParseError> {
        let ident = self.expect_ident()?;
        CmpOp::from_str(&ident).map_err(|e| self.error(e))
    }

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut module = Module::new();
        while let Some(tok) = self.peek() {
            match tok.value {
                Token::Extern => {
                    self.parse_extern(&mut module)?;
                }
                Token::Func => {
                    self.parse_func(&mut module)?;
                }
                _ => {
                    return Err(self.error(format!("Expected 'extern' or 'func', found '{}'", tok.value)));
                }
            }
        }
        Ok(module)
    }

    fn parse_extern(&mut self, module: &mut Module) -> Result<(), ParseError> {
        self.expect_token(Token::Extern)?;
        self.expect_token(Token::Func)?;
        let name = self.expect_func_name()?;
        self.expect_token(Token::LParen)?;
        let mut params = Vec::new();
        while let Some(tok) = self.peek() {
            if tok.value == Token::RParen {
                break;
            }
            params.push(self.parse_type()?);
            if let Some(next) = self.peek() {
                if next.value == Token::Comma {
                    self.next_token();
                } else {
                    break;
                }
            }
        }
        self.expect_token(Token::RParen)?;
        self.expect_token(Token::Arrow)?;
        let return_type = self.parse_type()?;
        module.add_extern(name, params, return_type);
        Ok(())
    }

    fn get_or_create_val(
        func: &mut Function,
        val_map: &mut HashMap<String, Value>,
        name: &str,
    ) -> Value {
        if let Some(&val) = val_map.get(name) {
            val
        } else {
            let val = func.values.push(ValueData {
                ty: Type::Void, // temporary placeholder type
                def: ValueDef::Param(usize::MAX), // placeholder def
                name: Some(name.to_string()),
            });
            val_map.insert(name.to_string(), val);
            val
        }
    }

    fn parse_value_ref(
        &mut self,
        func: &mut Function,
        val_map: &mut HashMap<String, Value>,
    ) -> Result<Value, ParseError> {
        let name = self.expect_value_name()?;
        Ok(Self::get_or_create_val(func, val_map, &name))
    }

    fn parse_block_ref(&mut self, block_map: &HashMap<String, Block>) -> Result<Block, ParseError> {
        let tok = self.next_token().ok_or_else(|| self.error("Expected block name, found end of file"))?;
        let name = match &tok.value {
            Token::Ident(s) => s.clone(),
            other => other.to_string(),
        };
        block_map.get(&name).copied().ok_or_else(|| {
            ParseError {
                message: format!("Unknown basic block reference: '{}'", name),
                line: tok.line,
                col: tok.col,
            }
        })
    }

    fn parse_func(&mut self, module: &mut Module) -> Result<(), ParseError> {
        self.expect_token(Token::Func)?;
        let name = self.expect_func_name()?;
        let mut func = Function::new(name, Type::Void);
        let mut val_map: HashMap<String, Value> = HashMap::new();

        self.expect_token(Token::LParen)?;
        while let Some(tok) = self.peek() {
            if tok.value == Token::RParen {
                break;
            }
            let param_name = self.expect_value_name()?;
            self.expect_token(Token::Colon)?;
            let ty = self.parse_type()?;
            let val = func.add_param(&param_name, ty);
            val_map.insert(param_name, val);

            if let Some(next) = self.peek() {
                if next.value == Token::Comma {
                    self.next_token();
                } else {
                    break;
                }
            }
        }
        self.expect_token(Token::RParen)?;
        self.expect_token(Token::Arrow)?;
        let return_type = self.parse_type()?;
        func.return_type = return_type;

        self.expect_token(Token::LBrace)?;

        // Scan block labels inside { ... } to create all basic blocks in source order
        let mut block_map: HashMap<String, Block> = HashMap::new();
        let mut scan_pos = self.pos;
        let mut brace_depth = 1;
        while scan_pos < self.tokens.len() && brace_depth > 0 {
            let tok = &self.tokens[scan_pos];
            match tok.value {
                Token::LBrace => brace_depth += 1,
                Token::RBrace => brace_depth -= 1,
                Token::Ident(ref name)
                    if scan_pos + 1 < self.tokens.len()
                        && self.tokens[scan_pos + 1].value == Token::Colon
                        && !block_map.contains_key(name) =>
                {
                    let blk = func.create_block(name);
                    block_map.insert(name.clone(), blk);
                }
                _ => {}
            }
            scan_pos += 1;
        }

        
        let mut current_block: Option<Block> = None;

        while let Some(tok) = self.peek() {
            if tok.value == Token::RBrace {
                self.next_token();
                break;
            }

            // Check if this is a block label
            if let Token::Ident(ref name) = tok.value
                && self.pos + 1 < self.tokens.len()
                && self.tokens[self.pos + 1].value == Token::Colon
            {
                let block_name = name.clone();
                self.next_token();
                self.next_token();
                current_block = Some(block_map[&block_name]);
                continue;
            }

            let block = current_block.ok_or_else(|| self.error("Instruction found outside of any basic block"))?;

            // Check if instruction defines a result value (%val : ty = ...)
            let mut result_val_info: Option<(String, Type)> = None;
            if let Some(Token::ValueName(_)) = self.peek().map(|t| &t.value) {
                let val_name = self.expect_value_name()?;
                self.expect_token(Token::Colon)?;
                let ty = self.parse_type()?;
                self.expect_token(Token::Equal)?;
                result_val_info = Some((val_name, ty));
            }

            let inst = self.parse_instruction(&mut func, &mut val_map, &block_map)?;

            let inst_id = func.insts.push(inst);
            func.blocks[block].insts.push(inst_id);

            if let Some((val_name, ty)) = result_val_info {
                if let Some(&existing_id) = val_map.get(&val_name) {
                    func.values[existing_id].ty = ty;
                    func.values[existing_id].def = ValueDef::Inst(inst_id);
                } else {
                    let new_id = func.create_value(ty, inst_id);
                    func.set_value_name(new_id, &val_name);
                    val_map.insert(val_name, new_id);
                }
            }
        }

        // Validate that no unresolved placeholder values remain
        for (_id, vdata) in func.values.iter() {
            if vdata.def == ValueDef::Param(usize::MAX) {
                return Err(self.error(format!(
                    "Undefined SSA value reference '%{}' in function '@{}'",
                    vdata.name.as_deref().unwrap_or("?"),
                    func.name
                )));
            }
        }

        module.add_function(func);
        Ok(())
    }

    fn parse_instruction(
        &mut self,
        func: &mut Function,
        val_map: &mut HashMap<String, Value>,
        block_map: &HashMap<String, Block>,
    ) -> Result<Instruction, ParseError> {
        let tok = self.next_token().ok_or_else(|| self.error("Expected instruction opcode, found end of file"))?;
        match tok.value {
            Token::ConstInt => {
                let next = self.next_token().ok_or_else(|| self.error("Expected integer literal after const.int"))?;
                if let Token::Int(i) = next.value {
                    Ok(Instruction::Const(Immediate::I64(i)))
                } else {
                    Err(self.error(format!("Expected integer literal, found '{}'", next.value)))
                }
            }
            Token::ConstFloat => {
                let next = self.next_token().ok_or_else(|| self.error("Expected float literal after const.float"))?;
                match next.value {
                    Token::Float(fl) => Ok(Instruction::Const(Immediate::F64(fl.to_bits()))),
                    Token::Int(i) => Ok(Instruction::Const(Immediate::F64((i as f64).to_bits()))),
                    _ => Err(self.error(format!("Expected float literal, found '{}'", next.value))),
                }
            }
            Token::Add => self.parse_binary_op(func, val_map, Instruction::Add),
            Token::Sub => self.parse_binary_op(func, val_map, Instruction::Sub),
            Token::Mul => self.parse_binary_op(func, val_map, Instruction::Mul),
            Token::Div => self.parse_binary_op(func, val_map, Instruction::Div),
            Token::Rem => self.parse_binary_op(func, val_map, Instruction::Rem),
            Token::And => self.parse_binary_op(func, val_map, Instruction::And),
            Token::Or => self.parse_binary_op(func, val_map, Instruction::Or),
            Token::Xor => self.parse_binary_op(func, val_map, Instruction::Xor),
            Token::Shl => self.parse_binary_op(func, val_map, Instruction::Shl),
            Token::Shr => self.parse_binary_op(func, val_map, Instruction::Shr),
            Token::Sar => self.parse_binary_op(func, val_map, Instruction::Sar),
            Token::Neg => {
                let a = self.parse_value_ref(func, val_map)?;
                Ok(Instruction::Neg(a))
            }
            Token::Not => {
                let a = self.parse_value_ref(func, val_map)?;
                Ok(Instruction::Not(a))
            }
            Token::Cmp => {
                let op = self.parse_cmp_op()?;
                let a = self.parse_value_ref(func, val_map)?;
                self.expect_token(Token::Comma)?;
                let b = self.parse_value_ref(func, val_map)?;
                Ok(Instruction::Cmp(op, a, b))
            }
            Token::Alloca => {
                let ty = self.parse_type()?;
                Ok(Instruction::Alloca(ty))
            }
            Token::Load => {
                let ty = self.parse_type()?;
                self.expect_token(Token::Comma)?;
                let ptr = self.parse_value_ref(func, val_map)?;
                self.expect_token(Token::Comma)?;
                let offset_tok = self.next_token().ok_or_else(|| self.error("Expected offset in load"))?;
                if let Token::Int(offset) = offset_tok.value {
                    Ok(Instruction::Load(ty, ptr, offset as i32))
                } else {
                    Err(self.error("Expected integer offset in load"))
                }
            }
            Token::Store => {
                let val = self.parse_value_ref(func, val_map)?;
                self.expect_token(Token::Comma)?;
                let ptr = self.parse_value_ref(func, val_map)?;
                self.expect_token(Token::Comma)?;
                let offset_tok = self.next_token().ok_or_else(|| self.error("Expected offset in store"))?;
                if let Token::Int(offset) = offset_tok.value {
                    Ok(Instruction::Store(val, ptr, offset as i32))
                } else {
                    Err(self.error("Expected integer offset in store"))
                }
            }
            Token::Jmp => {
                let blk = self.parse_block_ref(block_map)?;
                Ok(Instruction::Jmp(blk))
            }
            Token::Br => {
                let cond = self.parse_value_ref(func, val_map)?;
                self.expect_token(Token::Comma)?;
                let t_blk = self.parse_block_ref(block_map)?;
                self.expect_token(Token::Comma)?;
                let f_blk = self.parse_block_ref(block_map)?;
                Ok(Instruction::Br(cond, t_blk, f_blk))
            }
            Token::Ret => {
                if let Some(Token::ValueName(_)) = self.peek().map(|t| &t.value) {
                    let val = self.parse_value_ref(func, val_map)?;
                    Ok(Instruction::Ret(Some(val)))
                } else {
                    Ok(Instruction::Ret(None))
                }
            }
            Token::Call => {
                let target = self.expect_func_name()?;
                self.expect_token(Token::LParen)?;
                let mut args = Vec::new();
                while let Some(t) = self.peek() {
                    if t.value == Token::RParen {
                        break;
                    }
                    args.push(self.parse_value_ref(func, val_map)?);
                    if let Some(next) = self.peek() {
                        if next.value == Token::Comma {
                            self.next_token();
                        } else {
                            break;
                        }
                    }
                }
                self.expect_token(Token::RParen)?;
                Ok(Instruction::Call(target, args))
            }
            Token::Phi => {
                let mut pairs = Vec::new();
                while let Some(t) = self.peek() {
                    if t.value != Token::LBracket {
                        break;
                    }
                    self.next_token();
                    let val = self.parse_value_ref(func, val_map)?;
                    self.expect_token(Token::Comma)?;
                    let blk = self.parse_block_ref(block_map)?;
                    self.expect_token(Token::RBracket)?;
                    pairs.push((blk, val));
                    if let Some(next) = self.peek() {
                        if next.value == Token::Comma {
                            self.next_token();
                        } else {
                            break;
                        }
                    }
                }
                Ok(Instruction::Phi(pairs))
            }
            _ => Err(self.error(format!("Expected instruction opcode, found '{}'", tok.value))),
        }
    }

    fn parse_binary_op<F>(
        &mut self,
        func: &mut Function,
        val_map: &mut HashMap<String, Value>,
        constructor: F,
    ) -> Result<Instruction, ParseError>
    where
        F: FnOnce(Value, Value) -> Instruction,
    {
        let a = self.parse_value_ref(func, val_map)?;
        self.expect_token(Token::Comma)?;
        let b = self.parse_value_ref(func, val_map)?;
        Ok(constructor(a, b))
    }
}

pub fn parse(input: &str) -> Result<Module, ParseError> {
    let mut lexer = crate::lexer::Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(&tokens);
    parser.parse_module()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_func() {
        let input = "func @add(%a: i32, %b: i32) -> i32 {\nentry:\n    %sum: i32 = add %a, %b\n    ret %sum\n}";
        let module = parse(input).expect("should parse cleanly");
        assert_eq!(module.functions.len(), 1);
        let func = module.get_function_by_name("add").unwrap();
        assert_eq!(func.params.len(), 2);
        assert_eq!(func.blocks.len(), 1);
        let printed = func.to_string();
        assert_eq!(printed, input);
    }
}
