use x86_64::structures::idt::InterruptStackFrame;

use crate::{println, arch::x86_64::paging::Mapper, mm::get_kernel_context_virt};

mod handlers;
pub mod number;

fn no_op_isr(frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    println!("Dummy ISR {:#X} e: {:#X?}: \n {:#?}", index, error_code, frame);
}

pub fn eoi() {
    #[cfg(target_arch="x86_64")]
    {
        // TODO: this maps the lapic on every call. FIX THIS!!!
        crate::arch::x86_64::smp::lapic::Lapic::new().eoi();
    }
}

fn page_fault_err(frame: InterruptStackFrame, _index: u8, error_code: Option<u64>) {
    let cr2 = x86_64::registers::control::Cr2::read();
    unsafe {
        let mut m = Mapper::new(cr2, get_kernel_context_virt().unwrap().as_mut());

        loop {
            println!("{:?}", m.next_entry());
            if m.advance().is_err() { break; }
        }
    }
    panic!("<Kernel Pagefault> e: {:#}\n --\n Cr2: {:?}\n{:?}", error_code.unwrap(), cr2, frame);
}

pub fn interrupts_enabled() -> bool {
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::are_enabled()
}

pub fn enable_interrupts() {
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::enable();
}

pub fn disable_interrupts() -> bool {
    let out = interrupts_enabled();
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::disable();
    out
}

pub fn restore_interrupts(should: bool) {
    if should {
        enable_interrupts()
    }
}

pub fn without_interrupts<F: FnOnce()>(f: F) {
    let was = disable_interrupts();
    f();
    restore_interrupts(was);
}

/// Halt the CPU. Waits for the next interrupt
pub fn enable_and_halt() {
    enable_interrupts();
    unsafe {
        core::arch::asm!("hlt");
    }
}

pub fn init() -> Result<(), ()> {
    #[cfg(target_arch="x86_64")]
    {
        let idt = crate::arch::x86_64::idt::get_idt_mut().ok_or(())?;

        x86_64::set_general_handler!(idt, no_op_isr);
        println!("Noop handlers installed");

        handlers::install_handlers();
    }
    Ok(())
}

