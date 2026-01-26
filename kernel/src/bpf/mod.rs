pub mod helpers;

use alloc::vec::Vec;

use kernel_bpf::loader::BpfLoader;
use kernel_bpf::bytecode::program::BpfProgram;
use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::profile::ActiveProfile;
use kernel_bpf::execution::Interpreter;
use kernel_bpf::execution::{BpfError, BpfContext, BpfExecutor};

pub struct BpfManager {
    programs: Vec<BpfProgram<ActiveProfile>>,
}

impl BpfManager {
    pub fn new() -> Self {
        Self {
            programs: Vec::new(),
        }
    }

    pub fn load_program(&mut self, elf_bytes: &[u8]) -> Result<u32, BpfError> {
        let mut loader = BpfLoader::<ActiveProfile>::new();
        let obj = loader.load(elf_bytes).map_err(|_| BpfError::NotLoaded)?;
        
        if let Some(loaded_prog) = obj.programs().first() {
             let bpf_prog = BpfProgram::new(
                 loaded_prog.prog_type(),
                 loaded_prog.insns().to_vec(),
                 0 // TODO: Calculate stack usage via Verifier
             ).map_err(|_| BpfError::InvalidInstruction)?; 
             
             let id = self.programs.len() as u32;
             self.programs.push(bpf_prog);
             Ok(id)
        } else {
            Err(BpfError::NotLoaded)
        }
    }

    pub fn load_raw_program(&mut self, insns: Vec<BpfInsn>) -> Result<u32, BpfError> {
        let bpf_prog = BpfProgram::new(
            kernel_bpf::bytecode::program::BpfProgType::Unspec,
            insns,
            0 
        ).map_err(|_| BpfError::InvalidInstruction)?;

        let id = self.programs.len() as u32;
        self.programs.push(bpf_prog);
        Ok(id)
    }

    pub fn execute(&self, program_id: u32, ctx: &BpfContext) -> Result<u64, BpfError> {
        let program = self.programs.get(program_id as usize).ok_or(BpfError::NotLoaded)?;
        
        let interpreter = Interpreter::<ActiveProfile>::new();
        interpreter.execute(program, ctx)
    }
}
