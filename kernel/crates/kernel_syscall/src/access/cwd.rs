use kernel_abi::Errno;
use kernel_vfs::path::{AbsoluteOwnedPath, AbsolutePath};
use spin::RwLock;

pub trait CwdAccess {
    fn current_working_directory(&self) -> &RwLock<AbsoluteOwnedPath>;
    fn chdir(&self, path: &AbsolutePath) -> Result<(), Errno>;
}
