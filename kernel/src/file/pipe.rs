use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;

use conquer_once::spin::OnceCell;
use kernel_vfs::fs::{FileSystem, FsHandle};
use kernel_vfs::{
    CloseError, FsError, MkdirError, OpenError, ReadError, RmdirError, Stat, StatError, WriteError,
};
use spin::{Mutex, RwLock};

pub struct Pipe {
    buffer: Mutex<VecDeque<u8>>,
    // TODO: Use CondVar or Waker for blocking
}

impl Default for Pipe {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipe {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> usize {
        let mut buffer = self.buffer.lock();
        let mut read = 0;
        for b in buf {
            if let Some(byte) = buffer.pop_front() {
                *b = byte;
                read += 1;
            } else {
                break;
            }
        }
        read
    }

    pub fn write(&self, buf: &[u8]) -> usize {
        let mut buffer = self.buffer.lock();
        buffer.extend(buf);
        buf.len()
    }
}

pub struct PipeFs {
    pipes: BTreeMap<u64, Arc<Pipe>>,
    next_inode: u64,
}

impl Default for PipeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl PipeFs {
    pub fn new() -> Self {
        Self {
            pipes: BTreeMap::new(),
            next_inode: 1,
        }
    }

    pub fn create_pipe(&mut self) -> (FsHandle, FsHandle) {
        let inode = self.next_inode;
        self.next_inode += 1;

        let pipe = Arc::new(Pipe::new());
        self.pipes.insert(inode, pipe);

        // We use the same inode for both ends for now, but in reality
        // we might want separate handles if we track read/write ends differently.
        // For simple VFS interaction, we can just return the same handle ID.
        // The file descriptor flags/mode will determine read vs write.
        (FsHandle::from(inode), FsHandle::from(inode))
    }
}

impl FileSystem for PipeFs {
    fn open(&mut self, _path: &kernel_vfs::path::AbsolutePath) -> Result<FsHandle, OpenError> {
        Err(OpenError::NotFound) // Pipes are anonymous-only for now
    }

    fn close(&mut self, handle: FsHandle) -> Result<(), CloseError> {
        // TODO: refcounting? VfsNode holds Weak reference to FS, but FsHandle is just u64.
        // We probably shouldn't remove the pipe until all handles are closed.
        // But the current VFS doesn't expose refcounts on handles easily.
        // For now, we leak or implement a simple refcount in PipeFs?
        // Let's assume for this MVP that we don't delete pipes to avoid use-after-free
        // issues until we have a better handle lifecycle.
        // Or better: check if the Arc strong count is 1 (meaning only this map holds it).

        let _inode: u64 = handle.into();
        // if let Some(pipe) = self.pipes.get(&inode) {
        //     if Arc::strong_count(pipe) <= 1 {
        //         self.pipes.remove(&inode);
        //     }
        // }
        // Actually, VfsNode doesn't hold the Arc<Pipe>, the PipeFs does.
        // The VfsNode holds a FsHandle.
        // When VfsNode is dropped, it calls close().

        // Let's assume 2 handles per pipe initially (read/write).
        // This is tricky without more state.
        // For now, no-op.
        Ok(())
    }

    fn read(
        &mut self,
        handle: FsHandle,
        buf: &mut [u8],
        _offset: usize,
    ) -> Result<usize, ReadError> {
        let inode: u64 = handle.into();
        if let Some(pipe) = self.pipes.get(&inode) {
            let n = pipe.read(buf);
            if n == 0 && !buf.is_empty() {
                // Should block? For now return 0 (EOF) or EAGAIN?
                // Returning 0 usually means EOF.
                // If the write end is open but buffer empty, we should block.
                // If write end closed, return 0.
                // We don't track write end status yet.
                // Let's return 0 for "buffer empty" for now, which is technically EOF or non-blocking.
                Ok(0)
            } else {
                Ok(n)
            }
        } else {
            Err(ReadError::FsError(FsError::InvalidHandle))
        }
    }

    fn write(&mut self, handle: FsHandle, buf: &[u8], _offset: usize) -> Result<usize, WriteError> {
        let inode: u64 = handle.into();
        if let Some(pipe) = self.pipes.get(&inode) {
            Ok(pipe.write(buf))
        } else {
            Err(WriteError::FsError(FsError::InvalidHandle))
        }
    }

    fn stat(&mut self, _handle: FsHandle, stat: &mut Stat) -> Result<(), StatError> {
        stat.size = 0; // Unknown size
                       // TODO: Set S_IFIFO
        Ok(())
    }

    fn mkdir(&mut self, _path: &kernel_vfs::path::AbsolutePath) -> Result<(), MkdirError> {
        Err(MkdirError::FsError(FsError::InvalidHandle))
    }

    fn rmdir(&mut self, _path: &kernel_vfs::path::AbsolutePath) -> Result<(), RmdirError> {
        Err(RmdirError::FsError(FsError::InvalidHandle))
    }
}

pub static PIPE_FS: OnceCell<Arc<RwLock<PipeFs>>> = OnceCell::uninit();

pub fn init() {
    PIPE_FS.init_once(|| Arc::new(RwLock::new(PipeFs::new())));
}
