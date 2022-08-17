use alloc::{sync::{Arc, Weak}, boxed::Box};
use core::fmt::Debug;
use spin::RwLock;

use super::{Error, FsError};

use crate::{arch::VirtAddr, proc::process_list};

pub trait File: Read + Write + Debug + Send + Sync {
    fn open(&self) -> Result<(), Error> {
        Ok(())
    }
    fn close(&self) -> Result<(), Error> {
        Ok(())
    }
    fn content(&self) -> Result<&[u8], Error>;
    fn mmap(&self, vaddr: VirtAddr) -> Result<VirtAddr, Error> {
        Err(FsError::InvalidAccess)
    }
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
    WeakFile(Weak<RwLock<dyn File>>),
    File(FileNode),
    Directory
}

impl<F: File + 'static> From<F> for VirtualNode {
    fn from(file: F) -> Self {
        Self::File(FileNode { file: Arc::new(RwLock::new(file)) })
    }
}

impl VirtualNode {
    pub fn new_file<F: File + Default + 'static>() -> Self {
        Self::File(FileNode::new::<F>())
    }

    pub fn weak_clone(&self) -> Self {
        if let Self::File(node) = self {
            Self::WeakFile(Arc::downgrade(&node.file))
        } else {
            self.clone()
        }
    }

    pub fn upgrade(self) -> Option<Self> {
        match self {
            Self::WeakFile(weak) => {
                Some(Self::File(FileNode { file: weak.upgrade()? }))
            },
            _ => Some(self)
        }
    }

    pub fn contents(&self) -> Option<Box<[u8]>> {
        match self {
            Self::File(node) => {
                // TODO: return error instead of panic
                Some(node.file.read().content().unwrap().to_vec().into_boxed_slice())
            },
            Self::WeakFile(weak) => {
                // TODO: return error instead of panic
                Some(weak.upgrade()?.read().content().unwrap().to_vec().into_boxed_slice())
            }
            _ => None
        }
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

    pub fn mmap(&self, vaddr: VirtAddr) -> Result<VirtAddr, Error> {
        if let VirtualNode::File(file_node) = self {
            let res = file_node.file.write().mmap(vaddr);
            log::info!("mmaped file to: {:?}", res);
            res
        } else {
            Err(FsError::InvalidAccess)
        }
    }

    /// Add this virtual node to the current process' open files list
    pub fn open(&self) -> usize {
        let current = process_list().current();
        let mut lock = current.write();

        lock.add_open_file(self.clone())
    }

    /// Invoke the implementation's close method
    pub fn close(&self) {
        if let VirtualNode::File(file) = self {
            file.file.write().close();
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileAttributes {
    pub file_size: usize,
    pub access_time: usize,
    pub modified_time: usize,
    pub create_time: usize,
    pub blocks: usize,
}
