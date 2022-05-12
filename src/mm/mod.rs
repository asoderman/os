mod error;
mod vmm;
mod pmm;
mod region;

use crate::arch::PAGE_SIZE;
use crate::arch::x86_64::paging::Mapper;
use crate::arch::{PhysAddr, VirtAddr, x86_64::paging::MapError};
use libkloader::KernelInfo;
use spin::{Mutex, MutexGuard};
use vmm::init_vmm;
use x86_64::structures::paging::PageTableFlags;

pub use pmm::phys_to_virt;
pub use pmm::get_init_heap_section;
pub use error::MemoryManagerError;

use self::pmm::PhysicalMemoryManager;
use self::vmm::{VirtualMemoryManager, VirtualMemoryError};
pub use self::vmm::get_kernel_context_virt;

use lazy_static::lazy_static;

lazy_static! {
    static ref MM: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());
}

type Error = MemoryManagerError;

#[derive(Debug)]
pub struct MemoryManager {
    vmm: VirtualMemoryManager,
    pmm: PhysicalMemoryManager

}

impl MemoryManager {
    fn new() -> Self {
        MemoryManager {
            vmm: VirtualMemoryManager::default(),
            pmm: PhysicalMemoryManager::uninit(),
        }
    }

    /// Request a physical frame from the pmm
    pub fn request_frame(&mut self) -> PhysAddr {
        self.pmm.request_frame()
    }

    pub fn request_frame_at(&mut self, paddr: PhysAddr, size: usize) -> Option<PhysAddr> {
        self.pmm.request_range(paddr, size)
    }

    /// Returns a physical frame to the pmm
    ///
    /// # Unsafety
    /// This method is unsafe because there are no checks if the memory is still in use or if the
    /// memory is already free.
    pub unsafe fn release_frame(&mut self, paddr: PhysAddr) {
        self.pmm.release_frame(paddr)
    }

    /// Mutably borrow the memory at the provided PhysAddr
    pub unsafe fn get_phys_as_mut<T>(&self, paddr: PhysAddr) -> Option<&mut T> {
        self.pmm.get_phys_as_mut(paddr)
    }

    /// Identity maps a physical page into the virtual address space if it is available
    pub fn k_identity_map(&mut self, paddr: PhysAddr, size: usize) -> Result<(), Error> {
        let _frames = self.pmm.request_range(paddr, size).ok_or(VirtualMemoryError::RegionInUse("PMM: could not reserve requested range"));
        if let Err(e) = _frames {
            self.pmm.request_low_memory(paddr, size).ok_or(e)?;
        }

        unsafe {
            self.map_unchecked(paddr, size)
        }

    }

    /// Map the specified frame to the specified page.
    pub fn map_frame_to_page(&mut self, frame: PhysAddr, page: VirtAddr) -> Result<VirtAddr, Error> {
        self.vmm.reserve_region(page, 1)?;
        let kernel_pt = unsafe {
            get_kernel_context_virt().unwrap().as_mut()
        };
        // TODO: real error handling
        Mapper::new(page, kernel_pt).map_frame(frame, self).expect("Map error");
        Ok(page)
    }

