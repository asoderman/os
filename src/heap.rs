use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
    sync::atomic::Ordering, };
use libkloader::MemoryMapInfo;
use spin::{Mutex, MutexGuard};
use crate::arch::VirtAddr;

use crate::{dev::serial::write_serial_out, mm::get_init_heap_section};

use linked_list_allocator::LockedHeap;

// TODO: Implement slab allocator but for now use linked_list_allocator crate
#[global_allocator]
static ALLOC: LockedHeap = LockedHeap::empty();

struct LockedAllocator<A> {
    pub a: Mutex<A>,
}

impl<A> LockedAllocator<A> {
    pub const fn new(allocator: A) -> Self {
        LockedAllocator {
            a: Mutex::new(allocator),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, A> {
        self.a.lock()
    }
}

unsafe impl GlobalAlloc for LockedAllocator<TinyAlloc> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.a.lock().allocate(layout).as_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.a
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

#[alloc_error_handler]
fn alloc_error_panic(_info: Layout) -> ! {
    panic!("Alloc error occured: \n{:?}", _info);
}


struct TinyAlloc {
    start_addr: usize,
    used: usize,
    size: usize,
    initialized: bool,
}

impl TinyAlloc {
    const fn uninit() -> Self {
        TinyAlloc {
            start_addr: 0,
            used: 0,
            size: 0,
            initialized: false,
        }
    }

    pub fn init(&mut self, start: VirtAddr, end: VirtAddr) {
        assert!(!self.initialized);

        self.start_addr = start.as_u64() as usize;
        self.size = (end - start) as usize;
        self.initialized = true;
        write_serial_out("heap init complete\n");
    }
}

impl TinyAlloc {
    fn allocate(&mut self, layout: Layout) -> NonNull<u8> {
        if !self.initialized {
            write_serial_out("Tried to allocate before heap initialized");
            panic!("Tried to allocate before heap initialized");
        }

        let size = layout.size();
        let align = layout.align();
        let used = self.used;
        let ptr = unsafe { (self.start_addr as *mut u8).offset(used as isize) };

        let align_needed = ptr.align_offset(align);

        self.used += size + align_needed;

        unsafe { NonNull::new(ptr.offset(align_needed as isize)).unwrap() }
    }

    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {}
}

#[derive(Clone, Debug)]
pub struct HeapInitError(&'static str);

impl HeapInitError {
    pub fn as_str(&self) -> &str {
        self.0
    }
}

pub fn init_heap(
    mem_map: MemoryMapInfo,
    phys_offset: VirtAddr,
) -> Result<(VirtAddr, VirtAddr), HeapInitError> {
    let heap_frame_range = get_init_heap_section(16, mem_map).map_err(|e| HeapInitError(e))?;

    let heap_start = phys_offset.as_u64() + heap_frame_range.start.start_address().as_u64();
    let heap_end_exclusive = phys_offset + heap_frame_range.end.start_address().as_u64();
    let heap_size = heap_end_exclusive.as_u64() - heap_start;

    write_serial_out("initializing heap...\n");

    unsafe {
        ALLOC.lock().init(heap_start as usize, heap_size as usize);
    }
    write_serial_out("initializing heap returned \n");

    Ok((VirtAddr::new(heap_start), heap_end_exclusive))
}

mod test {
    use alloc::vec::Vec;

    #[test_case]
    fn test_small_alloc() {
        let v = alloc::vec![5u64; 10];

        assert_eq!(v.len(), 10);
        for i in v {
            assert_eq!(i, 5);
        }
    }

    #[test_case]
    fn test_large_alloc() {
        let v = alloc::vec![5u64; 1000];

        assert_eq!(v.len(), 1000);
        for i in v {
            assert_eq!(i, 5);
        }
    }

    #[test_case]
    fn test_many_small_alloc() {
        let mut v = Vec::new();

        for _ in 0..150 {
            let v2 = alloc::vec![5u8; 10];

            v.push(v2);
        }

        v.clear();

        for _ in 0..100 {
            let v2 = alloc::vec![5u8; 10];

            v.push(v2);
        }
    }
}
