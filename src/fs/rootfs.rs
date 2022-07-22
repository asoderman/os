use alloc::{collections::BTreeMap, sync::Arc};
use spin::RwLock;

use super::{Path, filesystem::FileSystem, Error, FsError, VirtualNode, file::FileAttributes};

use lazy_static::lazy_static;

lazy_static! {
    static ref ROOT: RwLock<RootFs> = RwLock::new(RootFs::default());
}

pub fn rootfs<'r>() -> &'r RwLock<RootFs> {
    &ROOT
}


#[derive(Debug, Default)]
pub struct RootFs {
    file_systems: BTreeMap<Path, Arc<RwLock<dyn FileSystem>>>
}

impl RootFs {
    pub(super) fn mount_filesystem(&mut self, fs: Arc<RwLock<dyn FileSystem>>, mount_point: Path) -> Result<(), Error> {
        if self.file_systems.insert(mount_point.clone(), fs.clone()).is_none() {
            fs.write().mount(mount_point, &[]);
            Ok(())
        } else {
            Err(FsError::Exists)
        }
    }

    pub(super) fn unmount_filesystem(&mut self, mount_point: &Path) -> Result<(), Error> {
        todo!()
    }

    fn fs_for_mountpoint(&self, path: &Path) -> Option<&Arc<RwLock<dyn FileSystem>>> {
        let mount_point = root_mount_point(path)?;
        self.file_systems.get(&mount_point)
    }

    pub fn create_file(&self, path: &Path) -> Result<(), Error> {
        self.fs_for_mountpoint(&path).ok_or(FsError::Exists)?.write().create_file(path.clone())
    }

    pub fn remove_file(&self, path: &Path) -> Result<(), Error> {
        self.fs_for_mountpoint(&path).ok_or(FsError::Exists)?.write().remove_file(path.clone())
    }

    pub fn create_dir(&self, path: &Path) -> Result<(), FsError> {
        self.fs_for_mountpoint(&path).ok_or(FsError::Exists)?.write().create_dir(path.clone())
    }

    pub fn remove_dir(&self, path: &Path) -> Result<(), Error> {
        self.fs_for_mountpoint(&path).ok_or(FsError::Exists)?.write().remove_dir(path.clone())
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.fs_for_mountpoint(path).map(|fs| fs.read().exists(path)).unwrap_or(false)
    }

    /// Returns a weak reference virtual node
    pub fn get_file(&self, path: &Path) -> Result<VirtualNode, Error> {
        self.fs_for_mountpoint(path).ok_or(FsError::Exists)?.read().get_file(path).map(|node| node.weak_clone())

    }
}

/// Takes an absolute path and returns the first component in the path
fn root_mount_point(path: &Path) -> Option<Path> {
    if path.is_absolute() {
        let path_clone = path.clone();
        let mut components = path_clone.components();

        let first = components.next()?;

        let mut mount_point = Path::from_components([first].into_iter());
        mount_point.remove_trailing_slash();
        Some(mount_point)
    } else {
        None
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test_case]
    fn test_path_mount_point() {
        let test_path = Path::from_str("/mount/point");
        let mount_point = root_mount_point(&test_path).unwrap();

        assert!(mount_point == Path::from_str("/mount"));
    }

}
