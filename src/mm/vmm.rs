use core::ptr::Unique;
use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::structures::paging::PageTable;
use crate::arch::x86_64::{PageSize, PageTableOps};
use crate::arch::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::env::memory_layout;
use core::fmt::Debug;
use super::frame_allocator::FrameAllocator;

use super::mapping::{Mapping, MappingType};
use super::pmm::{phys_to_virt, physical_memory_manager, virt_to_phys};
use super::region::MemRegion;

use itertools::Itertools;

use lazy_static::lazy_static;

lazy_static! {
    static ref KERNEL_ADDRESS_SPACE: Mutex<AddressSpace> = Mutex::new(AddressSpace::empty());
}

/// A virtual region of memory. If the regions overlap in any form their `Ord` will return `Equal`.
/// Otherwise their `Ord` is as expected.
#[derive(Debug, Copy, Clone)]
pub struct VirtualRegion {
    pub start: VirtAddr,
    pub size: usize,
}

impl VirtualRegion {
    pub fn new(start: VirtAddr, size: usize) -> Self {
        Self {
            start,
            size,
        }
    }

    /// Returns an iterator of each starting virtual address in the range
    pub fn pages(&self) -> impl Iterator<Item = VirtAddr> {
        (self.start.as_u64()..self.region_end() as u64).step_by(PAGE_SIZE).map(|addr| VirtAddr::new(addr))
    }

    pub fn huge_pages(&self) -> impl Iterator<Item = VirtAddr> {
        let huge_page_size: usize = PageSize::_2Mb.into();
        (self.start.as_u64()..self.region_end() as u64).step_by(huge_page_size).map(|addr| VirtAddr::new(addr))

    }

    pub fn end(&self) -> VirtAddr {
        self.start + (self.size * PAGE_SIZE) as u64 - 1u64
    }

    pub fn exclusive_end(&self) -> VirtAddr {
        self.start + (self.size * PAGE_SIZE) as u64
    }
}

impl MemRegion for VirtualRegion {
    fn region_start(&self) -> usize {
        self.start.as_u64() as usize
    }

    fn region_end(&self) -> usize {
        let start = self.start.as_u64() as usize;
        start + (self.size * PAGE_SIZE)
    }
}

impl Ord for VirtualRegion {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if self.overlaps(other) || self.contains(other) || self.within(other) {
            core::cmp::Ordering::Equal
        } else {
            self.start.cmp(&other.start)
        }
    }
}

