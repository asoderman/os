use core::cmp::Ordering;
use core::sync::atomic::AtomicUsize;

use crate::arch::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::dev::serial::write_serial_out;
use crate::error::Error;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use libkloader::{MemoryMapInfo, KernelInfo};
use libkloader::{uefi::MemoryType, MemoryDescriptor};

use x86_64::structures::paging::frame::{PhysFrame, PhysFrameRange};
use x86_64::structures::paging::page::Size4KiB;

use super::region::MemRegion;


static mut PHYS_OFFSET: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
enum Usage {
    Free = 0,
    AcpiReclaim,
    KernelHeap,
    KernelCode,
    KernelStack,
    Misc,
    Reserved,
    Unusable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PhysicalMemoryError {
    UnableToObtainPhysicalRegion
}

impl Error for PhysicalMemoryError {
    fn source(&self) -> Option<&Box<dyn Error>> {
        None
    }
}

#[derive(Debug)]
struct PhysicalRegion {
    start: PhysAddr,
    frames: u32,
    usage: Usage
}

impl MemRegion for PhysicalRegion {
    fn start(&self) -> usize {
        self.start.as_u64() as usize
    }

    fn end(&self) -> usize {
        (self.start.as_u64() + self.frames as u64 * PAGE_SIZE as u64) as usize
    }
}

impl PhysicalRegion {
    #[inline]
    fn is_free(&self) -> bool {
        self.usage == Usage::Free
    }

    /*
    #[inline]
    fn contains(&self, paddr: PhysAddr) -> bool {
        self.start <= paddr && paddr < self.exclusive_end()
    }
    */

    /// Removes the specified sub region from self.
    ///
    /// # Returns
    /// a tuple of (sub_region, (region before, region after))
    fn sub_region(mut self, start: PhysAddr, end: PhysAddr) -> (PhysicalRegion, (Option<PhysicalRegion>, Option<PhysicalRegion>)) {
        crate::println!("getting sub region {:?} -> {:?}", start, end);
        let end_region_size = (self.exclusive_end() - end) / PAGE_SIZE as u64;
        let sub_region_size = (end - start) / PAGE_SIZE as u64;
        let end_region = self.split(end_region_size as u32);

        if self.start == start && self.frames as u64 == sub_region_size {
            (self, (None, end_region))
        } else {
            let sub_region = self.split(sub_region_size as u32);
            crate::println!("sub region result {:?}", sub_region);
            (sub_region.unwrap(), (Some(self), end_region))
        }
    }

    /// Splits off a new physical region from self.
    ///
    /// # Returns
    /// The region that is split off
    fn split(&mut self, frames: u32) -> Option<PhysicalRegion> {
        if self.frames - frames > 1 {
            self.frames -= frames;
            Some( PhysicalRegion {
                start: self.exclusive_end(),
                frames,
                usage: Usage::Free
            })
        } else {
            None
        }
    }

    fn exclusive_end(&self) -> PhysAddr {
        debug_assert_eq!((self.start.as_u64() + self.frames as u64 * PAGE_SIZE as u64) % PAGE_SIZE as u64, 0, "PhysicalRegion.exclusive_end() not page aligned");
        self.start + self.frames as u64 * PAGE_SIZE as u64
    }

