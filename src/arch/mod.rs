pub mod x86_64;

pub use self::x86_64::{VirtAddr, PhysAddr};
pub use self::x86_64::PAGE_SIZE;

#[macro_export]
macro_rules! interrupt {
    ($num:expr, $isr:ident) => {
        #[cfg(target_arch="x86_64")]
        impl_and_register_x86_interrupt!($num, $isr)
    }
}
