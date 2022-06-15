
macro_rules! interrupt_num {
    ($name:ident, $num:literal) => {
        #[allow(non_snake_case)]
        #[allow(non_upper_case_globals)]
        impl Interrupt {
            pub const $name: u8 = $num;
        }
    }
}

/// This struct contains definitions for each interrupt number. It functions as a quasi enum but is
/// really defined as a const usize to allow easy conversions.
pub struct Interrupt {}

interrupt_num!(DivideError, 0);
interrupt_num!(Debug, 0x1);
interrupt_num!(Breakpoint, 0x3);
interrupt_num!(PageFault, 0xe);
interrupt_num!(Timer, 32);
interrupt_num!(Syscall, 0x80);
