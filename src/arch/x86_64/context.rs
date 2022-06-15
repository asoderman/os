use core::arch::asm;
use memoffset::offset_of;
use x86_64::{PhysAddr, VirtAddr};

use crate::proc::switch_hook;

#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct Context {
    rbx: usize,
    rsp: usize,
    rbp: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,

    rflags: usize,

    cr3: usize
}

impl Context {
    pub fn push(&mut self, value: usize) {
        unsafe {
            self.rsp -= core::mem::size_of::<usize>();
            *(self.rsp as *mut usize) = value;
        }
    }

    pub fn rsp(&self) -> VirtAddr {
        VirtAddr::new(self.rsp as u64)
    }

    pub fn set_rsp(&mut self, addr: VirtAddr) {
        self.rsp = addr.as_u64() as usize;
    }

    pub fn set_cr3(&mut self, addr: PhysAddr) {
        self.cr3 = addr.as_u64() as usize;
    }

    pub unsafe fn switch(&mut self, next: &mut Self) {
        switch(self, next)
    }
}

#[naked]
pub unsafe extern "C" fn switch(_current: &mut Context, _next: &mut Context) {
    asm!("
        mov [rdi+{r12}], r12
        mov r12, [rsi+{r12}]

        mov [rdi+{r13}], r13
        mov r13, [rsi+{r13}]

        mov [rdi+{r14}], r14
        mov r14, [rsi+{r14}]

        mov [rdi+{r15}], r15
        mov r15, [rsi+{r15}]

        mov [rdi+{rbx}], rbx
        mov rbx, [rsi+{rbx}]

        mov [rdi+{rsp}], rsp
        mov rsp, [rsi+{rsp}]

        mov [rdi+{rbp}], rbp
        mov rbp, [rsi+{rbp}]

        pushfq
        pop rax
        mov [rdi+{rflags}], rax

        push [rsi+{rflags}]
        popfq

        mov rcx, cr3
        mov [rdi+{cr3}], rcx
        mov rax, [rsi+{cr3}]
        cmp rax, rcx

        je 1f
        mov cr3, rax

1:
        jmp {switch_hook}
        ", 
        r12 = const(offset_of!(Context, r12)),
        r13 = const(offset_of!(Context, r13)),
        r14 = const(offset_of!(Context, r14)),
        r15 = const(offset_of!(Context, r15)),
        rbx = const(offset_of!(Context, rbx)),
        rsp = const(offset_of!(Context, rsp)),
        rbp = const(offset_of!(Context, rbp)),
        cr3 = const(offset_of!(Context, cr3)),
        rflags = const(offset_of!(Context, rflags)),
        switch_hook = sym switch_hook,
        options(noreturn)
        );
}

#[cfg(test)]
mod test {

    use super::*;
    use core::ptr::addr_of;

    static mut C1: usize = 0;
    static mut C2: usize = 0;
    static mut SWITCH_HAPPENED: bool = false;

    extern "C" fn test_return(_old: &mut Context, _new: &mut Context) {
    }

    fn switch_back() {
        unsafe {
            let prev = (C1 as *mut Context).as_mut().unwrap();
            let this = (C2 as *mut Context).as_mut().unwrap();
            SWITCH_HAPPENED = true;
            switch(this, prev);
        }
    }

    fn do_switch(current: &mut Context, next: &mut Context) {
        unsafe {
            switch(current, next)
        }
    }

    #[test_case]
    fn test_context_switch() {

        let mut current = Context::default();
        let mut next = Context::default();

        // Store the contexts globally so the other task can access them
        unsafe {
            C1 = addr_of!(current) as usize;
            C2 = addr_of!(next) as usize;
        }

        let new_stack = crate::stack::allocate_kernel_stack();
        next.rsp = new_stack.as_u64() as usize;

        next.push(switch_back as usize);
        next.push(test_return as usize);

        next.set_cr3(super::super::paging::get_cr3());

        do_switch(&mut current, &mut next);

        unsafe {
            assert!(SWITCH_HAPPENED);
        }
    }
}
