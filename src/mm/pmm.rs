use crate::arch::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::dev::serial::write_serial_out;
use crate::error::Error;
use alloc::boxed::Box;
use libkloader::{MemoryMapInfo, MemoryDescriptor};

use spin::{Once, Mutex};
use x86_64::structures::paging::frame::{PhysFrame, PhysFrameRange};
use x86_64::structures::paging::page::Size4KiB;

use crate::common::bitvec::UnorderedBitVec;

use super::frame_allocator::FrameAllocator;

static PHYS_OFFSET: Once<usize> = Once::new();

pub(super) static PMM: Once<Mutex<BitMapFrameAllocator>> = Once::new();

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicalMemoryError {
    UnableToObtainFrame,
}

impl Error for PhysicalMemoryError {
    fn source(&self) -> Option<&Box<dyn Error>> {
        None
    }
}

#[derive(Debug)]
pub struct BitMapFrameAllocator {
    bitmap: UnorderedBitVec,
}

impl BitMapFrameAllocator {
    pub fn uninit() -> Self {
        BitMapFrameAllocator {
            bitmap: UnorderedBitVec::new()
        }
    }

    pub fn init(&mut self, memory_map: &[MemoryDescriptor], heap_start: VirtAddr, heap_end: VirtAddr) {
        let page_count: usize = memory_map.iter().map(|d| d.page_count as usize).sum();
        self.bitmap.resize(page_count);

        for descriptor in memory_map {
            match descriptor.ty {
                3 | 4 | 7 => {
                    let start = descriptor.phys_start;

                    for page_num in 0..descriptor.page_count {
                        let addr = PhysAddr::new(start + (PAGE_SIZE as u64 * page_num));
                        self.deallocate_frame(addr);
                    }
                }
                _ => ()
            }
        }

        let heap_start_phys = PhysAddr::new(heap_start.as_u64() - PHYS_OFFSET.get().cloned().unwrap() as u64);
        let heap_end_phys = PhysAddr::new(heap_end.as_u64() - PHYS_OFFSET.get().cloned().unwrap() as u64);

        for page_num in page_number(heap_start_phys)..page_number(heap_end_phys) {
            self.mark_frame_available(page_address(page_num))
        }
    }

    #[allow(dead_code)]
    pub fn free_frames(&self) -> u32 {
        self.bitmap.count_ones()
    }

    #[allow(dead_code)]
    pub fn frame_count(&self) -> usize {
        self.bitmap.len()
    }

    fn mark_frame_available(&mut self, addr: PhysAddr) {
        self.bitmap.set(page_number(addr));
    }

    fn mark_frame_used(&mut self, addr: PhysAddr) {
        self.bitmap.clear(page_number(addr));
    }

    pub fn request_frame(&mut self, addr: PhysAddr) -> Result<PhysAddr, PhysicalMemoryError> {
        if self.is_available(addr) {
            self.mark_frame_used(addr);
            Ok(addr)
        } else {
            Err(PhysicalMemoryError::UnableToObtainFrame)
        }
    }

    pub fn is_available(&self, addr: PhysAddr) -> bool {
        self.bitmap.get(page_number(addr)).unwrap_or(false)
    }

    fn take_first_available(&mut self) -> PhysAddr {
        let num = self.bitmap.first_one().expect("PMM: OOM");
        let addr = PhysAddr::new((num * PAGE_SIZE) as u64);
        self.mark_frame_used(addr);

        // Don't hand out address 0x8000 so we can use it to boot aps
        if addr == PhysAddr::new(0x8000) {
            let new = self.take_first_available();
            self.mark_frame_available(addr);

            return new;
        }

        unsafe {
            (get_phys_as_mut(addr) as Option<&mut [u8; PAGE_SIZE]>).unwrap().fill(0);
        }

        addr
    }
}

pub(super) fn init(heap_range: (VirtAddr, VirtAddr)) {
    let mem_map = &crate::env::env().memory_map;
    PMM.call_once(|| {
        let mut pmm = BitMapFrameAllocator::uninit();
        pmm.init(mem_map, heap_range.0, heap_range.1);
        Mutex::new(pmm)
    });
}

