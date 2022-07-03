mod error;
pub mod frame_allocator;
mod mapping;
mod pmm;
mod region;
mod vmm;

use crate::arch::{PhysAddr, VirtAddr};
use crate::proc::Task;
use alloc::sync::Arc;
use log::trace;
use vmm::init_vmm;

pub use pmm::phys_to_virt;
pub use pmm::get_init_heap_section;
pub use error::MemoryManagerError;

use self::mapping::Mapping;
use self::pmm::physical_memory_manager;
use self::vmm::{VirtualMemoryError, get_kernel_context_virt};

pub use self::vmm::{VirtualRegion, AddressSpace};
pub use self::pmm::{write_physical, write_physical_slice, get_phys_as_mut};

type Error = MemoryManagerError;

type MapHandle = Arc<Mapping>;

/// Identity maps a physical page into the address space if it is available
fn identity_map(addr_space: &mut AddressSpace, paddr: PhysAddr) -> Result<MapHandle, Error> {
    let frame = physical_memory_manager().lock().request_frame(paddr)?;
    assert_eq!(frame, paddr);
    let mapping = mapping::Mapping::new_identity(frame);
    addr_space.insert_and_map(mapping, &mut *physical_memory_manager().lock()).map_err(|e| e.into())
}

/// Map the virtual region to any available memory within the provided address space
fn map(addr_space: &mut AddressSpace, vaddr: VirtAddr, pages: usize) -> Result<MapHandle, Error> {
    let region = VirtualRegion::new(vaddr, pages);
    let mapping = Mapping::new(region);
    addr_space.insert_and_map(mapping, &mut *physical_memory_manager().lock()).map_err(|e| e.into())
}

/// Map the provided virtual address to the provided physical address.
///
/// Caller must guarantee the physical address is valid!
unsafe fn map_mmio(addr_space: &mut AddressSpace, vaddr: VirtAddr, paddr: PhysAddr, size: usize) -> Result<MapHandle, Error> {
    let region = vmm::VirtualRegion::new(vaddr, size);
    let mapping = mapping::Mapping::new_mmio(region, paddr);
    addr_space.insert_and_map(mapping, &mut *physical_memory_manager().lock()).map_err(|e| e.into())
}

/// Unmaps the virtual address range from th address space and returns freed resources to the PMM
fn unmap(addr_space: &mut AddressSpace, vaddr: VirtAddr, pages: usize) -> Result<(), Error> {
    addr_space.release_region(vaddr, pages, &mut *physical_memory_manager().lock()).map_err(|e| e.into())
}

/// Map specified range to the current user address space
pub fn user_map(task: &mut Task, vaddr: VirtAddr, pages: usize) -> Result<MapHandle, Error> {
    trace!("[pid {}] Mapping {:?} to userspace", task.id, vaddr);
    let addr_space = task.address_space.as_mut().unwrap();
    let mapping = map(addr_space, vaddr, pages)?;
    mapping.mark_as_userspace(addr_space.page_table());
    Ok(mapping)
}

/// Map specified range to the current user address space
pub unsafe fn user_map_mmio_anywhere(task: &mut Task, paddr: PhysAddr, pages: usize) -> Result<MapHandle, Error> {
    let addr_space = task.address_space.as_mut().unwrap();
    let vaddr = addr_space
        .first_available_addr_above(VirtAddr::new(0), pages)
        .ok_or(VirtualMemoryError::NoAddressSpaceAvailable)?;
    trace!("[pid {}] Mapping {:?} to userspace", task.id, vaddr);
    let mapping = unsafe {
        map_mmio(addr_space, vaddr.start, paddr, pages)?
    };
    mapping.mark_as_userspace(addr_space.page_table());
    Ok(mapping)
}

pub fn user_unmap(task: &mut Task, vaddr: VirtAddr, pages: usize) -> Result<(), Error> {
    let addr_space = task.address_space.as_mut().unwrap();
    unmap(addr_space, vaddr, pages)
}