    fn merge(&mut self, other: &Self) -> Result<(), ()> {
        if self.exclusive_end() == other.start && self.is_free() && other.is_free() {
            self.frames += other.frames;
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Ord for PhysicalRegion {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if self.overlaps(other) || self.contains(other) || self.within(other) {
            core::cmp::Ordering::Equal
        } else {
            self.start.cmp(&other.start)
        }
    }
}

impl PartialOrd for PhysicalRegion {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.start() > other.end() {
            Some(core::cmp::Ordering::Greater)
        } else if self.end() < other.start() {
            Some(core::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl PartialEq for PhysicalRegion {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end() == other.end()
    }
}

impl Eq for PhysicalRegion {}

#[derive(Debug)]
struct FrameAllocator {
    /// A stack of free frames
    frames: Vec<PhysAddr>
}

impl FrameAllocator {
    fn new() -> Self {
        FrameAllocator { frames: Vec::new() }
    }

    fn fill(&mut self, region: PhysicalRegion) -> Option<PhysicalRegion> {
        // TODO: if the region is too large split and return part of it
        for i in 0u64..region.frames as u64 {
            let paddr = PhysAddr::new(region.start.as_u64() as u64 + i * PAGE_SIZE as u64);
            self.frames.push(paddr);
        }

        None
    }

    // TODO: Possible RAII. 
    fn allocate(&mut self) -> Option<PhysAddr> {
        self.frames.pop()
    }
    // TODO: use BTreeSet for deduping. depends on insertion time
    fn free(&mut self, paddr: PhysAddr) {
        self.frames.push(paddr);
    }

    fn frame_count(&self) -> usize {
        self.frames.len()
    }

    fn contains_frame(&self, frame: PhysAddr) -> bool {
        self.frames.contains(&frame)
    }

    fn get_frames(&mut self, range: PhysicalRegion ) -> Vec<PhysAddr> {
        self.frames.drain_filter(|p| range.contains_val(p.as_u64() as usize)).collect()
    }
}

#[derive(Debug)]
pub struct PhysicalMemoryManager {
    initialized: bool,
    /// The offset where the entire physical memory range is mapped.
    phys_offset: usize,
    heap_start_phys: PhysAddr,
    heap_end_phys: PhysAddr,
    free_regions: BTreeSet<PhysicalRegion>,
    free_low_memory: BTreeSet<PhysicalRegion>,
    unusable: BTreeSet<PhysicalRegion>,
    frames: FrameAllocator,
}

impl PhysicalMemoryManager {
    pub(super) fn init(&mut self, mem_map: &[MemoryDescriptor], heap_start: VirtAddr, heap_end: VirtAddr, phys_offset: usize) {
        self.phys_offset = phys_offset;
        self.heap_start_phys = PhysAddr::new(heap_start.as_u64() - phys_offset as u64);
        self.heap_end_phys = PhysAddr::new(heap_end.as_u64() - phys_offset as u64);

        // Populate the PMM
        for entry in mem_map {
            // TODO: Verify if I can reuse EfiLoaderCode and EfiLoaderData (1, 2)
            let usage = match entry.ty {
                3 | 4 | 7 => Usage::Free,
                9 => {
                    crate::println!("Acpi address: {:#X}", entry.phys_start);
                    Usage::AcpiReclaim
                },
                8 => Usage::Unusable,
                0 | 5 | 6 => Usage::Reserved,
                _ => Usage::Unusable
            };
            let region = PhysicalRegion {
                start: PhysAddr::new(entry.phys_start),
                frames: entry.page_count as u32,
                usage
            };

            // FIXME: 0 frames indicates an invalid region however if the memory is not zeroed how
            // will we detect if it is an invalid region or not.
            // This could be a bug with the bootloader passing the memory map to the kernel
            if region.frames == 0 {
                crate::println!("region {:?} with 0 frames skipping...", region.start);
                continue;
            }

            match usage {
                Usage::Free | Usage::AcpiReclaim => { 
                    if region.contains_val(self.heap_start_phys.as_u64() as usize) {
                        let (heap_region, (r1, r2)) = region.sub_region(self.heap_start_phys, self.heap_end_phys);
                        crate::println!("Heap region found {:#X?}", heap_region);
                        crate::println!("Remaining regions {:#X?} \n{:#X?}", r1, r2);
                        r1.map(|r| self.free_regions.insert(r));
                        r2.map(|r| self.free_regions.insert(r));
                    } else {
                        if region.start() >= 0x10000 {
                            self.free_regions.insert(region);
                        } else {
                            self.free_low_memory.insert(region);
                        }
                    }
                },
                _ => { self.unusable.insert(region); },
            }
        }

        self.fill_frame_allocator();

        crate::println!("[PMM Init] free memory: {:#} bytes", self.free());

        self.initialized = true;
    }

    /// Create an uninitialized pmm.
    pub(super) fn uninit() -> Self {
        Self {
            phys_offset: 0,
            initialized: false,
            heap_start_phys: PhysAddr::zero(),
            heap_end_phys: PhysAddr::zero(),
            free_regions: BTreeSet::new(),
            free_low_memory: BTreeSet::new(),
            unusable: BTreeSet::new(),
            frames: FrameAllocator::new(),
        }
    }

    fn fill_frame_allocator(&mut self) {
        let entry = self.free_regions.pop_first().expect("Out of physical memory");
        if entry.usage != Usage::AcpiReclaim {
            self.frames.fill(entry);
        } else {
            let new_entry = self.free_regions.pop_first().expect("Out of physical memory");
            self.frames.fill(new_entry);
            self.free_regions.insert(entry);
        }
    }

    fn unavailable_regions(&self) -> impl Iterator<Item = &PhysicalRegion> + '_ {
        self.unusable.iter().filter(|r| !r.is_free())
    }

    /// The total amount of free frames currently
    pub fn free_frames(&self) -> usize {
        let region_frames: usize = self.free_regions.iter().map(|r| r.frames as usize).sum();
        self.frames.frame_count() + region_frames
    }

    pub fn used_frames(&self) -> usize {
        self.unavailable_regions().map(|r| r.frames as usize).sum()
    }

    /// The amount of free physical memory in bytes.
    pub fn free(&self) -> usize {
        self.free_frames() * PAGE_SIZE
    }

    /// The amount of used physical memory in bytes.
    pub fn used(&self) -> usize {
        self.used_frames() * PAGE_SIZE
    }

    pub fn frame_count(&self) -> usize {
        self.frames.frame_count()
    }

    /// Reads T from a physical pointer
    pub unsafe fn read_phys<T>(&self, paddr: PhysAddr) -> T {
        (phys_to_virt(paddr).as_ptr() as *const T).read_volatile()
    }

    /// Returns a mutable reference for the corresponding physical address
    pub unsafe fn get_phys_as_mut<'a, T>(&self, paddr: PhysAddr) -> Option<&'a mut T> {
        let vaddr = phys_to_virt(paddr);
        (vaddr.as_mut_ptr() as *mut T).as_mut()
    }

    /// Gets the first available physical frame not in use.
    pub fn request_frame(&mut self) -> PhysAddr {
        let frame = self.frames.allocate().unwrap_or_else(|| {
            self.fill_frame_allocator();
            self.frames.allocate().unwrap()
        });

        unsafe {
            self.get_phys_as_mut::<[u8; PAGE_SIZE]>(frame).unwrap().fill(0);
        }
        frame
    }

    pub fn release_frame(&mut self, paddr: PhysAddr) {
        self.frames.free(paddr)
    }

    /// Finds the frame either in a contiguos region or in the frame allocator. 
    /// Uses linear search since the frame allocator is an unordered stack and the region set is
    /// ordered based on range.
    // TODO: we could potentially binary search the regions and return the closest range then check
    // if said range contains the address
    pub fn find_frame(&self, frame: PhysAddr) -> bool {
        let regions_search = self.free_regions.iter().find(|f| f.contains_val(frame.as_u64() as usize));

        regions_search.is_some() || self.frames.contains_frame(frame)
    }

    /// Allocates a frame within low memory (< 1 mb)
    pub fn request_low_memory(&mut self, addr: PhysAddr, size: usize) -> Option<PhysAddr> {
        let candidate_region = PhysicalRegion { start: addr, frames: size as u32, usage: Usage::Free };
        self.free_low_memory.take(&candidate_region).map(|r| r.start)
    }

    /// Request a specific physical memory region.
    pub fn request_range(&mut self, paddr: PhysAddr, size: usize) -> Option<PhysAddr> {
        let candidate_region = PhysicalRegion { start: paddr, frames: size as u32, usage: Usage::Free };
        let from_regions = self.free_regions.take(&candidate_region);

        if let Some(region) = from_regions {
            let (result, (x, xs)) = region.sub_region(paddr, paddr + size as u64 * PAGE_SIZE as u64);
            x.map(|f| self.free_regions.insert(f));
            xs.map(|f| self.free_regions.insert(f));
            Some(result.start)
        } else {
            let mut frames = self.frames.get_frames(candidate_region);
            frames.sort();
            let result = frames.pop()?;

            (result == paddr).then_some(result)
        }
    }
}

/// Returns the virtual address to the corresponding physical memory
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    unsafe {
        VirtAddr::new(paddr.as_u64() + *PHYS_OFFSET.get_mut() as u64)
    }
}

pub(super) fn init_phys_offset(offset: usize) {
    unsafe {
        *PHYS_OFFSET.get_mut() = offset;
    }
}

pub fn get_init_heap_section(
    size: usize,
    mem_map: MemoryMapInfo,
) -> Result<PhysFrameRange<Size4KiB>, &'static str> {
    let mut sect = None;

    let mut reserved_count = 0;

    let mut index = 0;

    write_serial_out("Mem type search begin \n");
    /*
    for descriptor in mem_map.iter() {
        match descriptor.ty {
                7 => {
                write_serial_out("Mem type match \n");
                if descriptor.page_count >= size as u64 {
                    sect = Some(descriptor.phys_start);
                    break;
                }
            }
            0 => { reserved_count += 1; },
            _ => ()
        }
    }
    */

    while index < mem_map.count {
        let descriptor_opt = mem_map.get(index);
        if descriptor_opt.is_none() {
            write_serial_out("descriptor is non\n");
        }

        write_serial_out("Candidate descriptor deref\n");
        let descriptor = descriptor_opt.unwrap();
        match descriptor.ty {
            7 => {
                write_serial_out("Mem type match \n");
                if descriptor.page_count >= size as u64 && descriptor.phys_start > 0x10000 {
                    sect = Some(descriptor.phys_start);
                    break;
                }
            }
            0 => {
                reserved_count += 1;
            }
            _ => (),
        }
        index += 1;
    }

    write_serial_out("Mem search done \n");
    if reserved_count == mem_map.count {
        return Err("<ERROR> Entire mem map reserved. Likely corrupted");
    }
    let start_address = PhysAddr::new(sect.ok_or("<ERROR>No suitable memory start_addr found\n")?);
    let start_frame = PhysFrame::containing_address(start_address);
    let end_frame = PhysFrame::containing_address(start_address + (size as u64 * PAGE_SIZE as u64));

    Ok(PhysFrame::range(start_frame, end_frame))
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::*;

    #[test_case]
    fn test_request_range() {
        // NOTE: This only tests finding the frame from the stack allocator and not in the regions
        let test_addr = memory_manager().request_frame();
        unsafe {
            memory_manager().release_frame(test_addr.clone());
        }
        let pre = memory_manager().pmm.find_frame(test_addr);
        let f = memory_manager().pmm.request_range(test_addr, 1);
        let post = memory_manager().pmm.find_frame(test_addr);

        assert!(pre);
        assert!(f.is_some());
        assert!(!post);

        memory_manager().pmm.release_frame(f.unwrap());
    }
}
