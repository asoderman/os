use core::slice::from_raw_parts;

use crate::proc::{process_list, yield_time, exit};
use crate::interrupt::{without_interrupts, disable_interrupts};

pub fn sleep(seconds: usize) -> usize {
    without_interrupts(|| {
        {
            let current = process_list().current();

            current.write().sleep_for(seconds);
            drop(current)
        }

        yield_time();
    });

    log::trace!("Sleep done");
    0
}

pub fn yield_() -> usize {
    without_interrupts(|| {
        yield_time()
    });

    0
}

pub fn do_exit(status: usize) -> usize {
    disable_interrupts();

    exit(status);

    0
}

pub fn log_print(ptr: *const u8, len: usize) -> usize {
    log::info!("user_put: {:p} len {}", ptr, len);
    let char_slice = unsafe { from_raw_parts(ptr, len) };
    let string = core::str::from_utf8(char_slice);
    log::info!("{:?}", string);

    0
}
