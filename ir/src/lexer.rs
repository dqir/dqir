use std::fmt;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    // Keywords
    Func,
    Extern,
    ConstInt,
    ConstFloat,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Neg,
    Not,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Cmp,
    Alloca,
    Load,
    Store,
    Jmp,
    Br,
    Ret,
    Call,
    Phi,

    // Identifiers and Names
    Ident(String),
    FuncName(String),
    ValueName(String),

    // Literals
    Int(i64),
    Float(f64),

    // Punctuation
    LBrace,   // {
    RBrace,   // }
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    Colon,    // :
    Equal,    // =
    Comma,    // ,
    Arrow,    // ->
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Func => write!(f, "func"),
            Token::Extern => write!(f, "extern"),
            Token::ConstInt => write!(f, "const.int"),
            Token::ConstFloat => write!(f, "const.float"),
            Token::Add => write!(f, "add"),
            Token::Sub => write!(f, "sub"),
            Token::Mul => write!(f, "mul"),
            Token::Div => write!(f, "div"),
            Token::Rem => write!(f, "rem"),
            Token::Neg => write!(f, "neg"),
            Token::Not => write!(f, "not"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Xor => write!(f, "xor"),
            Token::Shl => write!(f, "shl"),
            Token::Shr => write!(f, "shr"),
            Token::Sar => write!(f, "sar"),
            Token::Cmp => write!(f, "cmp"),
            Token::Alloca => write!(f, "alloca"),
            Token::Load => write!(f, "load"),
            Token::Store => write!(f, "store"),
            Token::Jmp => write!(f, "jmp"),
            Token::Br => write!(f, "br"),
            Token::Ret => write!(f, "ret"),
            Token::Call => write!(f, "call"),
            Token::Phi => write!(f, "phi"),
            Token::Ident(s) => write!(f, "{}", s),
            Token::FuncName(s) => write!(f, "@{}", s),
            Token::ValueName(s) => write!(f, "%{}", s),
            Token::Int(i) => write!(f, "{}", i),
            Token::Float(fl) => write!(f, "{}", fl),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Colon => write!(f, ":"),
            Token::Equal => write!(f, "="),
            Token::Comma => write!(f, ","),
            Token::Arrow => write!(f, "->"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Spanned<T> {
    pub value: T,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lex error at line {}, col {}: {}",
            self.line, self.col, self.message
        )
    }
}

pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos] as char)
        } else {
            None
        }
    }

    fn next_char(&mut self) -> Option<char> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos] as char;
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.next_char();
            } else if ch == ';' || (ch == '/' && self.pos + 1 < self.input.len() && self.input[self.pos + 1] == b'/') {
                while let Some(c) = self.peek_char() {
                    if c == '\n' {
                        break;
                    }
                    self.next_char();
                }
            } else {
                break;
            }
        }
    }

    fn lex_number(&mut self, start_line: usize, start_col: usize, negative: bool) -> Result<Spanned<Token>, LexError> {
        let mut num_str = String::new();
        if negative {
            num_str.push('-');
        }
        let mut has_dot = false;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.next_char();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                num_str.push(ch);
                self.next_char();
            } else {
                break;
            }
        }

        if has_dot {
            let fl: f64 = num_str.parse().map_err(|e| LexError {
                message: format!("Invalid float literal '{}': {}", num_str, e),
                line: start_line,
                col: start_col,
            })?;
            Ok(Spanned {
                value: Token::Float(fl),
                line: start_line,
                col: start_col,
            })
        } else {
            let i: i64 = num_str.parse().map_err(|e| LexError {
                message: format!("Invalid int literal '{}': {}", num_str, e),
                line: start_line,
                col: start_col,
            })?;
            Ok(Spanned {
                value: Token::Int(i),
                line: start_line,
                col: start_col,
            })
        }
    }

    fn lex_word(&mut self, start_line: usize, start_col: usize) -> Spanned<Token> {
        let mut word = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-' {
                word.push(ch);
                self.next_char();
            } else {
                break;
            }
        }

        let token = match word.as_str() {
            "func" => Token::Func,
            "extern" => Token::Extern,
            "const.int" => Token::ConstInt,
            "const.float" => Token::ConstFloat,
            "add" => Token::Add,
            "sub" => Token::Sub,
            "mul" => Token::Mul,
            "div" => Token::Div,
            "rem" => Token::Rem,
            "neg" => Token::Neg,
            "not" => Token::Not,
            "and" => Token::And,
            "or" => Token::Or,
            "xor" => Token::Xor,
            "shl" => Token::Shl,
            "shr" => Token::Shr,
            "sar" => Token::Sar,
            "cmp" => Token::Cmp,
            "alloca" => Token::Alloca,
            "load" => Token::Load,
            "store" => Token::Store,
            "jmp" => Token::Jmp,
            "br" => Token::Br,
            "ret" => Token::Ret,
            "call" => Token::Call,
            "phi" => Token::Phi,
            _ => Token::Ident(word),
        };

        Spanned {
            value: token,
            line: start_line,
            col: start_col,
        }
    }

    fn lex_prefixed_name(&mut self, start_line: usize, start_col: usize, is_func: bool) -> Result<Spanned<Token>, LexError> {
        let mut name = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-' {
                name.push(ch);
                self.next_char();
            } else {
                break;
            }
        }

        if name.is_empty() {
            return Err(LexError {
                message: "Expected identifier after prefix symbol".to_string(),
                line: start_line,
                col: start_col,
            });
        }

        let token = if is_func {
            Token::FuncName(name)
        } else {
            Token::ValueName(name)
        };

        Ok(Spanned {
            value: token,
            line: start_line,
            col: start_col,
        })
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned<Token>>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let start_line = self.line;
            let start_col = self.col;

            let Some(ch) = self.peek_char() else {
                break;
            };

            let token = match ch {
                '{' => {
                    self.next_char();
                    Token::LBrace
                }
                '}' => {
                    self.next_char();
                    Token::RBrace
                }
                '(' => {
                    self.next_char();
                    Token::LParen
                }
                ')' => {
                    self.next_char();
                    Token::RParen
                }
                '[' => {
                    self.next_char();
                    Token::LBracket
                }
                ']' => {
                    self.next_char();
                    Token::RBracket
                }
                ':' => {
                    self.next_char();
                    Token::Colon
                }
                '=' => {
                    self.next_char();
                    Token::Equal
                }
                ',' => {
                    self.next_char();
                    Token::Comma
                }
                '-' => {
                    self.next_char();
                    if let Some('>') = self.peek_char() {
                        self.next_char();
                        Token::Arrow
                    } else if let Some(next_ch) = self.peek_char() {
                        if next_ch.is_ascii_digit() {
                            let sp = self.lex_number(start_line, start_col, true)?;
                            tokens.push(sp);
                            continue;
                        } else {
                            return Err(LexError {
                                message: format!("Unexpected character after '-': '{}'", next_ch),
                                line: start_line,
                                col: start_col,
                            });
                        }
                    } else {
                        return Err(LexError {
                            message: "Unexpected end of input after '-'".to_string(),
                            line: start_line,
                            col: start_col,
                        });
                    }
                }
                '@' => {
                    self.next_char();
                    let sp = self.lex_prefixed_name(start_line, start_col, true)?;
                    tokens.push(sp);
                    continue;
                }
                '%' => {
                    self.next_char();
                    let sp = self.lex_prefixed_name(start_line, start_col, false)?;
                    tokens.push(sp);
                    continue;
                }
                c if c.is_ascii_digit() => {
                    let sp = self.lex_number(start_line, start_col, false)?;
                    tokens.push(sp);
                    continue;
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    tokens.push(self.lex_word(start_line, start_col));
                    continue;
                }
                _ => {
                    return Err(LexError {
                        message: format!("Unexpected character: '{}'", ch),
                        line: start_line,
                        col: start_col,
                    });
                }
            };

            tokens.push(Spanned {
                value: token,
                line: start_line,
                col: start_col,
            });
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let input = "func @main(%0: i32) -> i32 {\nentry:\n  %1 = const.int -42\n  ret %1\n}";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().expect("should tokenize cleanly");
        assert_eq!(tokens[0].value, Token::Func);
        assert_eq!(tokens[1].value, Token::FuncName("main".to_string()));
        assert_eq!(tokens[2].value, Token::LParen);
        assert_eq!(tokens[3].value, Token::ValueName("0".to_string()));
    }
}
