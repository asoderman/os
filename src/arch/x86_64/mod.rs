pub const PAGE_SIZE: usize = 4096;

use libkloader::KernelInfo;
pub use x86_64::{PhysAddr, VirtAddr};

pub mod context;
pub mod cpu;
pub mod idt;
#[macro_use]
pub mod interrupt;
mod gdt;
mod lapic_timer;
pub mod paging;
mod pic;
mod pit;
pub mod smp;
mod syscall;
pub mod timers;

pub use gdt::set_tss_rsp0;

/// Initialize as many platform components as we can here.
pub fn platform_init(bootinfo: &KernelInfo) {
    gdt::init_base_gdt();
    unsafe {
        gdt::load_kernel_gdt();
    }
    idt::init_idt().expect("Could not initialize IDT");
    crate::println!("Empty IDT initialized");

    smp::init_smp(bootinfo).expect("Could not initialize SMP");

    unsafe {
        gdt::load_per_cpu_gdt();
    }

    syscall::init_syscall();
}

/// Initialize as many platform components as we can here. Platform init but for ap's. Shared
/// components e.g. GDT should already be created by now they just need to be loaded.
pub(super) fn ap_init(lapic_id: usize) {
    unsafe {
        gdt::load_kernel_gdt();
    }
    idt::init_idt().expect("Could not load IDT on ap");
    smp::init_smp_ap(lapic_id);
    unsafe {
        gdt::load_per_cpu_gdt();
    }
    syscall::init_syscall();
}

/// Returns the apic id of the core the calls this
pub fn apic_id() -> u32 {
    smp::CpuLocals::try_get().map(|local| local.lapic_id as u32).unwrap_or_else(|| {
        smp::lapic::Lapic::new().id()
    })
}
