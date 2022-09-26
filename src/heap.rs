use core::alloc::Layout;
use libkloader::MemoryMapInfo;
use spin::Once;
use crate::arch::VirtAddr;

use crate::{dev::serial::write_serial_out, mm::get_init_heap_section};

use linked_list_allocator::LockedHeap;

// TODO: Implement slab allocator but for now use linked_list_allocator crate
#[global_allocator]
static ALLOC: LockedHeap = LockedHeap::empty();

pub static HEAP_READY: Once = Once::new();

#[alloc_error_handler]
fn alloc_error_panic(_info: Layout) -> ! {
    panic!("Alloc error occured: \n{:?}", _info);
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
    let heap_frame_range = get_init_heap_section(32, mem_map).map_err(|e| HeapInitError(e))?;

    let heap_start = phys_offset.as_u64() + heap_frame_range.start.start_address().as_u64();
    let heap_end_exclusive = phys_offset + heap_frame_range.end.start_address().as_u64();
    let heap_size = heap_end_exclusive.as_u64() - heap_start;

    write_serial_out("initializing heap...\n");

    unsafe {
        ALLOC.lock().init(heap_start as usize, heap_size as usize);
    }

    HEAP_READY.call_once(|| ());

    Ok((VirtAddr::new(heap_start), heap_end_exclusive))
}

#[cfg(test)]
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
