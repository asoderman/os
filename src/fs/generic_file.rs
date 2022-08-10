use crate::arch::VirtAddr;

use super::{Error, FsError, file::{File, Write, Read}};

/// A generic file that is able to expose kernel functions to userspace via file operations
#[derive(Default)]
pub struct GenericFile {
    pub open_impl: Option<fn() -> Result<(), FsError>>,
    pub close_impl: Option<fn() -> Result<(), FsError>>,
    pub read_impl: Option<fn(&mut [u8]) -> Result<usize, FsError>>,
    pub write_impl: Option<fn(&[u8]) -> Result<usize, FsError>>,
    pub mmap_impl: Option<fn(VirtAddr) -> Result<VirtAddr, FsError>>
}

impl core::fmt::Debug for GenericFile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GenericFile").finish()
    }
}

impl Read for GenericFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        if let Some(handler) = self.read_impl {
            handler(buf)
        } else {
            Err(FsError::InvalidAccess)
        }
    }
}

impl Write for GenericFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if let Some(handler) = self.write_impl {
            handler(buf)
        } else {
            Err(FsError::InvalidAccess)
        }
    }
}

impl File for GenericFile {
    fn content(&self) -> Result<&[u8], Error> {
        todo!()
    }

    fn position(&self) -> usize {
        todo!()
    }

    fn attributes(&self) -> super::file::FileAttributes {
        todo!()
    }
}

