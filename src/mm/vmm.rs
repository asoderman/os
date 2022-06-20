use core::ptr::NonNull;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering;
use alloc::collections::BTreeSet;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTable;
use crate::arch::x86_64::VirtAddr;
use crate::arch::PAGE_SIZE;
use crate::env::memory_layout;
use core::fmt::Debug;
use super::frame_allocator::FrameAllocator;
use super::mapping::Mapping;
use super::mapping::MappingType;
use super::pmm::phys_to_virt;
use super::region::MemRegion;

use itertools::Itertools;

static mut KERNEL_PAGE_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(0 as *mut _);

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
    pub fn pages(&self) -> impl IntoIterator<Item=VirtAddr> {
        (self.start.as_u64()..self.region_end() as u64).step_by(PAGE_SIZE).map(|addr| VirtAddr::new(addr))
    }

    /// Returns the size of the region in pages
    pub fn size(&self) -> usize {
        self.size
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
    UnmapNonPresent
}

#[derive(Debug, Default)]
pub struct VirtualMemoryManager {
    kernel_reserved: BTreeSet<Mapping>,
}

impl VirtualMemoryManager {
    fn new() -> Self {
        Self {
            kernel_reserved: BTreeSet::new(),
        }
    }

    pub(super) fn insert_and_map(&mut self, mut mapping: Mapping, frame_allocator: &mut impl FrameAllocator) -> Result<(), Error> {
        let kernel_pt = unsafe {
            get_kernel_context_virt().unwrap().as_mut()
        };
        // FIXME: Should throw an error if attempting to overwrite
        mapping.map(kernel_pt, frame_allocator).unwrap();
        self.insert_mapping(mapping)?;
        Ok(())
    }

    pub(super) fn insert_mapping(&mut self, mapping: Mapping) -> Result<(), Error> {
        self.kernel_reserved.insert(mapping).then_some(()).ok_or(VirtualMemoryError::RegionInUse(""))
    }

    /// Attempts to reserve the specified region of virtual memory. If the region is unavailable
    /// returns `Error`, The region may not necessarily be mapped
    pub fn reserve_region(&mut self, vaddr: VirtAddr, size: usize) -> Result<VirtAddr, Error> {
        let mapping = Mapping::new_empty(VirtualRegion {
            start: vaddr,
            size,
        });
        if self.kernel_reserved.insert(mapping) {
            Ok(vaddr)
        } else {
            Err(VirtualMemoryError::RegionInUse(""))
        }
    }

    /// Removes and unmaps the region containing the region provided as arguments
    pub fn release_region(&mut self, vaddr: VirtAddr, size: usize, frame_allocator: &mut impl FrameAllocator) -> Result<(), VirtualMemoryError> {
        let kernel_pt = unsafe {
            get_kernel_context_virt().unwrap().as_mut()
        };
        let empty_region = &Mapping::new_empty(VirtualRegion::new(vaddr, size));
        self.kernel_reserved.take(empty_region).unwrap().unmap(kernel_pt, frame_allocator, true)
    }

    pub fn mapping_containing(&self, addr: VirtAddr) -> Option<&Mapping> {
        self.kernel_reserved.get(&Mapping::from_address(addr))
    }

    /// Map any region above or at the provided address that can accomodate the provided size.
    pub fn reserve_any_after(&mut self, addr: VirtAddr, size: usize) -> Result<VirtAddr, Error> {
        let region = self.first_available_addr_above(addr, size).ok_or(VirtualMemoryError::NoAddressSpaceAvailable)?;

        self.reserve_region(region.start, size)
    }

    /// Find a suitable region at or above the provided address
    pub fn first_available_addr_above(&self, addr: VirtAddr, size: usize) -> Option<VirtualRegion> {
        let region = VirtualRegion::new(addr, size);

        if self.kernel_reserved.get(&Mapping::new_empty(region)).is_none() {
            return Some(region)
        }

        for (left, right) in self.kernel_reserved.range(Mapping::from_address(addr)..).tuple_windows() {
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

    unsafe {
        KERNEL_PAGE_TABLE.store(
            pml4.start_address().as_u64() as usize as *mut _,
            Ordering::SeqCst,
        );
    }

    // TODO: move this to a method
    let kernel_code = Mapping::existing(memory_layout().kernel_code_region(), MappingType::KernelCode);
    let kernel_data = Mapping::existing(memory_layout().kernel_data_region(), MappingType::KernelData);
    let kernel_stack = Mapping::existing(memory_layout().kernel_stack_region(), MappingType::KernelData);
    let kernel_phys = Mapping::existing(memory_layout().kernel_phys_region(), MappingType::KernelData);

    super::MM.lock().vmm.insert_mapping(kernel_code).unwrap();
    super::MM.lock().vmm.insert_mapping(kernel_data).unwrap();
    super::MM.lock().vmm.insert_mapping(kernel_stack).unwrap();
    super::MM.lock().vmm.insert_mapping(kernel_phys).unwrap();

}

/// The start of the MMIO address space comes after the higher half physical memory range
pub fn mmio_area_start() -> VirtAddr {
    // Align to 2mb since we use huge frames
    (memory_layout().phys_memory_start + ((memory_layout().phys_memory_size + 1) * PAGE_SIZE as u64)).align_up(0x1000000u64)
}

// TODO: doesnt need to be pub should be pub(super)
pub fn get_kernel_context_virt() -> Option<NonNull<PageTable>> {
    let paddr = unsafe { 
        PhysAddr::new(KERNEL_PAGE_TABLE.load(Ordering::SeqCst) as u64)
    };
    let vaddr = phys_to_virt(paddr);
    NonNull::new(vaddr.as_mut_ptr())
}
