use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;

use alloc::string::FromUtf8Error;
use alloc::{boxed::Box, vec::Vec};

pub use path::Path;

mod file;
mod filesystem;
mod path;
mod ramfs;

use spin::RwLock;

use lazy_static::lazy_static;

pub use file::VirtualNode;
pub use filesystem::FSAttributes;

use filesystem::FileSystem;

lazy_static! {
    static ref VFS_LIST: RwLock<Vec<VFS>> = RwLock::new(Vec::new());
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct VFS {
    pub id: usize,
    pub fs: Box<dyn FileSystem>,
}

impl VFS {
    /// Wrap a filesystem in the VFS interface. Does not add new VFS to the global mount list
    fn new(fs: Box<dyn FileSystem>) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fs,
        }
    }

    fn mount(mut self, root: Path, data: &[u8]) -> Result<(), Error> {
        self.fs.mount(root, data)?;

        VFS_LIST.write().push(self);

        Ok(())
    }

    fn unmount(&self) -> Result<(), Error> {
        // TODO handle error
        let index = VFS_LIST.read().iter().position(|vfs| vfs.id == self.id).unwrap();
        let unmounted_vfs = VFS_LIST.write().remove(index);

        unmounted_vfs.fs.unmount()?;

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

type Error = FsError;

#[derive(Debug, Clone)]
pub enum FsError {
    InvalidPath(Option<FromUtf8Error>),
    InvalidAccess,
    Exists,
}

pub fn init() {
    ramfs::init_ramfs();

}
