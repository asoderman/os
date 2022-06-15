use syscall::number::Syscall;

pub mod handlers;

pub use handlers::*;

pub fn syscall(a: usize, b: usize, c: usize, d: usize, si: usize, di: usize) -> usize {
    match a {
        Syscall::HELLO_WORLD => {
            println!("Syscall: hello world!");
            0
        },
        Syscall::SLEEP => sleep(b),
        Syscall::YIELD => yield_(),
        _ => usize::MAX
    }
}
