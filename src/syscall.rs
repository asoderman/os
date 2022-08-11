use log::info;
use syscall::number::Syscall;
use syscall::error::SyscallError;
use syscall::flags::MemoryFlags;

pub mod handlers;
pub mod userptr;

pub use handlers::*;

use crate::syscall::userptr::UserPtr;

use syscall::error::OK_VAL;

/// This function takes the state of the syscall as input then performs the necessary
/// validation/type conversions and finally invokes the matching syscall handler
#[allow(unused_variables)]
pub fn syscall(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> impl NumericResult {
    info!("Syscall no. : {:#X}", a);
    match a {
        Syscall::OPEN => {
            unsafe {
                open(UserPtr::try_from(b)?.read_bytes(c)).map(|a| a as isize)
            }
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
