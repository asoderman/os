use super::number::Syscall;
use super::{syscall0, syscall1, syscall2};

#[no_mangle]
pub extern "C" fn exit(status: usize) {
    unsafe {
        syscall1(Syscall::EXIT, status);
    }
}

#[no_mangle]
pub extern "C" fn k_log(ptr: *const u8, len: usize) {
    unsafe {
        syscall2(Syscall::LOGPRINT, ptr as usize, len)
    }
}

#[no_mangle]
pub extern "C" fn sleep(seconds: usize) {
    unsafe {
        syscall1(Syscall::SLEEP, seconds);
    }
}

#[no_mangle]
pub extern "C" fn yield_() {
    unsafe {
        syscall0(Syscall::YIELD);
    }
}
