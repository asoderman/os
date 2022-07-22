use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;

use alloc::string::FromUtf8Error;
use alloc::sync::Arc;
use alloc::{boxed::Box, vec::Vec};

pub use path::Path;

mod file;
mod filesystem;
mod generic_file;
mod path;
mod ramfs;
mod rootfs;

use spin::RwLock;

use lazy_static::lazy_static;

pub use generic_file::GenericFile;
pub use file::VirtualNode;
pub use filesystem::FSAttributes;

pub use rootfs::rootfs;

use filesystem::FileSystem;

/*

#[derive(Debug)]
struct VFS {
    pub fs: Box<dyn FileSystem>,
}

impl VFS {
    /// Wrap a filesystem in the VFS interface. Does not add new VFS to the global mount list
    fn new(fs: Box<dyn FileSystem>) -> Self {
        Self {
            fs,
        }
    }

    fn mount(mut self, root: Path, data: &[u8]) -> Result<(), Error> {
        self.fs.mount(root, data)?;

        VFS_LIST.write().push(self);

        Ok(())
    }

    fn read_dir(&self, path: Path) -> Result<Box<dyn Iterator<Item=Path>>, Error> {
        self.fs.read_dir(path)
    }

    fn create_dir(&mut self, path: Path) -> Result<(), Error> {
        self.fs.create_dir(path)
    }

    fn remove_dir(&mut self, path: Path) -> Result<(), Error> {
        self.fs.remove_dir(path)
    }

    fn root(&self) -> Result<Path, Error> {
        self.fs.root()
    }

    fn sync(&self) -> Result<(), Error> {
        todo!()
    }

    fn fid(&self) -> Result<(), Error> {
        todo!()
    }

    fn vget(&self) -> Result<(), Error> {
        todo!()
    }
}
*/

type Error = FsError;

#[derive(Debug, Clone)]
pub enum FsError {
    InvalidPath(Option<FromUtf8Error>),
    InvalidAccess,
    Exists,
    BadFd,
}
/// Construct a ram filesystem vfs object
fn init_devfs() {
    let dev_fs = Arc::new(RwLock::new(ramfs::RamFs::new()));

    let serial_device = crate::dev::serial::generic_serial_device();

    rootfs().write().mount_filesystem(dev_fs.clone(), Path::from_str("/dev")).expect("Could not create ramfs");

    dev_fs.write().insert_node(Path::from_str("/dev/serial"), serial_device.into()).expect("Could not create serial device file");
}

pub fn init() {
    ramfs::init_ramfs();
    init_devfs();
}
