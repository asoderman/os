mod error;
pub mod frame_allocator;
mod mapping;
pub mod memory;
mod pmm;
mod region;
mod vmm;

use crate::arch::{PhysAddr, VirtAddr};
use libkloader::KernelInfo;
use spin::{Mutex, MutexGuard};
use vmm::init_vmm;

pub use pmm::phys_to_virt;
pub use pmm::get_init_heap_section;
pub use error::MemoryManagerError;

use self::mapping::Mapping;
use self::pmm::BitMapFrameAllocator;
use self::vmm::{VirtualMemoryManager, VirtualMemoryError, VirtualRegion};
pub use self::vmm::get_kernel_context_virt;
pub use self::pmm::{write_physical, write_physical_slice, get_phys_as_mut};

use lazy_static::lazy_static;

lazy_static! {
    static ref MM: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());
}

type Error = MemoryManagerError;

#[derive(Debug)]
pub struct MemoryManager {
    vmm: VirtualMemoryManager,
    pmm: BitMapFrameAllocator

}

impl MemoryManager {
    fn new() -> Self {
        MemoryManager {
            vmm: VirtualMemoryManager::default(),
            pmm: BitMapFrameAllocator::uninit(),
        }
    }

    /// Identity maps a physical page into the virtual address space if it is available
    pub fn k_identity_map(&mut self, paddr: PhysAddr, size: usize) -> Result<(), Error> {
        let frame = self.pmm.request_frame(paddr)?;
        assert_eq!(frame, paddr);
        let mapping = mapping::Mapping::new_identity(frame);
        self.vmm.insert_and_map(mapping, &mut self.pmm)?;
        Ok(())

    }

    /// Map a writable page into the kernel context
    pub fn kmap(&mut self, vaddr: VirtAddr, pages: usize) -> Result<(), Error> {
        // TODO: make kernel specific only
        let region = VirtualRegion::new(vaddr, pages);
        let mapping = Mapping::new(region);
        // TODO: error handling
        self.vmm.insert_and_map(mapping, &mut self.pmm).unwrap();
        Ok(())
    }

    pub fn kunmap(&mut self, vaddr: VirtAddr) -> Result<(), VirtualMemoryError> {
        self.unmap_region(vaddr, 1)
    }

    pub fn unmap_region(&mut self, vaddr: VirtAddr, size: usize) -> Result<(), VirtualMemoryError> {
        self.vmm.release_region(vaddr, size, &mut self.pmm)
    }

    /// Identity map the provided physical address. Does not check if the memory is available for
    /// use but checks if the memory is within bounds of the entire physical memory
    pub unsafe fn kmap_identity_mmio(&mut self, paddr: PhysAddr, size: usize) -> Result<(), MemoryManagerError> {
        self.kmap_mmio(paddr, VirtAddr::new(paddr.as_u64()), size)?;

        Ok(())
    }

    pub unsafe fn kmap_mmio(&mut self, paddr: PhysAddr, vaddr: VirtAddr, size: usize) -> Result<(), MemoryManagerError> {
        let region = vmm::VirtualRegion::new(vaddr, size);
        let mapping = mapping::Mapping::new_mmio(region, paddr);
        self.vmm.insert_and_map(mapping, &mut self.pmm)?;

        Ok(())
    }

    pub unsafe fn kmap_mmio_anywhere(&mut self, paddr: PhysAddr, size: usize) -> Result<VirtAddr, MemoryManagerError> {
        let region = self.vmm.first_available_addr_above(memory::mmio_area_start(), size).ok_or(VirtualMemoryError::NoAddressSpaceAvailable)?;
        self.kmap_mmio(paddr, region.start, size)?;
        Ok(region.start)
    }

    pub fn init_pmm(&mut self, heap_range: (VirtAddr, VirtAddr), bootinfo: &KernelInfo) {
        pmm::init_phys_offset(bootinfo.phys_offset as usize);
        let mem_map = bootinfo.mem_map_info.get_memory_map();
        self.pmm.init(mem_map, heap_range.0, heap_range.1, bootinfo.phys_offset as usize);
    }
}

/// Locks the global MemoryManager.
pub fn memory_manager() -> MutexGuard<'static, MemoryManager> {
    MM.lock()
}

pub fn init(heap_range: (VirtAddr, VirtAddr), bootinfo: &KernelInfo) {
    MM.lock().init_pmm(heap_range, bootinfo);
    init_vmm();
}

