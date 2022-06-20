use syscall::number::Syscall;

pub mod handlers;

pub use handlers::*;

#[allow(unused_variables)]
pub fn syscall(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    match a {
        Syscall::HELLO_WORLD => {
            println!("Syscall: hello world!");
            0
        },
        Syscall::SLEEP => sleep(b),
        Syscall::YIELD => yield_(),
        Syscall::EXIT => do_exit(b),
        _ => usize::MAX
    }
}
