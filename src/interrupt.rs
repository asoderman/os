use x86_64::structures::idt::InterruptStackFrame;

use crate::{println, arch::x86_64::paging::Mapper, mm::get_kernel_context_virt};

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

pub fn without_interrupts<F: FnOnce()>(f: F) {
    let was = disable_interrupts();
    f();
    if was { enable_interrupts(); }
}

pub fn init() -> Result<(), ()> {
    crate::stack::print_stack_usage();
    println!("enter interrupt::init()");
    #[cfg(target_arch="x86_64")]
    {
        crate::arch::x86_64::idt::init_idt()?;
        println!("Empty IDT initialized");
        let idt = crate::arch::x86_64::idt::get_idt_mut().ok_or(())?;

        x86_64::set_general_handler!(idt, no_op_isr);
        x86_64::set_general_handler!(idt, page_fault_err, 0xe);
        println!("Noop handlers installed");
    }

    println!("Enabling irq");
    enable_interrupts();
    println!("irq enabled");
    Ok(())
}
