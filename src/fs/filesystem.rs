use core::fmt::Debug;
use alloc::boxed::Box;

use super::Error;
use super::path::Path;
use super::file::VirtualNode;

/// The type of the virtual file system
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum FsType {
    Block,
    Device,
    Ram,
    Character,
    Socket,
}

/// A trait representing an abstract file system operations and attributes
pub(super) trait FileSystem: Debug + Send + Sync {
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

    fn insert_node(&mut self, path: Path, node: VirtualNode) -> Result<(), Error>;

    fn attributes(&self) -> Option<FSAttributes>;
}


#[derive(Debug, Clone)]
pub struct FSAttributes {
    pub block_size: usize,
    pub files: usize,
    pub fs_type: FsType,
}