    /// Map a writable page into the kernel context
    pub fn kmap(&mut self, vaddr: VirtAddr, pages: usize) -> Result<(), Error> {
        let kernel_context = unsafe {
            get_kernel_context_virt().expect("kmap unable to get kernel context").as_mut()
        };
        self.vmm.reserve_region(vaddr, pages)?;

        let mut walker = Mapper::new(vaddr, kernel_context);

        loop {
            match walker.advance() {
                Err(MapError::BottomLevel) => { break; },
                Err(MapError::NotPresent) => { 
                    walker.map_next(self).unwrap();
                    walker.next_entry().set_flags(PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                },
                Err(MapError::HugeFrame) => { Err(VirtualMemoryError::RegionInUse("Attempted to kmap huge frame"))? },
                _ => {
                    walker.next_entry().set_flags(PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                }
            }
        }

        if pages > 1 {
            walker.map_adjacent(pages, self);
        }

        x86_64::instructions::tlb::flush(vaddr);
        Ok(())
    }

    pub fn kunmap(&mut self, vaddr: VirtAddr) -> Result<(), Error> {
        let frame = self.unmap(vaddr)?;
        self.pmm.release_frame(frame);
        self.vmm.release_region(vaddr, 1);
        x86_64::instructions::tlb::flush(vaddr);
        Ok(())
    }

    /// Unmaps the specified virtual address but does not return the underlying physical frame to
    /// the pmm. The physical frame backing a freed page table will still be returned to the pmm
    pub fn kunmap_untracked(&mut self, vaddr: VirtAddr) {

        let _ = self.unmap(vaddr);
        self.vmm.release_region(vaddr, 1);
        x86_64::instructions::tlb::flush(vaddr);
    }

    fn unmap(&mut self, vaddr: VirtAddr) -> Result<PhysAddr, VirtualMemoryError> {
        let kernel_context = unsafe {
            get_kernel_context_virt().expect("kmap unable to get kernel context").as_mut()
        };

        let mut walker = Mapper::new(vaddr, kernel_context);

        loop {
            match walker.advance() {
                Err(MapError::BottomLevel) => break,
                Err(MapError::NotPresent) => Err(VirtualMemoryError::UnmapNonPresent)?,
                _ => ()
            }
        }

        let frame = walker.unmap_next().map_err(|_| VirtualMemoryError::UnmapNonPresent)?;
        Ok(frame)
    }

    pub fn temp_page(&mut self, vaddr: VirtAddr) -> Result<TempPageGuard, Error> {
        self.kmap(vaddr, 1)?;
        Ok(TempPageGuard(vaddr))
    }

    /// Identity map the provided physical address. Does not check if the memory is available for
    /// use but checks if the memory is within bounds of the entire physical memory
    pub unsafe fn map_unchecked(&mut self, paddr: PhysAddr, size: usize) -> Result<(), MemoryManagerError> {
        let start = paddr.as_u64() as usize;
        let range = (start..(start + size * PAGE_SIZE)).step_by(PAGE_SIZE);

        for f in range {
            let vaddr = VirtAddr::new(f as u64);
            let paddr = PhysAddr::new(f as u64);

            self.map_frame_to_page(paddr, vaddr)?;
        }

        Ok(())
    }

    pub fn init_pmm(&mut self, heap_range: (VirtAddr, VirtAddr), bootinfo: &KernelInfo) {
        pmm::init_phys_offset(bootinfo.phys_offset as usize);
        // TODO: pass the kernel phys location via KernelInfo 
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

    #[test_case]
    fn test_pmm_alloc_and_free() {
        let get_count  = || { MM.lock().pmm.frame_count() };

        let starting_frames_in_alloc = get_count();
        let frame = MM.lock().pmm.request_frame();
        let frame2 = MM.lock().pmm.request_frame();
        let frame_count_after_alloc = get_count();

        MM.lock().pmm.release_frame(frame);
        MM.lock().pmm.release_frame(frame2);
        let frame_count_after_free = get_count();

        assert_ne!(frame, frame2);
        // TODO: Split these into two seperate test cases 
        if starting_frames_in_alloc > 1 {
            // If a fill does not occur
            assert_eq!(starting_frames_in_alloc - 2, frame_count_after_alloc);
            assert_eq!(starting_frames_in_alloc, frame_count_after_free);
        } else {
            // Fill occured
            assert!(starting_frames_in_alloc <= frame_count_after_alloc);
            assert_eq!(frame_count_after_alloc + 2, frame_count_after_free);
        }
    }

    /// The VMM should not allow caller to reserve a region in use.
    #[test_case]
    fn test_vmm_overlap_reject() {
        // TODO: do cleanup
        let test_region = VirtAddr::new(0);
        let region_size = 4;
        let within_test_region = test_region + 0x1000u64;
        let adjacent_region_start = test_region + region_size as u64 * PAGE_SIZE as u64;
        assert!(MM.lock().vmm
            .reserve_region(test_region, region_size)
            .is_ok());
        // Assert reservation of same region is rejected
        assert!(
            MM.lock().vmm
                .reserve_region(test_region, region_size)
                .is_err(),
            "VMM did not reject already reserved region"
        );
        assert!(
            MM.lock().vmm
                .reserve_region(test_region, region_size + 1)
                .is_err(),
            "VMM did not reject reserved super-region"
        );
        assert!(
            MM.lock().vmm
                .reserve_region(test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region"
        );
        assert!(
            MM.lock().vmm
                .reserve_region(within_test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region with different starts"
        );
        assert!(
            MM.lock().vmm
                .reserve_region(within_test_region, region_size)
                .is_err(),
            "VMM did not reject overlapping region"
        );

        assert!(MM.lock().vmm.reserve_region(adjacent_region_start, 1).is_ok(), "unable to reserve adjacent region. incorrect rejection");
    }

    #[test_case]
    fn test_kmap_kunmap() {
        let test_addr = VirtAddr::new(0x10000);
        let kmap_result = MM.lock().kmap(test_addr, 2);
        assert!(kmap_result.is_ok());
        unsafe { 
            (test_addr.as_mut_ptr() as *mut bool).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool));
            (test_addr.as_mut_ptr() as *mut bool).add(0x1000).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool).add(0x1000));
        }

        let pmm_frames = MM.lock().pmm.free_frames();

        let mut test_pt_walker = unsafe {
            Mapper::new(test_addr, get_kernel_context_virt().unwrap().as_mut())
        };
        let kunmap_result = MM.lock().kunmap(test_addr);

        assert!(kunmap_result.is_ok());

        let pmm_frames_after_unmap = MM.lock().pmm.free_frames();

        test_pt_walker.walk();
        assert!(test_pt_walker.next_entry().is_unused());
        // TODO: unmap only free a single page/frame this will need to be changed once it frees an
        // entire range
        assert_eq!(pmm_frames + 1, pmm_frames_after_unmap);
    }

    #[test_case]
    fn test_temp_page() {
        let test_addr = VirtAddr::new(0x10000);
        let temp_page_result = MM.lock().temp_page(test_addr);

        assert!(temp_page_result.is_ok());

        let phys_frames = MM.lock().pmm.free_frames();
        drop(temp_page_result.unwrap());

        let phys_frames_after_drop = MM.lock().pmm.free_frames();

        assert_eq!(phys_frames + 1, phys_frames_after_drop);
    }
}
