use core::{marker::PhantomData, ptr::NonNull};

use crate::mm::frame_allocator::FrameAllocator;

use alloc::vec::Vec;
use x86_64::{
    structures::paging::{page_table::PageTableEntry, PageTable, PageTableIndex, PageTableFlags},
    VirtAddr, PhysAddr,
};

use crate::mm::{phys_to_virt, physical_memory_manager};

const MAX_PAGE_ENTRIES: usize = 512;

const MAX_PAGE_LEVEL: usize = 4;

pub trait PageTableOps {
    fn deep_copy(&self) -> *mut Self;
}

impl PageTableOps for PageTable {
    /// Performs a deep (recursive) copy on a Page Table that copies the structure of the page table but does
    /// not copy the underlying data.
    ///
    /// # Safety
    /// This method must only be called on a level 4 page table
    fn deep_copy(&self) -> *mut Self {
        unsafe fn deep_copy_inner(pt: &PageTable, level: usize) -> (*mut PageTable, PhysAddr) {
            let new_page_table = allocate_page_table();

            for (i, entry) in pt.iter().enumerate() {
                if !entry.is_unused() {
                    if level == 4 && i > 509 {
                        new_page_table.0.as_mut().unwrap()[i].clone_from(entry);
                        continue;
                    } else if level == 1 {
                        // If the bottom level is reached copy the page table entry which should point
                        // to a physical frame
                        new_page_table.0.as_mut().unwrap()[i].clone_from(entry);
                    } else {
                        // Otherwise create a new page table and configure it the same way as the
                        // source page table
                        let pointed_to_page_table: *mut PageTable = phys_to_virt(entry.addr()).as_mut_ptr();
                        let copy = deep_copy_inner(pointed_to_page_table.as_mut().unwrap(), level - 1);

                        // Set the corresponding entry to the copied page table and keep the flags
                        new_page_table.0.as_mut().unwrap()[i].set_addr(copy.1, entry.flags())
                    }
                }
            }

            new_page_table
        }

        unsafe {
            deep_copy_inner(self, 4).0
        }
    }
}

fn allocate_page_table() -> (*mut PageTable, PhysAddr) {
    let phys = physical_memory_manager().lock().allocate_frame();
    (phys_to_virt(phys).as_mut_ptr(), phys)
}

/// RAII TLB Flush guard
pub struct Flusher(VirtAddr);

impl Drop for Flusher {
    fn drop(&mut self) {
        x86_64::instructions::tlb::flush(self.0);
    }
}

impl Flusher {
    #[allow(dead_code)]
    pub fn flush(self) -> VirtAddr {
        self.0
    }

    #[allow(dead_code)]
    pub fn ignore(self) -> VirtAddr {
        let addr = self.0;
        core::mem::forget(self);
        addr
    }
}

#[derive(Debug, PartialEq)]
pub enum MapError {
    /// Returned if at the end of the PT structure
    BottomLevel,
    /// Returned if at the top of the PT structure
    TopLevel,
    /// Returned if a huge page is encountered when attempting to descend the structure
    HugeFrame(usize),
    /// Returned if attempting to advance on a non present entry.
    NotPresent,
    /// Returned when trying to create a new mapping on an existing entry. This can be ignored with
    /// `map_unchecked`.
    PresentEntry
}

/// A struct that manages the state for performing page table walks using the virtual address provided
pub struct Mapper<'a> {
    addr: VirtAddr,
    indices: [PageTableIndex; MAX_PAGE_LEVEL],
    current_level: usize,
    current_ptr: NonNull<PageTable>,
    prev_ptrs: Vec<NonNull<PageTable>>,
    _phantom: PhantomData<&'a mut PageTable>
}

impl<'a> Mapper<'a> {
    /// Create a new walker for the provided VirtAddr
    pub fn new(addr: VirtAddr, page_table: &'a mut PageTable) -> Self {
        Mapper {
            addr,
            indices: [addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()],
            current_level: 4,
            current_ptr: NonNull::new(page_table as *mut PageTable).unwrap(),
            prev_ptrs: Vec::new(),
            _phantom: PhantomData::default(),
        }
    }

