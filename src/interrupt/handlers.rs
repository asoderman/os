use core::sync::atomic::Ordering;

use x86_64::structures::idt::InterruptStackFrame;

use super::{number::Interrupt, eoi};

use crate::{proc::{switch_next, TICKS_ELAPSED, PANIC}, arch::x86_64::interrupt::InterruptStack};

fn page_fault(frame: InterruptStackFrame, _index: u8, error_code: Option<u64>) {
    let cr2 = x86_64::registers::control::Cr2::read();

    panic!("<Kernel Pagefault> e: {:#}\n --\n Cr2: {:?}\n{:?}", error_code.unwrap(), cr2, frame);
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