/// Identity maps a physical page into the kernel address space if it is available
pub fn k_identity_map(paddr: PhysAddr) -> Result<MapHandle, Error> {
    let mut kernel_address_space = get_kernel_context_virt().lock();
    identity_map(&mut kernel_address_space, paddr)
}

/// Map any writable page into the kernel address space at the specified address
pub fn kmap(vaddr: VirtAddr, pages: usize) -> Result<MapHandle, Error> {
    // TODO: verify address is in higher half 
    //assert!(vaddr.as_u64() >= 0xFFFFFF8000000000);
    let mut kernel_address_space = get_kernel_context_virt().lock();
    map(&mut kernel_address_space, vaddr, pages)
}

/// Unmaps the specified virtual address range from the kernel address space
pub fn kunmap(vaddr: VirtAddr, pages: usize) -> Result<(), Error> {
    //assert!(vaddr.as_u64() >= 0xFFFFFF8000000000);
    let mut kernel_address_space = get_kernel_context_virt().lock();
    unmap(&mut kernel_address_space, vaddr, pages).map_err(|e| e.into())
}

/// Identity map the provided physical address. Does not check if the memory is available for
/// use.
pub unsafe fn kmap_identity_mmio(paddr: PhysAddr, size: usize) -> Result<MapHandle, MemoryManagerError> {
    kmap_mmio(paddr, VirtAddr::new(paddr.as_u64()), size)
}

pub unsafe fn kmap_mmio(paddr: PhysAddr, vaddr: VirtAddr, size: usize) -> Result<MapHandle, MemoryManagerError> {
    let mut kernel_address_space = get_kernel_context_virt().lock();
    map_mmio(&mut kernel_address_space, vaddr, paddr, size)
}

pub unsafe fn kmap_mmio_anywhere(paddr: PhysAddr, size: usize) -> Result<VirtAddr, MemoryManagerError> {
    let mut addr_space = get_kernel_context_virt().lock();
    let region = addr_space.first_available_addr_above(vmm::mmio_area_start(), size).ok_or(VirtualMemoryError::NoAddressSpaceAvailable)?;

    map_mmio(&mut addr_space, region.start, paddr, size)?;

    Ok(region.start)
}

pub fn init(heap_range: (VirtAddr, VirtAddr)) {
    pmm::init(heap_range);
    println!("pmm is init");
    init_vmm();
    println!("vmm is init");
}


/// RAII guard for a temporary page. When this struct is dropped the page is unmapped.
#[derive(Debug)]
pub struct TempPageGuard(VirtAddr);

/*
impl Drop for TempPageGuard {
    fn drop(&mut self) {
        MM.lock().kunmap(self.0).unwrap();
    }
}
*/

#[cfg(test)]
mod test {
    use super::*;

    use crate::arch::x86_64::paging::Mapper;

    use super::vmm::get_kernel_context_virt;

    #[test_case]
    fn test_kmap_kunmap() {
        let test_addr = VirtAddr::new(0x10000);
        let test_size = 2;
        // take the free frames before touching any memory
        let pmm_frames = physical_memory_manager().lock().free_frames();

        // Drop the Result + Arc ptr after we're done
        {
            let kmap_result = kmap(test_addr, test_size);
            assert!(kmap_result.is_ok());
        }

        unsafe { 
            (test_addr.as_mut_ptr() as *mut bool).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool));
            (test_addr.as_mut_ptr() as *mut bool).add(0x1000).write(true);
            assert!(*(test_addr.as_mut_ptr() as *mut bool).add(0x1000));
        }

        let kunmap_result = kunmap(test_addr, test_size);

        assert!(kunmap_result.is_ok());

        let pmm_frames_after_unmap = physical_memory_manager().lock().free_frames();

        let mut kernel_as = get_kernel_context_virt().lock();
        let mut test_pt_walker = Mapper::new(test_addr, kernel_as.page_table());

        test_pt_walker.walk();
        assert!(test_pt_walker.next_entry().is_unused());
        assert_eq!(pmm_frames, pmm_frames_after_unmap);
    }
}
