use x86_64::structures::idt::InterruptStackFrame;

use crate::println;

/// x86_64 interrupt numbers
/// TODO: Implement arm conversion
#[repr(usize)]
enum Interrupt {
    DivideError = 0,
    Debug = 1,
    Breakpoint = 3,
}

fn no_op_isr(frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    println!("Dummy ISR {:#X} e: {:#X?}: \n {:#?}", index, error_code, frame);
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

pub fn without_interrupts<F: FnOnce()>(f: F) {
    let was = disable_interrupts();
    f();
    if was { enable_interrupts(); }
}

pub fn init() -> Result<(), ()> {
    crate::util::print_stack_usage();
    println!("enter interrupt::init()");
    #[cfg(target_arch="x86_64")]
    {
        crate::arch::x86_64::idt::init_idt()?;
        println!("Empty IDT initialized");
        let idt = crate::arch::x86_64::idt::get_idt_mut().ok_or(())?;

        x86_64::set_general_handler!(idt, no_op_isr);
        println!("Noop handlers installed");
    }

    println!("Enabling irq");
    enable_interrupts();
    Ok(())
}
