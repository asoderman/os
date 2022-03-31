#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![feature(ptr_as_uninit)]
#![feature(abi_x86_interrupt)]
#![feature(map_first_last)]
#![test_runner(test_runner)]

#![reexport_test_harness_main = "test_main"]

extern crate alloc;

mod acpi;
mod arch;
mod dev;
mod heap;
mod info;
mod interrupt;
mod mm;
mod qemu;
#[cfg(test)]
mod test;
mod traits;
#[macro_use]
mod util;

use core::panic::PanicInfo;

use libkloader::{KernelInfo, VideoInfo};
use x86_64::VirtAddr;

use dev::serial::write_serial_out;
use heap::init_heap;

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
    util::set_stack_start(bootinfo.rsp);

    let heap_init_result =
        init_heap(bootinfo.mem_map_info, VirtAddr::new(bootinfo.phys_offset))
        .unwrap_or_else(|e| {
            write_serial_out(e.as_str());
            panic!();
        });

    println!("=== {} {} ===\n", info::KERNEL_NAME, info::KERNEL_VERSION);
    println!("start RSP: {:#X}", bootinfo.rsp);
    println!("RSP after heap init: {:#X}", util::get_rsp());
    println!("est stack usage: {:#X}", bootinfo.rsp - util::get_rsp());
    println!("Heap size: {}", heap_init_result.1 - heap_init_result.0);

    mm::init(heap_init_result, bootinfo);
    println!("Initializing interrupts");
    println!("est stack usage: {:#X}", bootinfo.rsp - util::get_rsp());
    interrupt::init().unwrap_or_else(|_| {
        println!("Unable to initialize interrupts");
    });


    #[cfg(test)]
    test_main();

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