pub fn temp_page(vaddr: VirtAddr) -> TempPageGuard {
    TempPageGuard(vaddr)
}

/// RAII guard for a temporary page. When this struct is dropped the page is unmapped.
#[derive(Debug)]
pub struct TempPageGuard(VirtAddr);

impl TempPageGuard {
    pub fn as_u64(&self) -> u64 {
        self.0.as_u64()
    }
}

impl Drop for TempPageGuard {
    fn drop(&mut self) {
        MM.lock().kunmap(self.0).unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::arch::PAGE_SIZE;
    use crate::arch::x86_64::paging::Mapper;

    use super::frame_allocator::FrameAllocator;

    #[test_case]
    fn test_pmm_alloc_and_free() {
        // Take lock for duration of test
        let mut mm = MM.lock();

        let starting_frame_count = mm.pmm.free_frames();
        let frame = mm.pmm.allocate_frame();
        let frame2 = mm.pmm.allocate_frame();
        let frame_count_after_alloc = mm.pmm.free_frames();

        mm.pmm.deallocate_frame(frame);
        mm.pmm.deallocate_frame(frame2);
        let frame_count_after_free = mm.pmm.free_frames();

        assert_ne!(frame, frame2);
        assert_ne!(starting_frame_count, frame_count_after_alloc);
        assert_eq!(starting_frame_count, frame_count_after_free);
    }

    /// The VMM should not allow caller to reserve a region in use.
    #[test_case]
    fn test_vmm_overlap_reject() {
        // TODO: do cleanup
        let test_region = VirtAddr::new(0);
        let region_size = 4;
        let within_test_region = test_region + 0x1000u64;
        let adjacent_region_start = test_region + region_size as u64 * PAGE_SIZE as u64;

        // Take lock for duration of test 
        let mut mm = MM.lock();
        assert!(mm.vmm
            .reserve_region(test_region, region_size)
            .is_ok());
        // Assert reservation of same region is rejected
        assert!(
            mm.vmm
                .reserve_region(test_region, region_size)
                .is_err(),
            "VMM did not reject already reserved region"
        );
        assert!(
            mm.vmm
                .reserve_region(test_region, region_size + 1)
                .is_err(),
            "VMM did not reject reserved super-region"
        );
        assert!(
            mm.vmm
                .reserve_region(test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region"
        );
        assert!(
            mm.vmm
                .reserve_region(within_test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region with different starts"
        );
        assert!(
            mm.vmm
                .reserve_region(within_test_region, region_size)
                .is_err(),
            "VMM did not reject overlapping region"
        );

        assert!(mm.vmm.reserve_region(adjacent_region_start, 1).is_ok(), "unable to reserve adjacent region. incorrect rejection");
    }

    #[test_case]
    fn test_kmap_kunmap() {
        let test_addr = VirtAddr::new(0x10000);
        // take lock for duration of the test 
        let mut mm = MM.lock();
        // take the free frames before touching any memory
        let pmm_frames = mm.pmm.free_frames();

        let kmap_result = mm.kmap(test_addr, 2);
        assert!(kmap_result.is_ok());

        unsafe { 
            (test_addr.as_mut_ptr() as *mut bool).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool));
            (test_addr.as_mut_ptr() as *mut bool).add(0x1000).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool).add(0x1000));
        }


        let mut test_pt_walker = unsafe {
            Mapper::new(test_addr, get_kernel_context_virt().unwrap().as_mut())
        };
        let kunmap_result = mm.kunmap(test_addr);

        assert!(kunmap_result.is_ok());

        let pmm_frames_after_unmap = mm.pmm.free_frames();

        test_pt_walker.walk();
        assert!(test_pt_walker.next_entry().is_unused());
        assert_eq!(pmm_frames, pmm_frames_after_unmap);
    }

    #[test_case]
    fn test_temp_page() {
        // TODO: is there a way to test this while holding the lock the entire time?
        // If a region is freed before we drop the temp page the test will fail
        let test_addr = VirtAddr::new(0x10000);
        let phys_frames = MM.lock().pmm.free_frames();
        MM.lock().kmap(test_addr, 1).expect("temp page test fail");
        let temp_page_result = temp_page(test_addr);

        drop(temp_page_result);

        let phys_frames_after_drop = MM.lock().pmm.free_frames();

        assert_eq!(phys_frames, phys_frames_after_drop);
    }
}
