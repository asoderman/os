use alloc::vec::Vec;
use log::warn;
use spin::RwLock;
use x86_64::{structures::paging::{PageTable, PageTableFlags}, VirtAddr};

use super::{vmm::{VirtualRegion, VirtualMemoryError}, frame_allocator::FrameAllocator, AddressSpace};
use crate::arch::{PhysAddr, x86_64::{paging::{Mapper, MapError}, PageSize}, PAGE_SIZE};

use bitflags::bitflags;

bitflags! {
    /// A bitflag structure that defines various attributes about a mapping
    struct Attributes: u8 {
        const NONE = 0;
        /// If this flag is set the mapping will not try to unmap itself on drop
        const NEEDS_UNMAP = 0b10000000;
        const READ = 0b1;
        const WRITE = 0b10;
        const EXECUTABLE = 0b100;
        const HUGE = 0b01000000;
        // Permissions
        const EX = Attributes::EXECUTABLE.bits | Attributes::READ.bits;
        const RW = Attributes::READ.bits | Attributes::WRITE.bits;
    }
}

#[derive(Debug)]
pub enum MappingType {
    KernelCode,
    KernelData,
    MMIO(PhysAddr),
    Identity(PhysAddr),
    /// Empty is just used for comparisons e.g. to retrieve a mapping from the list
    /// It should be combined with the NO_UNMAP attribute to skip the drop check
    Empty
}

/// A struct to represent a mapped region of memory
#[derive(Debug)]
pub struct Mapping {
    range: VirtualRegion,
    kind: MappingType,
    // Use Cell to allow us to modify attributes while the mapping is in the list
    attr: RwLock<Attributes>
}

impl Ord for Mapping {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.range.cmp(&other.range)
    }
}

impl PartialOrd for Mapping {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.range.partial_cmp(&other.range)
    }
}

impl PartialEq for Mapping {
    fn eq(&self, other: &Self) -> bool {
        self.range.eq(&other.range)
    }
}

impl Eq for Mapping { }

impl Drop for Mapping {
    fn drop(&mut self) {
        if self.attr.get_mut().contains(Attributes::NEEDS_UNMAP) {
            warn!("Mapping dropped: {:?}", self);
            //todo!("Implement unmap/releasing physical frames back to pmm on drop");
        }
    }
}

impl Mapping {
    /// Create a new instance for an existing mapping. e.g. things loaded from bootloader
    pub(super) fn existing(range: VirtualRegion, kind: MappingType) -> Self {
        Mapping {
            range,
            kind,
            attr: RwLock::new(Attributes::NEEDS_UNMAP)
        }
    }

    /// Create a new KernelData Mapping
    pub(super) fn new(range: VirtualRegion) -> Self {
        Mapping {
            range,
            kind: MappingType::KernelData,
            attr: RwLock::new(Attributes::RW)
        }
    }

    /// Create a new a empty mapping to represent a region. This is used for lookup and cannot be
    /// mapped.
    pub(super) fn new_empty(range: VirtualRegion) -> Self {
        Mapping {
            range,
            kind: MappingType::Empty,
            attr: RwLock::new(Attributes::empty())
        }
    }

    /// Create a new MMIO Mapping
    pub(super) fn new_mmio(range: VirtualRegion, paddr: PhysAddr, is_huge: bool) -> Self {
        let attr = RwLock::new(Attributes::RW);
        attr.write().set(Attributes::HUGE, is_huge);

        Mapping {
            range,
            kind: MappingType::MMIO(paddr),
            attr
        }
    }

    /// Create a new MMIO Mapping
    pub(super) fn new_identity(paddr: PhysAddr) -> Self {
        let range = VirtualRegion::new(VirtAddr::new(paddr.as_u64()), 1);
        Mapping {
            range,
            kind: MappingType::Identity(paddr),
            attr: RwLock::new(Attributes::RW)
        }
    }

    /// Create a new 1 page empty mapping from an address
    pub(super) fn from_address(addr: VirtAddr) -> Self {
        let region = VirtualRegion::new(addr, 1);
        Self::new_empty(region)
    }

    fn set_attr(&self, attr: Attributes) {
        self.attr.write().insert(attr);
    }

    fn remove_attr(&self, attr: Attributes) {
        self.attr.write().remove(attr);
    }

    pub fn virt_range(&self) -> &VirtualRegion {
        &self.range
    }

    pub fn page_count(&self) -> usize {
        self.range.size
    }

    #[allow(dead_code)]
    pub fn is_read_only(&self) -> bool {
        !self.attr.read().contains(Attributes::RW)
    }

