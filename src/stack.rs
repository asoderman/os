use crate::println;
use crate::arch::VirtAddr;
use crate::arch::PAGE_SIZE;

use core::arch::asm;

/// The starting kernel stack. On x64 this is the initial value of rsp
static mut STARTING_KERNEL_STACK_ADDR: u64 = 0;

const KERNEL_STACK_SIZE: usize = 4;
const KERNEL_STACK_SIZE_BYTES: usize = KERNEL_STACK_SIZE * PAGE_SIZE;

pub fn set_stack_start(rsp: u64) {
    unsafe { STARTING_KERNEL_STACK_ADDR = rsp; }
}

#[inline]
pub fn print_stack_usage() {
    unsafe {
        println!("est stack usage: {:#X}", STARTING_KERNEL_STACK_ADDR - get_rsp());
    }
}

#[inline]
pub fn get_rsp() -> u64 {
    let rsp;
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp);
    }
    rsp
}

static mut KERNEL_STACKS_ALLOCATED: usize = 1;

/// Allocates a new kernel stack in the higher half.
///
/// # Returns
/// the end of the kernel stack i.e. that stack ptr
pub fn allocate_kernel_stack() -> VirtAddr {
    let new_stack_base = unsafe {
        VirtAddr::new(STARTING_KERNEL_STACK_ADDR + (KERNEL_STACK_SIZE * PAGE_SIZE * KERNEL_STACKS_ALLOCATED) as u64).align_up(PAGE_SIZE as u64)
    };

    crate::println!("Allocating stack at base: {:?}", new_stack_base);
    crate::mm::memory_manager().kmap(new_stack_base, KERNEL_STACK_SIZE_BYTES / PAGE_SIZE).expect("Could not map new stack");

    unsafe {
        // TODO: Implement a real kernel stack allocator
        KERNEL_STACKS_ALLOCATED += 1;
    }

    crate::println!("Returning new rsp: {:?}", new_stack_base + KERNEL_STACK_SIZE_BYTES);
    new_stack_base + (KERNEL_STACK_SIZE_BYTES as u64)
}