pub fn physical_memory_manager<'pmm>() -> &'pmm Mutex<BitMapFrameAllocator> {
    PMM.get().unwrap()
}

fn page_number(addr: PhysAddr) -> usize {
    addr.as_u64() as usize / PAGE_SIZE
}

fn page_address(num: usize) -> PhysAddr {
    PhysAddr::new((num * PAGE_SIZE) as u64)
}

impl FrameAllocator for BitMapFrameAllocator {
    fn allocate_frame(&mut self) -> PhysAddr {
        self.take_first_available()
    }

    fn deallocate_frame(&mut self, frame: PhysAddr) {
        self.mark_frame_available(frame);
    }
}

/// Returns the virtual address to the corresponding physical memory
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr::new(paddr.as_u64() + PHYS_OFFSET.get().copied().unwrap() as u64)
}

pub(super) fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    PhysAddr::new(vaddr.as_u64() - PHYS_OFFSET.get().copied().unwrap() as u64)
}

pub unsafe fn write_physical<T: Sized + Clone>(addr: PhysAddr, value: T) {
    (phys_to_virt(addr).as_mut_ptr() as *mut T).write(value);
}

pub unsafe fn write_physical_slice<T: Sized + Clone>(addr: PhysAddr, value: &[T]) {
    let ptr = phys_to_virt(addr).as_mut_ptr() as *mut T;
    core::slice::from_raw_parts_mut(ptr, value.len()).clone_from_slice(value);
}

#[allow(dead_code)]
pub unsafe fn get_phys_as_mut<'t, T>(addr: PhysAddr) -> Option<&'t mut T> {
    let ptr = phys_to_virt(addr).as_mut_ptr() as *mut T;
    ptr.as_mut()
}

pub fn init_phys_offset(offset: usize) {
    PHYS_OFFSET.call_once(|| offset);
}

pub fn get_init_heap_section(
    size: usize,
    mem_map: MemoryMapInfo,
) -> Result<PhysFrameRange<Size4KiB>, &'static str> {
    let mut sect = None;

    write_serial_out("Mem type search begin \n");

    for descriptor in mem_map.get_memory_map() {
        match descriptor.ty {
            7 => {
                write_serial_out("Mem type match \n");
                // Don't use low memory
                if descriptor.page_count >= size as u64 && descriptor.phys_start > 0x10000 {
                    sect = Some(descriptor.phys_start);
                    break;
                }
            }
            _ => (),
        }
    }

    write_serial_out("Mem search done \n");
    let start_address = PhysAddr::new(sect.ok_or("<ERROR> No suitable memory start_addr found\n")?);
    let start_frame = PhysFrame::containing_address(start_address);
    let end_frame = PhysFrame::containing_address(start_address + (size as u64 * PAGE_SIZE as u64));

    Ok(PhysFrame::range(start_frame, end_frame))
}

#[cfg(test)]
mod test {
    use super::super::*;

    use frame_allocator::FrameAllocator;

    #[test_case]
    fn test_request_range() {
        // NOTE: This only tests finding the frame from the stack allocator and not in the regions
        let mut pmm = physical_memory_manager().lock();
        let test_addr = pmm.allocate_frame();
        pmm.deallocate_frame(test_addr.clone());
        let pre = pmm.is_available(test_addr);
        let f = pmm.request_frame(test_addr);
        let post = pmm.is_available(test_addr);

        assert!(pre);
        assert!(f.is_ok());
        assert!(!post);

        pmm.deallocate_frame(f.unwrap());
    }

    #[test_case]
    fn test_pmm_alloc_and_free() {
        // Take lock for duration of test
        let mut pmm = physical_memory_manager().lock();

        let starting_frame_count = pmm.free_frames();
        let frame = pmm.allocate_frame();
        let frame2 = pmm.allocate_frame();
        let frame_count_after_alloc = pmm.free_frames();

        pmm.deallocate_frame(frame);
        pmm.deallocate_frame(frame2);
        let frame_count_after_free = pmm.free_frames();

        assert_ne!(frame, frame2);
        assert_ne!(starting_frame_count, frame_count_after_alloc);
        assert_eq!(starting_frame_count, frame_count_after_free);
    }
}
