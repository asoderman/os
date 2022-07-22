use alloc::{vec::Vec, collections::BTreeMap, boxed::Box, sync::Arc};
use spin::RwLock;

use super::{Path, FileSystem, FSAttributes, Error, file::{File, FileAttributes, Read, Write, VirtualNode}, FsError};
use super::filesystem::FsType;

/// An in memory (only) filesystem
#[derive(Debug)]
pub struct RamFs {
    files: BTreeMap<Path, VirtualNode>,
    root: Path,
}

impl RamFs {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            root: Path::empty(),
        }
    }
}

impl FileSystem for RamFs {
    fn mount(&mut self, root: Path, _data: &[u8]) -> Result<(), Error> {
        self.root = root;

        Ok(())
    }

    fn unmount(&self) -> Result<(), Error> {
        todo!()
    }

    fn root(&self) -> Result<Path, Error> {
        Ok(self.root.clone())
    }

    fn sync(&self) -> Result<(), Error> {
        // TODO: Store copy on disk?
        Ok(())
    }

    fn fid(&self) -> Result<(), Error> {
        todo!()
    }

    fn vget(&self) -> Result<(), Error> {
        todo!()
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn read_dir(&self, path: Path) -> Result<alloc::boxed::Box<dyn Iterator<Item=Path>>, Error> {
        if path == self.root {
            let all: Vec<_> = self.files.iter().map(|(path, _)| path.clone()).collect();
            return Ok(Box::new(all.into_iter()));
        }
        let mut path_exists = false;
        let filtered: Vec<_> = self.files.iter().filter_map(|(p, _)| {
            if p.starts_with(&path) {
                path_exists = true;
                Some(p.clone())
            } else {
                None
            }
        }).collect();

        if path_exists {
            Ok(Box::new(filtered.into_iter()))
        } else {
            Err(FsError::Exists)
        }
    }

    fn create_dir(&mut self, mut path: Path) -> Result<(), Error> {
        if !path.starts_with(&self.root) {
            path = self.root.join(&path)
        }
        if self.files.insert(path, VirtualNode::Directory).is_some() {
            Err(FsError::Exists)
        } else {
            Ok(())
        }
    }

    fn remove_dir(&mut self, path: Path) -> Result<(), Error> {
        self.files.remove(&path).ok_or(FsError::Exists).map(|_| ())
    }

    fn get_file(&self, path: &Path) -> Result<&VirtualNode, Error> {
        self.files.get(&path).ok_or(FsError::Exists)
    }

    fn create_file(&mut self, path: Path) -> Result<(), Error> {
        if self.files.insert(path, VirtualNode::new_file::<MemoryFile>()).is_some() {
            Err(FsError::Exists)
        } else {
            Ok(())
        }
    }

    fn remove_file(&mut self, path: Path) -> Result<(), Error> {
        self.files.remove(&path).ok_or(FsError::Exists).map(|_| ())
    }

    fn attributes(&self) -> Option<FSAttributes> {
        Some(FSAttributes {
                block_size: 1,
                files: self.files.len(),
                fs_type: FsType::Ram,
        })
    }

    fn insert_node(&mut self, path: Path, node: VirtualNode) -> Result<(), Error> {
        // TODO: check if path is correct e.g. parent exists
        if self.files.insert(path, node).is_none() {
            Ok(())
        } else {
            Err(FsError::Exists)
        }
    }
}

#[derive(Debug, Default)]
struct MemoryFile {
    data: Vec<u8>,
    position: usize,
}

impl File for MemoryFile {
    fn content(&self) -> Result<&[u8], Error> {
        Ok(&self.data)
    }

    fn position(&self) -> usize {
        self.position
    }

    fn attributes(&self) -> super::file::FileAttributes {
        FileAttributes {
            file_size: self.data.len(),
            access_time: 0,
            modified_time: 0,
            create_time: 0,
            blocks: self.data.len()
        }
    }
}

impl Read for MemoryFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: Position
        buf[..self.data.len()].copy_from_slice(&self.data[self.position..]);

        Ok(core::cmp::min(buf.len(), self.data[self.position..].len()))
    }
}

impl Write for MemoryFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if self.data.len() < buf.len() {
            self.data.resize(buf.len(), 0);
        }
        self.data.copy_from_slice(buf);
        Ok(buf.len())
    }
}

/// Construct a ram filesystem vfs object
pub fn init_ramfs() {
    use super::rootfs;

    let ramfs_vfs = Arc::new(RwLock::new(RamFs::new()));

    rootfs().write().mount_filesystem(ramfs_vfs, Path::from_str("/tmp")).expect("Could not create ramfs");
}

#[cfg(test)]
mod test {

    use alloc::string::String;

    use super::*;

    fn test_fs() -> RamFs {
        let mut fs = RamFs::new();
        assert!(fs.mount(test_root_path(), &[]).is_ok());

        fs
    }

    fn test_root_path() -> Path {
        Path::from_str("/test")
    }

    fn test_node() -> VirtualNode {
        VirtualNode::new_file::<MemoryFile>()
    }

    #[test_case]
    fn test_mount() {
        let mut test_fs = RamFs::new();
        let test_root = Path::from_str("/test");

        let mount_result = test_fs.mount(test_root.clone(), &[]);

        assert!(mount_result.is_ok());
        assert_eq!(test_fs.root().unwrap(), test_root);
    }

    #[test_case]
    fn test_readdir_empty() {
        let test_fs = test_fs();

        let read_dir = test_fs.read_dir(test_fs.root().unwrap());
        let read_dir = read_dir.unwrap();
        assert_eq!(read_dir.count(), 0);
    }

    #[test_case]
    fn test_create_and_remove_dir() {
        let mut test_fs = test_fs();

        let create_result = test_fs.create_dir(Path::from_str("new"));

        let subpaths = test_fs.read_dir(test_fs.root().unwrap()).map(|i| i.collect());

        assert!(subpaths.is_ok());

        let subpaths: Vec<_> = subpaths.unwrap();

        assert!(create_result.is_ok());
        assert_eq!(subpaths.len(), 1);
        // Check that the expected path is in the results
        let created_dir = subpaths.into_iter().find(|p| p == &test_root_path().join(&Path::from_str("new")));
        assert!(created_dir.is_some());
    }

    #[test_case]
    fn test_create_and_remove_file() {
        let mut test_fs = test_fs();

        let test_path = Path::from_str("foo");

        assert_eq!(test_fs.attributes().unwrap().files, 0);
        test_fs.create_file(test_path.clone()).unwrap();
        assert_eq!(test_fs.attributes().unwrap().files, 1);

        test_fs.remove_file(test_path).unwrap();
        assert_eq!(test_fs.attributes().unwrap().files, 0);
    }

    #[test_case]
    fn test_read_and_write_file() {
        let mut test_fs = test_fs();
        let test_path = Path::from_str("foo");
        let test_data = "hello world".as_bytes();

        test_fs.create_file(test_path.clone()).unwrap();

        let file = test_fs.get_file(&test_path).expect("Could not get file");

        let bytes_written = file.write(test_data).expect("Could not write to file");
        assert_eq!(bytes_written, test_data.len());

        let mut read_buffer = [0u8; 64];

        let bytes_read = file.read(&mut read_buffer).expect("Could not read file");
        assert_eq!(bytes_read, bytes_written);
        let read_string = String::from_utf8(read_buffer[0..bytes_read].to_vec()).unwrap();

        assert_eq!(read_string, "hello world");
    }
}