    /// Advance the walker to the next PageTable. Returns Error if unable to presently advance
    /// however the issue may be resolvable.
    pub fn advance(&mut self) -> Result<(), MapError> {
        if self.current_level == 1 { return Err(MapError::BottomLevel) }

        let index = self.next_index();
        let entry = &self.current()[index];

        if entry.is_unused() { return Err(MapError::NotPresent) }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE | PageTableFlags::PRESENT) { return Err(MapError::HugeFrame(self.current_level)) }

        let next_addr = entry.addr();
        let next_vaddr = phys_to_virt(next_addr);

        self.prev_ptrs.push(self.current_ptr);
        self.current_ptr = NonNull::new(next_vaddr.as_mut_ptr()).unwrap();
        self.current_level -= 1;
        Ok(())
    }

    /// Walk to the lowest page level.
    #[allow(dead_code)]
    pub fn walk(&mut self) {
        loop {
            match self.advance() {
                Ok(_) => (),
                Err(_) => break,
            }
        }
    }

    /// Move the pointer to point to the previous page level
    pub fn ascend(&mut self) -> Result<(), MapError> {
        let prev = self.prev_ptrs.pop().ok_or(MapError::TopLevel)?;
        self.current_ptr = prev;
        self.current_level += 1;

        Ok(())
    }

    /// Increases the index for a specific page level e.g. increase the level 2 index because the
    /// level 1 table index exceeded 511
    fn increase_index(&mut self, page_level: usize) -> Result<(), MapError> {
        let i = u16::from(self.indices[MAX_PAGE_LEVEL - page_level]);
        let new = i + 1 % MAX_PAGE_ENTRIES as u16;

        self.indices[MAX_PAGE_LEVEL - page_level] = PageTableIndex::new(new);

        Ok(())
    }

    /// Get the current PageTable in the walk.
    fn current(&mut self) -> &mut PageTable {
        unsafe {
            self.current_ptr.as_mut()
        }
    }

    /// Get an index for the current pagetable to retrieve the pagetable entry containing the next
    /// table.
    fn next_index(&self) -> PageTableIndex {
        self.indices[MAX_PAGE_LEVEL - self.current_level]
    }

    /// Mutably borrow the entry in the current table that corresponds to the next table.
    pub fn next_entry(&mut self) -> &mut PageTableEntry {
        let index = self.next_index();
        &mut self.current()[index]
    }

    /// Maps next entry and marks it present.
    ///
    /// returns Error if the entry is already marked present.
    pub fn map_next(&mut self, frame_allocator: &mut impl FrameAllocator) -> Result<(), MapError> {
        let entry = self.next_entry();
        if !entry.is_unused() { return Err(MapError::PresentEntry); }

        let frame = frame_allocator.allocate_frame();
        entry.set_addr(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT);
        Ok(())
    }

    /// Sets the level 2 page entry to the physical frame provided and sets the huge frame flag.
    ///
    /// # Safety
    ///
    /// The caller must ensure the frame is a valid huge frame and that the mapper is in the
    /// correct state to set the frame i.e. the next_entry() is a level 2 page entry
    fn map_next_huge(&mut self, frame: PhysAddr) -> Result<(), MapError> {
        if self.current_level == 2 {
            let huge_page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::HUGE_PAGE;
            let entry = self.next_entry();
            if !entry.is_unused() { Err(MapError::PresentEntry)? }
            entry.set_addr(frame, huge_page_flags);

            Ok(())
        } else {
            // TODO: this needs its own error type
            Err(MapError::BottomLevel)
        }
    }

    /// Maps the provided frame to the address specified by the mapper. Currently only supports
    /// level 2 2mb frames
    ///
    /// # Safety
    /// Caller must ensure the provided frame can be mapped as a huge frame
    pub fn map_huge_frame(&mut self, frame: PhysAddr, frame_allocator: &mut impl FrameAllocator) -> Result<Flusher, MapError> {

        loop {
            match self.advance() {
                Err(MapError::NotPresent) => {
                    if self.current_level > 2 {
                        self.map_next(frame_allocator)?
                    }
                },
                Ok(_) => {
                    if self.current_level == 2 {
                        self.map_next_huge(frame)?;
                        break;
                    }
                },
                _ => todo!("Handle other cases when mapping huge frame")
            }
        }

        Ok(Flusher(self.addr))
    }

    pub fn map_frame(&mut self, frame: PhysAddr, frame_allocator: &mut impl FrameAllocator) -> Result<Flusher, MapError> {

        loop {
            // Ensure everything is writable by default
            self.next_entry().flags().set(PageTableFlags::WRITABLE, true);

            match self.advance() {
                Err(MapError::NotPresent) => {
                    if self.current_level != 1 {
                        self.map_next(frame_allocator)?
                    }
                },
                Err(MapError::BottomLevel) =>  { 
                    let index = self.next_index();
                    self.current()[index].set_addr(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                    return Ok(Flusher(self.addr))
                },
                Err(e) => { Err(e)? },
                _ => {
                    self.next_entry().flags().set(PageTableFlags::WRITABLE, true);
                    if !self.next_entry().flags().contains(PageTableFlags::WRITABLE) && self.next_entry().flags().contains(PageTableFlags::PRESENT) {
                        self.next_entry().set_flags(PageTableFlags::WRITABLE | PageTableFlags::PRESENT);
                        assert!(self.next_entry().flags().contains(PageTableFlags::WRITABLE));
                    }
                },
            }
        }
    }

    pub fn map(&mut self, frame_allocator: &mut impl FrameAllocator) -> Result<Flusher, MapError> {
        let frame = frame_allocator.allocate_frame();
        self.map_frame(frame, frame_allocator)
    }

    /// Unmaps the entry the walker would advance to next.
    ///
    /// Returns the unmapped_frame or Error if the entry is marked not present.
    pub fn unmap_next(&mut self) -> Result<PhysAddr, MapError> {
        if self.next_entry().is_unused() { Err(MapError::NotPresent)? };
        let frame = self.next_entry().addr();
        self.next_entry().set_unused();
        Ok(frame)
    }

    /// Unmaps a range of frames
    ///
    /// Flushes the unmapped address from the TLB
    pub fn unmap(&mut self, frame_allocator: &mut impl FrameAllocator, cleanup: bool) -> Result<PhysAddr, MapError> {
        loop {
            match self.advance() {
                Err(MapError::BottomLevel) | Err(MapError::HugeFrame(_)) => break,
                Ok(()) => (),
                _ => Err(MapError::NotPresent)?,
            }
        }

        let frame = self.unmap_next()?;

        if cleanup {
            loop {
                if self.current_is_empty() {
                    match self.ascend() {
                        Err(MapError::TopLevel) => break,
                        _ => ()
                    }

                    frame_allocator.deallocate_frame(self.unmap_next()?);

                } else {
                    break;
                }
            }
        }

        x86_64::instructions::tlb::flush(self.addr);
        Ok(frame)
    }

    /// Sets the flags for each page table entry of the specified address.
    ///
    /// # Safety:
    /// Do not attempt to set the huge page flag via this method
    pub fn set_flags(&mut self, flags: PageTableFlags) -> Result<Flusher, MapError> {
        assert!(!flags.contains(PageTableFlags::HUGE_PAGE));
        loop {
            if !self.next_entry().is_unused() {

                let next_flags = self.next_entry().flags();
                self.next_entry().set_flags(flags | next_flags);

                match self.advance() {
                    Err(MapError::NotPresent) => { unreachable!() },
                    Err(MapError::BottomLevel) | Err(MapError::HugeFrame(_))  => { return Ok(Flusher(self.addr)) },
                    _ => (),
                }
            } else {
                Err(MapError::NotPresent)?
            }
        }
    }

    pub fn clear_lowest_level_flags(&mut self, flags: PageTableFlags) -> Result<Flusher, MapError> {
        self.walk();

        if self.next_entry().is_unused() { Err(MapError::NotPresent)? }

        let new = self.next_entry().flags() & !flags;

        self.next_entry().set_flags(new);

        Ok(Flusher(self.addr))
    }

    /// Clears the provided flags ONLY in the top-most page table i.e. PML4
    pub fn clear_highest_level_flags(&mut self, flags: PageTableFlags) -> Result<Flusher, MapError> {
        assert!(self.current_level == MAX_PAGE_LEVEL, "Attempted to clear highest level flags while mapper is not set to highest page table");
        if self.next_entry().is_unused() { Err(MapError::NotPresent)? }

        let new = self.next_entry().flags() & !flags;

        self.next_entry().set_flags(new);

        Ok(Flusher(self.addr))
    }

    fn current_is_empty(&mut self) -> bool {
        self.current().iter().map(|e| e.is_unused()).fold(true, |acc, elem| acc && elem)
    }

    #[allow(dead_code)]
    fn entries_used_by_current(&mut self) -> Vec<(usize, &PageTableEntry)> {
        self.current().iter().enumerate().filter(|e| !e.1.is_unused()).collect()
    }

    /// Map the pages that are adjacent to the address provided.
    pub fn map_adjacent(&mut self, pages: usize, mm: &mut impl FrameAllocator) {
        if self.current_level != 1 { return; }
        let l1_index = usize::from(self.indices.last().unwrap().clone());
        let range_end = l1_index + pages;
        // the range to be mapped excluding the already mapped page
        let map_range = (l1_index..(range_end)).skip(1);

        // If we need multiple l2_pages
        if range_end >= MAX_PAGE_ENTRIES {
            let _next_l2_end = range_end % MAX_PAGE_ENTRIES;
        }

        for i in map_range {
            let frame = mm.allocate_frame();
            // if we cross a page table boundary
            if i % MAX_PAGE_ENTRIES == 0 && i != 0 {
                self.ascend().unwrap();
                self.increase_index(2).unwrap();
                self.map_next(mm).unwrap();
                self.advance().unwrap();
            }
            // TODO: remove hardcoded flags
            self.current()[i % MAX_PAGE_ENTRIES].set_addr(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        }
    }

    /// Returns the physical address of the lowest level of the page table structure
    pub fn get_phys_frame(&mut self) -> Option<(PhysAddr, bool)> {
        loop {
            match self.advance() {
                Err(MapError::BottomLevel) |
                    Err(MapError::HugeFrame(_)) => {
                        let index = self.next_index();
                        let current = &mut self.current()[index];
                        let result = (current.addr(), current.flags().contains(PageTableFlags::HUGE_PAGE));
                        return Some(result)
                    }
                Ok(_) => (),
                Err(_) => return None
            }
        }
    }
}

#[allow(dead_code)]
pub fn get_cr3() -> PhysAddr {
    x86_64::registers::control::Cr3::read().0.start_address()
}

#[cfg(test)]
mod test {

    use super::*;
    use super::super::PhysAddr;

    use crate::mm::get_phys_as_mut;

    /// Test the page table walker by following the address that maps to the start of physical
    /// memory. This is known to be a 2MB huge page mapped by kloader.
    #[test_case]
    fn test_pt_walker() {
        let phys_offset_vaddr = phys_to_virt(PhysAddr::new(0));
        let pml4 = x86_64::registers::control::Cr3::read().0;
        //let mm = memory_manager();
        let pt = unsafe {
            get_phys_as_mut(pml4.start_address()).unwrap()
        };

        let mut pt_walker = Mapper::new(phys_offset_vaddr, pt);

        let pml4_vaddr = phys_to_virt(pml4.start_address());
        assert_eq!(pt_walker.current() as *mut _ as u64, pml4_vaddr.as_u64());
        assert!(pt_walker.advance().is_ok(), "page level 2 panic");
        assert!(pt_walker.advance().is_ok(), "page level 2 panic");
        let walk_result = pt_walker.advance();
        assert!(walk_result.is_err(), "page level 1 should not exist because huge page");
        assert!(walk_result.unwrap_err() == MapError::HugeFrame(2), "Expected huge page");
    }

    #[test_case]
    fn test_page_table_copy() {
        let (test_page_table, _test_page_table_phys) = allocate_page_table();

        unsafe {
            let mut mapper = Mapper::new(VirtAddr::new(0x1000), test_page_table.as_mut().unwrap());
            // TODO: dependency injection for frame allocator in tests
            mapper.map(&mut *physical_memory_manager().lock()).expect("Could not create test mapping");

            drop(mapper);

            let mut mapper = Mapper::new(VirtAddr::new(0x1000), test_page_table.as_mut().unwrap());
            let (test_phys_frame, _) = mapper.get_phys_frame().unwrap();

            let copy_page_table = test_page_table.as_mut().unwrap().deep_copy();

            let mut copy_mapper = Mapper::new(VirtAddr::new(0x1000), copy_page_table.as_mut().unwrap());

            let (phys_frame, _is_huge) = copy_mapper.get_phys_frame().unwrap();

            assert_ne!(phys_frame.as_u64(), 0u64);
            assert_ne!(copy_page_table, test_page_table);
            assert_eq!(test_phys_frame, phys_frame);
        }

        // TODO: cleanup

    }
}
