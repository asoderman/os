use core::arch::asm;

use x86_64::{VirtAddr, PrivilegeLevel};
use x86_64::registers::model_specific::{LStar, Efer, EferFlags, Star, SFMask};
use x86_64::registers::segmentation::SegmentSelector;
use x86_64::structures::tss::TaskStateSegment;

use memoffset::offset_of;

use super::smp::thread_local::ProcessorControlBlock;
use super::interrupt::InterruptStack;
use super::gdt;

use crate::syscall::NumericResult;

// Clears trap and interrupt enable
const RFLAGS_MASK: u64 = 0x300;

/// Enables the usage of the `syscall` and `sysret` instructions and installs the syscall receiver
pub(super) fn init_syscall() {
    unsafe {
        let syscall_cs_ss_selector = SegmentSelector::new(*gdt::KERNEL_CS_INDEX.get().unwrap(), PrivilegeLevel::Ring0);

        let kernel_tls_selector = SegmentSelector::new(*gdt::KERNEL_TLS_INDEX.get_unchecked(), PrivilegeLevel::Ring3);

        // Set sysret cs/ss selector to kernel tls because when sysret is executed for 64bit code
        // the cpu loads [Star entry + 8] for ss and [Star entry + 16] for cs
        // The gdt must be ordered kernel tls -> user data -> user code 64 for this to work
        Star::write_raw(kernel_tls_selector.0, syscall_cs_ss_selector.0);

        // Allow because we are writing to the register but not modifying the const
        #[allow(const_item_mutation)]
        SFMask::MSR.write(RFLAGS_MASK);

        // install syscall receiver
        LStar::write(VirtAddr::new(syscall_receiver as u64));

        // Enable syscall instruction
        let mut efer = Efer::read();
        efer.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true);
        Efer::write(efer);
    }
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
        ss_sel = const(0x23),
        cs_sel = const(0x2b),
        options(noreturn)
        );
}

/// Syscall ABI 
///
/// a: rax
/// b: rdi
/// c: rsi
/// d: rdx
/// e: r8
/// f: r9
///
#[no_mangle]
pub extern "C" fn __inner_receiver(stack: &mut InterruptStack) {
    // Allow us to use threadlocals again
    // TODO: gsbase only
    unsafe {
        super::smp::thread_local::set_fs_base_to_gs_base();
    }
    stack.scratch.rax = crate::syscall::syscall(
        stack.scratch.rax, 
        stack.scratch.rdi,
        stack.scratch.rsi,
        stack.scratch.rdx,
        stack.scratch.r8,
        stack.scratch.r9,
    ).as_isize() as usize;
    unsafe {
        super::smp::thread_local::restore_fs_base();
    }
}
