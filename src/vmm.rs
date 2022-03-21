use core::ptr::NonNull;

use x86_64::structures::paging::{frame::PhysFrame, PageTable};

use crate::arch::x86_64::paging::pt_walk;
use crate::arch::x86_64::VirtAddr;

/*
pub fn k_map_to(frame: PhysFrame, addr: VirtAddr) -> Result<(), &'static str> {

}

#[must_use]
pub fn untracked_map(frame: PhysFrame, addr: VirtAddr) -> Result<(VirtAddr, usize), &'static str> {
    let mut pml4 = get_kernel_context();

   // pt_walk(addr, pml4.as_mut());
}
*/

fn get_kernel_context() -> NonNull<PageTable> {
    let pml4 = x86_64::registers::control::Cr3::read().0;

    NonNull::new(pml4.start_address().as_u64() as *mut _).unwrap()
}
