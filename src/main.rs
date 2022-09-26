#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![feature(ptr_as_uninit)]
#![feature(abi_x86_interrupt)]
#![feature(map_first_last)]
#![feature(drain_filter)]
#![feature(thread_local)]
#![feature(asm_const)]
#![feature(asm_sym)]
#![feature(const_ptr_offset_from)]
#![feature(naked_functions)]
#![feature(ptr_internals)]

#![test_runner(test_runner)]

#![reexport_test_harness_main = "test_main"]
#![recursion_limit = "1024"]

#[macro_use]
extern crate alloc;

#[macro_use]
mod util;

mod acpi;
mod arch;
mod cpu;
mod common;
mod dev;
mod fs;
mod elf;
mod env;
mod error;
mod heap;
mod info;
mod interrupt;
mod log;
mod mm;
mod proc;
mod qemu;
mod stack;
mod syscall;
#[cfg(test)]
mod test;
mod time;

use core::panic::PanicInfo;

use libkloader::KernelInfo;
use x86_64::VirtAddr;
use ::log::info;

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
    static_assert(!bootinfo.is_null(), "Bootinfo nullptr!");
    let info;
    unsafe {
        stack::set_stack_start((*bootinfo).rsp);
        info = bootinfo.as_ref().expect("Nullptr dereferenced for bootinfo");
    }
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

    log::init();

    info!("Heap size: {}", heap_init_result.1 - heap_init_result.0);

    env::init(bootinfo);
    drop(bootinfo);

    println!("\n=== {} {} ===\n", info::KERNEL_NAME, info::KERNEL_VERSION);

    mm::init(heap_init_result);

    info!("{:#X?}", env::memory_layout());

    arch::x86_64::platform_init();

    interrupt::init().unwrap_or_else(|_| {
        println!("Unable to initialize interrupts");
    });

    fs::init();

    #[cfg(test)]
    test_main();

    proc::process_list_mut().spawn(idle);
    loop {
        syscall::yield_();
    }
}

pub fn idle() {
    proc::process_list_mut().spawn(|| {

        proc::process_list_mut().spawn(|| {
            println!("Hello world! I am {}! Now I will die!", proc::pid());
            proc::new_user_test();
            proc::exit(0)
        });

        loop {
            println!("Hello from pid: {}, core: {}", proc::pid(), arch::x86_64::apic_id());
            println!("Pids: {:?}", proc::process_list().pids());
            syscall::sleep(2);
        }
    });
    loop {
        println!("Hello from pid: {}, core: {}", proc::pid(), arch::x86_64::apic_id());
        syscall::sleep(5);
    }
}

pub fn ap_main() {
    println!("ap main reached. waiting for scheduler");
    loop {
        syscall::yield_();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Print KERNEL PANIC before even attempting to get core num so we know it's a panic even if we
    // fault in try_apic_id
    interrupt::disable_interrupts();
    proc::PANIC.store(true, core::sync::atomic::Ordering::SeqCst);
    dev::serial::force_serial_unlock();

    write_serial_out("KERNEL PANIC ");
    let core = arch::x86_64::try_apic_id();
    println!("on core {:?}: {}", core, info);

    #[cfg(test)]
    {
        use qemu::{exit_qemu, QemuExitCode};
        use ::log::error;

        error!("");
        error!("Unit test failed!");
        error!("{}", info);

        exit_qemu(QemuExitCode::Failed);
    }
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

