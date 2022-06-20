use core::arch::asm;

use x86_64::VirtAddr;
use x86_64::registers::model_specific::{LStar, Efer, EferFlags, Star, SFMask};
use x86_64::structures::tss::TaskStateSegment;

use memoffset::offset_of;

use super::smp::thread_local::ProcessorControlBlock;
use super::interrupt::InterruptStack;
use super::gdt;

// Clears trap and interrupt enable
const RFLAGS_MASK: u64 = 0x300;

/// Enables the usage of the `syscall` and `sysret` instructions and installs the syscall receiver
pub(super) fn init_syscall() {
    unsafe {
        // Enable syscall instruction
        let mut efer = Efer::read();
        efer.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true);
        Efer::write(efer);

        let syscall_cs_ss_selector = (*gdt::KERNEL_CS_INDEX.get().unwrap()) << 3;
        let sysret_cs_ss_selector = (*gdt::USER_CS_INDEX.get().unwrap() << 3) | 3;
        Star::write_raw(sysret_cs_ss_selector, syscall_cs_ss_selector);

        // Allow because we are writing to the register but not modifying the const
        #[allow(const_item_mutation)]
        SFMask::MSR.write(RFLAGS_MASK);
    }

    // install syscall receiver
    LStar::write(VirtAddr::new(syscall_receiver as u64));
}

/// This is the asm entry point of the `syscall` instruction. This sets us up in a state where the
/// kernel can execute the general syscall handler by switching stacks/preserving registers
#[naked]
unsafe extern "C" fn syscall_receiver() {
        asm!(concat!("
        swapgs
        mov gs:[{tmp_user_stack}], rsp
        mov rsp, gs:[{tss_rsp0}]

        push QWORD PTR {ss_sel} // push stack selector
        push QWORD PTR gs:[{tmp_user_stack}] // push userspace rsp
        push r11                // push rflags
        push QWORD PTR {cs_sel} // push code selector
        push rcx                // push user rip

        push rax

        ",
        push_scratch!(),
        push_preserved!(),

        "
        mov rdi, rsp
        call __inner_receiver
        ",

        pop_preserved!(),
        pop_scratch!(),
        // Return
    //
    // We must test whether RCX is canonical; if it is not when running sysretq, the consequences
    // can be fatal.
    //
    // See https://xenproject.org/2012/06/13/the-intel-sysret-privilege-escalation/.
    //
    // This is not just theoretical; ptrace allows userspace to change RCX (via RIP) of target
    // processes.
    "
        // Set ZF iff forbidden bits 63:47 (i.e. the bits that must be sign extended) of the pushed
        // RCX are set.
        test DWORD PTR [rsp + 4], 0xFFFF8000

        // If ZF was set, i.e. the address was invalid higher-half, so jump to the slower iretq and
        // handle the error without being able to execute attacker-controlled code!
        jnz 1f

        // Otherwise, continue with the fast sysretq.

        pop rcx                 // Pop userspace return pointer
        add rsp, 8              // Pop fake userspace CS
        pop r11                 // Pop rflags
        pop QWORD PTR gs:[{tmp_user_stack}] // Pop userspace stack pointer
        mov rsp, gs:[{tmp_user_stack}]      // Restore userspace stack pointer
        swapgs                  // Restore gs from TSS to user data
        sysretq                 // Return into userspace; RCX=>RIP,R11=>RFLAGS

1:

        // Slow iretq
        xor rcx, rcx
        xor r11, r11
        swapgs
        iretq
        "

        ),
        tmp_user_stack = const(offset_of!(ProcessorControlBlock, tmp_user_rsp)),
        tss_rsp0 = const(offset_of!(ProcessorControlBlock, tss) + offset_of!(TaskStateSegment, privilege_stack_table)),
        ss_sel = const(0),
        cs_sel = const(0),
        options(noreturn)
        );
}

#[no_mangle]
pub extern "C" fn __inner_receiver(stack: &mut InterruptStack) {
    stack.scratch.rax = crate::syscall::syscall(
        stack.scratch.rax, 
        stack.preserved.rbx,
        stack.scratch.rcx,
        stack.scratch.rdx,
        stack.scratch.rsi,
        stack.scratch.rdi,
    );
}
