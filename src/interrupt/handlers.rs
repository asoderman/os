use core::sync::atomic::Ordering;

use alloc::fmt::format;
use x86_64::{structures::idt::InterruptStackFrame, VirtAddr};

use super::{number::Interrupt, eoi};

use crate::{proc::{switch_next, TICKS_ELAPSED, PANIC, process_list}, arch::x86_64::{interrupt::InterruptStack, set_fs_base_to_gs_base, restore_fs_base}, interrupt::without_interrupts, dev::serial::write_serial_out};

fn page_fault(frame: InterruptStackFrame, _index: u8, error_code: Option<u64>) {

    without_interrupts(|| {
        //todo!("Check if fault was caused in usermode to re-enable thread locals if needed");
        if frame.code_segment & 3 != 0 {
            // fault coming from user
            unsafe {
                core::arch::asm!("swapgs");
                set_fs_base_to_gs_base();
            }

        }
        if frame.instruction_pointer < VirtAddr::new(0xFFFFFFFF80000000u64) {
            let current = process_list().current();
            let mut lock = current.write();
            let address_space = &mut lock.address_space;
            let address_space = address_space.as_mut().unwrap();
            let mapping = address_space.mapping_containing(frame.instruction_pointer);

            if let Some(mapping) = mapping {
                if mapping.is_cow() {
                    log::info!("#PF: Copy on write");
                    mapping.perform_copy_on_write(address_space.page_table())
                }
            } else {
                // According to the address space an unmapped area was accessed
                todo!("Implement kill")
            }
        } else {
            // Page fault in kernel. Something is wrong
            panic!("<Kernel Pagefault> e: {:#}\n --\n Cr2: {:?}\n{:?}", error_code.unwrap(), frame.instruction_pointer, frame);
        }
        if frame.code_segment & 3 != 0 {
            // fault coming from user
            unsafe {
                restore_fs_base();
                core::arch::asm!("swapgs");
            }

        }
    });

}

pub fn timer(_stack: &mut InterruptStack) {
    if !PANIC.load(Ordering::SeqCst) && TICKS_ELAPSED.fetch_add(1, Ordering::Acquire) >= 10 {
        unsafe {
            eoi();
            switch_next();
        }
    } else {
        eoi();
    }
}

#[allow(dead_code)]
fn syscall(_frame: InterruptStackFrame, _index: u8, _error_code: Option<u64>) {
    todo!("This needs to be defined in the HAL since we need the full interrupt context");
    //println!("[WARNING]: int 0x80 used for (slow) syscall");
}

pub(super) fn install_handlers() {
    let idt = crate::arch::x86_64::idt::get_idt_mut().expect("Could not get mut ref to IDT");

    // TODO: need to declare var because this const breaks the macro
    let page_fault_num = Interrupt::PageFault;
    let syscall_num = Interrupt::Syscall;

    x86_64::set_general_handler!(idt, page_fault, page_fault_num);
    x86_64::set_general_handler!(idt, syscall, syscall_num);
    unsafe {
        crate::arch::x86_64::idt::reg_timer();
    }
    //x86_64::set_general_handler!(idt, timer, timer_num);
}
