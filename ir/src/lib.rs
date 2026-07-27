pub mod arena;
pub mod function;
pub mod inst;
pub mod lexer;
pub mod module;
pub mod opt;
pub mod parser;
pub mod printer;
pub mod types;

pub use arena::{Arena, Block, EntityId, FuncId, Inst, Value};
pub use function::{BlockData, Function, ValueData, ValueDef};
pub use inst::{CmpOp, Immediate, Instruction};
pub use lexer::{LexError, Spanned, Token};
pub use module::{ExternalDecl, Module};
pub use opt::optimize_function;
pub use parser::{parse, ParseError, Parser};
pub use printer::{format_block, format_inst, format_val};
pub use types::Type;
