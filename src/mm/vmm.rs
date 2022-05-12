use core::ptr::NonNull;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering;

use core::cmp;

use alloc::collections::BTreeSet;
use spin::RwLockReadGuard;
use spin::RwLockWriteGuard;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTable;
use x86_64::structures::paging::PageTableFlags;

use crate::arch::x86_64::paging::MapError;
use crate::arch::x86_64::paging::Mapper;
use crate::arch::x86_64::VirtAddr;
use crate::arch::PAGE_SIZE;

use core::fmt::Debug;

use lazy_static::lazy_static;
use spin::RwLock;

use super::pmm::phys_to_virt;
use super::region::MemRegion;

static mut KERNEL_PAGE_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(0 as *mut _);

/// A virtual region of memory. If the regions overlap in any form their `Ord` will return `Equal`.
/// Otherwise their `Ord` is as expected.
#[derive(Debug, Copy, Clone)]
struct VirtualRegion {
    start: VirtAddr,
    end: VirtAddr,
}

impl MemRegion for VirtualRegion {
    fn start(&self) -> usize {
        self.start.as_u64() as usize
    }

    fn end(&self) -> usize {
        self.end.as_u64() as usize
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
        if self.start > other.end {
            Some(core::cmp::Ordering::Greater)
        } else if self.end < other.start {
            Some(core::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl PartialEq for VirtualRegion {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end
    }
}
impl Eq for VirtualRegion {}

type Error = VirtualMemoryError;

#[derive(Debug, Clone, Copy)]
pub enum VirtualMemoryError {
    RegionInUse(&'static str),
    UnmapNonPresent
}

#[derive(Debug, Default)]
pub struct VirtualMemoryManager {
    reserved: BTreeSet<VirtualRegion>,
}

impl VirtualMemoryManager {
    fn new() -> Self {
        Self {
            reserved: BTreeSet::new(),
        }
    }

    /// Attempts to reserve the specified region of virtual memory. If the region is unavailable
    /// returns `Error`, The region may not necessarily be mapped
    pub fn reserve_region(&mut self, vaddr: VirtAddr, size: usize) -> Result<(), Error> {
        if self.reserved.insert(VirtualRegion {
            start: vaddr,
            end: vaddr + (size * PAGE_SIZE) as u64,
        }) {
            Ok(())
        } else {
            Err(VirtualMemoryError::RegionInUse(""))
        }
    }

    pub fn release_region(&mut self, vaddr: VirtAddr, size: usize) -> bool {
        self.reserved.remove(&VirtualRegion {
            start: vaddr,
            end: vaddr + (size * PAGE_SIZE) as u64,
        })
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
}

// TODO: doesnt need to be pub should be pub(super)
pub fn get_kernel_context_virt() -> Option<NonNull<PageTable>> {
    let paddr = unsafe { 
        PhysAddr::new(KERNEL_PAGE_TABLE.load(Ordering::SeqCst) as u64)
    };
    let vaddr = phys_to_virt(paddr);
    NonNull::new(vaddr.as_mut_ptr())
}
