
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct ScratchRegisters {
    pub r11: usize,
    pub r10: usize,
    pub r9: usize,
    pub r8: usize,
    pub rsi: usize,
    pub rdi: usize,
    pub rdx: usize,
    pub rcx: usize,
    pub rax: usize,
}

#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct PreservedRegisters {
    pub r15: usize,
    pub r14: usize,
    pub r13: usize,
    pub r12: usize,
    pub rbp: usize,
    pub rbx: usize,
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(packed)]
pub struct IretRegisters {
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,

    // ----
    // The following will only be present if interrupt is raised from another
    // privilege ring. Otherwise, they are undefined values.
    // ----

    pub rsp: usize,
    pub ss: usize
}

#[repr(C)]
#[derive(Debug)]
pub struct InterruptStack {
    pub preserved: PreservedRegisters,
    pub scratch: ScratchRegisters,
    pub iret: IretRegisters
}

#[macro_export]
macro_rules! push_scratch {
    () => { "
        // Push scratch registers
        push rcx
        push rdx
        push rdi
        push rsi
        push r8
        push r9
        push r10
        push r11
    " };
}
#[macro_export]
macro_rules! pop_scratch {
    () => { "
        // Pop scratch registers
        pop r11
        pop r10
        pop r9
        pop r8
        pop rsi
        pop rdi
        pop rdx
        pop rcx
        pop rax
    " };
}

#[macro_export]
macro_rules! push_preserved {
    () => { "
        // Push preserved registers
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
    " };
}
#[macro_export]
macro_rules! pop_preserved {
    () => { "
        // Pop preserved registers
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
    " };
}

macro_rules! check_and_swap_gs {
    () => {"
    cmp QWORD PTR [rsp+0x08], QWORD PTR 0x08
    je 1f
    swapgs
    1:
    "}
}

macro_rules! interrupt {
    ($name:ident, |$stack:ident| $handler:block) => {
        #[naked]
        pub unsafe extern "C" fn $name() {
            unsafe extern "C" fn inner($stack: &mut InterruptStack) {
                crate::arch::x86_64::smp::thread_local::set_fs_base_to_gs_base();
                $handler;
                crate::arch::x86_64::smp::thread_local::restore_fs_base();
            }
            core::arch::asm!(concat!(
                    check_and_swap_gs!(),
                    "
                    push rax
                    ",
                    push_scratch!(),
                    push_preserved!(),
                    "
                    mov rdi, rsp
                    call {inner}
                    ",
                    pop_preserved!(),
                    pop_scratch!(),
                    check_and_swap_gs!(),
                    "iretq"
            ),
                inner = sym inner,
                options(noreturn));
        }
    }
}

interrupt!(timer, |stack| {
    crate::interrupt::handlers::timer(stack);
});
