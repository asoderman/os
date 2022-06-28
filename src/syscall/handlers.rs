use core::slice::from_raw_parts;

use alloc::sync::Arc;
use syscall::error::SyscallError;
use syscall::flags::MemoryFlags;

use crate::arch::VirtAddr;
use crate::mm::{user_map, user_unmap};
use crate::proc::{process_list, yield_time, exit};
use crate::interrupt::{without_interrupts, disable_interrupts};

use super::userptr::UserPtr;

pub type Error = SyscallError;
type Result<T> = core::result::Result<T, Error>;

pub fn sleep(seconds: usize) -> Result<()> {
    without_interrupts(|| {
        {
            let current = process_list().current();

            current.write().sleep_for(seconds);
            drop(current)
        }

        yield_time();
    });

    log::trace!("Sleep done");
    Ok(())
}

pub fn yield_() -> Result<()> {
    without_interrupts(|| {
        yield_time()
    });

    Ok(())
}

pub fn do_exit(status: usize) -> Result<isize> {
    disable_interrupts();

    exit(status);

    Ok(0)
}

pub fn mmap(addr: UserPtr, pages: usize, flags: MemoryFlags) -> Result<isize> {
    let current = process_list().current();
    log::info!("flags: {:08b}", flags);
    user_map(&mut *current.write(), addr.addr(), pages).map_err(|_| SyscallError::Exist)?;

    Ok(pages as isize)
}

pub fn munmap(addr: UserPtr, pages: usize) -> Result<isize> {
    let current = process_list().current();
    // TODO: validate page count
    user_unmap(&mut *current.write(), addr.addr(), pages).map_err(|_| SyscallError::NoMem)?;

    Ok(pages as isize)
}

enum Protection {
    ReadOnly = 1,
    ReadWrite = 3,
    Executable = 5,
}

impl TryFrom<MemoryFlags> for Protection {

    type Error = SyscallError;

    fn try_from(f: MemoryFlags) -> Result<Self> {
        let mask = 0b111;
        let masked_flags = f.bits() & mask;
        let out = match masked_flags {
            5 => Self::Executable,
            3 => Self::ReadWrite,
            1 => Self::ReadOnly,
            _ => Err(SyscallError::InvalidFlags)?
        };

        Ok(out)
    }
}

pub fn mprotect(addr: UserPtr, pages: usize, flags: MemoryFlags) -> Result<()> {
    let current = process_list().current();
    let mut lock = current.write();

    let address_space = lock.address_space.as_mut().unwrap();

    let mapping = address_space
        .mapping_containing(addr.addr())
        .ok_or(SyscallError::NoMem)?;

    if mapping.page_count() != pages { Err(SyscallError::NoMem)? }

    drop(mapping);

    let prot = Protection::try_from(flags)?;

    let vmm_result = match prot {
        Protection::Executable => address_space.set_region_executable(addr.addr()),
        Protection::ReadWrite => address_space.set_region_readwrite(addr.addr()),
        Protection::ReadOnly => address_space.set_region_readonly(addr.addr()),
    };

    vmm_result.map_err(|_| SyscallError::NoMem)?;

    Ok(())
}

pub fn log_print(ptr: UserPtr, len: usize) -> Result<()> {
    log::info!("user_put: {:?} len {}", ptr.addr(), len);
    let char_slice = unsafe { ptr.read_bytes(len) };
    let string = core::str::from_utf8(char_slice);
    log::info!("{:?}", string);

    Ok(())
}
