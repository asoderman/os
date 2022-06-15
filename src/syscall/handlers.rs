use crate::proc::{process_list, yield_time, exit};
use crate::interrupt::{without_interrupts, disable_interrupts};

pub fn sleep(seconds: usize) -> usize {
    without_interrupts(|| {
        let current = process_list().current();

        current.write().sleep_for(seconds);

        yield_time();
    });

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
