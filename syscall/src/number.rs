
macro_rules! syscall_number {
    ($name:ident, $num:literal) => {
        impl Syscall {
            pub const $name: usize = $num;
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Syscall {}

syscall_number!(HELLO_WORLD, 0);
syscall_number!(SLEEP, 1);
syscall_number!(YIELD, 2);
syscall_number!(EXIT, 3);
