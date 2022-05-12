pub const PAGE_SIZE: usize = 4096;

pub use x86_64::{PhysAddr, VirtAddr};

pub mod cpu;
pub mod idt;
mod gdt;
pub mod paging;
mod pic;
mod pit;
pub mod smp;

pub fn platform_init() {
    gdt::init_gdt();
    unsafe {
        gdt::load_kernel_gdt();
    }
}

/// Returns the apic id of the core the calls this
pub fn apic_id() -> u32 {
    smp::lapic::read_apic_id_mmio()
}
