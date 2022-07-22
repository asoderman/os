#![no_std]
#![feature(core_ffi_c)]

use core::arch::asm;

pub mod call;
pub mod error;
pub mod flags;
pub mod number;

/// Syscall ABI 
///
/// a: rax
/// b: rdi
/// c: rsi
/// d: rdx
/// e: r8
/// f: r9
///
unsafe extern "C" fn syscall0(mut a: usize) -> isize {
    asm!("
    syscall
    ", inout("rax") a, out("r11") _, out("rcx") _
    );

    a as isize
}

unsafe extern "C" fn syscall1(mut a: usize, b: usize) -> isize {
    asm!("
    syscall
    ", inout("rax") a, in("rdi") b, out("r11") _, out("rcx") _
    );

    a as isize
}

unsafe extern "C" fn syscall2(mut a: usize, b: usize, c: usize) -> isize {
    asm!("
    syscall
    ", inout("rax") a, in("rdi") b, in("rsi") c, out("rcx") _, out("r11") _
    );

    a as isize
}

unsafe extern "C" fn syscall3(mut a: usize, b: usize, c: usize, d: usize) -> isize {
    let result: isize;
    asm!("
    syscall
    ", inout("rax") a, in("rdi") b, in("rsi") c, in("rdx") d, out("r11") _, out("rcx")_
    );

    a as isize
}

unsafe extern "C" fn syscall4(mut a: usize, b: usize, c: usize, d: usize, e: usize) -> isize {
    let result: isize;
    asm!("
    syscall
    ", inout("rax") a, in("rdi") b, in("rsi") c, in("rdx") d, in("r8") e, out("r11") _, out("rcx")_
    );

    a as isize
}

unsafe extern "C" fn syscall5(mut a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let result: isize;
    asm!("
    syscall
    ", inout("rax") a, in("rdi") b, in("rsi") c, in("rdx") d, in("r8") e, in("r9") f, out("r11") _, out("rcx")_
    );

    a as isize
}

mod panic {
    use core::panic::PanicInfo;
    #[cfg_attr(feature="staticlib", panic_handler)]
    #[allow(dead_code)]
    fn panic(_: &PanicInfo) -> !{
        crate::call::exit(1);
        loop {}
    }
}
