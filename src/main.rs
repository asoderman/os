#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![feature(ptr_as_uninit)]
#![test_runner(test_runner)]

#![reexport_test_harness_main = "test_main"]

extern crate alloc;

mod arch;
mod dev;
mod heap;
mod pmm;
mod qemu;
mod vmm;
#[cfg(test)]
mod test;
mod util;

use core::panic::PanicInfo;

use libkloader::{KernelInfo, VideoInfo};
use x86_64::VirtAddr;

use dev::serial::write_serial_out;
use heap::init_heap;
use pmm::init_pmm;

fn static_assert(b: bool, msg: &str) {
    if !b {
        write_serial_out(msg);
    }
    assert!(b);
}

#[no_mangle]
extern "C" fn start(bootinfo: *const KernelInfo) {
    let info = unsafe { core::ptr::read_unaligned(bootinfo) };
    main(&info);
}

fn main(bootinfo: &KernelInfo) {
    static_assert(!bootinfo.mem_map_info.start.is_null(), "Mem map null ptr");

    let heap_init_result =
        init_heap(bootinfo.mem_map_info, VirtAddr::new(bootinfo.phys_offset))
        .unwrap_or_else(|e| {
            write_serial_out(e.as_str());
            panic!();
        });

    println!("Heap size: {}", heap_init_result.1 - heap_init_result.0);

    init_pmm(heap_init_result);

    #[cfg(test)]
    test_main();

    println!("prinln! test: {}", "Hello world!");
    println!("prinln! test2: {}", 2);

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("KERNEL PANIC: {}", info);
    loop {}
}

#[cfg(test)]
fn test_runner(tests: &[&dyn test::Test]) {
    println!("\nRunning {} tests", tests.len());
    for test in tests {
        test.run();
    }

    crate::qemu::exit_qemu(crate::qemu::QemuExitCode::Success);
}

