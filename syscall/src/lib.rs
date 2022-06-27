#![no_std]
#![feature(cfg_panic)]

use core::arch::asm;

pub mod number;
pub mod call;

/// Syscall ABI 
///
/// a: rax
/// b: rdi
/// c: rsi
/// d: rdx
/// e: r8
/// f: r9
///
unsafe fn syscall0(a: usize) {
    asm!("
    mov rax, {}
    syscall
    ", in(reg) a
    );
}

unsafe fn syscall1(a: usize, b: usize) {
    asm!("
    mov rdi, {}
    mov rax, {}
    syscall
    ", in(reg) b, in(reg) a
    );
}

unsafe fn syscall2(a: usize, b: usize, c: usize) {
    asm!("
    mov rdi, {}
    mov rsi, {}
    mov rax, {}
    syscall
    ", in(reg) b, in(reg) c, in(reg) a
    );
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
