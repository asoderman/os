use x86_64::structures::idt::InterruptStackFrame;

use crate::info;

pub mod handlers;
pub mod number;

fn unhandled_interrupt(frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    panic!("Dummy ISR {:#X} e: {:#X?}: \n {:#?}", index, error_code, frame);
}

pub fn eoi() {
    #[cfg(target_arch="x86_64")]
    {
        crate::arch::x86_64::smp::lapic::Lapic::new().eoi();
    }
}

pub fn interrupts_enabled() -> bool {
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::are_enabled()
}

pub fn enable_interrupts() -> bool {
    let was = interrupts_enabled();
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::enable();
    was
}

pub fn disable_interrupts() -> bool {
    let was = interrupts_enabled();
    #[cfg(target_arch="x86_64")]
    x86_64::instructions::interrupts::disable();
    was
}

pub fn restore_interrupts(should: bool) {
    if should {
        enable_interrupts();
    } else {
        disable_interrupts();
    }
}

pub fn without_interrupts<F: FnOnce()>(f: F) {
    let was = disable_interrupts();
    f();
    restore_interrupts(was);
}

/// Halt the CPU. Waits for the next interrupt. Restores the interrupts to their original state
/// after
pub fn enable_and_halt() {
    let was = enable_interrupts();
    unsafe { core::arch::asm!("hlt"); }
    restore_interrupts(was);
}

pub fn init() -> Result<(), ()> {
    #[cfg(target_arch="x86_64")]
    {
        let idt = crate::arch::x86_64::idt::get_idt_mut().ok_or(())?;

        x86_64::set_general_handler!(idt, unhandled_interrupt);
        info!("Installed `unhandled_interrupt` handler");

        handlers::install_handlers();
    }
    Ok(())
}

