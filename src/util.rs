use core::arch::asm;


#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::dev::serial::print(format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}
static mut STARTING_RSP: u64 = 0;

pub fn set_stack_start(rsp: u64) {
    unsafe { STARTING_RSP = rsp; }
}

#[inline]
pub fn print_stack_usage() {
    unsafe {
        println!("est stack usage: {:#X}", STARTING_RSP - get_rsp());
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
