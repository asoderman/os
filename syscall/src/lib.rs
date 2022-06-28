#![no_std]
#![feature(cfg_panic)]

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
unsafe extern "C" fn syscall0(a: usize) -> isize {
    let result: isize;
    asm!("
    mov rax, {}
    syscall
    mov {}, rax
    ", in(reg) a, out(reg) result,
    );

    result
}

unsafe extern "C" fn syscall1(a: usize, b: usize) -> isize {
    let result: isize;
    asm!("
    mov rax, {}
    mov rdi, {}
    syscall
    mov {}, rax
    ", in(reg) a, in(reg) b, out(reg) result
    );

    result
}

unsafe extern "C" fn syscall2(a: usize, b: usize, c: usize) -> isize {
    let result: isize;
    asm!("
    mov rax, {}
    mov rdi, {}
    mov rsi, {}
    syscall
    mov {}, rax
    ", in(reg) a, in(reg) b, in(reg) c, out(reg) result
    );

    result
}

unsafe extern "C" fn syscall3(a: usize, b: usize, c: usize, d: usize) -> isize {
    let result: isize;
    asm!("
    mov rax, {}
    mov rdi, {}
    mov rsi, {}
    mov rdx, {}
    syscall
    mov {}, rax
    ", in(reg) a, in(reg) b, in(reg) c, in(reg) d, out(reg) result
    );

    result
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
