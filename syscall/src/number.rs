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

syscall_number!(OPEN, 0);
syscall_number!(CLOSE, 1);
syscall_number!(READ, 2);
syscall_number!(WRITE, 3);

syscall_number!(SLEEP, 4);
syscall_number!(YIELD, 5);
syscall_number!(EXIT, 6);

syscall_number!(LOGPRINT, 7);

syscall_number!(MMAP, 8);
syscall_number!(MUNMAP, 9);
syscall_number!(MPROTECT, 10);

syscall_number!(MKFILE, 11);
syscall_number!(MKDIR, 12);
syscall_number!(RMFILE, 13);
syscall_number!(RMDIR, 14);
syscall_number!(STAT, 15);
syscall_number!(EXECV, 16);
syscall_number!(CLONE, 17);
syscall_number!(MKFIFO, 18);
