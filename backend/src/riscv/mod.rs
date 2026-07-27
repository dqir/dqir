pub mod emit;
pub mod inst;
pub mod regalloc;
pub mod select;

pub use emit::emit_machine_code;
pub use inst::{MInst, Reg};
pub use regalloc::{allocate_registers, Allocation, RegAllocResult};
pub use select::{lower_function, lower_function_with_regalloc, schedule_instructions, LoweredFunction};
