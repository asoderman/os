use log::info;
use syscall::number::Syscall;

pub mod handlers;

pub use handlers::*;

#[allow(unused_variables)]
pub fn syscall(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    info!("Syscall no. : {:#X}", a);
    match a {
        Syscall::HELLO_WORLD => {
            println!("Syscall: hello world!");
            0
        },
        Syscall::SLEEP => sleep(b),
        Syscall::YIELD => yield_(),
        Syscall::EXIT => do_exit(b),
        Syscall::LOGPRINT => log_print(b as *const u8, c),
        _ => usize::MAX
    }
}
