use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::{structures::gdt::{GlobalDescriptorTable, Descriptor}, registers::segmentation::{CS, Segment, SegmentSelector, SS}, PrivilegeLevel};

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

static mut KERNEL_CS_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static mut KERNEL_DS_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static mut USER_CS_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static mut USER_DS_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Initialize the GDT
pub fn init_gdt() {
    crate::println!("Creating kernel GDT");
    unsafe {
        let kernel_cs = GDT.add_entry(Descriptor::kernel_code_segment());
        let kernel_ds = GDT.add_entry(Descriptor::kernel_data_segment());
        let user_cs = GDT.add_entry(Descriptor::user_code_segment());
        let user_ds = GDT.add_entry(Descriptor::user_data_segment());

        KERNEL_CS_INDEX.store(kernel_cs.index() as usize, Ordering::SeqCst);
        KERNEL_DS_INDEX.store(kernel_ds.index() as usize, Ordering::SeqCst);
        USER_CS_INDEX.store(user_cs.index() as usize, Ordering::SeqCst);
        USER_DS_INDEX.store(user_ds.index() as usize, Ordering::SeqCst);
    }
}

/// Loads the kernel GDT and sets the segment registers
pub unsafe fn load_kernel_gdt() {
    crate::println!("Loading GDT");
    GDT.load();

    CS::set_reg(SegmentSelector::new(KERNEL_CS_INDEX.load(Ordering::SeqCst) as u16, PrivilegeLevel::Ring0));
    SS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX.load(Ordering::SeqCst) as u16, PrivilegeLevel::Ring0));
}
