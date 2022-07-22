use super::{Error, FsError, file::{File, Write, Read}};

/// A generic file that is able to expose kernel functions to userspace
pub struct GenericFile {
    pub open_impl: fn() -> Result<(), FsError>,
    pub close_impl: fn() -> Result<(), FsError>,
    pub read_impl: fn(&mut [u8]) -> Result<usize, FsError>,
    pub write_impl: fn(&[u8]) -> Result<usize, FsError>,
}

impl core::fmt::Debug for GenericFile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GenericFile").finish()
    }
}

impl Default for GenericFile {
    fn default() -> Self {
        Self {
            open_impl: || { Ok(()) },
            close_impl: || { Ok(()) },
            read_impl: |_| { Err(FsError::InvalidAccess) },
            write_impl: |_| { Err(FsError::InvalidAccess) },
        }
    }
}

impl Read for GenericFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        (self.read_impl)(buf)
    }
}

impl Write for GenericFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        (self.write_impl)(buf)
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
