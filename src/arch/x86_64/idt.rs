use alloc::boxed::Box;
use x86_64::VirtAddr;
use x86_64::structures::idt::InterruptDescriptorTable;

static mut IDT: Option<IDTInfo> = None;

struct IDTInfo {
    ptr: Box<InterruptDescriptorTable>,
}

impl IDTInfo {
    fn get_idt(&self) -> &InterruptDescriptorTable {
        self.ptr.as_ref()
    }

    fn get_idt_mut(&mut self) -> &mut InterruptDescriptorTable {
        self.ptr.as_mut()
    }
}

/// Wraps the interrupt service routine in an extern "x86-interrupt" handler
#[allow(unused_macros)]
macro_rules! impl_x86_64_interrupt {
    ($isr:ident) => {
        paste! {
            pub extern "x86-interrupt" fn [<x86_ $isr>](frame: InterruptStackFrame) {
                $isr(frame)
            }
        }
    }
}

/// Implements a x86-interrupt abi wrapper around the isr then registers the x86 wrapped function
#[allow(unused_macros)]
macro_rules! impl_and_register_x86_interrupt {
    ($num:expr, $isr:ident) => {
        use paste::paste;
        impl_x86_64_interrupt!($isr);
        paste! {
            $crate::arch::x86_64::idt::register_interrupt_handler($num as usize, [<x86_ $isr>]);
        }
    }
}

/// Initialize and load an (empty) IDT
pub fn init_idt() -> Result<(), ()> {
    if get_idt().is_none() {
        let idt = Box::new(InterruptDescriptorTable::new());
        // Set the global IDT
        unsafe {
            IDT = Some(IDTInfo { ptr: idt });
            IDT.as_mut().ok_or(())?.get_idt_mut().reset();
        }
    }

    assert!(!x86_64::instructions::interrupts::are_enabled());
    get_idt().unwrap().load();
    Ok(())
}

pub fn get_idt() -> Option<&'static InterruptDescriptorTable> {
    unsafe {
        IDT.as_ref().map(|idt| idt.get_idt())
    }
}

pub fn get_idt_mut() -> Option<&'static mut InterruptDescriptorTable> {
    unsafe {
        IDT.as_mut().map(|idt| idt.get_idt_mut())
    }
}

pub unsafe fn register_interrupt_handler(num: usize, handler_addr: VirtAddr) {
    if let Some(idt) = &mut IDT {
        idt.get_idt_mut()[num].set_handler_addr(handler_addr).set_present(true);
    }
}

pub unsafe fn reg_timer() {
    let timer = crate::interrupt::number::Interrupt::Timer;

    register_interrupt_handler(timer as usize, VirtAddr::new(super::interrupt::timer as u64))
}

