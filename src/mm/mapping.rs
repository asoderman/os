use spin::RwLock;
use x86_64::{structures::paging::PageTable, VirtAddr};

use super::{vmm::{VirtualRegion, VirtualMemoryError}, frame_allocator::FrameAllocator};
use crate::arch::{PhysAddr, x86_64::paging::{Mapper, MapError}, PAGE_SIZE};

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
            println!("Mapping dropped: {:?}", self);
            todo!("Implement unmap/releasing physical frames back to pmm on drop");
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
    /// mapped. It must maintain `NO_UNMAP` so it does not get caught by the drop check.
    pub(super) fn new_empty(range: VirtualRegion) -> Self {
        Mapping {
            range,
            kind: MappingType::Empty,
            attr: RwLock::new(Attributes::empty())
        }
    }

    /// Create a new MMIO Mapping
    pub(super) fn new_mmio(range: VirtualRegion, paddr: PhysAddr) -> Self {
        Mapping {
            range,
            kind: MappingType::MMIO(paddr),
            attr: RwLock::new(Attributes::RW)
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

    #[allow(dead_code)]
    pub fn is_read_only(&self) -> bool {
        !self.attr.read().contains(Attributes::RW)
    }

    #[allow(dead_code)]
    pub fn read_only(&self) {
        self.remove_attr(Attributes::EX | Attributes::WRITE);
        self.set_attr(Attributes::READ);
    }

    #[allow(dead_code)]
    pub fn read_write(&self) {
        self.remove_attr(Attributes::EX);
        self.set_attr(Attributes::READ | Attributes::WRITE);
    }

    #[allow(dead_code)]
    pub fn executable(&self) {
        self.remove_attr(Attributes::WRITE);
        self.set_attr(Attributes::READ | Attributes::EX);
    }

    /// Map the pages to the provided frames
    pub fn map(&mut self, pt: &mut PageTable, frame_allocator: &mut impl FrameAllocator) -> Result<(), MapError> {
        match self.kind {
            MappingType::MMIO(paddr) | MappingType::Identity(paddr) => {
                for (i, page) in self.range.pages().into_iter().enumerate() {
                    Mapper::new(page, pt).map_frame(paddr + (i  * PAGE_SIZE), frame_allocator)?;
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
        for (_i, page) in self.range.pages().into_iter().enumerate() {
            let mut walker = Mapper::new(page, pt);

            let frame = walker.unmap(1, frame_allocator, cleanup).unwrap();//.map_err(|_| VirtualMemoryError::UnmapNonPresent)?;
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
