use core::{marker::PhantomData, ptr::NonNull};

use alloc::vec::Vec;
use x86_64::{
    structures::paging::{page_table::PageTableEntry, PageTable, PageTableIndex, PageTableFlags},
    VirtAddr, PhysAddr,
};

use crate::mm::{MemoryManager, phys_to_virt};

const MAX_PAGE_ENTRIES: usize = 511;

pub struct Flusher(VirtAddr);

impl Drop for Flusher {
    fn drop(&mut self) {
        x86_64::instructions::tlb::flush(self.0);
    }
}

#[derive(Debug, PartialEq)]
pub enum MapError {
    /// Returned if at the end of the PT structure
    BottomLevel,
    /// Returned if at the top of the PT structure
    TopLevel,
    /// Returned if a huge page is encountered when attempting to descend the structure
    HugeFrame,
    /// Returned if attempting to advance on a non present entry.
    NotPresent,
    /// Returned when trying to create a new mapping on an existing entry. This can be ignored with
    /// `map_unchecked`.
    PresentEntry
}

/// A struct that manages the state for performing page table walks using the virtual address provided
pub struct Mapper<'a> {
    addr: VirtAddr,
    indices: [PageTableIndex; 4],
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

    /// The current page table leve the struct is operating on
    pub fn pt_level(&self) -> usize {
        self.current_level
    }

    /// Advance the walker to the next PageTable. Returns Error if unable to presently advance
    /// however the issue may be resolvable.
    pub fn advance(&mut self) -> Result<(), MapError> {
        if self.current_level == 1 { return Err(MapError::BottomLevel) }

        let index = self.next_index();
        let entry = &self.current()[index];

        if entry.is_unused() { return Err(MapError::NotPresent) }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) { return Err(MapError::HugeFrame) }

        let next_addr = entry.addr();
        let next_vaddr = phys_to_virt(next_addr);

        self.prev_ptrs.push(self.current_ptr);
        self.current_ptr = NonNull::new(next_vaddr.as_mut_ptr()).unwrap();
        self.current_level -= 1;
        Ok(())
    }

    /// Walk to the lowest page level.
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
        let i = u16::from(self.indices[4 - page_level]);
        let new = i + 1 % 512;

        self.indices[4 - page_level] = PageTableIndex::new(new);

        Ok(())
    }

    /// Get the current PageTable in the walk.
    #[inline]
    fn current(&mut self) -> &mut PageTable {
        unsafe {
            self.current_ptr.as_mut()
        }
    }

    /// Get an index for the current pagetable to retrieve the pagetable entry containing the next
    /// table.
    #[inline]
    fn next_index(&self) -> PageTableIndex {
        // TODO: PML5 support
        self.indices[4 - self.current_level]
    }

    /// Mutably borrow the entry in the current table that corresponds to the next table.
    #[inline]
    pub fn next_entry(&mut self) -> &mut PageTableEntry {
        let index = self.next_index();
        &mut self.current()[index]
    }

    /// Maps next entry and marks it present.
    ///
    /// returns Error if the entry is already marked present.
    pub fn map_next(&mut self, mm: &mut MemoryManager) -> Result<(), MapError> {
        let entry = self.next_entry();
        if !entry.is_unused() { return Err(MapError::PresentEntry); }

        let frame = mm.request_frame();
        entry.set_addr(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT);
        Ok(())
    }

    pub fn map_frame(&mut self, frame: PhysAddr, mm: &mut MemoryManager) -> Result<Flusher, MapError> {

        loop {
            // Ensure everything is writable by default
            self.next_entry().flags().set(PageTableFlags::WRITABLE, true);

            match self.advance() {
                Err(MapError::NotPresent) => {
                    if self.current_level != 1 {
                        self.map_next(mm)?
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
                    if !self.next_entry().flags().contains(PageTableFlags::WRITABLE) {
                        self.next_entry().set_flags(PageTableFlags::WRITABLE | PageTableFlags::PRESENT);
                        assert!(self.next_entry().flags().contains(PageTableFlags::WRITABLE));
                    }
                },
            }
        }
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

    /// Map the pages that are adjacent to the address provided.
    pub fn map_adjacent(&mut self, pages: usize, mm: &mut MemoryManager) {
        // TODO: this function will not work across page table boundaries. e.g. if
        // l1_pagetable[511] is mapped it will attempt to map l1_pagetable[512] and presumably
        // crash
        if self.current_level != 1 { return; }
        let l1_index = usize::from(self.indices.last().unwrap().clone());
        let range_end = l1_index + pages;
        // the range to be mapped excluding the already mapped page
        let map_range = (l1_index..(range_end)).skip(1);

        // If we need multiple l2_pages
        if range_end > MAX_PAGE_ENTRIES {
            let _next_l2_end = range_end % 512;
        }

        for i in map_range {
            let frame = mm.request_frame();
            // if we cross a page table boundary
            if i % 512 == 0 && i != 0 {
                self.ascend().unwrap();
                self.increase_index(2).unwrap();
                self.map_next(mm).unwrap();
                self.advance().unwrap();
            }
            // TODO: remove hardcoded flags
            self.current()[i % 512].set_addr(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use super::super::PhysAddr;

    use crate::mm::memory_manager;

    /// Test the page table walker by following the address that maps to the start of physical
    /// memory. This is known to be a 2MB huge page mapped by kloader.
    #[test_case]
    fn test_pt_walker() {
        let phys_offset_vaddr = phys_to_virt(PhysAddr::new(0));
        let pml4 = x86_64::registers::control::Cr3::read().0;
        let mm = memory_manager();
        let pt = unsafe {
            mm.get_phys_as_mut(pml4.start_address()).unwrap()
        };

        let mut pt_walker = Mapper::new(phys_offset_vaddr, pt);

        let pml4_vaddr = phys_to_virt(pml4.start_address());
        assert_eq!(pt_walker.current() as *mut _ as u64, pml4_vaddr.as_u64());
        assert!(pt_walker.advance().is_ok(), "page level 2 panic");
        assert!(pt_walker.advance().is_ok(), "page level 2 panic");
        let walk_result = pt_walker.advance();
        assert!(walk_result.is_err(), "page level 1 should not exist because huge page");
        assert!(walk_result.unwrap_err() == MapError::HugeFrame, "Expected huge page");
    }
}
