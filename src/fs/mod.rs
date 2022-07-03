use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;

use alloc::{string::{String, FromUtf8Error}, boxed::Box, vec::Vec};

mod file;
mod ramfs;

use spin::RwLock;

use lazy_static::lazy_static;

use file::VirtualNode;
use ramfs::RamFs;

lazy_static! {
    static ref VFS_LIST: RwLock<Vec<VFS>> = RwLock::new(Vec::new());
}

/// The type of the virtual file system node
#[repr(u8)]
#[derive(Debug, Clone)]
enum FsType {
    Block,
    Device,
    Ram,
    Character,
    Socket,
}

/// A trait representing an abstract file system operations and attributes
trait FileSystem: Debug + Send + Sync {
    fn mount(&mut self, root: Path, data: &[u8]) -> Result<(), Error>;
    fn unmount(&self) -> Result<(), Error>;

    fn root(&self) -> Result<Path, Error>;
    fn sync(&self) -> Result<(), Error>;
    fn fid(&self) -> Result<(), Error>;
    fn vget(&self) -> Result<(), Error>;

    fn exists(&self, path: &Path) -> bool;

    fn read_dir(&self, path: Path) -> Result<Box<dyn Iterator<Item=Path>>, Error>;
    fn create_dir(&mut self, path: Path) -> Result<(), Error>;
    fn remove_dir(&mut self, path: Path) -> Result<(), Error>;

    fn get_file(&self, path: &Path) -> Result<&VirtualNode, Error>;
    fn create_file(&mut self, path: Path) -> Result<(), Error>;
    fn remove_file(&mut self, path: Path) -> Result<(), Error>;


    fn attributes(&self) -> Option<VFSAttributes>;
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
struct VFSAttributes {
    pub block_size: usize,
    pub files: usize,
    pub fs_type: FsType,
}

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

#[derive(Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub struct Path(String);

impl Path {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        let string = String::from_utf8(bytes.to_vec()).map_err(|e| FsError::InvalidPath(Some(e)))?;

        Ok(Self(string))
    }

    pub fn from_str(path: &str) -> Self {
        Path::new(path.as_bytes()).unwrap()
    }

    pub fn empty() -> Self {
        Self::from_str("")
    }

    pub fn join(&self, other: &Self) -> Self {
        let mut new_path = self.clone();
        new_path.append(other);
        new_path
    }

    pub fn append(&mut self, other: &Self) {
        if !self.0.ends_with('/') {
            self.0.push('/');
        }

        // If thw other path starts with a separator index around it
        let start_index = if other.0.starts_with('/') {
            1
        } else {
            0
        };

        self.0.push_str(&other.0.as_str()[start_index..]);
    }

    /// Returns an iterator of each dir that makes up the path
    fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    fn starts_with(&self, prefix: &Self) -> bool {
        self.0.starts_with(&prefix.0)
    }

    pub fn filename(&self) -> Option<&str> {
        self.components().last()
    }

    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }
}

type Error = FsError;

#[derive(Debug, Clone)]
pub enum FsError {
    InvalidPath(Option<FromUtf8Error>),
    InvalidAccess,
    Exists,
}

pub fn init_ramfs() {
    let mut ramfs_vfs = VFS::new(Box::new(RamFs::new()));

    ramfs_vfs.create_dir(Path::from_str("/ram"));
    ramfs_vfs.fs.create_file(Path::from_str("/ram/foo"));

    let attr = ramfs_vfs.fs.attributes().unwrap();

    log::info!("ramfs attr: {:#X?}", attr);
    let read_res: Vec<_> = ramfs_vfs.fs.read_dir(Path::from_str("/ram")).unwrap().collect();
    log::info!("ramfs attr: {:#X?}", read_res);

    ramfs_vfs.mount(Path::from_str("/"), &[]).expect("Could not mount ramfs");

}
