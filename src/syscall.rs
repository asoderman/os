use log::info;
use syscall::number::Syscall;
use syscall::error::SyscallError;
use syscall::flags::{MemoryFlags, OpenFlags};

pub mod handlers;
pub mod userptr;

pub use handlers::*;
use x86_64::VirtAddr;

use crate::syscall::userptr::UserPtr;

use syscall::error::OK_VAL;

/// This function takes the state of the syscall as input then performs the necessary
/// validation/type conversions and finally invokes the matching syscall handler
#[allow(unused_variables)]
pub fn syscall(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> impl NumericResult {
    info!("Syscall no. : {:#X}", a);
    match a {
        Syscall::OPEN => {
            open(UserPtr::try_from(b)?.to_path(c)?, OpenFlags::from_bits(d).ok_or(SyscallError::InvalidFlags)?).map(|a| a as isize)
        },
        Syscall::CLOSE => close(b).map(|_| OK_VAL),
        Syscall::READ => {
            unsafe {
                read(b, UserPtr::try_from(c)?.write_bytes(d)).map(|a| a as isize)
            }
        },
        Syscall::WRITE => {
            unsafe {
                write(b, UserPtr::try_from(c)?.read_bytes(d)).map(|a| a as isize)
            }
        },
        Syscall::MKDIR => {
            mkdir(UserPtr::try_from(b)?.to_path(c)?).map(|_| OK_VAL)
        }
        Syscall::RMDIR => {
            rmdir(UserPtr::try_from(b)?.to_path(c)?).map(|_| OK_VAL)
        }
        Syscall::MKFILE => {
            mkfile(UserPtr::try_from(b)?.to_path(c)?).map(|_| OK_VAL)
        }
        Syscall::RMFILE => {
            rmfile(UserPtr::try_from(b)?.to_path(c)?).map(|_| OK_VAL)
        }
        Syscall::CLONE => {
            // TODO: validate the fn ptr VirtAddr is in userspace!
            clone(VirtAddr::new(b as u64), c)
        }
        Syscall::EXECV => {
            execv(UserPtr::try_from(b)?.to_path(c)?, UserPtr::try_from(d)?.to_string(e)?).map(|_| OK_VAL)
        }
        Syscall::SLEEP => sleep(b).map(|_| OK_VAL),
        Syscall::YIELD => yield_().map(|_| OK_VAL),
        Syscall::EXIT => do_exit(b),
        Syscall::LOGPRINT => log_print(UserPtr::try_from(b)?, c).map(|_| OK_VAL),
        Syscall::MMAP => {
            mmap(UserPtr::try_from(b).unwrap(), c, MemoryFlags::from_bits(d).ok_or(SyscallError::InvalidFlags).unwrap(), e)
        },
        Syscall::MUNMAP => {
            munmap(UserPtr::try_from(b)?, c)
        },
        Syscall::MPROTECT => {
            mprotect(UserPtr::try_from(b)?, c, MemoryFlags::from_bits(d).ok_or(SyscallError::InvalidFlags)?).map(|_| OK_VAL)
        },
        _ => Err(SyscallError::NoSys)
    }
}

/// A trait representing a `Result` type that can be squashed into an isize representing the error
/// code
pub trait NumericResult {
    fn as_isize(self) -> isize;
}

impl NumericResult for Result<(), SyscallError> {
    fn as_isize(self) -> isize {
        match self {
            Ok(()) => 0,
            Err(e) => e as isize
        }
    }
}

impl NumericResult for Result<isize, SyscallError> {
    fn as_isize(self) -> isize {
        match self {
            Ok(val) => val as isize,
            Err(e) => e as isize,
        }
    }
}
