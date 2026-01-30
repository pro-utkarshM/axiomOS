use alloc::sync::Arc;
use core::sync::atomic::Ordering::Relaxed;

use kernel_syscall::access::{CwdAccess, FileAccess};
use kernel_syscall::stat::{StatAccess, UserStat, mode};
use kernel_vfs::node::VfsNode;
use kernel_vfs::path::AbsolutePath;
use spin::rwlock::RwLock;

use crate::U64Ext;
use crate::file::{OpenFileDescription, vfs};
use crate::mcore::context::ExecutionContext;
use crate::mcore::mtask::process::Process;
use crate::mcore::mtask::process::fd::{FdNum, FileDescriptor, FileDescriptorFlags};
use crate::mcore::mtask::task::Task;

mod mem;

pub struct KernelAccess<'a> {
    _task: &'a Task,
    process: Arc<Process>,
}

impl<'a> KernelAccess<'a> {
    pub fn new() -> Self {
        let task = ExecutionContext::load().current_task();
        let process = task.process().clone(); // TODO: can we remove the clone?

        KernelAccess {
            _task: task,
            process,
        }
    }
}

impl CwdAccess for KernelAccess<'_> {
    fn current_working_directory(&self) -> &RwLock<kernel_vfs::path::AbsoluteOwnedPath> {
        self.process.current_working_directory()
    }
}

pub struct FileInfo {
    node: VfsNode,
}

impl kernel_syscall::access::FileInfo for FileInfo {}

impl FileAccess for KernelAccess<'_> {
    type FileInfo = FileInfo;
    type Fd = FdNum;
    type OpenError = ();
    type ReadError = ();
    type WriteError = ();
    type CloseError = ();
    type LseekError = ();

    fn file_info(&self, path: &AbsolutePath) -> Option<Self::FileInfo> {
        Some(FileInfo {
            node: vfs().read().open(path).ok()?,
        })
    }

    fn open(&self, info: &Self::FileInfo) -> Result<Self::Fd, ()> {
        let ofd = OpenFileDescription::from(info.node.clone());
        let num = self
            .process
            .file_descriptors()
            .read()
            .keys()
            .fold(0, |acc, &fd| {
                if acc == Into::<i32>::into(fd) {
                    acc + 1
                } else {
                    acc
                }
            })
            .into();
        let fd = FileDescriptor::new(num, FileDescriptorFlags::empty(), ofd.into());

        self.process.file_descriptors().write().insert(num, fd);

        Ok(num)
    }

    fn read(&self, fd: Self::Fd, buf: &mut [u8]) -> Result<usize, ()> {
        let fds = self.process.file_descriptors();
        let guard = fds.read();

        let desc = guard.get(&fd).ok_or(())?;
        let ofd = desc.file_description();
        let offset = ofd.position().fetch_add(buf.len() as u64, Relaxed); // TODO: respect file max len
        ofd.read(buf, offset.into_usize()).map_err(|_| ())
    }

    fn write(&self, fd: Self::Fd, buf: &[u8]) -> Result<usize, ()> {
        let fds = self.process.file_descriptors();
        let guard = fds.read();

        let desc = guard.get(&fd).ok_or(())?;
        let ofd = desc.file_description();
        let offset = ofd.position().fetch_add(buf.len() as u64, Relaxed); // TODO: respect file max len
        ofd.write(buf, offset.into_usize()).map_err(|_| ())
    }

    fn close(&self, fd: Self::Fd) -> Result<(), ()> {
        self.process.file_descriptors().write().remove(&fd);
        Ok(())
    }

    fn lseek(&self, fd: Self::Fd, offset: i64, whence: i32) -> Result<usize, ()> {
        use kernel_syscall::unistd::{SEEK_CUR, SEEK_END, SEEK_SET};
        use kernel_vfs::Stat;

        let fds = self.process.file_descriptors();
        let guard = fds.read();

        let desc = guard.get(&fd).ok_or(())?;
        let ofd = desc.file_description();
        let current_pos = ofd.position().load(Relaxed);

        // Get file size for SEEK_END
        let file_size = {
            let mut stat = Stat::default();
            ofd.stat(&mut stat).map_err(|_| ())?;
            stat.size as u64
        };

        let new_pos = match whence {
            SEEK_SET => {
                if offset < 0 {
                    return Err(());
                }
                offset as u64
            }
            SEEK_CUR => {
                if offset < 0 {
                    current_pos.saturating_sub((-offset) as u64)
                } else {
                    current_pos.saturating_add(offset as u64)
                }
            }
            SEEK_END => {
                if offset < 0 {
                    file_size.saturating_sub((-offset) as u64)
                } else {
                    file_size.saturating_add(offset as u64)
                }
            }
            _ => return Err(()),
        };

        ofd.position().store(new_pos, Relaxed);
        Ok(new_pos.into_usize())
    }
}

impl StatAccess for KernelAccess<'_> {
    type StatError = ();

    fn fstat(&self, fd: Self::Fd) -> Result<UserStat, Self::StatError> {
        use kernel_vfs::Stat;

        let fds = self.process.file_descriptors();
        let guard = fds.read();

        let desc = guard.get(&fd).ok_or(())?;
        let ofd = desc.file_description();

        let mut vfs_stat = Stat::default();
        ofd.stat(&mut vfs_stat).map_err(|_| ())?;

        // Convert VFS stat to userspace stat structure
        let user_stat = UserStat {
            st_size: vfs_stat.size as i64,
            st_mode: mode::S_IFREG | 0o644, // Regular file with rw-r--r-- permissions
            st_blksize: 4096,               // Common block size
            st_blocks: (vfs_stat.size as i64 + 511) / 512, // Number of 512B blocks
            ..Default::default()
        };

        Ok(user_stat)
    }
}

impl kernel_syscall::access::MemoryRegionAccess for KernelAccess<'_> {
    type Region = KernelMemoryRegionHandle;

    fn create_and_track_mapping(
        &self,
        location: kernel_syscall::access::Location,
        size: usize,
        allocation_strategy: kernel_syscall::access::AllocationStrategy,
    ) -> Result<kernel_syscall::UserspacePtr<u8>, kernel_syscall::access::CreateMappingError> {
        // Use the MemoryAccess trait to create the mapping
        let mapping = <Self as kernel_syscall::access::MemoryAccess>::create_mapping(
            self,
            location,
            size,
            allocation_strategy,
        )?;

        let addr =
            <crate::syscall::access::mem::KernelMapping as kernel_syscall::access::Mapping>::addr(
                &mapping,
            );

        // Convert the mapping to a region and track it
        let region_handle = mapping.into_region_handle();
        self.add_memory_region(region_handle);

        Ok(addr)
    }

    fn add_memory_region(&self, region: Self::Region) {
        self.process.memory_regions().add_region(region.inner);
    }
}

/// A handle to a memory region that implements the MemoryRegion trait
/// from kernel_syscall. This bridges the gap between the syscall layer
/// and the kernel's internal MemoryRegion type.
pub struct KernelMemoryRegionHandle {
    addr: kernel_syscall::UserspacePtr<u8>,
    size: usize,
    inner: crate::mcore::mtask::process::mem::MemoryRegion,
}

impl kernel_syscall::access::MemoryRegion for KernelMemoryRegionHandle {
    fn addr(&self) -> kernel_syscall::UserspacePtr<u8> {
        self.addr
    }

    fn size(&self) -> usize {
        self.size
    }
}
