
use core::ptr::addr_of;

use libkloader::KernelInfo;

use crate::arch::{PAGE_SIZE, VirtAddr};

use spin::Once;

use super::vmm::VirtualRegion;

extern "C" {
    static __kernel_code_start: u8;
    static __kernel_code_end: u8;
    static __kernel_data_start: u8;
    static __kernel_data_end: u8;
}

static MEMORY_LAYOUT: Once<MemoryLayout> = Once::new();

#[derive(Debug)]
pub struct MemoryLayout {
    pub phys_memory_start: VirtAddr,
    pub phys_memory_size: u64,
    pub kernel_code_start: VirtAddr,
    pub kernel_code_end: VirtAddr,
    pub kernel_data_start: VirtAddr,
    pub kernel_data_end: VirtAddr,
    pub kernel_stack_area_base: VirtAddr,
    pub kernel_stack_area_end: VirtAddr,
}

impl MemoryLayout {
    fn to_region(start: VirtAddr, end: VirtAddr) -> VirtualRegion {
        let size = (end - start) / PAGE_SIZE as u64;
        VirtualRegion::new(start, size as usize)
    }

    pub fn kernel_phys_region(&self) -> VirtualRegion {
        VirtualRegion::new(self.phys_memory_start, self.phys_memory_size as usize)
    }

    pub fn kernel_code_region(&self) -> VirtualRegion {
        MemoryLayout::to_region(self.kernel_code_start, self.kernel_code_end)
    }

    pub fn kernel_data_region(&self) -> VirtualRegion {
        MemoryLayout::to_region(self.kernel_data_start, self.kernel_data_end)
    }

    pub fn kernel_stack_region(&self) -> VirtualRegion {
        MemoryLayout::to_region(self.kernel_stack_area_base, self.kernel_stack_area_end)
    }
}

/// Gather information about where the kernel is loaded and store it for later
pub fn init_memory_regions(bootinfo: &KernelInfo) {
    MEMORY_LAYOUT.call_once(|| {
            unsafe {
                MemoryLayout {
                    phys_memory_start: VirtAddr::new(bootinfo.phys_offset),
                    phys_memory_size: bootinfo.phys_mem_size,
                    kernel_code_start: VirtAddr::new(addr_of!(__kernel_code_start) as u64),
                    kernel_code_end: VirtAddr::new(addr_of!(__kernel_code_end) as u64),
                    kernel_data_start: VirtAddr::new(addr_of!(__kernel_data_start) as u64),
                    kernel_data_end: VirtAddr::new(addr_of!(__kernel_data_end) as u64),
                    kernel_stack_area_base: VirtAddr::new(bootinfo.rsp - (bootinfo.stack_size * PAGE_SIZE as u64)),
                    kernel_stack_area_end: VirtAddr::new(bootinfo.rsp),
                }
            }
        }
    );

}

/// Returns various constants about the address space
pub fn memory_layout() -> &'static MemoryLayout {
    MEMORY_LAYOUT.get().unwrap()
}

/// The start of the MMIO address space comes after the higher half physical memory range
pub fn mmio_area_start() -> VirtAddr {
    // Align to 2mb since we use huge frames
    (memory_layout().phys_memory_start + ((memory_layout().phys_memory_size + 1) * PAGE_SIZE as u64)).align_up(0x1000000u64)
}