    pub fn is_huge(&self) -> bool {
        self.attr.read().contains(Attributes::HUGE)
    }

    pub fn read_only(&self, address_space: &mut AddressSpace) {
        self.remove_attr(Attributes::EX | Attributes::WRITE);
        self.set_attr(Attributes::READ);
        clear_flags(self.range, address_space.page_table(), PageTableFlags::WRITABLE);
    }

    pub fn read_write(&self, address_space: &mut AddressSpace) {
        self.remove_attr(Attributes::EX);
        self.set_attr(Attributes::READ | Attributes::WRITE);
        set_flags(self.range, address_space.page_table(), PageTableFlags::WRITABLE);
    }

    pub fn executable(&self, address_space: &mut AddressSpace) {
        self.remove_attr(Attributes::WRITE);
        clear_flags(self.range, address_space.page_table(), PageTableFlags::WRITABLE);
        self.set_attr(Attributes::READ | Attributes::EX);
        todo!("NX bit")
    }

    pub fn mark_as_userspace(&self, pt: &mut PageTable) {
        let pages: Vec<VirtAddr> =
        if !self.is_huge() {
            self.virt_range().pages().collect()
        } else {
            self.virt_range().huge_pages().collect()
        };
        for p in pages {
            let mut mapper = Mapper::new(p, pt);
            mapper.set_flags(PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE).unwrap();
        }
    }

    /// Map the pages to the provided frames
    pub fn map(&mut self, pt: &mut PageTable, frame_allocator: &mut impl FrameAllocator) -> Result<(), MapError> {
        match self.kind {
            MappingType::MMIO(paddr) | MappingType::Identity(paddr) => {
                if self.is_huge() {
                    let huge_page_size: usize = PageSize::_2Mb.into();
                    for (i, page) in self.virt_range().huge_pages().enumerate() {
                        Mapper::new(page, pt).map_huge_frame(paddr + (i * huge_page_size), frame_allocator)?;
                    }
                } else {
                    for (i, page) in self.range.pages().into_iter().enumerate() {
                        Mapper::new(page, pt).map_frame(paddr + (i  * PAGE_SIZE), frame_allocator)?;
                    }
                }
            }
            MappingType::KernelData => {
                let mut arch_mapper = Mapper::new(self.range.start, pt);
                let attr = self.attr.read();
                if attr.contains(Attributes::RW) {
                    arch_mapper.map(frame_allocator)?;
                    if self.range.size > 1 {
                        arch_mapper.map_adjacent(self.range.size, frame_allocator)
                    }
                }
                else if attr.contains(Attributes::EX) {
                    todo!("Implement executable range");
                }
                else if attr.contains(Attributes::READ) {
                    todo!("Implement read only mapping");
                }
            }
            MappingType::KernelCode => {
                todo!("Implement kernel code mapping type (drivers?)");
            }
            MappingType::Empty => {
                panic!("Attempted to map empty mapping");
            }
        }
        self.set_attr(Attributes::NEEDS_UNMAP);
        Ok(())
    }

    pub fn unmap(self, pt: &mut PageTable, frame_allocator: &mut impl FrameAllocator, cleanup: bool) -> Result<(), VirtualMemoryError> {
        let pages: Vec<VirtAddr> =
        // Must collect the iterators to satisfy the type checker
        if self.is_huge() {
            self.range.huge_pages().collect()
        } else {
            self.range.pages().collect()
        };
        for page in pages {
            let mut mapper = Mapper::new(page, pt);
            let frame = mapper.unmap(frame_allocator, cleanup).unwrap();
            match self.kind {
                // Dont return a MMIO frame to pmm because it can't be used like normal memory
                MappingType::MMIO(_) => (),
                MappingType::KernelData | MappingType::Identity(_) => {
                    frame_allocator.deallocate_frame(frame);
                }
                _ => { panic!("Unmapped unsupported memory type"); }
            }
        }

        self.remove_attr(Attributes::NEEDS_UNMAP);
        Ok(())
    }
}

/// Helper function to set the flags all the way down the page table structure
fn set_flags(range: VirtualRegion, pt: &mut PageTable, flags: PageTableFlags) {
    for p in range.pages() {
        let mut mapper = Mapper::new(p, pt);
        mapper.set_flags(flags).unwrap();
    }
}

/// Helper function to clear the bottom-most level flags specified
fn clear_flags(range: VirtualRegion, pt: &mut PageTable, flags: PageTableFlags) {
    for p in range.pages() {
        let mut mapper = Mapper::new(p, pt);
        mapper.clear_lowest_level_flags(flags).unwrap();
    }
}
