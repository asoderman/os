use super::file::{File, Read, Write};
use super::FsError;

/// This macro allows the inclusion of files at compile time and exposes them via the file system.
/// Files included using this macro are placed inside the kernel executable.
#[macro_export]
macro_rules! include_file {
    ($path:literal) => {{
        use alloc::string::String;
        use crate::fs::rootfs;
        use crate::fs::Path;

        let filename_string = String::from($path);
        let filename = filename_string.split('/').last().unwrap();
        let mut dir = String::from("/tmp/include/");
        dir.push_str(filename);
        let path = Path::from_str(&dir);
        let result = rootfs()
            .read()
            .insert_node(
                path,
                HostFile::new(include_bytes!($path)).into()
            );

        if let Err(e) = result {
            log::warn!("{} : {:?}", $path, e);
        }
    }}
}

#[derive(Debug)]
pub struct HostFile {
    host_data: &'static [u8]
}

impl HostFile {
    pub fn new(data: &'static [u8]) -> Self {
        HostFile {
            host_data: data
        }
    }
}

impl Read for HostFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, super::Error> {
        // FIXME: do a proper size check here
        buf.copy_from_slice(&self.host_data[..buf.len()]);
        Ok(buf.len())
    }
}

impl Write for HostFile {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, super::Error> {
        Err(FsError::InvalidAccess)
    }
}

impl File for HostFile {
    fn content(&self) -> Result<&[u8], super::Error> {
        Ok(self.host_data)
    }

    fn position(&self) -> usize {
        todo!()
    }

    fn attributes(&self) -> super::file::FileAttributes {
        todo!()
    }
}
