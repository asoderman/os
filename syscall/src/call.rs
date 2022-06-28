use super::number::Syscall;
use super::{syscall0, syscall1, syscall2, syscall3};

#[no_mangle]
pub extern "C" fn exit(status: usize) -> isize {
    unsafe {
        syscall1(Syscall::EXIT, status)
    }
}

#[no_mangle]
pub extern "C" fn k_log(ptr: *const u8, len: usize) -> isize {
    unsafe {
        syscall2(Syscall::LOGPRINT, ptr as usize, len)
    }
}

#[no_mangle]
pub extern "C" fn sleep(seconds: usize) -> isize {
    unsafe {
        syscall1(Syscall::SLEEP, seconds)
    }
}

#[no_mangle]
pub extern "C" fn yield_() -> isize {
    unsafe {
        syscall0(Syscall::YIELD)
    }
}

#[no_mangle]
pub extern "C" fn mmap(ptr: *const u8, pages: usize, flags: usize) -> isize {
    unsafe {
        syscall3(Syscall::MMAP, ptr as usize, pages, flags)
    }
}

#[no_mangle]
pub extern "C" fn munmap(ptr: *const u8, pages: usize) -> isize {
    unsafe {
        syscall2(Syscall::MUNMAP, ptr as usize, pages)
    }
}

#[no_mangle]
pub extern "C" fn mprotect(ptr: *const u8, pages: usize, prot: usize) -> isize {
    unsafe {
        syscall3(Syscall::MPROTECT, ptr as usize, pages, prot)
    }
}
