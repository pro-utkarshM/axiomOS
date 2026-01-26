pub mod helpers;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::bytecode::program::BpfProgram;
use kernel_bpf::execution::{BpfContext, BpfError, BpfExecutor, Interpreter};
use kernel_bpf::loader::BpfLoader;
use kernel_bpf::profile::ActiveProfile;

pub struct BpfManager {
    programs: Vec<BpfProgram<ActiveProfile>>,
    attachments: BTreeMap<u32, Vec<u32>>,
}

impl Default for BpfManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BpfManager {
    pub fn new() -> Self {
        Self {
            programs: Vec::new(),
            attachments: BTreeMap::new(),
        }
    }

    pub fn load_program(&mut self, elf_bytes: &[u8]) -> Result<u32, BpfError> {
        let mut loader = BpfLoader::<ActiveProfile>::new();
        let obj = loader.load(elf_bytes).map_err(|_| BpfError::NotLoaded)?;

        if let Some(loaded_prog) = obj.programs().first() {
            let bpf_prog = BpfProgram::new(
                loaded_prog.prog_type(),
                loaded_prog.insns().to_vec(),
                0, // TODO: Calculate stack usage via Verifier
            )
            .map_err(|_| BpfError::InvalidInstruction)?;

            let id = self.programs.len() as u32;
            self.programs.push(bpf_prog);
            Ok(id)
        } else {
            Err(BpfError::NotLoaded)
        }
    }

    pub fn load_raw_program(&mut self, insns: Vec<BpfInsn>) -> Result<u32, BpfError> {
        let bpf_prog =
            BpfProgram::new(kernel_bpf::bytecode::program::BpfProgType::Unspec, insns, 0)
                .map_err(|_| BpfError::InvalidInstruction)?;

        let id = self.programs.len() as u32;
        self.programs.push(bpf_prog);
        Ok(id)
    }

    pub fn attach(&mut self, attach_type: u32, prog_id: u32) -> Result<(), BpfError> {
        if prog_id as usize >= self.programs.len() {
            return Err(BpfError::NotLoaded);
        }

        let list = self.attachments.entry(attach_type).or_default();
        if !list.contains(&prog_id) {
            list.push(prog_id);
        }
        Ok(())
    }

    pub fn execute(&self, program_id: u32, ctx: &BpfContext) -> Result<u64, BpfError> {
        let program = self
            .programs
            .get(program_id as usize)
            .ok_or(BpfError::NotLoaded)?;

        let interpreter = Interpreter::<ActiveProfile>::new();
        interpreter.execute(program, ctx)
    }

    pub fn execute_hooks(&self, attach_type: u32, ctx: &BpfContext) {
        if let Some(progs) = self.attachments.get(&attach_type) {
            for prog_id in progs {
                // For now, ignore errors from hooks
                let _ = self.execute(*prog_id, ctx);
            }
        }
    }
}
