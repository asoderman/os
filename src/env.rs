use crate::{arch::{VirtAddr, PAGE_SIZE}, mm::VirtualRegion};

use core::ptr::addr_of;
use alloc::boxed::Box;
use libkloader::{KernelInfo, MemoryDescriptor};
use spin::Once;

static KERNEL_ENV: Once<KernelEnv> = Once::new();

#[derive(Debug)]
pub struct KernelEnv {
    pub memory_layout: &'static MemoryLayout,
    pub memory_map: Box<[MemoryDescriptor]>,

    #[cfg(target_arch="x86_64")]
    pub rsdp_base: usize,

    pub video: Option<Video>
}

#[derive(Debug)]
pub struct Video {
    pub frame_buffer: VirtAddr,
    pub size: usize,
    pub height: usize,
    pub width: usize
}

pub fn init(bootinfo: &KernelInfo) {
    // Copy the memory map to the heap
    let memory_map = bootinfo.mem_map_info.get_memory_map().to_vec().into_boxed_slice();

    init_memory_regions(&bootinfo);

    KERNEL_ENV.call_once(|| {
        KernelEnv {
            memory_layout: memory_layout(),
            memory_map,

            #[cfg(target_arch="x86_64")]
            rsdp_base: bootinfo.acpi_info.rsdp_base as usize,

            video: Some(Video {
                frame_buffer: VirtAddr::new(bootinfo.video_info.frame_buffer as u64),
                size: bootinfo.video_info.frame_buffer_size as usize,
                height: bootinfo.video_info.height as usize,
                width: bootinfo.video_info.width as usize
            }),
        }
    });
}

pub fn env<'env>() -> &'env KernelEnv {
    KERNEL_ENV.get().expect("KernelEnv not initialized!")
}
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
fn init_memory_regions(bootinfo: &KernelInfo) {
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