use spin::Once;
use x86_64::{structures::gdt::{GlobalDescriptorTable, Descriptor}, registers::segmentation::{CS, Segment, SegmentSelector, SS, DS, ES, GS, FS}, PrivilegeLevel, VirtAddr};
use x86_64::instructions::tables::load_tss;

use super::smp::thread_local::ProcessorControlBlock;

static mut BASE_GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

#[thread_local]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

pub static KERNEL_CS_INDEX: Once<u16> = Once::new();
pub static KERNEL_DS_INDEX: Once<u16> = Once::new();
static KERNEL_TLS_INDEX: Once<u16> = Once::new();
pub static USER_CS_INDEX: Once<u16> = Once::new();
pub static USER_DS_INDEX: Once<u16> = Once::new();

/// Initialize the base GDT
pub fn init_base_gdt() {
    unsafe {
        let kernel_cs = BASE_GDT.add_entry(Descriptor::kernel_code_segment());
        let kernel_ds = BASE_GDT.add_entry(Descriptor::kernel_data_segment());
        let kernel_tls = BASE_GDT.add_entry(Descriptor::kernel_data_segment());
        let user_cs = BASE_GDT.add_entry(Descriptor::user_code_segment());
        let user_ds = BASE_GDT.add_entry(Descriptor::user_data_segment());

        KERNEL_CS_INDEX.call_once(|| kernel_cs.index());
        KERNEL_DS_INDEX.call_once(||kernel_ds.index());
        KERNEL_TLS_INDEX.call_once(||kernel_tls.index());
        USER_CS_INDEX.call_once(||user_cs.index());
        USER_DS_INDEX.call_once(|| user_ds.index());
    }
}

/// Loads the kernel GDT and sets the segment registers
pub unsafe fn load_kernel_gdt() {
    BASE_GDT.load();

    set_segment_regs();
}

/// Loads a per cpu GDT which is identical to the base GDT except it can contain a TSS
///
/// This must be called after thread locals are initialized
pub unsafe fn load_per_cpu_gdt() {
    GDT.clone_from(&BASE_GDT);
    let tss_selector = GDT.add_entry(Descriptor::tss_segment(&ProcessorControlBlock::get().tss));

    GDT.load();

    load_tss(tss_selector);
}

pub unsafe fn set_segment_regs() {
    CS::set_reg(SegmentSelector::new(*KERNEL_CS_INDEX.get_unchecked(), PrivilegeLevel::Ring0));
    SS::set_reg(SegmentSelector::new(*KERNEL_DS_INDEX.get_unchecked(), PrivilegeLevel::Ring0));
    DS::set_reg(SegmentSelector::new(*KERNEL_DS_INDEX.get_unchecked(), PrivilegeLevel::Ring0));
    ES::set_reg(SegmentSelector::new(0, PrivilegeLevel::Ring0));
    GS::set_reg(SegmentSelector::new(*KERNEL_TLS_INDEX.get_unchecked(), PrivilegeLevel::Ring0));
    FS::set_reg(SegmentSelector::new(*KERNEL_TLS_INDEX.get_unchecked(), PrivilegeLevel::Ring0));
}

/// Sets the (kernel) stack pointer to be loaded on privilege change
pub fn set_tss_rsp0(rsp: VirtAddr) {
    ProcessorControlBlock::get().tss.privilege_stack_table[0] = rsp;
}