impl PartialOrd for VirtualRegion {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.start > other.end() {
            Some(core::cmp::Ordering::Greater)
        } else if self.end() < other.start {
            Some(core::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl PartialEq for VirtualRegion {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end() == other.end()
    }
}

impl Eq for VirtualRegion {}

type Error = VirtualMemoryError;

#[derive(Debug, Clone, Copy)]
pub enum VirtualMemoryError {
    NoAddressSpaceAvailable,
    RegionInUse(&'static str),
    NotPresent
}

/// An abstraction of OS memory book keeping and hardware memory management mechanisms
#[derive(Debug)]
pub struct AddressSpace {
    page_table: Unique<PageTable>,
    mappings: BTreeSet<Arc<Mapping>>,
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        crate::interrupt::without_interrupts(|| {
            self.remove_all().expect("Could not cleanup dropped address space");
        });
    }
}

impl Clone for AddressSpace {
    /// Clones the top most page table and the set of all mappings. Does not clone each subsequent
    /// page table.
    fn clone(&self) -> Self {
        let phys_frame = physical_memory_manager().lock().allocate_frame();
        let page_table_ptr: *mut PageTable = phys_to_virt(phys_frame).as_mut_ptr();
        unsafe {
            // Perform shallow page table clone
            /*
             for (dst, src) in page_table_ptr.as_mut().unwrap().iter_mut().zip(self.page_table.as_ref().iter()) {
                 dst.clone_from(src);
             }
            */

            // FIXME: Fix hardcodings here. We need to verify the lower address space is completely
            // empty
            //
            // shallow clone the last two entries of the page table where the kernel
            // mappings are located
            (*page_table_ptr)[510].clone_from(&self.page_table.as_ref()[510]);
            (*page_table_ptr)[511].clone_from(&self.page_table.as_ref()[511]);

            Self {
                page_table: Unique::new_unchecked(page_table_ptr),
                mappings: self.mappings.clone() 
            }
        }
    }
}

impl AddressSpace {
    fn empty() -> Self {
        Self {
            page_table: Unique::dangling(),
            mappings: BTreeSet::new()
        }
    }

    /// Clones the highest page table and its `Mapping`s. Clears the `Mapping`s writeable flag and
    /// sets a COW attribute that is checked during a write page fault
    pub fn new_copy_on_write_from(src: &AddressSpace) -> Self {
        let top_level_page_table = src.page_table_clone();

        let mappings = src.mappings.iter().cloned().map(|mapping| {
            unsafe {
                let mut new_mapping = (*mapping).clone();
                new_mapping.cow(top_level_page_table.as_mut().unwrap());
                Arc::new(new_mapping)
            }
        }).collect();

        Self {
            page_table: Unique::new(top_level_page_table).unwrap(),
            mappings,
        }
    }

    /// Clones the kernel address space but the result will not have access to the mappings resulting in an
    /// "empty" address space but has the correct kernel page table setup
    pub fn new_user_from_kernel() -> Self {
        let mut addr_space = get_kernel_context_virt().lock().clone();

        addr_space.mappings.clear();

        addr_space
    }

    pub fn page_table(&mut self) -> &mut PageTable {
        unsafe {
            self.page_table.as_mut()
        }
    }

    /// Clones the highest level page table
    fn page_table_clone(&self) -> *mut PageTable {
        // FIXME: return level 4 page table frame to pmm
        unsafe {
            self.page_table.as_ref().deep_copy()
        }
    }

    /// Returns the physical address of the backing page table
    pub fn phys_addr(&self) -> PhysAddr {
        virt_to_phys(VirtAddr::new(self.page_table.as_ptr() as u64))
    }

    /// Query the address space if a particular address is valid
    pub fn address_mapped(&self, addr: VirtAddr) -> bool {
        for mapping in self.mappings.iter() {
            if mapping.virt_range().contains_val(addr.as_u64() as usize) {
                return true
            }
        }
        false
    }

    /// Remove all mappings from the user address space and attempt to unmap them if we own the
    /// sole reference
    pub fn remove_all(&mut self) -> Result<(), Error> {
        while let Some(m) = self.mappings.pop_first() {
            // FIXME: this will not unmapped shared memory even if that is the desired behavior
            if let Ok(mapping) = Arc::try_unwrap(m) {
                let range = mapping.virt_range().clone();
                mapping.unmap(self.page_table(), &mut *physical_memory_manager().lock(), true).unwrap();
            }
        }

        Ok(())
    }

    pub(super) fn insert_and_map(&mut self, mut mapping: Mapping, frame_allocator: &mut impl FrameAllocator) -> Result<Arc<Mapping>, Error> {
        // FIXME: Should throw an error if attempting to overwrite
        mapping.map(self.page_table(), frame_allocator).unwrap();
        self.insert_mapping(mapping)
    }

    pub(super) fn insert_mapping(&mut self, mapping: Mapping) -> Result<Arc<Mapping>, Error> {
        let arc_mapping = Arc::new(mapping);
        self.mappings.insert(arc_mapping.clone()).then_some(arc_mapping).ok_or(VirtualMemoryError::RegionInUse(""))
    }

    /// Removes and unmaps the region containing the region provided as arguments
    pub fn release_region(&mut self, vaddr: VirtAddr, size: usize, frame_allocator: &mut impl FrameAllocator) -> Result<(), VirtualMemoryError> {
        let empty_region = &Mapping::new_empty(VirtualRegion::new(vaddr, size));
        let region = self.mappings.take(empty_region).ok_or(VirtualMemoryError::NotPresent)?;

        // TODO: RAII unmapping for shared memory
        // Attempt to destroy the mapping
        let _ = Arc::try_unwrap(region).map(|r| r.unmap(self.page_table(), frame_allocator, true));
        Ok(())
    }

    /// Retrieves the mapping containing the specified address
    pub fn mapping_containing(&self, addr: VirtAddr) -> Option<Arc<Mapping>> {
        self.mappings.get(&Mapping::from_address(addr)).map(Arc::clone)
    }

    /// Sets the entire region containing the address to read/write permissions
    pub fn set_region_readwrite(&mut self, addr: VirtAddr) -> Result<(), Error> {
        let mapping = self.mapping_containing(addr).ok_or(VirtualMemoryError::NotPresent)?;
        mapping.read_write(self);
        Ok(())
    }

    /// Sets the entire region containing the address to readonly permissions
    pub fn set_region_readonly(&mut self, addr: VirtAddr) -> Result<(), Error> {
        let mapping = self.mapping_containing(addr).ok_or(VirtualMemoryError::NotPresent)?;
        mapping.read_only(self);
        Ok(())
    }

    /// Sets the entire region containing the address to executable permissions
    pub fn set_region_executable(&mut self, addr: VirtAddr) -> Result<(), Error> {
        let mapping = self.mapping_containing(addr).ok_or(VirtualMemoryError::NotPresent)?;
        mapping.executable(self);
        Ok(())
    }

    /// Find a suitable region at or above the provided address
    pub fn first_available_addr_above(&self, addr: VirtAddr, size: usize) -> Option<VirtualRegion> {
        let region = VirtualRegion::new(addr, size);

        if self.mappings.get(&Mapping::new_empty(region)).is_none() {
            return Some(region)
        }

        for (left, right) in self.mappings.range(Mapping::from_address(addr)..).tuple_windows() {
            if left.virt_range().contiguous(right.virt_range()) {
                continue;
            }

            let hole = (right.virt_range().start - left.virt_range().exclusive_end()) / PAGE_SIZE as u64;

            if hole >= size as u64 {
                return Some(VirtualRegion {
                    start: left.virt_range().exclusive_end(),
                    size
                })
            }
        }
        None
    }
}

pub fn init_vmm() {
    let pml4 = x86_64::registers::control::Cr3::read().0;

    // Construct the kernel address space
    let kas = {
        let paddr = pml4.start_address();
        Unique::new(phys_to_virt(paddr).as_mut_ptr()).unwrap()
    };

    // Store it within the lock
    let mut lock = KERNEL_ADDRESS_SPACE.lock();
    lock.page_table = kas;

    // TODO: move this to a method
    let kernel_code = Mapping::existing(memory_layout().kernel_code_region(), MappingType::KernelCode);
    let kernel_data = Mapping::existing(memory_layout().kernel_data_region(), MappingType::KernelData);
    let kernel_stack = Mapping::existing(memory_layout().kernel_stack_region(), MappingType::KernelData);
    let kernel_phys = Mapping::existing(memory_layout().kernel_phys_region(), MappingType::KernelData);

    lock.insert_mapping(kernel_code).unwrap();
    lock.insert_mapping(kernel_data).unwrap();
    lock.insert_mapping(kernel_stack).unwrap();
    lock.insert_mapping(kernel_phys).unwrap();
}

/// The start of the MMIO address space comes after the higher half physical memory range
pub fn mmio_area_start() -> VirtAddr {
    // Align to 2mb since we use huge frames
    (memory_layout().phys_memory_start + ((memory_layout().phys_memory_size + 1) * PAGE_SIZE as u64)).align_up(0x1000000u64)
}

pub fn get_kernel_context_virt() -> &'static Mutex<AddressSpace> {
    &KERNEL_ADDRESS_SPACE
}

#[cfg(test)]
mod test {
    /*
    use super::*;
    use crate::arch::PAGE_SIZE;

    /// The VMM should not allow caller to reserve a region in use.
    #[test_case]
    fn test_vmm_overlap_reject() {
        let test_region = VirtAddr::new(0);
        let region_size = 4;
        let within_test_region = test_region + 0x1000u64;
        let adjacent_region_start = test_region + region_size as u64 * PAGE_SIZE as u64;

        let mut vmm = AddressSpace::empty();

        assert!(vmm
            .reserve_region(test_region, region_size)
            .is_ok());
        // Assert reservation of same region is rejected
        assert!(
            vmm
                .reserve_region(test_region, region_size)
                .is_err(),
            "VMM did not reject already reserved region"
        );
        assert!(
            vmm
                .reserve_region(test_region, region_size + 1)
                .is_err(),
            "VMM did not reject reserved super-region"
        );
        assert!(
            vmm
                .reserve_region(test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region"
        );
        assert!(
            vmm
                .reserve_region(within_test_region, 1)
                .is_err(),
            "VMM did not reject reserved sub-region with different starts"
        );
        assert!(
            vmm
                .reserve_region(within_test_region, region_size)
                .is_err(),
            "VMM did not reject overlapping region"
        );

        assert!(vmm.reserve_region(adjacent_region_start, 1).is_ok(), "unable to reserve adjacent region. incorrect rejection");
    }
    */
}
