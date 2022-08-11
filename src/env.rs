use crate::arch::x86_64::paging::Mapper;
use crate::arch::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::mm::{VirtualRegion, phys_to_virt, init_phys_offset};

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
    pub frame_buffer_phys: PhysAddr,
    pub frame_buffer_page_size: usize,
    pub size: usize,
    pub height: usize,
    pub width: usize
}

pub fn init(bootinfo: &KernelInfo) {

    init_phys_offset(bootinfo.phys_offset as usize);

    // Copy the memory map to the heap
    let memory_map = bootinfo.mem_map_info.get_memory_map().to_vec().into_boxed_slice();

    init_memory_regions(&bootinfo);

    // Gather fb phys address
    let fb_vaddr = VirtAddr::new(bootinfo.video_info.frame_buffer as u64);
    let (frame_buffer_phys, is_huge) = fb_phys_addr(fb_vaddr);

    let frame_buffer_page_size = if is_huge {
        crate::arch::x86_64::PageSize::_2Mb.into()
    } else {
        PAGE_SIZE
    };

    KERNEL_ENV.call_once(|| {
        KernelEnv {
            memory_layout: memory_layout(),
            memory_map,

            #[cfg(target_arch="x86_64")]
            rsdp_base: bootinfo.acpi_info.rsdp_base as usize,

            video: Some(Video {
                frame_buffer: VirtAddr::new(bootinfo.video_info.frame_buffer as u64),
                frame_buffer_phys,
                frame_buffer_page_size,
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

/// Returns the physical address of the framebuffer and whether or not it uses a huge page
fn fb_phys_addr(fb: VirtAddr) -> (PhysAddr, bool) {
    log::info!("Getting fb phys addr");
    use x86_64::structures::paging::PageTable;
    let pt = unsafe {
        let paddr = x86_64::registers::control::Cr3::read_raw().0.start_address();
        let vaddr = phys_to_virt(paddr);
        let ptr = vaddr.as_mut_ptr() as *mut PageTable;
        ptr.as_mut().expect("Page table null ptr")
    };
    log::info!("pt: {:p}", pt);
    let res = Mapper::new(fb, pt).get_phys_frame();
    log::info!("{:?}", res);
    res.expect("Could not get the physical address of the frame buffer")
}

/// Returns various constants about the address space
pub fn memory_layout() -> &'static MemoryLayout {
    MEMORY_LAYOUT.get().unwrap()
}
