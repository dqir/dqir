use memmap2::MmapMut;

/// Memory-mapped JIT execution buffer allocated with mmap (read, write, exec).
pub struct JitMemory {
    exec_map: memmap2::Mmap,
    pub size: usize,
}

impl JitMemory {
    pub fn new(code: &[u8]) -> Result<Self, String> {
        let size = code.len().max(4096);
        let mut mut_map = MmapMut::map_anon(size)
            .map_err(|e| format!("Failed to allocate anonymous memory: {}", e))?;
        mut_map[..code.len()].copy_from_slice(code);
        let exec_map = mut_map
            .make_exec()
            .map_err(|e| format!("Failed to make memory executable: {}", e))?;
        Ok(Self {
            exec_map,
            size: code.len(),
        })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.exec_map.as_ptr()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.exec_map[..self.size]
    }
}

/// A compiled executable function ready to be invoked.
pub struct Executable {
    pub name: String,
    pub jit_mem: JitMemory,
}

impl Executable {
    pub fn new(name: impl Into<String>, code: &[u8]) -> Result<Self, String> {
        let jit_mem = JitMemory::new(code)?;
        Ok(Self {
            name: name.into(),
            jit_mem,
        })
    }

    /// Run the compiled code with given arguments.
    /// On native riscv64 targets, performs a direct JIT function call (mmap + call).
    /// On cross-compilation hosts (such as x86_64 Linux), runs the emitted RV64GC bytes in the embedded VM.
    pub fn run(&self, args: &[u64]) -> u64 {
        #[cfg(target_arch = "riscv64")]
        {
            unsafe {
                let fn_ptr: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 =
                    std::mem::transmute(self.jit_mem.as_ptr());
                let a0 = *args.get(0).unwrap_or(&0);
                let a1 = *args.get(1).unwrap_or(&0);
                let a2 = *args.get(2).unwrap_or(&0);
                let a3 = *args.get(3).unwrap_or(&0);
                let a4 = *args.get(4).unwrap_or(&0);
                let a5 = *args.get(5).unwrap_or(&0);
                let a6 = *args.get(6).unwrap_or(&0);
                let a7 = *args.get(7).unwrap_or(&0);
                fn_ptr(a0, a1, a2, a3, a4, a5, a6, a7)
            }
        }
        #[cfg(not(target_arch = "riscv64"))]
        {
            let mut vm = crate::vm::RiscvVm::new(self.jit_mem.as_slice());
            vm.run(args)
        }
    }
}
