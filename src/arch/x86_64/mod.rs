pub const PAGE_SIZE: usize = 4096;

use libkloader::KernelInfo;
pub use x86_64::{PhysAddr, VirtAddr};

pub mod cpu;
pub mod idt;
mod gdt;
pub mod paging;
mod pic;
mod pit;
pub mod smp;

/// Initialize as many platform components as we can here.
pub fn platform_init(bootinfo: &KernelInfo) {
    gdt::init_gdt();
    unsafe {
        gdt::load_kernel_gdt();
    }
    idt::init_idt().expect("Could not initialize IDT");
    crate::println!("Empty IDT initialized");

    smp::init_smp(bootinfo).expect("Could not initialize SMP");
}

/// Initialize as many platform components as we can here. Platform init but for ap's. Shared
/// components e.g. GDT should already be created by now they just need to be loaded.
pub(super) fn ap_init(lapic_id: usize) {
    unsafe {
        gdt::load_kernel_gdt();
    }
    idt::init_idt().expect("Could not load IDT on ap");
    smp::thread_local::init_thread_local(lapic_id);
}

/// Returns the apic id of the core the calls this
pub fn apic_id() -> u32 {
    smp::lapic::Lapic::new().id()
}
