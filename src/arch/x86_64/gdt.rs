use core::sync::atomic::{Ordering, AtomicU16};

use x86_64::{structures::gdt::{GlobalDescriptorTable, Descriptor}, registers::segmentation::{CS, Segment, SegmentSelector, SS, DS, ES, GS, FS}, PrivilegeLevel};

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

static mut KERNEL_CS_INDEX: AtomicU16 = AtomicU16::new(u16::MAX);
static mut KERNEL_DS_INDEX: AtomicU16 = AtomicU16::new(u16::MAX);
static mut KERNEL_TLS_INDEX: AtomicU16 = AtomicU16::new(u16::MAX);
static mut USER_CS_INDEX: AtomicU16 = AtomicU16::new(u16::MAX);
static mut USER_DS_INDEX: AtomicU16 = AtomicU16::new(u16::MAX);

extern "C" {
    static mut __tdata_start: u8;
    static mut __tbss_start: u8;
    static mut __tbss_end: u8;
}

/// Initialize the GDT
pub fn init_gdt() {
    crate::println!("Creating kernel GDT");
    unsafe {
        let kernel_cs = GDT.add_entry(Descriptor::kernel_code_segment());
        let kernel_ds = GDT.add_entry(Descriptor::kernel_data_segment());
        let kernel_tls = GDT.add_entry(Descriptor::kernel_data_segment());
        let user_cs = GDT.add_entry(Descriptor::user_code_segment());
        let user_ds = GDT.add_entry(Descriptor::user_data_segment());

        KERNEL_CS_INDEX.store(kernel_cs.index(), Ordering::SeqCst);
        KERNEL_DS_INDEX.store(kernel_ds.index(), Ordering::SeqCst);
        KERNEL_TLS_INDEX.store(kernel_tls.index(), Ordering::SeqCst);
        USER_CS_INDEX.store(user_cs.index(), Ordering::SeqCst);
        USER_DS_INDEX.store(user_ds.index(), Ordering::SeqCst);
    }
}

/// Loads the kernel GDT and sets the segment registers
pub unsafe fn load_kernel_gdt() {
    crate::println!("Loading GDT");
    GDT.load();

    CS::set_reg(SegmentSelector::new(KERNEL_CS_INDEX.load(Ordering::SeqCst) as u16, PrivilegeLevel::Ring0));
    SS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX.load(Ordering::SeqCst) as u16, PrivilegeLevel::Ring0));
    DS::set_reg(SegmentSelector::new(0, PrivilegeLevel::Ring0));
    ES::set_reg(SegmentSelector::new(0, PrivilegeLevel::Ring0));
    GS::set_reg(SegmentSelector::new(0, PrivilegeLevel::Ring0));
    FS::set_reg(SegmentSelector::new(KERNEL_TLS_INDEX.load(Ordering::SeqCst), PrivilegeLevel::Ring0));
}
