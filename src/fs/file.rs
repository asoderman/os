use alloc::sync::Arc;
use core::fmt::Debug;
use spin::RwLock;

use super::{Error, FsError};

pub trait File: Read + Write + Debug + Send + Sync {
    fn open(&self) -> Result<(), Error>;
    fn close(&self) -> Result<(), Error>;
    fn content(&self) -> Result<&[u8], Error>;
    fn position(&self) -> usize;
    fn attributes(&self) -> FileAttributes;
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error>;
}

pub trait Read {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>;
}

#[derive(Debug, Clone)]
enum Permission {
    Readable,
    Writable
}

#[derive(Debug, Clone)]
pub struct FileNode {
    file: Arc<RwLock<dyn File>>,
}

impl FileNode {
    fn new<F: File + Default + 'static>() -> Self {
        Self {
            file: Arc::new(RwLock::new(F::default())),
        }
    }
}

impl Read for FileNode {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        self.file.read().read(buf)
    }
}

impl Write for FileNode {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.file.write().write(buf)
    }
}

#[derive(Debug, Clone)]
pub enum VirtualNode {
    File(FileNode),
    Directory
}

impl VirtualNode {
    pub fn new_file<F: File + Default + 'static>() -> Self {
        Self::File(FileNode::new::<F>())
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if let VirtualNode::File(file_node) = self {
            file_node.read(buffer)
        } else {
            Err(FsError::InvalidAccess)
        }
    }

    pub fn write(&self, buffer: &[u8]) -> Result<usize, Error> {
        if let VirtualNode::File(file_node) = self {
            let mut node = file_node.clone();
            node.write(buffer)
        } else {
            Err(FsError::InvalidAccess)
        }
    }
}

pub struct FileAttributes {
    pub file_size: usize,
    pub access_time: usize,
    pub modified_time: usize,
    pub change_time: usize,
    pub blocks: usize,
}
