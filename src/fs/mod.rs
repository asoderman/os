use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;

use alloc::string::FromUtf8Error;
use alloc::sync::Arc;
use alloc::{boxed::Box, vec::Vec};

use crate::env::env;

pub use path::Path;

mod file;
mod filesystem;
mod generic_file;
mod path;
mod ramfs;
mod rootfs;

use spin::RwLock;

pub use generic_file::GenericFile;
pub use file::VirtualNode;
pub use filesystem::FSAttributes;

pub use rootfs::rootfs;

use filesystem::FileSystem;

use crate::mm::user_map_huge_mmio_anywhere;
use crate::proc::process_list;

type Error = FsError;

#[derive(Debug, Clone)]
pub enum FsError {
    InvalidPath(Option<FromUtf8Error>),
    InvalidAccess,
    Exists,
    BadFd,
    Mmap,
}

/// Creates a generic device for the framebuffer which only is able to memory map the framebuffer
/// to userspace or write to the buffer via syscall
fn generic_fb_device() -> GenericFile {
    let mut file = GenericFile::default();
    file.mmap_impl = Some(|_vaddr| {
        let fb_addr = env().video.as_ref().unwrap().frame_buffer_phys;
        log::info!("Video info: {:#?}", env().video.as_ref().unwrap());
        let current = process_list().current();
        let mut lock = current.write();
        unsafe {
            user_map_huge_mmio_anywhere(&mut *lock, fb_addr, 2).map_err(|_| FsError::Mmap).map(|mapping| mapping.virt_range().start)
        }
    });

    file
}

pub fn null_device() -> GenericFile {
    let mut file = GenericFile::default();
    file.read_impl = Some(|buf| { buf.fill(0); Ok(buf.len()) });
    file.write_impl = Some(|buf| Ok(buf.len()));

    file
}

/// Construct a ram filesystem vfs object
fn init_devfs() {
    let dev_fs = Arc::new(RwLock::new(ramfs::RamFs::new()));

    let serial_device = crate::dev::serial::generic_serial_device();

    let fb_device = generic_fb_device();

    rootfs().write().mount_filesystem(dev_fs.clone(), Path::from_str("/dev")).expect("Could not create ramfs");

    dev_fs.write().insert_node(Path::from_str("/dev/serial"), serial_device.into()).expect("Could not create serial device file");
    dev_fs.write().insert_node(Path::from_str("/dev/fb"), fb_device.into()).expect("Could not create framebuffer device file");
}

pub fn init() {
    ramfs::init_ramfs();
    init_devfs();
}
