pub mod helpers;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::bytecode::program::BpfProgram;
use kernel_bpf::execution::{BpfContext, BpfError, BpfExecutor, Interpreter};
use kernel_bpf::loader::BpfLoader;
use kernel_bpf::maps::{ArrayMap, BpfMap, HashMap as BpfHashMap};
use kernel_bpf::profile::ActiveProfile;

pub const ATTACH_TYPE_TIMER: u32 = 1;
pub const ATTACH_TYPE_GPIO: u32 = 2;

pub struct BpfManager {
    programs: Vec<BpfProgram<ActiveProfile>>,
    attachments: BTreeMap<u32, Vec<u32>>,
    maps: Vec<Box<dyn BpfMap<ActiveProfile>>>,
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
            maps: Vec::new(),
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

    // --- Map operations ---

    pub fn create_map(
        &mut self,
        map_type: u32,
        key_size: u32,
        value_size: u32,
        max_entries: u32,
    ) -> Result<u32, BpfError> {
        let map: Box<dyn BpfMap<ActiveProfile>> = match map_type {
            1 => {
                // Hash map
                Box::new(
                    BpfHashMap::<ActiveProfile>::with_sizes(key_size, value_size, max_entries)
                        .map_err(|_| BpfError::OutOfMemory)?,
                )
            }
            2 => {
                // Array map
                Box::new(
                    ArrayMap::<ActiveProfile>::with_entries(value_size, max_entries)
                        .map_err(|_| BpfError::OutOfMemory)?,
                )
            }
            _ => {
                log::warn!("Unsupported map type: {}", map_type);
                return Err(BpfError::InvalidInstruction);
            }
        };

        let id = self.maps.len() as u32;
        self.maps.push(map);
        log::info!(
            "Created map id={} type={} key_size={} value_size={} max_entries={}",
            id,
            map_type,
            key_size,
            value_size,
            max_entries
        );
        Ok(id)
    }

    pub fn map_lookup(&self, map_id: u32, key: &[u8]) -> Option<Vec<u8>> {
        self.maps.get(map_id as usize)?.lookup(key)
    }

    /// Look up a value by key and return a raw pointer.
    ///
    /// # Safety
    /// The caller must ensure the map will not be resized or deleted while the
    /// pointer is in use. The returned pointer is only valid while the map lock
    /// is held by the caller.
    pub unsafe fn map_lookup_ptr(&self, map_id: u32, key: &[u8]) -> Option<*mut u8> {
        // Safety: caller ensures map will not be resized or deleted while pointer is in use
        unsafe { self.maps.get(map_id as usize)?.lookup_ptr(key) }
    }

    pub fn map_update(
        &self,
        map_id: u32,
        key: &[u8],
        value: &[u8],
        flags: u64,
    ) -> Result<(), BpfError> {
        let map = self.maps.get(map_id as usize).ok_or(BpfError::NotLoaded)?;
        map.update(key, value, flags)
            .map_err(|_| BpfError::OutOfMemory)
    }

    pub fn map_delete(&self, map_id: u32, key: &[u8]) -> Result<(), BpfError> {
        let map = self.maps.get(map_id as usize).ok_or(BpfError::NotLoaded)?;
        map.delete(key).map_err(|_| BpfError::NotLoaded)
    }

    pub fn get_map_def(&self, map_id: u32) -> Option<&kernel_bpf::maps::MapDef> {
        self.maps.get(map_id as usize).map(|m| m.def())
    }
}
