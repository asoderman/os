use core::ffi::c_char;

use crate::syscall4;

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
pub extern "C" fn mmap(ptr: *const u8, pages: usize, flags: usize, fd: usize) -> isize {
    unsafe {
        syscall4(Syscall::MMAP, ptr as usize, pages, flags, fd)
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

#[no_mangle]
pub extern "C" fn open(path: *const c_char) -> isize {
    unsafe {
        syscall2(Syscall::OPEN, path as usize, c_str_len(path))
    }
}

#[no_mangle]
pub extern "C" fn close(fd: usize) -> isize {
    unsafe {
        syscall1(Syscall::CLOSE, fd)
    }
}

#[no_mangle]
pub extern "C" fn read(fd: usize, buffer: *mut u8, len: usize) -> isize {
    unsafe {
        syscall3(Syscall::READ, fd, buffer as usize, len)
    }
}

#[no_mangle]
pub extern "C" fn write(fd: usize, buffer: *const u8, len: usize) -> isize {
    unsafe {
        syscall3(Syscall::WRITE, fd, buffer as usize, len)
    }
}

#[no_mangle]
pub extern "C" fn execv(path: *const c_char, args: *const c_char) -> isize {
    unsafe {
        syscall4(Syscall::EXECV, path as usize, c_str_len(path), args as usize, c_str_len(args))
    }
}

#[no_mangle]
pub extern "C" fn clone(func: *const extern "C" fn(usize), arg: usize) -> isize {
    unsafe {
        syscall2(Syscall::CLONE, func as usize, arg)
    }
}

#[no_mangle]
pub extern "C" fn mkfifo(path: *const c_char) -> isize {
    unsafe {
        syscall2(Syscall::MKFIFO, path as usize, c_str_len(path))
    }
}

unsafe fn c_str_len(ptr: *const c_char) -> usize {
    let mut count = 0;

    loop {
        if ptr.add(count).read() == 0 {
            return count;
        }

        count += 1;
    }
}
