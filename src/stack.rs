use spin::Once;

use crate::mm::kmap;
use crate::mm::kunmap;
use crate::println;
use crate::arch::VirtAddr;
use crate::arch::PAGE_SIZE;

use core::arch::asm;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

/// The starting kernel stack. On x64 this is the initial value of rsp
static STARTING_KERNEL_STACK_ADDR: Once<u64> = Once::new();

const KERNEL_STACK_SIZE: usize = 4;
const KERNEL_STACK_SIZE_BYTES: usize = KERNEL_STACK_SIZE * PAGE_SIZE;

#[derive(Debug)]
pub struct KernelStack {
    base: VirtAddr,
    pages: usize
}

impl KernelStack {
    pub fn new() -> Self {
        let kernel_stack_number = KERNEL_STACKS_ALLOCATED.fetch_add(1, Ordering::AcqRel);
        let starting_stack_addr = STARTING_KERNEL_STACK_ADDR.get().copied().unwrap();

        let new_stack_base = 
            VirtAddr::new(starting_stack_addr + (KERNEL_STACK_SIZE * PAGE_SIZE * kernel_stack_number) as u64).align_up(PAGE_SIZE as u64);

        kmap(new_stack_base, KERNEL_STACK_SIZE).expect("Could not map new stack");

        Self {
            base: new_stack_base,
            pages: KERNEL_STACK_SIZE
        }
    }

    /// Creates a stack used to initialize an ap. The object is never instantiated but we do not
    /// want our new stack to be unmapped.
    #[allow(dead_code)]
    pub fn new_init() -> VirtAddr {
        let init_stack = Self::new();
        let rsp = init_stack.top();

        core::mem::forget(rsp);

        rsp
    }

    /// Returns a `VirtAddr` to the top of the stack.
    pub fn top(&self) -> VirtAddr {
        self.base + (self.pages * PAGE_SIZE)
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        kunmap(self.base, self.pages).unwrap();
    }
}

pub fn set_stack_start(rsp: u64) {
    STARTING_KERNEL_STACK_ADDR.call_once(|| rsp);
}

#[allow(dead_code)]
pub fn print_stack_usage() {
    println!("est stack usage: {:#X}, {:X}", STARTING_KERNEL_STACK_ADDR.get().copied().unwrap() - get_rsp(), get_rsp());
}

#[allow(dead_code)]
pub fn get_rsp() -> u64 {
    let rsp;
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp);
    }
    rsp
}

static KERNEL_STACKS_ALLOCATED: AtomicUsize = AtomicUsize::new(1);

/// Allocates a new kernel stack in the higher half.
///
/// # Returns
/// the end of the kernel stack i.e. that stack ptr
pub fn allocate_kernel_stack() -> VirtAddr {
    let kernel_stack_number = KERNEL_STACKS_ALLOCATED.fetch_add(1, Ordering::AcqRel);
    let starting_stack_addr = STARTING_KERNEL_STACK_ADDR.get().copied().unwrap();

    let new_stack_base = 
        VirtAddr::new(starting_stack_addr + (KERNEL_STACK_SIZE * PAGE_SIZE * kernel_stack_number) as u64).align_up(PAGE_SIZE as u64);

    println!("Allocating stack at base: {:?}", new_stack_base);
    kmap(new_stack_base, KERNEL_STACK_SIZE_BYTES / PAGE_SIZE).expect("Could not map new stack");


    println!("Returning new rsp: {:?}", new_stack_base + KERNEL_STACK_SIZE_BYTES);
    new_stack_base + (KERNEL_STACK_SIZE_BYTES as u64)
}
