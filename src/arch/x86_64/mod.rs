pub const PAGE_SIZE: usize = 4096;

pub use x86_64::{PhysAddr, VirtAddr};

pub mod idt;
pub mod paging;
